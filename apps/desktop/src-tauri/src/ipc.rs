//! Unix socket IPC listener (Phase 2 wires this into Tauri events).
//! Protocol: NDJSON, one `{v:1, type, ...}` object per line (see ADR-001/003).

/// Default socket path. Overridable via `NEKO_SOCK` env.
pub fn socket_path() -> String {
    std::env::var("NEKO_SOCK").unwrap_or_else(|_| "/tmp/neko.sock".to_string())
}

/// Protocol version we accept. Must match `@neko/protocol` PROTOCOL_VERSION.
pub const PROTOCOL_VERSION: u8 = 1;

/// Validate one raw line: returns the message `type` on success.
/// Never logs payload contents (may contain paths/commands).
/// Wired to the socket loop in Phase 2.
#[allow(dead_code)]
pub fn validate_line(line: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    if v.get("v") != Some(&serde_json::json!(PROTOCOL_VERSION)) {
        return Err(format!("bad version: {:?}", v.get("v")));
    }
    v.get("type")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "missing type".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_v1_status() {
        let t = validate_line(r#"{"v":1,"type":"session.status","at":1}"#).unwrap();
        assert_eq!(t, "session.status");
    }

    #[test]
    fn rejects_bad_version_and_garbage() {
        assert!(validate_line(r#"{"v":99,"type":"x"}"#).is_err());
        assert!(validate_line("not-json{{{").is_err());
        assert!(validate_line(r#"{"v":1}"#).is_err());
    }
}
