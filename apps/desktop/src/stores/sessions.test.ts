import { describe, expect, it } from "vitest";
import { useSessions } from "./sessions";

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
});
