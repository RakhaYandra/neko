//! Pending permission registry + serve reply (Phase 5, ADR-004).
//! Replies originate ONLY from explicit UI clicks via the
//! `reply_permission` Tauri command. No silent or automatic approvals exist.

use std::time::Duration;

/// One unresolved permission request from OpenCode.
#[derive(Debug, Clone)]
pub struct PendingPermission {
    pub request_id: String,
    pub session_id: String,
    pub action: String,
    pub resource: Option<String>,
    pub asked_at_ms: i64,
}

/// Strict reply vocabulary. Anything else is rejected before any HTTP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyDecision {
    Once,
    Always,
    Deny,
}

impl ReplyDecision {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "once" => Some(ReplyDecision::Once),
            "always" => Some(ReplyDecision::Always),
            "deny" => Some(ReplyDecision::Deny),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ReplyDecision::Once => "once",
            ReplyDecision::Always => "always",
            ReplyDecision::Deny => "reject",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReplyError {
    /// No serve address configured; TUI-only sessions land here.
    NoServeUrl,
    /// Serve unreachable or bad response (body truncated in message).
    Unreachable(String),
    /// Request already settled elsewhere (e.g. answered in the terminal).
    AlreadySettled,
}

impl std::fmt::Display for ReplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplyError::NoServeUrl => {
                write!(f, "no OpenCode serve connection (set NEKO_OPENCODE_URL)")
            }
            ReplyError::Unreachable(e) => write!(f, "serve unreachable: {e}"),
            ReplyError::AlreadySettled => write!(f, "already answered elsewhere"),
        }
    }
}

/// Serve base URL from env only. No port scanning, no guessed defaults.
pub fn serve_url() -> Option<String> {
    std::env::var("NEKO_OPENCODE_URL")
        .ok()
        .map(|u| u.trim_end_matches('/').to_string())
        .filter(|u| !u.is_empty())
}

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("reqwest client builds")
}

/// POST the decision to an explicit serve base URL. `message` is always
/// omitted in MVP. The caller resolves the URL (env in production).
pub async fn reply(
    client: &reqwest::Client,
    base_url: &str,
    request_id: &str,
    decision: ReplyDecision,
) -> Result<(), ReplyError> {
    let url = format!(
        "{}/permission/{request_id}/reply",
        base_url.trim_end_matches('/')
    );
    let res = client
        .post(&url)
        .json(&serde_json::json!({ "reply": decision.as_str() }))
        .send()
        .await
        .map_err(|e| ReplyError::Unreachable(trim_err(e.to_string())))?;
    if res.status().is_success() {
        return Ok(());
    }
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if status.as_u16() == 404 && body.contains("PermissionNotFound") {
        return Err(ReplyError::AlreadySettled);
    }
    Err(ReplyError::Unreachable(format!(
        "{status}: {}",
        trim_err(body)
    )))
}

fn trim_err(s: String) -> String {
    let mut s = s;
    s.truncate(160);
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn decision_parses_strictly() {
        assert_eq!(ReplyDecision::parse("once"), Some(ReplyDecision::Once));
        assert_eq!(ReplyDecision::parse("always"), Some(ReplyDecision::Always));
        assert_eq!(ReplyDecision::parse("deny"), Some(ReplyDecision::Deny));
        assert_eq!(ReplyDecision::parse("allow"), None);
        assert_eq!(ReplyDecision::parse(""), None);
        assert_eq!(ReplyDecision::Deny.as_str(), "reject");
    }

    async fn respond_once(status: &str, body: &'static [u8]) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let status = status.to_string();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf).await;
            let head = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            s.write_all(head.as_bytes()).await.unwrap();
            s.write_all(body).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn reply_ok_and_settled_mapping() {
        let base = respond_once("200 OK", b"true").await;
        let client = http_client();
        assert!(reply(&client, &base, "per_1", ReplyDecision::Once)
            .await
            .is_ok());

        let base = respond_once("404 Not Found", b"{\"_tag\":\"PermissionNotFoundError\"}").await;
        assert_eq!(
            reply(&client, &base, "per_x", ReplyDecision::Deny).await,
            Err(ReplyError::AlreadySettled)
        );
    }

    #[tokio::test]
    async fn reply_unreachable_is_an_error() {
        let client = http_client();
        let err = reply(&client, "http://127.0.0.1:1", "per_1", ReplyDecision::Once)
            .await
            .unwrap_err();
        assert!(matches!(err, ReplyError::Unreachable(_)));
    }
}
