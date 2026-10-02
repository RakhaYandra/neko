// Neko OpenCode plugin (observe-only, Phase 1).
// Adapts OpenCode hooks into the Neko envelope (ADR-003):
// `{v:1, type:"<neko.type>", at, sessionId, payload}`.
// Fire-and-forget: `|| true` so the agent never blocks on Neko.
// OpenCode ctx is untyped by upstream — `any` is honest here.
/* eslint-disable @typescript-eslint/no-explicit-any */
type NekoStatus = "idle" | "working" | "tool_running" | "completed" | "error";

// Live OpenCode status strings are verified in Phase 2; unknown maps to
// "idle" (no known activity) rather than inventing a busier state.
function normalizeStatus(s: unknown): NekoStatus {
  if (s === "idle" || s === "working" || s === "tool_running" || s === "completed" || s === "error")
    return s;
  return "idle";
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
      const allow = ["session.created", "session.status", "session.idle", "permission.asked"];
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
      } else {
        const patterns = Array.isArray(p.patterns) ? p.patterns.join(",") : undefined;
        await send("permission.requested", sessionId, {
          action: String(p.permission ?? "unknown"),
          ...(patterns ? { resource: patterns } : {}),
        });
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
  };
};
