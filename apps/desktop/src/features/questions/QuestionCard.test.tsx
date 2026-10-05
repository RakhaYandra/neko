import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QuestionCard } from "./QuestionCard";
import type { PendingQuestion } from "../../stores/sessions";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

const base: PendingQuestion = {
  requestId: "que_1",
  sessionId: "ses_1",
  askedAt: Date.now(),
  questions: [
    {
      question: "Which kind?",
      options: [{ label: "bar-widget" }, { label: "panel", description: "popup" }],
    },
  ],
};

const noop = () => {};

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(undefined);
});

describe("QuestionCard", () => {
  it("single-tap answers immediately", async () => {
    const onDone = vi.fn();
    render(<QuestionCard pending={base} project="alpha" onDone={onDone} onCancel={noop} />);
    fireEvent.click(screen.getByLabelText("Answer bar-widget"));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(invokeMock).toHaveBeenCalledWith("reply_question", {
      requestId: "que_1",
      answers: [["bar-widget"]],
    });
  });

  it("multiple select then submit", async () => {
    const multi: PendingQuestion = {
      ...base,
      questions: [{ question: "Pick", multiple: true, options: [{ label: "a" }, { label: "b" }] }],
    };
    render(<QuestionCard pending={multi} project="alpha" onDone={noop} onCancel={noop} />);
    const submit = screen.getByLabelText("Submit answers");
    expect(submit).toBeDisabled();
    fireEvent.click(screen.getByLabelText("Answer a"));
    fireEvent.click(screen.getByLabelText("Answer b"));
    expect(submit).not.toBeDisabled();
    fireEvent.click(submit);
    expect(invokeMock).toHaveBeenCalledWith("reply_question", {
      requestId: "que_1",
      answers: [["a", "b"]],
    });
  });

  it("dismiss rejects without answering", async () => {
    render(<QuestionCard pending={base} project="alpha" onDone={noop} onCancel={noop} />);
    fireEvent.click(screen.getByLabelText("Dismiss question"));
    expect(invokeMock).toHaveBeenCalledWith("reject_question", { requestId: "que_1" });
  });

  it("shows real error on failure", async () => {
    invokeMock.mockRejectedValueOnce(new Error("denied by serve"));
    render(<QuestionCard pending={base} project="alpha" onDone={noop} onCancel={noop} />);
    fireEvent.click(screen.getByLabelText("Answer bar-widget"));
    expect(await screen.findByText(/denied by serve/)).toBeInTheDocument();
  });

  it("multi-question asks route to terminal", () => {
    const multi: PendingQuestion = {
      ...base,
      questions: [
        { question: "Q1", options: [{ label: "a" }] },
        { question: "Q2", options: [{ label: "b" }] },
      ],
    };
    render(<QuestionCard pending={multi} project="alpha" onDone={noop} onCancel={noop} />);
    expect(screen.getByText(/answer in the terminal/)).toBeInTheDocument();
    expect(screen.queryByLabelText("Answer a")).not.toBeInTheDocument();
  });

  it("custom flag shows free-text notice", () => {
    const custom: PendingQuestion = {
      ...base,
      questions: [{ question: "Q", custom: true, options: [{ label: "a" }] }],
    };
    render(<QuestionCard pending={custom} project="alpha" onDone={noop} onCancel={noop} />);
    expect(screen.getByText(/Free-text answers unsupported/)).toBeInTheDocument();
    expect(screen.getByLabelText("Answer a")).toBeInTheDocument();
  });
});
