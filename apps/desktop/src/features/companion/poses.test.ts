import { describe, expect, it } from "vitest";
import { STATE_POSE, poseCrop } from "./poses";

describe("poses", () => {
  it("maps the 7 states to distinct cards", () => {
    const idx = Object.values(STATE_POSE);
    expect(new Set(idx).size).toBe(7);
  });

  it("crops the IDLE card at the verified geometry", () => {
    const c = poseCrop("idle");
    expect(c.scale).toBeCloseTo(6.4);
    expect(c.xPct).toBeCloseTo(52.5); // (24 + 18) / 80 * 100
    expect(c.yPct).toBeCloseTo(102.5); // (64 + 18) / 80 * 100
  });

  it("falls back to IDLE for unknown states", () => {
    expect(poseCrop("bogus")).toEqual(poseCrop("idle"));
  });
});
