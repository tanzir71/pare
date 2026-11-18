//! M8: growth monitor, opt-in scheduled cleanups and guide actions. Everything happens in a
//! FakeHome (invariant 9); nothing here runs Docker, WSL, diskpart or DISM.

mod common;

use std::sync::Arc;

use common::{DAY, FakeHome, write_file};
use pare_core::dispose::guide;
use pare_core::dispose::oplog::{LogRecord, read_log};
use pare_core::monitor::{self, MonitorState, growth, schedule};
use pare_core::platform::now_ms;
use pare_core::rules::RuleSet;
use pare_core::scanner::{ScanConfig, scan_rules};
use pare_core::settings::{RuleSchedule, Settings};
use pare_core::types::MS_PER_DAY;
use pare_core::{Disposal, Tier};

fn cfg(h: &FakeHome) -> ScanConfig {
    let mut c = h.config();
    c.rules = Some(Arc::new(RuleSet::builtin()));
    c
}

fn op_titles(h: &FakeHome) -> Vec<String> {
    read_log(&h.env.oplog_path())
        .into_iter()
        .filter_map(|r| match r {
            LogRecord::OpBegin { title, .. } => Some(title),
            _ => None,
        })
        .collect()
}

/// Invariant 1: without an opt-in schedule, the monitor never removes anything.
#[test]
fn nothing_runs_without_a_schedule() {
    let h = FakeHome::new();
    let old = h.env.temp.join("old.tmp");
    write_file(&old, 50_000, 3.0);
    let mut s = Settings::default();
    s.monitor.enabled = true;
    let r = monitor::tick(&h.env, &cfg(&h), &s, now_ms());
    assert!(r.scheduled.is_none() && r.ran.is_empty());
    assert!(old.exists());
    assert!(op_titles(&h).is_empty(), "no operation logged");
}

#[test]
fn scheduled_cleanup_removes_only_safe_old_unexcluded_items() {
    let h = FakeHome::new();
    let old = h.env.temp.join("old.tmp");
    let fresh = h.env.temp.join("fresh.tmp");
    write_file(&old, 50_000, 3.0);
    write_file(&fresh, 50_000, 0.01);
    // A Review rule's files, scheduled by hand-editing settings.json.
    let review = h.file(".codex/archived_sessions/rollout-1.jsonl", 50_000, 60.0);

    let mut s = Settings {
        schedules: vec![
            RuleSchedule {
                rule: "windows.user-temp".into(),
                every_days: 7,
            },
            RuleSchedule {
                rule: "codex.archived-sessions".into(),
                every_days: 1,
            },
        ],
        ..Default::default()
    };
    // "Always keep" wins over a schedule.
    s.exclusions = vec![h.env.temp.clone()];
    let t = now_ms();
    let r = monitor::tick(&h.env, &cfg(&h), &s, t);
    assert!(r.scheduled.is_none(), "{:?}", r.scheduled);
    assert!(old.exists());
    s.exclusions.clear();

    // A week later the schedule is due again.
    let now = t + 8 * MS_PER_DAY;
    let r = monitor::tick(&h.env, &cfg(&h), &s, now);
    let report = r.scheduled.expect("a scheduled run");
    assert!(report.freed_now > 0, "{report:#?}");
    assert!(!old.exists(), "old temp file cleaned");
    assert!(fresh.exists(), "modified in the last 24 h");
    assert!(review.exists(), "Review rules never run on a schedule");
    let titles = op_titles(&h);
    assert_eq!(titles.len(), 1);
    assert!(titles[0].starts_with("Scheduled cleanup"), "{titles:?}");

    // Not due again until the interval passes.
    write_file(&old, 50_000, 3.0);
    let r = monitor::tick(&h.env, &cfg(&h), &s, now + MS_PER_DAY);
    assert!(r.scheduled.is_none());
    assert!(old.exists());
    let st = MonitorState::load(&h.env);
    assert_eq!(st.last_runs.get("windows.user-temp"), Some(&now));
}

#[test]
fn only_builtin_verified_safe_rules_are_schedulable() {
    let set = RuleSet::builtin();
    let ids: Vec<String> = schedule::schedulable_rules(&set)
        .into_iter()
        .map(|r| r.id)
        .collect();
    assert!(ids.contains(&"windows.user-temp".to_string()));
    assert!(ids.contains(&"npm.cache".to_string()));
    for (_, r) in set.all_rules() {
        if ids.contains(&r.id) {
            assert!(r.verified && !r.admin && r.detector.is_empty(), "{}", r.id);
            assert_eq!(r.tier, pare_core::rules::TierDecl::Safe, "{}", r.id);
        }
    }
    for bad in [
        "codex.archived-sessions",          // review
        "docker.vhdx-guide",                // guide
        "claude-code.old-versions",         // not verified
        "windows.wer",                      // admin
        "claude-code.orphaned-transcripts", // detector
        "nope.nope",
    ] {
        assert!(schedule::check(&set, bad).is_err(), "{bad}");
        assert!(!ids.contains(&bad.to_string()), "{bad}");
    }
}

#[test]
fn growth_is_recorded_and_alerts_after_a_day() {
    let h = FakeHome::new();
    let c = cfg(&h);
    let t0 = now_ms();
    write_file(&h.env.temp.join("a.tmp"), 1_000_000, 3.0);
    let s0 = monitor::measure_growth(&c, &h.env, t0).unwrap();
    assert!(s0.rules.contains_key("windows.user-temp"));
    for i in 0..4 {
        write_file(&h.env.temp.join(format!("b{i}.tmp")), 1_000_000, 3.0);
    }
    monitor::measure_growth(&c, &h.env, t0 + 6 * MS_PER_DAY).unwrap();
    let alerts = growth::alerts(&growth::load(&h.env), 3_000_000);
    assert_eq!(alerts.len(), 1, "{alerts:?}");
    assert_eq!(alerts[0].rule_id, "windows.user-temp");
    assert!(alerts[0].delta() >= 4_000_000);
    assert_eq!(alerts[0].since, t0);
}

#[test]
fn monitor_checks_on_its_interval() {
    let h = FakeHome::new();
    write_file(&h.env.temp.join("a.tmp"), 10_000, 3.0);
    let mut s = Settings::default();
    s.monitor.enabled = true;
    s.monitor.check_hours = 6;
    let now = now_ms();
    monitor::tick(&h.env, &cfg(&h), &s, now);
    monitor::tick(&h.env, &cfg(&h), &s, now + 3_600_000);
    assert_eq!(growth::load(&h.env).len(), 1, "second check not due yet");
    monitor::tick(&h.env, &cfg(&h), &s, now + 7 * 3_600_000);
    assert_eq!(growth::load(&h.env).len(), 2);
    // Turned off: no checks at all.
    s.monitor.enabled = false;
    monitor::tick(&h.env, &cfg(&h), &s, now + 20 * 3_600_000);
    assert_eq!(growth::load(&h.env).len(), 2);
}

#[test]
fn guide_actions_come_from_rules_and_admin_steps_wait_for_elevation() {
    let h = FakeHome::new();
    let disk = h
        .env
        .local_appdata
        .join("Docker")
        .join("wsl")
        .join("disk")
        .join("docker_data.vhdx");
    write_file(&disk, 200_000, 2.0);
    std::fs::create_dir_all(h.env.system_root.clone().unwrap().join("WinSxS")).unwrap();
    let mut c = cfg(&h);
    c.check_in_use = false;
    let findings = scan_rules(&c, &h.env, |_, r| {
        r.id == "docker.vhdx-guide" || r.id == "windows.component-store-guide"
    });

    let docker = findings
        .iter()
        .find(|f| f.rule_id == "docker.vhdx-guide")
        .expect("docker guide");
    assert!(
        !docker.is_selectable(),
        "guides are never selected for cleanup"
    );
    let Disposal::Guide { steps, actions } = &docker.disposal else {
        panic!("{:?}", docker.disposal)
    };
    assert_eq!(steps.len(), 3);
    assert_eq!(actions.len(), 3);
    assert!(actions.iter().all(|a| a.step < steps.len()));
    let compact = actions
        .iter()
        .position(|a| a.run == ["pare:compact-vhd"])
        .unwrap();
    assert!(actions[compact].admin);
    let log = h.oplog();
    let e = guide::run_action(docker, compact, &h.env, &log, false).unwrap_err();
    assert!(e.contains("administrator"), "{e}");
    assert!(disk.exists());
    assert!(op_titles(&h).is_empty());

    let dism = findings
        .iter()
        .find(|f| f.rule_id == "windows.component-store-guide")
        .expect("component store guide");
    assert!(
        dism.paths.is_empty(),
        "nothing in WinSxS is ever a path Pare acts on"
    );
    assert_eq!(dism.bytes, 0, "no size claim Pare can't back up");
    assert!(!matches!(dism.tier, Tier::Safe));
    let Disposal::Guide { actions, .. } = &dism.disposal else {
        panic!()
    };
    assert_eq!(
        actions[0].run,
        [
            "dism.exe",
            "/Online",
            "/Cleanup-Image",
            "/StartComponentCleanup"
        ]
    );
    assert!(actions[0].admin);
    assert!(
        !actions[0]
            .run
            .iter()
            .any(|a| a.eq_ignore_ascii_case("/ResetBase"))
    );
    let _ = DAY;
}

#[test]
fn concurrent_passes_dont_double_run() {
    let h = FakeHome::new();
    let old = h.env.temp.join("old.tmp");
    write_file(&old, 50_000, 3.0);
    let s = Settings {
        schedules: vec![RuleSchedule {
            rule: "windows.user-temp".into(),
            every_days: 1,
        }],
        ..Default::default()
    };
    // Another Pare is mid-pass.
    let lock = h.env.pare_data_dir().join("monitor.lock");
    std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
    std::fs::write(&lock, b"").unwrap();
    let r = monitor::tick(&h.env, &cfg(&h), &s, now_ms());
    assert!(r.scheduled.is_none() && old.exists());
    std::fs::remove_file(&lock).unwrap();
    let r = monitor::tick(&h.env, &cfg(&h), &s, now_ms());
    assert!(r.scheduled.is_some() && !old.exists());
    assert!(!lock.exists(), "lock released");
}
