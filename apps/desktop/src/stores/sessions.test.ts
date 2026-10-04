import { describe, expect, it } from "vitest";
import { activeSession, useSessions } from "./sessions";

describe("sessions store", () => {
  it("mirrors a rust snapshot without inferring state", () => {
    useSessions.getState().setSnapshot([
      { id: "alpha", project: "alpha", status: "tool_running", lastActivityAt: 3 },
      { id: "beta", project: "beta", status: "idle", lastActivityAt: 2 },
    ]);
    const sessions = useSessions.getState().sessions;
    expect(sessions.map((s) => `${s.project}:${s.status}`)).toEqual([
      "alpha:tool_running",
      "beta:idle",
    ]);
  });

  it("drives the companion from explicit pick, else most recent", () => {
    const s = useSessions.getState();
    expect(activeSession(s)?.id).toBe("alpha");
    useSessions.getState().select("beta");
    expect(activeSession(useSessions.getState())?.id).toBe("beta");
    useSessions.getState().setSnapshot([
      { id: "alpha", project: "alpha", status: "idle", lastActivityAt: 4 },
    ]);
    // beta vanished: selection drops, falls back to alpha.
    expect(activeSession(useSessions.getState())?.id).toBe("alpha");
    useSessions.getState().select(null);
  });

  it("drops orphan pending for gone sessions", () => {
    useSessions.getState().setSnapshot(
      [{ id: "alpha", project: "alpha", status: "working", lastActivityAt: 5 }],
      [
        { requestId: "per_1", sessionId: "alpha", action: "bash", resource: null, askedAt: 5 },
        { requestId: "per_2", sessionId: "ghost", action: "bash", resource: null, askedAt: 5 },
      ],
    );
    expect(useSessions.getState().pending.map((p) => p.requestId)).toEqual(["per_1"]);
    useSessions.getState().setSnapshot([]);
  });
});
