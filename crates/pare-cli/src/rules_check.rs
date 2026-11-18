//! `pare rules check`: for each rule, expand its paths on this machine and list/measure what
//! matches. Read-only: nothing is opened for writing, nothing is removed.

use std::path::PathBuf;

use pare_core::Env;
use pare_core::format::format_bytes;
use pare_core::rules::matcher::{Match, PathPattern};
use pare_core::rules::{RuleSet, vars};
use pare_core::scan::measure::measure_path;
use serde::Serialize;

#[derive(Serialize)]
pub struct PathReport {
    pub pattern: String,
    pub expanded: String,
    pub base_exists: bool,
    pub matches: usize,
    pub bytes: u64,
    pub files: u64,
    pub examples: Vec<PathBuf>,
}

#[derive(Serialize)]
pub struct RuleReport {
    pub id: String,
    pub tool: String,
    pub verified: bool,
    pub matched: bool,
    pub paths: Vec<PathReport>,
}

pub fn check(set: &RuleSet, env: &Env, only: Option<&str>) -> Vec<RuleReport> {
    let drives: Vec<PathBuf> = pare_core::platform::list_drives().into_iter().filter(|d| d.is_fixed).map(|d| d.root).collect();
    let mut out = Vec::new();
    for (tool, rule) in set.all_rules() {
        if only.is_some_and(|o| !rule.id.starts_with(o)) {
            continue;
        }
        let mut paths = Vec::new();
        for pat in &rule.paths {
            for exp in vars::expand(pat, env, &drives) {
                let Some(p) = PathPattern::compile(&exp) else { continue };
                let base_exists = pare_core::platform::stat(p.base()).is_ok();
                let found: Vec<Match> = p.find();
                let (mut bytes, mut files) = (0, 0);
                for m in &found {
                    let pm = measure_path(m.path());
                    bytes += pm.allocated;
                    files += pm.files;
                }
                paths.push(PathReport {
                    pattern: pat.clone(),
                    expanded: exp,
                    base_exists,
                    matches: found.len(),
                    bytes,
                    files,
                    examples: found.iter().take(3).map(|m| m.path().to_path_buf()).collect(),
                });
            }
        }
        out.push(RuleReport {
            id: rule.id.clone(),
            tool: tool.tool.name.clone(),
            verified: rule.verified,
            matched: paths.iter().any(|p| p.matches > 0),
            paths,
        });
    }
    out
}

pub fn print(reports: &[RuleReport]) {
    for r in reports {
        let mark = if r.matched { "MATCH" } else { "-----" };
        println!("{mark} {:<40} {}{}", r.id, r.tool, if r.verified { " (verified)" } else { "" });
        for p in &r.paths {
            println!(
                "      {:<70} {:>5} matches {:>10} {:>8} files{}",
                p.expanded,
                p.matches,
                format_bytes(p.bytes),
                p.files,
                if p.base_exists { "" } else { "  (base missing)" }
            );
        }
    }
}
