//! Append-only operation log, `%LOCALAPPDATA%\Pare\logs\operations.jsonl` (invariant 8).
//! Every item is written as `pending` and synced to disk *before* its disposal runs, then a
//! second record carries the result.

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::types::{CategoryId, Timestamp};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ItemStatus {
    Pending,
    Ok,
    Partial,
    Failed,
    Skipped,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LogRecord {
    #[serde(rename_all = "camelCase")]
    OpBegin {
        op: String,
        ts: Timestamp,
        title: String,
        items: u64,
        bytes: u64,
    },
    #[serde(rename_all = "camelCase")]
    Item {
        op: String,
        ts: Timestamp,
        item: String,
        title: String,
        /// Primary path (the first of `paths`).
        path: PathBuf,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        paths: Vec<PathBuf>,
        bytes: u64,
        rule: String,
        category: CategoryId,
        disposal: String,
        result: ItemStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(default)]
        freed: u64,
        #[serde(default)]
        recycled: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    OpEnd {
        op: String,
        ts: Timestamp,
        freed_now: u64,
        recycled: u64,
        ok: u64,
        failed: u64,
        skipped: u64,
    },
    /// Items restored from the Recycle Bin (M5).
    #[serde(rename_all = "camelCase")]
    Restore {
        op: String,
        ts: Timestamp,
        path: PathBuf,
        result: ItemStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// Pare-recycled items permanently removed from the bin (M5).
    #[serde(rename_all = "camelCase")]
    Purge {
        op: String,
        ts: Timestamp,
        path: PathBuf,
        bytes: u64,
        result: ItemStatus,
    },
}

pub struct OpLog {
    path: PathBuf,
    file: Mutex<File>,
}

impl OpLog {
    pub fn open(path: &Path) -> io::Result<OpLog> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(OpLog {
            path: path.to_path_buf(),
            file: Mutex::new(file),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append records and flush them to disk.
    pub fn write_all(&self, records: &[LogRecord]) -> io::Result<()> {
        let mut buf = Vec::new();
        for r in records {
            serde_json::to_writer(&mut buf, r).map_err(io::Error::other)?;
            buf.push(b'\n');
        }
        let mut f = self
            .file
            .lock()
            .map_err(|_| io::Error::other("op log poisoned"))?;
        f.write_all(&buf)?;
        f.flush()?;
        f.sync_data()
    }

    pub fn write(&self, record: &LogRecord) -> io::Result<()> {
        self.write_all(std::slice::from_ref(record))
    }
}

/// Read every record; unreadable lines are skipped.
pub fn read_log(path: &Path) -> Vec<LogRecord> {
    let Ok(f) = File::open(path) else {
        return Vec::new();
    };
    BufReader::new(f)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect()
}
