pub mod ipc;
pub mod notify;
pub mod sessions;
pub mod tray;

use sessions::manager::SessionManager;
use sessions::permissions::{self, ReplyDecision};
use std::sync::Arc;
use tauri::{Emitter, Listener, Manager, State};

type SharedManager = Arc<tokio::sync::Mutex<SessionManager>>;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Answer a pending permission request. The ONLY reply path: explicit UI
/// click -> strict decision parse -> single registry take -> serve HTTP.
/// Unknown/settled requests and unreachable serve return Err, never act.
#[tauri::command]
async fn reply_permission(
    app: tauri::AppHandle,
    manager: State<'_, SharedManager>,
    request_id: String,
    reply: String,
) -> Result<(), String> {
    let decision = ReplyDecision::parse(&reply).ok_or_else(|| "invalid reply".to_string())?;
    let base =
        permissions::serve_url().ok_or_else(|| permissions::ReplyError::NoServeUrl.to_string())?;
    let http = permissions::http_client();
    let pending = {
        let mut m = manager.lock().await;
        m.take_pending(&request_id)
    };
    let Some(p) = pending else {
        return Err("unknown or already settled".to_string());
    };
    match permissions::reply(&http, &base, &p.request_id, decision).await {
        Ok(()) => {
            tracing::info!(request = %p.request_id, reply, "permission replied");
            let mut m = manager.lock().await;
            m.resolve_local(&p.session_id).await;
            let _ = app.emit("neko-event", m.snapshot().to_string());
            Ok(())
        }
        Err(e) => {
            // Upstream untouched or still pending: restore so UI can retry.
            if e != permissions::ReplyError::AlreadySettled {
                manager.lock().await.reinsert_pending(p);
            }
            Err(e.to_string())
        }
    }
}

#[tauri::command]
async fn get_setting(
    manager: State<'_, SharedManager>,
    key: String,
) -> Result<Option<String>, String> {
    Ok(manager.lock().await.setting_get(&key).await)
}

#[tauri::command]
async fn set_setting(
    manager: State<'_, SharedManager>,
    key: String,
    value: String,
) -> Result<(), String> {
    manager.lock().await.setting_set(&key, &value).await;
    Ok(())
}

#[tauri::command]
fn is_autostart(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let auto = app.autolaunch();
    if enabled {
        auto.enable()
    } else {
        auto.disable()
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn recenter(app: tauri::AppHandle) -> Result<(), String> {
    place_top_center(&app).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_always_on_top(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let Some(w) = app.get_webview_window("companion") else {
        return Ok(());
    };
    w.set_always_on_top(enabled).map_err(|e| e.to_string())
}

/// Position the companion top-center of the primary monitor.
/// Best-effort on Wayland (ADR-002): failure is logged, never fatal.
fn place_top_center(app: &tauri::AppHandle) -> tauri::Result<()> {
    let Some(win) = app.get_webview_window("companion") else {
        return Ok(());
    };
    let size = win.inner_size()?;
    let scale = win.scale_factor()?;
    let (mx, _my, mw) = match win.primary_monitor()? {
        Some(mon) => {
            let pos = mon.position();
            let size = mon.size();
            (pos.x as f64, pos.y as f64, size.width as f64)
        }
        None => return Ok(()),
    };
    let x = mx / scale + (mw / scale - size.width as f64 / scale) / 2.0;
    win.set_position(tauri::Position::Logical(tauri::LogicalPosition {
        x,
        y: 8.0,
    }))?;
    Ok(())
}

fn toggle_companion(app: &tauri::AppHandle) {
    let Some(w) = app.get_webview_window("companion") else {
        return;
    };
    match w.is_visible() {
        Ok(true) => {
            let _ = w.hide();
        }
        _ => {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

async fn tray_state(manager: &SharedManager, app: &tauri::AppHandle) -> tray::TrayState {
    use tauri_plugin_autostart::ManagerExt;
    let m = manager.lock().await;
    tray::TrayState {
        active_sessions: m.active_count(),
        notifications_on: m.setting_get("notifications").await.as_deref() != Some("0"),
        autostart_on: app.autolaunch().is_enabled().unwrap_or(false),
    }
}

async fn rebuild_tray(manager: &SharedManager, app: &tauri::AppHandle) {
    let state = tray_state(manager, app).await;
    if let Err(e) = tray::build(app, &state) {
        tracing::warn!(error = %e, "tray rebuild failed");
    }
}

/// Native notification for the three important events (master §18).
async fn maybe_notify(
    manager: &SharedManager,
    app: &tauri::AppHandle,
    neko_type: &str,
    payload: &serde_json::Value,
    project: Option<String>,
) {
    let m = manager.lock().await;
    if m.setting_get("notifications").await.as_deref() == Some("0") {
        return;
    }
    let project = project.unwrap_or_else(|| "OpenCode".to_string());
    match neko_type {
        "permission.requested" => {
            let action = payload
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let resource = payload
                .get("resource")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            notify::send(
                app,
                "Permission requested",
                &format!("{project} · {action} {resource}"),
            );
        }
        "session.completed" => notify::send(app, "Session completed", &project),
        "session.error" => notify::send(app, "Session error", &project),
        "session.status" => {
            let status = payload.get("status").and_then(|v| v.as_str()).unwrap_or("");
            if status == "completed" {
                notify::send(app, "Session completed", &project);
            } else if status == "error" {
                notify::send(app, "Session error", &project);
            }
        }
        _ => {}
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "neko=info".into()),
        )
        .init();
    tracing::info!(
        sock = ipc::socket_path(),
        proto = ipc::PROTOCOL_VERSION,
        "neko core starting"
    );
    let shortcut = tauri_plugin_global_shortcut::Builder::new()
        .with_shortcut("Super+Alt+N")
        .map(|b| {
            b.with_handler(|app, _shortcut, event| {
                use tauri_plugin_global_shortcut::ShortcutState;
                if event.state == ShortcutState::Pressed {
                    toggle_companion(app);
                }
            })
        });
    let shortcut = match shortcut {
        Ok(b) => {
            tracing::info!("global shortcut Super+Alt+N registered");
            b.build()
        }
        Err(e) => {
            tracing::warn!(error = %e, "shortcut unavailable, continuing without it");
            tauri_plugin_global_shortcut::Builder::new().build()
        }
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("Neko")
                .build(),
        )
        .plugin(shortcut)
        .invoke_handler(tauri::generate_handler![
            greet,
            reply_permission,
            get_setting,
            set_setting,
            is_autostart,
            set_autostart,
            set_always_on_top,
            recenter
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let sock = ipc::socket_path();
            // Consumer: validated events -> session engine -> UI snapshot.
            // Storage failure only kills this task (logged); the app survives.
            // The manager is shared with commands and tray toggles.
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, String)>();
            let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<SharedManager>();
            tauri::async_runtime::spawn(async move {
                let pool = match sessions::storage::open().await {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!(error = %e, "storage open failed");
                        return;
                    }
                };
                let manager = match SessionManager::load(pool).await {
                    Ok(m) => Arc::new(tokio::sync::Mutex::new(m)),
                    Err(e) => {
                        tracing::error!(error = %e, "session load failed");
                        return;
                    }
                };
                let _ = ready_tx.send(manager.clone());
                while let Some((t, line)) = rx.recv().await {
                    let parsed: serde_json::Value =
                        serde_json::from_str(&line).unwrap_or(serde_json::Value::Null);
                    let sid = parsed.get("sessionId").and_then(|s| s.as_str());
                    let payload = parsed
                        .get("payload")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));
                    let mut m = manager.lock().await;
                    let changed = m.apply(&t, sid, &payload).await;
                    let project = changed
                        .as_ref()
                        .and_then(|s| s.project_name.clone().or_else(|| s.project_path.clone()));
                    maybe_notify(&manager, &handle, &t, &payload, project).await;
                    let _ = handle.emit("neko-event", m.snapshot().to_string());
                    rebuild_tray(&manager, &handle).await;
                }
            });
            let manage_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(manager) = ready_rx.await {
                    // Tray + placement need the manager; build once ready.
                    rebuild_tray(&manager, &manage_handle).await;
                    if let Err(e) = place_top_center(&manage_handle) {
                        tracing::warn!(error = %e, "initial placement failed");
                    }
                    // Tray toggle events from Rust side.
                    let toggle_manager = manager.clone();
                    let toggle_handle = manage_handle.clone();
                    manage_handle.listen("neko-tray", move |event| {
                        let id = event.payload().to_string();
                        let m = toggle_manager.clone();
                        let h = toggle_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            toggle_setting(&m, &h, id.trim_matches('"')).await;
                        });
                    });
                    manage_handle.manage(manager);
                }
            });
            tauri::async_runtime::spawn(async move {
                match ipc::bind(&sock).await {
                    Ok(listener) => {
                        tracing::info!(sock = %sock, "ipc listening");
                        let on_event: ipc::EventCb = Arc::new(move |t, line| {
                            let _ = tx.send((t, line));
                        });
                        ipc::serve_on(listener, on_event).await;
                    }
                    Err(e) => tracing::error!(sock = %sock, error = %e, "ipc bind failed"),
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Tray check-menu toggles (notifications / autostart).
async fn toggle_setting(manager: &SharedManager, app: &tauri::AppHandle, id: &str) {
    use tauri_plugin_autostart::ManagerExt;
    match id {
        "notif" => {
            let m = manager.lock().await;
            let cur = m.setting_get("notifications").await;
            let next = if cur.as_deref() == Some("0") {
                "1"
            } else {
                "0"
            };
            m.setting_set("notifications", next).await;
            drop(m);
            rebuild_tray(manager, app).await;
        }
        "auto" => {
            let auto = app.autolaunch();
            let enabled = auto.is_enabled().unwrap_or(false);
            let r = if enabled {
                auto.disable()
            } else {
                auto.enable()
            };
            if let Err(e) = r {
                tracing::warn!(error = %e, "autostart toggle failed");
            }
            rebuild_tray(manager, app).await;
        }
        _ => {}
    }
}
