//! Pare core: scanning, rules, safety and disposal.
//!
//! This crate has no Tauri, UI or OS-dialog code so it can be tested headless and reused by
//! the CLI and MCP server.

/// Crate version, shown in the app's About screen.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::VERSION.is_empty());
    }
}
