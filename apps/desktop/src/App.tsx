import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useSessions, type NekoSession } from "./stores/sessions";

function isSnapshot(v: unknown): v is { sessions: NekoSession[] } {
  return (
    typeof v === "object" &&
    v !== null &&
    (v as { type?: unknown }).type === "sessions.snapshot" &&
    Array.isArray((v as { sessions?: unknown }).sessions)
  );
}

export default function App() {
  const sessions = useSessions((s) => s.sessions);
  const setSnapshot = useSessions((s) => s.setSnapshot);
  useEffect(() => {
    let off: (() => void) | undefined;
    listen<string>("neko-event", (e) => {
      try {
        const v: unknown = JSON.parse(e.payload);
        if (isSnapshot(v)) setSnapshot(v.sessions);
      } catch {
        // Non-snapshot payloads are ignored by the minimal Phase 3 UI.
      }
    }).then((f) => (off = f));
    return () => off?.();
  }, [setSnapshot]);
  return (
    <main style={{ fontFamily: "sans-serif", padding: 16, color: "#fff", background: "rgba(20,20,28,0.85)", borderRadius: 12 }}>
      <div>🐱 {sessions.length > 0 ? `${sessions.length} session${sessions.length > 1 ? "s" : ""}` : "waiting for OpenCode…"}</div>
      {sessions.map((s) => (
        <div key={s.id} style={{ fontSize: 12, opacity: 0.9 }}>
          ● {s.project} — {s.status}
        </div>
      ))}
    </main>
  );
}
