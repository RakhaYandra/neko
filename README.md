# Neko

> Your little OpenCode companion for Linux.

Local-first desktop companion that reflects OpenCode session state:
working, waiting-permission, completed, error — without opening the terminal.

## Status

Phase 6 — Linux integration (tray, notifications, autostart, shortcut, settings, AppImage/.deb).

## Layout

```text
apps/desktop          Tauri 2 + React 19 + TS + Vite (Rust core + UI)
packages/protocol     versioned IPC protocol (Zod schemas)
packages/opencode-plugin  OpenCode plugin (event adapter, Unix socket push)
docs/                 PRD/ARCHITECTURE/ADR/...
```

## Dev

```text
npm install
npm run dev        # Tauri dev window
```

Local-only: plugin pushes NDJSON over a `0600` Unix socket. No TCP, no cloud.
