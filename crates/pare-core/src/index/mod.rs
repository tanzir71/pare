//! Whole-drive index (M6). In fast mode (elevated, NTFS) the volume's MFT is read directly and
//! kept as a flat list of entries; the USN change journal then brings it up to date in about a
//! second instead of rescanning. The index converts into the same [`Tree`] the walker builds, so
//! every detector works on either.

#[cfg(windows)]
pub mod mft;
pub mod store;
pub mod usn;

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::platform::{self, ATTR_DIRECTORY, ATTR_REPARSE_POINT};
use crate::safety::protected_paths;
use crate::scan::tree::{Leaf, NO_NODE, Node, NodeId, RootInfo, Tree};
use crate::scan::walker::Keep;
use crate::types::Timestamp;

/// One name of one file (a hard-linked file has one entry per link).
#[derive(Clone, Debug, PartialEq)]
pub struct IndexEntry {
    /// NTFS file reference (record number | sequence << 48).
    pub id: u64,
    pub parent: u64,
    pub name: OsString,
    pub is_dir: bool,
    pub size: u64,
    pub allocated: u64,
    pub attrs: u32,
    pub links: u32,
    pub created: Timestamp,
    pub modified: Timestamp,
    pub accessed: Timestamp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JournalPos {
    pub journal_id: u64,
    pub usn: i64,
}

#[derive(Clone, Debug, Default)]
pub struct VolumeIndex {
    /// e.g. `C:\`
    pub root: PathBuf,
    pub root_id: u64,
    pub serial: u64,
    pub entries: Vec<IndexEntry>,
    pub journal: Option<JournalPos>,
    pub built: Timestamp,
    pub build_ms: u64,
}

impl VolumeIndex {
    fn children(&self) -> HashMap<u64, Vec<usize>> {
        let mut m: HashMap<u64, Vec<usize>> = HashMap::with_capacity(self.entries.len() / 4);
        for (i, e) in self.entries.iter().enumerate() {
            if e.id != self.root_id {
                m.entry(e.parent).or_default().push(i);
            }
        }
        m
    }

    /// Full path of an entry, following parents up to the root (None if the chain breaks).
    pub fn path_of(&self, idx: usize, by_id: &HashMap<u64, usize>) -> Option<PathBuf> {
        let mut parts: Vec<&OsStr> = Vec::new();
        let mut cur = idx;
        for _ in 0..512 {
            let e = &self.entries[cur];
            if e.id == self.root_id {
                return Some(parts.iter().rev().fold(self.root.clone(), |a, p| a.join(p)));
            }
            parts.push(&e.name);
            cur = *by_id.get(&e.parent)?;
        }
        None
    }

    /// Map file id → first entry (directories have one entry).
    pub fn by_id(&self) -> HashMap<u64, usize> {
        let mut m = HashMap::with_capacity(self.entries.len());
        for (i, e) in self.entries.iter().enumerate() {
            m.entry(e.id).or_insert(i);
        }
        m
    }
}

/// Subtree totals per entry (files, allocated, logical, newest), each file id counted once.
fn subtree_totals(
    idx: &VolumeIndex,
    children: &HashMap<u64, Vec<usize>>,
) -> Vec<(u64, u64, u64, Timestamp)> {
    let n = idx.entries.len();
    let mut totals = vec![(0u64, 0u64, 0u64, 0i64); n];
    // Iterative post-order from every directory reachable from the root.
    let by_id = idx.by_id();
    let Some(&root) = by_id.get(&idx.root_id) else {
        return totals;
    };
    let mut order: Vec<usize> = Vec::with_capacity(n);
    let mut stack = vec![root];
    let mut seen = vec![false; n];
    while let Some(i) = stack.pop() {
        if seen[i] {
            continue;
        }
        seen[i] = true;
        order.push(i);
        let e = &idx.entries[i];
        if e.is_dir && e.attrs & ATTR_REPARSE_POINT == 0 {
            if let Some(ch) = children.get(&e.id) {
                stack.extend(ch.iter().copied());
            }
        }
    }
    let mut counted = std::collections::HashSet::new();
    for &i in order.iter().rev() {
        let e = &idx.entries[i];
        if !e.is_dir {
            let own = counted.insert(e.id);
            totals[i] = (
                1,
                if own { e.allocated } else { 0 },
                if own { e.size } else { 0 },
                e.modified,
            );
            continue;
        }
        let mut t = (0, 0, 0, e.modified);
        if e.attrs & ATTR_REPARSE_POINT == 0 {
            if let Some(ch) = children.get(&e.id) {
                for &c in ch {
                    let x = totals[c];
                    t.0 += x.0;
                    t.1 += x.1;
                    t.2 += x.2;
                    t.3 = t.3.max(x.3);
                }
            }
        }
        totals[i] = t;
    }
    totals
}

fn raw_of(e: &IndexEntry) -> platform::RawEntry {
    platform::RawEntry {
        name: e.name.clone(),
        is_dir: e.is_dir,
        attrs: e.attrs | if e.is_dir { ATTR_DIRECTORY } else { 0 },
        reparse_tag: if e.attrs & ATTR_REPARSE_POINT != 0 {
            platform::IO_REPARSE_TAG_MOUNT_POINT
        } else {
            0
        },
        logical: e.size,
        allocated: e.allocated,
        file_id: e.id as u128,
        links: e.links,
        created: e.created,
        modified: e.modified,
        accessed: e.accessed,
    }
}

/// Build the scan tree for this volume. Hard-excluded folders (Windows, Program Files…) and
/// links become leaves whose contents are counted but never listed, exactly as the walker
/// would treat them; files below the keep thresholds fold into their folder's totals.
pub fn to_tree(idx: &VolumeIndex, keep: &Keep, env: &Env, files_seen: &mut u64) -> Tree {
    let children = idx.children();
    let totals = subtree_totals(idx, &children);
    let by_id = idx.by_id();
    let mut tree = Tree::default();
    let Some(&root) = by_id.get(&idx.root_id) else {
        return tree;
    };
    let mut root_raw = raw_of(&idx.entries[root]);
    root_raw.name = OsString::new();
    tree.nodes
        .push(Node::from_raw(&root_raw, NO_NODE, Leaf::No));
    tree.roots.push(RootInfo {
        path: idx.root.clone(),
        node: 0,
        volume: Some(idx.serial),
    });
    let base_min = if keep.all { 0 } else { keep.min_bytes };
    // (entry index, node id, path, keep_min)
    let mut queue: std::collections::VecDeque<(usize, NodeId, PathBuf, u64)> =
        std::collections::VecDeque::new();
    queue.push_back((root, 0, idx.root.clone(), base_min));
    while let Some((ei, nid, path, keep_min)) = queue.pop_front() {
        let Some(ch) = children.get(&idx.entries[ei].id) else {
            continue;
        };
        let start = tree.nodes.len() as u32;
        let mut count = 0u32;
        let mut folded = (0u32, 0u64, 0u64, 0i64);
        let mut subdirs = Vec::new();
        for &c in ch {
            let e = &idx.entries[c];
            if !e.is_dir {
                *files_seen += 1;
                crate::scan::kinds::add(
                    &mut tree.kinds,
                    crate::scan::kinds::FileKind::of(&e.name),
                    e.allocated,
                );
                if e.size >= keep_min {
                    tree.nodes.push(Node::from_raw(&raw_of(e), nid, Leaf::No));
                    count += 1;
                } else {
                    folded.0 += 1;
                    folded.1 += e.allocated;
                    folded.2 += e.size;
                    folded.3 = folded.3.max(e.modified);
                }
                continue;
            }
            let child_path = path.join(&e.name);
            let leaf = if e.attrs & ATTR_REPARSE_POINT != 0 {
                Leaf::Link
            } else if protected_paths::classify(&child_path, env, false).is_some() {
                Leaf::Excluded
            } else {
                Leaf::No
            };
            let id = tree.nodes.len() as u32;
            let mut node = Node::from_raw(&raw_of(e), nid, leaf);
            if leaf != Leaf::No {
                // Count what's inside, never list it.
                let t = totals[c];
                node.folded_files = t.0 as u32;
                node.folded_allocated = t.1;
                node.folded_logical = t.2;
                node.folded_newest = t.3;
                *files_seen += t.0;
            }
            tree.nodes.push(node);
            count += 1;
            if leaf == Leaf::No {
                let child_min = keep
                    .under
                    .iter()
                    .filter(|(u, _)| platform::path_starts_with(&child_path, u))
                    .map(|(_, m)| *m)
                    .fold(keep_min, u64::min);
                subdirs.push((c, id, child_path, child_min));
            }
        }
        {
            let n = &mut tree.nodes[nid as usize];
            if count > 0 {
                n.first_child = start;
                n.child_count = count;
            }
            n.folded_files = folded.0;
            n.folded_allocated = folded.1;
            n.folded_logical = folded.2;
            n.folded_newest = folded.3;
        }
        queue.extend(subdirs);
    }
    tree.aggregate();
    tree
}

/// Merge per-volume trees into one (roots in order).
pub fn merge_trees(trees: Vec<Tree>) -> Tree {
    let mut out = Tree::default();
    for t in trees {
        crate::scan::kinds::merge(&mut out.kinds, &t.kinds);
        let off = out.nodes.len() as u32;
        for mut n in t.nodes {
            if n.parent != NO_NODE {
                n.parent += off;
            }
            if n.first_child != NO_NODE {
                n.first_child += off;
            }
            out.nodes.push(n);
        }
        for mut r in t.roots {
            r.node += off;
            out.roots.push(r);
        }
    }
    out
}

/// `%LOCALAPPDATA%\Pare\index\<serial>.db`
pub fn db_path(env: &Env, serial: u64) -> PathBuf {
    env.index_dir().join(format!("{serial:016x}.db"))
}

/// Can this drive use fast mode? (NTFS, and this process is elevated.)
pub fn fast_mode_available(root: &Path) -> bool {
    platform::is_elevated()
        && platform::list_drives().iter().any(|d| {
            d.is_ntfs
                && platform::path_starts_with(root, &d.root)
                && platform::path_starts_with(&d.root, root)
        })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn entry(
        id: u64,
        parent: u64,
        name: &str,
        is_dir: bool,
        size: u64,
        modified: Timestamp,
    ) -> IndexEntry {
        IndexEntry {
            id,
            parent,
            name: name.into(),
            is_dir,
            size,
            allocated: size.div_ceil(4096) * 4096,
            attrs: if is_dir { ATTR_DIRECTORY } else { 0x20 },
            links: 1,
            created: modified,
            modified,
            accessed: modified,
        }
    }

    pub fn sample() -> VolumeIndex {
        let mut v = VolumeIndex {
            root: PathBuf::from(if cfg!(windows) { r"Z:\" } else { "/z" }),
            root_id: 5,
            serial: 7,
            ..Default::default()
        };
        v.entries = vec![
            entry(5, 5, "", true, 0, 1),
            entry(10, 5, "Users", true, 0, 1),
            entry(11, 10, "ana", true, 0, 1),
            entry(12, 11, "Downloads", true, 0, 1),
            entry(13, 12, "setup.exe", false, 5000, 2),
            entry(14, 11, "notes.txt", false, 100, 3),
            entry(15, 11, "movie.mkv", false, 900_000_000, 4),
            entry(20, 5, "Windows", true, 0, 1),
            entry(21, 20, "big.dll", false, 50_000_000, 5),
        ];
        // A hard link: the same file (id 15) under a second name.
        let mut link = entry(15, 12, "movie-link.mkv", false, 900_000_000, 4);
        link.links = 2;
        v.entries.push(link);
        v
    }

    #[test]
    fn index_to_tree_folds_and_excludes() {
        let d = tempfile::tempdir().unwrap();
        let mut env = Env::fake(d.path());
        env.downloads = PathBuf::from(if cfg!(windows) {
            r"Z:\Users\ana\Downloads"
        } else {
            "/z/Users/ana/Downloads"
        });
        let keep = Keep {
            all: false,
            under: vec![(env.downloads.clone(), 0)],
            min_bytes: 500_000_000,
        };
        let idx = sample();
        let mut seen = 0;
        let tree = to_tree(&idx, &keep, &env, &mut seen);
        let root = tree.roots[0].node;
        let ana = tree.find(&idx.root.join("Users").join("ana")).unwrap();
        // notes.txt is folded (small, outside Downloads); movie.mkv is kept (large).
        assert!(
            tree.find(&idx.root.join("Users").join("ana").join("notes.txt"))
                .is_none()
        );
        assert!(
            tree.find(&idx.root.join("Users").join("ana").join("movie.mkv"))
                .is_some()
        );
        // Everything in Downloads is kept.
        assert!(tree.find(&env.downloads.join("setup.exe")).is_some());
        assert_eq!(tree.node(ana).sub_files, 4);
        // The hard link is counted once.
        let movie_alloc = 900_000_000u64.div_ceil(4096) * 4096;
        assert_eq!(tree.node(ana).sub_allocated, movie_alloc + 4096 + 8192);
        if cfg!(windows) {
            // Windows is excluded: counted, never listed.
            let win = tree.find(&idx.root.join("Windows")).unwrap();
            assert_eq!(tree.node(win).leaf, Leaf::Excluded);
            assert_eq!(tree.node(win).child_count, 0);
            assert_eq!(tree.node(win).sub_files, 1);
            assert!(tree.node(root).sub_allocated >= tree.node(ana).sub_allocated + 50_000_000);
        }
        assert_eq!(seen, 5);
    }

    #[test]
    fn merge_keeps_links_consistent() {
        let d = tempfile::tempdir().unwrap();
        let env = Env::fake(d.path());
        let mut seen = 0;
        let a = to_tree(&sample(), &Keep::default(), &env, &mut seen);
        let b = to_tree(&sample(), &Keep::default(), &env, &mut seen);
        let n = a.nodes.len();
        let m = merge_trees(vec![a, b]);
        assert_eq!(m.roots.len(), 2);
        assert_eq!(m.roots[1].node as usize, n);
        let second_root = m.node(m.roots[1].node);
        assert!(second_root.first_child as usize > n);
    }
}
