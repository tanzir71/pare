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
    /// Cleanup rules (M3). `None` runs only the quick detectors.
    pub rules: Option<std::sync::Arc<crate::rules::RuleSet>>,
    /// Where to look for projects (M4). Empty = no project scan.
    pub project_roots: Vec<PathBuf>,
    /// Look for duplicate files (M5); reads file contents of same-size candidates.
    pub duplicates: bool,
    /// Whole-drive scans read the NTFS MFT when the process is elevated (M6).
    pub fast_mode: bool,
    /// Indexes kept between scans (by drive root), updated from the USN journal.
    pub index_cache: Option<
        std::sync::Arc<
            std::sync::Mutex<std::collections::HashMap<PathBuf, crate::index::VolumeIndex>>,
        >,
    >,
}

impl Default for ScanConfig {
    fn default() -> Self {
        ScanConfig {
            thresholds: Thresholds::default(),
            exclusions: Vec::new(),
            check_in_use: true,
            include_recycle_bin: true,
            now: None,
            rules: None,
            project_roots: Vec::new(),
            duplicates: true,
            fast_mode: true,
            index_cache: None,
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
    /// Time per phase and detector, in milliseconds.
    #[serde(default)]
    pub phases: Vec<(String, u64)>,
    /// Drives read from the NTFS file table (fast mode) rather than walked.
    #[serde(default)]
    pub fast_drives: Vec<PathBuf>,
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
    /// Every project found (M4), with git state and build artifacts.
    #[serde(default)]
    pub projects: Vec<crate::projects::Project>,
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
    let phase_start = std::time::Instant::now();
    let mut phases: Vec<(String, u64)> = Vec::new();
    // Whole drives: keep every folder but only the files detectors need, so memory stays
    // proportional to folders rather than files.
    let keep = if req.whole_drives {
        let dup = cfg
            .thresholds
            .duplicate_min_mb
            .saturating_mul(1_000_000)
            .max(1);
        crate::scan::walker::Keep {
            all: false,
            under: vec![
                (env.downloads.clone(), 0),
                (env.pictures.join("Screenshots"), 0),
                (env.desktop.clone(), dup),
                (env.documents.clone(), dup),
                (env.pictures.clone(), dup),
                (env.home.join("Videos"), dup),
                (env.home.join("Music"), dup),
            ],
            min_bytes: cfg
                .thresholds
                .large_file_mb
                .saturating_mul(1_000_000)
                .min(10_000_000),
        }
    } else {
        crate::scan::walker::Keep::default()
    };
    let (tree, fast_drives) = enumerate(&roots, req, cfg, env, cancel, progress, keep, &mut phases);
    phases.push(("enumerate".into(), phase_start.elapsed().as_millis() as u64));

    let mut result = ScanResult {
        started,
        finished: 0,
        roots: req.roots.clone(),
        findings: Vec::new(),
        categories: Vec::new(),
        stats: ScanStats::default(),
        cancelled: false,
        projects: Vec::new(),
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
        installed: if env.is_fake {
            Vec::new()
        } else {
            platform::installed_apps()
        },
    };

    let mut safety_opts = SafetyOptions::new(now);
    safety_opts.recent_ms = cfg.thresholds.recent_hours as i64 * MS_PER_HOUR;
    safety_opts.exclusions = cfg.exclusions.clone();
    safety_opts.check_in_use = cfg.check_in_use;
    safety_opts.running = safety::running_processes();

    // Downloads claims nodes that "large files" must not repeat, so it runs first.
    let claimed = std::sync::Mutex::new(std::collections::HashSet::new());
    let projects_found: std::sync::Mutex<Vec<crate::projects::Project>> =
        std::sync::Mutex::new(Vec::new());
    let mut detectors: Vec<Detector> = Vec::new();
    let rule_roots: Vec<PathBuf> = roots.clone();
    let drives_for_rules: Vec<PathBuf> = if env.is_fake {
        Vec::new()
    } else {
        platform::list_drives()
            .into_iter()
            .filter(|d| d.is_fixed)
            .map(|d| d.root)
            .collect()
    };
    let elevated = platform::is_elevated();
    if let Some(set) = cfg.rules.clone() {
        for (id, label, cat) in [
            ("junk", "Junk & temp files", CategoryId::Junk),
            ("ai-tools", "AI tool caches", CategoryId::AiTools),
            ("dev-caches", "Developer caches", CategoryId::DevCaches),
            ("browsers", "Browser caches", CategoryId::Browsers),
        ] {
            let set = set.clone();
            let rule_roots = rule_roots.clone();
            let drives = drives_for_rules.clone();
            let whole = req.whole_drives;
            let check_in_use = cfg.check_in_use;
            let recent_ms = cfg.thresholds.recent_hours as i64 * MS_PER_HOUR;
            detectors.push((
                id,
                label,
                Box::new(move |c: &DetectCtx| {
                    let ctx = crate::rules::engine::RuleCtx {
                        env: c.env,
                        now: c.now,
                        recent_ms,
                        drives: &drives,
                        elevated,
                        within: if whole { None } else { Some(&rule_roots) },
                        check_in_use,
                    };
                    crate::rules::engine::run_category(&set, &ctx, cat)
                }),
            ));
        }
    }
    if !cfg.project_roots.is_empty() {
        let ecos = cfg
            .rules
            .as_ref()
            .map(|r| r.ecosystems.clone())
            .unwrap_or_default();
        let roots = cfg.project_roots.clone();
        let idle = cfg.thresholds.idle_days;
        let pf = &projects_found;
        detectors.push((
            "projects",
            "Git projects",
            Box::new(move |c: &DetectCtx| {
                let found = crate::projects::scan_projects(&roots, &ecos, c.env, c.now, idle);
                let mut cands = Vec::new();
                let mut projects = Vec::new();
                for (p, cand) in found {
                    if let Some(cand) = cand {
                        cands.push(cand);
                    }
                    if p.git.is_some() {
                        cands.extend(crate::projects::stale_worktrees(&p.root, c.now));
                    }
                    projects.push(p);
                }
                *pf.lock().unwrap() = projects;
                cands
            }),
        ));
    }
    detectors.extend::<Vec<Detector>>(vec![
        (
            "downloads",
            "Downloads",
            Box::new(|c: &DetectCtx| {
                let (v, cl) = quick::downloads(c);
                claimed.lock().unwrap().extend(cl);
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
        (
            "screenshots",
            "Old screenshots",
            Box::new(crate::detectors::duplicates::screenshots),
        ),
    ]);
    let claimed_ref = &claimed;
    if cfg.duplicates {
        let threads = roots
            .first()
            .map(|r| crate::detectors::duplicates::hash_threads(r))
            .unwrap_or(2);
        detectors.push((
            "duplicates",
            "Duplicates",
            Box::new(move |c: &DetectCtx| {
                crate::detectors::duplicates::duplicates(c, threads, &claimed_ref.lock().unwrap())
            }),
        ));
    }
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
        let t0 = std::time::Instant::now();
        let candidates = run(&ctx);
        // An item belongs to one finding: later detectors skip nodes earlier ones claimed.
        if let Ok(mut cl) = claimed.lock() {
            for c in &candidates {
                cl.extend(c.nodes.iter().copied());
            }
        }
        let t_detect = t0.elapsed().as_millis() as u64;
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
        phases.push((format!("{id}: detect"), t_detect));
        phases.push((
            format!("{id}: measure+safety"),
            t0.elapsed().as_millis() as u64 - t_detect,
        ));
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
    // Link each project to its finding (the safety pass may have changed its tier).
    let mut projects = std::mem::take(&mut *projects_found.lock().unwrap());
    for p in &mut projects {
        if let Some(f) = result.findings.iter().find(|f| {
            f.category == CategoryId::ProjectBuild
                && f.paths
                    .iter()
                    .any(|x| crate::platform::path_starts_with(x, &p.root))
                && f.title == p.name
        }) {
            p.finding_id = Some(f.id.clone());
            p.tier = f.tier.clone();
            p.bytes = f.bytes;
        }
    }
    result.projects = projects;
    let snap = progress.snapshot();
    result.stats = ScanStats {
        files: snap.files,
        dirs: snap.dirs,
        bytes_seen: snap.bytes_seen,
        elapsed_ms: snap.elapsed_ms,
        phases,
        fast_drives,
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

/// Projects only (the Projects tab): discover, analyze, measure and run the safety pass.
pub fn scan_projects_only(
    roots: &[PathBuf],
    cfg: &ScanConfig,
    env: &Env,
) -> (Vec<crate::projects::Project>, Vec<Finding>) {
    let now = cfg.now.unwrap_or_else(now_ms);
    let ecos = cfg
        .rules
        .as_ref()
        .map(|r| r.ecosystems.clone())
        .unwrap_or_default();
    let found = crate::projects::scan_projects(roots, &ecos, env, now, cfg.thresholds.idle_days);
    let mut safety_opts = SafetyOptions::new(now);
    safety_opts.recent_ms = cfg.thresholds.recent_hours as i64 * MS_PER_HOUR;
    safety_opts.exclusions = cfg.exclusions.clone();
    safety_opts.check_in_use = cfg.check_in_use;
    let tree = Tree::default();
    let mut projects = Vec::new();
    let mut assessed: Vec<(Finding, Vec<PathBuf>)> = Vec::new();
    for (p, cand) in found {
        let mut cands: Vec<Candidate> = cand.into_iter().collect();
        if p.git.is_some() {
            cands.extend(crate::projects::stale_worktrees(&p.root, now));
        }
        for c in cands {
            let (mut f, probe) = measure(c, &tree);
            safety::assess(&mut f, &probe, env, &safety_opts);
            let files = if f.tier.is_protected() {
                Vec::new()
            } else {
                probe.recent_files
            };
            assessed.push((f, files));
        }
        projects.push(p);
    }
    if cfg.check_in_use {
        let groups: Vec<Vec<PathBuf>> = assessed
            .iter_mut()
            .map(|(_, p)| std::mem::take(p))
            .collect();
        for ((f, _), holders) in assessed.iter_mut().zip(safety::holders_batch(&groups)) {
            safety::apply_in_use(f, holders);
        }
    }
    let findings: Vec<Finding> = assessed.into_iter().map(|(f, _)| f).collect();
    for p in &mut projects {
        if let Some(f) = findings.iter().find(|f| {
            f.title == p.name
                && f.paths
                    .iter()
                    .any(|x| crate::platform::path_starts_with(x, &p.root))
        }) {
            p.finding_id = Some(f.id.clone());
            p.tier = f.tier.clone();
            p.bytes = f.bytes;
        }
    }
    (projects, findings)
}

/// Enumerate roots: drive roots use the MFT index when fast mode is possible, everything else
/// (and every failure) falls back to the walker.
#[allow(clippy::too_many_arguments)]
fn enumerate(
    roots: &[PathBuf],
    req: &ScanRequest,
    cfg: &ScanConfig,
    env: &Env,
    cancel: &AtomicBool,
    progress: &Progress,
    keep: crate::scan::walker::Keep,
    phases: &mut Vec<(String, u64)>,
) -> (Tree, Vec<PathBuf>) {
    let mut fast_trees = Vec::new();
    let mut fast_drives = Vec::new();
    let mut walk_roots = Vec::new();
    for r in roots {
        let is_drive_root = r.parent().is_none() || r.components().count() <= 2;
        if req.whole_drives
            && cfg.fast_mode
            && is_drive_root
            && crate::index::fast_mode_available(r)
        {
            let t = std::time::Instant::now();
            match fast_index(r, cfg, env) {
                Ok((idx, how)) => {
                    let mut files = 0u64;
                    let tree = crate::index::to_tree(&idx, &keep, env, &mut files);
                    progress.files.fetch_add(files, Ordering::Relaxed);
                    progress.bytes_seen.fetch_add(
                        tree.roots
                            .first()
                            .map(|x| tree.node(x.node).sub_allocated)
                            .unwrap_or(0),
                        Ordering::Relaxed,
                    );
                    phases.push((
                        format!("{} {how}", r.display()),
                        t.elapsed().as_millis() as u64,
                    ));
                    fast_trees.push(tree);
                    fast_drives.push(r.clone());
                    if let Some(cache) = &cfg.index_cache {
                        if let Ok(mut c) = cache.lock() {
                            c.insert(r.clone(), idx);
                        }
                    }
                    continue;
                }
                Err(e) => {
                    tracing::warn!(root = %r.display(), %e, "fast mode failed; walking instead")
                }
            }
        }
        walk_roots.push(r.clone());
    }
    let walked = Walker {
        env,
        cancel,
        progress,
        keep,
    }
    .walk(&walk_roots);
    if fast_trees.is_empty() {
        return (walked, fast_drives);
    }
    fast_trees.push(walked);
    (crate::index::merge_trees(fast_trees), fast_drives)
}

/// Get an up-to-date index for a drive: the cached one caught up from the USN journal, the
/// saved one from disk, or a fresh MFT read.
#[cfg(windows)]
fn fast_index(
    root: &PathBuf,
    cfg: &ScanConfig,
    env: &Env,
) -> Result<(crate::index::VolumeIndex, &'static str), String> {
    use crate::index::{db_path, mft, store};
    let cached = cfg
        .index_cache
        .as_ref()
        .and_then(|c| c.lock().ok().and_then(|mut m| m.remove(root)));
    let serial = platform::volume_serial(root).unwrap_or(0);
    let from_disk = || {
        store::load(&db_path(env, serial))
            .ok()
            .filter(|i| i.root == *root)
    };
    if let Some(mut idx) = cached.or_else(from_disk) {
        if mft::update(&mut idx).is_ok() {
            return Ok((idx, "index + USN journal"));
        }
    }
    let idx = mft::build(root)?;
    let save = idx.clone();
    let path = db_path(env, serial);
    std::thread::spawn(move || {
        if let Err(e) = store::save(&save, &path) {
            tracing::warn!(%e, "couldn't save the index");
        }
    });
    Ok((idx, "MFT"))
}

#[cfg(not(windows))]
fn fast_index(
    _root: &PathBuf,
    _cfg: &ScanConfig,
    _env: &Env,
) -> Result<(crate::index::VolumeIndex, &'static str), String> {
    Err("fast mode is Windows-only".into())
}
