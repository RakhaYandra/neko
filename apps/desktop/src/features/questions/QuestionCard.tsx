import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PendingQuestion } from "../../stores/sessions";

const STALE_MS = 5 * 60 * 1000;

function age(askedAt: number): string {
  const s = Math.max(0, Math.round((Date.now() - askedAt) / 1000));
  return s < 60 ? `${s}s ago` : `${Math.floor(s / 60)}m ago`;
}

export function QuestionCard({
  pending,
  project,
  onDone,
  onCancel,
}: {
  pending: PendingQuestion;
  project: string;
  onDone: () => void;
  onCancel: () => void;
}) {
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [now, setNow] = useState(() => Date.now());
  const stale = now - pending.askedAt > STALE_MS;
  const liveId = useRef(pending.requestId);
  const firstRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    liveId.current = pending.requestId;
    setBusy(null);
    setError(null);
    setSelected([]);
  }, [pending.requestId]);

  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 30_000);
    return () => clearInterval(t);
  }, []);

  useEffect(() => {
    firstRef.current?.focus();
  }, [pending.requestId]);

  const first = pending.questions[0];
  const extra = pending.questions.length - 1;

  async function answer(answers: string[][]) {
    const id = pending.requestId;
    setBusy("sending");
    setError(null);
    try {
      await invoke("reply_question", { requestId: id, answers });
      if (liveId.current !== id) return; // superseded
      onDone();
    } catch (e) {
      if (liveId.current !== id) return;
      setError(e instanceof Error ? e.message : typeof e === "string" ? e : "answer failed");
    } finally {
      if (liveId.current === id) setBusy(null);
    }
  }

  async function dismiss() {
    const id = pending.requestId;
    setBusy("sending");
    setError(null);
    try {
      await invoke("reject_question", { requestId: id });
      if (liveId.current !== id) return;
      onDone();
    } catch (e) {
      if (liveId.current !== id) return;
      setError(e instanceof Error ? e.message : typeof e === "string" ? e : "dismiss failed");
    } finally {
      if (liveId.current === id) setBusy(null);
    }
  }

  // Multi-question asks need ordered answers per question; MVP answers the
  // first only via terminal. Dismiss stays available.
  if (!first || extra > 0) {
    return (
      <div
        role="alertdialog"
        aria-label="Question"
        aria-live="assertive"
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.stopPropagation();
            onCancel();
          }
        }}
        style={{
          marginTop: 10,
          padding: 10,
          borderRadius: 10,
          border: "1px solid #58a6ff",
          background: "rgba(88,166,255,0.08)",
        }}
      >
        <div style={{ fontSize: 12, fontWeight: 700 }}>Question needs an answer</div>
        <div style={{ fontSize: 12, opacity: 0.85, marginTop: 4 }}>
          {project} · {age(pending.askedAt)}
        </div>
        <div style={{ fontSize: 11, opacity: 0.8, marginTop: 6 }}>
          {!first
            ? "No options provided — answer in the terminal."
            : `${extra + 1} questions at once — answer in the terminal.`}
        </div>
        <div style={{ display: "flex", gap: 6, marginTop: 8 }}>
          <button
            ref={firstRef}
            onClick={() => void dismiss()}
            disabled={busy !== null}
            aria-label="Dismiss question"
            style={{
              flex: 1,
              padding: "6px 0",
              borderRadius: 8,
              border: "1px solid #30363d",
              cursor: busy !== null ? "wait" : "pointer",
              fontSize: 12,
              color: "#fff",
              background: "rgba(255,255,255,0.08)",
            }}
          >
            {busy ? "…" : "Dismiss"}
          </button>
        </div>
        {error && <div style={{ fontSize: 11, color: "#f85149", marginTop: 6 }}>{error} — try again</div>}
      </div>
    );
  }

  const multiple = first.multiple === true;

  const toggle = (label: string) =>
    setSelected((s) => (s.includes(label) ? s.filter((x) => x !== label) : [...s, label]));

  return (
    <div
      role="alertdialog"
      aria-label="Question"
      aria-describedby="neko-q-text"
      aria-live="assertive"
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onCancel();
        }
      }}
      style={{
        marginTop: 10,
        padding: 10,
        borderRadius: 10,
        border: "1px solid #58a6ff",
        background: "rgba(88,166,255,0.08)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, fontWeight: 700 }}>
        <span>Question needs an answer</span>
        {stale && (
          <span
            style={{
              fontSize: 10,
              padding: "1px 6px",
              borderRadius: 9999,
              background: "rgba(88,166,255,0.2)",
            }}
          >
            expired · still answerable
          </span>
        )}
      </div>
      <div style={{ fontSize: 12, opacity: 0.85, marginTop: 4 }}>
        {project} · {age(pending.askedAt)}
      </div>
      <div id="neko-q-text" style={{ fontSize: 12, marginTop: 6, fontWeight: 600 }}>
        {first.header ? `${first.header}: ` : ""}
        {first.question}
      </div>
      {first.custom === true && (
        <div style={{ fontSize: 11, opacity: 0.7, marginTop: 4 }}>
          Free-text answers unsupported — pick an option or use the terminal.
        </div>
      )}
      <div style={{ display: "flex", flexDirection: "column", gap: 6, marginTop: 8 }}>
        {first.options.map((o, i) => {
          const isSel = selected.includes(o.label);
          return (
            <button
              key={o.label}
              ref={i === 0 ? firstRef : undefined}
              disabled={busy !== null}
              aria-label={`Answer ${o.label}`}
              aria-pressed={multiple ? isSel : undefined}
              onClick={() => {
                if (multiple) toggle(o.label);
                else void answer([[o.label]]);
              }}
              title={o.description}
              style={{
                padding: "6px 8px",
                borderRadius: 8,
                border: "1px solid #30363d",
                cursor: busy !== null ? "wait" : "pointer",
                fontSize: 12,
                textAlign: "left",
                color: "#fff",
                background: isSel ? "#1f6feb" : "rgba(255,255,255,0.06)",
                opacity: busy !== null ? 0.6 : 1,
              }}
            >
              {o.label}
              {o.description ? ` — ${o.description}` : ""}
            </button>
          );
        })}
      </div>
      <div style={{ display: "flex", gap: 6, marginTop: 8 }}>
        {multiple && (
          <button
            onClick={() => void answer([selected])}
            disabled={busy !== null || selected.length === 0}
            aria-label="Submit answers"
            style={{
              flex: 1,
              padding: "6px 0",
              borderRadius: 8,
              border: "1px solid #30363d",
              cursor: busy !== null || selected.length === 0 ? "wait" : "pointer",
              fontSize: 12,
              fontWeight: 700,
              color: "#fff",
              background: "#1f6feb",
              opacity: busy !== null || selected.length === 0 ? 0.5 : 1,
            }}
          >
            {busy ? "…" : `Answer${selected.length > 1 ? "s" : ""}`}
          </button>
        )}
        <button
          onClick={() => void dismiss()}
          disabled={busy !== null}
          aria-label="Dismiss question"
          style={{
            flex: multiple ? 1 : undefined,
            width: multiple ? undefined : "100%",
            padding: "6px 0",
            borderRadius: 8,
            border: "1px solid #30363d",
            cursor: busy !== null ? "wait" : "pointer",
            fontSize: 12,
            color: "#fff",
            background: "rgba(248,81,73,0.25)",
          }}
        >
          Dismiss
        </button>
      </div>
      {error && <div style={{ fontSize: 11, color: "#f85149", marginTop: 6 }}>{error} — try again</div>}
    </div>
  );
}
