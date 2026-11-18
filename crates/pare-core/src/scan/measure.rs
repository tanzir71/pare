//! Turn detector candidates into findings with honest sizes: allocated bytes, each file id
//! counted once, and a hard-linked file counted only when *every* link is inside the finding.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use rayon::prelude::*;
use xxhash_rust::xxh3::xxh3_64;

use super::tree::{NodeId, Tree};
use crate::platform;
use crate::types::{CategoryId, ComesBack, Dates, Disposal, Finding, GitInfo, Tier, Timestamp};

/// What a detector proposes. Sizes and dates are filled in by [`measure`].
#[derive(Clone, Debug)]
pub struct Candidate {
    pub rule_id: String,
    pub category: CategoryId,
    pub source: String,
    pub source_path: Option<PathBuf>,
    pub title: String,
    /// Tree nodes (files or directories) this finding removes.
    pub nodes: Vec<NodeId>,
    /// Paths not in the tree (e.g. found by a rule outside the scan scope). Measured on disk.
    pub extra_paths: Vec<PathBuf>,
    /// Size reported by an API instead of measured (the Recycle Bin): (bytes, items).
    pub fixed_size: Option<(u64, u64)>,
    pub tier: Tier,
    pub disposal: Disposal,
    pub comes_back: ComesBack,
    pub why: String,
    pub if_removed: String,
    pub created_by: Option<String>,
    pub badge: Option<String>,
    pub note: Option<String>,
    pub git: Option<GitInfo>,
    pub facts: Vec<(String, String)>,
    /// Admin rule: may reach `Windows\Temp` / `ProgramData\…\WER`.
    pub admin: bool,
    /// Processes that must not be running (rule `requires_closed`).
    pub requires_closed: Vec<String>,
    /// Sizes the rule engine already computed while walking (skips re-measuring).
    pub measured: Option<Measured>,
}

/// Precomputed measurement for candidates found outside the scan tree.
#[derive(Clone, Debug, Default)]
pub struct Measured {
    pub bytes: u64,
    pub logical: u64,
    pub items: u64,
    pub dates: Dates,
    pub newest: Timestamp,
    pub recent_files: Vec<PathBuf>,
}

/// One file to account for (see [`account`]).
#[derive(Clone, Debug)]
pub struct FileAcc {
    pub path: PathBuf,
    pub allocated: u64,
    pub logical: u64,
    pub file_id: u128,
    pub links: u32,
    pub modified: Timestamp,
}

/// Honest totals for a set of files: link counts fetched where unknown, each file id counted
/// once, and a hard-linked file counted only if all its links are in the set.
/// Returns (allocated, logical, distinct files).
///
/// Link counts aren't in bulk directory listings, so each needs a file open. Large trees are
/// sampled first: if 256 evenly spaced files have no extra links, none of them are assumed to
/// (npm's `node_modules`); if any do (pnpm), every file is checked.
pub fn account(files: &mut [FileAcc]) -> (u64, u64, u64) {
    const SAMPLE_ABOVE: usize = 4096;
    const SAMPLE: usize = 256;
    let need: Vec<usize> = files
        .iter()
        .enumerate()
        .filter(|(_, f)| f.links == 0 && f.allocated > 0)
        .map(|(i, _)| i)
        .collect();
    let query = |idx: &[usize], files: &[FileAcc]| -> Vec<(usize, u32)> {
        idx.par_iter()
            .map(|&i| {
                (
                    i,
                    platform::link_count(&files[i].path)
                        .map(|(l, _)| l)
                        .unwrap_or(1),
                )
            })
            .collect()
    };
    let queried = if need.len() > SAMPLE_ABOVE {
        let step = need.len() / SAMPLE;
        let sample: Vec<usize> = need.iter().step_by(step.max(1)).copied().collect();
        let got = query(&sample, files);
        if got.iter().all(|(_, l)| *l <= 1) {
            need.iter().map(|&i| (i, 1)).collect()
        } else {
            query(&need, files)
        }
    } else {
        query(&need, files)
    };
    for (i, l) in queried {
        files[i].links = l;
    }
    let mut occurrences: HashMap<u128, u32> = HashMap::new();
    for f in files.iter() {
        if f.file_id != 0 {
            *occurrences.entry(f.file_id).or_default() += 1;
        }
    }
    let mut counted: HashSet<u128> = HashSet::new();
    let (mut a, mut l, mut n) = (0u64, 0u64, 0u64);
    for f in files.iter() {
        if f.file_id != 0 && !counted.insert(f.file_id) {
            continue;
        }
        n += 1;
        let occ = if f.file_id != 0 {
            occurrences[&f.file_id]
        } else {
            1
        };
        if f.links <= 1 || occ >= f.links {
            a += f.allocated;
            l += f.logical;
        }
    }
    (a, l, n)
}

impl Candidate {
    pub fn new(rule_id: &str, category: CategoryId, title: impl Into<String>) -> Candidate {
        Candidate {
            rule_id: rule_id.to_string(),
            category,
            source: String::new(),
            source_path: None,
            title: title.into(),
            nodes: Vec::new(),
            extra_paths: Vec::new(),
            fixed_size: None,
            tier: Tier::Review,
            disposal: Disposal::Recycle,
            comes_back: ComesBack::No,
            why: String::new(),
            if_removed: String::new(),
            created_by: None,
            badge: None,
            note: None,
            git: None,
            facts: Vec::new(),
            admin: false,
            requires_closed: Vec::new(),
            measured: None,
        }
    }
}

/// Facts the safety pass needs that aren't part of the serialized finding.
#[derive(Clone, Debug, Default)]
pub struct Probe {
    /// Newest modification time of anything inside (files and folders).
    pub newest: Timestamp,
    /// Some file is a cloud-only placeholder.
    pub any_cloud: bool,
    /// Some part of it could not be listed, or is a link we didn't follow.
    pub opaque: bool,
    /// Most recently modified files (for in-use checks), newest first, capped.
    pub recent_files: Vec<PathBuf>,
    pub admin: bool,
    pub requires_closed: Vec<String>,
}

const IN_USE_PROBE_CAP: usize = 1000;

pub fn finding_id(rule_id: &str, paths: &[PathBuf]) -> String {
    let mut s = rule_id.to_string();
    for p in paths {
        s.push('|');
        s.push_str(&p.to_string_lossy().to_lowercase());
    }
    format!("{:016x}", xxh3_64(s.as_bytes()))
}

struct FileRef {
    node: NodeId,
    allocated: u64,
    logical: u64,
    file_id: u128,
    links: u32,
    modified: Timestamp,
}

/// Measure a candidate against the scan tree.
pub fn measure(c: Candidate, tree: &Tree) -> (Finding, Probe) {
    let mut paths: Vec<PathBuf> = c.nodes.iter().map(|&n| tree.path(n)).collect();
    paths.extend(c.extra_paths.iter().cloned());

    let mut dates = Dates::default();
    let mut probe = Probe {
        admin: c.admin,
        requires_closed: c.requires_closed.clone(),
        ..Probe::default()
    };
    let (bytes, logical_bytes, items) = if let Some(m) = c.measured.clone() {
        dates = m.dates;
        probe.newest = m.newest;
        probe.recent_files = m.recent_files;
        (m.bytes, m.logical, m.items)
    } else if let Some((b, i)) = c.fixed_size {
        (b, b, i)
    } else {
        // Collect files (dedupe overlapping targets by node).
        let mut seen_nodes: HashSet<NodeId> = HashSet::new();
        let mut files: Vec<FileRef> = Vec::new();
        for &target in &c.nodes {
            for id in tree.descendants(target) {
                if !seen_nodes.insert(id) {
                    continue;
                }
                let n = tree.node(id);
                probe.newest = probe.newest.max(n.modified);
                if n.leaf != super::tree::Leaf::No {
                    probe.opaque = true;
                }
                if n.is_dir {
                    if n.child_count == 0 {
                        dates.include(n.created, n.modified, n.accessed);
                    }
                    continue;
                }
                if n.is_cloud_placeholder() {
                    probe.any_cloud = true;
                }
                dates.include(n.created, n.modified, n.accessed);
                files.push(FileRef {
                    node: id,
                    allocated: n.allocated,
                    logical: n.logical,
                    file_id: n.file_id,
                    links: n.links,
                    modified: n.modified,
                });
            }
        }
        // Files measured from disk (outside the tree).
        let mut extra = (0u64, 0u64, 0u64);
        for p in &c.extra_paths {
            let m = measure_path(p);
            extra.0 += m.allocated;
            extra.1 += m.logical;
            extra.2 += m.files;
            probe.newest = probe.newest.max(m.newest);
            probe.any_cloud |= m.any_cloud;
            probe.opaque |= m.opaque;
            dates.include(m.created_min, m.newest, 0);
            probe.recent_files.extend(m.recent_files);
        }

        // Link counts: unknown on Windows bulk listings; ask only for files that hold space.
        let need: Vec<usize> = files
            .iter()
            .enumerate()
            .filter(|(_, f)| f.links == 0 && f.allocated > 0)
            .map(|(i, _)| i)
            .collect();
        let queried: Vec<(usize, u32)> = need
            .par_iter()
            .map(|&i| {
                let links = platform::link_count(&tree.path(files[i].node))
                    .map(|(l, _)| l)
                    .unwrap_or(1);
                (i, links)
            })
            .collect();
        for (i, l) in queried {
            files[i].links = l;
        }

        let mut occurrences: HashMap<u128, u32> = HashMap::new();
        for f in &files {
            if f.file_id != 0 {
                *occurrences.entry(f.file_id).or_default() += 1;
            }
        }
        let mut counted: HashSet<u128> = HashSet::new();
        let (mut a, mut l, mut items) = (0u64, 0u64, 0u64);
        for f in &files {
            if f.file_id != 0 && !counted.insert(f.file_id) {
                continue; // another path to the same file: count once
            }
            items += 1;
            let occ = if f.file_id != 0 {
                occurrences[&f.file_id]
            } else {
                1
            };
            let all_links_inside = f.links <= 1 || occ >= f.links;
            if all_links_inside {
                a += f.allocated;
                l += f.logical;
            }
        }

        // In-use probe: the most recently modified files.
        files.sort_by_key(|f| std::cmp::Reverse(f.modified));
        probe.recent_files.extend(
            files
                .iter()
                .take(IN_USE_PROBE_CAP)
                .map(|f| tree.path(f.node)),
        );
        (a + extra.0, l + extra.1, items + extra.2)
    };

    let finding = Finding {
        id: finding_id(&c.rule_id, &paths),
        rule_id: c.rule_id,
        category: c.category,
        source: c.source,
        source_path: c.source_path,
        title: c.title,
        paths,
        bytes,
        logical_bytes,
        items,
        tier: c.tier,
        disposal: c.disposal,
        comes_back: c.comes_back,
        why: c.why,
        if_removed: c.if_removed,
        dates,
        created_by: c.created_by,
        badge: c.badge,
        note: c.note,
        git: c.git,
        facts: c.facts,
    };
    (finding, probe)
}

/// On-disk measurement of a path that isn't in the scan tree (never follows links).
#[derive(Default, Debug)]
pub struct PathMeasure {
    pub allocated: u64,
    pub logical: u64,
    pub files: u64,
    pub newest: Timestamp,
    pub created_min: Timestamp,
    pub any_cloud: bool,
    pub opaque: bool,
    pub recent_files: Vec<PathBuf>,
}

pub fn measure_path(path: &std::path::Path) -> PathMeasure {
    let mut m = PathMeasure::default();
    let Ok(root) = platform::stat(path) else {
        return m;
    };
    let mut stack = vec![(path.to_path_buf(), root)];
    let mut files: Vec<FileAcc> = Vec::new();
    while let Some((p, e)) = stack.pop() {
        m.newest = m.newest.max(e.modified);
        if e.is_dir {
            if !e.is_traversable_dir() {
                m.opaque = true;
                continue;
            }
            match platform::list_dir(&p) {
                Ok(children) => {
                    for c in children {
                        let cp = p.join(&c.name);
                        stack.push((cp, c));
                    }
                }
                Err(_) => m.opaque = true,
            }
            continue;
        }
        if e.is_cloud_placeholder() {
            m.any_cloud = true;
        }
        if e.created > 0 && (m.created_min == 0 || e.created < m.created_min) {
            m.created_min = e.created;
        }
        files.push(FileAcc {
            path: p,
            allocated: e.allocated,
            logical: e.logical,
            file_id: e.file_id,
            links: e.links,
            modified: e.modified,
        });
    }
    let (a, l, n) = account(&mut files);
    m.allocated = a;
    m.logical = l;
    m.files = n;
    files.sort_by_key(|f| std::cmp::Reverse(f.modified));
    m.recent_files = files
        .into_iter()
        .take(IN_USE_PROBE_CAP)
        .map(|f| f.path)
        .collect();
    m
}
