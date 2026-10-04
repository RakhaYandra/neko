// Neko OpenCode plugin (observe-only, Phase 2).
// Adapts OpenCode hooks into the Neko envelope (ADR-003):
// `{v:1, type:"<neko.type>", at, sessionId, payload}`.
// Fire-and-forget: agent never blocks on Neko. No shell: writes NDJSON
// straight to the Unix socket via node:net, so payload quotes can't inject.
// OpenCode ctx is untyped by upstream — `any` is honest here.
/* eslint-disable @typescript-eslint/no-explicit-any */
import { createConnection } from "node:net";

type NekoStatus = "idle" | "working" | "tool_running" | "completed" | "error";

// Unknown OpenCode status strings map to "working" (assume activity)
// rather than hiding work as idle.
function normalizeStatus(s: unknown): NekoStatus {
  if (s === "idle" || s === "working" || s === "tool_running" || s === "completed" || s === "error")
    return s;
  return "working";
}

// Fail-closed display decision: only explicit approvals read as allow.
function normalizeDecision(r: unknown): "allow" | "deny" {
  return r === "once" || r === "always" ? "allow" : "deny";
}

// Socket path precedence, mirroring Rust `ipc::socket_path()`:
// `NEKO_SOCK` (non-empty) > `$XDG_RUNTIME_DIR/neko.sock` > `/tmp/neko.sock`.
export function nekoSocketPath(): string {
  const env = process.env.NEKO_SOCK;
  if (env && env.length > 0) return env;
  const xdg = process.env.XDG_RUNTIME_DIR;
  if (xdg && xdg.length > 0) return `${xdg}/neko.sock`;
  return "/tmp/neko.sock";
}

export const NekoPlugin = async () => {
  const SOCK = nekoSocketPath();
  const DEBUG = process.env.NEKO_DEBUG === "1";
  const send = async (type: string, sessionId: string | null, payload: Record<string, unknown>) => {
    try {
      const line = JSON.stringify({ v: 1, type, at: Date.now(), sessionId, payload });
      await new Promise<void>((resolve) => {
        let done = false;
        const finish = () => {
          if (!done) {
            done = true;
            resolve();
          }
        };
        try {
          const conn = createConnection(SOCK);
          const timer = setTimeout(() => {
            try {
              conn.destroy();
            } catch {
              // never blocks the agent
            }
            finish();
          }, 300);
          conn.on("error", (e) => {
            if (DEBUG) console.debug("[neko] socket send skipped:", String(e?.message ?? e));
            clearTimeout(timer);
            try {
              conn.destroy();
            } catch {
              // ignore
            }
            finish();
          });
          conn.on("connect", () => {
            conn.end(line + "\n", () => {
              clearTimeout(timer);
              finish();
            });
          });
        } catch (e) {
          if (DEBUG) console.debug("[neko] socket send skipped:", String(e));
          finish();
        }
      });
    } catch {
      // intentionally silent: Neko must never break the agent
    }
  };
  const requestIdOf = (p: any): string | undefined => {
    const v = p.requestID ?? p.requestId ?? p.id;
    return v != null ? String(v) : undefined;
  };
  const sessionOf = (p: any, info: any): string | null =>
    p.sessionID ?? info.sessionID ?? info.id ?? null;
  return {
    event: async ({ event }: any) => {
      const allow = [
        "session.created",
        "session.status",
        "session.idle",
        "session.error",
        "session.diff",
        "permission.asked",
        "permission.replied",
        "file.edited",
        "todo.updated",
      ];
      if (!allow.includes(event.type)) return;
      const p = event.properties ?? {};
      const info = p.info ?? {};
      const sessionId = sessionOf(p, info);
      if (event.type === "session.created") {
        const project = info.title ?? info.directory ?? undefined;
        await send("session.created", sessionId, project ? { project } : {});
      } else if (event.type === "session.status") {
        await send("session.status", sessionId, { status: normalizeStatus(p.status?.type) });
      } else if (event.type === "session.idle") {
        await send("session.status", sessionId, { status: "completed" });
      } else if (event.type === "session.error") {
        await send("session.error", sessionId, {
          message: String(p.error ?? p.message ?? "unknown error"),
        });
      } else if (event.type === "session.diff") {
        const files = Array.isArray(p.files) ? p.files.map(String) : [];
        await send("session.diff", sessionId, { files });
      } else if (event.type === "permission.asked") {
        const patterns = Array.isArray(p.patterns) ? p.patterns.join(",") : undefined;
        // Reply key: upstream uses `id` and sometimes `requestID`; accept both.
        // `permission` already names the tool; upstream `tool` is an object
        // {messageID, callID}, not forwarded.
        const requestId = requestIdOf(p);
        await send("permission.requested", sessionId, {
          action: String(p.permission ?? "unknown"),
          ...(patterns ? { resource: patterns } : {}),
          ...(requestId ? { requestId } : {}),
        });
      } else if (event.type === "permission.replied") {
        const action = p.permission != null ? String(p.permission) : undefined;
        const requestId = requestIdOf(p);
        await send("permission.resolved", sessionId, {
          ...(action ? { action } : {}),
          decision: normalizeDecision(p.response ?? p.reply),
          ...(requestId ? { requestId } : {}),
        });
      } else if (event.type === "file.edited") {
        await send("file.edited", sessionId, {
          path: String(p.file ?? p.metadata?.filepath ?? p.path ?? "unknown"),
        });
      } else {
        await send("todo.updated", sessionId, {});
      }
    },
    "tool.execute.before": async (input: any, output: any) => {
      const args = (output as any)?.args ?? {};
      const ref = args.filePath ?? args.path ?? args.command ?? args.url ?? undefined;
      await send("tool.started", (input as any)?.sessionID ?? null, {
        tool: String((input as any)?.tool ?? "unknown"),
        ...(ref ? { ref: String(ref) } : {}),
      });
    },
    "tool.execute.after": async (input: any) => {
      await send("tool.completed", (input as any)?.sessionID ?? null, {
        tool: String((input as any)?.tool ?? "unknown"),
      });
    },
  };
};
