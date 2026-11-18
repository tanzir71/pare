//! Disposal executor: Recycle Bin, direct delete, partial failures, re-validation, history.

mod common;

use std::fs;

use common::*;
use pare_core::dispose::executor::FailReason;
#[cfg(windows)]
use pare_core::dispose::oplog::ItemStatus;
#[cfg(windows)]
use pare_core::history;

#[test]
#[ignore]
fn helper_hold_file() {
    hold_file_main();
}

#[test]
fn changed_since_scan_is_skipped() {
    let h = FakeHome::new();
    let p = h.file("Downloads/old.part", 1000, 5.0);
    let out = h.scan();
    let f = find(&out.result.findings, "downloads.partial", "old.part")
        .unwrap()
        .clone();
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(&p, b"changed after the scan").unwrap();
    let report = h.clean(&[f], out.result.started);
    assert_eq!(report.skipped, 1);
    assert!(matches!(
        report.results[0].reason,
        Some(FailReason::ChangedSinceScan) | Some(FailReason::Protected)
    ));
    assert!(p.exists());
}

#[test]
fn already_gone_is_reported_not_failed() {
    let h = FakeHome::new();
    let p = h.file("Downloads/gone.part", 1000, 5.0);
    let out = h.scan();
    let f = find(&out.result.findings, "downloads.partial", "gone.part")
        .unwrap()
        .clone();
    fs::remove_file(&p).unwrap();
    let report = h.clean(&[f], out.result.started);
    assert_eq!(report.results[0].reason, Some(FailReason::NotFound));
    assert_eq!(report.failed, 0);
    assert!(report.summary.is_none());
}

#[test]
fn delete_removes_read_only_files_and_nested_folders() {
    let h = FakeHome::new();
    h.dir("Documents/empty/a/b/c");
    age_tree(&h.home().join("Documents/empty"), 3.0);
    let p = h.file("Downloads/ro.crdownload", 1000, 5.0);
    let mut perm = fs::metadata(&p).unwrap().permissions();
    perm.set_readonly(true);
    fs::set_permissions(&p, perm).unwrap();
    let out = h.scan();
    let sel: Vec<_> = out
        .result
        .findings
        .iter()
        .filter(|f| f.is_selectable())
        .cloned()
        .collect();
    assert_eq!(sel.len(), 2, "{sel:?}");
    let report = h.clean(&sel, out.result.started);
    assert_eq!(report.ok, 2, "{:?}", report.results);
    assert!(!p.exists());
    assert!(!h.home().join("Documents/empty").exists());
    assert!(h.home().join("Documents").exists());
    assert_eq!(report.freed_now, sel.iter().map(|f| f.bytes).sum::<u64>());
    assert_eq!(report.recycled, 0);
}

#[cfg(windows)]
#[test]
fn partial_failure_names_the_holding_process() {
    let h = FakeHome::new();
    let a = h.file("Downloads/a.crdownload", 1000, 5.0);
    let b = h.file("Downloads/b.crdownload", 1000, 5.0);
    let out = h.scan();
    let sel: Vec<_> = out
        .result
        .findings
        .iter()
        .filter(|f| f.is_selectable())
        .cloned()
        .collect();
    assert_eq!(sel.len(), 2);
    let _holder = Holder::spawn(&b);
    let report = h.clean(&sel, out.result.started);
    assert!(!a.exists(), "the free item is removed");
    assert!(b.exists(), "the held item survives");
    let rb = report
        .results
        .iter()
        .find(|r| r.title == "b.crdownload")
        .unwrap();
    assert_eq!(rb.reason, Some(FailReason::InUse));
    assert!(
        rb.error.as_deref().unwrap().starts_with("Open in"),
        "{:?}",
        rb.error
    );
    assert_eq!(
        report.summary.as_deref(),
        Some("1 item couldn't be removed because it's in use")
    );
}

/// M2 acceptance: moving 1,000 files to the Recycle Bin succeeds and appears in History; a
/// junction inside a selected folder is moved as a link and its target survives.
#[cfg(windows)]
#[test]
fn recycle_1000_files_then_history() {
    let h = FakeHome::new();
    for i in 0..1000 {
        h.file(&format!("Downloads/file-{i:04}.txt"), 100, 400.0);
    }
    let outside = h.dir.path().join("outside-target");
    write_file(&outside.join("keep.bin"), 4096, 400.0);
    let folder = h.dir("Downloads/zz-folder-with-junction");
    write_file(&folder.join("old.txt"), 100, 400.0);
    junction::create(&outside, folder.join("link")).unwrap();
    age_tree(&folder, 400.0);

    let out = h.scan();
    let sel: Vec<_> = out
        .result
        .findings
        .iter()
        .filter(|f| f.rule_id == "downloads.old" && f.is_selectable())
        .cloned()
        .collect();
    assert_eq!(sel.len(), 1001);
    let report = h.clean(&sel, out.result.started);
    purge_recycled_under(h.dir.path()); // leave the real Recycle Bin as we found it
    assert_eq!(
        report.ok,
        1001,
        "{:?}",
        report.results.iter().find(|r| r.status != ItemStatus::Ok)
    );
    assert_eq!(
        report.freed_now, 0,
        "recycling frees nothing until the bin is emptied"
    );
    assert_eq!(report.recycled, sel.iter().map(|f| f.bytes).sum::<u64>());
    assert!(!folder.exists());
    assert!(
        outside.join("keep.bin").exists(),
        "junction target must survive"
    );

    let ops = history::list(&h.env.oplog_path());
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].items, 1001);
    assert_eq!(ops[0].ok, 1001);
    assert_eq!(ops[0].disposals.get("recycle"), Some(&1001));
    let detail = history::get(&h.env.oplog_path(), &ops[0].id).unwrap();
    assert!(
        detail
            .items
            .iter()
            .all(|i| i.result == ItemStatus::Ok && i.recycled > 0)
    );
}
