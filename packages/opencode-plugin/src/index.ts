// Neko OpenCode plugin (observe-only, Phase 1).
// Pushes a slim NDJSON event per OpenCode hook over the Unix socket.
// Fire-and-forget: `|| true` so the agent never blocks on Neko.
// OpenCode ctx is untyped by upstream — `any` is honest here.
/* eslint-disable @typescript-eslint/no-explicit-any */
export const NekoPlugin = async ({ $ }: any) => {
  const SOCK = process.env.NEKO_SOCK ?? "/tmp/neko.sock";
  const send = async (obj: unknown) => {
    try {
      const line = JSON.stringify({ v: 1, at: Date.now(), ...(obj as object) });
      await $`printf '%s\n' ${line} | timeout 0.3 socat - UNIX-CONNECT:${SOCK} 2>/dev/null || true`;
    } catch {
      // intentionally silent: Neko must never break the agent
    }
  };
  return {
    event: async ({ event }: any) => {
      const allow = ["session.created", "session.status", "session.idle", "permission.asked"];
      if (!allow.includes(event.type)) return;
      const p = event.properties ?? {};
      const info = p.info ?? {};
      await send({
        type: event.type,
        session: p.sessionID ?? info.sessionID ?? info.id ?? null,
        status: p.status?.type ?? null,
        permission: p.permission ?? null,
      });
    },
    "tool.execute.before": async (input: any, output: any) => {
      const args = (output as any)?.args ?? {};
      await send({
        type: "tool.execute.before",
        session: (input as any)?.sessionID ?? null,
        tool: (input as any)?.tool ?? null,
        ref: args.filePath ?? args.path ?? args.command ?? args.url ?? "",
      });
    },
  };
};
