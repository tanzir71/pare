//! M2 quick detectors against FakeHome fixtures.

mod common;

use common::*;
use pare_core::{CategoryId, ComesBack, Disposal, Tier};

#[test]
fn installers_older_than_7_days_are_review() {
    let h = FakeHome::new();
    h.file("Downloads/old-setup.exe", 1000, 8.0);
    h.file("Downloads/app.msi", 1000, 30.0);
    h.file("Downloads/fresh-setup.exe", 1000, 3.0);
    h.file("Downloads/nested/tool.msix", 1000, 30.0);
    let out = h.scan();
    let f = find(
        &out.result.findings,
        "downloads.installers",
        "old-setup.exe",
    )
    .unwrap();
    assert_eq!(f.tier, Tier::Review);
    assert_eq!(f.disposal, Disposal::Recycle);
    assert_eq!(f.category, CategoryId::Downloads);
    assert!(find(&out.result.findings, "downloads.installers", "app.msi").is_some());
    assert!(find(&out.result.findings, "downloads.installers", "tool.msix").is_some());
    assert!(
        find(
            &out.result.findings,
            "downloads.installers",
            "fresh-setup.exe"
        )
        .is_none()
    );
    assert!(!f.why.is_empty() && !f.if_removed.is_empty());
}

#[test]
fn archives_with_an_extracted_sibling_folder() {
    let h = FakeHome::new();
    h.file("Downloads/photos.zip", 1000, 2.0);
    h.file("Downloads/Photos/a.jpg", 1000, 2.0);
    h.file("Downloads/lonely.7z", 1000, 2.0);
    let out = h.scan();
    let f = find(
        &out.result.findings,
        "downloads.extracted-archives",
        "photos.zip",
    )
    .expect("extracted archive");
    assert_eq!(f.tier, Tier::Review);
    assert!(f.why.contains("Photos"));
    assert!(
        find(
            &out.result.findings,
            "downloads.extracted-archives",
            "lonely.7z"
        )
        .is_none()
    );
}

#[test]
fn partial_downloads_older_than_a_day_are_safe_delete() {
    let h = FakeHome::new();
    h.file("Downloads/movie.mkv.crdownload", 5000, 2.0);
    h.file("Downloads/iso.part", 5000, 3.0);
    h.file("Downloads/active.partial", 5000, 0.1);
    let out = h.scan();
    let f = find(
        &out.result.findings,
        "downloads.partial",
        "movie.mkv.crdownload",
    )
    .unwrap();
    assert_eq!(f.tier, Tier::Safe);
    assert_eq!(f.disposal, Disposal::Delete);
    assert_eq!(f.comes_back, ComesBack::OnDemand);
    assert!(find(&out.result.findings, "downloads.partial", "iso.part").is_some());
    assert!(find(&out.result.findings, "downloads.partial", "active.partial").is_none());
}

#[test]
fn old_downloads_and_folders() {
    let h = FakeHome::new();
    h.file("Downloads/report-2024.pdf", 1000, 200.0);
    h.file("Downloads/recent.pdf", 1000, 20.0);
    h.file("Downloads/old-stuff/a.txt", 10, 300.0);
    h.file("Downloads/old-stuff/b.txt", 10, 250.0);
    age_tree(&h.home().join("Downloads/old-stuff"), 250.0);
    h.file("Downloads/mixed/new.txt", 10, 5.0);
    h.file("Downloads/mixed/old.txt", 10, 500.0);
    h.file("Downloads/desktop.ini", 10, 900.0);
    let out = h.scan();
    assert!(find(&out.result.findings, "downloads.old", "report-2024.pdf").is_some());
    assert!(find(&out.result.findings, "downloads.old", "old-stuff").is_some());
    assert!(find(&out.result.findings, "downloads.old", "recent.pdf").is_none());
    assert!(
        find(&out.result.findings, "downloads.old", "mixed").is_none(),
        "folder with a recent file"
    );
    assert!(find(&out.result.findings, "downloads.old", "desktop.ini").is_none());
}

#[test]
fn each_item_belongs_to_one_finding() {
    let h = FakeHome::new();
    h.file("Downloads/ancient-setup.exe", 3_000_000, 400.0);
    let mut cfg = h.config();
    cfg.thresholds.large_file_mb = 1;
    let out = h.scan_with(&[h.home()], &cfg);
    let hits: Vec<_> = out
        .result
        .findings
        .iter()
        .filter(|f| f.title == "ancient-setup.exe")
        .collect();
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].rule_id, "downloads.installers");
}

#[test]
fn large_files_respect_threshold_and_age() {
    let h = FakeHome::new();
    h.file("Documents/video.mp4", 2_500_000, 120.0);
    h.file("Documents/recent-video.mp4", 2_500_000, 10.0);
    h.file("Documents/small.txt", 1000, 400.0);
    h.file("Documents/vm.vhdx", 2_500_000, 400.0);
    h.file("AppData/Local/SomeApp/cache.bin", 2_500_000, 400.0);
    let mut cfg = h.config();
    cfg.thresholds.large_file_mb = 2;
    let out = h.scan_with(&[h.home()], &cfg);
    let f = find(&out.result.findings, "quick.large-files", "video.mp4").expect("large file");
    assert_eq!(f.tier, Tier::Review);
    assert_eq!(f.disposal, Disposal::Recycle);
    assert_eq!(f.category, CategoryId::LargeFiles);
    let titles: Vec<&str> = out
        .result
        .findings
        .iter()
        .filter(|f| f.rule_id == "quick.large-files")
        .map(|f| f.title.as_str())
        .collect();
    assert_eq!(
        titles,
        vec!["video.mp4"],
        "only the old, big, non-VM, non-AppData file"
    );
}

#[test]
fn empty_folders_top_most_only_and_skips() {
    let h = FakeHome::new();
    h.dir("Documents/empty-a/inner/deeper");
    h.dir("Documents/has-file/empty-child");
    h.file("Documents/has-file/x.txt", 5, 10.0);
    h.dir("Documents/.hidden-empty");
    h.dir("Documents/repo/.git");
    h.dir("Documents/repo/empty-in-repo");
    h.dir("Pictures/Screenshots");
    let out = h.scan();
    let mut titles: Vec<&str> = out
        .result
        .findings
        .iter()
        .filter(|f| f.rule_id == "quick.empty-folders")
        .map(|f| f.title.as_str())
        .collect();
    titles.sort();
    assert_eq!(titles, vec!["empty-a", "empty-child"]);
    let f = find(&out.result.findings, "quick.empty-folders", "empty-a").unwrap();
    // Just created → protected by recency; becomes Safe once older than 24 h.
    assert!(f.tier.is_protected());
    age_tree(&h.home().join("Documents/empty-a"), 3.0);
    let out = h.scan();
    let f = find(&out.result.findings, "quick.empty-folders", "empty-a").unwrap();
    assert_eq!(f.tier, Tier::Safe);
    assert_eq!(f.disposal, Disposal::Delete);
}

#[test]
fn card_totals_equal_sum_of_items() {
    let h = FakeHome::new();
    for i in 0..20 {
        h.file(&format!("Downloads/setup-{i}.exe"), 10_000 + i * 1000, 30.0);
        h.file(&format!("Downloads/part-{i}.crdownload"), 5_000, 3.0);
    }
    h.file("Documents/big1.iso", 3_000_000, 300.0);
    h.file("Documents/big2.iso", 4_000_000, 300.0);
    let mut cfg = h.config();
    cfg.thresholds.large_file_mb = 1;
    let out = h.scan_with(&[h.home()], &cfg);
    assert!(out.result.categories.len() >= 2);
    for c in &out.result.categories {
        let items: Vec<_> = out
            .result
            .findings
            .iter()
            .filter(|f| f.category == c.info.id && f.tier.is_selectable())
            .collect();
        assert_eq!(c.total_bytes(), items.iter().map(|f| f.bytes).sum::<u64>());
        assert_eq!(c.items, items.iter().map(|f| f.items).sum::<u64>());
        assert_eq!(
            c.safe_bytes + c.review_bytes,
            c.frees_now_bytes + c.recycle_bytes
        );
    }
    // Sorted by space you get back.
    let totals: Vec<u64> = out
        .result
        .categories
        .iter()
        .map(|c| c.total_bytes())
        .collect();
    assert!(totals.windows(2).all(|w| w[0] >= w[1]));
}
