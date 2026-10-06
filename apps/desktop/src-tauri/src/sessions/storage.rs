//! SQLite storage (Phase 3). XDG location, inline schema, 30-day prune.
//! Never stores prompts, source code, or tool output — only metadata.

use super::state::SessionStatus;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

/// Resolve the DB path: `NEKO_DB` wins (tests), else XDG data dir.
pub fn db_path() -> String {
    if let Ok(p) = std::env::var("NEKO_DB") {
        return p;
    }
    let base = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{home}/.local/share")
    });
    format!("{base}/neko/neko.db")
}

/// Open (creating) the pool and ensure the schema exists.
/// WAL + busy timeout keep single-process concurrency boring.
/// Refuses to fall back to world-writable /tmp when neither NEKO_DB,
/// XDG_DATA_HOME nor HOME is set — fail closed instead of scattering DB.
pub async fn open() -> Result<SqlitePool, sqlx::Error> {
    if std::env::var("NEKO_DB").is_err()
        && std::env::var("XDG_DATA_HOME").is_err()
        && std::env::var("HOME").is_err()
    {
        return Err(sqlx::Error::Configuration(
            "refusing to use /tmp fallback: set HOME, XDG_DATA_HOME or NEKO_DB".into(),
        ));
    }
    let path = db_path();
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{path}"))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(5));
    let pool = SqlitePool::connect_with(opts).await?;
    sqlx::query("PRAGMA synchronous = NORMAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;
    init_schema(&pool).await?;
    Ok(pool)
}

pub async fn init_schema(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            project_name TEXT,
            project_path TEXT,
            status TEXT NOT NULL DEFAULT 'idle',
            started_at INTEGER NOT NULL,
            last_activity_at INTEGER NOT NULL,
            completed_at INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS session_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            payload TEXT NOT NULL DEFAULT '{}',
            created_at INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_session_events_session
         ON session_events (session_id, created_at)",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL DEFAULT '',
            updated_at INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Delete events older than `days`. Session rows are kept (small).
pub async fn prune_events(pool: &SqlitePool, days: i64) -> Result<u64, sqlx::Error> {
    let now_ms = now_ms();
    let cutoff = now_ms - days * 86_400_000;
    let r = sqlx::query("DELETE FROM session_events WHERE created_at < ?")
        .bind(cutoff)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// Delete terminal sessions (completed/error/disconnected) older than `days`
/// plus their events. Returns (sessions, events) removed.
pub async fn prune_terminal(pool: &SqlitePool, days: i64) -> Result<(u64, u64), sqlx::Error> {
    let cutoff = now_ms() - days * 86_400_000;
    let ev = sqlx::query(
        "DELETE FROM session_events WHERE session_id IN
         (SELECT id FROM sessions WHERE status IN ('completed', 'error', 'disconnected')
          AND last_activity_at < ?)",
    )
    .bind(cutoff)
    .execute(pool)
    .await?
    .rows_affected();
    let se = sqlx::query(
        "DELETE FROM sessions WHERE status IN ('completed', 'error', 'disconnected')
         AND last_activity_at < ?",
    )
    .bind(cutoff)
    .execute(pool)
    .await?
    .rows_affected();
    if se > 0 {
        let _ = sqlx::query("VACUUM").execute(pool).await;
    }
    Ok((se, ev))
}

/// Delete ALL terminal sessions (completed/error/disconnected) regardless
/// of age, plus their events. Explicit user action (Clear button), unlike
/// the 90-day `prune_terminal`. Returns (sessions, events) removed.
pub async fn clear_disconnected(pool: &SqlitePool) -> Result<(u64, u64), sqlx::Error> {
    let ev = sqlx::query(
        "DELETE FROM session_events WHERE session_id IN
         (SELECT id FROM sessions WHERE status IN ('completed', 'error', 'disconnected'))",
    )
    .execute(pool)
    .await?
    .rows_affected();
    let se =
        sqlx::query("DELETE FROM sessions WHERE status IN ('completed', 'error', 'disconnected')")
            .execute(pool)
            .await?
            .rows_affected();
    if se > 0 {
        let _ = sqlx::query("VACUUM").execute(pool).await;
    }
    Ok((se, ev))
}

/// Mark every non-terminal session disconnected (startup recovery).
pub async fn mark_all_disconnected(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE sessions SET status = 'disconnected', updated_at = ?
         WHERE status NOT IN ('disconnected', 'completed', 'error')",
    )
    .bind(now_ms())
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Settings store (Phase 6). Plain string values; callers interpret.
pub async fn get_setting(pool: &SqlitePool, key: &str) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
}

pub async fn set_setting(pool: &SqlitePool, key: &str, value: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3",
    )
    .bind(key)
    .bind(value)
    .bind(now_ms())
    .execute(pool)
    .await?;
    Ok(())
}

/// One session row.
#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: String,
    pub project_name: Option<String>,
    pub project_path: Option<String>,
    pub status: SessionStatus,
    pub started_at: i64,
    pub last_activity_at: i64,
    pub completed_at: Option<i64>,
}

pub async fn load_sessions(pool: &SqlitePool) -> Result<Vec<SessionRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, project_name, project_path, status, started_at,
                last_activity_at, completed_at
         FROM sessions ORDER BY last_activity_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .filter_map(|r| {
            Some(SessionRow {
                id: r.get::<String, _>("id"),
                project_name: r.get::<Option<String>, _>("project_name"),
                project_path: r.get::<Option<String>, _>("project_path"),
                status: SessionStatus::parse(r.get::<String, _>("status").as_str())?,
                started_at: r.get::<i64, _>("started_at"),
                last_activity_at: r.get::<i64, _>("last_activity_at"),
                completed_at: r.get::<Option<i64>, _>("completed_at"),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn mem_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_schema(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn schema_prune_and_recovery() {
        let pool = mem_pool().await;
        let now = now_ms();
        sqlx::query(
            "INSERT INTO sessions (id, status, started_at, last_activity_at, created_at, updated_at)
             VALUES ('a', 'working', ?1, ?1, ?1, ?1)",
        )
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_events (session_id, event_type, created_at)
             VALUES ('a', 'tool.started', ?1)",
        )
        .bind(now - 31 * 86_400_000)
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(prune_events(&pool, 30).await.unwrap(), 1);
        assert_eq!(mark_all_disconnected(&pool).await.unwrap(), 1);
        let sessions = load_sessions(&pool).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].status, SessionStatus::Disconnected);
    }

    #[tokio::test]
    async fn clear_disconnected_keeps_active() {
        let pool = mem_pool().await;
        let now = now_ms();
        for (id, status) in [("a", "working"), ("b", "completed"), ("c", "error")] {
            sqlx::query(
                "INSERT INTO sessions (id, status, started_at, last_activity_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?3, ?3, ?3)",
            )
            .bind(id)
            .bind(status)
            .bind(now)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO session_events (session_id, event_type, created_at)
                 VALUES (?1, 'tool.started', ?2)",
            )
            .bind(id)
            .bind(now)
            .execute(&pool)
            .await
            .unwrap();
        }
        let (se, ev) = clear_disconnected(&pool).await.unwrap();
        assert_eq!(se, 2);
        assert_eq!(ev, 2);
        let sessions = load_sessions(&pool).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "a");
    }
}
