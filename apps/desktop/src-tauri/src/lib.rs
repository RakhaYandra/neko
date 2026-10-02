pub mod ipc;
pub mod sessions;

use sessions::manager::SessionManager;
use sessions::permissions::{self, ReplyDecision};
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

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
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, reply_permission])
        .setup(|app| {
            let handle = app.handle().clone();
            let sock = ipc::socket_path();
            // Consumer: validated events -> session engine -> UI snapshot.
            // Storage failure only kills this task (logged); the app survives.
            // The manager is shared with the reply_permission command.
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
                    m.apply(&t, sid, &payload).await;
                    let _ = handle.emit("neko-event", m.snapshot().to_string());
                }
            });
            let manage_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(manager) = ready_rx.await {
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
