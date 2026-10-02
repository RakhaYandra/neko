# PHASE-0-REPORT.md — Research & Technical Spike

Date: 2026-10-02. Env: Hyprland 0.56.2 (Wayland), opencode 1.18.32, Rust 1.98.1, Node 26.8.1.
Spike dir: `/tmp/neko-spike/` (disposable). E2E project: `/tmp/neko-e2e/`.

## PHASE STATUS: PASS

Chain proven end-to-end on local device, zero LLM cost:

```text
opencode serve (127.0.0.1:18789, project /tmp/neko-e2e)
  → plugin neko-spike.ts (V1 event hook)
  → Unix socket /tmp/neko-spike.sock (mode 0600, NDJSON)
  → Rust tokio listener (validates v==1)
  → (Tauri window verified separately, see T3)
```

## WHAT WAS VERIFIED

- T1 plugin: V1 `event` hook receives `session.created`; `tool.execute.before` hook works. Bundles clean via esbuild. Send path `printf | socat UNIX-CONNECT` with `|| true` never blocks agent.
- T2 socket: mode `0600`; `{"v":1,...}` accepted and printed; `{"v":99}` rejected; `not-json{{{` rejected with error, listener survives.
- T3 Tauri 2: `npm create tauri-app` (react-ts, npm) compiles (485 crates); window `transparent:true, decorations:false, alwaysOnTop:true, skipTaskbar:true` opens on Hyprland Wayland native, `mapped:1 visible:1`, no GBM/DMABUF crash, no Error 71. Only benign `gtk_window_set_titlebar` warning. (Always-on-top z-order on Wayland not strictly verifiable — see ADR-002.)
- T4 live E2E: `POST /session` on local serve → Rust log `EVENT type="session.created" session="ses_f045..."`. Kill Rust → serve alive. Restart Rust → next `session.created` received (reconnect free via per-send connections).
- Local-only: serve binds `127.0.0.1` by default; no TCP in Neko path; no remote/cloud involved.

## RISKS (carried to next phases)

1. Wayland `alwaysOnTop` not guaranteed upstream (tauri#3117, tao#1134). Mitigation: ADR-002 fallback.
2. Transparent-window GBM crash on Nvidia+WebKitGTK (tauri#14924). Not hit on this machine (check GPU before Phase 4).
3. Tray icon may not show on some Wayland compositors (tauri#14234). Verify on Hyprland in Phase 6.
4. `permission` reply (allow/deny) from outside TUI unverified — observe-only in Phase 0 by decision. Must spike before Phase 5.
5. `POST /session` shape used only for testing; production path is plugin push, not HTTP polling.

## DECISIONS

- ADR-000: npm (not pnpm) for TS workspaces.
- ADR-001: Unix Domain Socket + NDJSON, no TCP in MVP.
- ADR-002: Tauri Wayland fallback (accept degraded on-top/transparency).
- ADR-003: protocol envelope `{v:1, type, at, ...}`, receivers reject unknown `v`.

## TEST RESULTS

| Test | Result |
|---|---|
| Plugin bundles (esbuild) | PASS |
| Socket 0600 + valid accepted | PASS |
| Wrong version rejected | PASS |
| Malformed rejected, listener alive | PASS |
| Tauri window opens, Wayland, no crash | PASS |
| Live `session.created` → Rust | PASS |
| Rust killed → serve survives | PASS |
| Rust restarted → events flow again | PASS |

## NEXT PHASE

Phase 1 Foundation at `~/Projects/neko/`: monorepo (npm workspaces), Tauri+React+TS+Vite scaffold, Rust core skeleton, protocol package with Zod schemas, CI, logging, CODE-STYLE-GUIDE.md. Do NOT start Phase 2 bridge until Phase 1 builds green.
