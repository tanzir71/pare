//! `claude.orphaned_transcripts`: Claude Code keeps one folder per project under
//! `~\.claude\projects\<encoded path>`. When the project folder no longer exists, its
//! transcripts are orphaned.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use crate::platform;
use crate::rules::engine::{Base, Walker};
use crate::rules::matcher::Match;
use crate::scan::measure::Candidate;
use crate::types::Tier;

/// The working directory recorded in a project's transcripts, if any.
pub fn transcript_cwd(project_dir: &Path) -> Option<PathBuf> {
    let mut files: Vec<_> = platform::list_dir(project_dir)
        .ok()?
        .into_iter()
        .filter(|e| !e.is_dir && e.name.to_string_lossy().ends_with(".jsonl") && !e.is_cloud_placeholder())
        .collect();
    files.sort_by_key(|e| e.modified);
    for f in files.iter().take(3) {
        let Ok(file) = std::fs::File::open(project_dir.join(&f.name)) else { continue };
        // The first records are small; never read more than 256 KB.
        let reader = BufReader::new(file.take(256 * 1024));
        for line in reader.lines().take(50).map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(cwd) = v.get("cwd").and_then(|c| c.as_str()) {
                    if !cwd.is_empty() {
                        return Some(PathBuf::from(cwd));
                    }
                }
            }
        }
    }
    None
}

/// Claude Code encodes a path by replacing every character that isn't a letter or digit with
/// `-` (`D:\code\my-app` → `D--code-my-app`).
pub fn encode(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// Outcome of decoding a project folder name.
#[derive(Debug, PartialEq, Eq)]
pub enum Decoded {
    /// Some real folder encodes to this name.
    Exists(PathBuf),
    /// The drive exists but no folder matches: the project is gone.
    Missing,
    /// Can't tell (drive not present, or the name isn't a drive path).
    Unknown,
}

/// Lossy decode with a file-system search for any folder that encodes to `name`.
pub fn decode(name: &str) -> Decoded {
    let b = name.as_bytes();
    if b.len() < 3 || !b[0].is_ascii_alphabetic() || b[1] != b'-' || b[2] != b'-' {
        return Decoded::Unknown;
    }
    let root = PathBuf::from(format!("{}:\\", (b[0] as char).to_ascii_uppercase()));
    if platform::stat(&root).is_err() {
        return Decoded::Unknown;
    }
    let rest = &name[3..];
    if rest.is_empty() {
        return Decoded::Exists(root);
    }
    match search(&root, rest, 0) {
        Some(p) => Decoded::Exists(p),
        None => Decoded::Missing,
    }
}

fn search(dir: &Path, rest: &str, depth: usize) -> Option<PathBuf> {
    if depth > 40 {
        return None;
    }
    let children = platform::list_dir(dir).ok()?;
    for c in children.into_iter().filter(|c| c.is_dir) {
        let enc = encode(&c.name.to_string_lossy());
        if enc.is_empty() {
            continue;
        }
        if rest.eq_ignore_ascii_case(&enc) {
            return Some(dir.join(&c.name));
        }
        if rest.len() > enc.len() + 1
            && rest[..enc.len()].eq_ignore_ascii_case(&enc)
            && rest.as_bytes()[enc.len()] == b'-'
            && c.is_traversable_dir()
        {
            if let Some(p) = search(&dir.join(&c.name), &rest[enc.len() + 1..], depth + 1) {
                return Some(p);
            }
        }
    }
    None
}

pub fn orphaned_transcripts(w: &Walker, base: &Base, matches: &[Match]) -> Vec<Candidate> {
    let mut orphans: Vec<Match> = Vec::new();
    let mut maybe: Vec<Match> = Vec::new();
    for m in matches {
        let Match::Item(dir) = m else { continue };
        let Ok(meta) = platform::stat(dir) else { continue };
        if !meta.is_traversable_dir() {
            continue;
        }
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match transcript_cwd(dir) {
            Some(cwd) => {
                if platform::stat(&cwd).is_ok() {
                    continue;
                }
                // The drive itself may just be unplugged.
                let drive_ok = cwd.components().next().is_some_and(|c| {
                    let mut root = PathBuf::from(c.as_os_str());
                    root.push("\\");
                    platform::stat(&root).is_ok()
                });
                if drive_ok && encode(&cwd.to_string_lossy()).eq_ignore_ascii_case(&name) {
                    orphans.push(m.clone());
                } else if drive_ok {
                    maybe.push(m.clone());
                }
            }
            None => match decode(&name) {
                Decoded::Exists(_) | Decoded::Unknown => {}
                Decoded::Missing => maybe.push(m.clone()),
            },
        }
    }
    let mut out = Vec::new();
    if !orphans.is_empty() {
        let n = orphans.len();
        let c = w.collect(&orphans, false);
        for mut cand in base.with_recent_companion("Transcripts of deleted projects", c) {
            if cand.tier.is_selectable() {
                cand.badge = Some("Orphaned".into());
                cand.note = Some("Folders deleted".into());
                cand.why = format!(
                    "These Claude Code transcripts belong to {n} project folder{} that no longer exist on this PC.",
                    if n == 1 { "" } else { "s" }
                );
            }
            out.push(cand);
        }
    }
    if !maybe.is_empty() {
        let c = w.collect(&maybe, false);
        for mut cand in base.with_recent_companion("Transcripts that may belong to deleted projects", c) {
            if cand.tier.is_selectable() {
                cand.tier = Tier::Review;
                cand.why = "Pare couldn't find the project folders these transcripts were recorded in. They may have been moved or deleted.".into();
            }
            out.push(cand);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_matches_claude_code() {
        assert_eq!(encode(r"D:\claude-upgrade-26\file-manager"), "D--claude-upgrade-26-file-manager");
        assert_eq!(encode(r"C:\Users\ana\.config\x y"), "C--Users-ana--config-x-y");
    }

    #[test]
    fn reads_cwd_from_first_records() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("s.jsonl"),
            "{\"type\":\"summary\"}\n{\"type\":\"user\",\"cwd\":\"C:\\\\code\\\\gone\"}\n",
        )
        .unwrap();
        assert_eq!(transcript_cwd(d.path()), Some(PathBuf::from(r"C:\code\gone")));
    }

    #[cfg(windows)]
    #[test]
    fn decodes_against_the_file_system() {
        let d = tempfile::tempdir().unwrap();
        let real = d.path().join("my-app.v2").join("src");
        std::fs::create_dir_all(&real).unwrap();
        let enc = encode(&real.to_string_lossy());
        match decode(&enc) {
            Decoded::Exists(p) => assert!(platform::path_starts_with(&p, &real) && platform::path_starts_with(&real, &p)),
            other => panic!("{other:?}"),
        }
        let gone = encode(&d.path().join("deleted-project").to_string_lossy());
        assert_eq!(decode(&gone), Decoded::Missing);
        assert_eq!(decode("not-a-drive-path"), Decoded::Unknown);
    }
}
