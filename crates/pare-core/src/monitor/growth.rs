//! Size history per rule, for alerts like "`~\.codex` grew 12 GB this week".
//!
//! Snapshots come only from rules-only runs that cover every folder a rule matches (the
//! monitor, or a whole-PC scan), so two snapshots are always comparable.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::env::Env;
use crate::rules::RuleSet;
use crate::types::{CategoryId, Finding, MS_PER_DAY, MS_PER_HOUR, Timestamp};

/// One rule's size at one moment.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub bytes: u64,
    /// "Codex · Session logs"
    pub label: String,
    #[serde(default)]
    pub path: Option<PathBuf>,
    pub category: CategoryId,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub t: Timestamp,
    /// Rule id → size. A rule that matched nothing is absent (0 bytes).
    pub rules: BTreeMap<String, Sample>,
}

/// Something that grew a lot.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Growth {
    pub rule_id: String,
    pub label: String,
    pub path: Option<PathBuf>,
    pub category: CategoryId,
    pub from: u64,
    pub to: u64,
    /// When the baseline was measured (about a week ago).
    pub since: Timestamp,
}

impl Growth {
    pub fn delta(&self) -> u64 {
        self.to.saturating_sub(self.from)
    }
}

/// Sizes per rule from a rules-only run. Admin rules are skipped (they only run elevated, so
/// they'd appear and vanish with the process's rights), and so are rules Pare can't size.
pub fn snapshot(findings: &[Finding], set: &RuleSet, now: Timestamp) -> Snapshot {
    let mut rules: BTreeMap<String, Sample> = BTreeMap::new();
    for f in findings {
        let Some((tool, rule)) = set.rule(&f.rule_id) else {
            continue;
        };
        if rule.admin || rule.unmeasured || f.bytes == 0 {
            continue;
        }
        let s = rules.entry(f.rule_id.clone()).or_insert_with(|| Sample {
            bytes: 0,
            label: format!(
                "{} · {}",
                crate::i18n::tr(&tool.tool.name),
                crate::i18n::tr(&rule.title)
            ),
            path: f.source_path.clone(),
            category: f.category,
        });
        s.bytes += f.bytes;
    }
    Snapshot { t: now, rules }
}

pub fn load(env: &Env) -> Vec<Snapshot> {
    let mut v: Vec<Snapshot> = std::fs::read(env.growth_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    v.sort_by_key(|s| s.t);
    v
}

/// Append a snapshot and thin the history: everything from the last two days, then one per
/// day, and nothing older than 60 days.
pub fn record(env: &Env, snap: Snapshot) -> std::io::Result<()> {
    let mut all = load(env);
    let now = snap.t;
    all.push(snap);
    let all = thin(all, now);
    let path = env.growth_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec(&all).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(tmp, path)
}

fn thin(mut all: Vec<Snapshot>, now: Timestamp) -> Vec<Snapshot> {
    all.sort_by_key(|s| s.t);
    let mut out: Vec<Snapshot> = Vec::new();
    for s in all {
        if now - s.t > 60 * MS_PER_DAY {
            continue;
        }
        if now - s.t > 2 * MS_PER_DAY {
            // Older than two days: keep the last snapshot of each day.
            if let Some(prev) = out.last_mut() {
                if prev.t.div_euclid(MS_PER_DAY) == s.t.div_euclid(MS_PER_DAY) {
                    *prev = s;
                    continue;
                }
            }
        }
        out.push(s);
    }
    out
}

/// What grew by at least `min_bytes` since about a week ago. The baseline is the oldest
/// snapshot from the last 7½ days that is at least 20 hours older than the latest one, so a
/// fresh install has no alerts until there's a day of history.
pub fn alerts(history: &[Snapshot], min_bytes: u64) -> Vec<Growth> {
    let Some(latest) = history.iter().max_by_key(|s| s.t) else {
        return Vec::new();
    };
    let Some(base) = history
        .iter()
        .filter(|s| s.t >= latest.t - (7 * MS_PER_DAY + 12 * MS_PER_HOUR))
        .filter(|s| s.t <= latest.t - 20 * MS_PER_HOUR)
        .min_by_key(|s| s.t)
    else {
        return Vec::new();
    };
    let mut out: Vec<Growth> = latest
        .rules
        .iter()
        .filter_map(|(id, now)| {
            let from = base.rules.get(id).map_or(0, |s| s.bytes);
            (now.bytes > from && now.bytes - from >= min_bytes.max(1)).then(|| Growth {
                rule_id: id.clone(),
                label: now.label.clone(),
                path: now.path.clone(),
                category: now.category,
                from,
                to: now.bytes,
                since: base.t,
            })
        })
        .collect();
    out.sort_by_key(|g| std::cmp::Reverse(g.delta()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1_000_000_000;

    fn snap(t: Timestamp, sizes: &[(&str, u64)]) -> Snapshot {
        Snapshot {
            t,
            rules: sizes
                .iter()
                .map(|(id, b)| {
                    (
                        id.to_string(),
                        Sample {
                            bytes: *b,
                            label: id.to_string(),
                            path: None,
                            category: CategoryId::AiTools,
                        },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn weekly_growth_over_threshold() {
        let d = MS_PER_DAY;
        let h = vec![
            snap(10 * d, &[("codex.sessions", 2 * GB), ("npm.cache", GB)]),
            snap(14 * d, &[("codex.sessions", 9 * GB), ("npm.cache", GB)]),
            snap(
                17 * d,
                &[("codex.sessions", 14 * GB), ("npm.cache", 2 * GB)],
            ),
        ];
        let a = alerts(&h, 5 * GB);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].rule_id, "codex.sessions");
        assert_eq!(a[0].delta(), 12 * GB);
        assert_eq!(a[0].since, 10 * d);
    }

    #[test]
    fn no_alert_without_a_day_of_history_or_when_shrinking() {
        let h = vec![
            snap(MS_PER_DAY, &[("a.b", GB)]),
            snap(MS_PER_DAY + 3 * MS_PER_HOUR, &[("a.b", 30 * GB)]),
        ];
        assert!(alerts(&h, GB).is_empty());
        let h = vec![
            snap(0, &[("a.b", 30 * GB)]),
            snap(3 * MS_PER_DAY, &[("a.b", GB)]),
        ];
        assert!(alerts(&h, GB).is_empty());
    }

    #[test]
    fn thinning_keeps_one_per_day_and_sixty_days() {
        let d = MS_PER_DAY;
        let now = 100 * d;
        let mut all = vec![snap(10 * d, &[])];
        for i in 0..6 {
            all.push(snap(50 * d + i * 3 * MS_PER_HOUR, &[]));
        }
        all.push(snap(now - MS_PER_HOUR, &[]));
        all.push(snap(now, &[]));
        let t = thin(all, now);
        assert_eq!(
            t.iter().map(|s| s.t).collect::<Vec<_>>(),
            vec![50 * d + 15 * MS_PER_HOUR, now - MS_PER_HOUR, now]
        );
    }

    #[test]
    fn record_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::fake(dir.path());
        record(&env, snap(MS_PER_DAY, &[("a.b", GB)])).unwrap();
        record(&env, snap(3 * MS_PER_DAY, &[("a.b", 8 * GB)])).unwrap();
        let h = load(&env);
        assert_eq!(h.len(), 2);
        assert_eq!(alerts(&h, 5 * GB)[0].delta(), 7 * GB);
    }
}
