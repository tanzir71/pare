# Architecture

## 1. Stack decision (summary; reasoning in RESEARCH §4)

| Layer | Choice | Why |
|---|---|---|
| Desktop shell | **Tauri 2** (Rust + WebView2) | Small installer and low idle RAM (shipping a ~150 MB Electron app to *free space* would be ironic); Rust gives us direct, safe access to Win32 (MFT, Restart Manager, `IFileOperation`) |
| UI | React 19 + TypeScript (strict) + Vite, Tailwind v4 with token CSS variables, Radix primitives (Checkbox, Dialog, DropdownMenu, Tooltip, ScrollArea), TanStack Virtual for long lists, Zustand for state, lucide-react | The same tokens and components can render the landing page's product mock |
| Core | Rust crate `pare-core`, no Tauri dependency | Testable headless, reusable by the CLI and MCP server later |
| Landing | Astro 5 + Tailwind v4 on GitHub Pages | Static, fast, zero JS by default |
| Storage | SQLite (`rusqlite`, bundled) for the index (M6) and history; JSONL op log | Simple, local, inspectable |

### Key crates

`jwalk` (parallel walk) · `windows` (Win32: `GetFileInformationByHandleEx`, Restart Manager, `SHQueryRecycleBinW`/`SHEmptyRecycleBinW`, known folders, `IFileOperation`) · `trash` (Recycle Bin move, list, restore and purge on Windows) · `git2` with `vendored-libgit2` (status, ahead/behind, last commit, ignore checks) · `globset` · `toml` + `serde` · `xxhash-rust` (xxh3) + `blake3` · `rayon` · `tokio` (Tauri async) · `rusqlite` (bundled) · `sysinfo` (disks, processes) · `tracing` · `thiserror` · `time` · M6: `ntfs-reader` or `usn-journal-rs` (MFT enumeration + USN journal).

Check every crate's current version and maintenance status when adding it, and pin it in `Cargo.lock`.

## 2. Repo layout

```
pare/
├─ apps/
│  ├─ web/                       # Astro landing
│  └─ desktop/
│     ├─ src/                    # React UI
│     │  ├─ app/                 # routes: clean, review/:category, projects, history, settings, browse
│     │  ├─ components/          # Button, Badge, Checkbox, Chip, NavItem, SuggestionCard, FileRow, ProjectRow, DetailsPanel, BulkBar, TitleBar, Sidebar
│     │  ├─ lib/ipc.ts           # typed wrappers over Tauri invoke/listen (generated types via ts-rs or specta)
│     │  └─ state/               # zustand stores: scan, selection, settings
│     └─ src-tauri/
│        ├─ src/commands.rs      # thin IPC layer → pare-core
│        ├─ src/events.rs
│        └─ tauri.conf.json      # decorations:false, minWidth 960, minHeight 640, CSP strict
├─ crates/
│  ├─ pare-core/
│  │  ├─ src/scan/               # walker.rs, mft.rs (M6), usn.rs (M6), entry.rs, index.rs
│  │  ├─ src/rules/              # schema.rs, loader.rs, matcher.rs, vars.rs
│  │  ├─ src/detectors/          # quick.rs (M2), claude.rs, codex.rs, projects.rs, duplicates.rs, screenshots.rs
│  │  ├─ src/safety/             # tier.rs, in_use.rs (Restart Manager), protected_paths.rs, cloud.rs
│  │  ├─ src/dispose/            # executor.rs, recycle.rs, delete.rs, command.rs, oplog.rs
│  │  ├─ src/git.rs
│  │  └─ tests/                  # fixtures/home/**, integration tests per invariant
│  └─ pare-cli/                  # M8
├─ packages/tokens/
├─ rules/*.toml
└─ docs/
```

## 3. Data model (pare-core)

```rust
pub struct Entry {               // one file or dir seen by the scanner
    pub id: FileId,              // NTFS file reference (volume serial + 64/128-bit id)
    pub parent: Option<FileId>,
    pub path: PathBuf,           // materialized lazily for MFT mode
    pub is_dir: bool,
    pub logical: u64,            // file size
    pub allocated: u64,          // size on disk (compression/sparse aware)
    pub links: u32,              // hard link count
    pub created: SystemTime, pub modified: SystemTime, pub accessed: Option<SystemTime>,
    pub attrs: u32,              // FILE_ATTRIBUTE_*; used for cloud/reparse checks
}

pub enum Tier { Safe, Review, Protected { reason: ProtectReason } }
pub enum ComesBack { Automatically, OnDemand, No }
pub enum Disposal { Delete, Recycle, Command { program: String, args: Vec<String> }, Guide { steps: Vec<String> } }

pub struct Finding {             // one selectable item (file, folder, or logical group)
    pub id: FindingId,
    pub rule_id: String,         // e.g. "claude-code.debug-logs"
    pub category: CategoryId,    // e.g. "ai-tools"
    pub source: String,          // group header: "Claude Code", "D:\code\weather-dashboard"
    pub title: String,
    pub paths: Vec<PathBuf>,
    pub bytes: u64,              // allocated, deduped by FileId, counted only if links==1
    pub items: u64,
    pub tier: Tier,
    pub disposal: Disposal,
    pub comes_back: ComesBack,
    pub why: String, pub if_removed: String,
    pub dates: Dates,            // created_min, modified_max, accessed_max
    pub git: Option<GitInfo>,    // repo, branch, last_commit, dirty, ahead, has_remote
}

pub struct Category { pub id, title, description, icon, hue, findings: Vec<FindingId>, safe_bytes: u64, review_bytes: u64, items: u64 }
```

## 4. Pipelines

### 4.1 Scan
```
scope → enumerate (walker | MFT) ─┬─► Entry stream ─► detectors (rules engine + built-ins) ─► Findings
                                  └─► size aggregation (per dir, deduped by FileId)
Findings ─► safety pass (protected paths, recency, cloud attrs, process check) ─► Categories ─► UI events
```
- The walker never follows reparse points (check `FILE_ATTRIBUTE_REPARSE_POINT` before descending).
- Rules are matched against **directory candidates first** (for example `~\.claude\debug`), then their subtree is sized. That keeps rule matching O(rules × candidate dirs), not O(rules × files).
- Progress events are throttled to 4 per second: `{phase, files, bytes_seen, found_bytes, current_root}`.

### 4.2 Clean
```
selection ─► re-validate (exists? modified since scan? now in use? → re-tier) ─► group by disposal
         ─► write op-log "begin" ─► execute (recycle batch via IFileOperation; delete with RemoveDirectory on reparse points; commands with timeout)
         ─► per-item result ─► op-log "end" ─► CleanReport {freed_now, recycled, failed[]}
```
- Re-validation is mandatory. Anything modified after the scan becomes Protected and is skipped with the reason "Changed since scan".
- Deletion of directories is bottom-up. On `ERROR_SHARING_VIOLATION`, use Restart Manager to name the holding process in the error.

### 4.3 Projects (M4)
Discover `.git` dirs under the roots (also `.git` files for worktrees and submodules) → `git2::Repository::open` → statuses (exclude ignored) → ahead/behind against upstream → last commit time → artifact candidates from `rules/projects.toml` markers → keep only candidates where `repo.status_should_ignore(path)` is true → tier.

### 4.4 Duplicates (M5)
Size buckets (≥ 1 MB, skip cloud and system) → xxh3 of the first and last 64 KB → BLAKE3 full hash, streamed with a 1 MB buffer and limited parallelism (spinning disks: 1) → groups → keep-suggestion.

## 5. IPC (Tauri commands and events)

| Command | Returns | Notes |
|---|---|---|
| `list_drives()` | `Drive[]` | label, letter, fs, total, free, is_ntfs, is_fixed |
| `scan_start(scope: Scope, opts)` | `ScanId` | Emits `scan://progress`, `scan://category`, `scan://done`, `scan://error` |
| `scan_cancel(id)` | `()` | |
| `category_get(id)` | `CategoryDetail` | findings grouped by source, paginated |
| `finding_get(id)` | `FindingDetail` | facts table, git, sample paths |
| `clean(selection: FindingId[], opts)` | `OpId` | Emits `clean://progress`, `clean://done` (CleanReport) |
| `history_list()` / `history_get(op)` | | |
| `history_restore(op, items?)` | `RestoreReport` | recycled items only |
| `recycle_purge_pare_items(older_than_days?)` | `u64` freed | only items Pare moved |
| `settings_get()` / `settings_set(patch)` | | `%APPDATA%\Pare\settings.json` |
| `exclusion_add(path)` | | "Always keep" |
| `open_in_explorer(path)` | | `explorer.exe /select,` |
| `relaunch_elevated()` | | M6 fast mode |

Types are generated for TS with `specta` (tauri-specta) so the UI and core can't drift.

Tauri security: allowlist only these commands, a strict CSP, no remote URLs in the webview, and no `shell` plugin. Tool commands are executed by Rust from an allow-list defined in rules.

## 6. Files on disk

| Path | Content |
|---|---|
| `%APPDATA%\Pare\settings.json` | settings, exclusions, hidden cards |
| `%LOCALAPPDATA%\Pare\logs\operations.jsonl` | append-only op log |
| `%LOCALAPPDATA%\Pare\logs\app.log` | tracing logs (rotated, 5 × 2 MB) |
| `%LOCALAPPDATA%\Pare\history.db` | operations and items (SQLite) |
| `%LOCALAPPDATA%\Pare\index\<volume-serial>.db` | M6 file index + last USN |

Pare must exclude its own folders from scans.

## 7. Build and release

- CI: `ci.yml` (lint, test) and `release.yml` (on `v*` tags: `tauri-apps/tauri-action` builds the NSIS installer, plus MSI optionally, and uploads to a draft GitHub Release with checksums).
- Signing (M7): SignPath Foundation or Azure Artifact Signing step after build. Updater signing key kept in GitHub secrets (`TAURI_SIGNING_PRIVATE_KEY`).
- Store (M7): MSIX via `tauri bundle` / `msixbundle` tooling, submitted to Partner Center.
- Landing: `pages.yml` with `withastro/action`.

## 8. Testing strategy

- **Unit**: rule parsing, path variables, tiering, size dedupe, git classification.
- **Fixture integration**: `tests/fixtures/home` with a `FakeHome` that sets env overrides; generated trees for junctions, hard links, read-only files, long paths (> 260 chars, `\\?\` prefix), and Unicode names.
- **Invariant tests**: one per safety invariant in CLAUDE.md. CI fails if any is missing (keep a checklist test that asserts the list).
- **UI**: Vitest + Testing Library for selection logic; Playwright smoke against `tauri dev` (later).
- **Manual QA script** per release: a fresh Windows 11 VM with Claude Code, Codex, VS Code, Node and Python installed, run the scan, check categories against expectations, clean, restore.
