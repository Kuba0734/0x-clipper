import { create } from "zustand";

import type { Status } from "@/lib/ipc";

interface StatusState {
  status: Status | null;
  set: (s: Status) => void;
}

export const useStatus = create<StatusState>((set) => ({
  status: null,
  set: (status) => set({ status }),
}));
