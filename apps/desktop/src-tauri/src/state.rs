//! App state shared by commands: the environment, settings and the current scan.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use pare_core::Env;
use pare_core::scan::progress::Progress;
use pare_core::scanner::{DetectorStatus, ScanOutput};
use pare_core::settings::Settings;

pub struct AppState {
    pub env: Env,
    /// Built-in rules plus the user rules folder, loaded once at startup.
    pub rules: Arc<pare_core::rules::RuleSet>,
    pub settings: Mutex<Settings>,
    pub scan: Mutex<ScanSlot>,
    pub cleaning: AtomicBool,
    /// The last Projects-tab scan: projects, their artifact findings, and when it started.
    pub projects: Mutex<Option<ProjectsSlot>>,
    /// Items picked in Browse, as findings (so cleaning goes through the same safety pass).
    pub browse: Mutex<Vec<(i64, pare_core::Finding)>>,
    /// Fast-mode drive indexes, kept between scans and caught up from the USN journal.
    pub indexes:
        Arc<Mutex<std::collections::HashMap<std::path::PathBuf, pare_core::index::VolumeIndex>>>,
}

pub struct ProjectsSlot {
    pub started: i64,
    pub roots: Vec<std::path::PathBuf>,
    pub projects: Vec<pare_core::projects::Project>,
    pub findings: Vec<pare_core::Finding>,
}

#[derive(Default)]
pub struct ScanSlot {
    pub id: u64,
    pub running: bool,
    pub cancel: Option<Arc<AtomicBool>>,
    pub progress: Option<Arc<Progress>>,
    pub detectors: Vec<DetectorStatus>,
    /// The last finished scan. Cleaning only ever acts on findings from here.
    pub output: Option<Arc<ScanOutput>>,
}

impl AppState {
    pub fn new(env: Env) -> AppState {
        let settings = Settings::load(&env);
        let rules = Arc::new(pare_core::rules::RuleSet::with_user_rules(
            &env.user_rules_dir(),
        ));
        for w in &rules.warnings {
            tracing::warn!(%w, "user rule ignored");
        }
        AppState {
            rules,
            env,
            settings: Mutex::new(settings),
            scan: Mutex::new(ScanSlot::default()),
            cleaning: AtomicBool::new(false),
            projects: Mutex::new(None),
            browse: Mutex::new(Vec::new()),
            indexes: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }
}
