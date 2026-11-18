## Pare {version}

Pare finds caches, idle build folders, AI-tool logs and forgotten downloads, and shows you what's safe to remove **before** anything is deleted.

### Install

1. Download `Pare_*_x64-setup.exe` below. It installs for your user account only (no admin rights).
2. **This build isn't code-signed yet.** Windows SmartScreen may show "Windows protected your PC". Click **More info → Run anyway**. Signing comes with 1.0 (see the roadmap).
3. To check the download, compare its SHA-256 with `SHA256SUMS.txt`:
   `Get-FileHash .\Pare_*_x64-setup.exe -Algorithm SHA256`

### Safety

- A scan only reads. Nothing is removed until you choose.
- Your own files go to the Recycle Bin; only rebuildable caches are deleted directly.
- Anything used in the last 24 hours, open in an app, or part of uncommitted work is never suggested.
- No account, no telemetry. Every action is logged to `%LOCALAPPDATA%\Pare\logs\operations.jsonl`.

Pare is an independent project and is not affiliated with Microsoft, Google, Anthropic or OpenAI.
