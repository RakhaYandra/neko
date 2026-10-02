import { create } from "zustand";

export interface NekoSession {
  id: string;
  project: string;
  status: string;
  lastActivityAt: number;
}

interface SessionsState {
  sessions: NekoSession[];
  setSnapshot: (sessions: NekoSession[]) => void;
}

// Mirror only: transitions live in Rust (Phase 3). Never infer state here.
export const useSessions = create<SessionsState>((set) => ({
  sessions: [],
  setSnapshot: (sessions) => set({ sessions }),
}));
