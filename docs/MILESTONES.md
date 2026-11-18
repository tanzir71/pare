# Milestones

The landing page ships first, to a public GitHub repo. After that the app ships in increments, easiest and most useful features first. Each milestone ends in something a user can see: a deployed page or a GitHub Release.

| # | Milestone | Ships | Rough size | Why this order |
|---|---|---|---|---|
| M0 | Repo bootstrap | Public repo, CI, tokens | ½ day | Everything else needs it |
| **M1** | **Landing page** | **GitHub Pages site** | 1–2 days | Public presence first; collects stars and watchers before the app exists |
| M2 | Quick Clean MVP (Desktop and Downloads) | `v0.1.0` | 4–6 days | Only `std::fs` walking and the Recycle Bin. No admin, no git, no hashing |
| M3 | AI-tool and developer caches | `v0.2.0` | 4–6 days | Rule data plus a small engine; the main differentiator |
| M4 | Projects (git-aware) | `v0.3.0` | 3–5 days | Needs `git2`; builds on M3's tiers |
| M5 | Duplicates, screenshots, History, settings | `v0.4.0` | 4–6 days | Hashing, restore and preferences |
| M6 | Whole-drive fast scan and Browse | `v0.5.0` | 5–8 days | MFT/USN plus elevation. Hardest Windows work, so it comes last |
| M7 | 1.0: signing, updates, store, polish | `v1.0.0` | 4–6 days | Trust and distribution |
| M8+ | Monitor, schedules, CLI and MCP, VHDX compact | `v1.x` | — | Nice-to-haves |

Sizes assume one developer working with Claude Code. Treat them as relative, not as promises.

---

## M0 — Repo bootstrap

**Goal:** a public monorepo that builds in CI.

**Tasks**
- `git init`. Add `LICENSE` (MIT), `README.md` (from this handoff), `CLAUDE.md`, `docs/`, `rules/`, `design/tokens.json`.
- pnpm workspace (`apps/*`, `packages/*`) and Cargo workspace (`crates/*`, `apps/desktop/src-tauri`).
- `packages/tokens`: a build script that turns `design/tokens.json` into `tokens.css` (`:root` for light, `[data-theme=dark]` for dark) and a Tailwind v4 `@theme` file.
- `.github/workflows/ci.yml`: on push and PR, `pnpm lint`, `pnpm typecheck`, `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`. Run on `windows-latest` and `ubuntu-latest`; Windows-only code sits behind `cfg(windows)`.
- Issue templates: bug report, and a "Cleanup rule request" template (tool, path, what it contains, safe to delete?).
- Create the GitHub repo **public** and push: `gh repo create pare --public --source . --push`.

**Acceptance**
- [ ] Repo is public and CI is green on `main`
- [ ] `packages/tokens` outputs CSS variables that match `docs/DESIGN.md` §1

**Prompt for Claude Code**
> Read CLAUDE.md and docs/MILESTONES.md (M0). Bootstrap the monorepo exactly as described: pnpm + Cargo workspaces, MIT license, tokens package generating CSS variables from design/tokens.json, CI on Windows and Ubuntu, issue templates. Then create a public GitHub repo named `pare` with gh and push. Stop and show me the repo URL.

---

## M1 — Landing page (ship first)

**Goal:** the Figma landing page, live on GitHub Pages.

**Design:** Figma `node-id=16-2`. Spec: `docs/DESIGN.md` §6.

**Stack:** Astro 5 + Tailwind v4 (token CSS variables), in `apps/web`. Zero client JS except an optional FAQ disclosure (`<details>`) and the mobile nav toggle. Self-host fonts with `@fontsource-variable/geist` and `@fontsource-variable/geist-mono` (both OFL).

**Tasks**
- Sections in order: nav, hero (announcement pill, H1, sub, two CTAs, meta line, product mock), proof strip, features (6 cards), how it works (3 steps), tools chip cloud, safety band (dark), FAQ (6), final CTA, footer.
- Hero product mock: build it as **real HTML/CSS** using the same components as the app: a complete mini window with title bar, sidebar, storage summary, scope chips and one row of three suggestion cards. No fade, no crop, no screenshot image. That keeps it sharp, themeable and accurate. Swap in a real screenshot later only if you want to.
- CTAs: "Download for Windows" links to `https://github.com/<owner>/pare/releases/latest`. Until v0.1 exists, label it "Get notified" and link to the repo with `?tab=readme-ov-file#install` (or the Releases "Watch" docs). "Star on GitHub" links to the repo.
- Responsive breakpoints at 1440, 1024, 768 and 390 (see DESIGN §6.3).
- SEO: title, description, Open Graph and Twitter image (1200×630 built from the hero; it can be an Astro endpoint using `@vercel/og`-style rendering or a static PNG exported from Figma), `sitemap.xml`, `robots.txt`, favicon from `design/brand/pare-mark.svg`.
- Accessibility: one `h1`, landmarks, visible focus rings, WCAG AA contrast (tokens already pass), `prefers-reduced-motion`.
- Deploy: `.github/workflows/pages.yml` with `withastro/action` to GitHub Pages. Set `site` and `base` for `https://<owner>.github.io/pare/`. A custom domain comes later.

**Acceptance**
- [ ] Site is live at the Pages URL and matches Figma at 1440 within normal tolerance (fonts, spacing, colors from tokens)
- [ ] Lighthouse mobile: Performance ≥ 95, Accessibility 100, Best Practices ≥ 95, SEO 100
- [ ] No layout overflow at 390 px; nav collapses into a menu
- [ ] All copy matches the Figma frame and PRD §7 voice; the footer non-affiliation line is present

**Prompt for Claude Code**
> Implement M1 from docs/MILESTONES.md. Use the Figma MCP: get_design_context for node 16-2 (file f6552vSJ1ur7qFrxHArKZV) and build it in apps/web with Astro + Tailwind, using our token CSS variables (no raw hex). Build the hero product mock as HTML components, not an image. Add the Pages deploy workflow, push, and give me the live URL plus a Lighthouse report.

---

## M2 — Quick Clean MVP → `v0.1.0`

**Goal:** a working Windows app that scans Desktop, Downloads and Documents (plus custom folders), shows Files-by-Google-style cards, and frees space safely.

**Design:** Home `11-2`, Review `12-377`, Scanning `14-900`, Done `14-1051`.

**In scope**
- Tauri 2 shell: custom title bar (`decorations: false`, drag region, caption buttons), sidebar (Clean active; Browse, Projects and History show "Coming soon" except History), light and dark following the system theme.
- `pare-core` scanner v1: parallel walk with `jwalk`. Records path, allocated size, created, modified and accessed times, attributes, and file ID plus link count (via `GetFileInformationByHandleEx`). No reparse traversal. Skips cloud placeholders.
- Detectors (no TOML engine yet; hard-code them in `pare-core::detectors::quick`):
  - **Large files**: ≥ 500 MB, not modified for 90+ days → Review
  - **Downloads → installers**: `.exe .msi .msix .appx` older than 7 days → Review
  - **Downloads → archives already extracted**: `.zip .7z .rar` with a sibling folder of the same stem → Review
  - **Partial downloads**: `.crdownload .part .partial` older than 1 day → Safe (delete)
  - **Old downloads**: anything in Downloads not modified for 180+ days → Review
  - **Empty folders** (recursive, excluding protected roots) → Safe (delete)
  - **Recycle Bin**: size via `SHQueryRecycleBinW` per drive; empty via `SHEmptyRecycleBinW` → Safe
- UI: Home (storage summary with segmented bar, scope chips, cards grid, kept-safe note), category Review (list, group headers, tri-state select, details panel with dates and "why / if you remove it / comes back?", floating bulk bar), Scanning (progress ring, live counters, detector checklist, cancel), Done (freed total, before/after bars, breakdown).
- Disposal executor: Recycle Bin via the `trash` crate (`IFileOperation` with `FOF_ALLOWUNDO`), direct delete for Safe items, per-item results, partial-failure handling ("12 items couldn't be removed because they're in use").
- Operation log (JSONL) and a minimal **History** list (read-only, newest first).
- Packaging: NSIS installer from `tauri-action` on tag `v0.1.0`, GitHub Release (unsigned; the release notes say so and include SmartScreen guidance). Update the landing CTA to point to Releases.

**Out of scope:** rules engine, git, duplicates, MFT, settings beyond theme.

**Acceptance**
- [ ] Scanning a 200k-file folder fixture finishes, and the UI stays responsive (progress events at least every 250 ms)
- [ ] Every card total equals the sum of its items (allocated bytes, hard links counted once)
- [ ] Protected items (modified < 24 h, open in a process) can't be selected; there's a test for each
- [ ] Moving 1,000 files to the Recycle Bin succeeds and appears in History; a junction inside a selected folder is removed as a link and its target survives (test)
- [ ] App starts in under 1 s on a mid-range laptop; installer is under 15 MB
- [ ] `v0.1.0` release is published and the landing page links to it

**Prompt for Claude Code**
> Implement M2 from docs/MILESTONES.md. Start with crates/pare-core (scanner, quick detectors, disposal executor, op log) with tempdir tests for every safety invariant in CLAUDE.md. Then build the Tauri app UI from Figma nodes 11-2, 12-377, 14-900, 14-1051 using our components. Wire IPC as in docs/ARCHITECTURE.md §5. Finish with the release workflow and tag v0.1.0 after I approve.

---

## M3 — AI-tool and developer caches → `v0.2.0`

**Goal:** the differentiator. Pare knows Claude Code, Codex, Cursor, VS Code, Ollama and the main package managers, and explains each item.

**In scope**
- **Rule engine** (`pare-core::rules`): load `rules/*.toml` (embedded at compile time, plus an optional user rules folder), expand path variables and env overrides, glob with `globset`, apply `min_age_days`, `keep_newest`, `exclude`, process checks and tiering. Schema in `docs/RULES.md`.
- **Built-in detectors** that TOML can't express:
  - `claude.orphaned_transcripts`: for each `~\.claude\projects\<encoded>\`, read `cwd` from the first JSONL record (fall back to a lossy decode of the folder name). If that folder no longer exists, mark it Safe (orphaned), disposal Recycle Bin.
  - `codex.superseded_releases`: keep the release the launcher resolves to; everything older is Safe.
  - `active_session_guard`: any AI-session file modified in the last 24 h, or a tool process (`claude.exe`, `codex.exe`, `Cursor.exe`, `Code.exe`) holding it open, is Protected.
- **In-use detection** with the Windows Restart Manager (`RmStartSession`, `RmRegisterResources`, `RmGetList`) for selected items at clean time, plus a cheap process-name check at scan time.
- **Tool commands** as a disposal: `ollama rm <model>`, `pnpm store prune`, `npm cache clean --force`, `uv cache prune`, `go clean -cache`. Run with a timeout, capture output to the op log, and show the result.
- **Info-only cards** ("Guide" disposal) for things Pare must not delete itself: Docker/WSL VHDX compaction, Cursor `state.vscdb` (use "Delete Old Chats" in Cursor), old Claude desktop MSIX versions.
- **"Comes back?"** label on every item: *Yes, automatically* / *Re-downloads when needed* / *No*.
- New cards: AI tool caches, Developer caches, Browser caches (closed browsers only), Junk & temp files (`%TEMP%` > 24 h, `%LOCALAPPDATA%\CrashDumps`, thumbnail cache).

**Acceptance**
- [ ] Every rule in `rules/` has `verified = true`, meaning a maintainer checked the path on a real Windows 11 machine with that tool installed, and has a fixture test
- [ ] Running Claude Code or Codex sessions are never selectable (integration test with a fake process holding a handle)
- [ ] Config files in the never-touch list are never matched, even by broad globs (property test)
- [ ] Tool-command disposals show their exit status; failures don't block other items
- [ ] `v0.2.0` released; landing announcement pill updated

**Prompt for Claude Code**
> Implement M3. Build the TOML rule engine and the three built-in detectors in pare-core, with fixtures under crates/pare-core/tests/fixtures/home/. Then go through rules/*.toml one by one: for each rule, check the path on this machine (read-only: list and measure, never delete). Mark verified=true only when it matches, and report any rule that doesn't. Add Restart Manager in-use checks at clean time. Update the UI for "Comes back?" and Guide cards.

---

## M4 — Projects (git-aware) → `v0.3.0`

**Goal:** clean `node_modules`, `target`, `.venv` and friends only in projects that are committed, pushed and idle.

**Design:** Projects `13-647`.

**In scope**
- Project discovery under the user's chosen roots (default: `~\source`, `~\code`, `~\Projects`, `~\Documents\GitHub`, `D:\code` if present, plus "Add folder"). Stop descending once a repo root is found, but still detect nested repos and submodules.
- Git status with `git2` (vendored libgit2): dirty (tracked changes or untracked non-ignored files) → **Protected**; ahead of upstream → **Review** "Unpushed commits"; no upstream or remote → **Review** "No remote"; stashes → note; last commit date; branch.
- Artifact detection from `rules/projects.toml` (marker file → artifact dirs). An artifact dir is suggested only if git reports it as **ignored** (`git check-ignore`, or `status_should_ignore`). For non-git projects, use the marker rules but set the tier to Review.
- Idle = no commits and no source-file changes for N days (default 90; setting).
- Projects screen: filters (All, Idle 90+, Unpushed, Uncommitted, No remote), table, bulk summary card, rebuild hint per ecosystem.
- Clean-tab card "Old project build files" sums the Safe artifacts.

**Acceptance**
- [ ] Fixture repos cover: clean+pushed, dirty, untracked file, ahead, no remote, detached HEAD, submodule, worktree, artifact dir *not* ignored (must not be suggested)
- [ ] 50 repos scan in under 3 s (warm cache)
- [ ] `v0.3.0` released

---

## M5 — Duplicates, screenshots, History, settings → `v0.4.0`

**In scope**
- Duplicates: group by allocated size → xxh3 of the first and last 64 KB → full BLAKE3. Skip files under 1 MB by default, cloud placeholders and hard links to the same file ID. Keep suggestions: keep the oldest copy, or the copy in the preferred folder (Documents > Desktop > Downloads). Duplicates always go to the Recycle Bin.
- Old screenshots: `Pictures\Screenshots` older than 90 days → Review.
- History: restore Pare-recycled items (`trash::os_limited::restore_all`), "Empty Pare's items now", and automatic purge of Pare-recycled items after 30 days (setting; only items Pare moved).
- Settings: scan roots, exclusions, thresholds (large-file size, idle days, old-download days), disposal preference ("send caches to Recycle Bin too"), theme, language placeholder.
- Dark mode QA on all screens.

**Acceptance**
- [ ] Duplicate finder never suggests the last remaining copy; test with 3-way duplicates and hard links
- [ ] Restore puts a file back at its original path; collisions are reported, not overwritten

---

## M6 — Whole-drive fast scan and Browse → `v0.5.0`

**In scope**
- **Fast mode**: when the app runs elevated, enumerate the MFT (`ntfs-reader`, or `usn-journal-rs` / `FSCTL_ENUM_USN_DATA`) to build the full index in seconds. If the user declines, fall back to the walker. First version: a "Restart as administrator for fast mode" button. Later: a small elevated helper process that streams records over a named pipe.
- Persist the index in SQLite (`%LOCALAPPDATA%\Pare\index\<volume-serial>.db`). Next time, read the **USN journal** from the saved USN to apply changes incrementally, so a rescan takes about a second.
- Drive picker (fixed NTFS drives; FAT/exFAT and network drives use the walker).
- Browse tab: storage map (squarified treemap, Canvas) plus type categories (Images, Videos, Audio, Documents, Archives, Installers, Code, AI models), with drill-down and the same actions.
- Benchmark harness (`cargo bench` + a synthetic 1M-file volume in CI is impractical; document a manual benchmark protocol and record results in `docs/BENCHMARKS.md`).

**Acceptance**
- [ ] On a real 1M-file NTFS SSD volume, fast mode builds the index in under 15 s; record the actual number
- [ ] Incremental rescan after 1,000 file changes takes under 2 s
- [ ] Hard links and compressed files report correct allocated size

---

## M7 — 1.0: trust and distribution → `v1.0.0`

- **Code signing**: apply to the SignPath Foundation (free for OSI-licensed OSS) and sign NSIS/MSI in CI. Alternative: Azure Artifact Signing (~$9.99/month; individual developers are eligible only in the USA and Canada). Either way, SmartScreen reputation still has to build up over time.
- **Microsoft Store**: an MSIX build submitted through the free individual developer account. The Store signs and updates it, which avoids SmartScreen friction for Store installs.
- **Auto-update** for the GitHub build via `tauri-plugin-updater` (signed `latest.json` on Releases). Off by default until the user opts in on first run.
- **winget** manifest PR.
- First-run onboarding: three screens explaining tiers, Recycle Bin vs delete, and that nothing leaves the PC.
- Accessibility pass: full keyboard operation (lists with arrow keys, Space toggles, Shift-click range, Ctrl+A), screen-reader names on every control, focus order, reduced motion.
- Landing: real download size, changelog page, screenshots from the shipped app.

---

## M8+ — Later

**Status (November 2026):** all items below are built. The monitor and schedules run only while Pare is open (no service, scheduled task or startup entry). Admin steps (VHDX compaction, DISM) run only when Pare is already elevated; otherwise Pare shows the command to run in an administrator terminal. Localization ships English and Spanish.

- Tray monitor and growth alerts ("`~\.codex` grew 12 GB this week")
- Scheduled cleanups for Safe-only rules (opt-in, per rule)
- `pare` CLI (`pare scan --json`, `pare clean --rule claude-code.debug-logs --dry-run`) and an **MCP server**, so coding agents can ask "what's eating my disk?" and propose (never execute) cleanups
- Docker Desktop / WSL guided compaction (`docker system prune`, then `wsl --shutdown` + `Optimize-VHD` or a `diskpart compact vdisk` fallback)
- Stale git worktree cleanup (merged branch, clean tree)
- "Installer already installed" matching against the Uninstall registry keys
- Windows Update cleanup via DISM `/StartComponentCleanup` (admin)
- Localization
