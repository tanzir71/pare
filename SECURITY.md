# Security policy

Pare deletes files, so we treat anything that could remove the wrong thing as a security issue, not just a bug.

## Reporting a vulnerability

Please report privately through GitHub: open the repository's **Security** tab and choose **Report a vulnerability**. Don't open a public issue for security problems.

Include what you can:

- what happens, and what you expected to happen
- the Pare version and Windows version
- steps or a folder layout that reproduces it
- the relevant lines from `%LOCALAPPDATA%\Pare\logs\operations.jsonl`, with personal paths redacted

We'll confirm we received the report, keep you updated while we work on a fix, and credit you in the release notes if you'd like.

## What counts

Examples of things we want to hear about:

- Pare suggests or removes something it should protect: files changed in the last 24 hours, files open in another app, repositories with uncommitted or unpushed work, system folders, or cloud-only placeholders
- a cleanup follows a junction or symlink, or deletes a link's target
- a disposal skips the Recycle Bin when its rule says it should use it
- the app makes a network call other than the opt-in update check, or sends data anywhere
- a crafted cleanup rule in `rules/` that makes Pare touch paths outside its target

## Supported versions

Pare hasn't had a release yet. Once it does, security fixes go into the latest release only.
