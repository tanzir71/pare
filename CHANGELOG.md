# Changelog

All notable changes to Pare. Versions follow the milestones in docs/MILESTONES.md.

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
