import { create } from "zustand";

interface UiState {
  expanded: boolean;
  toggle: () => void;
  setExpanded: (v: boolean) => void;
  settingsView: boolean;
  setSettingsView: (v: boolean) => void;
  animations: boolean;
  setAnimations: (v: boolean) => void;
  opacity: number;
  setOpacity: (v: number) => void;
}

export const useUi = create<UiState>((set) => ({
  expanded: false,
  toggle: () => set((s) => ({ expanded: !s.expanded, settingsView: s.expanded ? false : s.settingsView })),
  setExpanded: (expanded) => set({ expanded }),
  settingsView: false,
  setSettingsView: (settingsView) => set({ settingsView }),
  animations: true,
  setAnimations: (animations) => set({ animations }),
  opacity: 0.88,
  setOpacity: (opacity) => set({ opacity }),
}));
