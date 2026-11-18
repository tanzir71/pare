//! Detectors propose [`Candidate`]s from the scan tree. Quick detectors are hard-coded (M2);
//! the rule engine (M3) and built-in detectors for logic TOML can't express live alongside.

pub mod claude;
pub mod codex;
pub mod ollama;
pub mod quick;

use serde::{Deserialize, Serialize};

use crate::env::Env;
use crate::scan::tree::{NodeId, Tree};
use crate::types::Timestamp;

/// User-adjustable thresholds (Settings → Thresholds).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Thresholds {
    pub large_file_mb: u64,
    pub large_file_days: u64,
    pub installer_days: u64,
    pub partial_days: u64,
    pub old_download_days: u64,
    pub idle_days: u64,
    pub screenshot_days: u64,
    pub duplicate_min_mb: u64,
    pub recent_hours: u64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            large_file_mb: 500,
            large_file_days: 90,
            installer_days: 7,
            partial_days: 1,
            old_download_days: 180,
            idle_days: 90,
            screenshot_days: 90,
            duplicate_min_mb: 1,
            recent_hours: 24,
        }
    }
}

pub struct DetectCtx<'a> {
    pub tree: &'a Tree,
    pub env: &'a Env,
    pub now: Timestamp,
    pub thresholds: &'a Thresholds,
    /// Scan-root node → label shown as the group header ("Documents", "This PC (C:)").
    pub root_labels: &'a [(NodeId, String)],
}
