import { create } from "zustand";

import { commands, type ClipInfo } from "@/lib/ipc";

interface LibraryState {
  clips: ClipInfo[];
  loaded: boolean;
  loading: boolean;
  load: () => Promise<void>;
  upsert: (c: ClipInfo) => void;
  remove: (path: string) => void;
  setThumb: (path: string, thumb: string) => void;
}

export const useLibrary = create<LibraryState>((set, get) => ({
  clips: [],
  loaded: false,
  loading: false,
  load: async () => {
    if (get().loading) return;
    set({ loading: true });
    try {
      const clips = await commands.listClips();
      set({ clips, loaded: true });
    } finally {
      set({ loading: false });
    }
  },
  upsert: (c) => {
    const rest = get().clips.filter((x) => x.path !== c.path);
    set({ clips: [c, ...rest].sort((a, b) => b.modifiedMs - a.modifiedMs) });
  },
  remove: (path) => set({ clips: get().clips.filter((c) => c.path !== path) }),
  setThumb: (path, thumb) =>
    set({ clips: get().clips.map((c) => (c.path === path ? { ...c, thumbnail: thumb } : c)) }),
}));
