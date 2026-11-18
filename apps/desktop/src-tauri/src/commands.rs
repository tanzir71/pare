//! Thin IPC layer over pare-core (ARCHITECTURE §5). The webview can only name findings by id;
//! paths and disposals always come from the scan result held here, never from the UI.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pare_core::dispose::executor::{CleanOptions, CleanProgress, CleanReport};
use pare_core::dispose::recycle::{PurgeReport, RestoreReport};
use pare_core::dispose::{OpLog, clean as core_clean};
use pare_core::history::{self, HistoryDetail, HistoryOp};
use pare_core::platform::{self, Drive};
use pare_core::scan::progress::{Progress, ProgressSnapshot};
use pare_core::scanner::{
    DetectorStatus, ScanConfig, ScanEvent, ScanOutput, ScanRequest, ScanRoot, scan,
    summarize,
};
use pare_core::settings::Settings;
use pare_core::{CategoryId, CategorySummary, Disposal, Finding, ProtectReason, Tier, Timestamp};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    core_version: String,
    platform: String,
    log_dir: PathBuf,
    settings_path: PathBuf,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        core_version: pare_core::VERSION.to_string(),
        platform: std::env::consts::OS.to_string(),
        log_dir: state.env.oplog_path().parent().map(|p| p.to_path_buf()).unwrap_or_default(),
        settings_path: state.env.settings_path(),
    }
}

#[tauri::command]
pub fn list_drives() -> Vec<Drive> {
    platform::list_drives()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownFolders {
    home: PathBuf,
    desktop: PathBuf,
    downloads: PathBuf,
    documents: PathBuf,
    pictures: PathBuf,
}

#[tauri::command]
pub fn known_folders(state: State<'_, AppState>) -> KnownFolders {
    let e = &state.env;
    KnownFolders {
        home: e.home.clone(),
        desktop: e.desktop.clone(),
        downloads: e.downloads.clone(),
        documents: e.documents.clone(),
        pictures: e.pictures.clone(),
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload {
    scan_id: u64,
    #[serde(flatten)]
    snapshot: ProgressSnapshot,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EventPayload {
    scan_id: u64,
    event: ScanEvent,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedCount {
    reason: ProtectReason,
    count: u64,
}

/// What Home needs after a scan.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    scan_id: u64,
    running: bool,
    started: Timestamp,
    finished: Timestamp,
    cancelled: bool,
    roots: Vec<ScanRoot>,
    categories: Vec<CategorySummary>,
    files: u64,
    elapsed_ms: u64,
    protected: Vec<ProtectedCount>,
    detectors: Vec<DetectorStatus>,
}

fn summary_of(id: u64, out: &ScanOutput, detectors: &[DetectorStatus]) -> ScanSummary {
    let mut protected: Vec<ProtectedCount> = Vec::new();
    for f in &out.result.findings {
        if let Tier::Protected { reason, .. } = &f.tier {
            match protected.iter_mut().find(|p| p.reason == *reason) {
                Some(p) => p.count += 1,
                None => protected.push(ProtectedCount {
                    reason: *reason,
                    count: 1,
                }),
            }
        }
    }
    ScanSummary {
        scan_id: id,
        running: false,
        started: out.result.started,
        finished: out.result.finished,
        cancelled: out.result.cancelled,
        roots: out.result.roots.clone(),
        categories: out.result.categories.clone(),
        files: out.result.stats.files,
        elapsed_ms: out.result.stats.elapsed_ms,
        protected,
        detectors: detectors.to_vec(),
    }
}

fn config_from(settings: &Settings, rules: Arc<pare_core::rules::RuleSet>) -> ScanConfig {
    ScanConfig {
        thresholds: settings.thresholds.clone(),
        exclusions: settings.exclusions.clone(),
        check_in_use: true,
        include_recycle_bin: true,
        now: None,
        rules: Some(rules),
    }
}

/// "Send caches to the Recycle Bin too": every direct delete becomes a recycle.
fn apply_disposal_preference(out: &mut ScanOutput, settings: &Settings) {
    if !settings.caches_to_recycle_bin {
        return;
    }
    for f in &mut out.result.findings {
        if f.disposal == Disposal::Delete {
            f.disposal = Disposal::Recycle;
        }
    }
    out.result.categories = summarize(&out.result.findings);
}

#[tauri::command]
pub fn scan_start(app: AppHandle, state: State<'_, AppState>, request: ScanRequest) -> CmdResult<u64> {
    if request.roots.is_empty() {
        return Err("Pick at least one place to scan.".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(Progress::default());
    let id = {
        let mut slot = state.scan.lock().map_err(err)?;
        if let Some(c) = &slot.cancel {
            c.store(true, Ordering::Relaxed);
        }
        slot.id += 1;
        slot.running = true;
        slot.cancel = Some(cancel.clone());
        slot.progress = Some(progress.clone());
        slot.detectors.clear();
        slot.id
    };
    let env = state.env.clone();
    let settings = state.settings.lock().map_err(err)?.clone();
    let cfg = config_from(&settings, state.rules.clone());
    let done = Arc::new(AtomicBool::new(false));

    // Progress sampler: 4 events per second, independent of the walker.
    {
        let app = app.clone();
        let progress = progress.clone();
        let done = done.clone();
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                let _ = app.emit(
                    "scan://progress",
                    ProgressPayload {
                        scan_id: id,
                        snapshot: progress.snapshot(),
                    },
                );
                std::thread::sleep(Duration::from_millis(250));
            }
        });
    }

    std::thread::spawn(move || {
        use tauri::Manager;
        let state = app.state::<AppState>();
        let detectors: Mutex<Vec<DetectorStatus>> = Mutex::new(Vec::new());
        let on_event = |ev: ScanEvent| {
            if let ScanEvent::Detector(d) = &ev {
                if let Ok(mut v) = detectors.lock() {
                    match v.iter_mut().find(|x| x.id == d.id) {
                        Some(x) => *x = d.clone(),
                        None => v.push(d.clone()),
                    }
                }
            }
            let _ = app.emit("scan://event", EventPayload { scan_id: id, event: ev });
        };
        let mut out = scan(&request, &cfg, &env, &progress, &cancel, &on_event);
        apply_disposal_preference(&mut out, &settings);
        done.store(true, Ordering::Relaxed);
        let _ = app.emit(
            "scan://progress",
            ProgressPayload {
                scan_id: id,
                snapshot: progress.snapshot(),
            },
        );
        let dets = detectors.lock().map(|d| d.clone()).unwrap_or_default();
        let summary = summary_of(id, &out, &dets);
        if let Ok(mut slot) = state.scan.lock() {
            if slot.id == id {
                slot.running = false;
                slot.detectors = dets;
                if !out.result.cancelled || slot.output.is_none() {
                    slot.output = Some(Arc::new(out));
                }
            }
        }
        let _ = app.emit("scan://done", summary);
    });
    Ok(id)
}

#[tauri::command]
pub fn scan_cancel(state: State<'_, AppState>, scan_id: u64) -> CmdResult<()> {
    let slot = state.scan.lock().map_err(err)?;
    if slot.id == scan_id {
        if let Some(c) = &slot.cancel {
            c.store(true, Ordering::Relaxed);
        }
    }
    Ok(())
}

/// The current state: a running scan's id, or the last finished scan's summary.
#[tauri::command]
pub fn scan_summary(state: State<'_, AppState>) -> CmdResult<Option<ScanSummary>> {
    let slot = state.scan.lock().map_err(err)?;
    let mut s = slot.output.as_ref().map(|o| summary_of(slot.id, o, &slot.detectors));
    if slot.running {
        if let Some(s) = &mut s {
            s.running = true;
        }
    }
    Ok(s)
}

fn with_output<T>(state: &State<'_, AppState>, f: impl FnOnce(&ScanOutput) -> T) -> CmdResult<T> {
    let slot = state.scan.lock().map_err(err)?;
    let out = slot.output.as_ref().ok_or("No scan yet.")?;
    Ok(f(out))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDetail {
    summary: Option<CategorySummary>,
    findings: Vec<Finding>,
}

#[tauri::command]
pub fn category_get(state: State<'_, AppState>, id: CategoryId) -> CmdResult<CategoryDetail> {
    with_output(&state, |out| {
        let mut findings: Vec<Finding> = out
            .result
            .findings
            .iter()
            .filter(|f| f.category == id)
            .cloned()
            .collect();
        findings.sort_by_key(|f| std::cmp::Reverse(f.bytes));
        CategoryDetail {
            summary: out.result.categories.iter().find(|c| c.info.id == id).cloned(),
            findings,
        }
    })
}

#[tauri::command]
pub fn finding_get(state: State<'_, AppState>, id: String) -> CmdResult<Option<Finding>> {
    with_output(&state, |out| out.result.findings.iter().find(|f| f.id == id).cloned())
}

#[tauri::command]
pub fn protected_list(state: State<'_, AppState>) -> CmdResult<Vec<Finding>> {
    with_output(&state, |out| {
        out.result
            .findings
            .iter()
            .filter(|f| f.tier.is_protected())
            .cloned()
            .collect()
    })
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CleanDone {
    report: CleanReport,
    drives_before: Vec<Drive>,
    drives_after: Vec<Drive>,
}

/// Dispose of the selected findings (by id). Runs in the background; emits `clean://progress`
/// and `clean://done`.
#[tauri::command]
pub fn clean(app: AppHandle, state: State<'_, AppState>, ids: Vec<String>) -> CmdResult<()> {
    if state.cleaning.swap(true, Ordering::SeqCst) {
        return Err("A cleanup is already running.".into());
    }
    let (findings, scan_started) = match with_output(&state, |out| {
        let f: Vec<Finding> = out
            .result
            .findings
            .iter()
            .filter(|f| ids.contains(&f.id))
            .cloned()
            .collect();
        (f, out.result.started)
    }) {
        Ok(v) => v,
        Err(e) => {
            state.cleaning.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };
    let env = state.env.clone();
    let exclusions = state.settings.lock().map(|s| s.exclusions.clone()).unwrap_or_default();
    std::thread::spawn(move || {
        use tauri::Manager;
        let state = app.state::<AppState>();
        let drives_before = platform::list_drives();
        let report = match OpLog::open(&env.oplog_path()) {
            Ok(log) => {
                let mut opts = CleanOptions::new(scan_started);
                opts.exclusions = exclusions;
                let app2 = app.clone();
                core_clean(&findings, &env, &log, &opts, &move |p: CleanProgress| {
                    let _ = app2.emit("clean://progress", p);
                })
            }
            Err(e) => {
                let _ = app.emit("clean://error", format!("Couldn't open the operation log: {e}"));
                state.cleaning.store(false, Ordering::SeqCst);
                return;
            }
        };
        let drives_after = platform::list_drives();
        // Drop what's gone from the current result so Home reflects it.
        if let Ok(mut slot) = state.scan.lock() {
            if let Some(out) = slot.output.as_mut() {
                let done: Vec<&str> = report
                    .results
                    .iter()
                    .filter(|r| {
                        matches!(r.status, pare_core::dispose::oplog::ItemStatus::Ok)
                            || r.reason == Some(pare_core::dispose::executor::FailReason::NotFound)
                    })
                    .map(|r| r.finding_id.as_str())
                    .collect();
                let mut o = (**out).clone();
                o.result.findings.retain(|f| !done.contains(&f.id.as_str()));
                o.result.categories = summarize(&o.result.findings);
                *out = Arc::new(o);
            }
        }
        state.cleaning.store(false, Ordering::SeqCst);
        let _ = app.emit(
            "clean://done",
            CleanDone {
                report,
                drives_before,
                drives_after,
            },
        );
    });
    Ok(())
}

#[tauri::command]
pub fn history_list(state: State<'_, AppState>) -> Vec<HistoryOp> {
    history::list(&state.env.oplog_path())
}

#[tauri::command]
pub fn history_get(state: State<'_, AppState>, op: String) -> Option<HistoryDetail> {
    history::get(&state.env.oplog_path(), &op)
}

#[tauri::command]
pub fn history_restore(
    state: State<'_, AppState>,
    op: String,
    items: Option<Vec<String>>,
) -> CmdResult<RestoreReport> {
    let log = OpLog::open(&state.env.oplog_path()).map_err(err)?;
    Ok(pare_core::dispose::recycle::restore(&log, &op, items.as_deref()))
}

#[tauri::command]
pub fn recycle_purge_pare_items(
    state: State<'_, AppState>,
    op: Option<String>,
    older_than_days: Option<u32>,
) -> CmdResult<PurgeReport> {
    let log = OpLog::open(&state.env.oplog_path()).map_err(err)?;
    Ok(pare_core::dispose::recycle::purge_pare_items(&log, op.as_deref(), older_than_days))
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> CmdResult<Settings> {
    Ok(state.settings.lock().map_err(err)?.clone())
}

#[tauri::command]
pub fn settings_set(state: State<'_, AppState>, patch: serde_json::Value) -> CmdResult<Settings> {
    let mut s = state.settings.lock().map_err(err)?;
    let next = s.patch(patch)?;
    next.save(&state.env).map_err(err)?;
    *s = next.clone();
    Ok(next)
}

/// "Always keep this": add an exclusion and protect matching findings right away.
#[tauri::command]
pub fn exclusion_add(state: State<'_, AppState>, path: PathBuf) -> CmdResult<Settings> {
    let next = {
        let mut s = state.settings.lock().map_err(err)?;
        if !s.exclusions.iter().any(|p| p == &path) {
            s.exclusions.push(path.clone());
        }
        s.save(&state.env).map_err(err)?;
        s.clone()
    };
    let mut slot = state.scan.lock().map_err(err)?;
    if let Some(out) = slot.output.as_mut() {
        let mut o = (**out).clone();
        for f in &mut o.result.findings {
            if f.paths.iter().any(|p| platform::path_starts_with(p, &path) || platform::path_starts_with(&path, p)) {
                f.tier = std::mem::replace(&mut f.tier, Tier::Safe)
                    .downgrade(Tier::protected(ProtectReason::Excluded));
            }
        }
        o.result.categories = summarize(&o.result.findings);
        *out = Arc::new(o);
    }
    Ok(next)
}

#[tauri::command]
pub fn open_in_explorer(path: PathBuf) -> CmdResult<()> {
    if platform::stat(&path).is_err() {
        return Err("That item no longer exists.".into());
    }
    #[cfg(windows)]
    {
        let mut arg = std::ffi::OsString::from("/select,");
        arg.push(path.as_os_str());
        std::process::Command::new("explorer.exe").arg(arg).spawn().map_err(err)?;
    }
    #[cfg(not(windows))]
    {
        let dir = path.parent().unwrap_or(&path);
        std::process::Command::new("xdg-open").arg(dir).spawn().map_err(err)?;
    }
    Ok(())
}

/// Background housekeeping at startup: empty Pare's own Recycle Bin items older than the
/// configured age (only items Pare moved).
pub fn startup_housekeeping(state: &AppState) {
    let days = state.settings.lock().ok().and_then(|s| s.purge_pare_items_after_days);
    let Some(days) = days else { return };
    let path = state.env.oplog_path();
    if !path.exists() {
        return;
    }
    std::thread::spawn(move || {
        if let Ok(log) = OpLog::open(&path) {
            let r = pare_core::dispose::recycle::purge_pare_items(&log, None, Some(days));
            if r.items > 0 {
                tracing::info!(items = r.items, bytes = r.bytes, "purged old Pare items from the Recycle Bin");
            }
        }
    });
}
