//! Apply USN journal changes to an index. Records are reconciled per file: the last record for a
//! file says whether it was deleted, and otherwise where it lives now; its size and times are
//! read fresh from the file system.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;

use super::{IndexEntry, VolumeIndex};
use crate::platform::{ATTR_DIRECTORY, RawEntry};

#[derive(Clone, Debug)]
pub struct Change {
    pub id: u64,
    pub parent: u64,
    pub name: OsString,
    pub deleted: bool,
    pub attrs: u32,
}

/// Returns the number of files that changed.
pub fn apply(
    idx: &mut VolumeIndex,
    changes: &[Change],
    stat: &dyn Fn(&Path) -> Option<RawEntry>,
) -> usize {
    // Last record per file wins.
    let mut last: HashMap<u64, &Change> = HashMap::new();
    let mut order: Vec<u64> = Vec::new();
    for c in changes {
        if c.id == 0 || c.id == idx.root_id {
            continue;
        }
        if last.insert(c.id, c).is_none() {
            order.push(c.id);
        }
    }
    // Deletions and moves first (drop old entries), then (re)insert current state.
    let ids: std::collections::HashSet<u64> = order.iter().copied().collect();
    idx.entries.retain(|e| !ids.contains(&e.id));
    // Parents may be new in this batch, so insert in dependency-friendly order: directories
    // first, then files; resolve paths after each insertion round.
    let mut pending: Vec<&Change> = order
        .iter()
        .filter_map(|id| last.get(id).copied())
        .filter(|c| !c.deleted)
        .collect();
    pending.sort_by_key(|c| c.attrs & ATTR_DIRECTORY == 0);
    let changed = order.len();
    for _ in 0..4 {
        if pending.is_empty() {
            break;
        }
        let by_id = idx.by_id();
        let mut next = Vec::new();
        for c in pending {
            let Some(&pi) = by_id.get(&c.parent) else {
                next.push(c);
                continue;
            };
            let Some(parent_path) = idx.path_of(pi, &by_id) else {
                next.push(c);
                continue;
            };
            let path = parent_path.join(&c.name);
            // A file that's gone again since counts as deleted.
            if let Some(s) = stat(&path) {
                idx.entries.push(IndexEntry {
                    id: c.id,
                    parent: c.parent,
                    name: c.name.clone(),
                    is_dir: s.is_dir,
                    size: s.logical,
                    allocated: s.allocated,
                    attrs: s.attrs,
                    links: s.links.max(1),
                    created: s.created,
                    modified: s.modified,
                    accessed: s.accessed,
                });
            }
        }
        pending = next;
    }
    // Files whose parent never showed up (outside the index) can't be placed; they were
    // already counted as changed.
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::tests::{entry, sample};

    fn raw(size: u64, is_dir: bool) -> RawEntry {
        RawEntry {
            name: OsString::new(),
            is_dir,
            attrs: if is_dir { ATTR_DIRECTORY } else { 0x20 },
            reparse_tag: 0,
            logical: size,
            allocated: size.div_ceil(4096) * 4096,
            file_id: 0,
            links: 1,
            created: 9,
            modified: 9,
            accessed: 9,
        }
    }

    #[test]
    fn create_modify_delete_rename() {
        let mut idx = sample();
        let fs: HashMap<String, RawEntry> = [
            ("Users/ana/new-dir", raw(0, true)),
            ("Users/ana/new-dir/new.txt", raw(1234, false)),
            ("Users/ana/notes.txt", raw(5_000_000, false)),
            ("Users/ana/Downloads/setup-renamed.exe", raw(5000, false)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        let root = idx.root.clone();
        let stat = move |p: &Path| {
            let rel = p
                .strip_prefix(&root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            fs.get(&rel).cloned()
        };
        let changes = vec![
            // a file created inside a directory created in the same batch
            Change {
                id: 31,
                parent: 30,
                name: "new.txt".into(),
                deleted: false,
                attrs: 0x20,
            },
            Change {
                id: 30,
                parent: 11,
                name: "new-dir".into(),
                deleted: false,
                attrs: ATTR_DIRECTORY,
            },
            // notes.txt grew
            Change {
                id: 14,
                parent: 11,
                name: "notes.txt".into(),
                deleted: false,
                attrs: 0x20,
            },
            // movie.mkv deleted (both its names go)
            Change {
                id: 15,
                parent: 11,
                name: "movie.mkv".into(),
                deleted: true,
                attrs: 0x20,
            },
            // setup.exe renamed
            Change {
                id: 13,
                parent: 12,
                name: "setup.exe".into(),
                deleted: false,
                attrs: 0x20,
            },
            Change {
                id: 13,
                parent: 12,
                name: "setup-renamed.exe".into(),
                deleted: false,
                attrs: 0x20,
            },
        ];
        let n = apply(&mut idx, &changes, &stat);
        assert_eq!(n, 5);
        let find = |id: u64| {
            idx.entries
                .iter()
                .filter(|e| e.id == id)
                .collect::<Vec<_>>()
        };
        assert_eq!(find(31).len(), 1);
        assert_eq!(find(31)[0].size, 1234);
        assert_eq!(find(30).len(), 1);
        assert_eq!(find(14)[0].size, 5_000_000);
        assert!(
            find(15).is_empty(),
            "deleted file and its hard link are gone"
        );
        assert_eq!(find(13).len(), 1);
        assert_eq!(find(13)[0].name, OsString::from("setup-renamed.exe"));
        let _ = entry(1, 1, "x", false, 0, 0);
    }
}
