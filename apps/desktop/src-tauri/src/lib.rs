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

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pare");
}
