import { create } from "zustand";

import { commands, errorMessage, type EncoderInfo, type Settings } from "@/lib/ipc";
import { useToasts } from "./toasts";

interface SettingsState {
  settings: Settings | null;
  encoders: EncoderInfo[];
  saving: boolean;
  load: () => Promise<void>;
  /** Ändert die Einstellungen lokal sofort und speichert entprellt im Core. */
  update: (fn: (s: Settings) => Settings) => void;
  replace: (s: Settings) => void;
  setEncoders: (e: EncoderInfo[]) => void;
}

let saveTimer: number | undefined;

export const useSettings = create<SettingsState>((set, get) => ({
  settings: null,
  encoders: [],
  saving: false,
  load: async () => {
    const [settings, encoders] = await Promise.all([
      commands.getSettings(),
      commands.getEncoders(),
    ]);
    set({ settings, encoders });
  },
  update: (fn) => {
    const cur = get().settings;
    if (!cur) return;
    const next = fn(cur);
    set({ settings: next, saving: true });
    window.clearTimeout(saveTimer);
    saveTimer = window.setTimeout(async () => {
      try {
        const saved = await commands.updateSettings(get().settings ?? next);
        set({ settings: saved, saving: false });
      } catch (e) {
        set({ saving: false });
        useToasts.getState().push({ kind: "error", title: errorMessage(e) });
      }
    }, 300);
  },
  replace: (settings) => set({ settings }),
  setEncoders: (encoders) => set({ encoders }),
}));
