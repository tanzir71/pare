//! Segment-wise glob matching against the file system. A pattern like
//! `C:/Users/ana/AppData/Local/Google/Chrome/User Data/*/Cache/**` is walked one segment at a
//! time, so only directories the pattern can reach are listed (rules × candidate dirs, not
//! rules × files). Reparse points are never traversed.

use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobMatcher, GlobSet, GlobSetBuilder};

use crate::platform::{self, RawEntry};

#[derive(Clone, Debug)]
enum Seg {
    Lit(String),
    Glob(GlobMatcher),
    AnyDepth,
}

/// A compiled, already-expanded path pattern (`/` separators).
#[derive(Clone, Debug)]
pub struct PathPattern {
    pub raw: String,
    base: PathBuf,
    segs: Vec<Seg>,
}

/// What a pattern matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Match {
    /// The path itself (file or directory).
    Item(PathBuf),
    /// Everything inside this directory (pattern ended in `/**`); the directory itself stays.
    Contents(PathBuf),
}

impl Match {
    pub fn path(&self) -> &Path {
        match self {
            Match::Item(p) | Match::Contents(p) => p,
        }
    }
}

fn is_glob(seg: &str) -> bool {
    seg.contains(['*', '?', '[', '{'])
}

fn seg_matcher(seg: &str) -> Option<GlobMatcher> {
    GlobBuilder::new(seg)
        .case_insensitive(true)
        .literal_separator(true)
        .build()
        .ok()
        .map(|g| g.compile_matcher())
}

impl PathPattern {
    pub fn compile(expanded: &str) -> Option<PathPattern> {
        let norm = expanded.replace('\\', "/");
        let mut parts = norm.split('/').filter(|s| !s.is_empty() && *s != ".").peekable();
        // Base: leading literal segments. A drive ("C:") or UNC root keeps its separator.
        let mut base = String::new();
        if norm.starts_with('/') && !norm.starts_with("//") {
            base.push('/');
        } else if norm.starts_with("//") {
            base.push_str("//");
        }
        let mut segs = Vec::new();
        let mut in_base = true;
        for p in parts.by_ref() {
            if in_base && !is_glob(p) && p != ".." {
                if !base.is_empty() && !base.ends_with('/') {
                    base.push('/');
                }
                base.push_str(p);
                if p.ends_with(':') {
                    base.push('/');
                }
                continue;
            }
            in_base = false;
            segs.push(if p == "**" {
                Seg::AnyDepth
            } else if is_glob(p) {
                Seg::Glob(seg_matcher(p)?)
            } else {
                Seg::Lit(p.to_string())
            });
        }
        if base.is_empty() {
            return None;
        }
        Some(PathPattern {
            raw: expanded.to_string(),
            base: PathBuf::from(base),
            segs,
        })
    }

    /// The literal directory this pattern starts from.
    pub fn base(&self) -> &Path {
        &self.base
    }

    /// Walk the file system and return what this pattern matches.
    pub fn find(&self) -> Vec<Match> {
        let mut out = Vec::new();
        let Ok(meta) = platform::stat(&self.base) else {
            return out;
        };
        walk(&self.base, &meta, &self.segs, &mut out, 0);
        out.sort_by(|a, b| a.path().cmp(b.path()));
        out.dedup();
        out
    }
}

const MAX_DEPTH: usize = 64;

fn walk(dir: &Path, meta: &RawEntry, segs: &[Seg], out: &mut Vec<Match>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    let Some(first) = segs.first() else {
        out.push(Match::Item(dir.to_path_buf()));
        return;
    };
    if !meta.is_traversable_dir() {
        return;
    }
    let rest = &segs[1..];
    match first {
        Seg::AnyDepth if rest.is_empty() => out.push(Match::Contents(dir.to_path_buf())),
        Seg::AnyDepth => {
            // Zero directories…
            walk(dir, meta, rest, out, depth + 1);
            // …or one or more.
            if let Ok(children) = platform::list_dir(dir) {
                for c in children.into_iter().filter(|c| c.is_traversable_dir()) {
                    walk(&dir.join(&c.name), &c, segs, out, depth + 1);
                }
            }
        }
        Seg::Lit(name) => {
            let child = dir.join(name);
            if let Ok(m) = platform::stat(&child) {
                walk(&child, &m, rest, out, depth + 1);
            }
        }
        Seg::Glob(m) => {
            if let Ok(children) = platform::list_dir(dir) {
                for c in children {
                    if m.is_match(Path::new(&c.name)) {
                        walk(&dir.join(&c.name), &c, rest, out, depth + 1);
                    }
                }
            }
        }
    }
}

/// A set of full-path globs (`exclude`, `[[protect]]`), matched case-insensitively against
/// `/`-separated absolute paths. Patterns may be relative-anywhere (`**/rootfs.vhdx.zst`).
#[derive(Clone, Debug)]
pub struct PathSet {
    set: GlobSet,
    empty: bool,
}

impl PathSet {
    pub fn new<I: IntoIterator<Item = String>>(patterns: I) -> PathSet {
        let mut b = GlobSetBuilder::new();
        let mut empty = true;
        for p in patterns {
            let p = p.replace('\\', "/");
            // A pattern for a directory also covers everything inside it.
            for variant in [p.clone(), format!("{}/**", p.trim_end_matches('/'))] {
                if let Ok(g) = GlobBuilder::new(&variant).case_insensitive(true).literal_separator(true).build() {
                    b.add(g);
                    empty = false;
                }
            }
        }
        PathSet {
            set: b.build().unwrap_or_else(|_| GlobSet::empty()),
            empty,
        }
    }

    pub fn is_match(&self, path: &Path) -> bool {
        if self.empty {
            return false;
        }
        let s = path.to_string_lossy().replace('\\', "/");
        self.set.is_match(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"x").unwrap();
    }

    fn rel(root: &Path, m: &Match) -> String {
        let s = m.path().strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
        match m {
            Match::Item(_) => s,
            Match::Contents(_) => format!("{s}/**"),
        }
    }

    #[test]
    fn segment_matching() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        touch(&r.join("User Data/Default/Cache/a"));
        touch(&r.join("User Data/Profile 1/Cache/b"));
        touch(&r.join("User Data/Profile 1/Cookies"));
        touch(&r.join("sessions/2026/01/rollout-1.jsonl"));
        touch(&r.join("sessions/2026/02/x/rollout-2.jsonl"));
        touch(&r.join("sessions/other.txt"));
        let root = r.to_string_lossy().replace('\\', "/");

        let p = PathPattern::compile(&format!("{root}/User Data/*/Cache/**")).unwrap();
        let got: Vec<String> = p.find().iter().map(|m| rel(r, m)).collect();
        assert_eq!(got, vec!["User Data/Default/Cache/**", "User Data/Profile 1/Cache/**"]);

        let p = PathPattern::compile(&format!("{root}/sessions/**/rollout-*.jsonl")).unwrap();
        let got: Vec<String> = p.find().iter().map(|m| rel(r, m)).collect();
        assert_eq!(got, vec!["sessions/2026/01/rollout-1.jsonl", "sessions/2026/02/x/rollout-2.jsonl"]);

        let p = PathPattern::compile(&format!("{root}/user data/*")).unwrap();
        assert_eq!(p.find().len(), if cfg!(windows) { 2 } else { 0 }, "case-insensitive on Windows");

        let p = PathPattern::compile(&format!("{root}/missing/**")).unwrap();
        assert!(p.find().is_empty());
    }

    #[test]
    fn path_sets() {
        let s = PathSet::new(["**/User Data/*/Cookies*".to_string(), "C:/Users/ana/.ssh".to_string()]);
        assert!(s.is_match(Path::new("C:/x/User Data/Default/Cookies")));
        assert!(s.is_match(Path::new("C:/x/User Data/Default/Cookies-journal")));
        assert!(s.is_match(Path::new("C:/Users/ana/.ssh/id_ed25519")));
        assert!(s.is_match(Path::new("c:/users/ANA/.ssh")));
        assert!(!s.is_match(Path::new("C:/x/User Data/Default/Cache/a")));
    }

    #[cfg(windows)]
    #[test]
    fn never_follows_junctions() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        touch(&r.join("target/secret/file.txt"));
        fs::create_dir_all(r.join("cache")).unwrap();
        junction::create(r.join("target"), r.join("cache/link")).unwrap();
        let root = r.to_string_lossy().replace('\\', "/");
        let p = PathPattern::compile(&format!("{root}/cache/**/file.txt")).unwrap();
        assert!(p.find().is_empty(), "matched through a junction");
        let p = PathPattern::compile(&format!("{root}/cache/*")).unwrap();
        assert_eq!(p.find().len(), 1, "the link itself can match");
    }
}
