import { z } from "zod";

export const PROTOCOL_VERSION = 1 as const;

const base = z.object({
  v: z.literal(PROTOCOL_VERSION),
  type: z.string(),
  at: z.number().int().nonnegative(),
  sessionId: z.string().nullish(),
});

export function isSupportedVersion(v: unknown): boolean {
  return v === PROTOCOL_VERSION;
}

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
    requestId: z.string().optional(),
  }),
});

export const toolStarted = base.extend({
  type: z.literal("tool.started"),
  payload: z.object({
    tool: z.string(),
    ref: z.string().optional(),
  }),
});

export const sessionCompleted = base.extend({
  type: z.literal("session.completed"),
  payload: z.object({}),
});

export const sessionError = base.extend({
  type: z.literal("session.error"),
  payload: z.object({ message: z.string() }),
});

export const sessionDiff = base.extend({
  type: z.literal("session.diff"),
  payload: z.object({ files: z.array(z.string()) }),
});

export const toolCompleted = base.extend({
  type: z.literal("tool.completed"),
  payload: z.object({ tool: z.string() }),
});

export const permissionResolved = base.extend({
  type: z.literal("permission.resolved"),
  payload: z.object({
    action: z.string().optional(),
    decision: z.enum(["allow", "deny"]),
    requestId: z.string().optional(),
  }),
});

export const fileEdited = base.extend({
  type: z.literal("file.edited"),
  payload: z.object({ path: z.string() }),
});

export const todoUpdated = base.extend({
  type: z.literal("todo.updated"),
  payload: z.object({}),
});

export const questionOption = z.object({
  label: z.string(),
  description: z.string().optional(),
});

export const questionItem = z.object({
  question: z.string(),
  header: z.string().optional(),
  options: z.array(questionOption),
  multiple: z.boolean().optional(),
  custom: z.boolean().optional(),
});

export const questionAsked = base.extend({
  type: z.literal("question.asked"),
  payload: z.object({
    requestId: z.string().optional(),
    questions: z.array(questionItem),
  }),
});

export const questionResolved = base.extend({
  type: z.literal("question.resolved"),
  payload: z.object({
    requestId: z.string().optional(),
  }),
});

export const nekoMessage = z.discriminatedUnion("type", [
  sessionCreated,
  sessionStatus,
  permissionRequested,
  toolStarted,
  sessionCompleted,
  sessionError,
  sessionDiff,
  toolCompleted,
  permissionResolved,
  fileEdited,
  todoUpdated,
  questionAsked,
  questionResolved,
]);

export type NekoMessage = z.infer<typeof nekoMessage>;

export function parseMessage(line: string): NekoMessage {
  return nekoMessage.parse(JSON.parse(line));
}
