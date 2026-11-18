# CLAUDE.md — Pare

Pare is a free, open-source Windows cleanup app that helps people **decide** what to delete. It works like Files by Google (suggestion cards, bulk review, calm UI) and adds a scanner that knows about developer tools and AI tools, plus git-aware project cleanup.

Read this file first, then the doc for the milestone you're working on.

| Doc | What it's for |
|---|---|
| `docs/MILESTONES.md` | **What to build next.** Ordered milestones, acceptance criteria, and a ready-to-use prompt for each one |
| `docs/PRD.md` | Product requirements, safety model, UX flows, copy rules |
| `docs/DESIGN.md` | Design tokens, components, screen specs, Figma node links |
| `docs/ARCHITECTURE.md` | Monorepo layout, Rust core, scanner pipeline, IPC, data model |
| `docs/RULES.md` + `rules/*.toml` | Cleanup rule catalog (paths, tiers, disposal) |
| `docs/RESEARCH.md` | Competitors, tech-stack decision, distribution, sources |
| `design/tokens.json` | Design tokens (source for CSS variables) |
| `design/brand/pare-mark.svg` | Approved logo mark (option E). Source for favicon and app icons |
| `design/previews/*.png` | PNG exports of every Figma frame. Use them when the Figma MCP isn't connected |

## Source of truth for design

Figma file: https://www.figma.com/design/f6552vSJ1ur7qFrxHArKZV/File-Manager

- Foundations (tokens, icons, components): `node-id=4-2`
- App screens: `node-id=4-3` (01 Home `11-2`, 02 Review `12-377`, 03 Projects `13-647`, 04 Scanning `14-900`, 05 Done `14-1051`)
- Landing page: `node-id=16-2`

If the Figma MCP server is connected, call `get_design_context` with the node ID before you build a screen. Treat the result as a reference and map it onto our tokens and components. Don't paste generated absolute-positioned code. If Figma and `docs/DESIGN.md` disagree, Figma wins for visuals and `docs/PRD.md` wins for behaviour and copy.

## Repo layout (target, created in M0)

```
apps/web/            Astro landing page (M1)
apps/desktop/        Tauri 2 app: src/ = React + TS UI, src-tauri/ = Rust shell + IPC commands
crates/pare-core/    Scanning, rules, safety, disposal. Pure Rust; holds most of the tests
crates/pare-cli/     (later) CLI on top of pare-core
packages/tokens/     tokens.json → CSS variables, shared by web and desktop
rules/               TOML cleanup rules (data, community-editable)
docs/                Specs (this handoff)
```

## Commands (after M0)

```bash
pnpm install
pnpm --filter web dev            # landing page at http://localhost:4321
pnpm --filter desktop tauri dev  # desktop app (Windows)
cargo test -p pare-core          # core tests (tempdir fixtures only)
cargo clippy --workspace -- -D warnings
pnpm lint && pnpm typecheck
```

Prerequisites on Windows: Rust stable (MSVC toolchain), Visual Studio Build Tools ("Desktop development with C++"), Node 20+, pnpm 9+, WebView2 (included with Windows 11).

## Safety invariants (non-negotiable)

This app deletes files. Every change must keep these true. Tests must cover each one.

1. **Nothing is deleted without an explicit user action.** A scan only reads. No auto-clean before M8, and even then only with opt-in rules.
2. **Disposal follows the rule.** `delete` is allowed only for rebuildable data (caches, build output, temp). Anything else goes to the **Recycle Bin**, or runs the tool's own command (for example `ollama rm` or `pnpm store prune`). If a rule says nothing, use the Recycle Bin.
3. **Protected never becomes selectable.** Protected covers: modified in the last 24 h, held open by a process, inside a repo with uncommitted changes, OS and system paths, and cloud-only placeholders.
4. **Never follow reparse points.** Junctions and symlinks are not traversed during scan or delete. Deleting a link removes the link, never its target.
5. **Never hydrate cloud files.** Skip files with `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`, `RECALL_ON_OPEN` or `OFFLINE` when hashing or reading content.
6. **Hard-excluded roots:** `C:\Windows` (only `Windows\Temp` via an admin rule), `Program Files*`, `ProgramData` (only `WER`), `WindowsApps`, `System Volume Information`, `$Recycle.Bin` internals, `pagefile.sys`, `hiberfil.sys`, `swapfile.sys`, and user config such as `~\.ssh`, `~\.gnupg`, `~\.claude\settings*.json`, `~\.codex\auth.json`, `~\.codex\config.toml`.
7. **Size claims use allocated size** (size on disk), counted once per file ID (hard links). Don't promise space you can't actually reclaim.
8. **Every disposal is logged** to `%LOCALAPPDATA%\Pare\logs\operations.jsonl` before it runs (path, bytes, rule id, disposal, timestamp, result).
9. **Tests never touch real user folders.** Use `tempfile` fixtures and a `FakeHome` that overrides `%USERPROFILE%`, `%APPDATA%`, `%LOCALAPPDATA%` and `%TEMP%`.
10. **No network calls** from the app except the opt-in update check. No telemetry.

## Working agreement

- Work one milestone at a time, on a branch `m<N>-<slug>`. Open a PR. Merge when the acceptance criteria in `docs/MILESTONES.md` pass, then tag `v0.<N>.0` when the milestone ships a release.
- Before writing code for a milestone, restate its acceptance criteria as a checklist in the PR description.
- Keep `pare-core` free of Tauri, UI and OS-dialog code. The Tauri layer is thin.
- Rules are data: add or adjust cleanup targets in `rules/*.toml`. Only add a Rust "detector" when TOML can't express the logic (orphan detection, git checks).
- UI: React function components, TypeScript strict, Tailwind with token CSS variables only. No raw hex in components. Icons come from `lucide-react` and match the names in `docs/DESIGN.md`.
- Copy: follow the voice in `docs/PRD.md` §7. Every suggestion shows *why*, *what you lose*, and *whether it comes back*.
- Commits: Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`).

## Ask the maintainer before

- Adding a dependency that has a native or C build, or that's larger than ~1 MB in the final binary
- Changing a safety invariant, a rule tier from `review` to `safe`, or a disposal from `recycle` to `delete`
- Anything that needs admin rights, a background service, a driver, or a scheduled task
- Publishing a release, changing the license, or registering store or signing accounts
