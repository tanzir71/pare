//! Lock-free scan progress counters. The UI layer samples [`Progress::snapshot`] on its own
//! timer (every 250 ms), so the walker never blocks on event delivery.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Starting,
    Enumerating,
    Detecting,
    Measuring,
    Done,
    Cancelled,
}

impl Phase {
    fn from_u8(v: u8) -> Phase {
        match v {
            1 => Phase::Enumerating,
            2 => Phase::Detecting,
            3 => Phase::Measuring,
            4 => Phase::Done,
            5 => Phase::Cancelled,
            _ => Phase::Starting,
        }
    }
    fn as_u8(self) -> u8 {
        match self {
            Phase::Starting => 0,
            Phase::Enumerating => 1,
            Phase::Detecting => 2,
            Phase::Measuring => 3,
            Phase::Done => 4,
            Phase::Cancelled => 5,
        }
    }
}

pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes_seen: AtomicU64,
    pub found_bytes: AtomicU64,
    /// Detectors finished / total, for the checklist.
    pub detectors_done: AtomicU64,
    pub detectors_total: AtomicU64,
    phase: AtomicU8,
    current: Mutex<String>,
    started: Instant,
    /// Rough total bytes to enumerate (used drive space for whole-drive scans), if known.
    pub estimate_bytes: AtomicU64,
}

impl Default for Progress {
    fn default() -> Self {
        Progress {
            files: AtomicU64::new(0),
            dirs: AtomicU64::new(0),
            bytes_seen: AtomicU64::new(0),
            found_bytes: AtomicU64::new(0),
            detectors_done: AtomicU64::new(0),
            detectors_total: AtomicU64::new(0),
            phase: AtomicU8::new(0),
            current: Mutex::new(String::new()),
            started: Instant::now(),
            estimate_bytes: AtomicU64::new(0),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProgressSnapshot {
    pub phase: Phase,
    pub files: u64,
    pub dirs: u64,
    pub bytes_seen: u64,
    pub found_bytes: u64,
    pub current: String,
    pub elapsed_ms: u64,
    /// 0..=1. Enumeration is 0–70 %, detection and measuring the rest.
    pub fraction: f64,
    pub detectors_done: u64,
    pub detectors_total: u64,
}

impl Progress {
    pub fn set_phase(&self, p: Phase) {
        self.phase.store(p.as_u8(), Ordering::Relaxed);
    }

    pub fn phase(&self) -> Phase {
        Phase::from_u8(self.phase.load(Ordering::Relaxed))
    }

    pub fn set_current(&self, path: &Path) {
        if let Ok(mut c) = self.current.try_lock() {
            *c = path.to_string_lossy().into_owned();
        }
    }

    pub fn snapshot(&self) -> ProgressSnapshot {
        let phase = self.phase();
        let files = self.files.load(Ordering::Relaxed);
        let bytes_seen = self.bytes_seen.load(Ordering::Relaxed);
        let estimate = self.estimate_bytes.load(Ordering::Relaxed);
        let det_done = self.detectors_done.load(Ordering::Relaxed);
        let det_total = self.detectors_total.load(Ordering::Relaxed).max(1);
        let enum_frac = if estimate > 0 {
            (bytes_seen as f64 / estimate as f64).min(0.99)
        } else {
            // Unknown total: approach 1 asymptotically with the number of files seen.
            1.0 - (-(files as f64) / 60_000.0).exp()
        };
        let fraction = match phase {
            Phase::Starting => 0.0,
            Phase::Enumerating => 0.7 * enum_frac,
            Phase::Detecting | Phase::Measuring => {
                0.7 + 0.3 * (det_done as f64 / det_total as f64).min(0.99)
            }
            Phase::Done | Phase::Cancelled => 1.0,
        };
        ProgressSnapshot {
            phase,
            files,
            dirs: self.dirs.load(Ordering::Relaxed),
            bytes_seen,
            found_bytes: self.found_bytes.load(Ordering::Relaxed),
            current: self.current.lock().map(|c| c.clone()).unwrap_or_default(),
            elapsed_ms: self.started.elapsed().as_millis() as u64,
            fraction,
            detectors_done: det_done,
            detectors_total: self.detectors_total.load(Ordering::Relaxed),
        }
    }
}
