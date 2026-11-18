# Pare

**Make room on your PC. Keep what matters.**

Pare is a free, open-source cleanup app for Windows 10 and 11. It finds caches, idle build folders, AI-tool logs and forgotten downloads, and tells you what's safe to remove **before** anything is deleted.

> Status: in development. See [docs/MILESTONES.md](docs/MILESTONES.md) for the plan.

## Why Pare

- **Suggestions, not guesswork.** Files-by-Google-style cards: AI tool caches, old project build files, developer caches, large files, downloads, duplicates, junk.
- **Every item explains itself:** why it's suggested, what you lose, and whether it comes back on its own.
- **Knows your tools:** Claude Code, Codex, Cursor, VS Code, Ollama, npm, pnpm, pip, uv, Cargo, Gradle, NuGet, Docker and more.
- **Git-aware:** cleans `node_modules`, `target` and `.venv` only in projects that are committed, pushed and idle.
- **Careful by default:** your files go to the Recycle Bin first; anything used in the last 24 hours, open in an app, or part of uncommitted work is never suggested.
- **Local and auditable:** no account, no telemetry, open-source rules, and a log of every action.

## Install

Releases will appear on the [Releases page](../../releases). Until code signing is in place (M7), Windows SmartScreen may warn on first run.

## Develop

See [CLAUDE.md](CLAUDE.md) for commands, conventions and the safety rules every change must keep.

```bash
pnpm install
pnpm --filter web dev            # landing page
pnpm --filter desktop tauri dev  # desktop app
cargo test -p pare-core
```

## Contribute a cleanup rule

Rules live in [`rules/`](rules) as TOML. The schema is in [docs/RULES.md](docs/RULES.md). Please include the exact path, what's inside, how you verified it, and whether the app rebuilds it.

## License

MIT. Pare is an independent project and is not affiliated with Microsoft, Google, Anthropic or OpenAI.
