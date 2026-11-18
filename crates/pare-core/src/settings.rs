//! `%APPDATA%\Pare\settings.json`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::detectors::Thresholds;
use crate::env::Env;
use crate::scanner::ScanRoot;
use crate::types::CategoryId;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Theme,
    /// "Always keep" paths: never suggested.
    pub exclusions: Vec<PathBuf>,
    /// Folders the user added as scan scopes ("Add folder").
    pub custom_roots: Vec<ScanRoot>,
    /// Where to look for git projects (M4). Empty = defaults.
    pub project_roots: Vec<PathBuf>,
    pub thresholds: Thresholds,
    /// "Send caches to the Recycle Bin too": turn every `delete` into `recycle`.
    pub caches_to_recycle_bin: bool,
    /// Empty Pare's own Recycle Bin items after this many days (None = never).
    pub purge_pare_items_after_days: Option<u32>,
    /// Hidden suggestion cards.
    pub hidden_cards: Vec<CategoryId>,
    /// Opt-in update check (M7). Off until the user says yes.
    pub check_for_updates: bool,
    /// First-run onboarding seen (M7).
    pub onboarding_done: bool,
    /// UI language (BCP 47 tag, e.g. `en`, `es`). Empty = follow Windows.
    pub language: String,
    /// Growth monitor and notification area (M8).
    pub monitor: MonitorSettings,
    /// Opt-in scheduled cleanups, one per Safe rule (M8).
    pub schedules: Vec<RuleSchedule>,
}

/// M8 monitor. Everything here runs only while Pare is open (its window or its notification
/// area icon): Pare installs no service and no scheduled task.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct MonitorSettings {
    /// Check rule folders for growth every `check_hours` and alert on big jumps.
    pub enabled: bool,
    /// Keep running in the notification area when the window closes.
    pub tray: bool,
    /// Alert when something grows by at least this many GB in a week.
    pub alert_gb: u32,
    pub check_hours: u32,
}

impl Default for MonitorSettings {
    fn default() -> Self {
        MonitorSettings {
            enabled: false,
            tray: false,
            alert_gb: 5,
            check_hours: 6,
        }
    }
}

/// "Clean this automatically": one Safe rule, every `every_days` days.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleSchedule {
    pub rule: String,
    pub every_days: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: Theme::System,
            exclusions: Vec::new(),
            custom_roots: Vec::new(),
            project_roots: Vec::new(),
            thresholds: Thresholds::default(),
            caches_to_recycle_bin: false,
            purge_pare_items_after_days: Some(30),
            hidden_cards: Vec::new(),
            check_for_updates: false,
            onboarding_done: false,
            language: String::new(),
            monitor: MonitorSettings::default(),
            schedules: Vec::new(),
        }
    }
}

impl Settings {
    pub fn load(env: &Env) -> Settings {
        std::fs::read(env.settings_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, env: &Env) -> std::io::Result<()> {
        let path = env.settings_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?,
        )?;
        std::fs::rename(tmp, path)
    }

    /// Merge a JSON patch (top-level keys) into these settings.
    pub fn patch(&self, patch: serde_json::Value) -> Result<Settings, String> {
        let mut v = serde_json::to_value(self).map_err(|e| e.to_string())?;
        if let (Some(obj), Some(p)) = (v.as_object_mut(), patch.as_object()) {
            for (k, val) in p {
                obj.insert(k.clone(), val.clone());
            }
        }
        serde_json::from_value(v).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_patch() {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::fake(dir.path());
        assert_eq!(Settings::load(&env), Settings::default());
        let s = Settings::default()
            .patch(serde_json::json!({ "theme": "dark", "exclusions": ["C:\\keep"] }))
            .unwrap();
        s.save(&env).unwrap();
        let back = Settings::load(&env);
        assert_eq!(back.theme, Theme::Dark);
        assert_eq!(back.exclusions, vec![PathBuf::from("C:\\keep")]);
        assert!(!back.check_for_updates, "updates are opt-in");
        assert!(
            !back.monitor.enabled && !back.monitor.tray,
            "monitor is opt-in"
        );
        assert!(
            back.schedules.is_empty(),
            "no scheduled cleanups by default"
        );
    }
}
