// Fixture backend for previewing the UI in a browser (`pnpm --filter desktop dev`). It mirrors
// the Figma screens' numbers. It is never used inside the Tauri app.
import type { Backend, Unlisten } from "./ipc";
import type {
  CategoryId,
  CategorySummary,
  CleanDone,
  CleanReport,
  DetectorStatus,
  Disposal,
  Drive,
  Finding,
  HistoryDetail,
  HistoryOp,
  ScanSummary,
  Settings,
  Tier,
} from "./types";

const GB = 1e9;
const MB = 1e6;
const DAY = 86_400_000;
const now = Date.now();
const H = "C:\\Users\\ana";

type Listener = (payload: unknown) => void;
const listeners = new Map<string, Set<Listener>>();
function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((l) => l(payload));
}

let idSeq = 0;
function f(
  category: CategoryId,
  source: string,
  title: string,
  path: string,
  bytes: number,
  tier: Tier,
  disposal: Disposal,
  extra: Partial<Finding> = {},
): Finding {
  idSeq += 1;
  return {
    id: `mock-${idSeq}`,
    ruleId: extra.ruleId ?? `${category}.mock`,
    category,
    source,
    sourcePath: extra.sourcePath ?? null,
    title,
    paths: [path],
    bytes,
    logicalBytes: bytes,
    items: extra.items ?? 1,
    tier,
    disposal,
    comesBack: extra.comesBack ?? "no",
    why: extra.why ?? "Pare found this while scanning.",
    ifRemoved: extra.ifRemoved ?? "It goes to the Recycle Bin, so you can restore it.",
    dates: extra.dates ?? { createdMin: now - 120 * DAY, modifiedMax: now - 60 * DAY, accessedMax: now - 30 * DAY },
    createdBy: extra.createdBy ?? null,
    badge: extra.badge ?? null,
    note: extra.note ?? null,
    git: extra.git ?? null,
    facts: extra.facts ?? [],
  };
}
const SAFE: Tier = { kind: "safe" };
const REVIEW: Tier = { kind: "review" };
const DEL: Disposal = { kind: "delete" };
const REC: Disposal = { kind: "recycle" };

function buildFindings(): Finding[] {
  idSeq = 0;
  const cc = { createdBy: "Claude Code", sourcePath: `${H}\\.claude` };
  const out: Finding[] = [
    f("ai-tools", "Claude Code", "Debug logs > 7 days old", `${H}\\.claude\\debug`, 2.9 * GB, SAFE, DEL, {
      ...cc, items: 214, comesBack: "automatically", note: "9 days ago",
      why: "Diagnostic logs Claude Code writes while it runs. They can grow to several GB.",
      ifRemoved: "Old debug output is gone. Claude Code keeps working and writes new logs.",
    }),
    f("ai-tools", "Claude Code", "Deleted-project transcripts", `${H}\\.claude\\projects\\D--old-app`, 2.1 * GB, SAFE, REC, {
      ...cc, items: 412, badge: "Orphaned", note: "Folders deleted",
      why: "These Claude Code transcripts belong to 23 project folders that no longer exist on this PC.",
      ifRemoved: "You can't resume those old sessions. Claude Code and your current projects keep working.",
      dates: { createdMin: now - 259 * DAY, modifiedMax: now - 153 * DAY, accessedMax: null },
    }),
    f("ai-tools", "Claude Code", "File-history snapshots", `${H}\\.claude\\file-history`, 1.1 * GB, SAFE, REC, {
      ...cc, items: 1210, note: "Since Mar 2026",
      why: "Snapshots Claude Code keeps so it can rewind edits in past sessions.",
      ifRemoved: "You can't rewind edits from those old sessions.",
    }),
    f("ai-tools", "Claude Code", "Transcripts > 30 days (148)", `${H}\\.claude\\projects`, 0.9 * GB, REVIEW, REC, {
      ...cc, items: 148, note: "Opened 2 mo ago",
      why: "Old conversation transcripts.", ifRemoved: "You can't resume or review these sessions.",
    }),
    f("ai-tools", "Claude Code", "Current session · pare", `${H}\\.claude\\projects\\D--code-pare`, 41 * MB,
      { kind: "protected", reason: "in-use", detail: "Open in claude.exe" }, REC, { ...cc, note: "Active now" }),
    f("ai-tools", "Codex", "Archived sessions (312)", `${H}\\.codex\\archived_sessions`, 1.6 * GB, REVIEW, REC, {
      createdBy: "Codex", sourcePath: `${H}\\.codex`, items: 312, note: "Since Jan 2026",
      why: "Sessions you archived. Archiving moves them here but doesn't free space.",
      ifRemoved: "You can't reopen these archived sessions.",
    }),
    f("ai-tools", "Codex", "Old CLI releases (4)", `${H}\\.codex\\packages\\standalone\\releases`, 1.2 * GB, SAFE, DEL, {
      createdBy: "Codex", sourcePath: `${H}\\.codex`, items: 4, badge: "Old versions", note: "In-use kept", comesBack: "on-demand",
    }),
    f("ai-tools", "Cursor & VS Code", "Editor caches and logs", `${H}\\AppData\\Roaming\\Code\\Cache`, 1.3 * GB, SAFE, DEL, {
      createdBy: "VS Code", sourcePath: `${H}\\AppData\\Roaming`, items: 3400, comesBack: "automatically", note: "Rebuilds",
    }),
    f("ai-tools", "Cursor & VS Code", "Large Cursor chat database", `${H}\\AppData\\Roaming\\Cursor\\User\\globalStorage\\state.vscdb`, 0.9 * GB, REVIEW,
      { kind: "guide", steps: ["In Cursor, run 'Developer: Delete Old Chats…' from the command palette.", "Pick an age cutoff; Cursor compacts the database afterwards."] },
      { createdBy: "Cursor", sourcePath: `${H}\\AppData\\Roaming`, note: "Guide" }),
    f("dev-caches", "Node.js package managers", "npm download cache", `${H}\\AppData\\Local\\npm-cache`, 3.2 * GB, SAFE, DEL, { comesBack: "on-demand", items: 18000 }),
    f("dev-caches", "Python tools", "pip cache", `${H}\\AppData\\Local\\pip\\Cache`, 1.4 * GB, SAFE, DEL, { comesBack: "on-demand", items: 900 }),
    f("dev-caches", "Rust, Go, JVM & .NET", "Cargo registry and git caches", `${H}\\.cargo\\registry`, 1.5 * GB, SAFE, DEL, { comesBack: "on-demand", items: 5000 }),
    f("large-files", "Documents", "family-trip-2023.mov", `${H}\\Documents\\family-trip-2023.mov`, 2.4 * GB, REVIEW, REC, { note: "Changed 8 months ago" }),
    f("large-files", "Desktop", "ubuntu-22.04.iso", `${H}\\Desktop\\ubuntu-22.04.iso`, 1.9 * GB, REVIEW, REC, { note: "Changed 1 year ago" }),
    f("downloads", "Installers", "VSCodeUserSetup-x64.exe", `${H}\\Downloads\\VSCodeUserSetup-x64.exe`, 98 * MB, REVIEW, REC, { note: "3 months ago", comesBack: "on-demand" }),
    f("downloads", "Old downloads", "datasets", `${H}\\Downloads\\datasets`, 2.9 * GB, REVIEW, REC, { note: "Changed 7 months ago", items: 120 }),
    f("downloads", "Unfinished downloads", "movie.mkv.crdownload", `${H}\\Downloads\\movie.mkv.crdownload`, 0.2 * GB, SAFE, DEL, { comesBack: "on-demand" }),
    f("recycle-bin", "Recycle Bin", "Recycle Bin on C:", "C:\\", 2.0 * GB, SAFE, { kind: "empty-recycle-bin", drive: "C:\\" }, { items: 342 }),
    f("junk", "Windows", "Temporary files older than 1 day", `${H}\\AppData\\Local\\Temp`, 0.9 * GB, SAFE, DEL, { items: 2410, comesBack: "automatically" }),
    f("empty-folders", "Documents", "old-notes", `${H}\\Documents\\old-notes`, 0, SAFE, DEL, { items: 0 }),
  ];
  return out;
}

let findings = buildFindings();

function summarize(list: Finding[]): CategorySummary[] {
  const meta: Record<CategoryId, [string, string, string, CategorySummary["hue"]]> = {
    "ai-tools": ["AI tool caches", "Claude Code, Codex and Cursor sessions, logs and caches", "Sparkles", "violet"],
    "project-build": ["Old project build files", "node_modules, target and .venv in idle projects", "Package", "orange"],
    "dev-caches": ["Developer caches", "npm, pnpm, pip, uv and Cargo download caches", "Terminal", "teal"],
    "large-files": ["Large files", "Files over 500 MB you haven't changed in months", "File", "pink"],
    downloads: ["Downloads", "Installers and archives you've already used", "Download", "blue"],
    "recycle-bin": ["Recycle Bin", "Files you already deleted, still taking up space", "Trash2", "slate"],
    duplicates: ["Duplicates", "Exact copies of the same file", "Copy", "amber"],
    junk: ["Junk & temp files", "Windows temp files, crash dumps and thumbnail cache", "Archive", "green"],
    browsers: ["Browser caches", "Cached pages and scripts from browsers that are closed", "Monitor", "blue"],
    screenshots: ["Old screenshots", "Screenshots older than 3 months in Pictures", "Image", "blue"],
    "empty-folders": ["Empty folders", "Folders with nothing inside", "FolderOpen", "slate"],
  };
  const map = new Map<CategoryId, CategorySummary>();
  for (const x of list) {
    const [title, description, icon, hue] = meta[x.category];
    const s =
      map.get(x.category) ??
      ({
        id: x.category, title, description, icon, hue,
        safeBytes: 0, reviewBytes: 0, protectedBytes: 0, items: 0, findings: 0, sources: 0,
        protectedFindings: 0, freesNowBytes: 0, recycleBytes: 0,
      } satisfies CategorySummary);
    map.set(x.category, s);
    if (x.tier.kind === "protected") {
      s.protectedBytes += x.bytes;
      s.protectedFindings += 1;
      continue;
    }
    if (x.disposal.kind === "guide") continue;
    if (x.tier.kind === "safe") s.safeBytes += x.bytes;
    else s.reviewBytes += x.bytes;
    if (x.disposal.kind === "recycle") s.recycleBytes += x.bytes;
    else s.freesNowBytes += x.bytes;
    s.items += x.items;
    s.findings += 1;
  }
  for (const s of map.values()) {
    s.sources = new Set(list.filter((x) => x.category === s.id).map((x) => x.source)).size;
  }
  return [...map.values()].sort((a, b) => b.safeBytes + b.reviewBytes - (a.safeBytes + a.reviewBytes));
}

const drives: Drive[] = [
  { root: "C:\\", letter: "C:", label: "Windows", fs: "NTFS", total: 512 * GB, free: 100 * GB, isNtfs: true, isFixed: true },
  { root: "D:\\", letter: "D:", label: "Data", fs: "NTFS", total: 1000 * GB, free: 640 * GB, isNtfs: true, isFixed: true },
];

let settings: Settings = {
  theme: "system", exclusions: [], customRoots: [{ path: "D:\\code", label: "D:\\code" }], projectRoots: [],
  thresholds: { largeFileMb: 500, largeFileDays: 90, installerDays: 7, partialDays: 1, oldDownloadDays: 180, idleDays: 90, screenshotDays: 90, duplicateMinMb: 1, recentHours: 24 },
  cachesToRecycleBin: false, purgePareItemsAfterDays: 30, hiddenCards: [], checkForUpdates: false, onboardingDone: false, language: "en",
};

let summary: ScanSummary | null = null;
const history: HistoryDetail[] = [];
let scanSeq = 0;

const DETECTORS: [string, string][] = [
  ["junk", "Junk & temp files"],
  ["ai-tools", "AI tool caches"],
  ["dev-caches", "Developer caches"],
  ["projects", "Git projects"],
  ["duplicates", "Duplicates"],
  ["large-files", "Large & old files"],
];

function runScan(id: number) {
  const started = Date.now();
  const detectors: DetectorStatus[] = DETECTORS.map(([d, label]) => ({ id: d, label, state: "waiting", bytes: 0 }));
  detectors.forEach((d) => emit("scan://event", { scanId: id, event: { type: "detector", ...d } }));
  let tick = 0;
  const total = 16;
  const timer = setInterval(() => {
    tick += 1;
    const fraction = Math.min(tick / total, 1);
    const di = Math.floor(fraction * detectors.length);
    detectors.forEach((d, i) => {
      const state = i < di ? "done" : i === di ? "running" : "waiting";
      if (d.state !== state) {
        d.state = state;
        const cats = summarize(findings);
        d.bytes = cats.find((c) => c.id === d.id)?.safeBytes ?? 0;
        emit("scan://event", { scanId: id, event: { type: "detector", ...d } });
        if (state === "done") {
          const c = cats.find((c) => c.id === d.id);
          if (c) emit("scan://event", { scanId: id, event: { type: "category", ...c } });
        }
      }
    });
    emit("scan://progress", {
      scanId: id, phase: fraction < 0.7 ? "enumerating" : fraction < 1 ? "detecting" : "done",
      files: Math.round(1_240_000 * fraction), dirs: 0, bytesSeen: 400 * GB * fraction,
      foundBytes: 38.6 * GB * fraction, current: "C:\\Users\\ana", elapsedMs: Date.now() - started,
      fraction, detectorsDone: di, detectorsTotal: detectors.length,
    });
    if (tick >= total) {
      clearInterval(timer);
      detectors.forEach((d) => (d.state = "done"));
      summary = {
        scanId: id, running: false, started, finished: Date.now(), cancelled: false,
        roots: [{ path: "C:\\", label: "This PC (C:)" }, { path: "D:\\", label: "This PC (D:)" }],
        categories: summarize(findings), files: 1_240_000, elapsedMs: Date.now() - started,
        protected: [{ reason: "uncommitted", count: 3 }, { reason: "in-use", count: 2 }, { reason: "recently-modified", count: 41 }],
        detectors,
      };
      emit("scan://done", summary);
    }
  }, 180);
}

function clean(ids: string[]) {
  const sel = findings.filter((x) => ids.includes(x.id) && x.tier.kind !== "protected" && x.disposal.kind !== "guide");
  setTimeout(() => {
    const opId = `op-mock-${Date.now()}`;
    const results = sel.map((x) => ({
      findingId: x.id, title: x.title, category: x.category, path: x.paths[0] ?? "", disposal: x.disposal.kind,
      status: "ok" as const, freed: x.disposal.kind === "recycle" ? 0 : x.bytes,
      recycled: x.disposal.kind === "recycle" ? x.bytes : 0, error: null, reason: null, output: null,
    }));
    const byCat = new Map<CategoryId, { category: CategoryId; title: string; freed: number; recycled: number; items: number }>();
    for (const r of results) {
      const t = byCat.get(r.category) ?? { category: r.category, title: summarize(findings).find((c) => c.id === r.category)?.title ?? r.category, freed: 0, recycled: 0, items: 0 };
      t.freed += r.freed; t.recycled += r.recycled; t.items += 1;
      byCat.set(r.category, t);
    }
    const report: CleanReport = {
      opId, started: Date.now() - 1500, finished: Date.now(),
      freedNow: results.reduce((a, r) => a + r.freed, 0), recycled: results.reduce((a, r) => a + r.recycled, 0),
      ok: results.length, failed: 0, skipped: 0, results,
      byCategory: [...byCat.values()].sort((a, b) => b.freed + b.recycled - a.freed - a.recycled), summary: null,
    };
    const freed = report.freedNow;
    const after = drives.map((d) => (d.letter === "C:" ? { ...d, free: d.free + freed } : d));
    findings = findings.filter((x) => !ids.includes(x.id));
    if (summary) summary = { ...summary, categories: summarize(findings) };
    const op: HistoryOp = {
      id: opId, started: report.started, finished: report.finished, title: "Free up", items: results.length,
      ok: results.length, failed: 0, skipped: 0, freedNow: report.freedNow, recycled: report.recycled,
      disposals: results.reduce<Record<string, number>>((a, r) => ({ ...a, [r.disposal]: (a[r.disposal] ?? 0) + 1 }), {}),
      restorable: results.filter((r) => r.recycled > 0).length,
    };
    history.unshift({
      op,
      items: results.map((r) => ({ findingId: r.findingId, title: r.title, path: r.path, paths: [], bytes: r.freed + r.recycled, rule: "mock", category: r.category, disposal: r.disposal, result: "ok", error: null, freed: r.freed, recycled: r.recycled, ts: Date.now(), output: null, restored: false, purged: false })),
    });
    const done: CleanDone = { report, drivesBefore: drives, drivesAfter: after };
    emit("clean://done", done);
  }, 1200);
}

export const mockBackend: Backend = {
  async invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
    const r = (v: unknown) => v as T;
    switch (cmd) {
      case "app_info":
        return r({ version: "0.1.0", coreVersion: "0.1.0", platform: "browser preview", logDir: `${H}\\AppData\\Local\\Pare\\logs`, settingsPath: `${H}\\AppData\\Roaming\\Pare\\settings.json` });
      case "list_drives":
        return r(drives);
      case "known_folders":
        return r({ home: H, desktop: `${H}\\Desktop`, downloads: `${H}\\Downloads`, documents: `${H}\\Documents`, pictures: `${H}\\Pictures` });
      case "scan_start": {
        scanSeq += 1;
        findings = buildFindings();
        const id = scanSeq;
        setTimeout(() => runScan(id), 50);
        return r(id);
      }
      case "scan_cancel":
        return r(undefined);
      case "scan_summary":
        return r(summary);
      case "category_get": {
        const id = args.id as CategoryId;
        return r({ summary: summarize(findings).find((c) => c.id === id) ?? null, findings: findings.filter((x) => x.category === id).sort((a, b) => b.bytes - a.bytes) });
      }
      case "finding_get":
        return r(findings.find((x) => x.id === args.id) ?? null);
      case "protected_list":
        return r(findings.filter((x) => x.tier.kind === "protected"));
      case "clean":
        clean(args.ids as string[]);
        return r(undefined);
      case "history_list":
        return r(history.map((h) => h.op));
      case "history_get":
        return r(history.find((h) => h.op.id === args.op) ?? null);
      case "history_restore":
        return r({ restored: 1, collisions: [], missing: [], failed: [] });
      case "recycle_purge_pare_items":
        return r({ items: 3, bytes: 2.1 * GB, failed: 0 });
      case "settings_get":
        return r(settings);
      case "settings_set":
        settings = { ...settings, ...(args.patch as Partial<Settings>) };
        return r(settings);
      case "exclusion_add":
        settings = { ...settings, exclusions: [...settings.exclusions, args.path as string] };
        findings = findings.map((x) => (x.paths.includes(args.path as string) ? { ...x, tier: { kind: "protected", reason: "excluded" } } : x));
        if (summary) summary = { ...summary, categories: summarize(findings) };
        return r(settings);
      case "open_in_explorer":
        return r(undefined);
      default:
        throw new Error(`mock: unknown command ${cmd}`);
    }
  },
  async listen<T>(event: string, cb: (payload: T) => void): Promise<Unlisten> {
    const set = listeners.get(event) ?? new Set<Listener>();
    const l = cb as Listener;
    set.add(l);
    listeners.set(event, set);
    return () => set.delete(l);
  },
};
