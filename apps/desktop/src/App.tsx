import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { MotionConfig } from "motion/react";
import { activeSession, useSessions, type NekoSession, type PendingRequest, type PendingQuestion } from "./stores/sessions";
import { useUi } from "./stores/ui";
import { Companion } from "./features/companion/Companion";
import { SessionsList } from "./features/companion/SessionsList";
import { PermissionBubble, WaitingNotice } from "./features/permissions/PermissionBubble";
import { QuestionCard } from "./features/questions/QuestionCard";
import { Settings } from "./features/settings/Settings";

const COMPACT_H = 160;
const ROW_H = 30;
const CHROME_H = 118;
const BUBBLE_H = 190;
const QUESTION_H = 230;
const SETTINGS_H = 330;

function isSnapshot(v: unknown): v is { sessions: NekoSession[]; pending?: PendingRequest[]; questions?: PendingQuestion[] } {
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
  const pending = useSessions((s) => s.pending);
  const questions = useSessions((s) => s.questions);
  const selectedId = useSessions((s) => s.selectedId);
  const select = useSessions((s) => s.select);
  const setSnapshot = useSessions((s) => s.setSnapshot);
  const expanded = useUi((s) => s.expanded);
  const toggle = useUi((s) => s.toggle);
  const setExpanded = useUi((s) => s.setExpanded);
  const settingsView = useUi((s) => s.settingsView);
  const setSettingsView = useUi((s) => s.setSettingsView);
  const animations = useUi((s) => s.animations);
  const opacity = useUi((s) => s.opacity);

  useEffect(() => {
    let off: (() => void) | undefined;
    listen<string>("neko-event", (e) => {
      try {
        const v: unknown = JSON.parse(e.payload);
        if (isSnapshot(v)) setSnapshot(v.sessions, v.pending ?? [], v.questions ?? []);
      } catch {
        // Non-snapshot payloads are ignored by the Phase 6 UI.
      }
    })
      .then((f) => (off = f))
      .catch(() => {
        // Tauri event unavailable (tests/headless): UI stays empty.
      });
    return () => off?.();
  }, [setSnapshot]);

  // Tray menu actions (Rust emits these; see src-tauri/src/tray.rs).
  useEffect(() => {
    // Window surely exists here; Rust setup runs too early (silent no-op).
    invoke("recenter", {}).catch(() => {});
    let off: (() => void) | undefined;
    listen<string>("neko-ui", (e) => {
      let action = e.payload;
      try {
        const parsed: unknown = JSON.parse(e.payload);
        if (typeof parsed === "string") action = parsed;
      } catch {
        action = e.payload.replace(/^"|"$/g, "");
      }
      if (action === "expand") {
        setSettingsView(false);
        setExpanded(true);
      } else if (action === "settings") {
        setExpanded(true);
        setSettingsView(true);
      }
    })
      .then((f) => (off = f))
      .catch(() => {});
    return () => off?.();
  }, [setExpanded, setSettingsView]);

  const activePending = pending.find((p) => p.sessionId === active?.id) ?? null;
  const activeQuestion = questions.find((q) => q.sessionId === active?.id) ?? null;
  const waitingNoKey = active?.status === "waiting_permission" && activePending === null;

  // Auto-expand when the active session needs the user (master §17).
  useEffect(() => {
    if (activePending) setExpanded(true);
  }, [activePending?.requestId, setExpanded]);

  useEffect(() => {
    if (activeQuestion) setExpanded(true);
  }, [activeQuestion?.requestId, setExpanded]);

  // Grow the window when expanded so the list never clips.
  useEffect(() => {
    let h = COMPACT_H;
    if (expanded) {
      h = settingsView
        ? SETTINGS_H
        : CHROME_H +
          (activePending || waitingNoKey ? BUBBLE_H : 0) +
          (activeQuestion && !activePending ? QUESTION_H : 0) +
          sessions.length * ROW_H;
    }
    getCurrentWindow()
      .setSize(new LogicalSize(320, h))
      .catch(() => {
        // Wayland compositors may ignore programmatic resize (ADR-002).
      });
  }, [expanded, settingsView, sessions.length, activePending, activeQuestion, waitingNoKey]);

  return (
    <MotionConfig reducedMotion={animations ? "user" : "never"}>
      <main
        onKeyDown={(e) => {
          if (e.key === "Escape") setExpanded(false);
        }}
        style={{
          fontFamily: "'JetBrains Mono', 'ui-monospace', 'monospace'",
          padding: 14,
          color: "#c9d1d9",
          background: `rgba(13,17,23,${opacity})`,
          borderRadius: 14,
          border: "1px solid #30363d",
          minHeight: "100vh",
          boxSizing: "border-box",
        }}
      >
        <button
          data-tauri-drag-region
          onClick={toggle}
          aria-expanded={expanded}
          aria-label={expanded ? "Collapse companion" : "Expand companion"}
          style={{
            cursor: "pointer",
            background: "none",
            border: "none",
            padding: 0,
            width: "100%",
            textAlign: "left",
            color: "inherit",
          }}
        >
          <Companion session={active} />
        </button>
        {expanded && activePending && active && (
          <PermissionBubble
            pending={activePending}
            project={active.project}
            onDone={() => setExpanded(false)}
            onCancel={() => setExpanded(false)}
          />
        )}
        {expanded && !activePending && waitingNoKey && <WaitingNotice />}
        {expanded && !activePending && activeQuestion && active && (
          <QuestionCard
            pending={activeQuestion}
            project={active.project}
            onDone={() => setExpanded(false)}
            onCancel={() => setExpanded(false)}
          />
        )}
        {expanded && !settingsView && (
          <SessionsList sessions={sessions} selectedId={selectedId} onSelect={select} />
        )}
        {expanded && settingsView && <Settings />}
      </main>
    </MotionConfig>
  );
}
