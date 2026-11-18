# Changelog

All notable changes to Pare. Versions follow the milestones in docs/MILESTONES.md.

## M8 — Monitor, schedules, guided compaction, localization (unreleased)

- Growth monitor: while Pare is open (or in the notification area), it checks the folders its rules know about a few times a day and tells you when one grows fast ("~\.codex grew 12 GB this week"). Off until you turn it on.
- Scheduled cleanups: pick Safe items to clean every week or month. Only built-in, verified Safe rules qualify; each run skips anything changed in the last 24 hours, open in an app or on your "Always keep" list, and shows up in History.
- Guided cleanup you can run step by step: Docker Desktop and WSL disks (prune, shut down WSL, compact the disk) and Windows Update leftovers (DISM component cleanup). Steps that need administrator rights only run when Pare is already elevated; otherwise Pare shows the command to copy.
- Spanish translation of the app, including rule descriptions; the language follows Windows or your choice in Settings.

## v1.0.0 — Trust and distribution (unreleased)

- First-run onboarding: what Safe, Review and Keep mean, Recycle Bin first, and that nothing leaves your PC.
- Opt-in update check (off until you turn it on), signed updates for GitHub builds.
- Code-signing, Microsoft Store and winget packaging prepared (see docs/DISTRIBUTION.md).
- Accessibility: keyboard selection in lists (arrows, Space, Shift-click, Ctrl+A), labels on every icon-only control, reduced motion.

## v0.5.0 — Whole-drive fast scan and Browse (unreleased)

- Fast mode: when Pare runs as administrator on NTFS, it reads the file table directly and catches up from the USN change journal on the next scan.
- Browse: a storage map of whole drives, file types with their largest files, and the same safe actions.

## v0.4.0 — Duplicates, screenshots, history and settings (unreleased)

- Duplicate finder (size, then the first and last 64 KB, then every byte) that always keeps one copy.
- Old screenshots, restore from History, "Empty Pare's items now", and automatic emptying of Pare's own Recycle Bin items after 30 days.
- Settings for scan folders, "Always keep", thresholds, disposal and appearance. Dark mode passes WCAG AA.

## v0.3.0 — Projects (unreleased)

- Git-aware cleanup of node_modules, target, .venv and friends, only when git ignores them and the project is committed, pushed and idle.

## v0.2.0 — AI-tool and developer caches (unreleased)

- Rules for Claude Code, Codex, Cursor, VS Code, Ollama, LM Studio, Hugging Face, npm, pnpm, Yarn, pip, uv, Cargo, Go, Gradle, Maven, NuGet, browsers and Windows temp files.
- Every item says why it's suggested, what you lose, and whether it comes back.

## v0.1.0 — Quick Clean (unreleased)

- Scans Desktop, Downloads, Documents, custom folders or the whole PC: large files, installers, extracted archives, partial and old downloads, empty folders and the Recycle Bin.
- Your files go to the Recycle Bin; rebuildable data is deleted directly; every action is logged on your PC.
