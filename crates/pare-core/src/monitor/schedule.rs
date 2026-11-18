//! Opt-in scheduled cleanups (M8). The user turns a schedule on per rule; only built-in,
//! verified, Safe rules qualify, and each run goes through the same scan, safety pass and
//! executor as a click on "Clean up" (recency, in use, exclusions, the operation log).

use serde::Serialize;

use crate::dispose::executor::{CleanOptions, CleanReport, clean};
use crate::dispose::oplog::OpLog;
use crate::env::Env;
use crate::rules::{DisposalDecl, RuleDef, RuleSet, TierDecl};
use crate::scanner::{ScanConfig, scan_rules};
use crate::settings::{RuleSchedule, Settings};
use crate::types::{ComesBack, Disposal, Finding, MS_PER_DAY, Tier, Timestamp};

use super::MonitorState;

/// A rule the user may put on a schedule (Settings lists these).
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SchedulableRule {
    pub id: String,
    pub tool: String,
    pub title: String,
    pub why: String,
    pub if_removed: String,
    pub comes_back: ComesBack,
    pub disposal: String,
}

/// Why a rule can't be scheduled, or `Ok`.
pub fn check(set: &RuleSet, rule_id: &str) -> Result<(), &'static str> {
    let Some((tool, rule)) = set.rule(rule_id) else {
        return Err("unknown rule");
    };
    if !tool.builtin {
        return Err("only built-in rules can run on a schedule");
    }
    eligible(rule)
}

fn eligible(rule: &RuleDef) -> Result<(), &'static str> {
    if rule.tier != TierDecl::Safe {
        return Err("only Safe rules can run on a schedule");
    }
    if !rule.verified {
        return Err("the rule hasn't been verified");
    }
    if rule.admin {
        return Err("the rule needs administrator rights");
    }
    if !matches!(
        rule.disposal,
        DisposalDecl::Delete | DisposalDecl::Recycle | DisposalDecl::Command
    ) {
        return Err("guides can't run on a schedule");
    }
    if !rule.detector.is_empty() {
        return Err("rules with detectors need a review");
    }
    Ok(())
}

pub fn schedulable_rules(set: &RuleSet) -> Vec<SchedulableRule> {
    set.tools
        .iter()
        .filter(|t| t.builtin)
        .flat_map(|t| t.rules.iter().map(move |r| (t, r)))
        .filter(|(_, r)| eligible(r).is_ok())
        .map(|(t, r)| SchedulableRule {
            id: r.id.clone(),
            tool: crate::i18n::tr(&t.tool.name),
            title: crate::i18n::tr(&r.title),
            why: crate::i18n::tr(&r.why),
            if_removed: crate::i18n::tr(&r.if_removed),
            comes_back: r.comes_back,
            disposal: match r.disposal {
                DisposalDecl::Delete => "delete",
                DisposalDecl::Recycle => "recycle",
                DisposalDecl::Command => "command",
                DisposalDecl::Guide => "guide",
            }
            .into(),
        })
        .collect()
}

/// Schedules whose interval has passed (a new schedule is due at the next check).
pub fn due(schedules: &[RuleSchedule], state: &MonitorState, now: Timestamp) -> Vec<String> {
    let mut ids: Vec<String> = schedules
        .iter()
        .filter(|s| s.every_days > 0)
        .filter(|s| {
            state
                .last_runs
                .get(&s.rule)
                .is_none_or(|t| now - t >= s.every_days as i64 * MS_PER_DAY)
        })
        .map(|s| s.rule.clone())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// What a scheduled run would clean: Safe, selectable findings of eligible rules only.
pub fn plan(set: &RuleSet, ids: &[String], cfg: &ScanConfig, env: &Env) -> Vec<Finding> {
    let ids: Vec<&String> = ids.iter().filter(|id| check(set, id).is_ok()).collect();
    if ids.is_empty() {
        return Vec::new();
    }
    let mut cfg = cfg.clone();
    cfg.check_in_use = true;
    scan_rules(&cfg, env, |t, r| t.builtin && ids.contains(&&r.id))
        .into_iter()
        .filter(|f| matches!(f.tier, Tier::Safe) && f.is_selectable())
        .filter(|f| ids.contains(&&f.rule_id))
        .collect()
}

/// Run the due schedules. Returns `None` when there was nothing to clean.
pub fn run(
    set: &RuleSet,
    ids: &[String],
    cfg: &ScanConfig,
    settings: &Settings,
    env: &Env,
    log: &OpLog,
) -> Option<CleanReport> {
    let started = crate::platform::now_ms();
    // "Always keep" applies to scheduled runs exactly as it does to a scan.
    let mut cfg = cfg.clone();
    for e in &settings.exclusions {
        if !cfg.exclusions.contains(e) {
            cfg.exclusions.push(e.clone());
        }
    }
    cfg.thresholds.recent_hours = cfg.thresholds.recent_hours.max(24);
    let cfg = &cfg;
    let mut findings = plan(set, ids, cfg, env);
    if findings.is_empty() {
        return None;
    }
    if settings.caches_to_recycle_bin {
        for f in &mut findings {
            if f.disposal == Disposal::Delete {
                f.disposal = Disposal::Recycle;
            }
        }
    }
    let mut opts = CleanOptions::new(started);
    opts.recent_ms = cfg.thresholds.recent_hours as i64 * crate::types::MS_PER_HOUR;
    opts.exclusions = settings.exclusions.clone();
    opts.label = Some(crate::i18n::tr("Scheduled cleanup"));
    Some(clean(&findings, env, log, &opts, &|_| {}))
}
