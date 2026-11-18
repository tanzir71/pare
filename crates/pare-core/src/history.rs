//! History, built from the op log (newest first).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::dispose::oplog::{ItemStatus, LogRecord, read_log};
use crate::types::{CategoryId, Timestamp};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryOp {
    pub id: String,
    pub started: Timestamp,
    pub finished: Option<Timestamp>,
    pub title: String,
    pub items: u64,
    pub ok: u64,
    pub failed: u64,
    pub skipped: u64,
    pub freed_now: u64,
    pub recycled: u64,
    /// Disposal kind → item count.
    pub disposals: BTreeMap<String, u64>,
    /// Recycled items not yet restored or purged.
    pub restorable: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub finding_id: String,
    pub title: String,
    pub path: PathBuf,
    pub paths: Vec<PathBuf>,
    pub bytes: u64,
    pub rule: String,
    pub category: CategoryId,
    pub disposal: String,
    pub result: ItemStatus,
    pub error: Option<String>,
    pub freed: u64,
    pub recycled: u64,
    pub ts: Timestamp,
    pub output: Option<String>,
    pub restored: bool,
    pub purged: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDetail {
    pub op: HistoryOp,
    pub items: Vec<HistoryItem>,
}

struct Building {
    op: HistoryOp,
    items: Vec<HistoryItem>,
}

fn build(records: Vec<LogRecord>) -> Vec<Building> {
    let mut ops: BTreeMap<String, Building> = BTreeMap::new();
    let mut restored: Vec<(String, PathBuf)> = Vec::new();
    let mut purged: Vec<(String, PathBuf)> = Vec::new();
    for r in records {
        match r {
            LogRecord::OpBegin { op, ts, title, .. } => {
                ops.insert(
                    op.clone(),
                    Building {
                        op: HistoryOp {
                            id: op,
                            started: ts,
                            finished: None,
                            title,
                            items: 0,
                            ok: 0,
                            failed: 0,
                            skipped: 0,
                            freed_now: 0,
                            recycled: 0,
                            disposals: BTreeMap::new(),
                            restorable: 0,
                        },
                        items: Vec::new(),
                    },
                );
            }
            LogRecord::Item {
                op,
                ts,
                item,
                title,
                path,
                paths,
                bytes,
                rule,
                category,
                disposal,
                result,
                error,
                freed,
                recycled,
                output,
            } => {
                let Some(b) = ops.get_mut(&op) else { continue };
                let new = HistoryItem {
                    finding_id: item.clone(),
                    title,
                    path,
                    paths,
                    bytes,
                    rule,
                    category,
                    disposal,
                    result,
                    error,
                    freed,
                    recycled,
                    ts,
                    output,
                    restored: false,
                    purged: false,
                };
                match b.items.iter_mut().find(|i| i.finding_id == item) {
                    Some(existing) => *existing = new,
                    None => b.items.push(new),
                }
            }
            LogRecord::OpEnd {
                op,
                ts,
                freed_now,
                recycled,
                ..
            } => {
                if let Some(b) = ops.get_mut(&op) {
                    b.op.finished = Some(ts);
                    b.op.freed_now = freed_now;
                    b.op.recycled = recycled;
                }
            }
            LogRecord::Restore {
                op, path, result, ..
            } => {
                if result == ItemStatus::Ok {
                    restored.push((op, path));
                }
            }
            LogRecord::Purge {
                op, path, result, ..
            } => {
                if result == ItemStatus::Ok {
                    purged.push((op, path));
                }
            }
        }
    }
    let mut v: Vec<Building> = ops.into_values().collect();
    for b in &mut v {
        for it in &mut b.items {
            it.restored = restored.iter().any(|(o, p)| o == &b.op.id && p == &it.path);
            it.purged = purged.iter().any(|(o, p)| o == &b.op.id && p == &it.path);
        }
        let op = &mut b.op;
        op.items = b.items.len() as u64;
        op.ok = b
            .items
            .iter()
            .filter(|i| i.result == ItemStatus::Ok)
            .count() as u64;
        op.failed = b
            .items
            .iter()
            .filter(|i| matches!(i.result, ItemStatus::Failed | ItemStatus::Partial))
            .count() as u64;
        op.skipped = b
            .items
            .iter()
            .filter(|i| i.result == ItemStatus::Skipped)
            .count() as u64;
        op.restorable = b
            .items
            .iter()
            .filter(|i| i.recycled > 0 && !i.restored && !i.purged)
            .count() as u64;
        op.disposals.clear();
        for i in &b.items {
            if i.result != ItemStatus::Skipped {
                *op.disposals.entry(i.disposal.clone()).or_default() += 1;
            }
        }
        if op.finished.is_none() {
            op.freed_now = b.items.iter().map(|i| i.freed).sum();
            op.recycled = b.items.iter().map(|i| i.recycled).sum();
        }
    }
    v.sort_by_key(|b| std::cmp::Reverse(b.op.started));
    v
}

/// Operations, newest first.
pub fn list(log_path: &Path) -> Vec<HistoryOp> {
    build(read_log(log_path))
        .into_iter()
        .map(|b| b.op)
        .collect()
}

pub fn get(log_path: &Path, op: &str) -> Option<HistoryDetail> {
    build(read_log(log_path))
        .into_iter()
        .find(|b| b.op.id == op)
        .map(|b| HistoryDetail {
            op: b.op,
            items: b.items,
        })
}

/// Total freed across all operations ("freed so far", stored locally only).
pub fn total_freed(log_path: &Path) -> u64 {
    list(log_path).iter().map(|o| o.freed_now).sum()
}
