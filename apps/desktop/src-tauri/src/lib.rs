pub mod ipc;
pub mod notify;
pub mod sessions;
pub mod tray;

use sessions::manager::SessionManager;
use sessions::permissions::{self, ReplyDecision};
use std::sync::Arc;
use tauri::{Emitter, Listener, Manager};

type SharedManager = Arc<tokio::sync::Mutex<SessionManager>>;

/// Returned when the session engine never came up (e.g. unreadable DB).
const ENGINE_DOWN: &str = "engine unavailable: session storage failed to start";

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
    request_id: String,
    reply: String,
) -> Result<(), String> {
    // Storage may have failed at startup; answer clearly instead of panicking.
    let manager = app.try_state::<SharedManager>().ok_or(ENGINE_DOWN)?;
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
            let snapshot = {
                let mut m = manager.lock().await;
                m.resolve_local(&p.session_id).await;
                m.snapshot().to_string()
            };
            let _ = app.emit("neko-event", snapshot);
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
async fn get_setting(app: tauri::AppHandle, key: String) -> Result<Option<String>, String> {
    let manager = app.try_state::<SharedManager>().ok_or(ENGINE_DOWN)?;
    let pool = { manager.lock().await.pool() };
    Ok(storage_get(&pool, &key).await)
}

#[tauri::command]
async fn set_setting(app: tauri::AppHandle, key: String, value: String) -> Result<(), String> {
    let manager = app.try_state::<SharedManager>().ok_or(ENGINE_DOWN)?;
    let pool = { manager.lock().await.pool() };
    storage_set(&pool, &key, &value).await;
    Ok(())
}

async fn storage_get(pool: &sqlx::SqlitePool, key: &str) -> Option<String> {
    sessions::storage::get_setting(pool, key)
        .await
        .ok()
        .flatten()
}

async fn storage_set(pool: &sqlx::SqlitePool, key: &str, value: &str) {
    let _ = sessions::storage::set_setting(pool, key, value).await;
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
    let (active_sessions, pool) = {
        let m = manager.lock().await;
        (m.active_count(), m.pool())
    };
    tray::TrayState {
        active_sessions,
        notifications_on: storage_get(&pool, "notifications").await.as_deref() != Some("0"),
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
/// Sync + caller-checked: never takes the manager lock.
fn maybe_notify(
    app: &tauri::AppHandle,
    neko_type: &str,
    payload: &serde_json::Value,
    project: Option<String>,
) {
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
            // Bounded so a flooding sender can't grow memory without limit.
            let (tx, mut rx) = tokio::sync::mpsc::channel::<(String, String)>(512);
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
                    // One lock for the whole cycle: the tokio Mutex is not
                    // reentrant, so notify/tray must be fed from here.
                    let (changed, snapshot, notif_on) = {
                        let mut m = manager.lock().await;
                        let changed = m.apply(&t, sid, &payload).await;
                        let notif_on = m.setting_get("notifications").await.as_deref() != Some("0");
                        (changed, m.snapshot().to_string(), notif_on)
                    };
                    let project = changed
                        .as_ref()
                        .and_then(|s| s.project_name.clone().or_else(|| s.project_path.clone()));
                    if notif_on {
                        maybe_notify(&handle, &t, &payload, project);
                    }
                    let _ = handle.emit("neko-event", snapshot);
                    rebuild_tray(&manager, &handle).await;
                }
            });
            let manage_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match ready_rx.await {
                    Ok(manager) => {
                        // Tray + placement need the manager; build once ready.
                        rebuild_tray(&manager, &manage_handle).await;
                        if let Err(e) = place_top_center(&manage_handle) {
                            tracing::warn!(error = %e, "initial placement failed");
                        }
                        // Stale sweeper: sessions idle >30min go disconnected.
                        let sweep_manager = manager.clone();
                        let sweep_handle = manage_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            let mut tick =
                                tokio::time::interval(std::time::Duration::from_secs(60));
                            loop {
                                tick.tick().await;
                                let (n, snapshot) = {
                                    let mut m = sweep_manager.lock().await;
                                    let n = m.sweep_stale(30 * 60 * 1000).await;
                                    (n, m.snapshot().to_string())
                                };
                                if n > 0 {
                                    let _ = sweep_handle.emit("neko-event", snapshot);
                                    rebuild_tray(&sweep_manager, &sweep_handle).await;
                                }
                            }
                        });
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
                    Err(_) => {
                        tracing::error!("session engine unavailable: storage failed to start");
                    }
                }
            });
            tauri::async_runtime::spawn(async move {
                match ipc::bind(&sock).await {
                    Ok(listener) => {
                        tracing::info!(sock = %sock, "ipc listening");
                        let on_event: ipc::EventCb = Arc::new(move |t, line| {
                            if let Err(e) = tx.try_send((t, line)) {
                                tracing::warn!("ipc channel full, dropping event");
                                let _ = e;
                            }
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
            let pool = { manager.lock().await.pool() };
            let cur = storage_get(&pool, "notifications").await;
            let next = if cur.as_deref() == Some("0") {
                "1"
            } else {
                "0"
            };
            storage_set(&pool, "notifications", next).await;
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
