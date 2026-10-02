# CODE-STYLE-GUIDE.md

Minimal rules (Phase 1). Expand only when violated twice.

## General
- Smallest diff that works. No speculative abstraction (YAGNI).
- English code + comments. Concise, no AI-slop comments.
- `cargo fmt` + `clippy -D warnings` must pass. ESLint recommended must pass.

## TypeScript / React
- `strict` TS. Zod at every IPC boundary — never trust raw JSON.
- Zustand stores: one topic per store, selectors per component.
- Tailwind for styling. `image-rendering: pixelated` for all sprite art.

## Rust
- One responsibility per module (`ipc/`, `sessions/`, ...).
- Structured logging via `tracing`. Levels: ERROR/WARN/INFO/DEBUG/TRACE.
- Never log prompts, secrets, source code, or full command output.
- Reject malformed input, never panic on it.

## Commits
- Conventional Commits: `feat:`, `fix:`, `chore:`, `docs:`.
- Small, logical, one concern per commit.

## Docs
- Every architectural change updates docs + adds/updates an ADR in `docs/`.
