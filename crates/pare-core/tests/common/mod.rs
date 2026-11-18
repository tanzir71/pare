//! Test fixtures. Everything lives in a temp dir; `FakeHome` gives pare-core an [`Env`] whose
//! %USERPROFILE%, %APPDATA%, %LOCALAPPDATA% and %TEMP% point inside it (invariant 9).
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime};

use pare_core::detectors::Thresholds;
use pare_core::dispose::{CleanOptions, CleanReport, OpLog, clean};
use pare_core::scan::progress::Progress;
use pare_core::scanner::{ScanConfig, ScanOutput, ScanRequest, ScanRoot, scan};
use pare_core::{Env, Finding};

pub const DAY: f64 = 86_400.0;

pub struct FakeHome {
    pub dir: tempfile::TempDir,
    pub env: Env,
}

impl FakeHome {
    pub fn new() -> FakeHome {
        let dir = tempfile::Builder::new()
            .prefix("pare-test-")
            .tempdir()
            .unwrap();
        let env = Env::fake(dir.path());
        FakeHome { dir, env }
    }

    pub fn home(&self) -> &Path {
        &self.env.home
    }

    /// Create a file relative to `~` with `size` bytes, last modified `age_days` ago.
    pub fn file(&self, rel: &str, size: usize, age_days: f64) -> PathBuf {
        let p = self.home().join(rel);
        write_file(&p, size, age_days);
        p
    }

    pub fn dir(&self, rel: &str) -> PathBuf {
        let p = self.home().join(rel);
        fs::create_dir_all(&p).unwrap();
        p
    }

    pub fn config(&self) -> ScanConfig {
        ScanConfig {
            thresholds: Thresholds::default(),
            exclusions: Vec::new(),
            check_in_use: true,
            include_recycle_bin: false,
            now: None,
        }
    }

    /// Scan the given folders (absolute paths).
    pub fn scan_with(&self, roots: &[&Path], cfg: &ScanConfig) -> ScanOutput {
        let req = ScanRequest {
            roots: roots
                .iter()
                .map(|p| ScanRoot {
                    path: p.to_path_buf(),
                    label: p.file_name().unwrap().to_string_lossy().into_owned(),
                })
                .collect(),
            drives: Vec::new(),
            whole_drives: false,
        };
        let progress = Progress::default();
        let cancel = AtomicBool::new(false);
        scan(&req, cfg, &self.env, &progress, &cancel, &|_| {})
    }

    /// Scan the whole fake home.
    pub fn scan(&self) -> ScanOutput {
        self.scan_with(&[self.home()], &self.config())
    }

    pub fn oplog(&self) -> OpLog {
        OpLog::open(&self.env.oplog_path()).unwrap()
    }

    pub fn clean(&self, findings: &[Finding], scan_started: i64) -> CleanReport {
        let log = self.oplog();
        let mut opts = CleanOptions::new(scan_started);
        opts.check_in_use = true;
        clean(findings, &self.env, &log, &opts, &|_| {})
    }
}

pub fn write_file(p: &Path, size: usize, age_days: f64) {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(p, vec![b'x'; size]).unwrap();
    set_age(p, age_days);
}

pub fn set_age(p: &Path, age_days: f64) {
    let t = SystemTime::now() - Duration::from_secs_f64(age_days * DAY);
    let ft = filetime::FileTime::from_system_time(t);
    filetime::set_file_times(p, ft, ft).unwrap();
}

pub fn find<'a>(findings: &'a [Finding], rule: &str, title: &str) -> Option<&'a Finding> {
    findings
        .iter()
        .find(|f| f.rule_id == rule && f.title == title)
}

/// Spawn a child process (this test binary, running the ignored `helper_hold_file` test) that
/// opens `path` exclusively and holds it until dropped.
pub struct Holder {
    child: Child,
    release: PathBuf,
}

impl Holder {
    pub fn spawn(path: &Path) -> Holder {
        let ready = path.with_extension("ready");
        let release = path.with_extension("release");
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "helper_hold_file", "--nocapture"])
            .env("PARE_HOLD_FILE", path)
            .spawn()
            .unwrap();
        let start = std::time::Instant::now();
        while !ready.exists() {
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "holder process didn't start"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = fs::remove_file(&ready);
        Holder { child, release }
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        let _ = fs::write(&self.release, b"");
        let _ = self.child.wait();
        let _ = fs::remove_file(&self.release);
    }
}

/// Body of the helper process. Call from an `#[ignore]` test named `helper_hold_file`.
pub fn hold_file_main() {
    let Ok(p) = std::env::var("PARE_HOLD_FILE") else {
        return;
    };
    let p = PathBuf::from(p);
    #[cfg(windows)]
    let _f = {
        use std::os::windows::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&p)
            .unwrap()
    };
    #[cfg(not(windows))]
    let _f = fs::File::open(&p).unwrap();
    fs::write(p.with_extension("ready"), b"").unwrap();
    let release = p.with_extension("release");
    let start = std::time::Instant::now();
    while !release.exists() && start.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Remove items this test put into the real Recycle Bin (originals under `under`).
#[cfg(windows)]
pub fn purge_recycled_under(under: &Path) {
    if let Ok(items) = trash::os_limited::list() {
        let mine: Vec<_> = items
            .into_iter()
            .filter(|i| pare_core::platform::path_starts_with(&i.original_parent, under))
            .collect();
        if !mine.is_empty() {
            let _ = trash::os_limited::purge_all(mine);
        }
    }
}

/// Set the modification time of `path` and everything under it (links are not followed).
pub fn age_tree(path: &Path, age_days: f64) {
    let Ok(m) = fs::symlink_metadata(path) else {
        return;
    };
    if m.file_type().is_symlink() {
        let t = SystemTime::now() - Duration::from_secs_f64(age_days * DAY);
        let ft = filetime::FileTime::from_system_time(t);
        let _ = filetime::set_symlink_file_times(path, ft, ft);
        return;
    }
    if m.is_dir() {
        for e in fs::read_dir(path).unwrap() {
            age_tree(&e.unwrap().path(), age_days);
        }
    }
    set_age(path, age_days);
}
