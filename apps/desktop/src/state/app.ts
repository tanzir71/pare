import { create } from "zustand";
import { api } from "../lib/ipc";
import type { AppInfo, CategoryId, Drive, KnownFolders, Settings } from "../lib/types";

export type Screen =
  | { name: "clean" }
  | { name: "scanning" }
  | { name: "review"; category: CategoryId }
  | { name: "done" }
  | { name: "projects" }
  | { name: "browse" }
  | { name: "history"; op?: string }
  | { name: "settings" };

interface AppStore {
  screen: Screen;
  ready: boolean;
  drives: Drive[];
  folders: KnownFolders | null;
  settings: Settings | null;
  info: AppInfo | null;
  go: (s: Screen) => void;
  init: () => Promise<void>;
  refreshDrives: () => Promise<void>;
  updateSettings: (patch: Partial<Settings>) => Promise<void>;
  setSettings: (s: Settings) => void;
}

export const useApp = create<AppStore>((set) => ({
  screen: { name: "clean" },
  ready: false,
  drives: [],
  folders: null,
  settings: null,
  info: null,
  go: (screen) => set({ screen }),
  init: async () => {
    const [drives, folders, settings, info] = await Promise.all([
      api.listDrives(),
      api.knownFolders(),
      api.settingsGet(),
      api.appInfo(),
    ]);
    set({ drives, folders, settings, info, ready: true });
  },
  refreshDrives: async () => set({ drives: await api.listDrives() }),
  updateSettings: async (patch) => set({ settings: await api.settingsSet(patch) }),
  setSettings: (settings) => set({ settings }),
}));

/** The drive shown in the sidebar meter: the system drive, else the first fixed drive. */
export function primaryDrive(drives: Drive[]): Drive | undefined {
  return drives.find((d) => d.letter.toUpperCase() === "C:") ?? drives.find((d) => d.isFixed) ?? drives[0];
}

export function driveLabel(d: Drive): string {
  return `${d.label || "Local Disk"} (${d.letter})`;
}
