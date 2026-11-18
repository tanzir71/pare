//! M2 acceptance: a large folder scan finishes and progress can be sampled while it runs
//! (the UI samples every 250 ms; sampling must never block on the walker).

mod common;

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use common::*;
use pare_core::scan::progress::{Phase, Progress};
use pare_core::scanner::{ScanRequest, ScanRoot, scan};

fn build_tree(root: &Path, files: usize) {
    let per_dir = 100;
    for d in 0..files.div_ceil(per_dir) {
        let dir = root.join(format!("d{:03}", d / 100)).join(format!("e{d:05}"));
        fs::create_dir_all(&dir).unwrap();
        for f in 0..per_dir.min(files - d * per_dir) {
            fs::write(dir.join(format!("f{f:03}.txt")), b"pare").unwrap();
        }
    }
}

fn run(files: usize) -> (Duration, Vec<Duration>, u64) {
    let h = FakeHome::new();
    let root = h.dir("Documents/fixture");
    build_tree(&root, files);
    let req = ScanRequest {
        roots: vec![ScanRoot {
            path: root.clone(),
            label: "fixture".into(),
        }],
        ..ScanRequest::default()
    };
    let cfg = h.config();
    let progress = Progress::default();
    let cancel = AtomicBool::new(false);
    let mut sample_costs = Vec::new();
    let mut last_files = 0u64;
    let start = Instant::now();
    let out = std::thread::scope(|s| {
        let job = s.spawn(|| scan(&req, &cfg, &h.env, &progress, &cancel, &|_| {}));
        while !job.is_finished() {
            let t = Instant::now();
            let snap = progress.snapshot();
            sample_costs.push(t.elapsed());
            assert!(snap.files >= last_files, "progress went backwards");
            last_files = snap.files;
            std::thread::sleep(Duration::from_millis(50));
        }
        job.join().unwrap()
    });
    let elapsed = start.elapsed();
    assert_eq!(progress.phase(), Phase::Done);
    assert_eq!(out.result.stats.files, files as u64);
    (elapsed, sample_costs, out.result.stats.files)
}

#[test]
fn progress_is_sampled_without_blocking() {
    let (_, costs, files) = run(5_000);
    assert_eq!(files, 5_000);
    let worst = costs.iter().max().copied().unwrap_or_default();
    assert!(worst < Duration::from_millis(250), "a progress sample took {worst:?}");
}

/// Run with `cargo test -p pare-core --release --test scan_perf -- --ignored --nocapture`.
#[test]
#[ignore]
fn scan_200k_files() {
    let (elapsed, costs, files) = run(200_000);
    let worst = costs.iter().max().copied().unwrap_or_default();
    println!(
        "scanned {files} files in {:.2} s; {} progress samples, worst sample {:?}",
        elapsed.as_secs_f64(),
        costs.len(),
        worst
    );
    assert!(worst < Duration::from_millis(250));
}
