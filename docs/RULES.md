# Cleanup rules

Rules are data in `rules/*.toml`. They're embedded in the app at build time and community-editable through pull requests. A user rules folder (`%APPDATA%\Pare\rules\`) can add rules but **can't loosen** built-in tiers.

> Every starter rule ships with `verified = false`. A path only becomes `verified = true` after a maintainer measures it on a real Windows 11 machine with the tool installed (M3 task). Paths here come from tool docs, issue trackers and other cleaners' public catalogs (see RESEARCH.md sources), and some tools change their layout between versions.

## Schema

```toml
[tool]
id = "claude-code"               # unique, kebab-case
name = "Claude Code"             # shown as the group header
category = "ai-tools"            # ai-tools | dev-caches | browsers | system | downloads | projects
processes = ["claude.exe"]       # if running and holding files under these paths → Protected
docs = "https://…"               # optional, shown in details

[[rule]]
id = "claude-code.debug-logs"    # <tool>.<slug>
title = "Debug logs older than 7 days"
paths = ["~/.claude/debug/**"]   # globs; variables below
exclude = []                     # globs never matched by this rule
min_age_days = 7                 # by modified time of each file (dirs: newest descendant)
keep_newest = 0                  # keep N newest matches (e.g. versions)
tier = "safe"                    # safe | review  (protected is computed, never declared)
disposal = "delete"              # delete | recycle | command | guide
command = []                     # for disposal = "command", e.g. ["ollama", "rm", "{model}"]
guide = []                       # for disposal = "guide": steps shown to the user
comes_back = "automatically"     # automatically | on-demand | no
why = "Diagnostic logs Claude Code writes while it runs."
if_removed = "Old debug output is gone. Claude Code keeps working."
requires_closed = false          # if true, the item is Protected while any `processes` runs
admin = false                    # needs elevation (system paths)
detector = ""                    # optional built-in Rust detector id for logic TOML can't express
verified = false
sources = ["https://github.com/anthropics/claude-code/issues/11646"]

[[protect]]                      # paths this tool must never have touched, by any rule
paths = ["~/.claude/settings*.json", "~/.claude.json", "~/.claude/CLAUDE.md"]
```

**Path variables:** `~` = `%USERPROFILE%` · `%APPDATA%` · `%LOCALAPPDATA%` · `%TEMP%` · `%PROGRAMDATA%` · `${ENV:-default}` (for example `${OLLAMA_MODELS:-~/.ollama/models}`) · `{drive}` (each fixed drive root). Paths are case-insensitive and use `/` in TOML.

**Validation rules (enforced by tests):**
- `disposal = "delete"` requires `comes_back` = `automatically` or `on-demand`.
- `tier = "safe"` requires `why` and `if_removed`.
- No rule may match a path in any `[[protect]]` block or the global hard-excluded roots.
- Every rule has a fixture test in `crates/pare-core/tests/rules/`.

## Built-in detectors

| Detector id | Used by | Logic |
|---|---|---|
| `claude.orphaned_transcripts` | `claude-code.orphaned-transcripts` | For each `~/.claude/projects/<encoded>/`: read `cwd` from the first JSONL line. If the folder doesn't exist → Safe, recycle. Fallback: decode the folder name (`C--Users-ana-code-x` → `C:\Users\ana\code\x`); if the decode is ambiguous, set the tier to Review |
| `codex.superseded_releases` | `codex.old-releases` | Find the release the launcher points to and keep it plus the newest; the rest are Safe |
| `active_session_guard` | all ai-tools | Any match modified < 24 h, or held by a listed process → Protected ("In use") |
| `projects.artifacts` | `projects.toml` | Marker → artifact dirs; only git-ignored dirs; tier from git state (see MILESTONES M4) |
| `downloads.extracted_archive` | M2 quick | `X.zip` next to folder `X\` |
| `duplicates` | M5 | see ARCHITECTURE §4.4 |

## Starter catalog (files in `rules/`)

| File | Covers |
|---|---|
| `claude-code.toml` | Claude Code CLI data in `~/.claude` (+ old versions) |
| `claude-desktop.toml` | Claude desktop app caches and VM bundle (Review / guide) |
| `codex.toml` | Codex CLI sessions, archives, old releases |
| `editors.toml` | VS Code and Cursor Electron caches, logs, `state.vscdb.backup` |
| `local-models.toml` | Ollama, LM Studio, Hugging Face hub cache |
| `node.toml` | npm, pnpm, Yarn |
| `python.toml` | pip, uv |
| `rust-go-jvm-dotnet.toml` | Cargo, Go, Gradle, Maven, NuGet |
| `windows.toml` | User temp, crash dumps, thumbnail cache, WER, Recycle Bin, Docker/WSL guides |
| `browsers.toml` | Chrome and Edge caches (never cookies, history or passwords) |
| `projects.toml` | Marker → artifact mapping for M4 |

## Engine notes (M3)

- **Matching** is segment-wise: `~/.claude/projects/*/*.jsonl` lists only `projects/`, then each project folder. Reparse points are never traversed.
- **Filters per file:** `exclude`, every `[[protect]]` block, the hard-excluded roots, `min_age_days`, cloud placeholders, and the 24-hour recency window. What's left collapses into the fewest paths (a folder stands in for its contents only when everything inside qualifies and the folder itself wasn't changed in the last 24 hours).
- **Recently used files** are left out of a rule's finding and reported as a separate Protected row ("used in the last 24 hours"), so one active session never blocks cleaning the rest.
- **`keep_newest`** applies per path pattern to matched items (versions), never to `/**` contents.
- **Unverified rules never claim Safe:** a rule with `verified = false` is shown as Review until a maintainer checks it.
- **`prune` commands** (`pnpm store prune`, `uv cache prune`) report 0 bytes up front because the tool decides what's unused; the Done screen shows what was actually freed (measured before and after).
- **`active_session_guard`** is the safety pass itself: the recency window, the Restart Manager check, and `requires_closed` process checks apply to every rule.
- **User rules** (`%APPDATA%\Pare\rules\*.toml`) can add rules but can't shadow a built-in id, can't use `disposal = "command"`, and their `delete` becomes `recycle`.

## Verification (Windows 11, November 2026)

Checked read-only with `pare rules check` (lists and measures matches; never deletes).

| Status | Rules |
|---|---|
| `verified = true` (path matched real data) | `browsers.cache` (Edge) · `claude-code.debug-logs` · `claude-code.orphaned-transcripts` · `claude-code.old-transcripts` · `claude-code.file-history` · `claude-code.caches` · `claude-desktop.app-data-guide` · `codex.archived-sessions` · `codex.old-sessions` · `editors.electron-caches` · `cursor.state-backup` · `cursor.state-db-guide` · `lmstudio.models` · `huggingface.hub` · `npm.cache` · `pnpm.store` · `pip.cache` · `uv.cache` · `cargo.registry` · `gradle.caches` · `maven.repository` · `nuget.http-cache` · `nuget.packages` · `windows.user-temp` · `windows.crash-dumps` · `windows.thumbnail-cache` · `windows.wer` · `docker.vhdx-guide` |
| Not verifiable on the test machine | `claude-code.old-versions` (Claude Code was installed with npm, not the native installer) · `claude-desktop.electron-cache`, `claude-desktop.vm-bundle-archive`, `claude-desktop.vm-bundle` (the installed version uses `%LOCALAPPDATA%\Claude-Data` instead; see below) · `codex.old-releases` (no standalone Codex CLI; the Codex desktop app ships as MSIX packages) · `editors.cpptools-ipch`, `ollama.models`, `yarn.cache`, `go.build-cache` (tool not installed) |

**Claude desktop, new layout.** Current Claude desktop builds keep caches, the VM bundle and Cowork sessions in `%LOCALAPPDATA%\Claude-Data\claude_<id>\`, with a README that says nothing inside may be deleted by hand and points to *Settings → This computer → System → Storage*. Pare therefore treats that folder as never-touch (`[[protect]]`) and only shows a guide (`claude-desktop.app-data-guide`).
