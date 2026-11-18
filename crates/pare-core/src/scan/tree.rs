//! In-memory tree of everything a scan saw. Children of a node are stored contiguously, parents
//! always precede their children, so bottom-up aggregation is a reverse pass.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::platform::{ATTR_HIDDEN, ATTR_SYSTEM, RawEntry};
use crate::types::Timestamp;

pub type NodeId = u32;
pub const NO_NODE: NodeId = u32::MAX;

/// Why a directory's contents are not in the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leaf {
    /// Walked normally (or a file).
    No,
    /// Junction, symlink or other reparse point: never traversed.
    Link,
    /// Hard-excluded root (Windows, Program Files, ~\.ssh…).
    Excluded,
    /// Could not be listed (access denied, vanished).
    Error,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub name: Box<OsStr>,
    pub parent: NodeId,
    pub first_child: NodeId,
    pub child_count: u32,
    pub is_dir: bool,
    pub leaf: Leaf,
    pub attrs: u32,
    pub reparse_tag: u32,
    pub logical: u64,
    pub allocated: u64,
    pub file_id: u128,
    pub links: u32,
    pub created: Timestamp,
    pub modified: Timestamp,
    pub accessed: Timestamp,
    // Aggregates (directories include their whole subtree; files equal themselves).
    pub sub_allocated: u64,
    pub sub_logical: u64,
    pub sub_files: u64,
    /// Newest modification time of any file in the subtree, or of the dir itself.
    pub sub_newest: Timestamp,
    /// Number of leaves in the subtree that were not walked (links, excluded, errors).
    pub sub_opaque: u32,
    /// Files in this directory that weren't kept as nodes (whole-drive scans keep only the
    /// files detectors need); they still count toward every total.
    pub folded_files: u32,
    pub folded_allocated: u64,
    pub folded_logical: u64,
    pub folded_newest: Timestamp,
}

impl Node {
    pub fn from_raw(e: &RawEntry, parent: NodeId, leaf: Leaf) -> Node {
        Node {
            name: e.name.clone().into_boxed_os_str(),
            parent,
            first_child: NO_NODE,
            child_count: 0,
            is_dir: e.is_dir,
            leaf,
            attrs: e.attrs,
            reparse_tag: e.reparse_tag,
            logical: e.logical,
            allocated: e.allocated,
            file_id: e.file_id,
            links: e.links,
            created: e.created,
            modified: e.modified,
            accessed: e.accessed,
            sub_allocated: 0,
            sub_logical: 0,
            sub_files: 0,
            sub_newest: e.modified,
            sub_opaque: 0,
            folded_files: 0,
            folded_allocated: 0,
            folded_logical: 0,
            folded_newest: 0,
        }
    }

    pub fn is_hidden_or_system(&self) -> bool {
        self.attrs & (ATTR_HIDDEN | ATTR_SYSTEM) != 0
    }

    pub fn is_cloud_placeholder(&self) -> bool {
        crate::platform::is_cloud_attrs(self.attrs)
    }

    pub fn extension_lower(&self) -> Option<String> {
        Path::new(&*self.name)
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
    }
}

/// One scanned root.
#[derive(Clone, Debug)]
pub struct RootInfo {
    pub path: PathBuf,
    pub node: NodeId,
    pub volume: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub roots: Vec<RootInfo>,
}

impl Tree {
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    pub fn children(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let n = self.node(id);
        let start = n.first_child;
        let count = if start == NO_NODE { 0 } else { n.child_count };
        (0..count).map(move |i| start + i)
    }

    /// The root this node belongs to.
    pub fn root_of(&self, mut id: NodeId) -> NodeId {
        while self.node(id).parent != NO_NODE {
            id = self.node(id).parent;
        }
        id
    }

    /// Full path of a node.
    pub fn path(&self, id: NodeId) -> PathBuf {
        let mut parts: Vec<&OsStr> = Vec::new();
        let mut cur = id;
        loop {
            let n = self.node(cur);
            if n.parent == NO_NODE {
                break;
            }
            parts.push(&n.name);
            cur = n.parent;
        }
        let root = self
            .roots
            .iter()
            .find(|r| r.node == cur)
            .map(|r| r.path.clone())
            .unwrap_or_default();
        parts.iter().rev().fold(root, |acc, p| acc.join(p))
    }

    /// Find the node for an absolute path (exact components, case-insensitive on Windows).
    pub fn find(&self, path: &Path) -> Option<NodeId> {
        for r in &self.roots {
            if !crate::platform::path_starts_with(path, &r.path) {
                continue;
            }
            let mut cur = r.node;
            let rest = path.components().skip(r.path.components().count());
            let mut ok = true;
            for c in rest {
                match self
                    .children(cur)
                    .find(|&ch| crate::platform::eq_component(&self.node(ch).name, c.as_os_str()))
                {
                    Some(ch) => cur = ch,
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                return Some(cur);
            }
        }
        None
    }

    /// Depth-first iterator over `id` and every descendant.
    pub fn descendants(&self, id: NodeId) -> Descendants<'_> {
        Descendants {
            tree: self,
            stack: vec![id],
        }
    }

    /// Compute subtree aggregates, counting each file id once (hard links within the scan).
    pub fn aggregate(&mut self) {
        let mut seen: HashSet<u128> = HashSet::new();
        // Files first: decide which occurrence of a hard-linked file "owns" the bytes.
        let mut own = vec![true; self.nodes.len()];
        for (i, n) in self.nodes.iter().enumerate() {
            if !n.is_dir && n.file_id != 0 && !seen.insert(n.file_id) {
                own[i] = false;
            }
        }
        for (n, owns) in self.nodes.iter_mut().zip(own.iter().copied()) {
            if n.is_dir {
                n.sub_allocated = n.folded_allocated;
                n.sub_logical = n.folded_logical;
                n.sub_files = n.folded_files as u64;
                n.sub_newest = n.modified.max(n.folded_newest);
                n.sub_opaque = u32::from(n.leaf != Leaf::No);
            } else {
                n.sub_allocated = if owns { n.allocated } else { 0 };
                n.sub_logical = if owns { n.logical } else { 0 };
                n.sub_files = 1;
                n.sub_newest = n.modified;
                n.sub_opaque = 0;
            }
        }
        for i in (0..self.nodes.len()).rev() {
            let p = self.nodes[i].parent;
            if p == NO_NODE {
                continue;
            }
            let (a, l, f, newest, opaque) = {
                let n = &self.nodes[i];
                (
                    n.sub_allocated,
                    n.sub_logical,
                    n.sub_files,
                    n.sub_newest,
                    n.sub_opaque,
                )
            };
            let parent = &mut self.nodes[p as usize];
            parent.sub_allocated += a;
            parent.sub_logical += l;
            parent.sub_files += f;
            parent.sub_newest = parent.sub_newest.max(newest);
            parent.sub_opaque += opaque;
        }
    }
}

pub struct Descendants<'a> {
    tree: &'a Tree,
    stack: Vec<NodeId>,
}

impl Iterator for Descendants<'_> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        let id = self.stack.pop()?;
        let n = self.tree.node(id);
        if n.first_child != NO_NODE {
            for i in (0..n.child_count).rev() {
                self.stack.push(n.first_child + i);
            }
        }
        Some(id)
    }
}
