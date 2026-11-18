# Pare

**Make room on your PC. Keep what matters.**

Pare is a free, open-source cleanup app for Windows 10 and 11. It finds caches, idle build folders, AI-tool logs and forgotten downloads, and tells you what's safe to remove **before** anything is deleted.

> Status: v0.1.0 is out, with everything through M8 (English and Spanish). Code signing, the Microsoft Store and winget are next. See [docs/MILESTONES.md](docs/MILESTONES.md), [CHANGELOG.md](CHANGELOG.md) and [docs/DISTRIBUTION.md](docs/DISTRIBUTION.md).

## Why Pare

- **Suggestions, not guesswork.** Files-by-Google-style cards: AI tool caches, old project build files, developer caches, large files, downloads, duplicates, junk.
- **Every item explains itself:** why it's suggested, what you lose, and whether it comes back on its own.
- **Knows your tools:** Claude Code, Codex, Cursor, VS Code, Ollama, npm, pnpm, pip, uv, Cargo, Gradle, NuGet, Docker and more.
- **Git-aware:** cleans `node_modules`, `target` and `.venv` only in projects that are committed, pushed and idle.
- **Careful by default:** your files go to the Recycle Bin first; anything used in the last 24 hours, open in an app, or part of uncommitted work is never suggested.
- **Local and auditable:** no account, no telemetry, open-source rules, and a log of every action.

## Install

Download `Pare_<version>_x64-setup.exe` from the [latest release](https://github.com/tanzir71/pare/releases/latest). It installs for your user account only (no administrator rights).

The installer isn't code-signed yet, so Windows SmartScreen may say "Windows protected your PC" the first time: click **More info → Run anyway**. To check the download, compare `Get-FileHash .\Pare_*_x64-setup.exe -Algorithm SHA256` with `SHA256SUMS.txt` on the release page.

## Develop

See [CLAUDE.md](CLAUDE.md) for commands, conventions and the safety rules every change must keep.

```bash
pnpm install
pnpm --filter web dev            # landing page
pnpm --filter desktop tauri dev  # desktop app
cargo test -p pare-core
```

Command line (read-only unless you pass `--yes`):

```bash
pare scan [--path <folder>]... [--json]        # suggestions, like the app
pare clean --rule claude-code.debug-logs        # dry run; add --yes to do it
pare rules check                                # list and measure each rule's paths
pare mcp                                        # MCP server for coding agents (propose, never execute)
pare index C:\ --save                           # fast-mode index (administrator terminal)
```

## Contribute a cleanup rule

Rules live in [`rules/`](rules) as TOML. The schema is in [docs/RULES.md](docs/RULES.md). Please include the exact path, what's inside, how you verified it, and whether the app rebuilds it.

## License

MIT. Pare is an independent project and is not affiliated with Microsoft, Google, Anthropic or OpenAI.
