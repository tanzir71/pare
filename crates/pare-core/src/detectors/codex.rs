//! `codex.superseded_releases`: the Codex standalone installer keeps every release it ever
//! downloaded. Keep the one the launcher points to and the newest; the rest are Safe.

use std::path::{Path, PathBuf};

use crate::platform;
use crate::rules::engine::{Base, Walker};
use crate::rules::matcher::Match;
use crate::scan::measure::Candidate;

/// Release names a pointer next to `releases/` refers to: a `current` link, or a small text
/// file (`current`, `current.txt`, `version`, `version.txt`) naming the release.
fn pointed_to(releases_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Some(parent) = releases_dir.parent() else { return out };
    for name in ["current", "current.txt", "version", "version.txt", "latest"] {
        for dir in [releases_dir, parent] {
            let p = dir.join(name);
            let Ok(meta) = platform::stat(&p) else { continue };
            if meta.is_link() {
                if let Ok(t) = std::fs::read_link(&p) {
                    if let Some(n) = t.file_name() {
                        out.push(n.to_string_lossy().into_owned());
                    }
                }
            } else if !meta.is_dir && meta.logical < 4096 && !meta.is_cloud_placeholder() {
                if let Ok(s) = std::fs::read_to_string(&p) {
                    let s = s.trim();
                    if !s.is_empty() {
                        out.push(Path::new(s).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| s.to_string()));
                    }
                }
            }
        }
    }
    out
}

/// Compare release names like `0.12.3`, `rust-v0.40.0`, `codex-1.2.10` by their numbers.
fn version_key(name: &str) -> Vec<u64> {
    name.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect()
}

pub fn superseded_releases(w: &Walker, base: &Base, matches: &[Match]) -> Vec<Candidate> {
    let mut releases: Vec<(PathBuf, platform::RawEntry)> = matches
        .iter()
        .filter_map(|m| match m {
            Match::Item(p) => platform::stat(p).ok().filter(|e| e.is_dir).map(|e| (p.clone(), e)),
            Match::Contents(_) => None,
        })
        .collect();
    if releases.len() < 2 {
        return Vec::new();
    }
    let releases_dir = releases[0].0.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let pointers = pointed_to(&releases_dir);
    releases.sort_by(|a, b| {
        let ka = version_key(&a.0.file_name().unwrap_or_default().to_string_lossy());
        let kb = version_key(&b.0.file_name().unwrap_or_default().to_string_lossy());
        kb.cmp(&ka).then(b.1.modified.cmp(&a.1.modified))
    });
    let newest_by_mtime = releases.iter().max_by_key(|r| r.1.modified).map(|r| r.0.clone());
    let keep = |p: &Path, i: usize| {
        let name = p.file_name().unwrap_or_default().to_string_lossy();
        i == 0 || Some(p) == newest_by_mtime.as_deref() || pointers.iter().any(|x| x.eq_ignore_ascii_case(&name))
    };
    let old: Vec<Match> = releases
        .iter()
        .enumerate()
        .filter(|(i, (p, _))| !keep(p, *i))
        .map(|(_, (p, _))| Match::Item(p.clone()))
        .collect();
    if old.is_empty() {
        return Vec::new();
    }
    let n = old.len();
    let c = w.collect(&old, false);
    base.with_recent_companion(&format!("Old CLI releases ({n})"), c)
        .into_iter()
        .map(|mut cand| {
            if cand.tier.is_selectable() {
                cand.badge = Some("Old versions".into());
                cand.note = Some("In-use kept".into());
            }
            cand
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_sort_numerically() {
        let mut v = vec!["0.9.0", "0.10.1", "0.10.0", "rust-v0.2.0"];
        v.sort_by_key(|s| std::cmp::Reverse(version_key(s)));
        assert_eq!(v, vec!["0.10.1", "0.10.0", "0.9.0", "rust-v0.2.0"]);
    }
}
