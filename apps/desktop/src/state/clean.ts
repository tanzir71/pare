import { create } from "zustand";
import { api, on } from "../lib/ipc";
import type { CleanDone, CleanProgress } from "../lib/types";
import { useApp } from "./app";
import { useScan } from "./scan";

interface CleanStore {
  running: boolean;
  progress: CleanProgress | null;
  result: CleanDone | null;
  error: string | null;
  /** The Home "Free up" confirmation is shown once per session (DESIGN §3 01). */
  confirmedThisSession: boolean;
  lastIds: string[];
  run: (ids: string[]) => Promise<void>;
  confirmSession: () => void;
}

export const useClean = create<CleanStore>((set) => ({
  running: false,
  progress: null,
  result: null,
  error: null,
  confirmedThisSession: false,
  lastIds: [],
  run: async (ids) => {
    if (ids.length === 0) return;
    set({ running: true, progress: null, error: null, lastIds: ids });
    try {
      await api.clean(ids);
    } catch (e) {
      set({ running: false, error: String(e) });
    }
  },
  confirmSession: () => set({ confirmedThisSession: true }),
}));

let wired = false;
export function wireCleanEvents() {
  if (wired) return;
  wired = true;
  void on("clean://progress", (p) => useClean.setState({ progress: p }));
  void on("clean://error", (e) => useClean.setState({ running: false, error: e }));
  void on("clean://done", (done) => {
    useClean.setState({ running: false, result: done });
    void useScan.getState().reload();
    void useApp.getState().refreshDrives();
    useApp.getState().go({ name: "done" });
  });
}
