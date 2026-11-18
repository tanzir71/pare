// The command list doubles as the IPC allow-list: only these are callable from the webview
// (see capabilities/default.json).
const COMMANDS: &[&str] = &[
    "app_info",
    "list_drives",
    "known_folders",
    "scan_start",
    "scan_cancel",
    "scan_summary",
    "category_get",
    "finding_get",
    "protected_list",
    "clean",
    "history_list",
    "history_get",
    "history_restore",
    "recycle_purge_pare_items",
    "settings_get",
    "settings_set",
    "exclusion_add",
    "open_in_explorer",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
