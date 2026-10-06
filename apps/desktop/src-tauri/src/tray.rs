//! System tray (Phase 6, master §19). Built once; later states sync the
//! count text and checkmarks in place. Destroy/recreate raced async dbus
//! unregistration ("already exported"), so it only happens as a fallback
//! when cached handles go stale.

use std::sync::Mutex;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

pub const TRAY_ID: &str = "main";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayState {
    pub active_sessions: usize,
    pub notifications_on: bool,
    pub autostart_on: bool,
}

#[derive(Clone)]
struct TrayHandles {
    count: MenuItem<tauri::Wry>,
    notif: CheckMenuItem<tauri::Wry>,
    auto: CheckMenuItem<tauri::Wry>,
}

static HANDLES: std::sync::OnceLock<Mutex<Option<TrayHandles>>> = std::sync::OnceLock::new();

fn handles() -> &'static Mutex<Option<TrayHandles>> {
    HANDLES.get_or_init(|| Mutex::new(None))
}

fn count_text(s: &TrayState) -> String {
    format!(
        "● {} active session{}",
        s.active_sessions,
        if s.active_sessions == 1 { "" } else { "s" }
    )
}

/// Sync the tray to `s`: in-place text/check updates, no destroy.
/// Falls back to a full rebuild only when cached handles go stale.
pub fn sync(app: &AppHandle, s: &TrayState) -> tauri::Result<()> {
    if let Some(h) = handles().lock().unwrap().clone() {
        if update(&h, s).is_ok() {
            return Ok(());
        }
        tracing::debug!("tray handles stale, rebuilding");
    }
    build_fresh(app, s)
}

fn update(h: &TrayHandles, s: &TrayState) -> tauri::Result<()> {
    h.count.set_text(count_text(s))?;
    h.notif.set_checked(s.notifications_on)?;
    h.auto.set_checked(s.autostart_on)?;
    Ok(())
}

fn build_fresh(app: &AppHandle, s: &TrayState) -> tauri::Result<()> {
    if app.remove_tray_by_id(TRAY_ID).is_some() {
        tracing::debug!("rebuilt tray menu");
    }
    let title = MenuItem::with_id(app, "title", "Neko", false, None::<&str>)?;
    let count = MenuItem::with_id(app, "count", count_text(s), false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open Sessions", true, None::<&str>)?;
    let toggle_win =
        MenuItem::with_id(app, "toggle_win", "Show/Hide Companion", true, None::<&str>)?;
    let notif = CheckMenuItem::with_id(
        app,
        "notif",
        "Notifications",
        true,
        s.notifications_on,
        None::<&str>,
    )?;
    let auto = CheckMenuItem::with_id(
        app,
        "auto",
        "Start on Login",
        true,
        s.autostart_on,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(app, "settings", "Open Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &title,
            &count,
            &PredefinedMenuItem::separator(app)?,
            &open,
            &toggle_win,
            &PredefinedMenuItem::separator(app)?,
            &notif,
            &auto,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Neko — OpenCode companion")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                // Funneled through the single visibility owner in lib.rs
                // (persists the setting); see toggle_setting.
                let _ = tray.app_handle().emit("neko-tray", "toggle_win");
            }
        })
        .build(app)?;
    *handles().lock().unwrap() = Some(TrayHandles {
        count: count.clone(),
        notif: notif.clone(),
        auto: auto.clone(),
    });
    Ok(())
}

fn companion(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    app.get_webview_window("companion")
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => {
            if let Some(w) = companion(app) {
                let _ = w.show();
                let _ = w.set_focus();
            }
            let _ = app.emit("neko-ui", "expand");
        }
        "toggle_win" => {
            let _ = app.emit("neko-tray", "toggle_win");
        }
        "settings" => {
            if let Some(w) = companion(app) {
                let _ = w.show();
            }
            let _ = app.emit("neko-ui", "settings");
        }
        "notif" | "auto" => {
            let _ = app.emit("neko-tray", id);
        }
        "quit" => app.exit(0),
        _ => {}
    }
}
