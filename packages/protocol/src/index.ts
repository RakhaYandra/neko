import { z } from "zod";

export const PROTOCOL_VERSION = 1 as const;

const base = z.object({
  v: z.literal(PROTOCOL_VERSION),
  type: z.string(),
  at: z.number(),
  sessionId: z.string().optional(),
});

export const sessionCreated = base.extend({
  type: z.literal("session.created"),
  payload: z.object({ project: z.string().optional() }),
});

export const sessionStatus = base.extend({
  type: z.literal("session.status"),
  payload: z.object({
    status: z.enum(["idle", "working", "tool_running", "completed", "error"]),
  }),
});

export const permissionRequested = base.extend({
  type: z.literal("permission.requested"),
  payload: z.object({
    action: z.string(),
    resource: z.string().optional(),
  }),
});

export const nekoMessage = z.discriminatedUnion("type", [
  sessionCreated,
  sessionStatus,
  permissionRequested,
]);

export type NekoMessage = z.infer<typeof nekoMessage>;

export function parseMessage(line: string): NekoMessage {
  return nekoMessage.parse(JSON.parse(line));
}
