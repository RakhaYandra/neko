import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { MotionConfig } from "motion/react";
import { activeSession, useSessions, type NekoSession } from "./stores/sessions";
import { useUi } from "./stores/ui";
import { Companion } from "./features/companion/Companion";
import { SessionsList } from "./features/companion/SessionsList";

const COMPACT_H = 160;
const ROW_H = 30;
const CHROME_H = 118;

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
  const active = useSessions(activeSession);
  const selectedId = useSessions((s) => s.selectedId);
  const select = useSessions((s) => s.select);
  const setSnapshot = useSessions((s) => s.setSnapshot);
  const expanded = useUi((s) => s.expanded);
  const toggle = useUi((s) => s.toggle);

  useEffect(() => {
    let off: (() => void) | undefined;
    listen<string>("neko-event", (e) => {
      try {
        const v: unknown = JSON.parse(e.payload);
        if (isSnapshot(v)) setSnapshot(v.sessions);
      } catch {
        // Non-snapshot payloads are ignored by the Phase 4 UI.
      }
    }).then((f) => (off = f));
    return () => off?.();
  }, [setSnapshot]);

  // Grow the window when expanded so the list never clips.
  useEffect(() => {
    const h = expanded ? CHROME_H + sessions.length * ROW_H : COMPACT_H;
    getCurrentWindow()
      .setSize(new LogicalSize(320, h))
      .catch(() => {
        // Wayland compositors may ignore programmatic resize (ADR-002).
      });
  }, [expanded, sessions.length]);

  return (
    <MotionConfig reducedMotion="user">
      <main
        style={{
          fontFamily: "sans-serif",
          padding: 14,
          color: "#fff",
          background: "rgba(13,17,23,0.88)",
          borderRadius: 14,
          border: "1px solid #30363d",
          minHeight: "100vh",
          boxSizing: "border-box",
        }}
      >
        <div
          data-tauri-drag-region
          onClick={toggle}
          title={expanded ? "Collapse" : "Expand"}
          style={{ cursor: "pointer" }}
        >
          <Companion session={active} />
        </div>
        {expanded && (
          <SessionsList sessions={sessions} selectedId={selectedId} onSelect={select} />
        )}
      </main>
    </MotionConfig>
  );
}
