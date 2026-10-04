import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SessionsList } from "./SessionsList";

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
});
