//! The desktop UI (apps/desktop/src/lib/types.ts) reads these JSON shapes. Pin them so a Rust
//! rename can't silently break the screens.

use pare_core::detectors::Thresholds;
use pare_core::scanner::{DetectorState, DetectorStatus, ScanEvent};
use pare_core::settings::Settings;
use pare_core::{CategoryId, ComesBack, Dates, Disposal, Finding, ProtectReason, Tier};
use serde_json::{Value, json};

fn finding() -> Finding {
    Finding {
        id: "f1".into(),
        rule_id: "claude-code.debug-logs".into(),
        category: CategoryId::AiTools,
        source: "Claude Code".into(),
        source_path: Some("C:\\Users\\ana\\.claude".into()),
        title: "Debug logs".into(),
        paths: vec!["C:\\Users\\ana\\.claude\\debug".into()],
        bytes: 10,
        logical_bytes: 9,
        items: 2,
        tier: Tier::Protected {
            reason: ProtectReason::InUse,
            detail: Some("Open in claude.exe".into()),
        },
        disposal: Disposal::EmptyRecycleBin {
            drive: "C:\\".into(),
        },
        comes_back: ComesBack::OnDemand,
        why: "w".into(),
        if_removed: "i".into(),
        dates: Dates {
            created_min: Some(1),
            modified_max: Some(2),
            accessed_max: None,
        },
        created_by: Some("Claude Code".into()),
        badge: None,
        note: None,
        git: None,
        facts: vec![("Rule".into(), "x".into())],
    }
}

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

#[test]
fn finding_shape() {
    let v = serde_json::to_value(finding()).unwrap();
    assert_eq!(
        keys(&v),
        [
            "badge",
            "bytes",
            "category",
            "comesBack",
            "createdBy",
            "dates",
            "disposal",
            "facts",
            "git",
            "id",
            "ifRemoved",
            "items",
            "logicalBytes",
            "note",
            "paths",
            "ruleId",
            "source",
            "sourcePath",
            "tier",
            "title",
            "why"
        ]
    );
    assert_eq!(
        v["tier"],
        json!({"kind": "protected", "reason": "in-use", "detail": "Open in claude.exe"})
    );
    assert_eq!(
        v["disposal"],
        json!({"kind": "empty-recycle-bin", "drive": "C:\\"})
    );
    assert_eq!(v["comesBack"], "on-demand");
    assert_eq!(v["category"], "ai-tools");
    assert_eq!(
        v["dates"],
        json!({"createdMin": 1, "modifiedMax": 2, "accessedMax": null})
    );
    assert_eq!(v["facts"], json!([["Rule", "x"]]));
    assert_eq!(
        serde_json::to_value(Tier::Safe).unwrap(),
        json!({"kind": "safe"})
    );
    assert_eq!(
        serde_json::to_value(Disposal::Command {
            program: "pnpm".into(),
            args: vec!["store".into()]
        })
        .unwrap(),
        json!({"kind": "command", "program": "pnpm", "args": ["store"]})
    );
}

#[test]
fn scan_event_and_category_shapes() {
    let ev = ScanEvent::Detector(DetectorStatus {
        id: "junk".into(),
        label: "Junk".into(),
        state: DetectorState::Running,
        bytes: 5,
    });
    assert_eq!(
        serde_json::to_value(ev).unwrap(),
        json!({"type": "detector", "id": "junk", "label": "Junk", "state": "running", "bytes": 5})
    );
    let cats = pare_core::scanner::summarize(&[{
        let mut f = finding();
        f.tier = Tier::Safe;
        f.disposal = Disposal::Delete;
        f
    }]);
    let v = serde_json::to_value(ScanEvent::Category(cats[0].clone())).unwrap();
    assert_eq!(v["type"], "category");
    for k in [
        "id",
        "title",
        "description",
        "icon",
        "hue",
        "safeBytes",
        "reviewBytes",
        "protectedBytes",
        "items",
        "findings",
        "sources",
        "protectedFindings",
        "freesNowBytes",
        "recycleBytes",
    ] {
        assert!(v.get(k).is_some(), "CategorySummary lacks {k}: {v}");
    }
    assert_eq!(v["id"], "ai-tools");
    assert_eq!(v["hue"], "violet");
}

#[test]
fn settings_shape() {
    let v = serde_json::to_value(Settings::default()).unwrap();
    assert_eq!(
        keys(&v),
        [
            "cachesToRecycleBin",
            "checkForUpdates",
            "customRoots",
            "exclusions",
            "hiddenCards",
            "language",
            "monitor",
            "onboardingDone",
            "projectRoots",
            "purgePareItemsAfterDays",
            "schedules",
            "theme",
            "thresholds"
        ]
    );
    assert_eq!(v["theme"], "system");
    assert_eq!(
        keys(&v["monitor"]),
        ["alertGb", "checkHours", "enabled", "tray"]
    );
    assert_eq!(
        keys(&serde_json::to_value(Thresholds::default()).unwrap()),
        [
            "duplicateMinMb",
            "idleDays",
            "installerDays",
            "largeFileDays",
            "largeFileMb",
            "oldDownloadDays",
            "partialDays",
            "recentHours",
            "screenshotDays"
        ]
    );
}
