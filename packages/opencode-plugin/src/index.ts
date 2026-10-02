// Neko OpenCode plugin (observe-only, Phase 2).
// Adapts OpenCode hooks into the Neko envelope (ADR-003):
// `{v:1, type:"<neko.type>", at, sessionId, payload}`.
// Fire-and-forget: `|| true` so the agent never blocks on Neko.
// OpenCode ctx is untyped by upstream — `any` is honest here.
/* eslint-disable @typescript-eslint/no-explicit-any */
type NekoStatus = "idle" | "working" | "tool_running" | "completed" | "error";

// Unknown OpenCode status strings map to "idle" (no known activity)
// rather than inventing a busier state.
function normalizeStatus(s: unknown): NekoStatus {
  if (s === "idle" || s === "working" || s === "tool_running" || s === "completed" || s === "error")
    return s;
  return "idle";
}

// Fail-closed display decision: only explicit approvals read as allow.
function normalizeDecision(r: unknown): "allow" | "deny" {
  return r === "once" || r === "always" ? "allow" : "deny";
}

export const NekoPlugin = async ({ $ }: any) => {
  const SOCK = process.env.NEKO_SOCK ?? "/tmp/neko.sock";
  const send = async (type: string, sessionId: string | null, payload: Record<string, unknown>) => {
    try {
      const line = JSON.stringify({ v: 1, type, at: Date.now(), sessionId, payload });
      await $`printf '%s\n' ${line} | timeout 0.3 socat - UNIX-CONNECT:${SOCK} 2>/dev/null || true`;
    } catch {
      // intentionally silent: Neko must never break the agent
    }
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
        // `id` is the reply key (R4). `permission` already names the tool;
        // upstream `tool` is an object {messageID, callID}, not forwarded.
        await send("permission.requested", sessionId, {
          action: String(p.permission ?? "unknown"),
          ...(patterns ? { resource: patterns } : {}),
          ...(p.id != null ? { requestId: String(p.id) } : {}),
        });
      } else if (event.type === "permission.replied") {
        const action = p.permission != null ? String(p.permission) : undefined;
        await send("permission.resolved", sessionId, {
          ...(action ? { action } : {}),
          decision: normalizeDecision(p.response ?? p.reply),
          ...(p.requestID != null ? { requestId: String(p.requestID) } : {}),
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
