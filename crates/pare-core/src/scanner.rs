//! Scan pipeline: enumerate → detectors → measure → safety pass → categories (ARCHITECTURE §4.1).
//! A scan only reads. Nothing here deletes anything.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::detectors::{DetectCtx, Thresholds, quick};
use crate::env::Env;
use crate::platform::{self, now_ms, path_starts_with};
use crate::safety::{self, SafetyOptions};
use crate::scan::measure::{Candidate, measure};
use crate::scan::progress::{Phase, Progress};
use crate::scan::tree::{NodeId, Tree};
use crate::scan::walker::{Walker, dedupe_roots};
use crate::types::{CategoryId, CategorySummary, Finding, MS_PER_HOUR, Tier, Timestamp};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScanRoot {
    pub path: PathBuf,
    pub label: String,
}

/// What to scan.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanRequest {
    pub roots: Vec<ScanRoot>,
    /// Drives whose Recycle Bin is checked (and whose used space estimates progress).
    pub drives: Vec<PathBuf>,
    /// True when the scope is whole drives ("This PC").
    pub whole_drives: bool,
}

/// How to scan (from settings).
#[derive(Clone, Debug)]
pub struct ScanConfig {
    pub thresholds: Thresholds,
    pub exclusions: Vec<PathBuf>,
    pub check_in_use: bool,
    pub include_recycle_bin: bool,
    /// Override "now" (tests).
    pub now: Option<Timestamp>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        ScanConfig {
            thresholds: Thresholds::default(),
            exclusions: Vec::new(),
            check_in_use: true,
            include_recycle_bin: true,
            now: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DetectorState {
    Waiting,
    Running,
    Done,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DetectorStatus {
    pub id: String,
    pub label: String,
    pub state: DetectorState,
    pub bytes: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ScanEvent {
    Detector(DetectorStatus),
    Category(CategorySummary),
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub files: u64,
    pub dirs: u64,
    pub bytes_seen: u64,
    pub elapsed_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub started: Timestamp,
    pub finished: Timestamp,
    pub roots: Vec<ScanRoot>,
    pub findings: Vec<Finding>,
    pub categories: Vec<CategorySummary>,
    pub stats: ScanStats,
    pub cancelled: bool,
}

#[derive(Clone)]
pub struct ScanOutput {
    pub result: ScanResult,
    /// Everything the walker saw (used by Browse and re-measuring). Shared, cheap to clone.
    pub tree: std::sync::Arc<Tree>,
}

type Detector<'a> = (
    &'static str,
    &'static str,
    Box<dyn Fn(&DetectCtx) -> Vec<Candidate> + Sync + 'a>,
);

/// Run a scan. `on_event` receives detector state changes and category summaries as soon as
/// they're ready; progress counters are sampled from `progress` by the caller.
pub fn scan(
    req: &ScanRequest,
    cfg: &ScanConfig,
    env: &Env,
    progress: &Progress,
    cancel: &AtomicBool,
    on_event: &(dyn Fn(ScanEvent) + Sync),
) -> ScanOutput {
    let started = now_ms();
    let now = cfg.now.unwrap_or(started);
    progress.set_phase(Phase::Enumerating);

    if req.whole_drives && !env.is_fake {
        let used: u64 = platform::list_drives()
            .iter()
            .filter(|d| req.drives.iter().any(|r| path_starts_with(r, &d.root)))
            .map(|d| d.used())
            .sum();
        progress.estimate_bytes.store(used, Ordering::Relaxed);
    }

    let paths: Vec<PathBuf> = req.roots.iter().map(|r| r.path.clone()).collect();
    let roots = dedupe_roots(&paths);
    let tree = Walker {
        env,
        cancel,
        progress,
    }
    .walk(&roots);

    let mut result = ScanResult {
        started,
        finished: 0,
        roots: req.roots.clone(),
        findings: Vec::new(),
        categories: Vec::new(),
        stats: ScanStats::default(),
        cancelled: false,
    };

    if cancel.load(Ordering::Relaxed) {
        progress.set_phase(Phase::Cancelled);
        result.cancelled = true;
        result.finished = now_ms();
        return ScanOutput {
            result,
            tree: std::sync::Arc::new(tree),
        };
    }

    let root_labels: Vec<(NodeId, String)> = tree
        .roots
        .iter()
        .map(|r| {
            let label = req
                .roots
                .iter()
                .find(|x| platform::normalize(&x.path) == r.path)
                .map(|x| x.label.clone())
                .unwrap_or_else(|| r.path.to_string_lossy().into_owned());
            (r.node, label)
        })
        .collect();

    progress.set_phase(Phase::Detecting);
    let ctx = DetectCtx {
        tree: &tree,
        env,
        now,
        thresholds: &cfg.thresholds,
        root_labels: &root_labels,
    };

    let mut safety_opts = SafetyOptions::new(now);
    safety_opts.recent_ms = cfg.thresholds.recent_hours as i64 * MS_PER_HOUR;
    safety_opts.exclusions = cfg.exclusions.clone();
    safety_opts.check_in_use = cfg.check_in_use;
    safety_opts.running = safety::running_processes();

    // Downloads claims nodes that "large files" must not repeat, so it runs first.
    let claimed = std::sync::Mutex::new(std::collections::HashSet::new());
    let mut detectors: Vec<Detector> = vec![
        (
            "downloads",
            "Downloads",
            Box::new(|c: &DetectCtx| {
                let (v, cl) = quick::downloads(c);
                *claimed.lock().unwrap() = cl;
                v
            }),
        ),
        (
            "large-files",
            "Large & old files",
            Box::new(|c: &DetectCtx| quick::large_files(c, &claimed.lock().unwrap())),
        ),
        (
            "empty-folders",
            "Empty folders",
            Box::new(quick::empty_folders),
        ),
    ];
    if cfg.include_recycle_bin && !env.is_fake {
        let drives = req.drives.clone();
        detectors.push((
            "recycle-bin",
            "Recycle Bin",
            Box::new(move |_c: &DetectCtx| quick::recycle_bin(&drives)),
        ));
    }

    progress
        .detectors_total
        .store(detectors.len() as u64, Ordering::Relaxed);
    for (id, label, _) in &detectors {
        on_event(ScanEvent::Detector(DetectorStatus {
            id: id.to_string(),
            label: label.to_string(),
            state: DetectorState::Waiting,
            bytes: 0,
        }));
    }

    for (id, label, run) in &detectors {
        if cancel.load(Ordering::Relaxed) {
            result.cancelled = true;
            break;
        }
        on_event(ScanEvent::Detector(DetectorStatus {
            id: id.to_string(),
            label: label.to_string(),
            state: DetectorState::Running,
            bytes: 0,
        }));
        let candidates = run(&ctx);
        let measured: Vec<_> = candidates
            .into_par_iter()
            .map(|c| measure(c, &tree))
            .collect();
        let mut assessed: Vec<(Finding, Vec<PathBuf>)> = measured
            .into_iter()
            .map(|(mut f, probe)| {
                safety::assess(&mut f, &probe, env, &safety_opts);
                let probe_files = if f.tier.is_protected() {
                    Vec::new()
                } else {
                    probe.recent_files
                };
                (f, probe_files)
            })
            .collect();
        if cfg.check_in_use {
            let groups: Vec<Vec<PathBuf>> = assessed
                .iter_mut()
                .map(|(_, p)| std::mem::take(p))
                .collect();
            for ((f, _), holders) in assessed.iter_mut().zip(safety::holders_batch(&groups)) {
                safety::apply_in_use(f, holders);
            }
        }
        let mut found = 0u64;
        let mut touched: Vec<CategoryId> = Vec::new();
        for (f, _) in assessed {
            if f.bytes == 0 && f.items == 0 && !matches!(f.category, CategoryId::EmptyFolders) {
                continue;
            }
            if f.tier.is_selectable() {
                found += f.bytes;
            }
            if !touched.contains(&f.category) {
                touched.push(f.category);
            }
            result.findings.push(f);
        }
        progress.found_bytes.fetch_add(found, Ordering::Relaxed);
        progress.detectors_done.fetch_add(1, Ordering::Relaxed);
        on_event(ScanEvent::Detector(DetectorStatus {
            id: id.to_string(),
            label: label.to_string(),
            state: DetectorState::Done,
            bytes: found,
        }));
        let summaries = summarize(&result.findings);
        for s in summaries
            .into_iter()
            .filter(|s| touched.contains(&s.info.id))
        {
            on_event(ScanEvent::Category(s));
        }
    }

    result.categories = summarize(&result.findings);
    let snap = progress.snapshot();
    result.stats = ScanStats {
        files: snap.files,
        dirs: snap.dirs,
        bytes_seen: snap.bytes_seen,
        elapsed_ms: snap.elapsed_ms,
    };
    result.finished = now_ms();
    progress.set_phase(if result.cancelled {
        Phase::Cancelled
    } else {
        Phase::Done
    });
    ScanOutput {
        result,
        tree: std::sync::Arc::new(tree),
    }
}

/// Category cards, sorted by reclaimable bytes (largest first).
pub fn summarize(findings: &[Finding]) -> Vec<CategorySummary> {
    let mut map: BTreeMap<CategoryId, CategorySummary> = BTreeMap::new();
    let mut sources: BTreeMap<CategoryId, std::collections::BTreeSet<String>> = BTreeMap::new();
    for f in findings {
        let s = map.entry(f.category).or_insert_with(|| CategorySummary {
            info: f.category.info(),
            safe_bytes: 0,
            review_bytes: 0,
            protected_bytes: 0,
            items: 0,
            findings: 0,
            sources: 0,
            protected_findings: 0,
            frees_now_bytes: 0,
            recycle_bytes: 0,
        });
        match &f.tier {
            Tier::Safe => s.safe_bytes += f.bytes,
            Tier::Review => s.review_bytes += f.bytes,
            Tier::Protected { .. } => {
                s.protected_bytes += f.bytes;
                s.protected_findings += 1;
                continue;
            }
        }
        if !f.disposal.is_actionable() {
            // Guides are shown but never counted as space Pare frees.
            if matches!(f.tier, Tier::Safe) {
                s.safe_bytes -= f.bytes;
            } else {
                s.review_bytes -= f.bytes;
            }
        } else if f.disposal.frees_now() {
            s.frees_now_bytes += f.bytes;
        } else {
            s.recycle_bytes += f.bytes;
        }
        s.items += f.items;
        s.findings += 1;
        sources
            .entry(f.category)
            .or_default()
            .insert(f.source.clone());
    }
    let mut v: Vec<CategorySummary> = map
        .into_iter()
        .map(|(id, mut s)| {
            s.sources = sources.get(&id).map_or(0, |x| x.len() as u64);
            s
        })
        .collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.total_bytes()));
    v
}
