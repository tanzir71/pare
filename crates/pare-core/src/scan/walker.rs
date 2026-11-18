//! Parallel directory walker (rayon). Lists each directory in bulk, never follows reparse points,
//! never descends into hard-excluded roots, and never opens file contents.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use rayon::prelude::*;

use super::progress::Progress;
use super::tree::{Leaf, NO_NODE, Node, RootInfo, Tree};
use crate::env::Env;
use crate::platform::{self, RawEntry};
use crate::safety::protected_paths;

#[derive(Default)]
struct DirResult {
    leaves: Vec<(RawEntry, Leaf)>,
    subdirs: Vec<(RawEntry, DirResult)>,
    error: bool,
    folded: (u32, u64, u64, i64),
}

/// Which files become tree nodes. Directories are always kept.
#[derive(Clone, Debug)]
pub struct Keep {
    /// Keep every file (folder scans).
    pub all: bool,
    /// Keep files at least this large under these folders (0 = every file, e.g. Downloads).
    pub under: Vec<(PathBuf, u64)>,
    /// Keep files at least this large (large-file candidates).
    pub min_bytes: u64,
}

impl Default for Keep {
    fn default() -> Self {
        Keep {
            all: true,
            under: Vec::new(),
            min_bytes: u64::MAX,
        }
    }
}

pub struct Walker<'a> {
    pub env: &'a Env,
    pub cancel: &'a AtomicBool,
    pub progress: &'a Progress,
    pub keep: Keep,
}

static DIR_TICK: AtomicU64 = AtomicU64::new(0);

impl Walker<'_> {
    /// Walk `roots` (already de-duplicated and normalized) into a [`Tree`].
    pub fn walk(&self, roots: &[PathBuf]) -> Tree {
        let results: Vec<(PathBuf, Option<RawEntry>, Leaf, DirResult)> = roots
            .par_iter()
            .map(|root| {
                let Ok(meta) = platform::stat(root) else {
                    return (root.clone(), None, Leaf::Error, DirResult::default());
                };
                if !meta.is_dir {
                    return (root.clone(), Some(meta), Leaf::No, DirResult::default());
                }
                if !meta.is_traversable_dir() {
                    return (root.clone(), Some(meta), Leaf::Link, DirResult::default());
                }
                if protected_paths::classify(root, self.env, false).is_some() {
                    return (
                        root.clone(),
                        Some(meta),
                        Leaf::Excluded,
                        DirResult::default(),
                    );
                }
                let r = self.walk_dir(root, self.keep_min(root, if self.keep.all { 0 } else { self.keep.min_bytes }));
                (root.clone(), Some(meta), Leaf::No, r)
            })
            .collect();

        let mut tree = Tree::default();
        for (path, meta, leaf, result) in results {
            let Some(mut meta) = meta else { continue };
            meta.name = path
                .file_name()
                .map(|n| n.to_os_string())
                .unwrap_or_default();
            let root_id = tree.nodes.len() as u32;
            tree.nodes.push(Node::from_raw(&meta, NO_NODE, leaf));
            tree.roots.push(RootInfo {
                volume: platform::volume_serial(&path),
                path,
                node: root_id,
            });
            let mut queue: VecDeque<(u32, DirResult)> = VecDeque::new();
            queue.push_back((root_id, result));
            while let Some((id, r)) = queue.pop_front() {
                if r.error {
                    tree.nodes[id as usize].leaf = Leaf::Error;
                }
                {
                    let n = &mut tree.nodes[id as usize];
                    n.folded_files = r.folded.0;
                    n.folded_allocated = r.folded.1;
                    n.folded_logical = r.folded.2;
                    n.folded_newest = r.folded.3;
                }
                let start = tree.nodes.len() as u32;
                let count = (r.leaves.len() + r.subdirs.len()) as u32;
                for (e, leaf) in &r.leaves {
                    tree.nodes.push(Node::from_raw(e, id, *leaf));
                }
                for (e, sub) in r.subdirs {
                    let cid = tree.nodes.len() as u32;
                    tree.nodes.push(Node::from_raw(&e, id, Leaf::No));
                    queue.push_back((cid, sub));
                }
                if count > 0 {
                    let n = &mut tree.nodes[id as usize];
                    n.first_child = start;
                    n.child_count = count;
                }
            }
        }
        tree.aggregate();
        tree
    }

    /// Smallest file size kept under `dir`, given the parent's.
    fn keep_min(&self, dir: &Path, parent_min: u64) -> u64 {
        self.keep
            .under
            .iter()
            .filter(|(u, _)| platform::path_starts_with(dir, u))
            .map(|(_, m)| *m)
            .fold(parent_min, u64::min)
    }

    fn walk_dir(&self, path: &Path, keep_min: u64) -> DirResult {
        if self.cancel.load(Ordering::Relaxed) {
            return DirResult::default();
        }
        let entries = match platform::list_dir(path) {
            Ok(e) => e,
            Err(err) => {
                tracing::debug!(path = %path.display(), %err, "cannot list directory");
                return DirResult {
                    error: true,
                    ..DirResult::default()
                };
            }
        };
        self.progress.dirs.fetch_add(1, Ordering::Relaxed);
        if DIR_TICK.fetch_add(1, Ordering::Relaxed) % 64 == 0 {
            self.progress.set_current(path);
        }
        let mut leaves = Vec::new();
        let mut to_walk = Vec::new();
        let mut bytes = 0u64;
        let mut files = 0u64;
        let mut folded = (0u32, 0u64, 0u64, 0i64);
        for e in entries {
            if e.is_dir {
                if !e.is_traversable_dir() {
                    leaves.push((e, Leaf::Link));
                    continue;
                }
                let child = path.join(&e.name);
                if protected_paths::classify(&child, self.env, false).is_some() {
                    leaves.push((e, Leaf::Excluded));
                } else {
                    to_walk.push((e, child));
                }
            } else {
                files += 1;
                bytes += e.allocated;
                if e.logical >= keep_min {
                    leaves.push((e, Leaf::No));
                } else {
                    folded.0 += 1;
                    folded.1 += e.allocated;
                    folded.2 += e.logical;
                    folded.3 = folded.3.max(e.modified);
                }
            }
        }
        self.progress.files.fetch_add(files, Ordering::Relaxed);
        self.progress.bytes_seen.fetch_add(bytes, Ordering::Relaxed);
        let subdirs = to_walk
            .into_par_iter()
            .map(|(e, child)| {
                let r = self.walk_dir(&child, self.keep_min(&child, keep_min));
                (e, r)
            })
            .collect();
        DirResult {
            leaves,
            subdirs,
            error: false,
            folded,
        }
    }
}

/// Drop roots nested inside other roots, normalize, and remove duplicates.
pub fn dedupe_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = roots.iter().map(|r| platform::normalize(r)).collect();
    v.sort_by_key(|p| p.components().count());
    let mut out: Vec<PathBuf> = Vec::new();
    for r in v {
        if !out.iter().any(|o| platform::path_starts_with(&r, o)) {
            out.push(r);
        }
    }
    out
}
