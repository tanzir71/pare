//! M4: git-aware projects. Fixture repositories are built with libgit2 inside a FakeHome.

mod common;

use std::path::{Path, PathBuf};
use std::time::Instant;

use common::*;
use git2::{Repository, Signature, Time};
use pare_core::projects::{ProjectStatus, scan_projects};
use pare_core::rules::RuleSet;
use pare_core::{Disposal, ProtectReason, Tier};

const OLD: f64 = 200.0;

fn sig(days_ago: f64) -> Signature<'static> {
    let secs = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
        - days_ago * DAY) as i64;
    Signature::new("Fixture", "fixture@example.invalid", &Time::new(secs, 0)).unwrap()
}

/// Create a repo with `files` (path, contents) committed `days_ago`, plus `.gitignore`.
fn repo(dir: &Path, gitignore: &str, files: &[(&str, &str)], days_ago: f64) -> Repository {
    std::fs::create_dir_all(dir).unwrap();
    let r = Repository::init(dir).unwrap();
    std::fs::write(dir.join(".gitignore"), gitignore).unwrap();
    for (p, c) in files {
        let f = dir.join(p);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, c).unwrap();
    }
    age_tree(dir, days_ago);
    commit_all(&r, "initial", days_ago);
    r
}

fn commit_all(r: &Repository, msg: &str, days_ago: f64) -> git2::Oid {
    let mut idx = r.index().unwrap();
    idx.add_all(["*"], git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    idx.write().unwrap();
    let tree = r.find_tree(idx.write_tree().unwrap()).unwrap();
    let parents: Vec<git2::Commit> = r
        .head()
        .ok()
        .and_then(|h| h.peel_to_commit().ok())
        .into_iter()
        .collect();
    let refs: Vec<&git2::Commit> = parents.iter().collect();
    let s = sig(days_ago);
    r.commit(Some("HEAD"), &s, &s, msg, &tree, &refs).unwrap()
}

/// Pretend the branch was pushed: an `origin` remote and an up-to-date tracking ref.
fn set_pushed(r: &Repository) {
    r.remote("origin", "https://example.invalid/fixture.git")
        .unwrap();
    let head = r.head().unwrap();
    let branch = head.shorthand().unwrap().to_string();
    let oid = head.target().unwrap();
    r.reference(
        &format!("refs/remotes/origin/{branch}"),
        oid,
        true,
        "fixture",
    )
    .unwrap();
    let mut b = r.find_branch(&branch, git2::BranchType::Local).unwrap();
    b.set_upstream(Some(&format!("origin/{branch}"))).unwrap();
}

fn node_modules(dir: &Path, n: usize) {
    for i in 0..n {
        write_file(
            &dir.join("node_modules")
                .join(format!("pkg{i}"))
                .join("index.js"),
            2000,
            OLD,
        );
    }
    age_tree(&dir.join("node_modules"), OLD);
}

fn ecos() -> Vec<pare_core::rules::EcosystemDef> {
    RuleSet::builtin().ecosystems
}

fn by_name<'a>(
    v: &'a [(
        pare_core::projects::Project,
        Option<pare_core::scan::measure::Candidate>,
    )],
    name: &str,
) -> &'a (
    pare_core::projects::Project,
    Option<pare_core::scan::measure::Candidate>,
) {
    v.iter().find(|(p, _)| p.name == name).unwrap_or_else(|| {
        panic!(
            "project {name} not found in {:?}",
            v.iter().map(|(p, _)| &p.name).collect::<Vec<_>>()
        )
    })
}

#[test]
fn fixture_repos_cover_every_git_state() {
    let h = FakeHome::new();
    let code = h.dir("code");
    let pkg = r#"{"name":"x"}"#;
    let ignore = "node_modules/\ntarget/\n";

    // clean + pushed + idle → Safe
    let clean = code.join("clean-pushed");
    let r = repo(
        &clean,
        ignore,
        &[("package.json", pkg), ("src/index.js", "1")],
        OLD,
    );
    set_pushed(&r);
    node_modules(&clean, 3);

    // dirty: a tracked file changed → Protected
    let dirty = code.join("dirty");
    let r = repo(
        &dirty,
        ignore,
        &[("package.json", pkg), ("a.txt", "1")],
        OLD,
    );
    set_pushed(&r);
    std::fs::write(dirty.join("a.txt"), "changed").unwrap();
    node_modules(&dirty, 1);

    // untracked, not ignored file → Protected
    let untracked = code.join("untracked");
    let r = repo(&untracked, ignore, &[("package.json", pkg)], OLD);
    set_pushed(&r);
    std::fs::write(untracked.join("notes.txt"), "new").unwrap();
    node_modules(&untracked, 1);

    // ahead of upstream → Review "Unpushed"
    let ahead = code.join("ahead");
    let r = repo(&ahead, ignore, &[("package.json", pkg)], OLD);
    set_pushed(&r);
    std::fs::write(ahead.join("more.txt"), "x").unwrap();
    age_tree(&ahead, OLD);
    commit_all(&r, "local only", OLD);
    node_modules(&ahead, 1);

    // no remote → Review
    let noremote = code.join("no-remote");
    repo(&noremote, ignore, &[("package.json", pkg)], OLD);
    node_modules(&noremote, 1);

    // detached HEAD → Review
    let detached = code.join("detached");
    let r = repo(&detached, ignore, &[("package.json", pkg)], OLD);
    set_pushed(&r);
    let oid = r.head().unwrap().target().unwrap();
    r.set_head_detached(oid).unwrap();
    node_modules(&detached, 1);

    // nested repo (as a submodule checkout would be) inside a clean repo
    let outer = code.join("outer");
    let r = repo(
        &outer,
        "node_modules/\nvendor/\n",
        &[("package.json", pkg)],
        OLD,
    );
    set_pushed(&r);
    let inner = outer.join("vendor").join("inner");
    let ri = repo(
        &inner,
        "target/\n",
        &[("Cargo.toml", "[package]\nname=\"inner\"")],
        OLD,
    );
    set_pushed(&ri);
    write_file(&inner.join("target/debug/app.exe"), 5000, OLD);
    age_tree(&inner.join("target"), OLD);

    // linked worktree (a `.git` *file*)
    let wt_main = code.join("wt-main");
    let r = repo(
        &wt_main,
        ignore,
        &[("Cargo.toml", "[package]\nname=\"w\"")],
        OLD,
    );
    set_pushed(&r);
    let wt = code.join("wt-feature");
    r.worktree("wt-feature", &wt, None).unwrap();

    // an artifact dir git does NOT ignore (tracked build output) → never suggested
    let tracked_build = code.join("tracked-build");
    let r = repo(
        &tracked_build,
        "node_modules/\n",
        &[("package.json", pkg), ("build/out.js", "tracked")],
        OLD,
    );
    set_pushed(&r);
    node_modules(&tracked_build, 1);

    // not in git → Review, Recycle Bin
    let plain = code.join("plain");
    write_file(&plain.join("package.json"), 20, OLD);
    node_modules(&plain, 1);
    age_tree(&plain, OLD);

    // recently active (but clean & pushed) → Review
    let active = code.join("active");
    let r = repo(&active, ignore, &[("package.json", pkg)], 3.0);
    set_pushed(&r);
    node_modules(&active, 1);

    let now = pare_core::platform::now_ms();
    let found = scan_projects(std::slice::from_ref(&code), &ecos(), &h.env, now, 90);

    let (p, c) = by_name(&found, "clean-pushed");
    assert_eq!(p.status, ProjectStatus::CleanPushed);
    let c = c.as_ref().expect("clean-pushed has artifacts");
    assert_eq!(c.tier, Tier::Safe);
    assert_eq!(c.disposal, Disposal::Delete);
    assert_eq!(c.extra_paths, vec![clean.join("node_modules")]);
    assert!(p.idle_days.unwrap() >= 90);
    assert!(p.git.as_ref().unwrap().commits >= 1);

    for name in ["dirty", "untracked"] {
        let (p, c) = by_name(&found, name);
        assert_eq!(p.status, ProjectStatus::Uncommitted, "{name}");
        assert!(
            matches!(
                c.as_ref().unwrap().tier,
                Tier::Protected {
                    reason: ProtectReason::Uncommitted,
                    ..
                }
            ),
            "{name}"
        );
    }
    let (p, c) = by_name(&found, "ahead");
    assert_eq!(p.status, ProjectStatus::Unpushed);
    assert_eq!(p.git.as_ref().unwrap().ahead, 1);
    assert_eq!(c.as_ref().unwrap().tier, Tier::Review);
    assert_eq!(
        by_name(&found, "no-remote").0.status,
        ProjectStatus::NoRemote
    );
    assert_eq!(
        by_name(&found, "no-remote").1.as_ref().unwrap().tier,
        Tier::Review
    );
    assert_eq!(
        by_name(&found, "detached").0.status,
        ProjectStatus::Detached
    );
    assert_eq!(by_name(&found, "active").0.status, ProjectStatus::Active);
    assert_eq!(
        by_name(&found, "active").1.as_ref().unwrap().tier,
        Tier::Review
    );

    // Nested repo: found on its own, with its parent recorded; the outer repo doesn't claim it.
    let (pi, ci) = by_name(&found, "inner");
    assert_eq!(pi.parent.as_deref(), Some(outer.as_path()));
    assert_eq!(ci.as_ref().unwrap().extra_paths, vec![inner.join("target")]);
    assert!(
        by_name(&found, "outer").1.is_none(),
        "outer has no node_modules of its own"
    );

    // Worktree opens through its .git file.
    let (pw, _) = by_name(&found, "wt-feature");
    assert!(pw.git.is_some(), "worktree read as a git repo");

    // Tracked build/ is never suggested; ignored node_modules is.
    let (_, ct) = by_name(&found, "tracked-build");
    let paths = &ct.as_ref().unwrap().extra_paths;
    assert!(paths.contains(&tracked_build.join("node_modules")));
    assert!(!paths.iter().any(|p| p.ends_with("build")), "{paths:?}");

    let (pp, cp) = by_name(&found, "plain");
    assert_eq!(pp.status, ProjectStatus::NotGit);
    let cp = cp.as_ref().unwrap();
    assert_eq!(cp.tier, Tier::Review);
    assert_eq!(cp.disposal, Disposal::Recycle);
}

#[test]
fn scan_links_projects_to_findings() {
    let h = FakeHome::new();
    let code = h.dir("code");
    let p = code.join("old-app");
    let r = repo(&p, "node_modules/\n", &[("package.json", "{}")], OLD);
    set_pushed(&r);
    node_modules(&p, 5);
    let mut cfg = h.config();
    cfg.rules = Some(std::sync::Arc::new(RuleSet::builtin()));
    cfg.project_roots = vec![code.clone()];
    let out = h.scan_with(&[h.home()], &cfg);
    let proj = out
        .result
        .projects
        .iter()
        .find(|x| x.name == "old-app")
        .unwrap();
    let fid = proj.finding_id.as_ref().expect("linked finding");
    let f = out.result.findings.iter().find(|f| &f.id == fid).unwrap();
    assert_eq!(f.category, pare_core::CategoryId::ProjectBuild);
    assert_eq!(f.tier, Tier::Safe);
    assert!(f.bytes > 0);
    let card = out
        .result
        .categories
        .iter()
        .find(|c| c.info.id == pare_core::CategoryId::ProjectBuild)
        .unwrap();
    assert_eq!(card.safe_bytes, f.bytes);
}

/// M4 acceptance: 50 repos scan in under 3 s with a warm cache.
#[test]
fn fifty_repos_under_three_seconds() {
    let h = FakeHome::new();
    let code = h.dir("code");
    for i in 0..50 {
        let p = code.join(format!("repo-{i:02}"));
        let r = repo(
            &p,
            "node_modules/\n",
            &[("package.json", "{}"), ("src/a.js", "a"), ("src/b.js", "b")],
            OLD,
        );
        set_pushed(&r);
        node_modules(&p, 20);
    }
    let roots: Vec<PathBuf> = vec![code];
    let now = pare_core::platform::now_ms();
    let _warm = scan_projects(&roots, &ecos(), &h.env, now, 90);
    let t = Instant::now();
    let found = scan_projects(&roots, &ecos(), &h.env, now, 90);
    let elapsed = t.elapsed();
    assert_eq!(found.len(), 50);
    assert!(
        found
            .iter()
            .all(|(p, c)| p.status == ProjectStatus::CleanPushed && c.is_some())
    );
    println!("50 repos in {elapsed:?}");
    assert!(elapsed.as_secs_f64() < 3.0, "took {elapsed:?}");
}
