//! M6 fast mode against a real NTFS volume. Reading the MFT and the USN journal needs an
//! elevated process, so these are `#[ignore]`d; run them from an administrator terminal:
//!
//! ```text
//! cargo test -p pare-core --release --test fast_mode -- --ignored --nocapture
//! ```
//! They only read the volume's metadata; the files they create live in a temp folder.
#![cfg(windows)]

mod common;

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use pare_core::index::{self, mft};
use pare_core::platform;

fn system_drive() -> PathBuf {
    let d = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    PathBuf::from(format!("{d}\\"))
}

#[test]
#[ignore]
fn mft_build_and_incremental_update() {
    if !platform::is_elevated() {
        eprintln!("skipped: run elevated");
        return;
    }
    let root = system_drive();
    let t = Instant::now();
    let mut idx = mft::build(&root).expect("MFT read");
    let build = t.elapsed();
    let files = idx.entries.iter().filter(|e| !e.is_dir).count();
    println!(
        "MFT index of {}: {} entries ({files} files) in {:.2} s",
        root.display(),
        idx.entries.len(),
        build.as_secs_f64()
    );
    assert!(files > 10_000);

    // 1,000 changes, then an incremental update.
    let dir = tempfile::tempdir().unwrap();
    for i in 0..1000 {
        fs::write(dir.path().join(format!("f{i:04}.bin")), vec![7u8; 10_000]).unwrap();
    }
    let t = Instant::now();
    let changed = mft::update(&mut idx).expect("journal");
    let inc = t.elapsed();
    println!(
        "USN update: {changed} changed in {:.3} s",
        inc.as_secs_f64()
    );
    assert!(changed >= 1000);
    assert!(inc.as_secs_f64() < 2.0, "incremental rescan took {inc:?}");

    // Hard links: one file id, several names, counted once.
    let a = dir.path().join("linked.bin");
    fs::write(&a, vec![1u8; 100_000]).unwrap();
    fs::hard_link(&a, dir.path().join("linked-2.bin")).unwrap();
    mft::update(&mut idx).unwrap();
    let id = platform::link_count(&a).unwrap().1 as u64;
    let names: Vec<_> = idx
        .entries
        .iter()
        .filter(|e| e.id & 0xFFFF_FFFF_FFFF == id & 0xFFFF_FFFF_FFFF)
        .collect();
    assert!(
        names.len() >= 2 || names.iter().all(|e| e.links == 2),
        "{names:?}"
    );
}

#[test]
#[ignore]
fn compressed_file_reports_size_on_disk() {
    if !platform::is_elevated() {
        eprintln!("skipped: run elevated");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("compressible.txt");
    fs::write(&f, "pare ".repeat(400_000)).unwrap();
    // NTFS compression via compact.exe (part of Windows).
    let ok = std::process::Command::new("compact")
        .args(["/c", "/q"])
        .arg(&f)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        eprintln!("skipped: compact.exe unavailable");
        return;
    }
    let win32 = platform::stat(&f).unwrap().allocated;
    let root = system_drive();
    let idx = mft::build(&root).unwrap();
    let by_id = idx.by_id();
    let e = idx
        .entries
        .iter()
        .enumerate()
        .find(|(i, _)| {
            idx.path_of(*i, &by_id).is_some_and(|p| {
                platform::path_starts_with(&p, &f) && platform::path_starts_with(&f, &p)
            })
        })
        .map(|(_, e)| e)
        .expect("file in index");
    println!(
        "compressed: logical {} bytes, on disk {} (Win32 {win32})",
        e.size, e.allocated
    );
    assert_eq!(e.allocated, win32);
    assert!(e.allocated < e.size);
    let _ = index::db_path;
}
