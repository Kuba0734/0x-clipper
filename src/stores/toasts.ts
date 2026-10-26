import { create } from "zustand";

import { EXIT_MS } from "@/lib/anim";

export type ToastKind = "success" | "error" | "info";

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  detail?: string;
  /** Anzeigedauer bis zum Auto-Dismiss (Fortschrittsleiste läuft so lange). */
  durationMs: number;
  /** Läuft gerade die Ausblend-Animation? */
  leaving?: boolean;
}

interface ToastState {
  toasts: Toast[];
  push: (t: Omit<Toast, "id" | "durationMs" | "leaving">) => void;
  dismiss: (id: number) => void;
}

let nextId = 1;

export const useToasts = create<ToastState>((set, get) => ({
  toasts: [],
  push: (t) => {
    const id = nextId++;
    const durationMs = t.kind === "error" ? 5000 : 3000;
    // Höchstens drei gleichzeitig; ältere gehen sofort.
    set({ toasts: [...get().toasts.slice(-2), { ...t, id, durationMs }] });
    window.setTimeout(() => get().dismiss(id), durationMs);
  },
  dismiss: (id) => {
    const cur = get().toasts.find((t) => t.id === id);
    if (!cur || cur.leaving) return;
    // Erst ausblenden lassen, dann entfernen.
    set({ toasts: get().toasts.map((t) => (t.id === id ? { ...t, leaving: true } : t)) });
    window.setTimeout(() => set({ toasts: get().toasts.filter((t) => t.id !== id) }), EXIT_MS);
  },
}));
