//! App state shared by commands: the environment, settings and the current scan.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use pare_core::Env;
use pare_core::scan::progress::Progress;
use pare_core::scanner::{DetectorStatus, ScanOutput};
use pare_core::settings::Settings;

pub struct AppState {
    pub env: Env,
    pub settings: Mutex<Settings>,
    pub scan: Mutex<ScanSlot>,
    pub cleaning: AtomicBool,
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
        AppState {
            env,
            settings: Mutex::new(settings),
            scan: Mutex::new(ScanSlot::default()),
            cleaning: AtomicBool::new(false),
        }
    }
}
