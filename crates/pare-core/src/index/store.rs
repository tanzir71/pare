//! Persist an index in SQLite (`%LOCALAPPDATA%\Pare\index\<serial>.db`) so the next start can
//! catch up from the saved USN instead of reading the whole MFT again.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};

use super::{IndexEntry, JournalPos, VolumeIndex};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS entries (
  id INTEGER NOT NULL, parent INTEGER NOT NULL, name BLOB NOT NULL, is_dir INTEGER NOT NULL,
  size INTEGER NOT NULL, alloc INTEGER NOT NULL, attrs INTEGER NOT NULL, links INTEGER NOT NULL,
  ctime INTEGER NOT NULL, mtime INTEGER NOT NULL, atime INTEGER NOT NULL
);";

fn name_bytes(n: &std::ffi::OsStr) -> &[u8] {
    n.as_encoded_bytes()
}

fn name_from(b: Vec<u8>) -> OsString {
    // SAFETY: the bytes were produced by `as_encoded_bytes` on this platform (see `save`).
    unsafe { OsString::from_encoded_bytes_unchecked(b) }
}

pub fn save(idx: &VolumeIndex, path: &Path) -> rusqlite::Result<()> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("db.tmp");
    let _ = std::fs::remove_file(&tmp);
    {
        let mut db = Connection::open(&tmp)?;
        db.execute_batch("PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF;")?;
        db.execute_batch(SCHEMA)?;
        let tx = db.transaction()?;
        {
            let mut m = tx.prepare("INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)")?;
            m.execute(params!["root", idx.root.to_string_lossy()])?;
            m.execute(params!["root_id", idx.root_id.to_string()])?;
            m.execute(params!["serial", idx.serial.to_string()])?;
            m.execute(params!["built", idx.built.to_string()])?;
            m.execute(params!["build_ms", idx.build_ms.to_string()])?;
            if let Some(j) = idx.journal {
                m.execute(params!["journal_id", j.journal_id.to_string()])?;
                m.execute(params!["usn", j.usn.to_string()])?;
            }
            let mut ins = tx.prepare(
                "INSERT INTO entries (id, parent, name, is_dir, size, alloc, attrs, links, ctime, mtime, atime)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;
            for e in &idx.entries {
                ins.execute(params![
                    e.id as i64,
                    e.parent as i64,
                    name_bytes(&e.name),
                    e.is_dir,
                    e.size as i64,
                    e.allocated as i64,
                    e.attrs,
                    e.links,
                    e.created,
                    e.modified,
                    e.accessed
                ])?;
            }
        }
        tx.commit()?;
    }
    std::fs::rename(&tmp, path)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    Ok(())
}

pub fn load(path: &Path) -> rusqlite::Result<VolumeIndex> {
    let db = Connection::open(path)?;
    let meta = |k: &str| -> Option<String> {
        db.query_row("SELECT value FROM meta WHERE key = ?1", [k], |r| r.get(0))
            .ok()
    };
    let num = |k: &str| meta(k).and_then(|v| v.parse::<i64>().ok());
    let mut idx = VolumeIndex {
        root: PathBuf::from(meta("root").unwrap_or_default()),
        root_id: meta("root_id").and_then(|v| v.parse().ok()).unwrap_or(0),
        serial: meta("serial").and_then(|v| v.parse().ok()).unwrap_or(0),
        built: num("built").unwrap_or(0),
        build_ms: num("build_ms").unwrap_or(0) as u64,
        journal: match (meta("journal_id").and_then(|v| v.parse().ok()), num("usn")) {
            (Some(journal_id), Some(usn)) => Some(JournalPos { journal_id, usn }),
            _ => None,
        },
        entries: Vec::new(),
    };
    let mut q = db.prepare("SELECT id, parent, name, is_dir, size, alloc, attrs, links, ctime, mtime, atime FROM entries")?;
    let rows = q.query_map([], |r| {
        Ok(IndexEntry {
            id: r.get::<_, i64>(0)? as u64,
            parent: r.get::<_, i64>(1)? as u64,
            name: name_from(r.get(2)?),
            is_dir: r.get(3)?,
            size: r.get::<_, i64>(4)? as u64,
            allocated: r.get::<_, i64>(5)? as u64,
            attrs: r.get(6)?,
            links: r.get(7)?,
            created: r.get(8)?,
            modified: r.get(9)?,
            accessed: r.get(10)?,
        })
    })?;
    for e in rows {
        idx.entries.push(e?);
    }
    Ok(idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let d = tempfile::tempdir().unwrap();
        let mut idx = crate::index::tests::sample();
        idx.journal = Some(JournalPos {
            journal_id: 42,
            usn: 1_000_000,
        });
        idx.built = 123;
        let p = d.path().join("idx.db");
        save(&idx, &p).unwrap();
        let back = load(&p).unwrap();
        assert_eq!(back.entries, idx.entries);
        assert_eq!(back.journal, idx.journal);
        assert_eq!(back.root, idx.root);
        assert_eq!(back.root_id, 5);
        assert_eq!(back.built, 123);
    }
}
