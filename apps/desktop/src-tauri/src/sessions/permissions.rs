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

/// One unresolved question (option picker) from OpenCode. Answers are label
/// selections, never free text in MVP — same single-shot gate as permissions.
#[derive(Debug, Clone)]
pub struct PendingQuestion {
    pub request_id: String,
    pub session_id: String,
    /// Raw `questions` array from the event payload (validated upstream).
    pub questions: serde_json::Value,
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

/// POST selected option labels for one question. `answers` nests per
/// question in order: `[[labels for q0], [labels for q1], …]`.
/// 404 means settled or unknown — same AlreadySettled contract as
/// permissions (drop local copy, never retry).
pub async fn reply_question(
    client: &reqwest::Client,
    base_url: &str,
    request_id: &str,
    answers: &[Vec<String>],
) -> Result<(), ReplyError> {
    let url = format!(
        "{}/question/{request_id}/reply",
        base_url.trim_end_matches('/')
    );
    let res = client
        .post(&url)
        .json(&serde_json::json!({ "answers": answers }))
        .send()
        .await
        .map_err(|e| ReplyError::Unreachable(trim_err(e.to_string())))?;
    if res.status().is_success() {
        return Ok(());
    }
    if res.status().as_u16() == 404 {
        return Err(ReplyError::AlreadySettled);
    }
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    Err(ReplyError::Unreachable(format!(
        "{status}: {}",
        trim_err(body)
    )))
}

/// Reject (dismiss without answering) one question. Same 404 contract.
pub async fn reject_question(
    client: &reqwest::Client,
    base_url: &str,
    request_id: &str,
) -> Result<(), ReplyError> {
    let url = format!(
        "{}/question/{request_id}/reject",
        base_url.trim_end_matches('/')
    );
    let res = client
        .post(&url)
        .send()
        .await
        .map_err(|e| ReplyError::Unreachable(trim_err(e.to_string())))?;
    if res.status().is_success() {
        return Ok(());
    }
    if res.status().as_u16() == 404 {
        return Err(ReplyError::AlreadySettled);
    }
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    Err(ReplyError::Unreachable(format!(
        "{status}: {}",
        trim_err(body)
    )))
}

/// Rehydrate pending questions after a restart. Best-effort, same shape
/// tolerance as `list_pending`: items without id/session are skipped.
pub async fn list_questions(client: &reqwest::Client, base_url: &str) -> Vec<PendingQuestion> {
    let url = format!("{}/question", base_url.trim_end_matches('/'));
    let res = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(error = %trim_err(e.to_string()), "question rehydrate skipped");
            return Vec::new();
        }
    };
    if !res.status().is_success() {
        return Vec::new();
    }
    let items: Vec<serde_json::Value> = res.json().await.unwrap_or_default();
    let now = super::storage::now_ms();
    items
        .iter()
        .filter_map(|v| {
            Some(PendingQuestion {
                request_id: v.get("id")?.as_str()?.to_string(),
                session_id: v.get("sessionID")?.as_str()?.to_string(),
                questions: v
                    .get("questions")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
                asked_at_ms: now,
            })
        })
        .collect()
}
/// Does serve know this session? Used on the 404-reply path to tell a
/// terminal-answered serve ask apart from a TUI-origin ask (which serve can
/// never answer). Unreachable serve reads as known (conservative: keep the
/// old message and the retry entry).
pub async fn session_exists(client: &reqwest::Client, base_url: &str, session_id: &str) -> bool {
    let url = format!("{}/session/{session_id}", base_url.trim_end_matches('/'));
    match client.get(&url).send().await {
        Ok(r) => r.status().is_success(),
        Err(_) => true,
    }
}

fn trim_err(s: String) -> String {
    let mut s = s;
    s.truncate(160);
    s
}

/// Rehydrate pending requests after a restart (Phase 7). Best-effort:
/// any failure yields an empty list, never an error.
pub async fn list_pending(client: &reqwest::Client, base_url: &str) -> Vec<PendingPermission> {
    let url = format!("{}/permission", base_url.trim_end_matches('/'));
    let res = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(error = %trim_err(e.to_string()), "pending rehydrate skipped");
            return Vec::new();
        }
    };
    if !res.status().is_success() {
        return Vec::new();
    }
    let items: Vec<serde_json::Value> = res.json().await.unwrap_or_default();
    let now = super::storage::now_ms();
    items
        .iter()
        .filter_map(|v| {
            Some(PendingPermission {
                request_id: v.get("id")?.as_str()?.to_string(),
                session_id: v.get("sessionID")?.as_str()?.to_string(),
                action: v
                    .get("permission")
                    .and_then(|a| a.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                resource: v
                    .get("patterns")
                    .and_then(|p| p.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str())
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .filter(|s| !s.is_empty()),
                asked_at_ms: now,
            })
        })
        .collect()
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
    async fn session_exists_maps_status() {
        let client = http_client();
        let base = respond_once("200 OK", b"{}").await;
        assert!(session_exists(&client, &base, "ses_1").await);
        let base = respond_once("404 Not Found", b"nope").await;
        assert!(!session_exists(&client, &base, "ses_x").await);
        // Unreachable reads as known (conservative: keep old message).
        assert!(session_exists(&client, "http://127.0.0.1:1", "ses_1").await);
    }

    #[tokio::test]
    async fn question_reply_reject_and_settled_mapping() {
        let client = http_client();
        let base = respond_once("200 OK", b"true").await;
        assert!(
            reply_question(&client, &base, "que_1", &[vec!["bar-widget".to_string()]])
                .await
                .is_ok()
        );

        let base = respond_once("200 OK", b"true").await;
        assert!(reject_question(&client, &base, "que_1").await.is_ok());

        let base = respond_once("404 Not Found", b"nope").await;
        assert_eq!(
            reply_question(&client, &base, "que_x", &[vec!["a".to_string()]]).await,
            Err(ReplyError::AlreadySettled)
        );
        let base = respond_once("404 Not Found", b"nope").await;
        assert_eq!(
            reject_question(&client, &base, "que_x").await,
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
