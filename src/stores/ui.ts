import { create } from "zustand";

export type Page = "dashboard" | "library" | "settings";

interface UiState {
  page: Page;
  settingsCategory: string;
  go: (p: Page) => void;
  setSettingsCategory: (c: string) => void;
}

export const useUi = create<UiState>((set) => ({
  page: "dashboard",
  settingsCategory: "general",
  go: (page) => set({ page }),
  setSettingsCategory: (settingsCategory) => set({ settingsCategory }),
}));
