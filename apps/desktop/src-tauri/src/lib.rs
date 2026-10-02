pub mod ipc;
pub mod sessions;

use sessions::manager::SessionManager;
use std::sync::Arc;
use tauri::Emitter;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
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
        .invoke_handler(tauri::generate_handler![greet])
        .setup(|app| {
            let handle = app.handle().clone();
            let sock = ipc::socket_path();
            // Consumer: validated events -> session engine -> UI snapshot.
            // Storage failure only kills this task (logged); the app survives.
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, String)>();
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
