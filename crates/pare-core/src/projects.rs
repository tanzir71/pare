//! Git-aware project cleanup (M4): find projects under the user's roots, read their git state,
//! and suggest build artifacts (`node_modules`, `target`, `.venv`, …) only when git ignores them
//! and the project is committed, pushed and idle.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobMatcher};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::env::Env;
use crate::format::{age_phrase, duration_phrase};
use crate::platform::{self, eq_component};
use crate::rules::EcosystemDef;
use crate::scan::measure::{Candidate, Measured, measure_path};
use crate::types::{
    CategoryId, ComesBack, Dates, Disposal, GitInfo, MS_PER_DAY, ProtectReason, Tier, Timestamp,
};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectStatus {
    /// Committed, pushed and idle: artifacts are Safe.
    CleanPushed,
    /// Committed and pushed but changed recently.
    Active,
    Unpushed,
    NoUpstream,
    NoRemote,
    Detached,
    Uncommitted,
    NotGit,
}

impl ProjectStatus {
    pub fn label(self) -> &'static str {
        match self {
            ProjectStatus::CleanPushed => "Clean & pushed",
            ProjectStatus::Active => "Active",
            ProjectStatus::Unpushed => "Unpushed commits",
            ProjectStatus::NoUpstream => "Not pushed",
            ProjectStatus::NoRemote => "No remote",
            ProjectStatus::Detached => "Detached HEAD",
            ProjectStatus::Uncommitted => "Uncommitted",
            ProjectStatus::NotGit => "Not in git",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub ecosystem: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
    pub git: Option<GitInfo>,
    pub status: ProjectStatus,
    /// Newest commit or tracked-file change.
    pub last_change: Option<Timestamp>,
    pub idle_days: Option<u64>,
    pub ecosystems: Vec<String>,
    pub rebuild: Vec<String>,
    pub artifacts: Vec<Artifact>,
    pub bytes: u64,
    pub tier: Tier,
    /// The finding that removes this project's artifacts (if any).
    pub finding_id: Option<String>,
    /// Set for repos found inside another repo (submodules, nested clones).
    pub parent: Option<PathBuf>,
}

/// Default places to look: ~\source, ~\code, ~\Projects, ~\Documents\GitHub, D:\code.
pub fn default_roots(env: &Env) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = ["source", "source/repos", "code", "Projects", "repos", "dev"]
        .iter()
        .map(|p| env.home.join(p))
        .collect();
    v.push(env.documents.join("GitHub"));
    if !env.is_fake {
        v.push(PathBuf::from(r"D:\code"));
    }
    v.into_iter()
        .filter(|p| platform::stat(p).is_ok_and(|e| e.is_traversable_dir()))
        .collect()
}

/// Directories never searched for projects.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".venv",
    "venv",
    "AppData",
    "$Recycle.Bin",
    ".cache",
    "bower_components",
];

fn compiled(ecos: &[EcosystemDef]) -> Vec<(EcosystemDef, Vec<GlobMatcher>, Vec<GlobMatcher>)> {
    let glob = |p: &str| {
        GlobBuilder::new(p)
            .case_insensitive(true)
            .literal_separator(true)
            .build()
            .ok()
            .map(|g| g.compile_matcher())
    };
    ecos.iter()
        .map(|e| {
            (
                e.clone(),
                e.markers.iter().filter_map(|m| glob(m)).collect(),
                e.artifacts.iter().filter_map(|a| glob(a)).collect(),
            )
        })
        .collect()
}

fn has_marker(
    dir: &Path,
    names: &[std::ffi::OsString],
    markers: &[GlobMatcher],
    raw: &[String],
) -> bool {
    // Markers may contain a path (ProjectSettings/ProjectVersion.txt).
    raw.iter()
        .any(|m| m.contains('/') && platform::stat(&dir.join(m)).is_ok())
        || names
            .iter()
            .any(|n| markers.iter().any(|g| g.is_match(Path::new(n))))
}

/// Find project roots under `roots`: git repositories (stop at the repo root, but keep looking
/// inside it for nested repos and submodules) and, outside git, folders with a marker file.
pub fn discover(
    roots: &[PathBuf],
    ecos: &[EcosystemDef],
    env: &Env,
) -> Vec<(PathBuf, Option<PathBuf>)> {
    let comp = compiled(ecos);
    let mut out: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    // (dir, depth, enclosing repo)
    let mut stack: Vec<(PathBuf, usize, Option<PathBuf>)> =
        roots.iter().map(|r| (r.clone(), 0, None)).collect();
    while let Some((dir, depth, repo)) = stack.pop() {
        if depth > 8 || !seen.insert(dir.clone()) {
            continue;
        }
        if crate::safety::protected_paths::classify(&dir, env, false).is_some() {
            continue;
        }
        let Ok(children) = platform::list_dir(&dir) else {
            continue;
        };
        let names: Vec<std::ffi::OsString> = children.iter().map(|c| c.name.clone()).collect();
        let is_repo = names.iter().any(|n| eq_component(n, OsStr::new(".git")));
        let mut enclosing = repo.clone();
        if is_repo {
            out.push((dir.clone(), repo.clone()));
            enclosing = Some(dir.clone());
        } else if repo.is_none()
            && comp
                .iter()
                .any(|(e, m, _)| has_marker(&dir, &names, m, &e.markers))
        {
            out.push((dir.clone(), None));
            continue; // a non-git project: don't look inside
        }
        // Inside a repo only nested repos matter, so don't go deep.
        let max_depth = if enclosing.is_some() { depth + 4 } else { 8 };
        for c in children {
            if !c.is_traversable_dir()
                || c.attrs & (platform::ATTR_HIDDEN | platform::ATTR_SYSTEM) != 0
            {
                continue;
            }
            let n = c.name.to_string_lossy();
            if n.starts_with('.') || SKIP_DIRS.iter().any(|s| n.eq_ignore_ascii_case(s)) {
                continue;
            }
            if comp
                .iter()
                .any(|(_, _, arts)| arts.iter().any(|a| a.is_match(Path::new(&c.name))))
                && enclosing.is_some()
            {
                continue; // build output: no repos in there worth reporting
            }
            if depth < max_depth {
                stack.push((dir.join(&c.name), depth + 1, enclosing.clone()));
            }
        }
    }
    out.sort();
    out
}

struct GitState {
    info: GitInfo,
    last_change: Option<Timestamp>,
}

fn git_state(root: &Path) -> Option<(GitState, git2::Repository)> {
    let mut repo = git2::Repository::open(root).ok()?;
    let mut info = GitInfo {
        repo: root.to_path_buf(),
        ..GitInfo::default()
    };
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(false)
        .include_ignored(false)
        .exclude_submodules(true);
    if let Ok(st) = repo.statuses(Some(&mut opts)) {
        info.dirty_files = st
            .iter()
            .filter(|s| s.status() != git2::Status::CURRENT)
            .count() as u64;
        info.dirty = info.dirty_files > 0;
    }
    info.has_remote = repo.remotes().map(|r| !r.is_empty()).unwrap_or(false);
    let mut last_commit = None;
    if let Ok(head) = repo.head() {
        info.detached = repo.head_detached().unwrap_or(false);
        if !info.detached {
            info.branch = head.shorthand().ok().map(String::from);
        }
        if let Ok(commit) = head.peel_to_commit() {
            last_commit = Some(commit.time().seconds() * 1000);
            info.last_commit = last_commit;
            if let Ok(mut walk) = repo.revwalk() {
                if walk.push(commit.id()).is_ok() {
                    info.commits = walk.take(10_000).count() as u64;
                }
            }
            if let Some(name) = info.branch.clone() {
                if let Ok(branch) = repo.find_branch(&name, git2::BranchType::Local) {
                    if let Ok(up) = branch.upstream() {
                        info.has_upstream = true;
                        if let Some(up_oid) = up.get().target() {
                            if let Ok((ahead, _behind)) =
                                repo.graph_ahead_behind(commit.id(), up_oid)
                            {
                                info.ahead = ahead as u64;
                            }
                        }
                    }
                }
            }
        }
    }
    let mut stashes = 0u64;
    let _ = repo.stash_foreach(|_, _, _| {
        stashes += 1;
        true
    });
    info.stashes = stashes;
    // Newest tracked-file change, from the index's stat cache (no working-tree walk).
    let index_newest = repo
        .index()
        .ok()
        .and_then(|idx| idx.iter().map(|e| e.mtime.seconds() as i64 * 1000).max());
    let last_change = match (last_commit, index_newest) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    };
    Some((GitState { info, last_change }, repo))
}

/// Analyze one project and build its artifact finding.
pub fn analyze(
    root: &Path,
    parent: Option<&Path>,
    ecos: &[EcosystemDef],
    now: Timestamp,
    idle_days: u64,
) -> (Project, Option<Candidate>) {
    let comp = compiled(ecos);
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());
    let git = git_state(root);

    // Marker folders: the root and subfolders a few levels down (monorepos), skipping nested repos.
    let mut artifacts: Vec<Artifact> = Vec::new();
    let mut ecosystems: Vec<String> = Vec::new();
    let mut rebuild: Vec<String> = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    let mut seen_art: HashSet<PathBuf> = HashSet::new();
    while let Some((dir, depth)) = stack.pop() {
        let Ok(children) = platform::list_dir(&dir) else {
            continue;
        };
        let names: Vec<std::ffi::OsString> = children.iter().map(|c| c.name.clone()).collect();
        if depth > 0 && names.iter().any(|n| eq_component(n, OsStr::new(".git"))) {
            continue; // nested repo: analyzed on its own
        }
        let mut art_names: Vec<&GlobMatcher> = Vec::new();
        for (e, markers, arts) in &comp {
            if has_marker(&dir, &names, markers, &e.markers) {
                if !ecosystems.contains(&e.id) {
                    ecosystems.push(e.id.clone());
                    if !e.rebuild.is_empty() {
                        rebuild.push(e.rebuild.clone());
                    }
                }
                for c in &children {
                    if c.is_dir && arts.iter().any(|a| a.is_match(Path::new(&c.name))) {
                        let p = dir.join(&c.name);
                        if seen_art.insert(p.clone()) {
                            artifacts.push(Artifact {
                                name: c.name.to_string_lossy().into_owned(),
                                path: p,
                                bytes: 0,
                                ecosystem: e.id.clone(),
                            });
                        }
                    }
                }
                art_names.extend(arts.iter());
            }
        }
        if depth < 3 {
            for c in children.iter().filter(|c| c.is_traversable_dir()) {
                let n = c.name.to_string_lossy();
                if n.starts_with('.') || SKIP_DIRS.iter().any(|s| n.eq_ignore_ascii_case(s)) {
                    continue;
                }
                if comp
                    .iter()
                    .any(|(_, _, arts)| arts.iter().any(|a| a.is_match(Path::new(&c.name))))
                {
                    continue;
                }
                stack.push((dir.join(&c.name), depth + 1));
            }
        }
    }

    // Only artifacts git ignores (a tracked or unignored `build/` is somebody's source).
    if let Some((_, repo)) = &git {
        let workdir = repo
            .workdir()
            .map(|w| w.to_path_buf())
            .unwrap_or_else(|| root.to_path_buf());
        artifacts.retain(|a| {
            let rel = a.path.strip_prefix(&workdir).unwrap_or(&a.path);
            repo.is_path_ignored(rel).unwrap_or(false)
        });
    }
    let mut dates = Dates::default();
    let mut newest = 0;
    let mut recent_files = Vec::new();
    let mut items = 0;
    let mut logical = 0;
    for a in &mut artifacts {
        let m = measure_path(&a.path);
        a.bytes = m.allocated;
        items += m.files;
        logical += m.logical;
        dates.include(m.created_min, m.newest, 0);
        newest = newest.max(m.newest);
        recent_files.extend(m.recent_files.into_iter().take(200));
    }
    let bytes: u64 = artifacts.iter().map(|a| a.bytes).sum();

    let (status, info, last_change) = match &git {
        None => (ProjectStatus::NotGit, None, None),
        Some((g, _)) => {
            let i = &g.info;
            let idle = g
                .last_change
                .is_some_and(|t| now - t >= idle_days as i64 * MS_PER_DAY);
            let s = if i.dirty {
                ProjectStatus::Uncommitted
            } else if i.detached {
                ProjectStatus::Detached
            } else if !i.has_remote {
                ProjectStatus::NoRemote
            } else if !i.has_upstream {
                ProjectStatus::NoUpstream
            } else if i.ahead > 0 {
                ProjectStatus::Unpushed
            } else if idle {
                ProjectStatus::CleanPushed
            } else {
                ProjectStatus::Active
            };
            (s, Some(i.clone()), g.last_change)
        }
    };
    let tier = match status {
        ProjectStatus::Uncommitted => Tier::Protected {
            reason: ProtectReason::Uncommitted,
            detail: info
                .as_ref()
                .map(|i| format!("{} files changed", i.dirty_files)),
        },
        ProjectStatus::CleanPushed => Tier::Safe,
        _ => Tier::Review,
    };
    let idle = last_change.map(|t| ((now - t).max(0) / MS_PER_DAY) as u64);

    let candidate = if artifacts.is_empty() {
        None
    } else {
        let parent_dir = root.parent().map(|p| p.to_path_buf());
        let mut c = Candidate::new("projects.artifacts", CategoryId::ProjectBuild, name.clone());
        c.source = parent_dir
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        c.source_path = parent_dir;
        c.extra_paths = artifacts.iter().map(|a| a.path.clone()).collect();
        c.tier = tier.clone();
        let is_git = info.is_some();
        // Ignored build output rebuilds; outside git we can't be sure, so it's recycled.
        c.disposal = if is_git {
            Disposal::Delete
        } else {
            Disposal::Recycle
        };
        c.comes_back = ComesBack::OnDemand;
        let list = artifacts
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        c.why = match status {
            ProjectStatus::CleanPushed => format!(
                "Build files and dependencies ({list}) in a project that's committed, pushed and unchanged for {}. Git ignores these folders, so they aren't part of your code.",
                last_change
                    .map(|t| duration_phrase(now - t))
                    .unwrap_or_default()
            ),
            ProjectStatus::NotGit => format!(
                "Build files and dependencies ({list}). This folder isn't in git, so Pare can't confirm they're unused."
            ),
            other => format!(
                "Build files and dependencies ({list}). The project is {}, so check before removing.",
                other.label().to_lowercase()
            ),
        };
        c.if_removed = if rebuild.is_empty() {
            "Rebuild them the next time you work on the project.".into()
        } else {
            format!("Rebuild them anytime with {}.", rebuild.join(" or "))
        };
        c.note = last_change.map(|t| format!("Last change {}", age_phrase(now, t)));
        c.badge = Some(status.label().to_string());
        c.git = info.clone();
        c.facts.push(("Build files".into(), list));
        if !rebuild.is_empty() {
            c.facts.push(("Rebuild with".into(), rebuild.join(" · ")));
        }
        c.measured = Some(Measured {
            bytes,
            logical,
            items,
            dates,
            newest,
            recent_files,
        });
        Some(c)
    };

    let project = Project {
        root: root.to_path_buf(),
        name,
        git: info,
        status,
        last_change,
        idle_days: idle,
        ecosystems,
        rebuild,
        artifacts,
        bytes,
        tier,
        finding_id: None,
        parent: parent.map(|p| p.to_path_buf()),
    };
    (project, candidate)
}

/// Discover and analyze every project under `roots` (in parallel).
pub fn scan_projects(
    roots: &[PathBuf],
    ecos: &[EcosystemDef],
    env: &Env,
    now: Timestamp,
    idle_days: u64,
) -> Vec<(Project, Option<Candidate>)> {
    discover(roots, ecos, env)
        .par_iter()
        .map(|(root, parent)| analyze(root, parent.as_deref(), ecos, now, idle_days))
        .collect()
}

/// Linked worktrees of `root` whose branch is already merged into the main branch and that have
/// no uncommitted changes. Removed with `git worktree remove`, which keeps the branch.
pub fn stale_worktrees(root: &Path, now: Timestamp) -> Vec<Candidate> {
    let Ok(repo) = git2::Repository::open(root) else {
        return Vec::new();
    };
    if repo.is_worktree() {
        return Vec::new();
    }
    let Some(main) = ["main", "master", "trunk", "develop"].iter().find_map(|b| {
        repo.find_branch(b, git2::BranchType::Local)
            .ok()
            .and_then(|br| br.get().target())
    }) else {
        return Vec::new();
    };
    let Ok(names) = repo.worktrees() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for name in names.iter().filter_map(|n| n.ok().flatten()) {
        let Ok(wt) = repo.find_worktree(name) else {
            continue;
        };
        if wt.validate().is_err()
            || !matches!(wt.is_locked(), Ok(git2::WorktreeLockStatus::Unlocked))
        {
            continue;
        }
        let path = wt.path().to_path_buf();
        let Ok(wrepo) = git2::Repository::open_from_worktree(&wt) else {
            continue;
        };
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(false)
            .include_ignored(false);
        let dirty = wrepo
            .statuses(Some(&mut opts))
            .map(|s| s.iter().any(|e| e.status() != git2::Status::CURRENT))
            .unwrap_or(true);
        if dirty {
            continue;
        }
        let Some(head) = wrepo.head().ok().and_then(|h| h.target()) else {
            continue;
        };
        let merged = head == main || repo.graph_descendant_of(main, head).unwrap_or(false);
        if !merged {
            continue;
        }
        let branch = wrepo
            .head()
            .ok()
            .and_then(|h| h.shorthand().ok().map(String::from))
            .unwrap_or_else(|| name.to_string());
        let m = measure_path(&path);
        let project = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut c = Candidate::new(
            "projects.stale-worktree",
            CategoryId::ProjectBuild,
            format!("Worktree {name}"),
        );
        c.source = project.clone();
        c.source_path = Some(root.to_path_buf());
        c.extra_paths = vec![path.clone()];
        c.tier = Tier::Review;
        c.disposal = Disposal::Command {
            program: "git".into(),
            args: vec![
                "-C".into(),
                root.to_string_lossy().into_owned(),
                "worktree".into(),
                "remove".into(),
                path.to_string_lossy().into_owned(),
            ],
        };
        c.comes_back = ComesBack::OnDemand;
        c.why = format!(
            "A git worktree of {project} on branch {branch}, already merged into the main branch, with no uncommitted changes."
        );
        c.if_removed = format!(
            "Git removes the worktree folder. The branch {branch} and its commits stay in the repository; add the worktree again anytime."
        );
        c.note = Some(format!("Changed {}", age_phrase(now, m.newest)));
        c.badge = Some("Merged".into());
        c.facts.push(("Branch".into(), branch));
        let mut dates = Dates::default();
        dates.include(m.created_min, m.newest, 0);
        c.measured = Some(Measured {
            bytes: m.allocated,
            logical: m.logical,
            items: m.files,
            dates,
            newest: m.newest,
            recent_files: m.recent_files,
        });
        out.push(c);
    }
    out
}
