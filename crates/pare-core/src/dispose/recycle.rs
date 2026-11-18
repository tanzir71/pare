//! Pare's own Recycle Bin items: restore them (History) and empty only what Pare moved
//! ("Empty Pare's items now", automatic purge after 30 days). Never touches other bin items.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::oplog::{ItemStatus, LogRecord, OpLog, read_log};
use crate::platform::{self, now_ms};
use crate::types::{MS_PER_DAY, Timestamp};

/// One path Pare moved to the Recycle Bin, from the op log.
#[derive(Clone, Debug, PartialEq)]
pub struct RecycledByPare {
    pub op: String,
    pub item: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub ts: Timestamp,
}

/// Recycled paths from the log that haven't been restored or purged since.
pub fn recycled_by_pare(log_path: &Path) -> Vec<RecycledByPare> {
    let records = read_log(log_path);
    let mut out: Vec<RecycledByPare> = Vec::new();
    for r in &records {
        if let LogRecord::Item {
            op,
            item,
            path,
            paths,
            recycled,
            result,
            ts,
            ..
        } = r
        {
            if *recycled == 0 || !matches!(result, ItemStatus::Ok | ItemStatus::Partial) {
                continue;
            }
            let all: Vec<PathBuf> = if paths.is_empty() {
                vec![path.clone()]
            } else {
                paths.clone()
            };
            let share = *recycled / all.len().max(1) as u64;
            for p in all {
                out.push(RecycledByPare {
                    op: op.clone(),
                    item: item.clone(),
                    path: p,
                    bytes: share,
                    ts: *ts,
                });
            }
        }
    }
    for r in &records {
        match r {
            LogRecord::Restore {
                op,
                path,
                result: ItemStatus::Ok,
                ..
            }
            | LogRecord::Purge {
                op,
                path,
                result: ItemStatus::Ok,
                ..
            } => {
                out.retain(|x| !(x.op == *op && x.path == *path));
            }
            _ => {}
        }
    }
    out
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub restored: u64,
    /// Original location is occupied again: reported, never overwritten.
    pub collisions: Vec<PathBuf>,
    /// No longer in the Recycle Bin (emptied elsewhere).
    pub missing: Vec<PathBuf>,
    pub failed: Vec<(PathBuf, String)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PurgeReport {
    pub items: u64,
    pub bytes: u64,
    pub failed: u64,
}

#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
fn matching_trash_items(
    wanted: &[RecycledByPare],
) -> Result<Vec<(RecycledByPare, trash::TrashItem)>, String> {
    let items = trash::os_limited::list().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for w in wanted {
        // Same original path, deleted within 10 minutes of the logged time.
        let best = items
            .iter()
            .filter(|t| {
                let orig = t.original_path();
                platform::path_starts_with(&orig, &w.path)
                    && platform::path_starts_with(&w.path, &orig)
            })
            .min_by_key(|t| (t.time_deleted * 1000 - w.ts).abs());
        if let Some(t) = best {
            if (t.time_deleted * 1000 - w.ts).abs() <= 10 * 60 * 1000 {
                out.push((w.clone(), t.clone()));
            }
        }
    }
    Ok(out)
}

/// Restore Pare-recycled items of one operation (optionally only some finding ids).
#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
pub fn restore(log: &OpLog, op: &str, items: Option<&[String]>) -> RestoreReport {
    let mut report = RestoreReport::default();
    let wanted: Vec<RecycledByPare> = recycled_by_pare(log.path())
        .into_iter()
        .filter(|r| r.op == op && items.is_none_or(|ids| ids.contains(&r.item)))
        .collect();
    let matched = match matching_trash_items(&wanted) {
        Ok(m) => m,
        Err(e) => {
            report.failed = wanted.into_iter().map(|w| (w.path, e.clone())).collect();
            return report;
        }
    };
    for w in &wanted {
        if !matched.iter().any(|(m, _)| m.path == w.path) {
            report.missing.push(w.path.clone());
        }
    }
    for (w, t) in matched {
        let (result, error) = if platform::stat(&w.path).is_ok() {
            report.collisions.push(w.path.clone());
            (
                ItemStatus::Skipped,
                Some("Something already exists at the original location".to_string()),
            )
        } else {
            match trash::os_limited::restore_all([t]) {
                Ok(()) => {
                    report.restored += 1;
                    (ItemStatus::Ok, None)
                }
                Err(e) => {
                    report.failed.push((w.path.clone(), e.to_string()));
                    (ItemStatus::Failed, Some(e.to_string()))
                }
            }
        };
        let _ = log.write(&LogRecord::Restore {
            op: w.op.clone(),
            ts: now_ms(),
            path: w.path.clone(),
            result,
            error,
        });
    }
    report
}

/// Permanently remove items *Pare* moved to the Recycle Bin: those of `op`, or (with `None`)
/// all of them older than `older_than_days`.
#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
pub fn purge_pare_items(
    log: &OpLog,
    op: Option<&str>,
    older_than_days: Option<u32>,
) -> PurgeReport {
    let now = now_ms();
    let wanted: Vec<RecycledByPare> = recycled_by_pare(log.path())
        .into_iter()
        .filter(|r| op.is_none_or(|o| r.op == o))
        .filter(|r| older_than_days.is_none_or(|d| now - r.ts >= d as i64 * MS_PER_DAY))
        .collect();
    let mut report = PurgeReport::default();
    let Ok(matched) = matching_trash_items(&wanted) else {
        report.failed = wanted.len() as u64;
        return report;
    };
    for (w, t) in matched {
        let ok = trash::os_limited::purge_all([t]).is_ok();
        if ok {
            report.items += 1;
            report.bytes += w.bytes;
        } else {
            report.failed += 1;
        }
        let _ = log.write(&LogRecord::Purge {
            op: w.op.clone(),
            ts: now_ms(),
            path: w.path.clone(),
            bytes: w.bytes,
            result: if ok {
                ItemStatus::Ok
            } else {
                ItemStatus::Failed
            },
        });
    }
    report
}
