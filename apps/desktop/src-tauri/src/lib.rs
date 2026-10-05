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

/// Answer a pending permission request. Two doors, one core: the UI button
/// (Tauri command) and `neko reply` (single-instance CLI handoff) both run
/// `answer_permission`. Strict decision parse, single registry take, serve
/// HTTP. Unknown/settled requests and unreachable serve return Err, never act.
#[tauri::command]
async fn reply_permission(
    app: tauri::AppHandle,
    request_id: String,
    reply: String,
) -> Result<(), String> {
    // Storage may have failed at startup; answer clearly instead of panicking.
    let manager = app.try_state::<SharedManager>().ok_or(ENGINE_DOWN)?;
    let (result, snapshot) = answer_permission(&manager, &request_id, &reply).await;
    if let Some(s) = snapshot {
        publish_snapshot(&app, &s);
    }
    result
}

/// Shared reply core. Returns the command result plus an optional fresh
/// snapshot the caller must publish (every settled path resyncs the UI;
/// only transport failures keep the entry for retry and publish nothing).
async fn answer_permission(
    manager: &SharedManager,
    request_id: &str,
    reply: &str,
) -> (Result<(), String>, Option<String>) {
    let decision = match ReplyDecision::parse(reply) {
        Some(d) => d,
        None => return (Err("invalid reply".to_string()), None),
    };
    let base = match permissions::serve_url() {
        Some(b) => b,
        None => return (Err(permissions::ReplyError::NoServeUrl.to_string()), None),
    };
    let http = permissions::http_client();
    let pending = {
        let mut m = manager.lock().await;
        m.take_pending(request_id)
    };
    let Some(p) = pending else {
        // Self-heal: the UI is ahead of the engine. Push the truth so dead
        // buttons vanish instead of persisting until the next event.
        let snapshot = manager.lock().await.snapshot().to_string();
        return (
            Err("unknown or already settled (stale view?)".to_string()),
            Some(snapshot),
        );
    };
    match permissions::reply(&http, &base, &p.request_id, decision).await {
        Ok(()) => {
            tracing::info!(request = %p.request_id, reply, "permission replied");
            let snapshot = {
                let mut m = manager.lock().await;
                m.resolve_local(&p.session_id).await;
                m.snapshot().to_string()
            };
            (Ok(()), Some(snapshot))
        }
        Err(e) => {
            // Transport problems: entry restored, UI untouched (retry valid).
            if e != permissions::ReplyError::AlreadySettled {
                manager.lock().await.reinsert_pending(p);
                return (Err(e.to_string()), None);
            }
            // Settled upstream. TUI-origin asks are unknown to serve and can
            // never be answered from Neko — drop and yield to the terminal.
            if !permissions::session_exists(&http, &base, &p.session_id).await {
                let snapshot = manager.lock().await.snapshot().to_string();
                return (
                    Err(
                        "answer in the terminal — this ask comes from a terminal session"
                            .to_string(),
                    ),
                    Some(snapshot),
                );
            }
            // Known session, request gone (answered elsewhere or silently
            // dropped on interrupt): drop our copy and push the truth so the
            // bubble yields instead of showing dead buttons.
            let snapshot = manager.lock().await.snapshot().to_string();
            (
                Err("no longer pending — settled or interrupted elsewhere".to_string()),
                Some(snapshot),
            )
        }
    }
}

/// Publish one snapshot to the UI and mirror it to the status file QML
/// polls. Emitting and persisting stay paired: every UI truth update is a
/// widget truth update too.
fn publish_snapshot(app: &tauri::AppHandle, snapshot: &str) {
    let _ = app.emit("neko-event", snapshot);
    persist_status(snapshot);
}

/// Mirror the latest snapshot for external readers (Omarchy widget).
/// Best-effort: a failed write is logged, never fatal.
fn persist_status(snapshot: &str) {
    let path = ipc::status_path();
    if let Err(e) = std::fs::write(&path, snapshot) {
        tracing::warn!(path = %path, error = %e, "status file write failed");
    }
}

/// CLI subcommand parsed from second-process argv (single-instance handoff).
/// `neko` with no (recognized) args just focuses the running window.
#[derive(Debug, PartialEq, Eq)]
enum CliCommand {
    ReplyPermission { request_id: String, reply: String },
}

fn parse_cli_args(args: &[String]) -> Option<CliCommand> {
    let mut it = args.iter().skip(1);
    match it.next().map(String::as_str) {
        Some("reply") => match (it.next(), it.next(), it.next()) {
            (Some(kind), Some(id), Some(dec))
                if kind == "permission"
                    && it.next().is_none()
                    && ReplyDecision::parse(dec).is_some() =>
            {
                Some(CliCommand::ReplyPermission {
                    request_id: id.to_string(),
                    reply: dec.to_string(),
                })
            }
            _ => None,
        },
        _ => None,
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

/// Trailing-edge coalescer for tray rebuilds on the hot event path.
/// The event loop fires per IPC message; bursts (tool started/completed +
/// status) would otherwise rebuild the whole menu once per event.
/// Toggle/sweep/init paths bypass this and call `rebuild_tray` directly.
#[derive(Debug, Default)]
struct TrayDebouncer {
    version: u64,
}

impl TrayDebouncer {
    fn trigger(&mut self) -> u64 {
        self.version += 1;
        self.version
    }

    fn should_run(&self, v: u64) -> bool {
        self.version == v
    }
}

type SharedDebouncer = Arc<tokio::sync::Mutex<TrayDebouncer>>;

async fn request_tray_rebuild(
    deb: &SharedDebouncer,
    manager: &SharedManager,
    app: &tauri::AppHandle,
) {
    let v = deb.lock().await.trigger();
    let deb = deb.clone();
    let manager = manager.clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if deb.lock().await.should_run(v) {
            rebuild_tray(&manager, &app).await;
        }
    });
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
        // First: a second launch hands its args to the running instance and
        // exits before setup, so it can never steal the socket, the tray or
        // the global shortcut (previously it panicked on the shortcut).
        // Recognized CLI (`neko reply permission <id> <decision>`) is
        // executed here; anything else just focuses the companion window.
        // Fire-and-forget by design (QML use): verify via the status file.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            match parse_cli_args(&args) {
                Some(CliCommand::ReplyPermission { request_id, reply }) => {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        match app.try_state::<SharedManager>() {
                            Some(manager) => {
                                let (result, snapshot) =
                                    answer_permission(&manager, &request_id, &reply).await;
                                if let Some(s) = snapshot {
                                    publish_snapshot(&app, &s);
                                }
                                match result {
                                    Ok(()) => tracing::info!(request = %request_id, "cli reply ok"),
                                    Err(e) => tracing::warn!(request = %request_id, error = %e, "cli reply failed"),
                                }
                            }
                            None => tracing::warn!("cli reply: engine unavailable"),
                        }
                    });
                }
                None => {
                    if let Some(w) = app.get_webview_window("companion") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
            }
        }))
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
            let tray_deb: SharedDebouncer =
                Arc::new(tokio::sync::Mutex::new(TrayDebouncer::default()));
            let tray_deb_consumer = tray_deb.clone();
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
                    publish_snapshot(&handle, &snapshot);
                    request_tray_rebuild(&tray_deb_consumer, &manager, &handle).await;
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
                                    publish_snapshot(&sweep_handle, &snapshot);
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
                        manage_handle.manage(manager.clone());
                        // Fresh full snapshot on startup: any UI holding stale
                        // state (surviving frontend, second window) resyncs
                        // immediately instead of showing dead buttons until
                        // the next event. TUI-origin pendings lost on restart
                        // correctly disappear — they are unanswerable now.
                        {
                            let m = manager.lock().await;
                            publish_snapshot(&manage_handle, &m.snapshot().to_string());
                        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debouncer_coalesces_burst_to_last() {
        let mut d = TrayDebouncer::default();
        let mut last = 0;
        for _ in 0..10 {
            last = d.trigger();
        }
        // Only the newest version may run; older sleeps stay inert.
        assert!(d.should_run(last));
        assert!(!d.should_run(last - 1));
        assert!(!d.should_run(1));
    }

    fn argv(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn cli_parses_reply_strictly() {
        assert_eq!(
            parse_cli_args(&argv(&["neko", "reply", "permission", "per_1", "once"])),
            Some(CliCommand::ReplyPermission {
                request_id: "per_1".to_string(),
                reply: "once".to_string(),
            })
        );
        // Bare launch focuses the window.
        assert_eq!(parse_cli_args(&argv(&["neko"])), None);
        // Unknown subcommand focuses the window.
        assert_eq!(parse_cli_args(&argv(&["neko", "dance"])), None);
        // Wrong kind, bad decision, or trailing args: ignored.
        assert_eq!(
            parse_cli_args(&argv(&["neko", "reply", "question", "q_1", "once"])),
            None
        );
        assert_eq!(
            parse_cli_args(&argv(&["neko", "reply", "permission", "per_1", "maybe"])),
            None
        );
        assert_eq!(
            parse_cli_args(&argv(&[
                "neko",
                "reply",
                "permission",
                "per_1",
                "once",
                "x"
            ])),
            None
        );
        assert_eq!(
            parse_cli_args(&argv(&["neko", "reply", "permission"])),
            None
        );
    }

    #[test]
    fn status_file_mirrors_snapshot() {
        let path = format!("/tmp/neko-status-test-{}.json", std::process::id());
        let saved = std::env::var("NEKO_STATUS").ok();
        std::env::set_var("NEKO_STATUS", &path);
        persist_status(r#"{"v":1,"type":"sessions.snapshot"}"#);
        let back = std::fs::read_to_string(&path).unwrap();
        assert_eq!(back, r#"{"v":1,"type":"sessions.snapshot"}"#);
        let _ = std::fs::remove_file(&path);
        match saved {
            Some(v) => std::env::set_var("NEKO_STATUS", v),
            None => std::env::remove_var("NEKO_STATUS"),
        }
    }
}
