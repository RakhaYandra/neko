# API-GUIDE.md — Neko event bridge (Phase 2)

Pipeline: OpenCode hooks → `@neko/opencode-plugin` → Unix socket (NDJSON)
→ `neko` Rust core → Tauri event `neko-event` → UI. Observe-only: Neko never
replies to permissions and never blocks the agent (all sends end `|| true`).

## Transport

- Socket: `/tmp/neko.sock`, mode `0600`, override via `NEKO_SOCK` (both sides).
- One NDJSON object per line, one connection per message (fire-and-forget).
- Rust rejects unknown `v` and malformed JSON with a warning; listener survives.
- Debug headless: `NEKO_SOCK=/tmp/neko.sock cargo run --manifest-path
  apps/desktop/src-tauri/Cargo.toml --example neko-listen`

## Envelope (ADR-003)

`{v:1, type:"<neko.type>", at:<ms>, sessionId:<string|null>, payload:{...}}`
Full schemas: `packages/protocol/src/index.ts` (`parseMessage`).

## OpenCode → Neko type map

| OpenCode hook        | Neko `type`          | Payload                                      |
| -------------------- | -------------------- | -------------------------------------------- |
| `session.created`    | `session.created`    | `{project?}` from `info.title`/`directory`   |
| `session.status`     | `session.status`     | `{status}` normalized, unknown → `idle`      |
| `session.idle`       | `session.status`     | `{status:"completed"}`                       |
| `session.error`      | `session.error`      | `{message}` best-effort                      |
| `session.diff`       | `session.diff`       | `{files: string[]}`, `[]` when unknown       |
| `tool.execute.before`| `tool.started`       | `{tool, ref?}` from args path/command/url    |
| `tool.execute.after` | `tool.completed`     | `{tool}`                                     |
| `permission.asked`   | `permission.requested`| `{action, resource?}` from patterns         |
| `permission.replied` | `permission.resolved`| `{action?, decision}` fail-closed to `deny`  |
| `file.edited`        | `file.edited`        | `{path}` best-effort                         |
| `todo.updated`       | `todo.updated`       | `{}` (signal only)                           |

All other OpenCode events are ignored by the allow-list.

## E2E smoke (zero LLM cost)

```sh
rm -rf /tmp/neko-e2e && mkdir -p /tmp/neko-e2e/.opencode/plugins
cp packages/opencode-plugin/dist/index.js /tmp/neko-e2e/.opencode/plugins/neko.js
NEKO_SOCK=/tmp/neko-e2e.sock cargo run --manifest-path apps/desktop/src-tauri/Cargo.toml --example neko-listen &
NEKO_SOCK=/tmp/neko-e2e.sock opencode serve /tmp/neko-e2e --port 18789 &
curl -X POST 127.0.0.1:18789/session -H 'content-type: application/json' -d '{}'
# expect: EVENT type="session.created"
```

## Deferred

- Reply path (allow/deny from Neko UI) is NOT implemented; needs a
  `serve`-HTTP/SDK spike before Phase 5. Plugin stays observe-only.
- `session.completed`/`todo.updated` carry empty payloads; Phase 3 enriches.
