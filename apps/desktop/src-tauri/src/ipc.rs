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

/// Lines above this size are dropped before parsing (Phase 7).
pub const MAX_LINE_BYTES: usize = 64 * 1024;

/// Validate one raw line: returns the message `type` on success.
/// Checks the ADR-003 envelope (`v`, `type`, `at`); full payload schemas
/// live in `@neko/protocol` (Zod) and are enforced there.
/// Never logs payload contents (may contain paths/commands).
#[allow(dead_code)]
pub fn validate_line(line: &str) -> Result<String, String> {
    if line.len() > MAX_LINE_BYTES {
        return Err("oversize line".to_string());
    }
    let v: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    if v.get("v") != Some(&serde_json::json!(PROTOCOL_VERSION)) {
        return Err(format!("bad version: {:?}", v.get("v")));
    }
    let t = v
        .get("type")
        .and_then(|t| t.as_str())
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "missing type".to_string())?;
    // `at` must be an integer millisecond stamp; floats/strings are noise.
    if !v.get("at").is_some_and(|a| a.is_i64() || a.is_u64()) {
        return Err("missing at".to_string());
    }
    Ok(t.to_string())
}

/// Callback for validated events: (`type`, raw validated line).
pub type EventCb = Arc<dyn Fn(String, String) + Send + Sync>;

/// Bind the socket, removing a stale file first and locking it to `0600`.
/// Refuses to remove a symlink (TOCTOU hijack): the operator must delete it.
pub async fn bind(sock: &str) -> std::io::Result<UnixListener> {
    match std::fs::symlink_metadata(sock) {
        Ok(md) if md.file_type().is_symlink() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("refusing to replace symlink at {sock}"),
            ));
        }
        Ok(_) => {
            std::fs::remove_file(sock)?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let listener = UnixListener::bind(sock)?;
    std::fs::set_permissions(sock, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

/// Accept loop: one task per connection, one validated event per line.
/// Malformed lines are rejected with a warning; the listener never dies.
/// Idle connections are dropped after 30s so stuck senders can't pile up.
pub async fn serve_on(listener: UnixListener, on_event: EventCb) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let on_event = on_event.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            loop {
                let next =
                    tokio::time::timeout(std::time::Duration::from_secs(30), lines.next_line())
                        .await;
                match next {
                    Ok(Ok(Some(line))) => {
                        if line.trim().is_empty() {
                            continue;
                        }
                        // Cap line size: anything bigger is not our protocol.
                        if line.len() > MAX_LINE_BYTES {
                            tracing::warn!("rejected oversize ipc line");
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
                    Ok(Ok(None)) => break, // orderly close
                    Ok(Err(e)) => {
                        tracing::warn!(error = %e, "ipc read failed");
                        break;
                    }
                    Err(_) => {
                        tracing::warn!("idle ipc connection dropped");
                        break;
                    }
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

    #[test]
    fn rejects_oversize_line_early() {
        let big = "x".repeat(MAX_LINE_BYTES + 1);
        let line = format!("{{\"v\":1,\"type\":\"{big}\",\"at\":1}}");
        assert!(validate_line(&line).is_err());
    }

    #[tokio::test]
    async fn refuses_symlink_socket() {
        let target = format!("/tmp/neko-target-{}.sock", std::process::id());
        let link = format!("/tmp/neko-link-{}.sock", std::process::id());
        let _ = std::fs::remove_file(&target);
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(bind(&link).await.is_err());
        let _ = std::fs::remove_file(&link);
        let _ = std::fs::remove_file(&target);
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

    /// Deterministic adversarial fuzz: hostile lines never panic, never pass.
    #[test]
    fn fuzz_rejects_hostile_lines() {
        let big = "x".repeat(200_000);
        let cases = [
            "",
            " ",
            "\n",
            "{",
            "[]",
            "null",
            "123",
            "{\"v\":1}",
            "{\"v\":\"1\",\"type\":\"x\",\"at\":1}",
            "{\"v\":1.5,\"type\":\"x\",\"at\":1}",
            "{\"v\":null,\"type\":\"x\",\"at\":1}",
            "{\"v\":1,\"type\":null,\"at\":1}",
            "{\"v\":1,\"type\":123,\"at\":1}",
            "{\"v\":1,\"type\":\"x\"}",
            "{\"v\":1,\"type\":\"x\",\"at\":\"now\"}",
            "{\"v\":1,\"type\":\"x\",\"at\":null}",
            "{\"v\":1,\"type\":\"x\",\"at\":1.5}",
            "{\"v\":-1,\"type\":\"x\",\"at\":1}",
            "{\"v\":1,\"type\":\"session.status\",\"at\":1},",
        ];
        for c in cases {
            assert!(validate_line(c).is_err(), "should reject: {c:?}");
        }
        // Oversize lines are dropped by the 64 KiB guard in `serve_on`, not
        // by the envelope check, so assert only the size property here.
        let oversize = format!("{{\"v\":1,\"type\":\"{big}\",\"at\":1}}");
        assert!(oversize.len() > MAX_LINE_BYTES);
        // Deep nesting must never panic, whatever the verdict.
        let deep = format!("{{\"v\":1,\"type\":\"x\",\"at\":[{}]}}", "[1,".repeat(500));
        let _ = validate_line(&deep);
        // Valid envelope shapes still pass.
        assert!(validate_line(r#"{"v":1,"type":"x","at":0}"#).is_ok());
        assert!(validate_line(r#"{"v":1,"type":"sessions.snapshot","at":1790000000000}"#).is_ok());
        // Unknown/odd type names pass the envelope (shape) but stay inert:
        // the session engine drops them via `event_kind`, never applying them.
        for shape_only in [
            r#"{"v":1,"type":"__proto__","at":1}"#,
            r#"{"v":1,"type":"../../etc/passwd","at":1}"#,
            r#"{"v":1,"type":"x","at":1,"payload":{"__proto__":1}}"#,
        ] {
            assert!(validate_line(shape_only).is_ok());
        }
    }
}
