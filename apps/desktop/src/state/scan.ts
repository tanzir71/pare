import { create } from "zustand";
import { api, on } from "../lib/ipc";
import type {
  CategorySummary,
  DetectorStatus,
  Drive,
  KnownFolders,
  ProgressPayload,
  ScanRequest,
  ScanSummary,
  Settings,
} from "../lib/types";
import { useApp } from "./app";

/** Scope chip keys: "pc", known folders, or a custom folder path. */
export type ScopeKey = "pc" | "desktop" | "downloads" | "documents" | `path:${string}`;

interface ScanStore {
  scope: ScopeKey[];
  scanId: number | null;
  running: boolean;
  progress: ProgressPayload | null;
  detectors: DetectorStatus[];
  /** Streamed while scanning, then replaced by the summary's. */
  categories: CategorySummary[];
  summary: ScanSummary | null;
  error: string | null;
  toggleScope: (key: ScopeKey) => void;
  start: () => Promise<void>;
  cancel: () => Promise<void>;
  reload: () => Promise<void>;
}

function driveOf(path: string): string {
  const m = /^([A-Za-z]:)/.exec(path);
  return m ? `${m[1]!.toUpperCase()}\\` : path;
}

export function buildRequest(scope: ScopeKey[], drives: Drive[], folders: KnownFolders, settings: Settings | null): ScanRequest {
  if (scope.includes("pc")) {
    const fixed = drives.filter((d) => d.isFixed);
    return {
      roots: fixed.map((d) => ({ path: d.root, label: `This PC (${d.letter})` })),
      drives: fixed.map((d) => d.root),
      wholeDrives: true,
    };
  }
  const roots = scope.map((k) => {
    switch (k) {
      case "desktop":
        return { path: folders.desktop, label: "Desktop" };
      case "downloads":
        return { path: folders.downloads, label: "Downloads" };
      case "documents":
        return { path: folders.documents, label: "Documents" };
      default: {
        const path = k.slice(5);
        const custom = settings?.customRoots.find((r) => r.path === path);
        return { path, label: custom?.label ?? path };
      }
    }
  });
  return { roots, drives: [...new Set(roots.map((r) => driveOf(r.path)))], wholeDrives: false };
}

export const useScan = create<ScanStore>((set, get) => ({
  scope: ["pc"],
  scanId: null,
  running: false,
  progress: null,
  detectors: [],
  categories: [],
  summary: null,
  error: null,
  toggleScope: (key) =>
    set((s) => {
      if (key === "pc") return { scope: ["pc"] };
      const rest = s.scope.filter((k) => k !== "pc");
      const next = rest.includes(key) ? rest.filter((k) => k !== key) : [...rest, key];
      return { scope: next.length ? next : ["pc"] };
    }),
  start: async () => {
    const { drives, folders, settings, go } = useApp.getState();
    if (!folders) return;
    const req = buildRequest(get().scope, drives, folders, settings);
    set({ running: true, progress: null, detectors: [], categories: [], error: null });
    go({ name: "scanning" });
    try {
      const id = await api.scanStart(req);
      set({ scanId: id });
    } catch (e) {
      set({ running: false, error: String(e) });
    }
  },
  cancel: async () => {
    const id = get().scanId;
    if (id != null) await api.scanCancel(id);
  },
  reload: async () => {
    const summary = await api.scanSummary();
    set({ summary, categories: summary?.categories ?? [] });
  },
}));

let wired = false;
/** Subscribe to scan events once for the app's lifetime. */
export function wireScanEvents() {
  if (wired) return;
  wired = true;
  void on("scan://progress", (p) => {
    if (p.scanId === useScan.getState().scanId) useScan.setState({ progress: p });
  });
  void on("scan://event", ({ scanId, event }) => {
    const s = useScan.getState();
    if (scanId !== s.scanId) return;
    if (event.type === "detector") {
      const { type: _t, ...d } = event;
      const list = s.detectors.some((x) => x.id === d.id)
        ? s.detectors.map((x) => (x.id === d.id ? d : x))
        : [...s.detectors, d];
      useScan.setState({ detectors: list });
    } else {
      const { type: _t, ...c } = event;
      const cats = [...s.categories.filter((x) => x.id !== c.id), c].sort(
        (a, b) => b.safeBytes + b.reviewBytes - (a.safeBytes + a.reviewBytes),
      );
      useScan.setState({ categories: cats });
    }
  });
  void on("scan://done", (summary) => {
    const s = useScan.getState();
    if (summary.scanId !== s.scanId) return;
    useScan.setState({
      running: false,
      summary: summary.cancelled && s.summary ? s.summary : summary,
      categories: summary.cancelled && s.summary ? s.summary.categories : summary.categories,
      detectors: summary.detectors,
    });
    const app = useApp.getState();
    if (app.screen.name === "scanning") app.go({ name: "clean" });
  });
}
