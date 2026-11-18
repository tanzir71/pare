// Mirrors the serde JSON of pare-core and the Tauri commands (src-tauri/src/commands.rs).
// Keep in sync with crates/pare-core/src/types.rs.

export type Timestamp = number; // Unix ms

export type CategoryId =
  | "ai-tools"
  | "project-build"
  | "dev-caches"
  | "large-files"
  | "downloads"
  | "recycle-bin"
  | "duplicates"
  | "junk"
  | "browsers"
  | "screenshots"
  | "empty-folders";

export type Hue = "violet" | "blue" | "teal" | "orange" | "pink" | "green" | "amber" | "slate";

export type ProtectReason =
  | "recently-modified"
  | "in-use"
  | "tool-running"
  | "uncommitted"
  | "system-path"
  | "cloud-only"
  | "excluded"
  | "changed-since-scan"
  | "never-touch";

export type Tier =
  | { kind: "safe" }
  | { kind: "review" }
  | { kind: "protected"; reason: ProtectReason; detail?: string };

export type ComesBack = "automatically" | "on-demand" | "no";

export type Disposal =
  | { kind: "delete" }
  | { kind: "recycle" }
  | { kind: "command"; program: string; args: string[] }
  | { kind: "guide"; steps: string[] }
  | { kind: "empty-recycle-bin"; drive: string };

export interface Dates {
  createdMin: Timestamp | null;
  modifiedMax: Timestamp | null;
  accessedMax: Timestamp | null;
}

export interface GitInfo {
  repo: string;
  branch: string | null;
  lastCommit: Timestamp | null;
  commits: number;
  dirty: boolean;
  dirtyFiles: number;
  ahead: number;
  hasRemote: boolean;
  hasUpstream: boolean;
  stashes: number;
  detached: boolean;
}

export interface Finding {
  id: string;
  ruleId: string;
  category: CategoryId;
  source: string;
  sourcePath: string | null;
  title: string;
  paths: string[];
  bytes: number;
  logicalBytes: number;
  items: number;
  tier: Tier;
  disposal: Disposal;
  comesBack: ComesBack;
  why: string;
  ifRemoved: string;
  dates: Dates;
  createdBy: string | null;
  badge: string | null;
  note: string | null;
  git: GitInfo | null;
  facts: [string, string][];
}

export interface CategorySummary {
  id: CategoryId;
  title: string;
  description: string;
  icon: string;
  hue: Hue;
  safeBytes: number;
  reviewBytes: number;
  protectedBytes: number;
  items: number;
  findings: number;
  sources: number;
  protectedFindings: number;
  freesNowBytes: number;
  recycleBytes: number;
}

export interface CategoryDetail {
  summary: CategorySummary | null;
  findings: Finding[];
}

export interface Drive {
  root: string;
  letter: string;
  label: string;
  fs: string;
  total: number;
  free: number;
  isNtfs: boolean;
  isFixed: boolean;
}

export interface KnownFolders {
  home: string;
  desktop: string;
  downloads: string;
  documents: string;
  pictures: string;
}

export interface ScanRoot {
  path: string;
  label: string;
}

export interface ScanRequest {
  roots: ScanRoot[];
  drives: string[];
  wholeDrives: boolean;
}

export type Phase = "starting" | "enumerating" | "detecting" | "measuring" | "done" | "cancelled";

export interface ProgressPayload {
  scanId: number;
  phase: Phase;
  files: number;
  dirs: number;
  bytesSeen: number;
  foundBytes: number;
  current: string;
  elapsedMs: number;
  fraction: number;
  detectorsDone: number;
  detectorsTotal: number;
}

export type DetectorState = "waiting" | "running" | "done";

export interface DetectorStatus {
  id: string;
  label: string;
  state: DetectorState;
  bytes: number;
}

export type ScanEvent = ({ type: "detector" } & DetectorStatus) | ({ type: "category" } & CategorySummary);

export interface ScanEventPayload {
  scanId: number;
  event: ScanEvent;
}

export interface ScanSummary {
  scanId: number;
  running: boolean;
  started: Timestamp;
  finished: Timestamp;
  cancelled: boolean;
  roots: ScanRoot[];
  categories: CategorySummary[];
  files: number;
  elapsedMs: number;
  protected: { reason: ProtectReason; count: number }[];
  detectors: DetectorStatus[];
}

export type ItemStatus = "pending" | "ok" | "partial" | "failed" | "skipped";

export type FailReason =
  | "in-use"
  | "access-denied"
  | "not-found"
  | "protected"
  | "changed-since-scan"
  | "command-failed"
  | "log-unavailable"
  | "other";

export interface CleanItemResult {
  findingId: string;
  title: string;
  category: CategoryId;
  path: string;
  disposal: string;
  status: ItemStatus;
  freed: number;
  recycled: number;
  error: string | null;
  reason: FailReason | null;
  output: string | null;
}

export interface CleanReport {
  opId: string;
  started: Timestamp;
  finished: Timestamp;
  freedNow: number;
  recycled: number;
  ok: number;
  failed: number;
  skipped: number;
  results: CleanItemResult[];
  byCategory: { category: CategoryId; title: string; freed: number; recycled: number; items: number }[];
  summary: string | null;
}

export interface CleanDone {
  report: CleanReport;
  drivesBefore: Drive[];
  drivesAfter: Drive[];
}

export interface CleanProgress {
  done: number;
  total: number;
  current: string;
  freed: number;
}

export interface HistoryOp {
  id: string;
  started: Timestamp;
  finished: Timestamp | null;
  title: string;
  items: number;
  ok: number;
  failed: number;
  skipped: number;
  freedNow: number;
  recycled: number;
  disposals: Record<string, number>;
  restorable: number;
}

export interface HistoryItem {
  findingId: string;
  title: string;
  path: string;
  paths: string[];
  bytes: number;
  rule: string;
  category: CategoryId;
  disposal: string;
  result: ItemStatus;
  error: string | null;
  freed: number;
  recycled: number;
  ts: Timestamp;
  output: string | null;
  restored: boolean;
  purged: boolean;
}

export interface HistoryDetail {
  op: HistoryOp;
  items: HistoryItem[];
}

export interface RestoreReport {
  restored: number;
  collisions: string[];
  missing: string[];
  failed: [string, string][];
}

export interface PurgeReport {
  items: number;
  bytes: number;
  failed: number;
}

export type Theme = "system" | "light" | "dark";

export interface Thresholds {
  largeFileMb: number;
  largeFileDays: number;
  installerDays: number;
  partialDays: number;
  oldDownloadDays: number;
  idleDays: number;
  screenshotDays: number;
  duplicateMinMb: number;
  recentHours: number;
}

export interface Settings {
  theme: Theme;
  exclusions: string[];
  customRoots: ScanRoot[];
  projectRoots: string[];
  thresholds: Thresholds;
  cachesToRecycleBin: boolean;
  purgePareItemsAfterDays: number | null;
  hiddenCards: CategoryId[];
  checkForUpdates: boolean;
  onboardingDone: boolean;
  language: string;
}

export interface AppInfo {
  version: string;
  coreVersion: string;
  platform: string;
  logDir: string;
  settingsPath: string;
}

export type ProjectStatus =
  | "clean-pushed"
  | "active"
  | "unpushed"
  | "no-upstream"
  | "no-remote"
  | "detached"
  | "uncommitted"
  | "not-git";

export interface Artifact {
  name: string;
  path: string;
  bytes: number;
  ecosystem: string;
}

export interface Project {
  root: string;
  name: string;
  git: GitInfo | null;
  status: ProjectStatus;
  lastChange: Timestamp | null;
  idleDays: number | null;
  ecosystems: string[];
  rebuild: string[];
  artifacts: Artifact[];
  bytes: number;
  tier: Tier;
  findingId: string | null;
  parent: string | null;
}

export interface ProjectsView {
  checked: Timestamp | null;
  roots: string[];
  projects: Project[];
  findings: Finding[];
}
