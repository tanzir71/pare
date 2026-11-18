//! Runs rules against the file system and turns matches into [`Candidate`]s.
//!
//! Per rule: expand paths → match segment-wise → walk matched items file by file applying
//! `exclude`, `[[protect]]`, never-touch, `min_age_days`, recency and cloud filters → collapse
//! what's left into the fewest paths (a folder when everything inside qualifies) → measure.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::matcher::{Match, PathPattern, PathSet};
use super::{DisposalDecl, LoadedTool, RuleDef, RuleSet, TierDecl, category_for, vars};
use crate::env::Env;
use crate::format::{age_phrase, format_bytes};
use crate::platform::{self, RawEntry};
use crate::safety::protected_paths;
use crate::scan::measure::{Candidate, FileAcc, Measured, account};
use crate::types::{
    CategoryId, ComesBack, Dates, Disposal, MS_PER_DAY, ProtectReason, Tier, Timestamp,
};

/// Built-in detectors rules may reference (`detector = "…"`).
pub const DETECTORS: &[&str] = &["claude.orphaned_transcripts", "codex.superseded_releases"];

pub struct RuleCtx<'a> {
    pub env: &'a Env,
    pub now: Timestamp,
    pub recent_ms: i64,
    pub drives: &'a [PathBuf],
    pub elevated: bool,
    /// Only matches inside these roots count (None = everywhere).
    pub within: Option<&'a [PathBuf]>,
    pub check_in_use: bool,
}

/// Run every rule whose category is `category`.
pub fn run_category(set: &RuleSet, ctx: &RuleCtx, category: CategoryId) -> Vec<Candidate> {
    let protect = PathSet::new(
        set.protect
            .iter()
            .flat_map(|p| vars::expand(p, ctx.env, ctx.drives))
            .collect::<Vec<_>>(),
    );
    let mut out = Vec::new();
    for (tool, rule) in set.all_rules() {
        if category_for(&tool.tool, rule) != category {
            continue;
        }
        out.extend(run_rule(tool, rule, ctx, &protect));
    }
    out
}

/// Run one rule.
pub fn run_rule(
    tool: &LoadedTool,
    rule: &RuleDef,
    ctx: &RuleCtx,
    protect: &PathSet,
) -> Vec<Candidate> {
    if rule.admin && !ctx.elevated {
        return Vec::new();
    }
    let patterns: Vec<PathPattern> = rule
        .paths
        .iter()
        .flat_map(|p| vars::expand(p, ctx.env, ctx.drives))
        .filter_map(|p| PathPattern::compile(&p))
        .collect();
    let within = |m: &Match| {
        ctx.within.is_none_or(|roots| {
            roots
                .iter()
                .any(|r| platform::path_starts_with(m.path(), r))
        })
    };
    let mut matches: Vec<Match> = Vec::new();
    for p in &patterns {
        let mut found: Vec<Match> = p.find().into_iter().filter(|m| within(m)).collect();
        if rule.keep_newest > 0 && rule.detector.is_empty() {
            // keep_newest applies to versioned items (`…/versions/*`), never to `/**` contents.
            let (items, contents): (Vec<Match>, Vec<Match>) =
                found.into_iter().partition(|m| matches!(m, Match::Item(_)));
            found = keep_newest(items, rule.keep_newest as usize);
            found.extend(contents);
        }
        matches.extend(found);
    }
    if matches.is_empty() {
        return Vec::new();
    }
    let exclude = PathSet::new(
        rule.exclude
            .iter()
            .flat_map(|p| vars::expand(p, ctx.env, ctx.drives))
            .collect::<Vec<_>>(),
    );
    let w = Walker {
        ctx,
        rule,
        exclude: &exclude,
        protect,
    };
    let base = Base::new(tool, rule, ctx, &patterns);

    match rule.detector.as_str() {
        "claude.orphaned_transcripts" => {
            return crate::detectors::claude::orphaned_transcripts(&w, &base, &matches);
        }
        "codex.superseded_releases" => {
            return crate::detectors::codex::superseded_releases(&w, &base, &matches);
        }
        _ => {}
    }

    match rule.disposal {
        // Guides never touch anything, so never-touch lists don't hide them (Cursor's live
        // state.vscdb is both protected and the subject of a guide).
        DisposalDecl::Guide => matches
            .iter()
            .filter_map(|m| {
                let pm = crate::scan::measure::measure_path(m.path());
                if pm.files == 0 {
                    return None;
                }
                let mut c = Collected::default();
                c.units.push(m.path().to_path_buf());
                c.newest = pm.newest;
                c.dates.include(pm.created_min, pm.newest, 0);
                let mut cand = base.candidate(&rule.title, c);
                if let Some(x) = cand.measured.as_mut() {
                    x.bytes = pm.allocated;
                    x.logical = pm.logical;
                    x.items = pm.files;
                }
                cand.title = m
                    .path()
                    .file_name()
                    .map(|n| format!("{} · {}", rule.title, n.to_string_lossy()))
                    .unwrap_or_else(|| rule.title.clone());
                cand.note = Some("Guide".into());
                Some(cand)
            })
            .collect(),
        DisposalDecl::Command if rule.command.iter().any(|a| a.contains("{model}")) => {
            crate::detectors::ollama::models(&w, &base, &matches)
        }
        DisposalDecl::Command => {
            let c = w.collect(&matches, true);
            if c.files.is_empty() {
                return Vec::new();
            }
            let prune = rule.command.iter().any(|a| a == "prune");
            let mut cand = base.candidate_with(&rule.title, c, !prune);
            cand.extra_paths = matches.iter().map(|m| m.path().to_path_buf()).collect();
            if prune {
                // The tool decides what's unused: don't promise space we can't confirm.
                let size = cand.measured.as_ref().map_or(0, |m| m.bytes);
                if let Some(m) = cand.measured.as_mut() {
                    m.bytes = 0;
                    m.logical = 0;
                }
                cand.note = Some(format!(
                    "Frees only what no project uses (store: {})",
                    format_bytes(size)
                ));
                cand.facts.push(("Store size".into(), format_bytes(size)));
            }
            vec![cand]
        }
        DisposalDecl::Delete | DisposalDecl::Recycle => {
            let c = w.collect(&matches, false);
            base.with_recent_companion(&rule.title, c)
        }
    }
}

/// Drop the `n` most recently modified matched items.
fn keep_newest(mut items: Vec<Match>, n: usize) -> Vec<Match> {
    let mut with_t: Vec<(Timestamp, Match)> = items
        .drain(..)
        .map(|m| (crate::scan::measure::measure_path(m.path()).newest, m))
        .collect();
    with_t.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    with_t.into_iter().skip(n).map(|(_, m)| m).collect()
}

/// What a walk over matched items found.
#[derive(Default)]
pub struct Collected {
    /// Fewest paths that cover every qualifying file (folders when fully covered).
    pub units: Vec<PathBuf>,
    pub files: Vec<FileAcc>,
    pub dates: Dates,
    pub newest: Timestamp,
    /// Files skipped because they changed within the recency window.
    pub recent: Vec<FileAcc>,
    pub cloud: u64,
}

pub struct Walker<'a> {
    pub ctx: &'a RuleCtx<'a>,
    pub rule: &'a RuleDef,
    pub exclude: &'a PathSet,
    pub protect: &'a PathSet,
}

impl Walker<'_> {
    fn blocked(&self, path: &Path) -> bool {
        protected_paths::classify(path, self.ctx.env, self.rule.admin).is_some()
            || self.protect.is_match(path)
            || self.exclude.is_match(path)
    }

    /// Walk matches. `everything` ignores age filters (sizes for guides and commands).
    pub fn collect(&self, matches: &[Match], everything: bool) -> Collected {
        let mut c = Collected::default();
        for m in matches {
            match m {
                Match::Item(p) => {
                    let Ok(e) = platform::stat(p) else { continue };
                    if self.visit(p, &e, everything, &mut c) {
                        c.units.push(p.clone());
                        c.newest = c.newest.max(e.modified);
                    }
                }
                Match::Contents(d) => {
                    let Ok(children) = platform::list_dir(d) else {
                        continue;
                    };
                    for e in children {
                        let p = d.join(&e.name);
                        if self.visit(&p, &e, everything, &mut c) {
                            c.units.push(p);
                            c.newest = c.newest.max(e.modified);
                        }
                    }
                }
            }
        }
        c.units.sort();
        c.units.dedup();
        c
    }

    /// Returns true when `path` and everything under it qualifies (so the caller can use the
    /// path itself as one unit); otherwise pushes the qualifying sub-paths as units.
    fn visit(&self, path: &Path, e: &RawEntry, everything: bool, c: &mut Collected) -> bool {
        if self.blocked(path) {
            return false;
        }
        let min_age = if everything {
            0
        } else {
            self.rule.min_age_days.unwrap_or(0) as i64 * MS_PER_DAY
        };
        let too_new = |t: Timestamp| t > self.ctx.now - min_age;
        let recent = |t: Timestamp| !everything && t > self.ctx.now - self.ctx.recent_ms;

        if !e.is_dir || !e.is_traversable_dir() {
            if e.is_dir {
                // A junction or symlink: removing it removes only the link.
                return !recent(e.modified);
            }
            if e.is_cloud_placeholder() {
                c.cloud += 1;
                return false;
            }
            let acc = FileAcc {
                path: path.to_path_buf(),
                allocated: e.allocated,
                logical: e.logical,
                file_id: e.file_id,
                links: e.links,
                modified: e.modified,
            };
            if min_age > 0 && too_new(e.modified) {
                return false;
            }
            if recent(e.modified) {
                c.recent.push(acc);
                return false;
            }
            c.dates.include(e.created, e.modified, e.accessed);
            c.newest = c.newest.max(e.modified);
            c.files.push(acc);
            return true;
        }

        let Ok(children) = platform::list_dir(path) else {
            return false;
        };
        let mut all = true;
        let mut sub_units = Vec::new();
        for ch in children {
            let p = path.join(&ch.name);
            if self.visit(&p, &ch, everything, c) {
                sub_units.push((p, ch.modified));
            } else {
                all = false;
            }
        }
        // A folder can only stand in for its contents when it isn't recently changed itself.
        if all && !recent(e.modified) {
            return true;
        }
        for (p, t) in sub_units {
            c.newest = c.newest.max(t);
            c.units.push(p);
        }
        false
    }
}

/// Fields shared by every candidate a rule produces.
pub struct Base<'a> {
    /// Split units other apps hold open out of findings (the Restart Manager check).
    pub check_in_use: bool,
    pub tool: &'a LoadedTool,
    pub rule: &'a RuleDef,
    pub category: CategoryId,
    pub source_path: Option<PathBuf>,
    pub now: Timestamp,
}

impl<'a> Base<'a> {
    fn new(
        tool: &'a LoadedTool,
        rule: &'a RuleDef,
        ctx: &RuleCtx,
        patterns: &[PathPattern],
    ) -> Base<'a> {
        Base {
            check_in_use: ctx.check_in_use,
            tool,
            rule,
            category: category_for(&tool.tool, rule),
            source_path: patterns.first().map(|p| p.base().to_path_buf()),
            now: ctx.now,
        }
    }

    pub fn disposal(&self) -> Disposal {
        match self.rule.disposal {
            DisposalDecl::Delete => Disposal::Delete,
            DisposalDecl::Recycle => Disposal::Recycle,
            DisposalDecl::Command => {
                let mut it = self.rule.command.iter().cloned();
                Disposal::Command {
                    program: it.next().unwrap_or_default(),
                    args: it.collect(),
                }
            }
            DisposalDecl::Guide => Disposal::Guide {
                steps: self.rule.guide.clone(),
            },
        }
    }

    /// A candidate from collected files.
    pub fn candidate(&self, title: &str, c: Collected) -> Candidate {
        self.candidate_with(title, c, true)
    }

    /// `count_links = false` sums sizes without checking hard links (for sizes shown only as
    /// information, e.g. a store a tool prunes itself).
    pub fn candidate_with(&self, title: &str, c: Collected, count_links: bool) -> Candidate {
        let mut files = c.files;
        let (bytes, logical, items) = if count_links {
            account(&mut files)
        } else {
            (
                files.iter().map(|f| f.allocated).sum(),
                files.iter().map(|f| f.logical).sum(),
                files.len() as u64,
            )
        };
        files.sort_by_key(|f| std::cmp::Reverse(f.modified));
        let mut cand = Candidate::new(&self.rule.id, self.category, title);
        cand.source = self.tool.tool.name.clone();
        cand.source_path = self.source_path.clone();
        cand.created_by = Some(self.tool.tool.name.clone());
        cand.extra_paths = c.units;
        // A rule nobody has checked on a real PC yet never claims Safe.
        cand.tier = match (self.rule.tier, self.rule.verified) {
            (TierDecl::Safe, true) => Tier::Safe,
            _ => Tier::Review,
        };
        if !self.rule.verified {
            cand.facts.push((
                "Rule status".into(),
                "Not yet checked on a real PC, so it's shown as Review".into(),
            ));
        }
        cand.disposal = self.disposal();
        cand.comes_back = self.rule.comes_back;
        cand.why = self.rule.why.clone();
        cand.if_removed = self.rule.if_removed.clone();
        cand.admin = self.rule.admin;
        if self.rule.requires_closed {
            cand.requires_closed = self.tool.tool.processes.clone();
        }
        if c.newest > 0 {
            cand.note = Some(format!("Changed {}", age_phrase(self.now, c.newest)));
        }
        if let Some(docs) = &self.tool.tool.docs {
            cand.facts.push(("Docs".into(), docs.clone()));
        }
        cand.facts.push(("Rule".into(), self.rule.id.clone()));
        cand.measured = Some(Measured {
            bytes,
            logical,
            items,
            dates: c.dates,
            newest: c.newest,
            recent_files: files.iter().take(1000).map(|f| f.path.clone()).collect(),
        });
        cand
    }

    /// The rule's candidate plus Protected companions for what was left out: files changed in
    /// the last 24 hours, and units another app holds open (so one open temp file doesn't block
    /// the rest of %TEMP%).
    pub fn with_recent_companion(&self, title: &str, mut c: Collected) -> Vec<Candidate> {
        let recent = std::mem::take(&mut c.recent);
        let mut out = Vec::new();
        if self.check_in_use && c.units.len() > 1 {
            let (held, names) = split_held(&mut c);
            if !held.files.is_empty() || !held.units.is_empty() {
                let mut k = self.candidate(&format!("{title}: open in another app"), held);
                k.tier = Tier::Protected {
                    reason: ProtectReason::InUse,
                    detail: Some(format!("Open in {names}")),
                };
                k.note = Some("In use".into());
                k.why = "Another app has these files open, so Pare leaves them alone.".into();
                k.if_removed = "Nothing: Pare doesn't remove files that are in use.".into();
                out.push(k);
            }
        }
        if !c.files.is_empty() || !c.units.is_empty() {
            out.push(self.candidate(title, c));
        }
        if !recent.is_empty() {
            let bytes: u64 = recent.iter().map(|f| f.allocated).sum();
            if bytes >= 1_000_000 {
                let mut k = Candidate::new(
                    &self.rule.id,
                    self.category,
                    format!("{title}: used in the last 24 hours"),
                );
                k.source = self.tool.tool.name.clone();
                k.source_path = self.source_path.clone();
                k.created_by = Some(self.tool.tool.name.clone());
                k.tier = Tier::protected(ProtectReason::RecentlyModified);
                k.disposal = self.disposal();
                k.comes_back = self.rule.comes_back;
                k.why =
                    "These files changed in the last 24 hours, so Pare leaves them alone.".into();
                k.if_removed = "Nothing: Pare doesn't remove recently used files.".into();
                k.note = Some("Active now".into());
                let newest = recent.iter().map(|f| f.modified).max().unwrap_or(0);
                k.extra_paths = self.source_path.iter().cloned().collect();
                k.measured = Some(Measured {
                    bytes,
                    logical: recent.iter().map(|f| f.logical).sum(),
                    items: recent.len() as u64,
                    dates: Dates {
                        created_min: None,
                        modified_max: Some(newest),
                        accessed_max: None,
                    },
                    newest,
                    recent_files: Vec::new(),
                });
                out.push(k);
            }
        }
        out
    }
}

/// Move units whose files another process holds open out of `c`. Returns them and the
/// holders' names.
fn split_held(c: &mut Collected) -> (Collected, String) {
    // Up to 20 newest files per unit go to the Restart Manager, batched.
    let mut per_unit: Vec<Vec<PathBuf>> = vec![Vec::new(); c.units.len()];
    let mut order: Vec<&FileAcc> = c.files.iter().collect();
    order.sort_by_key(|f| std::cmp::Reverse(f.modified));
    for f in order {
        if let Some(i) = c
            .units
            .iter()
            .position(|u| platform::path_starts_with(&f.path, u))
        {
            if per_unit[i].len() < 20 {
                per_unit[i].push(f.path.clone());
            }
        }
    }
    let holders = crate::safety::holders_batch(&per_unit);
    let mut held = Collected::default();
    let mut names: Vec<String> = Vec::new();
    let mut keep_units = Vec::new();
    for (u, h) in std::mem::take(&mut c.units).into_iter().zip(holders) {
        if h.is_empty() {
            keep_units.push(u);
        } else {
            names.extend(h.into_iter().map(|x| x.name));
            held.units.push(u);
        }
    }
    c.units = keep_units;
    let (held_files, files): (Vec<FileAcc>, Vec<FileAcc>) =
        std::mem::take(&mut c.files).into_iter().partition(|f| {
            held.units
                .iter()
                .any(|u| platform::path_starts_with(&f.path, u))
        });
    c.files = files;
    held.newest = held_files.iter().map(|f| f.modified).max().unwrap_or(0);
    held.files = held_files;
    names.sort();
    names.dedup();
    (held, names.join(", "))
}

/// Is any process from `names` running (case-insensitive)?
pub fn any_running(names: &[String], running: &HashSet<String>) -> Option<String> {
    names
        .iter()
        .find(|n| running.contains(&n.to_lowercase()))
        .cloned()
}

/// Group sizes per rule id, for `pare rules check` and tests.
pub fn summarize_rule(cands: &[Candidate]) -> HashMap<String, (u64, u64)> {
    let mut m: HashMap<String, (u64, u64)> = HashMap::new();
    for c in cands {
        let e = m.entry(c.rule_id.clone()).or_default();
        if let Some(x) = &c.measured {
            e.0 += x.bytes;
            e.1 += x.items;
        }
    }
    m
}

/// `ComesBack` label used in rule listings.
pub fn comes_back_label(c: ComesBack) -> &'static str {
    c.label()
}
