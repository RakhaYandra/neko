# RELIABILITY.md — Neko (Phase 7)

Measured on this machine (Omarchy, Hyprland 0.56.2 Wayland, opencode 1.18.32,
`tauri dev` build). Numbers are observations, not promises.

## Guarantees (verified)

| Failure | Result | How |
| --- | --- | --- |
| Neko killed | OpenCode keeps working | live: `POST /session` answered after `kill -9` |
| OpenCode restarted | events flow again without Neko restart | live: new session arrived post-restart |
| Socket disconnected / stale file | rebound on next start, mode still `0600` | Phase 0 + Phase 7 runs |
| Garbage on socket | 6 hostile lines rejected, next valid line applied | live: `after-junk` landed |
| Neko restarted | sessions recovered from SQLite, pending re-adopted from serve | live: previous session `completed` survived |
| Serve dies while replying | command errors, entry restored for retry | unit + Phase 5 live |
| DB unreadable | app stays alive, logs `storage open failed`, commands answer `engine unavailable` | live: corrupted file |
| Shortcut already taken | logs `shortcut unavailable`, app continues | code path, not exercised |

## Measured budget

| Metric | Observed |
| --- | --- |
| Idle CPU (30 s, idle companion) | 0.93 % |
| RSS | 237–259 MB (dev build; WebKitGTK + Vite) |
| Threads | 43–44 |
| SQLite file | 4 KB after 4 sessions / 28 events |
| Snapshot emit | synchronous, no event dropped in a 100-event burst (unit) |

RSS is dominated by WebKitGTK in `tauri dev`; release build is expected lower
but was not measured separately.

## Hardening added in Phase 7

- Pending permissions rehydrate from `GET {serve}/permission` at startup, so a
  restart does not orphan a request. No serve URL = silent no-op.
- `sweep_stale` every 60 s: non-terminal sessions idle > 30 min become
  `disconnected` (silent OpenCode death no longer shows "working" forever).
- `prune_terminal(90 days)` removes old terminal sessions plus events, then
  `VACUUM`. `prune_events(30 days)` as before.
- SQLite opened with WAL + 5 s busy timeout.
- IPC: 30 s idle timeout per connection, 64 KiB line cap, unknown types inert.
- Commands use `try_state` so a failed storage open returns an error string
  instead of panicking.
- Bubble: Allow autofocused, Tab order Allow → Always → Deny, Esc collapses.

## Known limits (not fixed, on purpose)

- Long-running event bursts are applied serially; no backpressure metric yet.
- Corrupt DB disables the session engine (no auto-repair; DB is preserved).
- Tray menu rebuilds on every event; cheap now, could be debounced later.
- X11 untested here (Wayland only).
- Frame-level sprite animation (2-frame loops) still unimplemented.
