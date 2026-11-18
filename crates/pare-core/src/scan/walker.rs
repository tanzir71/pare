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
}

pub struct Walker<'a> {
    pub env: &'a Env,
    pub cancel: &'a AtomicBool,
    pub progress: &'a Progress,
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
                let r = self.walk_dir(root);
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

    fn walk_dir(&self, path: &Path) -> DirResult {
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
                leaves.push((e, Leaf::No));
            }
        }
        self.progress.files.fetch_add(files, Ordering::Relaxed);
        self.progress.bytes_seen.fetch_add(bytes, Ordering::Relaxed);
        let subdirs = to_walk
            .into_par_iter()
            .map(|(e, child)| {
                let r = self.walk_dir(&child);
                (e, r)
            })
            .collect();
        DirResult {
            leaves,
            subdirs,
            error: false,
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
