//! One test per safety invariant in CLAUDE.md. `invariant_checklist` fails if an invariant is
//! added there without a matching `invN_` test here.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::*;
use pare_core::dispose::executor::FailReason;
use pare_core::dispose::oplog::{ItemStatus, LogRecord, read_log};
use pare_core::safety::protected_paths::{classify, contains_never_touch};
use pare_core::safety::{SafetyOptions, assess};
use pare_core::scan::measure::Probe;
use pare_core::{ComesBack, Disposal, Env, ProtectReason, Tier};

#[test]
#[ignore]
fn helper_hold_file() {
    hold_file_main();
}

#[test]
fn invariant_checklist() {
    let claude_md =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CLAUDE.md")).unwrap();
    let section = claude_md
        .split("## Safety invariants")
        .nth(1)
        .unwrap()
        .split("\n## ")
        .next()
        .unwrap();
    let count = section
        .lines()
        .filter(|l| {
            l.trim_start()
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
                && l.contains(". **")
        })
        .count();
    assert_eq!(
        count, 10,
        "CLAUDE.md lists {count} invariants; update this checklist"
    );
    let me = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/invariants.rs"))
        .unwrap();
    for i in 1..=count {
        assert!(
            me.contains(&format!("fn inv{i}_")),
            "missing a test for invariant {i}"
        );
    }
}

/// 1. Nothing is deleted without an explicit user action: a scan only reads.
#[test]
fn inv1_scan_only_reads() {
    let h = FakeHome::new();
    let files = [
        h.file("Downloads/setup.exe", 2000, 30.0),
        h.file("Downloads/movie.part", 2000, 10.0),
        h.file("Downloads/very-old.txt", 100, 400.0),
        h.file("Documents/big.iso", 3_000_000, 200.0),
    ];
    h.dir("Documents/empty/inner");
    let before: Vec<_> = files
        .iter()
        .map(|f| fs::metadata(f).unwrap().modified().unwrap())
        .collect();
    let mut cfg = h.config();
    cfg.thresholds.large_file_mb = 1;
    let out = h.scan_with(&[h.home()], &cfg);
    assert!(
        !out.result.findings.is_empty(),
        "fixture should produce findings"
    );
    for (f, t) in files.iter().zip(before) {
        assert!(f.exists(), "{} disappeared during a scan", f.display());
        assert_eq!(fs::metadata(f).unwrap().modified().unwrap(), t);
    }
    assert!(h.home().join("Documents/empty/inner").exists());
    assert!(
        !h.env.oplog_path().exists(),
        "a scan must not write disposal records"
    );
}

/// 2. Disposal follows the rule: `delete` only for rebuildable data; everything else is recycled.
#[test]
fn inv2_disposal_follows_rule() {
    let h = FakeHome::new();
    h.file("Downloads/setup.exe", 2000, 30.0);
    h.file("Downloads/movie.crdownload", 2000, 10.0);
    h.file("Downloads/archive.zip", 2000, 30.0);
    h.dir("Downloads/archive");
    h.file("Downloads/archive/a.txt", 10, 30.0);
    h.file("Downloads/old.pdf", 2000, 400.0);
    h.file("Documents/big.iso", 3_000_000, 200.0);
    h.dir("Documents/empty");
    let mut cfg = h.config();
    cfg.thresholds.large_file_mb = 1;
    let out = h.scan_with(&[h.home()], &cfg);
    for f in &out.result.findings {
        if f.disposal == Disposal::Delete {
            assert!(
                f.comes_back != ComesBack::No || f.bytes == 0,
                "{} is deleted directly but doesn't come back",
                f.rule_id
            );
        }
    }
    // Your own files go to the Recycle Bin.
    for rule in [
        "downloads.installers",
        "downloads.old",
        "downloads.extracted-archives",
        "quick.large-files",
    ] {
        let f = out
            .result
            .findings
            .iter()
            .find(|f| f.rule_id == rule)
            .unwrap_or_else(|| panic!("{rule}"));
        assert_eq!(f.disposal, Disposal::Recycle, "{rule}");
    }
}

/// 3. Protected never becomes selectable: recent, open in a process, system, excluded.
#[test]
fn inv3_protected_never_selectable() {
    let h = FakeHome::new();
    let recent = h.file("Downloads/new-setup.exe", 2000, 0.2); // installer rule needs 7 d, so…
    let recent_old_dl = h.file("Downloads/recent-partial.crdownload", 2000, 1.5);
    fs::write(&recent_old_dl, b"touched").unwrap(); // modified now: < 24 h
    let _ = recent;
    let mut cfg = h.config();
    cfg.thresholds.partial_days = 0;
    let out = h.scan_with(&[h.home()], &cfg);
    let f = find(
        &out.result.findings,
        "downloads.partial",
        "recent-partial.crdownload",
    )
    .expect("partial found");
    assert_eq!(f.tier, Tier::protected(ProtectReason::RecentlyModified));
    assert!(!f.is_selectable());

    // Even if a protected finding is passed to clean(), it is skipped and survives.
    let report = h.clean(std::slice::from_ref(f), out.result.started);
    assert_eq!(report.skipped, 1);
    assert_eq!(report.results[0].reason, Some(FailReason::Protected));
    assert!(recent_old_dl.exists());

    // Exclusions ("Always keep this").
    let kept = h.file("Downloads/keep-me.msi", 2000, 30.0);
    let mut cfg = h.config();
    cfg.exclusions = vec![kept.clone()];
    let out = h.scan_with(&[h.home()], &cfg);
    let f = find(&out.result.findings, "downloads.installers", "keep-me.msi").unwrap();
    assert_eq!(f.tier, Tier::protected(ProtectReason::Excluded));
}

/// 3 (continued). A file held open by another process is Protected ("In use").
#[cfg(windows)]
#[test]
fn inv3_open_in_a_process_is_protected() {
    let h = FakeHome::new();
    let held = h.file("Downloads/held-setup.exe", 4096, 30.0);
    let _holder = Holder::spawn(&held);
    let out = h.scan();
    let f = find(
        &out.result.findings,
        "downloads.installers",
        "held-setup.exe",
    )
    .expect("installer found");
    match &f.tier {
        Tier::Protected {
            reason: ProtectReason::InUse,
            detail,
        } => {
            assert!(
                detail.as_deref().unwrap_or("").starts_with("Open in"),
                "{detail:?}"
            );
        }
        other => panic!("expected In use, got {other:?}"),
    }
    assert!(!f.is_selectable());
}

/// 4. Never follow reparse points (scan or delete). Deleting a link removes the link only.
#[cfg(windows)]
#[test]
fn inv4_never_follow_reparse_points() {
    let h = FakeHome::new();
    let outside = h.dir.path().join("outside-target");
    write_file(&outside.join("precious.bin"), 5_000_000, 400.0);
    let selected = h.dir("Downloads/old-project");
    write_file(&selected.join("readme.txt"), 100, 400.0);
    junction::create(&outside, selected.join("link-to-outside")).unwrap();
    age_tree(&selected, 400.0);

    // Scan: the junction is a leaf; the 5 MB behind it is never counted.
    let out = h.scan();
    let all_bytes: u64 = out.result.findings.iter().map(|f| f.bytes).sum();
    assert!(
        all_bytes < 5_000_000,
        "scan followed a junction ({all_bytes} bytes)"
    );
    assert!(
        out.tree
            .find(&selected.join("link-to-outside").join("precious.bin"))
            .is_none()
    );

    // Delete: the folder goes, the junction is removed as a link, the target survives.
    let mut f = find(&out.result.findings, "downloads.old", "old-project")
        .expect("old download")
        .clone();
    f.disposal = Disposal::Delete; // exercise the direct-delete path
    f.comes_back = ComesBack::OnDemand;
    let report = h.clean(&[f], out.result.started);
    assert_eq!(report.ok, 1, "{:?}", report.results);
    assert!(!selected.exists());
    assert!(
        outside.join("precious.bin").exists(),
        "junction target was deleted"
    );
}

/// 5. Never hydrate cloud files: placeholders are Protected and never read.
#[test]
fn inv5_never_hydrate_cloud_files() {
    use pare_core::platform::{
        ATTR_OFFLINE, ATTR_RECALL_ON_DATA_ACCESS, ATTR_RECALL_ON_OPEN, is_cloud_attrs,
    };
    for a in [
        ATTR_RECALL_ON_DATA_ACCESS,
        ATTR_RECALL_ON_OPEN,
        ATTR_OFFLINE,
    ] {
        assert!(is_cloud_attrs(a));
    }
    assert!(!is_cloud_attrs(0x20));
    let h = FakeHome::new();
    h.file("Downloads/cloud-setup.exe", 10, 30.0);
    let out = h.scan();
    let mut f = find(
        &out.result.findings,
        "downloads.installers",
        "cloud-setup.exe",
    )
    .unwrap()
    .clone();
    f.tier = Tier::Review;
    let probe = Probe {
        any_cloud: true,
        ..Probe::default()
    };
    assess(
        &mut f,
        &probe,
        &h.env,
        &SafetyOptions::new(pare_core::platform::now_ms()),
    );
    assert_eq!(f.tier, Tier::protected(ProtectReason::CloudOnly));
}

/// 6. Hard-excluded roots and never-touch config are never matched, scanned or removed.
#[test]
fn inv6_hard_excluded_roots() {
    let h = FakeHome::new();
    let env = &h.env;
    if cfg!(windows) {
        for p in [
            r"C:\Windows\System32\drivers\etc\hosts",
            r"C:\Program Files\App\app.exe",
            r"C:\Program Files (x86)\App\app.exe",
            r"D:\ProgramData\Thing\cache.bin",
            r"C:\Program Files\WindowsApps\pkg",
            r"E:\WindowsApps\pkg",
            r"C:\System Volume Information\x",
            r"C:\$Recycle.Bin\S-1-5\x",
            r"C:\pagefile.sys",
            r"C:\hiberfil.sys",
            r"C:\swapfile.sys",
        ] {
            assert!(
                classify(Path::new(p), env, false).is_some(),
                "{p} must be excluded"
            );
        }
        // Admin exceptions only reach Windows\Temp and WER.
        assert!(classify(Path::new(r"C:\Windows\Temp\x.tmp"), env, false).is_some());
        assert!(classify(Path::new(r"C:\Windows\Temp\x.tmp"), env, true).is_none());
        assert!(
            classify(
                Path::new(r"C:\ProgramData\Microsoft\Windows\WER\ReportArchive\r"),
                env,
                true
            )
            .is_none()
        );
        assert!(classify(Path::new(r"C:\ProgramData\Microsoft\Other"), env, true).is_some());
        assert!(classify(Path::new(r"C:\Windows\System32"), env, true).is_some());
    }
    for rel in [
        ".ssh/id_ed25519",
        ".gnupg/pubring.kbx",
        ".claude/settings.json",
        ".claude/settings.local.json",
        ".codex/auth.json",
        ".codex/config.toml",
    ] {
        let p = rel.split('/').fold(env.home.clone(), |a, c| a.join(c));
        assert!(
            classify(&p, env, true).is_some(),
            "{rel} must be never-touch"
        );
    }
    assert!(
        classify(
            &env.home.join(".claude").join("debug").join("x.txt"),
            env,
            false
        )
        .is_none()
    );
    assert!(contains_never_touch(&env.home.join(".claude"), env));
    assert!(contains_never_touch(&env.home, env));
    assert!(!contains_never_touch(
        &env.home.join(".claude").join("debug"),
        env
    ));

    // The walker never enters ~\.ssh; nothing from it can become a finding.
    h.file(".ssh/old-key.zip", 10, 400.0);
    h.dir(".ssh/old-key");
    let out = h.scan();
    assert!(
        out.tree
            .find(&env.home.join(".ssh").join("old-key.zip"))
            .is_none()
    );
    assert!(
        out.result
            .findings
            .iter()
            .all(|f| f.paths.iter().all(|p| classify(p, env, true).is_none()))
    );
}

/// 7. Sizes are allocated bytes, hard links counted once, and only when all links are inside.
#[cfg(windows)]
#[test]
fn inv7_allocated_size_and_hard_links() {
    let h = FakeHome::new();
    let dl = h.dir("Downloads/old-folder");
    write_file(&dl.join("a.bin"), 1_000_000, 400.0);
    fs::hard_link(dl.join("a.bin"), dl.join("a-link.bin")).unwrap();
    write_file(&dl.join("b.bin"), 1_000_000, 400.0);
    // A hard link that lives outside the selection: removing b frees nothing.
    let elsewhere = h.dir("Documents/keep");
    fs::hard_link(dl.join("b.bin"), elsewhere.join("b-elsewhere.bin")).unwrap();
    age_tree(&dl, 400.0);
    let out = h.scan();
    let f = find(&out.result.findings, "downloads.old", "old-folder").expect("old folder");
    let a_alloc = pare_core::platform::stat(&dl.join("a.bin"))
        .unwrap()
        .allocated;
    assert!(a_alloc >= 1_000_000);
    assert_eq!(
        f.bytes, a_alloc,
        "a counted once; b's other link is outside"
    );
    assert_eq!(f.items, 2, "two distinct files");
    // Card totals equal the sum of their items.
    for c in &out.result.categories {
        let sum: u64 = out
            .result
            .findings
            .iter()
            .filter(|x| {
                x.category == c.info.id && x.tier.is_selectable() && x.disposal.is_actionable()
            })
            .map(|x| x.bytes)
            .sum();
        assert_eq!(c.total_bytes(), sum, "{:?}", c.info.id);
    }
}

/// 8. Every disposal is logged before it runs, with path, bytes, rule, disposal, time, result.
#[test]
fn inv8_every_disposal_logged_first() {
    let h = FakeHome::new();
    let p = h.file("Downloads/stale.crdownload", 5000, 10.0);
    let out = h.scan();
    let f = find(
        &out.result.findings,
        "downloads.partial",
        "stale.crdownload",
    )
    .unwrap()
    .clone();
    let report = h.clean(std::slice::from_ref(&f), out.result.started);
    assert_eq!(report.ok, 1);
    assert!(!p.exists());
    let recs = read_log(&h.env.oplog_path());
    let items: Vec<&LogRecord> = recs
        .iter()
        .filter(|r| matches!(r, LogRecord::Item { .. }))
        .collect();
    assert_eq!(items.len(), 2, "pending + result");
    match (items[0], items[1]) {
        (
            LogRecord::Item {
                path,
                bytes,
                rule,
                disposal,
                result: ItemStatus::Pending,
                ts: t0,
                ..
            },
            LogRecord::Item {
                result: ItemStatus::Ok,
                ts: t1,
                freed,
                ..
            },
        ) => {
            assert_eq!(path, &p);
            assert_eq!(*bytes, f.bytes);
            assert_eq!(rule, "downloads.partial");
            assert_eq!(disposal, "delete");
            assert!(t0 <= t1);
            assert_eq!(*freed, f.bytes);
        }
        other => panic!("unexpected log order: {other:?}"),
    }
    assert!(matches!(recs.first(), Some(LogRecord::OpBegin { .. })));
    assert!(matches!(recs.last(), Some(LogRecord::OpEnd { .. })));
}

/// 9. Tests never touch real user folders: the FakeHome is self-contained.
#[test]
fn inv9_fake_home_is_isolated() {
    let root = tempfile::tempdir().unwrap();
    let env = Env::fake(root.path());
    for p in [
        &env.home,
        &env.appdata,
        &env.local_appdata,
        &env.temp,
        &env.downloads,
        &env.documents,
        &env.desktop,
    ] {
        assert!(
            p.starts_with(root.path()),
            "{} escapes the fake root",
            p.display()
        );
    }
    assert!(env.oplog_path().starts_with(root.path()));
    assert!(env.is_fake);
    // Real variables (e.g. PATH) are invisible to rule expansion.
    assert!(env.var("PATH").is_none());
    assert_eq!(
        env.var("USERPROFILE").map(PathBuf::from),
        Some(env.home.clone())
    );
}

/// 10. No network calls from pare-core: no HTTP client in its dependencies.
#[test]
fn inv10_no_network_dependencies() {
    let manifest =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    for banned in [
        "reqwest",
        "hyper",
        "ureq",
        "curl",
        "isahc",
        "surf",
        "attohttpc",
        "tokio-tungstenite",
    ] {
        assert!(
            !manifest.lines().any(|l| l.trim_start().starts_with(banned)),
            "pare-core must not depend on {banned}"
        );
    }
}
