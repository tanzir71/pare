//! M3: every rule in rules/*.toml has a fixture test; built-in detectors; never-touch
//! property test; in-use AI sessions; tool-command exit codes.

#[path = "../common/mod.rs"]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::*;
#[cfg(windows)]
use pare_core::ProtectReason;
use pare_core::detectors::claude::encode;
use pare_core::dispose::executor::FailReason;
use pare_core::dispose::oplog::ItemStatus;
use pare_core::platform::path_starts_with;
use pare_core::rules::engine::{RuleCtx, run_rule};
use pare_core::rules::matcher::PathSet;
use pare_core::rules::{DisposalDecl, RuleSet, vars};
use pare_core::safety::protected_paths::classify;
use pare_core::scan::measure::Candidate;
use pare_core::{Disposal, Tier};

#[test]
#[ignore]
fn helper_hold_file() {
    hold_file_main();
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/home")
}

/// Copy tests/fixtures/home into the FakeHome, expanding `ENC~a~b` names and `{HOME}`.
fn copy_fixture(h: &FakeHome, age_days: f64) {
    fn go(src: &Path, dst: &Path, home: &Path) {
        for e in fs::read_dir(src).unwrap() {
            let e = e.unwrap();
            let name = e.file_name().to_string_lossy().into_owned();
            if name == "README.md" && src.ends_with("home") {
                continue;
            }
            let out_name = match name.strip_prefix("ENC~") {
                Some(rest) => encode(
                    &rest
                        .split('~')
                        .fold(home.to_path_buf(), |a, c| a.join(c))
                        .to_string_lossy(),
                ),
                None => name.clone(),
            };
            let out = dst.join(out_name);
            if e.file_type().unwrap().is_dir() {
                fs::create_dir_all(&out).unwrap();
                go(&e.path(), &out, home);
            } else {
                let mut bytes = fs::read(e.path()).unwrap();
                if name.ends_with(".jsonl") {
                    let esc = serde_json::to_string(&home.to_string_lossy()).unwrap();
                    let esc = &esc[1..esc.len() - 1];
                    bytes = String::from_utf8(bytes)
                        .unwrap()
                        .replace("{HOME}", esc)
                        .into_bytes();
                }
                fs::write(&out, bytes).unwrap();
            }
        }
    }
    go(&fixture_root(), h.home(), h.home());
    age_tree(h.home(), age_days);
}

fn ctx<'a>(h: &'a FakeHome, within: &'a [PathBuf]) -> RuleCtx<'a> {
    RuleCtx {
        env: &h.env,
        now: pare_core::platform::now_ms(),
        recent_ms: 24 * 3_600_000,
        drives: &[],
        elevated: true,
        within: Some(within),
        check_in_use: true,
    }
}

fn protect_set(set: &RuleSet, h: &FakeHome) -> PathSet {
    PathSet::new(
        set.protect
            .iter()
            .flat_map(|p| vars::expand(p, &h.env, &[]))
            .collect::<Vec<_>>(),
    )
}

fn covers(c: &Candidate, file: &Path) -> bool {
    c.extra_paths.iter().any(|p| path_starts_with(file, p))
}

/// Turn an expanded pattern into a concrete fixture path (globs → "fixture", `**` → two dirs).
fn plant(pattern: &str, n: usize) -> (PathBuf, PathBuf) {
    let segs: Vec<&str> = pattern.split('/').collect();
    let mut p = String::new();
    let mut item_root = None;
    for (i, s) in segs.iter().enumerate() {
        if i > 0 {
            p.push('/');
        }
        if *s == "**" {
            p.push_str("deep/er");
        } else if s.contains('*') {
            p.push_str(&s.replace('*', &format!("fixture{n}")));
        } else {
            p.push_str(s);
        }
        if s.contains('*') && item_root.is_none() {
            item_root = Some(PathBuf::from(&p));
        }
    }
    let last = segs.last().copied().unwrap_or("");
    let mut file = PathBuf::from(&p);
    if last == "**" || (!last.contains('.') && last.contains('*')) {
        file = file.join("file.bin");
    }
    let root = item_root.unwrap_or_else(|| file.clone());
    (file, root)
}

#[test]
fn every_rule_matches_its_fixture() {
    let set = RuleSet::builtin();
    let mut checked = 0;
    for (tool, rule) in set.all_rules() {
        if !rule.detector.is_empty() || rule.command.iter().any(|a| a.contains("{model}")) {
            continue; // dedicated tests below
        }
        let h = FakeHome::new();
        let age = rule.min_age_days.unwrap_or(0) as f64 + 5.0;
        let mut planted = Vec::new();
        for pat in &rule.paths {
            for exp in vars::expand(pat, &h.env, &[]) {
                let copies = rule.keep_newest as usize + 1;
                for n in 0..copies {
                    let (file, root) = plant(&exp, n);
                    write_file(&file, 4096, age + n as f64);
                    age_tree(&root, age + n as f64);
                    // The oldest copy is the one past keep_newest.
                    if n == copies - 1 {
                        planted.push(file);
                    }
                }
            }
        }
        let within = vec![h.dir.path().to_path_buf()];
        let cands = run_rule(tool, rule, &ctx(&h, &within), &protect_set(&set, &h));
        assert!(
            !cands.is_empty(),
            "{} matched nothing; planted {planted:#?}",
            rule.id
        );
        for f in &planted {
            assert!(
                cands.iter().any(|c| covers(c, f)),
                "{}: {} not covered by {:#?}",
                rule.id,
                f.display(),
                cands.iter().map(|c| &c.extra_paths).collect::<Vec<_>>()
            );
        }
        for c in &cands {
            assert_eq!(c.rule_id, rule.id);
            match rule.disposal {
                DisposalDecl::Guide => assert!(matches!(c.disposal, Disposal::Guide { .. })),
                DisposalDecl::Command => assert!(matches!(c.disposal, Disposal::Command { .. })),
                DisposalDecl::Delete => assert_eq!(c.disposal, Disposal::Delete),
                DisposalDecl::Recycle => assert_eq!(c.disposal, Disposal::Recycle),
            }
        }
        checked += 1;
    }
    assert!(checked >= 25, "only {checked} rules checked");
}

#[test]
fn min_age_and_keep_newest_filter() {
    let set = RuleSet::builtin();
    let (tool, rule) = set.rule("claude-code.debug-logs").unwrap();
    let h = FakeHome::new();
    let old = h.file(".claude/debug/old.txt", 1000, 30.0);
    let young = h.file(".claude/debug/young.txt", 1000, 3.0);
    let within = vec![h.dir.path().to_path_buf()];
    let c = run_rule(tool, rule, &ctx(&h, &within), &protect_set(&set, &h));
    assert_eq!(c.len(), 1);
    assert!(covers(&c[0], &old));
    assert!(!covers(&c[0], &young), "younger than min_age_days");

    let (tool, rule) = set.rule("claude-code.old-versions").unwrap();
    for (i, v) in ["1.0.1", "1.0.2", "1.0.3"].iter().enumerate() {
        let d = h.home().join(".local/share/claude/versions").join(v);
        write_file(&d.join("claude.exe"), 1000, 30.0 - i as f64);
        age_tree(&d, 30.0 - i as f64);
    }
    let c = run_rule(tool, rule, &ctx(&h, &within), &protect_set(&set, &h));
    let all: Vec<&PathBuf> = c.iter().flat_map(|c| &c.extra_paths).collect();
    assert_eq!(all.len(), 1, "keeps the 2 newest: {all:?}");
    assert!(all[0].ends_with("1.0.1"));
}

#[test]
fn claude_orphaned_transcripts() {
    let h = FakeHome::new();
    copy_fixture(&h, 60.0);
    let set = RuleSet::builtin();
    let (tool, rule) = set.rule("claude-code.orphaned-transcripts").unwrap();
    let within = vec![h.dir.path().to_path_buf()];
    let c = run_rule(tool, rule, &ctx(&h, &within), &protect_set(&set, &h));
    let orphan = c
        .iter()
        .find(|c| c.title == "Transcripts of deleted projects")
        .unwrap_or_else(|| {
            panic!(
                "no orphan finding in {:#?}",
                c.iter()
                    .map(|c| (&c.title, &c.extra_paths))
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(orphan.tier, Tier::Safe);
    assert_eq!(orphan.badge.as_deref(), Some("Orphaned"));
    assert_eq!(orphan.disposal, Disposal::Recycle);
    let deleted = h.home().join(".claude/projects").join(encode(
        &h.home().join("code").join("deleted-app").to_string_lossy(),
    ));
    let live = h.home().join(".claude/projects").join(encode(
        &h.home().join("code").join("live-app").to_string_lossy(),
    ));
    assert!(covers(orphan, &deleted.join("0001.jsonl")));
    assert!(
        !covers(orphan, &deleted.join("memory").join("notes.md")),
        "project memory is never-touch"
    );
    assert!(
        !c.iter().any(|c| covers(c, &live.join("0002.jsonl"))),
        "live project kept"
    );
}

#[test]
fn codex_superseded_releases_keep_current_and_newest() {
    let h = FakeHome::new();
    copy_fixture(&h, 60.0);
    let rel = h.home().join(".codex/packages/standalone/releases");
    for (v, age) in [("0.40.0", 90.0), ("0.41.0", 60.0), ("0.42.0", 30.0)] {
        age_tree(&rel.join(v), age);
    }
    let set = RuleSet::builtin();
    let (tool, rule) = set.rule("codex.old-releases").unwrap();
    let within = vec![h.dir.path().to_path_buf()];
    let c = run_rule(tool, rule, &ctx(&h, &within), &protect_set(&set, &h));
    assert_eq!(c.len(), 1, "{c:#?}");
    assert_eq!(c[0].extra_paths, vec![rel.join("0.40.0")]);
    assert_eq!(c[0].badge.as_deref(), Some("Old versions"));
}

#[test]
fn ollama_models_count_only_unshared_blobs() {
    let h = FakeHome::new();
    copy_fixture(&h, 60.0);
    let set = RuleSet::builtin();
    let (tool, rule) = set.rule("ollama.models").unwrap();
    let within = vec![h.dir.path().to_path_buf()];
    let c = run_rule(tool, rule, &ctx(&h, &within), &protect_set(&set, &h));
    let tiny = c
        .iter()
        .find(|c| c.title == "Model tiny:latest")
        .expect("tiny");
    assert_eq!(
        tiny.disposal,
        Disposal::Command {
            program: "ollama".into(),
            args: vec!["rm".into(), "tiny:latest".into()]
        }
    );
    let blobs = h.home().join(".ollama/models/blobs");
    let alloc = |n: &str| pare_core::platform::stat(&blobs.join(n)).unwrap().allocated;
    assert_eq!(
        tiny.measured.as_ref().unwrap().bytes,
        alloc("sha256-aaa") + alloc("sha256-cfg1")
    );
    assert!(c.iter().any(|c| c.title == "Model small:q4"));
}

/// M3 acceptance: config files in the never-touch list are never matched, even by broad globs.
mod never_touch_property {
    use super::*;
    use pare_core::ComesBack;
    use pare_core::rules::{LoadedTool, RuleDef, TierDecl, ToolDef};
    use proptest::prelude::*;

    const NEVER: &[&str] = &[
        ".ssh/id_ed25519",
        ".ssh/config",
        ".gnupg/pubring.kbx",
        ".claude/settings.json",
        ".claude/settings.local.json",
        ".claude/.credentials.json",
        ".claude.json",
        ".codex/auth.json",
        ".codex/config.toml",
        ".claude/projects/p/memory/notes.md",
        ".claude/agents/a.md",
        ".codex/AGENTS.md",
        ".cargo/bin/cargo.exe",
    ];
    const OTHER: &[&str] = &[
        ".claude/debug/x.txt",
        ".codex/sessions/r.jsonl",
        ".cache/y.bin",
        "notes/z.txt",
    ];

    fn broad_rule(pattern: &str) -> (LoadedTool, RuleDef) {
        let rule = RuleDef {
            id: "test.broad".into(),
            title: "Broad".into(),
            paths: vec![pattern.into()],
            exclude: vec![],
            min_age_days: None,
            keep_newest: 0,
            tier: TierDecl::Safe,
            disposal: DisposalDecl::Recycle,
            command: vec![],
            guide: vec![],
            comes_back: ComesBack::No,
            why: "x".into(),
            if_removed: "y".into(),
            requires_closed: false,
            admin: false,
            detector: String::new(),
            verified: false,
            sources: vec![],
        };
        let tool = LoadedTool {
            tool: ToolDef {
                id: "test".into(),
                name: "Test".into(),
                category: "ai-tools".into(),
                processes: vec![],
                docs: None,
            },
            rules: vec![rule.clone()],
            file: "test.toml".into(),
            builtin: false,
        };
        (tool, rule)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(24))]
        #[test]
        fn broad_globs_never_match_never_touch(
            never in proptest::sample::subsequence(NEVER.to_vec(), 1..NEVER.len()),
            other in proptest::sample::subsequence(OTHER.to_vec(), 0..OTHER.len()),
            pattern in proptest::sample::select(vec!["~/**", "~/*", "~/.claude/**", "~/.codex/**", "~/.*", "~/.*/**", "~/.claude/*", "~/.claude/projects/*/**"]),
        ) {
            let h = FakeHome::new();
            for f in never.iter().chain(other.iter()) {
                h.file(f, 100, 40.0);
            }
            age_tree(h.home(), 40.0);
            let set = RuleSet::builtin();
            let (tool, rule) = broad_rule(pattern);
            let within = vec![h.dir.path().to_path_buf()];
            let cands = run_rule(&tool, &rule, &ctx(&h, &within), &protect_set(&set, &h));
            for c in &cands {
                for p in &c.extra_paths {
                    prop_assert!(classify(p, &h.env, false).is_none(), "{} is never-touch", p.display());
                    for f in &never {
                        let nt = f.split('/').fold(h.home().to_path_buf(), |a, s| a.join(s));
                        prop_assert!(!path_starts_with(&nt, p), "{} would remove never-touch {}", p.display(), nt.display());
                    }
                }
            }
        }
    }
}

/// M3 acceptance: running AI sessions are never selectable (a fake process holds the file).
#[cfg(windows)]
#[test]
fn running_session_is_never_selectable() {
    let h = FakeHome::new();
    copy_fixture(&h, 60.0);
    let deleted = h.home().join(".claude/projects").join(encode(
        &h.home().join("code").join("deleted-app").to_string_lossy(),
    ));
    let session = deleted.join("0001.jsonl");
    let _holder = Holder::spawn(&session);
    let mut cfg = h.config();
    cfg.rules = Some(std::sync::Arc::new(RuleSet::builtin()));
    let out = h.scan_with(&[h.home()], &cfg);
    let f = out
        .result
        .findings
        .iter()
        // The holder opens the transcript exclusively, so its cwd can't be read: the folder is
        // then judged by its name alone. Either way it must be protected.
        .find(|f| {
            f.rule_id == "claude-code.orphaned-transcripts"
                && f.paths.iter().any(|p| path_starts_with(&session, p))
        })
        .unwrap_or_else(|| panic!("no finding covers the session: {:#?}", out.result.findings));
    assert!(
        matches!(
            f.tier,
            Tier::Protected {
                reason: ProtectReason::InUse,
                ..
            }
        ),
        "{:?}",
        f.tier
    );
    assert!(!f.is_selectable());
    let report = h.clean(std::slice::from_ref(f), out.result.started);
    assert_eq!(report.skipped, 1);
    assert!(session.exists());
}

/// M3 acceptance: tool commands report their exit status; a failure doesn't block other items.
#[test]
fn tool_command_exit_status_and_isolation() {
    let h = FakeHome::new();
    let p = h.file("Downloads/old.part", 1000, 5.0);
    let out = h.scan();
    let partial = find(&out.result.findings, "downloads.partial", "old.part")
        .unwrap()
        .clone();
    let (program, args): (&str, Vec<String>) = if cfg!(windows) {
        ("cmd", vec!["/c".into(), "echo nope & exit 3".into()])
    } else {
        ("sh", vec!["-c".into(), "echo nope; exit 3".into()])
    };
    let mut cmd = partial.clone();
    cmd.id = "cmd-finding".into();
    cmd.title = "Fake tool cleanup".into();
    cmd.rule_id = "test.command".into();
    cmd.disposal = Disposal::Command {
        program: program.into(),
        args,
    };
    cmd.paths = vec![h.dir("cache-dir")];
    let report = h.clean(&[cmd, partial], out.result.started);
    let c = report
        .results
        .iter()
        .find(|r| r.finding_id == "cmd-finding")
        .unwrap();
    assert_eq!(c.status, ItemStatus::Failed);
    assert_eq!(c.reason, Some(FailReason::CommandFailed));
    assert!(
        c.error.as_deref().unwrap().contains("code 3"),
        "{:?}",
        c.error
    );
    assert!(c.output.as_deref().unwrap().contains("nope"));
    let d = report
        .results
        .iter()
        .find(|r| r.title == "old.part")
        .unwrap();
    assert_eq!(d.status, ItemStatus::Ok);
    assert!(!p.exists());
}

/// One file another app holds open is split out; the rest of %TEMP% stays suggestible.
#[cfg(windows)]
#[test]
fn held_temp_file_does_not_block_the_rest() {
    let h = FakeHome::new();
    let free = h.env.temp.join("old-free.tmp");
    let held = h.env.temp.join("old-held.tmp");
    write_file(&free, 4096, 5.0);
    write_file(&held, 4096, 5.0);
    let _holder = Holder::spawn(&held);
    let set = RuleSet::builtin();
    let (tool, rule) = set.rule("windows.user-temp").unwrap();
    let within = vec![h.dir.path().to_path_buf()];
    let c = run_rule(tool, rule, &ctx(&h, &within), &protect_set(&set, &h));
    let main = c
        .iter()
        .find(|c| c.tier == Tier::Safe)
        .expect("suggestible part");
    assert!(covers(main, &free));
    assert!(!covers(main, &held));
    let kept = c.iter().find(|c| c.tier.is_protected()).expect("held part");
    assert!(covers(kept, &held));
    assert!(matches!(
        kept.tier,
        Tier::Protected {
            reason: ProtectReason::InUse,
            ..
        }
    ));
}
