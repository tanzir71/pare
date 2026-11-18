//! Clean pipeline (ARCHITECTURE §4.2): re-validate → log "pending" → execute → log results.
//! Nothing is removed without an explicit selection, and every disposal is logged first.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::oplog::{ItemStatus, LogRecord, OpLog};
use crate::env::Env;
use crate::format::{format_bytes, format_count};
use crate::platform::{self, now_ms, path_starts_with};
use crate::safety::protected_paths;
use crate::scan::measure::measure_path;
use crate::types::{CategoryId, Disposal, Finding, MS_PER_HOUR, ProtectReason, Timestamp};

#[derive(Clone, Debug)]
pub struct CleanOptions {
    /// When the scan that produced these findings started. Anything modified later is skipped.
    pub scan_started: Timestamp,
    pub recent_ms: i64,
    pub exclusions: Vec<PathBuf>,
    pub check_in_use: bool,
    pub command_timeout: Duration,
    /// Shown before the History title, e.g. "Scheduled cleanup".
    pub label: Option<String>,
}

impl CleanOptions {
    pub fn new(scan_started: Timestamp) -> CleanOptions {
        CleanOptions {
            scan_started,
            recent_ms: 24 * MS_PER_HOUR,
            exclusions: Vec::new(),
            check_in_use: true,
            command_timeout: Duration::from_secs(300),
            label: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FailReason {
    InUse,
    AccessDenied,
    NotFound,
    Protected,
    ChangedSinceScan,
    CommandFailed,
    LogUnavailable,
    Other,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CleanItemResult {
    pub finding_id: String,
    pub title: String,
    pub category: CategoryId,
    pub path: PathBuf,
    pub disposal: String,
    pub status: ItemStatus,
    pub freed: u64,
    pub recycled: u64,
    pub error: Option<String>,
    pub reason: Option<FailReason>,
    pub output: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CategoryTotal {
    pub category: CategoryId,
    pub title: String,
    pub freed: u64,
    pub recycled: u64,
    pub items: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CleanReport {
    pub op_id: String,
    pub started: Timestamp,
    pub finished: Timestamp,
    /// Space freed right away (deletes, commands, emptied bins).
    pub freed_now: u64,
    /// Bytes moved to the Recycle Bin (freed only when the bin is emptied).
    pub recycled: u64,
    pub ok: u64,
    pub failed: u64,
    pub skipped: u64,
    pub results: Vec<CleanItemResult>,
    pub by_category: Vec<CategoryTotal>,
    /// "12 items couldn't be removed because they're in use"
    pub summary: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CleanProgress {
    pub done: u64,
    pub total: u64,
    pub current: String,
    pub freed: u64,
}

pub(crate) fn new_op_id(now: Timestamp) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    format!(
        "op-{now:x}-{:x}-{:x}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

fn result_for(f: &Finding, status: ItemStatus) -> CleanItemResult {
    CleanItemResult {
        finding_id: f.id.clone(),
        title: f.title.clone(),
        category: f.category,
        path: f.paths.first().cloned().unwrap_or_default(),
        disposal: f.disposal.kind().to_string(),
        status,
        freed: 0,
        recycled: 0,
        error: None,
        reason: None,
        output: None,
    }
}

fn classify_io(e: &io::Error) -> FailReason {
    match e.raw_os_error() {
        Some(32) | Some(33) => FailReason::InUse,
        Some(5) => FailReason::AccessDenied,
        Some(2) | Some(3) => FailReason::NotFound,
        _ => match e.kind() {
            io::ErrorKind::NotFound => FailReason::NotFound,
            io::ErrorKind::PermissionDenied => FailReason::AccessDenied,
            _ => FailReason::Other,
        },
    }
}

struct Subtree {
    newest: Timestamp,
    any_cloud: bool,
    never_touch: bool,
    recent_files: Vec<(Timestamp, PathBuf)>,
}

/// Walk a selected path (without following links) to re-check it before disposal.
fn probe_subtree(path: &Path, env: &Env) -> io::Result<Subtree> {
    let root = platform::stat(path)?;
    let mut s = Subtree {
        newest: 0,
        any_cloud: false,
        never_touch: false,
        recent_files: Vec::new(),
    };
    let mut stack = vec![(path.to_path_buf(), root)];
    while let Some((p, e)) = stack.pop() {
        s.newest = s.newest.max(e.modified);
        if protected_paths::classify(&p, env, true).is_some() {
            s.never_touch = true;
        }
        if e.is_dir {
            if e.is_traversable_dir() {
                if let Ok(children) = platform::list_dir(&p) {
                    for c in children {
                        stack.push((p.join(&c.name), c));
                    }
                }
            }
            continue;
        }
        if e.is_cloud_placeholder() {
            s.any_cloud = true;
        }
        s.recent_files.push((e.modified, p));
    }
    s.recent_files.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    s.recent_files.truncate(1000);
    Ok(s)
}

/// Re-validate a finding right before disposal. `Err` means skip it, with the reason.
fn validate(
    f: &Finding,
    env: &Env,
    opts: &CleanOptions,
    now: Timestamp,
) -> Result<Vec<PathBuf>, (FailReason, String)> {
    if let crate::types::Tier::Protected { reason, .. } = &f.tier {
        return Err((FailReason::Protected, reason.explain().to_string()));
    }
    if !f.disposal.is_actionable() {
        return Err((
            FailReason::Protected,
            crate::i18n::tr("Pare only shows steps for this item"),
        ));
    }
    if matches!(f.disposal, Disposal::EmptyRecycleBin { .. }) {
        return Ok(Vec::new());
    }
    if f.paths.is_empty() {
        return Err((FailReason::NotFound, crate::i18n::tr("Nothing to remove")));
    }
    let mut recent: Vec<PathBuf> = Vec::new();
    for p in &f.paths {
        if let Some(r) = protected_paths::classify(p, env, true) {
            return Err((FailReason::Protected, r.explain().into()));
        }
        if protected_paths::is_essential_folder(p, env) {
            return Err((
                FailReason::Protected,
                ProtectReason::SystemPath.explain().into(),
            ));
        }
        if protected_paths::contains_never_touch(p, env) {
            return Err((
                FailReason::Protected,
                ProtectReason::NeverTouch.explain().into(),
            ));
        }
        if opts
            .exclusions
            .iter()
            .any(|ex| path_starts_with(p, ex) || path_starts_with(ex, p))
        {
            return Err((
                FailReason::Protected,
                ProtectReason::Excluded.explain().into(),
            ));
        }
        if matches!(f.disposal, Disposal::Command { .. }) {
            continue; // the tool decides what to remove; it may not exist as a single path
        }
        let s = match probe_subtree(p, env) {
            Ok(s) => s,
            Err(e) if classify_io(&e) == FailReason::NotFound => {
                return Err((FailReason::NotFound, crate::i18n::tr("Already gone")));
            }
            Err(e) => return Err((classify_io(&e), e.to_string())),
        };
        if s.never_touch {
            return Err((
                FailReason::Protected,
                ProtectReason::NeverTouch.explain().into(),
            ));
        }
        if s.newest > opts.scan_started {
            return Err((
                FailReason::ChangedSinceScan,
                crate::i18n::tr("Changed since scan"),
            ));
        }
        if s.newest > now - opts.recent_ms {
            return Err((
                FailReason::Protected,
                ProtectReason::RecentlyModified.explain().into(),
            ));
        }
        if s.any_cloud {
            return Err((
                FailReason::Protected,
                ProtectReason::CloudOnly.explain().into(),
            ));
        }
        recent.extend(s.recent_files.into_iter().map(|(_, p)| p));
    }
    recent.truncate(1000);
    Ok(recent)
}

/// Bottom-up delete that never traverses reparse points: a junction or symlink is removed as
/// a link and its target is left alone.
#[derive(Default)]
struct DeleteStats {
    freed: u64,
    removed: u64,
    failures: Vec<(PathBuf, io::Error)>,
}

fn delete_tree(path: &Path, stats: &mut DeleteStats) {
    let meta = match platform::stat(path) {
        Ok(m) => m,
        Err(e) => {
            if classify_io(&e) != FailReason::NotFound {
                stats.failures.push((path.to_path_buf(), e));
            }
            return;
        }
    };
    if !meta.is_dir {
        match platform::remove_file(path) {
            Ok(()) => {
                stats.removed += 1;
                if meta.links <= 1 && !meta.is_reparse() {
                    stats.freed += meta.allocated;
                }
            }
            Err(e) => stats.failures.push((path.to_path_buf(), e)),
        }
        return;
    }
    if meta.is_reparse() {
        // Junction / directory symlink: remove the link itself, never what it points to.
        if let Err(e) = platform::remove_dir(path) {
            stats.failures.push((path.to_path_buf(), e));
        }
        return;
    }
    match platform::list_dir(path) {
        Ok(children) => {
            for c in children {
                delete_tree(&path.join(&c.name), stats);
            }
        }
        Err(e) => {
            stats.failures.push((path.to_path_buf(), e));
            return;
        }
    }
    if let Err(e) = platform::remove_dir(path) {
        // A child already failed; don't report the same problem twice.
        if stats.failures.is_empty() {
            stats.failures.push((path.to_path_buf(), e));
        }
    }
}

fn describe_failure(path: &Path, e: &io::Error) -> (FailReason, String) {
    let reason = classify_io(e);
    let msg = match reason {
        FailReason::InUse | FailReason::AccessDenied => {
            match platform::processes_holding(&[path.to_path_buf()]) {
                Ok(h) if !h.is_empty() => {
                    let names: Vec<String> = h.into_iter().map(|h| h.name).collect();
                    return (
                        FailReason::InUse,
                        crate::i18n::trf("Open in {apps}", &[("apps", &names.join(", "))]),
                    );
                }
                _ if reason == FailReason::InUse => crate::i18n::tr("In use by another app"),
                _ => crate::i18n::tr("Access denied"),
            }
        }
        FailReason::NotFound => crate::i18n::tr("Already gone"),
        _ => e.to_string(),
    };
    (reason, msg)
}

/// Run a tool's own cleanup command with a timeout, capturing its output.
pub fn run_command(
    program: &str,
    args: &[String],
    timeout: Duration,
) -> (bool, Option<i32>, String) {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return (false, None, format!("Couldn't start {program}: {e}")),
    };
    let read = |r: Option<Box<dyn io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut s = String::new();
            if let Some(mut r) = r {
                let mut buf = Vec::new();
                let _ = r.read_to_end(&mut buf);
                s = String::from_utf8_lossy(&buf).into_owned();
            }
            s
        })
    };
    let out_t = read(
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn io::Read + Send>),
    );
    let err_t = read(
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn io::Read + Send>),
    );
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => break None,
        }
    };
    let mut output = out_t.join().unwrap_or_default();
    let err = err_t.join().unwrap_or_default();
    if !err.trim().is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&err);
    }
    if output.len() > 8000 {
        let mut cut = 8000;
        while !output.is_char_boundary(cut) {
            cut -= 1;
        }
        output.truncate(cut);
        output.push_str("\n…");
    }
    match status {
        Some(s) => (s.success(), s.code(), output),
        None => (
            false,
            None,
            format!("Timed out after {} s\n{output}", timeout.as_secs()),
        ),
    }
}

/// Dispose of the selected findings. Every finding is re-validated; the plan is written to the
/// op log (and synced) before anything runs; per-item results are logged after.
pub fn clean(
    findings: &[Finding],
    env: &Env,
    log: &OpLog,
    opts: &CleanOptions,
    on_progress: &dyn Fn(CleanProgress),
) -> CleanReport {
    let started = now_ms();
    let op = new_op_id(started);
    let mut results: Vec<CleanItemResult> = Vec::new();
    let mut plan: Vec<&Finding> = Vec::new();

    let mut probes: Vec<Vec<PathBuf>> = Vec::new();
    for f in findings {
        match validate(f, env, opts, started) {
            Ok(files) => {
                plan.push(f);
                probes.push(files);
            }
            Err((reason, msg)) => {
                let mut r = result_for(f, ItemStatus::Skipped);
                r.reason = Some(reason);
                r.error = Some(msg);
                results.push(r);
            }
        }
    }
    if opts.check_in_use {
        let held = crate::safety::holders_batch(&probes);
        let mut kept = Vec::new();
        for (f, holders) in plan.into_iter().zip(held) {
            if holders.is_empty() {
                kept.push(f);
            } else {
                let mut names: Vec<String> = holders.into_iter().map(|h| h.name).collect();
                names.sort();
                names.dedup();
                let mut r = result_for(f, ItemStatus::Skipped);
                r.reason = Some(FailReason::InUse);
                r.error = Some(crate::i18n::trf(
                    "Open in {apps}",
                    &[("apps", &names.join(", "))],
                ));
                results.push(r);
            }
        }
        plan = kept;
    }

    // Log the plan before touching anything.
    let mut begin = vec![LogRecord::OpBegin {
        op: op.clone(),
        ts: started,
        title: {
            let size = format_bytes(plan.iter().map(|f| f.bytes).sum());
            match &opts.label {
                Some(l) => {
                    crate::i18n::trf("{label}: free up {size}", &[("label", l), ("size", &size)])
                }
                None => crate::i18n::trf("Free up {size}", &[("size", &size)]),
            }
        },
        items: plan.len() as u64,
        bytes: plan.iter().map(|f| f.bytes).sum(),
    }];
    for f in &plan {
        begin.push(item_record(&op, f, ItemStatus::Pending, None, 0, 0, None));
    }
    for r in &results {
        if let Some(f) = findings.iter().find(|f| f.id == r.finding_id) {
            begin.push(item_record(
                &op,
                f,
                ItemStatus::Skipped,
                r.error.clone(),
                0,
                0,
                None,
            ));
        }
    }
    if let Err(e) = log.write_all(&begin) {
        for f in plan {
            let mut r = result_for(f, ItemStatus::Failed);
            r.reason = Some(FailReason::LogUnavailable);
            r.error = Some(format!("Couldn't write the operation log: {e}"));
            results.push(r);
        }
        return finish(op, started, results, None);
    }

    let total = plan.len() as u64;
    let mut done = 0u64;
    let mut freed_so_far = 0u64;
    let mut progress = |current: &str, freed: u64| {
        done += 1;
        freed_so_far += freed;
        on_progress(CleanProgress {
            done,
            total,
            current: current.to_string(),
            freed: freed_so_far,
        });
    };

    // Recycle in batches (one IFileOperation per batch), then check each item.
    let recycle: Vec<&Finding> = plan
        .iter()
        .copied()
        .filter(|f| matches!(f.disposal, Disposal::Recycle))
        .collect();
    for batch in recycle.chunks(200) {
        let paths: Vec<&PathBuf> = batch.iter().flat_map(|f| f.paths.iter()).collect();
        let batch_err = trash::delete_all(paths.iter().map(|p| p.as_path())).err();
        for f in batch {
            let still: Vec<&PathBuf> = f
                .paths
                .iter()
                .filter(|p| platform::stat(p).is_ok())
                .collect();
            let mut r = result_for(f, ItemStatus::Ok);
            if still.is_empty() {
                r.recycled = f.bytes;
            } else {
                // Retry individually to learn why.
                let mut last_err: Option<(FailReason, String)> = None;
                for p in &still {
                    if let Err(e) = trash::delete(p) {
                        let io_e = io::Error::other(e.to_string());
                        last_err = Some(match platform::processes_holding(&[(*p).clone()]) {
                            Ok(h) if !h.is_empty() => (
                                FailReason::InUse,
                                format!(
                                    "Open in {}",
                                    h.into_iter().map(|h| h.name).collect::<Vec<_>>().join(", ")
                                ),
                            ),
                            _ => (classify_io(&io_e), io_e.to_string()),
                        });
                    }
                }
                let remaining = f.paths.iter().filter(|p| platform::stat(p).is_ok()).count();
                if remaining == 0 {
                    r.recycled = f.bytes;
                } else {
                    r.status = if remaining < f.paths.len() {
                        ItemStatus::Partial
                    } else {
                        ItemStatus::Failed
                    };
                    let (reason, msg) = last_err.unwrap_or((
                        FailReason::Other,
                        batch_err
                            .as_ref()
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| crate::i18n::tr("Couldn't move to the Recycle Bin")),
                    ));
                    r.reason = Some(reason);
                    r.error = Some(msg);
                }
            }
            progress(&f.title, 0);
            results.push(r);
        }
    }

    for f in plan
        .iter()
        .copied()
        .filter(|f| !matches!(f.disposal, Disposal::Recycle))
    {
        let mut r = result_for(f, ItemStatus::Ok);
        match &f.disposal {
            Disposal::Delete => {
                let mut stats = DeleteStats::default();
                for p in &f.paths {
                    delete_tree(p, &mut stats);
                }
                if stats.failures.is_empty() {
                    // The scan's measurement already accounts for hard links inside the finding.
                    r.freed = if f.bytes > 0 { f.bytes } else { stats.freed };
                } else {
                    r.freed = stats.freed.min(f.bytes);
                    r.status = if stats.removed > 0 {
                        ItemStatus::Partial
                    } else {
                        ItemStatus::Failed
                    };
                    let (p, e) = &stats.failures[0];
                    let (reason, msg) = describe_failure(p, e);
                    r.reason = Some(reason);
                    r.error = Some(if stats.failures.len() > 1 {
                        format!(
                            "{msg} ({} files couldn't be removed)",
                            format_count(stats.failures.len() as u64)
                        )
                    } else {
                        msg
                    });
                }
            }
            Disposal::EmptyRecycleBin { drive } => {
                match platform::recycle_bin_empty(Path::new(drive)) {
                    Ok(()) => r.freed = f.bytes,
                    Err(e) => {
                        r.status = ItemStatus::Failed;
                        r.reason = Some(FailReason::Other);
                        r.error = Some(e.to_string());
                    }
                }
            }
            Disposal::Command { program, args } => {
                let before: u64 = f.paths.iter().map(|p| measure_path(p).allocated).sum();
                let (ok, code, output) = run_command(program, args, opts.command_timeout);
                let after: u64 = f.paths.iter().map(|p| measure_path(p).allocated).sum();
                r.freed = before.saturating_sub(after);
                r.output = Some(match code {
                    Some(c) => format!("Exit code {c}\n{output}"),
                    None => output,
                });
                if !ok {
                    r.status = ItemStatus::Failed;
                    r.reason = Some(FailReason::CommandFailed);
                    r.error = Some(match code {
                        Some(c) => format!("{program} exited with code {c}"),
                        None => format!("{program} didn't finish"),
                    });
                }
            }
            Disposal::Recycle | Disposal::Guide { .. } => {}
        }
        progress(&f.title, r.freed);
        results.push(r);
    }

    let end_records: Vec<LogRecord> = results
        .iter()
        .filter(|r| r.status != ItemStatus::Skipped)
        .filter_map(|r| {
            findings.iter().find(|f| f.id == r.finding_id).map(|f| {
                item_record(
                    &op,
                    f,
                    r.status,
                    r.error.clone(),
                    r.freed,
                    r.recycled,
                    r.output.clone(),
                )
            })
        })
        .collect();
    let report = finish(op.clone(), started, results, None);
    let mut recs = end_records;
    recs.push(LogRecord::OpEnd {
        op,
        ts: report.finished,
        freed_now: report.freed_now,
        recycled: report.recycled,
        ok: report.ok,
        failed: report.failed,
        skipped: report.skipped,
    });
    if let Err(e) = log.write_all(&recs) {
        tracing::error!(%e, "couldn't write op-log results");
    }
    report
}

fn item_record(
    op: &str,
    f: &Finding,
    result: ItemStatus,
    error: Option<String>,
    freed: u64,
    recycled: u64,
    output: Option<String>,
) -> LogRecord {
    LogRecord::Item {
        op: op.to_string(),
        ts: now_ms(),
        item: f.id.clone(),
        title: f.title.clone(),
        path: f
            .paths
            .first()
            .cloned()
            .or_else(|| match &f.disposal {
                Disposal::EmptyRecycleBin { drive } => Some(PathBuf::from(drive)),
                _ => None,
            })
            .unwrap_or_default(),
        paths: if f.paths.len() > 1 {
            f.paths.clone()
        } else {
            Vec::new()
        },
        bytes: f.bytes,
        rule: f.rule_id.clone(),
        category: f.category,
        disposal: f.disposal.kind().to_string(),
        result,
        error,
        freed,
        recycled,
        output,
    }
}

fn finish(
    op: String,
    started: Timestamp,
    results: Vec<CleanItemResult>,
    summary: Option<String>,
) -> CleanReport {
    let mut by: BTreeMap<CategoryId, CategoryTotal> = BTreeMap::new();
    let (mut freed, mut recycled, mut ok, mut failed, mut skipped) = (0, 0, 0, 0, 0);
    for r in &results {
        freed += r.freed;
        recycled += r.recycled;
        match r.status {
            ItemStatus::Ok => ok += 1,
            ItemStatus::Partial | ItemStatus::Failed => failed += 1,
            ItemStatus::Skipped => skipped += 1,
            ItemStatus::Pending => {}
        }
        if r.freed > 0 || r.recycled > 0 {
            let t = by.entry(r.category).or_insert_with(|| CategoryTotal {
                category: r.category,
                title: r.category.info().title,
                freed: 0,
                recycled: 0,
                items: 0,
            });
            t.freed += r.freed;
            t.recycled += r.recycled;
            t.items += 1;
        }
    }
    let summary = summary.or_else(|| failure_summary(&results));
    let mut by_category: Vec<CategoryTotal> = by.into_values().collect();
    by_category.sort_by_key(|t| std::cmp::Reverse(t.freed + t.recycled));
    CleanReport {
        op_id: op,
        started,
        finished: now_ms(),
        freed_now: freed,
        recycled,
        ok,
        failed,
        skipped,
        results,
        by_category,
        summary,
    }
}

fn failure_summary(results: &[CleanItemResult]) -> Option<String> {
    let failed: Vec<&CleanItemResult> = results
        .iter()
        .filter(|r| {
            matches!(r.status, ItemStatus::Failed | ItemStatus::Partial)
                || (r.status == ItemStatus::Skipped && r.reason != Some(FailReason::NotFound))
        })
        .collect();
    if failed.is_empty() {
        return None;
    }
    let n = failed.len() as u64;
    let count = format_count(n);
    let args = [("n", count.as_str())];
    use crate::i18n::{tr, trf};
    let in_use = failed
        .iter()
        .filter(|r| r.reason == Some(FailReason::InUse))
        .count();
    let changed = failed
        .iter()
        .filter(|r| {
            matches!(
                r.reason,
                Some(FailReason::ChangedSinceScan) | Some(FailReason::Protected)
            )
        })
        .count();
    Some(match (in_use as u64 == n, changed as u64 == n, n == 1) {
        (true, _, true) => tr("1 item couldn't be removed because it's in use"),
        (true, _, false) => trf(
            "{n} items couldn't be removed because they're in use",
            &args,
        ),
        (_, true, true) => tr("1 item changed since the scan and was kept"),
        (_, true, false) => trf("{n} items changed since the scan and were kept", &args),
        (_, _, true) => tr("1 item couldn't be removed"),
        _ => trf("{n} items couldn't be removed", &args),
    })
}
