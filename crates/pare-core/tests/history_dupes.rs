//! M5: duplicates never suggest the last copy; restore puts files back and reports collisions.

mod common;

use std::fs;

use common::*;
use pare_core::{CategoryId, Disposal, Tier};

fn content(seed: u8, len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

fn write_bytes(p: &std::path::Path, bytes: &[u8], age: f64) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
    set_age(p, age);
}

/// M5 acceptance: 3-way duplicates and hard links — the last remaining copy is never suggested.
#[test]
fn duplicates_keep_one_copy_and_ignore_hard_links() {
    let h = FakeHome::new();
    let video = content(1, 3_000_000);
    // Three real copies; the Documents one is preferred.
    let doc = h.home().join("Documents/trip.mov");
    let desk = h.home().join("Desktop/trip.mov");
    let dl = h.home().join("Downloads/trip (1).mov");
    write_bytes(&doc, &video, 40.0);
    write_bytes(&desk, &video, 30.0);
    write_bytes(&dl, &video, 20.0);
    // A file whose only "duplicate" is a hard link to itself: one copy, never suggested.
    let solo = h.home().join("Documents/solo.bin");
    write_bytes(&solo, &content(2, 2_000_000), 40.0);
    fs::hard_link(&solo, h.home().join("Desktop/solo-link.bin")).unwrap();
    // Same size, different bytes at the end: not a duplicate.
    let mut almost = video.clone();
    *almost.last_mut().unwrap() ^= 0xFF;
    write_bytes(&h.home().join("Documents/trip-edited.mov"), &almost, 40.0);
    // Below the size threshold.
    write_bytes(&h.home().join("Documents/a.txt"), b"same", 40.0);
    write_bytes(&h.home().join("Desktop/a.txt"), b"same", 40.0);
    age_tree(h.home(), 40.0);

    let out = h.scan();
    let dupes: Vec<_> = out
        .result
        .findings
        .iter()
        .filter(|f| f.category == CategoryId::Duplicates)
        .collect();
    assert_eq!(dupes.len(), 1, "{dupes:#?}");
    let f = dupes[0];
    assert_eq!(f.tier, Tier::Review);
    assert_eq!(f.disposal, Disposal::Recycle);
    assert_eq!(f.paths.len(), 2, "two of three copies");
    assert!(
        !f.paths.iter().any(|p| p == &doc),
        "the Documents copy is kept"
    );
    assert!(
        f.facts
            .iter()
            .any(|(k, v)| k == "Kept copy" && v.ends_with("trip.mov"))
    );
    // Even selecting everything leaves one copy on disk.
    let report = h.clean(std::slice::from_ref(f), out.result.started);
    purge_recycled_under_any(h.dir.path());
    assert!(doc.exists());
    assert_eq!(report.failed, 0, "{:?}", report.results);
    // Re-scan: nothing left to suggest.
    let again = h.scan();
    assert!(
        !again
            .result
            .findings
            .iter()
            .any(|f| f.category == CategoryId::Duplicates)
    );
}

/// M5 acceptance: restore puts a file back at its original path; collisions are reported, not
/// overwritten.
#[cfg(windows)]
#[test]
fn restore_and_collisions() {
    use pare_core::dispose::recycle;
    let h = FakeHome::new();
    let a = h.file("Downloads/report-old.pdf", 2000, 400.0);
    let b = h.file("Downloads/photo-old.jpg", 2000, 400.0);
    let out = h.scan();
    let sel: Vec<_> = out
        .result
        .findings
        .iter()
        .filter(|f| f.rule_id == "downloads.old")
        .cloned()
        .collect();
    assert_eq!(sel.len(), 2);
    let report = h.clean(&sel, out.result.started);
    assert_eq!(report.ok, 2);
    assert!(!a.exists() && !b.exists());

    // Something new now lives where b was.
    fs::write(&b, b"a new file with the same name").unwrap();
    let log = h.oplog();
    let r = recycle::restore(&log, &report.op_id, None);
    assert_eq!(r.restored, 1, "{r:?}");
    assert!(a.exists(), "restored to its original path");
    assert_eq!(r.collisions, vec![b.clone()]);
    assert_eq!(
        fs::read(&b).unwrap(),
        b"a new file with the same name",
        "never overwritten"
    );

    // History reflects it; emptying Pare's items removes only what Pare moved.
    let detail = pare_core::history::get(&h.env.oplog_path(), &report.op_id).unwrap();
    assert!(detail.items.iter().any(|i| i.restored && i.path == a));
    let p = recycle::purge_pare_items(&log, Some(&report.op_id), None);
    assert_eq!(p.items, 1, "{p:?}");
    purge_recycled_under_any(h.dir.path());
}

fn purge_recycled_under_any(under: &std::path::Path) {
    #[cfg(windows)]
    purge_recycled_under(under);
    #[cfg(not(windows))]
    let _ = under;
}
