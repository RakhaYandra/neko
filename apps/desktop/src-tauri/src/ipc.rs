//! Unix socket IPC listener (Phase 2: wired into Tauri events).
//! Protocol: NDJSON, one `{v:1, type, ...}` object per line (see ADR-001/003).

use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixListener;

/// Default socket path. Overridable via `NEKO_SOCK` env.
pub fn socket_path() -> String {
    std::env::var("NEKO_SOCK").unwrap_or_else(|_| "/tmp/neko.sock".to_string())
}

/// Protocol version we accept. Must match `@neko/protocol` PROTOCOL_VERSION.
pub const PROTOCOL_VERSION: u8 = 1;

/// Validate one raw line: returns the message `type` on success.
/// Checks the ADR-003 envelope (`v`, `type`, `at`); full payload schemas
/// live in `@neko/protocol` (Zod) and are enforced there.
/// Never logs payload contents (may contain paths/commands).
#[allow(dead_code)]
pub fn validate_line(line: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    if v.get("v") != Some(&serde_json::json!(PROTOCOL_VERSION)) {
        return Err(format!("bad version: {:?}", v.get("v")));
    }
    let t = v
        .get("type")
        .and_then(|t| t.as_str())
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "missing type".to_string())?;
    if !v.get("at").is_some_and(|a| a.is_number()) {
        return Err("missing at".to_string());
    }
    Ok(t.to_string())
}

/// Callback for validated events: (`type`, raw validated line).
pub type EventCb = Arc<dyn Fn(String, String) + Send + Sync>;

/// Bind the socket, removing a stale file first and locking it to `0600`.
pub async fn bind(sock: &str) -> std::io::Result<UnixListener> {
    let _ = std::fs::remove_file(sock);
    let listener = UnixListener::bind(sock)?;
    std::fs::set_permissions(sock, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

/// Accept loop: one task per connection, one validated event per line.
/// Malformed lines are rejected with a warning; the listener never dies.
pub async fn serve_on(listener: UnixListener, on_event: EventCb) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let on_event = on_event.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                match validate_line(&line) {
                    Ok(t) => {
                        tracing::info!(msg_type = %t, "neko event");
                        on_event(t, line);
                    }
                    Err(e) => tracing::warn!(error = %e, "rejected ipc line"),
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn accepts_v1_status() {
        let t = validate_line(r#"{"v":1,"type":"session.status","at":1}"#).unwrap();
        assert_eq!(t, "session.status");
    }

    #[test]
    fn rejects_bad_version_and_garbage() {
        assert!(validate_line(r#"{"v":99,"type":"x","at":1}"#).is_err());
        assert!(validate_line("not-json{{{").is_err());
        assert!(validate_line(r#"{"v":1}"#).is_err());
    }

    #[test]
    fn rejects_missing_at_and_empty_type() {
        assert!(validate_line(r#"{"v":1,"type":"session.status"}"#).is_err());
        assert!(validate_line(r#"{"v":1,"type":"","at":1}"#).is_err());
    }

    #[tokio::test]
    async fn socket_roundtrip_delivers_valid_line() {
        use tokio::io::AsyncWriteExt;
        let sock = format!("/tmp/neko-test-{}.sock", std::process::id());
        let listener = bind(&sock).await.unwrap();
        let mode = std::fs::metadata(&sock).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let got: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let cb: EventCb = {
            let got = got.clone();
            Arc::new(move |t, line| got.lock().unwrap().push((t, line)))
        };
        let _srv = tokio::spawn(serve_on(listener, cb));
        let mut cli = tokio::net::UnixStream::connect(&sock).await.unwrap();
        cli.write_all(b"{\"v\":1,\"type\":\"session.status\",\"at\":1}\n")
            .await
            .unwrap();
        cli.write_all(b"{\"v\":99,\"type\":\"x\",\"at\":1}\nnot-json{{{\n")
            .await
            .unwrap();
        cli.shutdown().await.unwrap();
        for _ in 0..50 {
            if !got.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let got = got.lock().unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "session.status");
        let _ = std::fs::remove_file(&sock);
    }
}
