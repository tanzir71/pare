//! Where things live: user profile, AppData, temp, known folders, and Pare's own folders.
//!
//! Everything in pare-core takes an [`Env`] instead of reading process environment variables,
//! so tests use [`Env::fake`] (the "FakeHome") and never touch real user folders.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Env {
    /// `%USERPROFILE%`, `~`
    pub home: PathBuf,
    /// `%APPDATA%` (Roaming)
    pub appdata: PathBuf,
    /// `%LOCALAPPDATA%`
    pub local_appdata: PathBuf,
    /// `%TEMP%`
    pub temp: PathBuf,
    /// `%PROGRAMDATA%`
    pub program_data: PathBuf,
    pub desktop: PathBuf,
    pub downloads: PathBuf,
    pub documents: PathBuf,
    pub pictures: PathBuf,
    /// `%SystemRoot%` (e.g. `C:\Windows`). `None` in a fake environment.
    pub system_root: Option<PathBuf>,
    /// Other variables available to rule path expansion (`${OLLAMA_MODELS:-…}`).
    vars: BTreeMap<String, String>,
    /// True for [`Env::fake`]: system integrations (Recycle Bin, drives) are disabled.
    pub is_fake: bool,
}

impl Env {
    /// The real environment of this process.
    pub fn from_process() -> Env {
        let vars: BTreeMap<String, String> = std::env::vars()
            .map(|(k, v)| (k.to_ascii_uppercase(), v))
            .collect();
        let get = |k: &str| vars.get(k).map(PathBuf::from);
        let home = get("USERPROFILE")
            .or_else(|| get("HOME"))
            .unwrap_or_else(|| PathBuf::from("."));
        let appdata = get("APPDATA").unwrap_or_else(|| home.join("AppData").join("Roaming"));
        let local_appdata =
            get("LOCALAPPDATA").unwrap_or_else(|| home.join("AppData").join("Local"));
        let temp = get("TEMP")
            .or_else(|| get("TMP"))
            .unwrap_or_else(std::env::temp_dir);
        let program_data = get("PROGRAMDATA").unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
        let system_root = get("SYSTEMROOT").or_else(|| get("WINDIR"));
        let kf = |kind: crate::platform::KnownFolder, fallback: &str| {
            crate::platform::known_folder(kind).unwrap_or_else(|| home.join(fallback))
        };
        Env {
            desktop: kf(crate::platform::KnownFolder::Desktop, "Desktop"),
            downloads: kf(crate::platform::KnownFolder::Downloads, "Downloads"),
            documents: kf(crate::platform::KnownFolder::Documents, "Documents"),
            pictures: kf(crate::platform::KnownFolder::Pictures, "Pictures"),
            home,
            appdata,
            local_appdata,
            temp,
            program_data,
            system_root,
            vars,
            is_fake: false,
        }
    }

    /// A self-contained fake environment rooted at `root` (the "FakeHome"). Creates the folders.
    /// No real environment variables are visible to rule expansion.
    pub fn fake(root: &Path) -> Env {
        let home = root.join("Users").join("ana");
        let appdata = home.join("AppData").join("Roaming");
        let local_appdata = home.join("AppData").join("Local");
        let env = Env {
            temp: local_appdata.join("Temp"),
            program_data: root.join("ProgramData"),
            desktop: home.join("Desktop"),
            downloads: home.join("Downloads"),
            documents: home.join("Documents"),
            pictures: home.join("Pictures"),
            home,
            appdata,
            local_appdata,
            system_root: None,
            vars: BTreeMap::new(),
            is_fake: true,
        };
        for dir in [
            &env.home,
            &env.appdata,
            &env.local_appdata,
            &env.temp,
            &env.program_data,
            &env.desktop,
            &env.downloads,
            &env.documents,
            &env.pictures,
        ] {
            let _ = std::fs::create_dir_all(dir);
        }
        env
    }

    /// Set an extra variable (for rule expansion tests, e.g. `OLLAMA_MODELS`).
    #[must_use]
    pub fn with_var(mut self, key: &str, value: &str) -> Env {
        self.vars
            .insert(key.to_ascii_uppercase(), value.to_string());
        self
    }

    /// Look up a variable, case-insensitively. The well-known folders always win.
    pub fn var(&self, key: &str) -> Option<String> {
        let k = key.to_ascii_uppercase();
        let p = |p: &Path| Some(p.to_string_lossy().into_owned());
        match k.as_str() {
            "USERPROFILE" | "HOME" => p(&self.home),
            "APPDATA" => p(&self.appdata),
            "LOCALAPPDATA" => p(&self.local_appdata),
            "TEMP" | "TMP" => p(&self.temp),
            "PROGRAMDATA" => p(&self.program_data),
            "SYSTEMROOT" | "WINDIR" => self.system_root.as_deref().and_then(p),
            _ => self.vars.get(&k).cloned(),
        }
    }

    /// `%LOCALAPPDATA%\Pare`
    pub fn pare_data_dir(&self) -> PathBuf {
        self.local_appdata.join("Pare")
    }

    /// `%APPDATA%\Pare`
    pub fn pare_config_dir(&self) -> PathBuf {
        self.appdata.join("Pare")
    }

    /// `%LOCALAPPDATA%\Pare\logs\operations.jsonl`
    pub fn oplog_path(&self) -> PathBuf {
        self.pare_data_dir().join("logs").join("operations.jsonl")
    }

    /// `%APPDATA%\Pare\settings.json`
    pub fn settings_path(&self) -> PathBuf {
        self.pare_config_dir().join("settings.json")
    }

    /// `%APPDATA%\Pare\rules` (user rules; can add but never loosen built-in tiers).
    pub fn user_rules_dir(&self) -> PathBuf {
        self.pare_config_dir().join("rules")
    }

    /// `%LOCALAPPDATA%\Pare\index`
    pub fn index_dir(&self) -> PathBuf {
        self.pare_data_dir().join("index")
    }
}
