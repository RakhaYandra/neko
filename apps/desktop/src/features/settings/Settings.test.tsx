import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { Settings } from "./Settings";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (cmd: string) => {
    if (cmd === "get_setting") return "1";
    if (cmd === "is_autostart") return false;
    return undefined;
  });
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("Settings", () => {
  it("rolls back toggle when persist fails", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "get_setting") return "1";
      if (cmd === "is_autostart") return false;
      if (cmd === "set_setting") throw new Error("db locked");
      return undefined;
    });
    render(<Settings />);
    await act(async () => {
      await vi.runAllTimersAsync();
    });
    const toggle = screen.getByLabelText("Notifications");
    expect(toggle).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(toggle);
    await act(async () => {
      await vi.runAllTimersAsync();
    });
    expect(screen.getByLabelText("Notifications")).toHaveAttribute("aria-pressed", "true");
  });

  it("debounces opacity slider persistence", async () => {
    render(<Settings />);
    await act(async () => {
      await vi.runAllTimersAsync();
    });
    invokeMock.mockClear();
    const slider = screen.getByLabelText("Opacity");
    for (let i = 0; i < 10; i++) {
      fireEvent.change(slider, { target: { value: String(50 + i) } });
    }
    expect(invokeMock).not.toHaveBeenCalledWith("set_setting", expect.anything());
    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
    });
    const calls = invokeMock.mock.calls.filter((c) => c[0] === "set_setting");
    expect(calls.length).toBe(1);
  });

  it("close button is labelled", async () => {
    render(<Settings />);
    await act(async () => {
      await vi.runAllTimersAsync();
    });
    expect(screen.getByLabelText("Close settings")).toBeInTheDocument();
  });
});
