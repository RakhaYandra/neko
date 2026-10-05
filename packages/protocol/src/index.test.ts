import { describe, expect, it } from "vitest";
import { parseMessage } from "./index";

describe("protocol", () => {
  it("accepts session.status v1", () => {
    const m = parseMessage(
      JSON.stringify({ v: 1, type: "session.status", at: 1, payload: { status: "working" } }),
    );
    expect(m.type).toBe("session.status");
  });
  it("accepts tool.started v1", () => {
    const m = parseMessage(
      JSON.stringify({
        v: 1,
        type: "tool.started",
        at: 1,
        sessionId: "ses_1",
        payload: { tool: "bash", ref: "npm install" },
      }),
    );
    expect(m.type).toBe("tool.started");
  });
  it("accepts permission.requested v1", () => {
    const m = parseMessage(
      JSON.stringify({
        v: 1,
        type: "permission.requested",
        at: 1,
        sessionId: "ses_1",
        payload: { action: "bash" },
      }),
    );
    expect(m.type).toBe("permission.requested");
  });
  it("accepts the remaining Phase 2 types", () => {
    const cases: Array<[string, unknown]> = [
      ["session.completed", {}],
      ["session.error", { message: "boom" }],
      ["session.diff", { files: ["a.ts"] }],
      ["tool.completed", { tool: "bash" }],
      ["permission.resolved", { action: "bash", decision: "allow" }],
      ["file.edited", { path: "src/a.ts" }],
      ["todo.updated", {}],
    ];
    for (const [type, payload] of cases) {
      const m = parseMessage(JSON.stringify({ v: 1, type, at: 1, sessionId: "s", payload }));
      expect(m.type).toBe(type);
    }
  });
  it("rejects wrong version", () => {
    expect(() =>
      parseMessage(JSON.stringify({ v: 99, type: "session.status", at: 1, payload: {} })),
    ).toThrow();
  });
  it("rejects malformed json", () => {
    expect(() => parseMessage("not-json{{{")).toThrow();
  });
  it("accepts question.asked with options", () => {
    const m = parseMessage(
      JSON.stringify({
        v: 1,
        type: "question.asked",
        at: 1,
        sessionId: "s",
        payload: {
          requestId: "que_1",
          questions: [
            {
              question: "Which kind?",
              options: [{ label: "bar-widget" }, { label: "panel", description: "popup" }],
            },
          ],
        },
      }),
    );
    expect(m.type).toBe("question.asked");
  });
  it("rejects non-integer / negative at", () => {
    for (const at of [1.5, -1, "now", null]) {
      expect(() =>
        parseMessage(JSON.stringify({ v: 1, type: "session.status", at, payload: {} })),
      ).toThrow();
    }
  });
});
