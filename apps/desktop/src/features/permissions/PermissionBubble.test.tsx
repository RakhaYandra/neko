import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { PermissionBubble } from "./PermissionBubble";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

const base = {
  requestId: "per_1",
  sessionId: "ses_1",
  action: "bash",
  resource: "echo hi",
  askedAt: Date.now(),
};

beforeEach(() => {
  invokeMock.mockReset();
});

describe("PermissionBubble", () => {
  it("shows real error message on reply failure", async () => {
    invokeMock.mockRejectedValueOnce(new Error("denied by serve"));
    render(
      <PermissionBubble pending={base} project="alpha" onDone={() => {}} onCancel={() => {}} />,
    );
    fireEvent.click(screen.getByLabelText("Allow permission"));
    expect(await screen.findByText(/denied by serve/)).toBeInTheDocument();
  });

  it("ignores stale resolve after requestId changes", async () => {
    let resolveFirst!: () => void;
    invokeMock.mockImplementationOnce(
      () => new Promise<void>((res) => void (resolveFirst = res)),
    );
    const onDone = vi.fn();
    const { rerender } = render(
      <PermissionBubble pending={base} project="alpha" onDone={onDone} onCancel={() => {}} />,
    );
    fireEvent.click(screen.getByLabelText("Allow permission"));
    rerender(
      <PermissionBubble
        pending={{ ...base, requestId: "per_2" }}
        project="alpha"
        onDone={onDone}
        onCancel={() => {}}
      />,
    );
    await act(async () => {
      resolveFirst();
    });
    expect(onDone).not.toHaveBeenCalled();
  });

  it("Escape cancels once via stopPropagation", async () => {
    const onCancel = vi.fn();
    const { container } = render(
      <PermissionBubble pending={base} project="alpha" onDone={() => {}} onCancel={onCancel} />,
    );
    const dialog = container.firstChild as HTMLElement;
    const stop = vi.fn();
    fireEvent.keyDown(screen.getByRole("alertdialog"), {
      key: "Escape",
      stopPropagation: stop,
    });
    expect(dialog).toBeInTheDocument();
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("shows expired badge for old requests", () => {
    render(
      <PermissionBubble
        pending={{ ...base, askedAt: Date.now() - 6 * 60 * 1000 }}
        project="alpha"
        onDone={() => {}}
        onCancel={() => {}}
      />,
    );
    expect(screen.getByText(/expired/)).toBeInTheDocument();
  });
});
