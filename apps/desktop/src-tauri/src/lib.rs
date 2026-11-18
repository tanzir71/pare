//! Pare desktop shell: window, state and the IPC commands. All logic lives in pare-core.

mod commands;
mod state;

use state::AppState;

pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("PARE_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .try_init();

    let state = AppState::new(pare_core::Env::from_process());
    commands::startup_housekeeping(&state);

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init());
    // The updater only exists in builds signed for it (the release workflow passes the public
    // key). It never runs unless the user turned on "Check for updates".
    if let Some(pubkey) = option_env!("PARE_UPDATER_PUBKEY").filter(|k| !k.is_empty()) {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().pubkey(pubkey).build());
    }
    builder
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_drives,
            commands::known_folders,
            commands::scan_start,
            commands::scan_cancel,
            commands::scan_summary,
            commands::category_get,
            commands::finding_get,
            commands::protected_list,
            commands::clean,
            commands::history_list,
            commands::history_get,
            commands::history_restore,
            commands::recycle_purge_pare_items,
            commands::settings_get,
            commands::settings_set,
            commands::exclusion_add,
            commands::open_in_explorer,
            commands::projects_list,
            commands::projects_scan,
            commands::browse_view,
            commands::browse_kind_files,
            commands::browse_item,
            commands::relaunch_elevated,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pare");
}
