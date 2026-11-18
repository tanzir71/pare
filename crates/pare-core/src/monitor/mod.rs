//! M8 monitor: growth alerts and opt-in scheduled cleanups.
//!
//! Both run only while Pare is open, in its window or its notification area icon. Pare
//! installs no Windows service and no scheduled task; the app calls [`tick`] on a timer.

pub mod growth;
pub mod schedule;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::dispose::executor::CleanReport;
use crate::dispose::oplog::OpLog;
use crate::env::Env;
use crate::scanner::{ScanConfig, scan_rules};
use crate::settings::Settings;
use crate::types::{MS_PER_DAY, MS_PER_HOUR, Timestamp};

pub use growth::Growth;

/// `%LOCALAPPDATA%\Pare\monitor.json`.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct MonitorState {
    /// Last growth check.
    pub last_check: Timestamp,
    /// When each rule's growth alert was last raised (at most one per rule per week).
    pub alerted: BTreeMap<String, Timestamp>,
    /// When each scheduled rule last ran.
    pub last_runs: BTreeMap<String, Timestamp>,
}

impl MonitorState {
    pub fn load(env: &Env) -> MonitorState {
        std::fs::read(env.monitor_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, env: &Env) -> std::io::Result<()> {
        let path = env.monitor_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?,
        )?;
        std::fs::rename(tmp, path)
    }
}

#[derive(Default, Debug)]
pub struct TickReport {
    /// Alerts not raised in the last week (show a notification for these).
    pub new_alerts: Vec<Growth>,
    /// A scheduled cleanup ran.
    pub scheduled: Option<CleanReport>,
    /// Rules whose schedule came due (even if nothing needed cleaning).
    pub ran: Vec<String>,
}

/// Measure every rule and record a growth snapshot (sizes only: no in-use checks).
pub fn measure_growth(cfg: &ScanConfig, env: &Env, now: Timestamp) -> Option<growth::Snapshot> {
    let set = cfg.rules.clone()?;
    let mut cfg = cfg.clone();
    cfg.check_in_use = false;
    cfg.now = Some(now);
    let findings = scan_rules(&cfg, env, |_, r| !r.admin);
    let snap = growth::snapshot(&findings, &set, now);
    let _ = growth::record(env, snap.clone());
    Some(snap)
}

/// Current alerts for Home (doesn't change what's been notified).
pub fn current_alerts(env: &Env, settings: &Settings) -> Vec<Growth> {
    growth::alerts(
        &growth::load(env),
        settings.monitor.alert_gb as u64 * 1_000_000_000,
    )
}

/// Held while a pass runs, so two Pare windows never run the same schedule twice.
struct PassLock(std::path::PathBuf);

impl PassLock {
    fn take(env: &Env) -> Option<PassLock> {
        let path = env.pare_data_dir().join("monitor.lock");
        let _ = std::fs::create_dir_all(env.pare_data_dir());
        // A lock left by a crash expires after an hour.
        let stale = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .is_ok_and(|t| t.elapsed().is_ok_and(|e| e.as_secs() > 3600));
        if stale {
            let _ = std::fs::remove_file(&path);
        }
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .ok()
            .map(|_| PassLock(path))
    }
}

impl Drop for PassLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// One monitor pass: a growth check when it's due (and the monitor is on), then any due
/// scheduled cleanups. Returns an empty report if another pass is running.
pub fn tick(env: &Env, cfg: &ScanConfig, settings: &Settings, now: Timestamp) -> TickReport {
    let mut report = TickReport::default();
    let Some(_lock) = PassLock::take(env) else {
        return report;
    };
    let mut state = MonitorState::load(env);
    let every = settings.monitor.check_hours.max(1) as i64 * MS_PER_HOUR;
    if settings.monitor.enabled && now - state.last_check >= every {
        measure_growth(cfg, env, now);
        state.last_check = now;
        for a in current_alerts(env, settings) {
            let fresh = state
                .alerted
                .get(&a.rule_id)
                .is_none_or(|t| now - t >= 7 * MS_PER_DAY);
            if fresh {
                state.alerted.insert(a.rule_id.clone(), now);
                report.new_alerts.push(a);
            }
        }
    }
    if let Some(set) = cfg.rules.clone() {
        let due = schedule::due(&settings.schedules, &state, now);
        if !due.is_empty() {
            if let Ok(log) = OpLog::open(&env.oplog_path()) {
                report.scheduled = schedule::run(&set, &due, cfg, settings, env, &log);
                for id in &due {
                    state.last_runs.insert(id.clone(), now);
                }
                report.ran = due;
            }
        }
    }
    let _ = state.save(env);
    report
}
