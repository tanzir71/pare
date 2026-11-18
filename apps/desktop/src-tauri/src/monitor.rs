//! M8: the notification-area icon and the monitor timer. Both live only as long as Pare runs;
//! Pare installs no service and no scheduled task. The work itself is in pare-core.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use pare_core::format::format_bytes;
use pare_core::i18n::{tr, trf};
use pare_core::monitor::{self, Growth};
use pare_core::platform::now_ms;
use pare_core::types::MS_PER_DAY;
use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::state::AppState;

const TRAY_ID: &str = "pare";
static RUNNING: AtomicBool = AtomicBool::new(false);

/// Start the timer and show the icon if the user turned it on.
pub fn setup(app: &AppHandle) {
    sync_tray(app);
    let app = app.clone();
    std::thread::spawn(move || {
        // Let startup (and any scan the user starts right away) settle first.
        std::thread::sleep(Duration::from_secs(90));
        loop {
            run(&app, false);
            std::thread::sleep(Duration::from_secs(15 * 60));
        }
    });
}

/// Closing the window hides it to the notification area when the user asked for that.
pub fn keep_in_tray(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .settings
        .lock()
        .map(|s| s.monitor.tray)
        .unwrap_or(false)
}

/// Add or remove the icon to match settings.
pub fn sync_tray(app: &AppHandle) {
    let want = keep_in_tray(app);
    let have = app.tray_by_id(TRAY_ID).is_some();
    if want && !have {
        if let Err(e) = build_tray(app) {
            tracing::warn!(%e, "couldn't create the notification area icon");
        }
    } else if !want && have {
        let _ = app.remove_tray_by_id(TRAY_ID);
        show_main(app);
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", tr("Open Pare"), true, None::<&str>)?;
    let check = MenuItem::with_id(app, "check", tr("Check for growth now"), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", tr("Quit Pare"), true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &check, &sep, &quit])?;
    let mut b = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Pare")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show_main(app),
            "check" => {
                let app = app.clone();
                std::thread::spawn(move || run(&app, true));
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = ev
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon().cloned() {
        b = b.icon(icon);
    }
    b.build(app)?;
    Ok(())
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// "~\.codex grew 12 GB this week"
pub fn alert_text(g: &Growth, home: &std::path::Path, now: i64) -> String {
    let what = match &g.path {
        Some(p) => match p.strip_prefix(home) {
            Ok(rest) => format!("~\\{}", rest.display()),
            Err(_) => p.display().to_string(),
        },
        None => g.label.clone(),
    };
    let days = ((now - g.since) as f64 / MS_PER_DAY as f64).round() as i64;
    let size = format_bytes(g.delta());
    let args = [("what", what.as_str()), ("size", size.as_str())];
    if days >= 6 {
        trf("{what} grew {size} this week", &args)
    } else if days <= 1 {
        trf("{what} grew {size} since yesterday", &args)
    } else {
        trf(
            "{what} grew {size} in {n} days",
            &[args[0], args[1], ("n", &days.to_string())],
        )
    }
}

/// Rebuild the icon so its menu uses the current language.
pub fn refresh_tray(app: &AppHandle) {
    if app.tray_by_id(TRAY_ID).is_some() {
        let _ = app.remove_tray_by_id(TRAY_ID);
        if let Err(e) = build_tray(app) {
            tracing::warn!(%e, "couldn't recreate the notification area icon");
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScheduledPayload {
    report: pare_core::dispose::CleanReport,
}

fn notify(app: &AppHandle, body: &str) {
    let _ = app.notification().builder().title("Pare").body(body).show();
}

/// One monitor pass. `manual` is the tray's "Check for growth now".
pub fn run(app: &AppHandle, manual: bool) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    let state = app.state::<AppState>();
    let settings = match state.settings.lock() {
        Ok(s) => s.clone(),
        Err(_) => {
            RUNNING.store(false, Ordering::SeqCst);
            return;
        }
    };
    let busy = state.cleaning.load(Ordering::SeqCst)
        || state.scan.lock().map(|s| s.running).unwrap_or(true);
    let wanted = manual || settings.monitor.enabled || !settings.schedules.is_empty();
    // Scheduled cleanups hold the same flag as "Clean up", so the two never overlap.
    let cleans = !manual && !settings.schedules.is_empty();
    // Never start while the user is scanning or cleaning; try again next time.
    if busy || !wanted || (cleans && state.cleaning.swap(true, Ordering::SeqCst)) {
        RUNNING.store(false, Ordering::SeqCst);
        return;
    }
    let cfg = crate::commands::config_from(&settings, state.rules.clone());
    let env = state.env.clone();
    let now = now_ms();
    if manual {
        monitor::measure_growth(&cfg, &env, now);
        let alerts = monitor::current_alerts(&env, &settings);
        let body = match alerts.first() {
            Some(g) => alert_text(g, &env.home, now),
            None => trf(
                "Nothing grew by more than {n} GB this week.",
                &[("n", &settings.monitor.alert_gb.to_string())],
            ),
        };
        notify(app, &body);
        let _ = app.emit("monitor://alerts", alerts);
    } else {
        let r = monitor::tick(&env, &cfg, &settings, now);
        for g in r.new_alerts.iter().take(3) {
            notify(app, &alert_text(g, &env.home, now));
        }
        if !r.new_alerts.is_empty() {
            let _ = app.emit("monitor://alerts", monitor::current_alerts(&env, &settings));
        }
        if let Some(report) = r.scheduled {
            if report.freed_now + report.recycled > 0 {
                notify(
                    app,
                    &trf(
                        "Scheduled cleanup freed {size}. See History to review it.",
                        &[("size", &format_bytes(report.freed_now + report.recycled))],
                    ),
                );
            }
            let _ = app.emit("monitor://scheduled", ScheduledPayload { report });
        }
    }
    if cleans {
        state.cleaning.store(false, Ordering::SeqCst);
    }
    RUNNING.store(false, Ordering::SeqCst);
}
