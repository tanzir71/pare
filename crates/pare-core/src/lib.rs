//! Pare core: scanning, rules, safety and disposal.
//!
//! This crate has no Tauri, UI or OS-dialog code so it can be tested headless and reused by
//! the CLI and MCP server.

pub mod detectors;
pub mod dispose;
pub mod env;
pub mod format;
pub mod history;
pub mod index;
pub mod monitor;
pub mod platform;
pub mod projects;
pub mod rules;
pub mod safety;
pub mod scan;
pub mod scanner;
pub mod settings;
pub mod types;

pub use env::Env;
pub use scanner::{ScanConfig, ScanOutput, ScanRequest, ScanResult, ScanRoot, scan};
pub use types::*;

/// Crate version, shown in the app's About screen.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::VERSION.is_empty());
    }
}
