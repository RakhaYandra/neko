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
  it("rejects wrong version", () => {
    expect(() =>
      parseMessage(JSON.stringify({ v: 99, type: "session.status", at: 1, payload: {} })),
    ).toThrow();
  });
  it("rejects malformed json", () => {
    expect(() => parseMessage("not-json{{{")).toThrow();
  });
});
