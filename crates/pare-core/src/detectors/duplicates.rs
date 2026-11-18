//! Duplicates (M5): size buckets → xxh3 of the first and last 64 KB → full BLAKE3.
//! Cloud placeholders are never read; hard links to the same file are one copy, not
//! duplicates; one copy of every group is always kept (never the last remaining copy).

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use xxhash_rust::xxh3::Xxh3;

use super::DetectCtx;
use crate::format::format_bytes;
use crate::platform::{self, eq_component};
use crate::scan::measure::{Candidate, FileAcc, Measured, account};
use crate::scan::tree::{Leaf, NO_NODE, NodeId};
use crate::types::{CategoryId, ComesBack, Dates, Disposal, Tier, Timestamp};

const EDGE: u64 = 64 * 1024;

/// Folders whose copy is preferred as "the one to keep", best first.
fn preference(path: &Path, ctx: &DetectCtx) -> usize {
    let order = [
        &ctx.env.documents,
        &ctx.env.pictures,
        &ctx.env.desktop,
        &ctx.env.downloads,
    ];
    order
        .iter()
        .position(|p| platform::path_starts_with(path, p))
        .unwrap_or(order.len())
}

#[derive(Clone, Debug)]
struct Copy {
    node: NodeId,
    path: PathBuf,
    size: u64,
    allocated: u64,
    file_id: u128,
    created: Timestamp,
    modified: Timestamp,
    accessed: Timestamp,
}

fn quick_hash(path: &Path, size: u64) -> Option<u64> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut h = Xxh3::new();
    let mut buf = vec![0u8; EDGE as usize];
    let n = f.read(&mut buf).ok()?;
    h.update(&buf[..n]);
    if size > EDGE * 2 {
        f.seek(SeekFrom::End(-(EDGE as i64))).ok()?;
        let n = f.read(&mut buf).ok()?;
        h.update(&buf[..n]);
    }
    h.update(&size.to_le_bytes());
    Some(h.digest())
}

fn full_hash(path: &Path) -> Option<[u8; 32]> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Some(*h.finalize().as_bytes())
}

/// Folders whose contents are app data or build output, never "your" duplicates.
const SKIP: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".venv",
    "venv",
    "__pycache__",
    "AppData",
    "$Recycle.Bin",
    ".cache",
];

pub fn duplicates(ctx: &DetectCtx, threads: usize, claimed: &HashSet<NodeId>) -> Vec<Candidate> {
    let tree = ctx.tree;
    let min = ctx
        .thresholds
        .duplicate_min_mb
        .saturating_mul(1_000_000)
        .max(1);
    // Collect candidate files, skipping hidden/system/app folders.
    let mut files: Vec<Copy> = Vec::new();
    let mut stack: Vec<NodeId> = tree.roots.iter().map(|r| r.node).collect();
    while let Some(id) = stack.pop() {
        let n = tree.node(id);
        if n.is_dir {
            if n.parent != NO_NODE
                && (n.is_hidden_or_system()
                    || n.name.to_string_lossy().starts_with('.')
                    || SKIP.iter().any(|s| eq_component(&n.name, OsStr::new(s))))
            {
                continue;
            }
            stack.extend(tree.children(id));
            continue;
        }
        // Claimed by another suggestion: neither suggested nor kept here (so the kept copy can
        // never be one that another card removes).
        if n.leaf != Leaf::No
            || n.logical < min
            || n.is_cloud_placeholder()
            || n.is_hidden_or_system()
            || claimed.contains(&id)
        {
            continue;
        }
        files.push(Copy {
            node: id,
            path: tree.path(id),
            size: n.logical,
            allocated: n.allocated,
            file_id: n.file_id,
            created: n.created,
            modified: n.modified,
            accessed: n.accessed,
        });
    }

    // 1. Same size. Paths that are the same file (hard links) collapse into one copy.
    let mut by_size: HashMap<u64, Vec<Copy>> = HashMap::new();
    for f in files {
        let bucket = by_size.entry(f.size).or_default();
        if f.file_id != 0 && bucket.iter().any(|c| c.file_id == f.file_id) {
            continue;
        }
        bucket.push(f);
    }
    let buckets: Vec<Vec<Copy>> = by_size.into_values().filter(|v| v.len() > 1).collect();

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads.max(1))
        .build();
    let run = |work: &(dyn Fn() -> Vec<Vec<Copy>> + Sync)| match &pool {
        Ok(p) => p.install(work),
        Err(_) => work(),
    };

    // 2. Same first/last 64 KB, then 3. same full BLAKE3.
    let groups: Vec<Vec<Copy>> = run(&|| {
        buckets
            .par_iter()
            .flat_map(|b| {
                let mut q: HashMap<u64, Vec<Copy>> = HashMap::new();
                for c in b {
                    if let Some(h) = quick_hash(&c.path, c.size) {
                        q.entry(h).or_default().push(c.clone());
                    }
                }
                let mut out = Vec::new();
                for v in q.into_values().filter(|v| v.len() > 1) {
                    let mut full: HashMap<[u8; 32], Vec<Copy>> = HashMap::new();
                    for c in v {
                        if let Some(h) = full_hash(&c.path) {
                            full.entry(h).or_default().push(c);
                        }
                    }
                    out.extend(full.into_values().filter(|v| v.len() > 1));
                }
                out
            })
            .collect()
    });

    let mut out = Vec::new();
    for mut g in groups {
        // Keep the copy in the preferred folder; then the oldest.
        g.sort_by_key(|c| (preference(&c.path, ctx), c.created, c.path.clone()));
        let keep = g.remove(0);
        if g.is_empty() {
            continue;
        }
        let name = keep
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut c = Candidate::new(
            "duplicates.exact",
            CategoryId::Duplicates,
            format!("{name} ({} copies)", g.len() + 1),
        );
        c.source = keep
            .path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        c.source_path = keep.path.parent().map(|p| p.to_path_buf());
        c.nodes = g.iter().map(|x| x.node).collect();
        c.tier = Tier::Review;
        c.disposal = Disposal::Recycle; // always: these are your files
        c.comes_back = ComesBack::No;
        c.why = format!(
            "Exact copies of {name} ({} each). Pare compared every byte.",
            format_bytes(keep.size)
        );
        c.if_removed = format!(
            "The copy in {} stays. The others go to the Recycle Bin.",
            keep.path
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        );
        c.note = Some(format!("{} other copies", g.len()));
        c.badge = Some("Duplicate".into());
        c.facts
            .push(("Kept copy".into(), keep.path.display().to_string()));
        let mut accs: Vec<FileAcc> = g
            .iter()
            .map(|x| FileAcc {
                path: x.path.clone(),
                allocated: x.allocated,
                logical: x.size,
                file_id: x.file_id,
                links: tree.node(x.node).links,
                modified: x.modified,
            })
            .collect();
        let (bytes, logical, items) = account(&mut accs);
        let mut dates = Dates::default();
        for x in &g {
            dates.include(x.created, x.modified, x.accessed);
        }
        c.measured = Some(Measured {
            bytes,
            logical,
            items,
            dates,
            newest: g.iter().map(|x| x.modified).max().unwrap_or(0),
            recent_files: g.iter().map(|x| x.path.clone()).collect(),
        });
        out.push(c);
    }
    out
}

/// Old screenshots: `Pictures\Screenshots` older than 90 days (setting).
pub fn screenshots(ctx: &DetectCtx) -> Vec<Candidate> {
    let tree = ctx.tree;
    let dir = ctx.env.pictures.join("Screenshots");
    let Some(id) = tree.find(&dir) else {
        return Vec::new();
    };
    let min_age = ctx.thresholds.screenshot_days as i64 * crate::types::MS_PER_DAY;
    let mut out = Vec::new();
    for c in tree.descendants(id).skip(1) {
        let n = tree.node(c);
        if n.is_dir || n.is_hidden_or_system() || ctx.now - n.modified < min_age {
            continue;
        }
        let ext = n.extension_lower().unwrap_or_default();
        if !["png", "jpg", "jpeg", "gif", "bmp", "webp", "heic"].contains(&ext.as_str()) {
            continue;
        }
        let mut cand = Candidate::new(
            "screenshots.old",
            CategoryId::Screenshots,
            n.name.to_string_lossy().into_owned(),
        );
        cand.nodes = vec![c];
        cand.source = "Screenshots".into();
        cand.source_path = Some(dir.clone());
        cand.tier = Tier::Review;
        cand.disposal = Disposal::Recycle;
        cand.comes_back = ComesBack::No;
        cand.why = format!(
            "A screenshot taken {}.",
            crate::format::age_phrase(ctx.now, n.created.max(n.modified).min(n.modified))
        );
        cand.if_removed =
            "It goes to the Recycle Bin, so you can restore it until the bin is emptied.".into();
        cand.note = Some(crate::format::age_phrase(ctx.now, n.modified));
        out.push(cand);
    }
    out
}

/// Hashing threads for a path: 1 on spinning disks, a few on SSDs.
pub fn hash_threads(root: &Path) -> usize {
    use sysinfo::{DiskKind, Disks};
    let Ok(disks) = Disks::new_with_refreshed_list() else {
        return 2;
    };
    let best = disks
        .iter()
        .filter(|d| platform::path_starts_with(root, d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len());
    match best.map(|d| d.kind()) {
        Some(DiskKind::HDD) => 1,
        _ => 4,
    }
}
