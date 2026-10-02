# SECURITY-GUIDE.md — Neko (Phase 5: permission reply)

Neko is local-first, but permission replies can let an agent run commands.
This guide states the exact guarantees; every reply-path change must keep them.

## Guarantees

1. **Explicit clicks only.** The sole reply path is the `reply_permission`
   Tauri command, fed by the bubble buttons. No auto-approve, no auto-reject,
   no timers that decide, no defaults that answer.
2. **Single-shot registry.** The manager removes the pending entry BEFORE any
   HTTP fires (`take_pending`). A repeated click finds nothing and errors.
   On transport failure the entry is restored for manual retry; on
   `AlreadySettled` it stays gone.
3. **Strict vocabulary.** Only `once`/`always`/`deny` parse; anything else is
   rejected before any network call. Replies carry no `message`.
4. **Localhost only.** Target is `NEKO_OPENCODE_URL` (env, default unset).
   No port scanning, no remote hosts, no cloud. Serve unreachable → bubble
   error, agent keeps waiting, nothing is approved.
5. **Nothing sensitive at rest.** Pending registry is memory-only. SQLite
   stores no prompts, commands, code, or secrets (DATABASE-GUIDE.md).
   Logs carry types and IDs, never payloads.
6. **External truth wins.** A terminal-side answer arrives as
   `permission.resolved` and clears the registry; Neko never fights it.

## Operator setup

- Run `opencode serve` (or `web`) and export
  `NEKO_OPENCODE_URL=http://127.0.0.1:<port>` before starting Neko.
- TUI-only sessions show "answer in the terminal" instead of buttons.

## Known limits (MVP)

- No per-request confirmation detail beyond action + resource pattern.
- `always` persists inside OpenCode, not Neko; audit there.
- Concurrent requests for one session show newest-first; all stay answerable.
