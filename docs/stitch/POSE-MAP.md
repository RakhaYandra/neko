# POSE-MAP.md — Stitch sprite poses → Neko states

Source: `apps/desktop/src/assets/neko-sprite.svg` (16-pose atlas, 4×4 grid,
card pitch 120px, sprite 16×16 px at 4× scale, card origin + (20,24) inner).
Light sheet: `neko-sprite-light.svg` (1–2 frames per pose for animation).
Dark sheet: `neko-animation-dark.svg` (Frame-2 suite + palette + daemon triggers).

## Primary mapping (Neko state machine → atlas pose)

| Neko state         | Pose | Atlas label      | Notes                              |
| ------------------ | ---- | ---------------- | ---------------------------------- |
| DISCONNECTED       | 7    | DISCONNECTED     | grey socket-plug cat               |
| IDLE               | 1    | IDLE (– – Zzz)   | default resting pose               |
| WORKING            | 2    | CODING (Paws)    | primary active pose                |
| TOOL_RUNNING       | 3    | BUSY (> < +Tool) | wrench/tool variant                |
| WAITING_PERMISSION | 4    | PERMISSION (!)   | amber alert, expands to bubble     |
| COMPLETED          | 5    | COMPLETED (♪)    | success jingle                     |
| ERROR              | 6    | ERROR (X X)      | red X eyes                         |

## Transient / secondary poses (Phase 4+)

| Situation              | Pose | Atlas label        |
| ---------------------- | ---- | ------------------ |
| Permission allowed     | 15   | ALLOWED (✓ Paws)   |
| Permission denied      | 14   | DENIED (Paws X)    |
| Reconnecting IPC       | 10   | RECONNECT          |
| Thinking (no tool yet) | 8    | THINKING (…)       |
| Celebration easter egg | 9    | CELEBRATE (Party)  |
| Multi-session badge    | 16   | MULTI-SESSION      |
| Dragging window        | 12   | DRAGGING           |
| Listening / mic future | 13   | LISTEN (Earwave)   |
| Hidden / peek mode     | 11   | PEEKING            |

## Animation

Prefer the light sheet's Frame 1 ↔ Frame 2 pairs for idle/working loops
(COMPANION states: sleeping/coding/busy/attention/happy/sad per master
prompt §10). Dark Frame-2 suite covers the same 16 poses for dark theme.
Frame timing TBD in Phase 4 (suggest 2 fps pixel stepping).

## App icon

Derived from pose 1 (IDLE) inner sprite, see `src-tauri/icons/` (generated
via `tauri icon` from a 1024px nearest-neighbor upscale on `#0d1117`).

## Still missing (Stitch side)

3 items stuck at `Generating...` (2× 512px images + 1 screen) — re-check
or regenerate in Stitch before Phase 4; no repo action needed until then.
