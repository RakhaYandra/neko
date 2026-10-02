import { create } from "zustand";

export interface NekoSession {
  id: string;
  project: string;
  status: string;
  lastActivityAt: number;
}

interface SessionsState {
  sessions: NekoSession[];
  selectedId: string | null;
  setSnapshot: (sessions: NekoSession[]) => void;
  select: (id: string | null) => void;
}

// Mirror only: transitions live in Rust (Phase 3). Never infer state here.
export const useSessions = create<SessionsState>((set) => ({
  sessions: [],
  selectedId: null,
  setSnapshot: (sessions) =>
    set((s) => ({
      sessions,
      // Drop selection when its session disappears.
      selectedId: sessions.some((x) => x.id === s.selectedId) ? s.selectedId : null,
    })),
  select: (selectedId) => set({ selectedId }),
}));

// The session driving the companion: explicit pick, else most recent.
export function activeSession(s: SessionsState): NekoSession | null {
  if (s.sessions.length === 0) return null;
  return s.sessions.find((x) => x.id === s.selectedId) ?? s.sessions[0];
}
