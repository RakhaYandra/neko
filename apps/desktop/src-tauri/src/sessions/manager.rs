//! Session registry (Phase 3). Owns the state machine, persistence, and the
//! UI snapshot. Unknown session IDs are created on demand as `idle`.

use super::permissions::PendingPermission;
use super::state::{transition, EventKind, Outcome, SessionStatus};
use super::storage::{self, SessionRow};
use sqlx::SqlitePool;
use std::collections::HashMap;

pub struct SessionManager {
    pool: SqlitePool,
    sessions: HashMap<String, SessionRow>,
    pending: HashMap<String, PendingPermission>,
}

impl SessionManager {
    /// Load from DB, recover stale sessions, prune old events.
    pub async fn load(pool: SqlitePool) -> Result<Self, sqlx::Error> {
        storage::mark_all_disconnected(&pool).await?;
        let pruned = storage::prune_events(&pool, 30).await?;
        tracing::info!(pruned, "session events pruned");
        let sessions = storage::load_sessions(&pool)
            .await?
            .into_iter()
            .map(|s| (s.id.clone(), s))
            .collect();
        Ok(Self {
            pool,
            sessions,
            pending: HashMap::new(),
        })
    }

    /// Apply one validated Neko event. Returns the changed session, if any.
    pub async fn apply(
        &mut self,
        neko_type: &str,
        session_id: Option<&str>,
        payload: &serde_json::Value,
    ) -> Option<SessionRow> {
        let sid = session_id?;
        let kind = event_kind(neko_type, payload)?;
        let now = storage::now_ms();
        let row = self.ensure(sid, now).await?;
        match transition(row.status, kind) {
            Outcome::Rejected => {
                tracing::warn!(session = %sid, event = %neko_type, from = %row.status.as_str(), "rejected transition");
                return None;
            }
            Outcome::Unchanged => {
                self.touch(sid, now).await;
            }
            Outcome::Changed(to) => {
                self.set_status(sid, to, now, project_of(neko_type, payload))
                    .await;
            }
        }
        self.record(sid, neko_type, now).await;
        if neko_type == "permission.requested" {
            self.register_pending(sid, payload, now);
        } else if neko_type == "permission.resolved" {
            self.clear_pending(sid, payload);
        }
        self.sessions.get(sid).cloned()
    }

    fn register_pending(&mut self, sid: &str, payload: &serde_json::Value, now: i64) {
        let Some(request_id) = payload.get("requestId").and_then(|v| v.as_str()) else {
            return; // TUI-only session: no reply key, observe-only.
        };
        let action = payload
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let resource = payload
            .get("resource")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        self.pending.insert(
            request_id.to_string(),
            PendingPermission {
                request_id: request_id.to_string(),
                session_id: sid.to_string(),
                action,
                resource,
                asked_at_ms: now,
            },
        );
    }

    fn clear_pending(&mut self, sid: &str, payload: &serde_json::Value) {
        if let Some(id) = payload.get("requestId").and_then(|v| v.as_str()) {
            self.pending.remove(id);
            return;
        }
        self.pending.retain(|_, p| p.session_id != sid);
    }

    /// Take a pending request for an explicit UI reply. Removal is the
    /// single gate: without it no HTTP can fire. Re-insert on failure.
    pub fn take_pending(&mut self, request_id: &str) -> Option<PendingPermission> {
        self.pending.remove(request_id)
    }

    pub fn reinsert_pending(&mut self, p: PendingPermission) {
        self.pending.insert(p.request_id.clone(), p);
    }

    /// Mark the owning session resolved after a successful reply.
    pub async fn resolve_local(&mut self, session_id: &str) {
        let now = storage::now_ms();
        if self.ensure(session_id, now).await.is_none() {
            return;
        }
        if transition(
            self.sessions
                .get(session_id)
                .map(|s| s.status)
                .unwrap_or(SessionStatus::Idle),
            EventKind::PermissionResolved,
        ) == Outcome::Changed(SessionStatus::Working)
        {
            self.set_status(session_id, SessionStatus::Working, now, None)
                .await;
        } else {
            self.touch(session_id, now).await;
        }
        self.record(session_id, "permission.resolved", now).await;
    }

    /// Sessions ordered by recent activity; first is the active one.
    pub fn ordered(&self) -> Vec<&SessionRow> {
        let mut v: Vec<&SessionRow> = self.sessions.values().collect();
        v.sort_by_key(|s| std::cmp::Reverse(s.last_activity_at));
        v
    }

    /// Count of sessions that need attention (tray badge).
    pub fn active_count(&self) -> usize {
        use super::state::SessionStatus as S;
        self.sessions
            .values()
            .filter(|s| matches!(s.status, S::Working | S::ToolRunning | S::WaitingPermission))
            .count()
    }

    pub async fn setting_get(&self, key: &str) -> Option<String> {
        storage::get_setting(&self.pool, key).await.ok().flatten()
    }

    pub async fn setting_set(&self, key: &str, value: &str) {
        let _ = storage::set_setting(&self.pool, key, value).await;
    }

    /// Minimal mirror snapshot for the UI. No payloads, no prompts.
    /// Pending requests ride along so the bubble can render exact keys.
    pub fn snapshot(&self) -> serde_json::Value {
        let sessions: Vec<serde_json::Value> = self
            .ordered()
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "project": s.project_name.clone()
                        .or_else(|| s.project_path.clone())
                        .unwrap_or_else(|| s.id.clone()),
                    "status": s.status.as_str(),
                    "lastActivityAt": s.last_activity_at,
                })
            })
            .collect();
        let mut pending: Vec<&PendingPermission> = self.pending.values().collect();
        pending.sort_by_key(|p| p.asked_at_ms);
        let pending: Vec<serde_json::Value> = pending
            .iter()
            .map(|p| {
                serde_json::json!({
                    "requestId": p.request_id,
                    "sessionId": p.session_id,
                    "action": p.action,
                    "resource": p.resource,
                    "askedAt": p.asked_at_ms,
                })
            })
            .collect();
        serde_json::json!({
            "v": 1,
            "type": "sessions.snapshot",
            "at": storage::now_ms(),
            "sessions": sessions,
            "pending": pending,
        })
    }

    async fn ensure(&mut self, sid: &str, now: i64) -> Option<&SessionRow> {
        if !self.sessions.contains_key(sid) {
            let r = sqlx::query(
                "INSERT INTO sessions (id, status, started_at, last_activity_at, created_at, updated_at)
                 VALUES (?1, 'idle', ?2, ?2, ?2, ?2)",
            )
            .bind(sid)
            .bind(now)
            .execute(&self.pool)
            .await;
            if let Err(e) = r {
                tracing::error!(session = %sid, error = %e, "session insert failed");
                return None;
            }
            self.sessions.insert(
                sid.to_string(),
                SessionRow {
                    id: sid.to_string(),
                    project_name: None,
                    project_path: None,
                    status: SessionStatus::Idle,
                    started_at: now,
                    last_activity_at: now,
                    completed_at: None,
                },
            );
        }
        self.sessions.get(sid)
    }

    async fn touch(&mut self, sid: &str, now: i64) {
        let _ =
            sqlx::query("UPDATE sessions SET last_activity_at = ?2, updated_at = ?2 WHERE id = ?1")
                .bind(sid)
                .bind(now)
                .execute(&self.pool)
                .await;
        if let Some(s) = self.sessions.get_mut(sid) {
            s.last_activity_at = now;
        }
    }

    async fn set_status(
        &mut self,
        sid: &str,
        to: SessionStatus,
        now: i64,
        project: Option<String>,
    ) {
        let completed_at: Option<i64> = (to == SessionStatus::Completed).then_some(now);
        let _ = sqlx::query(
            "UPDATE sessions SET status = ?2, last_activity_at = ?3, updated_at = ?3,
             completed_at = COALESCE(?4, completed_at),
             project_name = COALESCE(?5, project_name) WHERE id = ?1",
        )
        .bind(sid)
        .bind(to.as_str())
        .bind(now)
        .bind(completed_at)
        .bind(project.clone())
        .execute(&self.pool)
        .await;
        if let Some(s) = self.sessions.get_mut(sid) {
            s.status = to;
            s.last_activity_at = now;
            if completed_at.is_some() {
                s.completed_at = completed_at;
            }
            if project.is_some() {
                s.project_name = project;
            }
        }
    }

    /// History keeps type + time only; payloads are never stored.
    async fn record(&self, sid: &str, neko_type: &str, now: i64) {
        let _ = sqlx::query(
            "INSERT INTO session_events (session_id, event_type, created_at) VALUES (?1, ?2, ?3)",
        )
        .bind(sid)
        .bind(neko_type)
        .bind(now)
        .execute(&self.pool)
        .await;
    }
}

fn event_kind(neko_type: &str, payload: &serde_json::Value) -> Option<EventKind> {
    match neko_type {
        "session.created" => Some(EventKind::Created),
        "session.status" => payload
            .get("status")
            .and_then(|s| s.as_str())
            .and_then(SessionStatus::parse)
            .map(EventKind::Status),
        "session.completed" => Some(EventKind::Completed),
        "session.error" => Some(EventKind::Error),
        "session.diff" | "file.edited" | "todo.updated" => Some(EventKind::Activity),
        "tool.started" => Some(EventKind::ToolStarted),
        "tool.completed" => Some(EventKind::ToolCompleted),
        "permission.requested" => Some(EventKind::PermissionRequested),
        "permission.resolved" => Some(EventKind::PermissionResolved),
        _ => None,
    }
}

fn project_of(neko_type: &str, payload: &serde_json::Value) -> Option<String> {
    (neko_type == "session.created")
        .then(|| payload.get("project")?.as_str().map(str::to_string))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn mem_manager() -> SessionManager {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        storage::init_schema(&pool).await.unwrap();
        SessionManager::load(pool).await.unwrap()
    }

    fn payload_status(s: &str) -> serde_json::Value {
        serde_json::json!({ "status": s })
    }

    #[tokio::test]
    async fn tracks_three_sessions_like_pulse_lifeos_shiftbase() {
        let mut m = mem_manager().await;
        m.apply(
            "session.created",
            Some("pulse"),
            &serde_json::json!({"project":"pulse"}),
        )
        .await;
        m.apply("session.status", Some("pulse"), &payload_status("working"))
            .await;
        m.apply(
            "tool.started",
            Some("pulse"),
            &serde_json::json!({"tool":"bash"}),
        )
        .await;
        m.apply(
            "session.created",
            Some("lifeos"),
            &serde_json::json!({"project":"lifeos"}),
        )
        .await;
        // Distinct millis so "most recent first" is deterministic.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        m.apply("session.created", Some("shiftbase"), &serde_json::json!({}))
            .await;
        // Distinct millis so "most recent first" is deterministic.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        m.apply(
            "session.completed",
            Some("shiftbase"),
            &serde_json::json!({}),
        )
        .await;

        let snap = m.snapshot();
        let sessions = snap.get("sessions").unwrap().as_array().unwrap();
        assert_eq!(sessions.len(), 3);
        let by_id = |id: &str| {
            sessions
                .iter()
                .find(|s| s.get("id").unwrap() == id)
                .unwrap()
                .clone()
        };
        assert_eq!(by_id("pulse").get("status").unwrap(), "tool_running");
        assert_eq!(by_id("lifeos").get("status").unwrap(), "idle");
        assert_eq!(by_id("shiftbase").get("status").unwrap(), "completed");
        // Most recent activity first: shiftbase completed last.
        assert_eq!(sessions[0].get("id").unwrap(), "shiftbase");
    }

    #[tokio::test]
    async fn reload_recovers_and_permission_flows() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        storage::init_schema(&pool).await.unwrap();
        let mut m = SessionManager::load(pool.clone()).await.unwrap();
        m.apply("session.created", Some("a"), &serde_json::json!({}))
            .await;
        m.apply("session.status", Some("a"), &payload_status("working"))
            .await;
        m.apply(
            "permission.requested",
            Some("a"),
            &serde_json::json!({"action":"bash"}),
        )
        .await;
        // Simulate restart: working session must come back disconnected.
        let m2 = SessionManager::load(pool).await.unwrap();
        let s = m2.sessions.get("a").unwrap();
        assert_eq!(s.status, SessionStatus::Disconnected);
        assert_eq!(m2.ordered().len(), 1);
    }

    #[tokio::test]
    async fn pending_registry_take_and_snapshot() {
        let mut m = mem_manager().await;
        m.apply("session.created", Some("a"), &serde_json::json!({}))
            .await;
        m.apply(
            "permission.requested",
            Some("a"),
            &serde_json::json!({"action":"bash","resource":"echo *","requestId":"per_1"}),
        )
        .await;
        let snap = m.snapshot();
        let pending = snap.get("pending").unwrap().as_array().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].get("requestId").unwrap(), "per_1");

        // Take gates the reply; second take fails (single-shot).
        let p = m.take_pending("per_1").unwrap();
        assert_eq!(p.session_id, "a");
        assert!(m.take_pending("per_1").is_none());

        // External resolution clears by exact id.
        m.reinsert_pending(p);
        m.apply(
            "permission.resolved",
            Some("a"),
            &serde_json::json!({"decision":"allow","requestId":"per_1"}),
        )
        .await;
        assert!(m
            .snapshot()
            .get("pending")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(m.sessions.get("a").unwrap().status, SessionStatus::Working);
    }
}
