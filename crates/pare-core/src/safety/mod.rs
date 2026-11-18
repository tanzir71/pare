//! The safety pass. Rules declare Safe or Review; signals here can only *downgrade* a finding
//! toward Protected (PRD §5.1). Protected findings are never selectable.

pub mod protected_paths;

use std::collections::HashSet;
use std::path::PathBuf;

use crate::env::Env;
use crate::platform::{self, path_starts_with};
use crate::scan::measure::Probe;
use crate::types::{Finding, MS_PER_HOUR, ProcessHolder, ProtectReason, Tier, Timestamp};

#[derive(Clone, Debug)]
pub struct SafetyOptions {
    pub now: Timestamp,
    /// Anything modified more recently than this is Protected (default 24 h).
    pub recent_ms: i64,
    /// "Always keep" paths.
    pub exclusions: Vec<PathBuf>,
    /// Ask the Restart Manager which processes hold files open.
    pub check_in_use: bool,
    /// Lower-case names of running processes (for `requires_closed` rules).
    pub running: HashSet<String>,
}

impl SafetyOptions {
    pub fn new(now: Timestamp) -> SafetyOptions {
        SafetyOptions {
            now,
            recent_ms: 24 * MS_PER_HOUR,
            exclusions: Vec::new(),
            check_in_use: true,
            running: HashSet::new(),
        }
    }
}

/// Lower-case names of all running processes.
pub fn running_processes() -> HashSet<String> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    let Ok(mut sys) = System::new() else {
        return HashSet::new();
    };
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    sys.processes()
        .values()
        .map(|p| p.name().to_string_lossy().to_lowercase())
        .collect()
}

/// Apply every downgrade signal to `f` except "open in a process" (see [`holders_batch`]).
pub fn assess(f: &mut Finding, probe: &Probe, env: &Env, opts: &SafetyOptions) {
    let mut tier = std::mem::replace(&mut f.tier, Tier::Safe);

    for p in &f.paths {
        if let Some(reason) = protected_paths::classify(p, env, probe.admin) {
            tier = tier.downgrade(Tier::protected(reason));
        }
        if protected_paths::is_essential_folder(p, env) {
            tier = tier.downgrade(Tier::protected(ProtectReason::SystemPath));
        }
        if protected_paths::contains_never_touch(p, env) {
            tier = tier.downgrade(Tier::protected(ProtectReason::NeverTouch));
        }
        for ex in &opts.exclusions {
            if path_starts_with(p, ex) || path_starts_with(ex, p) {
                tier = tier.downgrade(Tier::protected(ProtectReason::Excluded));
            }
        }
    }

    if probe.any_cloud {
        tier = tier.downgrade(Tier::protected(ProtectReason::CloudOnly));
    }

    if !f.paths.is_empty() && probe.newest > opts.now - opts.recent_ms {
        tier = tier.downgrade(Tier::protected(ProtectReason::RecentlyModified));
    }

    if !tier.is_protected() {
        if let Some(proc_name) = probe
            .requires_closed
            .iter()
            .find(|p| opts.running.contains(&p.to_lowercase()))
        {
            tier = tier.downgrade(Tier::Protected {
                reason: ProtectReason::ToolRunning,
                detail: Some(format!("{proc_name} is running")),
            });
        }
    }

    f.tier = tier;
}

/// Mark a finding Protected because processes hold its files open.
pub fn apply_in_use(f: &mut Finding, holders: Vec<ProcessHolder>) {
    if holders.is_empty() {
        return;
    }
    let mut names: Vec<String> = holders.into_iter().map(|h| h.name).collect();
    names.sort();
    names.dedup();
    let tier = std::mem::replace(&mut f.tier, Tier::Safe);
    f.tier = tier.downgrade(Tier::Protected {
        reason: ProtectReason::InUse,
        detail: Some(format!("Open in {}", names.join(", "))),
    });
}

/// For each group of files, the processes holding any of them open (empty when free).
/// One Restart Manager session checks a whole batch; only batches with a holder are bisected,
/// so the common case (nothing open) costs a handful of sessions, not one per item.
pub fn holders_batch(groups: &[Vec<PathBuf>]) -> Vec<Vec<ProcessHolder>> {
    const MAX_FILES_PER_SESSION: usize = 20_000;
    let mut out: Vec<Vec<ProcessHolder>> = vec![Vec::new(); groups.len()];
    fn rec(groups: &[Vec<PathBuf>], idx: &[usize], out: &mut [Vec<ProcessHolder>]) {
        let union: Vec<PathBuf> = idx
            .iter()
            .flat_map(|&i| groups[i].iter().cloned())
            .collect();
        if union.is_empty() {
            return;
        }
        let held = platform::processes_holding(&union).unwrap_or_default();
        if held.is_empty() {
            return;
        }
        if idx.len() == 1 {
            out[idx[0]] = held;
            return;
        }
        let (a, b) = idx.split_at(idx.len() / 2);
        rec(groups, a, out);
        rec(groups, b, out);
    }
    let mut chunk: Vec<usize> = Vec::new();
    let mut files = 0usize;
    for (i, g) in groups.iter().enumerate() {
        if g.is_empty() {
            continue;
        }
        if files + g.len() > MAX_FILES_PER_SESSION && !chunk.is_empty() {
            rec(groups, &chunk, &mut out);
            chunk.clear();
            files = 0;
        }
        chunk.push(i);
        files += g.len();
    }
    if !chunk.is_empty() {
        rec(groups, &chunk, &mut out);
    }
    out
}
