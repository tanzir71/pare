# Pare — Product requirements

## 1. Problem

Windows drives fill up with things people can't judge: AI-assistant session logs, package-manager caches, `node_modules` in projects nobody has opened in a year, VM disk images, installers that already ran, three copies of the same video. The existing tools fall into two camps:

- **Analyzers** (WizTree, WinDirStat, TreeSize) are fast and show *where* space goes, but they leave the *should I delete this?* decision entirely to the user.
- **Cleaners** (Storage Sense, PC Manager, CCleaner, BleachBit) clean generic temp files, but don't explain much and don't understand developer or AI tools, let alone git.

Meanwhile AI coding tools have made the problem worse and stranger. Public issue trackers report `~/.claude` growing to multiple GB with no cleanup, Codex sub-agent sessions consuming hundreds of GB, and desktop AI apps leaving 10 GB+ VM bundles in AppData (sources in RESEARCH.md).

## 2. Product

**Pare is a calm, explain-everything cleanup app for Windows.** It scans a whole drive or just the folders you choose, groups what it finds into Files-by-Google-style suggestion cards, labels every item **Safe**, **Review** or **Protected**, shows the dates and context you need to decide, and frees space in bulk with safe disposal defaults.

Tagline: *Make room on your PC. Keep what matters.*

## 3. Users

1. **Everyday Windows user with a full C: drive.** Wants one button and confidence nothing important disappears. Mostly uses Home → "Free up X GB" and Downloads/Large files/Duplicates.
2. **Developer / AI power user.** Has 10–100 repos, several package managers, and Claude Code, Codex, Cursor or Ollama. Wants the AI-tool and project cleanup, git awareness and exact paths. Will also read the rules on GitHub.
3. **IT / helpdesk (later).** Wants a portable, auditable tool with a log and a CLI. Served by the op log in M2 and the CLI in M8.

## 4. Principles

1. **Explain before delete.** Every item answers *why it's here*, *what you lose*, and *does it come back*.
2. **Safe by default.** Protected items can't be selected. Your own files go to the Recycle Bin. Only rebuildable data is deleted directly.
3. **Honest numbers.** Allocated size, hard links counted once, and "freed now" kept separate from "in Recycle Bin".
4. **Fast enough to be casual.** Quick Clean in seconds; whole drives in seconds with fast mode (M6).
5. **Local and auditable.** No account, no telemetry, rules are open data, every action is logged.
6. **Calm UI.** One primary action per screen, no scare tactics, no "PC health score", no fake urgency.

## 5. Safety model

### 5.1 Tiers

| Tier | Meaning | Selectable | Default selection | Examples |
|---|---|---|---|---|
| **Safe** | Rebuilds itself, re-downloads on demand, is already deleted, or is orphaned and of no further use | Yes | **Selected** in "Free up" | npm cache, `target/` in a pushed idle repo, `%TEMP%` > 24 h, Recycle Bin, transcripts for deleted projects |
| **Review** | Your data, or history that can't be recreated, and probably unused | Yes | Not selected | Old downloads, large files, duplicates, Codex archived sessions, Ollama models, repos with unpushed commits |
| **Protected** | Must not be touched now | **No** | — | Modified < 24 h, open in a process, repo with uncommitted changes, system paths, cloud-only files, user exclusions |

Tier comes from the rule, then signals **can only downgrade it** (Safe → Review → Protected), never upgrade it. For example, a Safe cache that's open in `Code.exe` becomes Protected until VS Code closes.

### 5.2 Disposal

Moving a file to the Recycle Bin **doesn't free any space** until the bin is emptied, because the bin lives on the same drive. Pare is explicit about this:

| Disposal | When | Space freed | Undo |
|---|---|---|---|
| `delete` | Rebuildable data only (rule says `comes_back = yes / on-demand`) | Immediately | No; the app rebuilds it |
| `recycle` | Everything else; the default when a rule says nothing | When the bin is emptied. Pare offers "Empty Pare's items now" and auto-empties **only the items Pare moved** after 30 days (setting) | Restore from History or Explorer |
| `command` | The tool has its own cleanup command (`ollama rm`, `pnpm store prune`, `npm cache clean --force`, `uv cache prune`, `go clean -cache`) | Immediately | Per tool |
| `guide` | Pare must not touch it (Docker/WSL VHDX, Cursor `state.vscdb`, MSIX app versions) | — | Shows steps; never deletes |

The UI always splits totals: "25.9 GB freed · 2.1 GB in the Recycle Bin".

### 5.3 Hard guarantees
These are the same as CLAUDE.md "Safety invariants"; tests must cover each one.

## 6. Features

### 6.1 Scan (M2, M6)
- Scopes: This PC (all fixed drives), single drive, Desktop, Downloads, Documents, Pictures, custom folders. Multiple chips can be combined.
- Walker mode (no admin) in M2; NTFS fast mode (MFT + USN journal, admin) in M6.
- Streaming results: cards appear while scanning continues. Cancelable. Rescans are incremental after M6.
- Per item: allocated and logical size, created, modified, accessed (shown as "Last opened" only where NTFS last-access updates are on; otherwise hidden), attributes, owner tool (from rule), git info (for project items).

### 6.2 Clean tab: suggestion cards
Card = category. Card shows the tier mix, reclaimable bytes, item count and the CTA "Review". Categories, in default order by bytes:

| Category | Milestone | Default tier |
|---|---|---|
| AI tool caches (Claude Code, Codex, Claude desktop, Cursor, VS Code, Ollama, LM Studio, Hugging Face) | M3 | Mixed |
| Old project build files | M4 | Safe (when pushed and idle) |
| Developer caches (npm, pnpm, Yarn, pip, uv, Cargo, Gradle, NuGet, Go) | M3 | Safe |
| Large files (≥ 500 MB, unmodified 90+ days) | M2 | Review |
| Downloads (installers, extracted archives, partials, old) | M2 | Review / Safe (partials) |
| Recycle Bin | M2 | Safe |
| Duplicates | M5 | Review |
| Junk & temp files | M3 | Safe |
| Browser caches (closed browsers) | M3 | Safe |
| Old screenshots | M5 | Review |
| Empty folders | M2 | Safe |

Users can hide a card (like Files by Google's "hide suggestion"), and bring it back in Settings.

### 6.3 Review list
Group by source, sort by size/age/name/tier, filter by tier, search. Bulk selection: *Select all safe*, *Select older than…*, *Select larger than…*, Shift-range, Ctrl+A. The details panel shows dates side by side: **Created · Modified · Last opened · Last commit (projects) · Age**.

### 6.4 Projects (M4)
Git-aware list of repos with idle detection and artifact sizes. See MILESTONES M4.

### 6.5 History (M2 read-only, M5 restore)
Every cleanup is an operation with items, bytes, disposal and result. Restore recycled items. Export as CSV.

### 6.6 Settings (M5)
Scan roots, exclusions ("Always keep"), thresholds, disposal preferences, appearance, updates (opt-in), language.

### 6.7 Later (M8+)
Tray growth monitor, schedules for Safe-only rules, CLI and MCP server, VHDX compaction wizard, worktree cleanup, installed-installer matching.

## 7. Voice and copy

- **Plain, specific, calm.** "Debug logs older than 7 days", not "Junk detected!".
- **Always the three answers:** *Why Pare suggests this* · *If you remove it* · *Comes back?* (Yes, automatically / Re-downloads when needed / No).
- **Name the tool and path.** Users trust what they can verify: `C:\Users\ana\.claude\debug`.
- **Never shame or scare.** No "Your PC is at risk", no red health meters.
- **Numbers:** "12.8 GB", "1,912 items", "8 months ago". Exact bytes in tooltips.
- **Buttons say what happens:** "Free up 8.8 GB", "Move to Recycle Bin", "Empty 2.1 GB now". Avoid bare "OK" or "Clean".
- Product names (Claude Code, Codex, Cursor, VS Code, Docker…) appear as text only, never as logos. Footer and About carry the non-affiliation line.

## 8. Non-goals

Registry cleaning, "RAM boosters", driver updaters, antivirus, startup "optimizers", browser history/cookie wiping, secure shredding (maybe later, as a separate explicit action), cloud storage management.

## 9. Success signals (no telemetry)

GitHub stars and release downloads, Store installs and rating, rule PRs from the community, issue reports of wrong suggestions (target: zero data-loss reports), the user-visible "freed so far" total (stored locally only).

## 10. Open questions

1. Trademark and domain check for "Pare" before 1.0 (see RESEARCH §6).
2. Should Safe items in AI tools default to selected, given users' attachment to session history? Current answer: only orphaned and log-type items are Safe; history is Review.
3. Should there ever be a paid tier? Current answer: no. Keep it free and MIT; revisit only for an IT/fleet edition.
