//! Headless IPC listener for debugging and E2E tests (see docs/API-GUIDE.md).
//! Usage: `NEKO_SOCK=/tmp/neko.sock cargo run --example neko-listen`
//! Prints one validated `type` per line; never prints payloads.
use std::sync::Arc;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "neko=info".into()),
        )
        .init();
    let sock = neko_lib::ipc::socket_path();
    let listener = neko_lib::ipc::bind(&sock).await.expect("ipc bind failed");
    eprintln!("listening on {sock}");
    let on_event: neko_lib::ipc::EventCb = Arc::new(|t, _line| println!("EVENT type=\"{t}\""));
    neko_lib::ipc::serve_on(listener, on_event).await;
}
