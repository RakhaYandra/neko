import { describe, expect, it } from "vitest";
import { parseMessage } from "@neko/protocol";

// Guards the UI-side contract: every envelope the plugin emits over the
// socket must validate before it reaches the Tauri event boundary.
describe("ipc contract", () => {
  it("accepts a canonical session.status envelope", () => {
    const m = parseMessage(
      JSON.stringify({
        v: 1,
        type: "session.status",
        at: Date.now(),
        sessionId: "ses_1",
        payload: { status: "working" },
      }),
    );
    expect(m.type).toBe("session.status");
  });

  it("rejects envelopes with unknown version", () => {
    expect(() =>
      parseMessage(
        JSON.stringify({ v: 99, type: "session.status", at: 1, payload: {} }),
      ),
    ).toThrow();
  });
});
