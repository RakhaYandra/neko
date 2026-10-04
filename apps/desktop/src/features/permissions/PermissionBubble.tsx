import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PendingRequest } from "../../stores/sessions";

const STALE_MS = 5 * 60 * 1000;

type Reply = "once" | "always" | "deny";

// Tab order: Allow → Always → Deny. Deny last so it is not the default.
const BUTTONS: Array<{ reply: Reply; label: string; primary?: boolean }> = [
  { reply: "once", label: "Allow", primary: true },
  { reply: "always", label: "Always" },
  { reply: "deny", label: "Deny" },
];

function age(askedAt: number): string {
  const s = Math.max(0, Math.round((Date.now() - askedAt) / 1000));
  return s < 60 ? `${s}s ago` : `${Math.floor(s / 60)}m ago`;
}

export function PermissionBubble({
  pending,
  project,
  onDone,
  onCancel,
}: {
  pending: PendingRequest;
  project: string;
  onDone: () => void;
  onCancel: () => void;
}) {
  const [busy, setBusy] = useState<Reply | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const stale = now - pending.askedAt > STALE_MS;
  const allowRef = useRef<HTMLButtonElement>(null);
  const liveId = useRef(pending.requestId);

  // Focus Allow so the common answer is one Enter away.
  useEffect(() => {
    liveId.current = pending.requestId;
    setBusy(null);
    setError(null);
    allowRef.current?.focus();
  }, [pending.requestId]);

  // Re-render age/stale badge without new snapshots.
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 30_000);
    return () => clearInterval(t);
  }, []);

  async function answer(reply: Reply) {
    const id = pending.requestId;
    setBusy(reply);
    setError(null);
    try {
      await invoke("reply_permission", { requestId: id, reply });
      if (liveId.current !== id) return; // superseded: session moved on
      onDone();
    } catch (e) {
      if (liveId.current !== id) return;
      setError(e instanceof Error ? e.message : typeof e === "string" ? e : "reply failed");
    } finally {
      if (liveId.current === id) setBusy(null);
    }
  }

  return (
    <div
      role="alertdialog"
      aria-label="Permission request"
      aria-describedby="neko-perm-action"
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
        border: "1px solid #ffb224",
        background: "rgba(255,178,36,0.08)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, fontWeight: 700 }}>
        <span>Permission requested</span>
        {stale && (
          <span
            style={{
              fontSize: 10,
              padding: "1px 6px",
              borderRadius: 9999,
              background: "rgba(255,178,36,0.2)",
            }}
          >
            expired · still answerable
          </span>
        )}
      </div>
      <div style={{ fontSize: 12, opacity: 0.85, marginTop: 4 }}>
        {project} · {age(pending.askedAt)}
      </div>
      <div
        id="neko-perm-action"
        style={{
          fontFamily: "monospace",
          fontSize: 12,
          marginTop: 6,
          padding: 6,
          borderRadius: 6,
          background: "rgba(0,0,0,0.35)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {pending.action}
        {pending.resource ? ` ${pending.resource}` : ""}
      </div>
      <div style={{ display: "flex", gap: 6, marginTop: 8 }}>
        {BUTTONS.map((b) => (
          <button
            key={b.reply}
            ref={b.reply === "once" ? allowRef : undefined}
            disabled={busy !== null}
            aria-label={`${b.label} permission`}
            onClick={() => void answer(b.reply)}
            style={{
              flex: 1,
              padding: "6px 0",
              borderRadius: 8,
              border: "1px solid #30363d",
              cursor: busy !== null ? "wait" : "pointer",
              fontSize: 12,
              fontWeight: b.primary ? 700 : 400,
              color: "#fff",
              background: b.reply === "deny" ? "rgba(248,81,73,0.25)" : b.primary ? "#1f6feb" : "rgba(255,255,255,0.08)",
              opacity: busy !== null && busy !== b.reply ? 0.5 : 1,
            }}
          >
            {busy === b.reply ? "…" : b.label}
          </button>
        ))}
      </div>
      {error && <div style={{ fontSize: 11, color: "#f85149", marginTop: 6 }}>{error} — try again</div>}
    </div>
  );
}

/// Waiting state with no reply key (TUI session or no serve link).
export function WaitingNotice() {
  return (
    <div
      style={{
        marginTop: 10,
        padding: 8,
        borderRadius: 10,
        fontSize: 11,
        opacity: 0.8,
        background: "rgba(255,255,255,0.05)",
      }}
    >
      Waiting on the terminal — no serve connection, answer there.
    </div>
  );
}
