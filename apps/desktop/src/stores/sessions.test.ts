import { describe, expect, it } from "vitest";
import { activeSession, useSessions } from "./sessions";

describe("sessions store", () => {
  it("mirrors a rust snapshot without inferring state", () => {
    useSessions.getState().setSnapshot([
      { id: "pulse", project: "pulse", status: "tool_running", lastActivityAt: 3 },
      { id: "lifeos", project: "lifeos", status: "idle", lastActivityAt: 2 },
    ]);
    const sessions = useSessions.getState().sessions;
    expect(sessions.map((s) => `${s.project}:${s.status}`)).toEqual([
      "pulse:tool_running",
      "lifeos:idle",
    ]);
  });

  it("drives the companion from explicit pick, else most recent", () => {
    const s = useSessions.getState();
    expect(activeSession(s)?.id).toBe("pulse");
    useSessions.getState().select("lifeos");
    expect(activeSession(useSessions.getState())?.id).toBe("lifeos");
    useSessions.getState().setSnapshot([
      { id: "pulse", project: "pulse", status: "idle", lastActivityAt: 4 },
    ]);
    // lifeos vanished: selection drops, falls back to pulse.
    expect(activeSession(useSessions.getState())?.id).toBe("pulse");
    useSessions.getState().select(null);
  });
});
