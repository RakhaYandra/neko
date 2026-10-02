import { create } from "zustand";

interface UiState {
  expanded: boolean;
  toggle: () => void;
  setExpanded: (v: boolean) => void;
}

export const useUi = create<UiState>((set) => ({
  expanded: false,
  toggle: () => set((s) => ({ expanded: !s.expanded })),
  setExpanded: (expanded) => set({ expanded }),
}));
