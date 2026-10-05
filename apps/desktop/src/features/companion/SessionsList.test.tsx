import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SessionsList, shortId, age } from "./SessionsList";

describe("SessionsList", () => {
  it("shows empty state when no sessions", () => {
    render(<SessionsList sessions={[]} selectedId={null} onSelect={() => {}} />);
    expect(screen.getByText(/No sessions/)).toBeInTheDocument();
  });

  it("marks active session with aria-current", () => {
    const sessions = [
      { id: "a", project: "alpha", status: "working", lastActivityAt: 2 },
      { id: "b", project: "beta", status: "idle", lastActivityAt: 1 },
    ];
    const onSelect = vi.fn();
    render(<SessionsList sessions={sessions} selectedId={null} onSelect={onSelect} />);
    const buttons = screen.getAllByRole("button");
    expect(buttons[0]).toHaveAttribute("aria-current", "true");
    fireEvent.click(buttons[1]);
    expect(onSelect).toHaveBeenCalledWith("b");
  });

  it("shortens ids and ages timestamps for stream rows", () => {
    expect(shortId("ses_efa475bddffe1kfaVsdVhvtgOb")).toBe("ses_efa475…hvtgOb");
    expect(shortId("abc")).toBe("abc");
    expect(age(Date.now())).toBe("0s");
    expect(age(Date.now() - 14_000)).toBe("14s");
    expect(age(Date.now() - 5 * 60_000)).toBe("5m");
  });
});
