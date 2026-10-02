import { describe, expect, it } from "vitest";
import { parseMessage } from "./index";

describe("protocol", () => {
  it("accepts session.status v1", () => {
    const m = parseMessage(
      JSON.stringify({ v: 1, type: "session.status", at: 1, payload: { status: "working" } }),
    );
    expect(m.type).toBe("session.status");
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
