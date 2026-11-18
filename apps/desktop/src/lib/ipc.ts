// Typed wrappers over Tauri invoke/listen. Outside Tauri (plain `vite dev` in a browser) a mock
// backend with fixture data is used so screens can be previewed and tested; it never touches
// the file system.
import type {
  AppInfo,
  CategoryDetail,
  CategoryId,
  CleanDone,
  CleanProgress,
  Drive,
  Finding,
  HistoryDetail,
  HistoryOp,
  KnownFolders,
  ProjectsView,
  ProgressPayload,
  PurgeReport,
  RestoreReport,
  ScanEventPayload,
  ScanRequest,
  ScanSummary,
  Settings,
} from "./types";

export type Unlisten = () => void;

export interface Backend {
  invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  listen<T>(event: string, cb: (payload: T) => void): Promise<Unlisten>;
}

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

let backendPromise: Promise<Backend> | null = null;

function backend(): Promise<Backend> {
  if (!backendPromise) {
    backendPromise = isTauri
      ? Promise.all([import("@tauri-apps/api/core"), import("@tauri-apps/api/event")]).then(
          ([core, event]) => ({
            invoke: <T>(cmd: string, args?: Record<string, unknown>) => core.invoke<T>(cmd, args),
            listen: async <T>(name: string, cb: (payload: T) => void) => {
              const un = await event.listen<T>(name, (e) => cb(e.payload));
              return un;
            },
          }),
        )
      : import("./mock").then((m) => m.mockBackend);
  }
  return backendPromise;
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const b = await backend();
  return b.invoke<T>(cmd, args);
}

export const api = {
  appInfo: () => call<AppInfo>("app_info"),
  listDrives: () => call<Drive[]>("list_drives"),
  knownFolders: () => call<KnownFolders>("known_folders"),
  scanStart: (request: ScanRequest) => call<number>("scan_start", { request }),
  scanCancel: (scanId: number) => call<void>("scan_cancel", { scanId }),
  scanSummary: () => call<ScanSummary | null>("scan_summary"),
  categoryGet: (id: CategoryId) => call<CategoryDetail>("category_get", { id }),
  findingGet: (id: string) => call<Finding | null>("finding_get", { id }),
  protectedList: () => call<Finding[]>("protected_list"),
  clean: (ids: string[]) => call<void>("clean", { ids }),
  historyList: () => call<HistoryOp[]>("history_list"),
  historyGet: (op: string) => call<HistoryDetail | null>("history_get", { op }),
  historyRestore: (op: string, items?: string[]) =>
    call<RestoreReport>("history_restore", { op, items: items ?? null }),
  recyclePurgePareItems: (op?: string, olderThanDays?: number) =>
    call<PurgeReport>("recycle_purge_pare_items", { op: op ?? null, olderThanDays: olderThanDays ?? null }),
  settingsGet: () => call<Settings>("settings_get"),
  settingsSet: (patch: Partial<Settings>) => call<Settings>("settings_set", { patch }),
  exclusionAdd: (path: string) => call<Settings>("exclusion_add", { path }),
  openInExplorer: (path: string) => call<void>("open_in_explorer", { path }),
  projectsList: () => call<ProjectsView>("projects_list"),
  projectsScan: () => call<ProjectsView>("projects_scan"),
};

export interface Events {
  "scan://progress": ProgressPayload;
  "scan://event": ScanEventPayload;
  "scan://done": ScanSummary;
  "clean://progress": CleanProgress;
  "clean://done": CleanDone;
  "clean://error": string;
}

export async function on<K extends keyof Events>(
  event: K,
  cb: (payload: Events[K]) => void,
): Promise<Unlisten> {
  const b = await backend();
  return b.listen<Events[K]>(event, cb);
}

/** Custom title bar caption buttons. No-ops outside Tauri. */
export const windowControls = {
  async minimize() {
    if (!isTauri) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().minimize();
  },
  async toggleMaximize() {
    if (!isTauri) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().toggleMaximize();
  },
  async close() {
    if (!isTauri) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
  },
  async isMaximized(): Promise<boolean> {
    if (!isTauri) return false;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow().isMaximized();
  },
  async onResized(cb: () => void): Promise<Unlisten> {
    if (!isTauri) return () => {};
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow().onResized(cb);
  },
};
