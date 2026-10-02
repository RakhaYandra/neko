# DATABASE-GUIDE.md — Neko SQLite (Phase 3)

Engine: SQLx (`sqlite` bundled — no system SQLite needed, CI-safe),
runtime query API only (no `query!` macros, no `DATABASE_URL` at build).

## Location

- Default: `$XDG_DATA_HOME/neko/neko.db`, else `~/.local/share/neko/neko.db`.
- Override: `NEKO_DB=/tmp/x.db` (tests, debugging). Parent dirs auto-created.
- Schema is inline `CREATE TABLE IF NOT EXISTS` in
  `src-tauri/src/sessions/storage.rs` (no migration framework in MVP).

## Tables

`sessions(id, project_name, project_path, status, started_at,
last_activity_at, completed_at, created_at, updated_at)` — status is one of
`disconnected|idle|working|tool_running|waiting_permission|completed|error`.

`session_events(id, session_id, event_type, payload, created_at)` — history
keeps **type + time only**; `payload` is always `'{}'`.

`settings(key, value, updated_at)` — reserved, unused in Phase 3.

## What is NEVER stored

Prompts, source code, tool output, secrets. `session.error` messages stay in
logs/memory only. Event payloads (commands, paths) are dropped before insert.

## Lifecycle

- Startup: `mark_all_disconnected()` (stale recovery) + `prune_events(30)`.
- Session rows are kept indefinitely (small); only events are pruned.
- Unknown session IDs are created on demand as `idle`, then transitioned.
