# Research: market, innovation, tech stack, distribution, naming

Compiled Nov 18, 2026. Prices and versions change, so re-check before quoting them publicly.

## 1. Market snapshot

| Product | Platform / model | What it does well | Gap Pare fills |
|---|---|---|---|
| **Files by Google** | Android, free, no ads | The UX reference: a Clean tab of suggestion cards (junk, duplicates, large files, downloads, old screenshots, unused apps), review-then-delete, hideable cards | Not on Windows; no dev or AI awareness |
| **Windows Storage Sense / Cleanup recommendations** | Built in | Temp files, large/unused files, OneDrive-synced files, unused apps; automatic schedules | Generic categories, little explanation, nothing for dev tools, AI tools or git |
| **Microsoft PC Manager** | Free; region-limited | Deep cleanup, large files, duplicates, downloaded-files view | One review saw it estimate 2.2 GB and free 425 MB without explaining why. That's a trust gap |
| **WizTree** | Free personal, paid commercial | Reads the NTFS MFT directly, so it's extremely fast; treemap | Analyzer only: shows where space goes, not what's safe to delete |
| **WinDirStat 2.x** | Open source (GPL) | 2024 rewrite added multithreading, MFT scanning and dark mode | Same: analyzer only |
| **TreeSize** | Freemium | Reports, cloud and network targets | Pro-oriented, analyzer |
| **CCleaner / BleachBit** | Freemium / OSS | Broad app-cache coverage | Little context per item; no git or AI-tool logic |
| **Czkawka / Krokiet** | OSS (Rust), very popular | Duplicates, similar images, big and empty files | General-purpose, not explanatory |
| **Kudu, Mole, eclean, Fluent Cleaner, ebb** | New wave, 2025–26 | Open rule catalogs (Kudu), AI-session protections (Mole), "explained" cleaning (eclean), MCP integration (ebb) | Mostly cross-platform or Mac-first; none combines Files-style review, git-aware projects and MFT speed on Windows |
| **kondo, npkill, claude-code-cleaner** | OSS CLIs/TUIs | Developer artifacts; Claude Code data | Terminal-only, single-purpose |

**Takeaway:** the analyzers are fast but don't advise, and the cleaners advise but don't explain or understand developers. Nobody on Windows pairs a calm Files-like review flow with AI-tool and git awareness.

## 2. Why now: AI tools fill disks

- Claude Code keeps everything under `~\.claude` with no size cap. Issues report debug logs over 3 GB, a 293 GB disk filled by Claude Code data on Windows, and 1.7 GB in three weeks. The settings docs describe `cleanupPeriodDays`, but session data still accumulates.
- Codex CLI records every session as JSONL rollouts. One public issue documents sub-agent sessions consuming about 731 GiB, and archiving moves rollouts without freeing space.
- The Claude desktop app keeps a 10–12 GB VM bundle in AppData, which in the MSIX install can't be moved to another drive. Users also report old MSIX versions piling up (10–13 GB).
- Cursor's `state.vscdb` (and its `.backup`) has been reported at 50–100 GB.

The pain is real, recent, publicly documented, and growing with AI adoption. That's what makes the "AI tool caches" card the headline feature.

## 3. Innovation: what Pare does differently

1. **Three answers for every item:** why it's here, what you lose, and whether it comes back.
2. **Tiering with downgrade-only signals:** Safe, Review or Protected. Recency, open handles, git state and cloud attributes can only make an item *safer to keep*.
3. **Honest disposal.** Rebuildable data is deleted; your files go to the Recycle Bin; tool commands are used where they exist; "guide" handles things Pare must never touch. Totals separate "freed now" from "in Recycle Bin". Most cleaners blur this.
4. **Git-aware project cleanup:** only artifact dirs that git ignores, only in clean, pushed, idle repos.
5. **AI-tool rules with orphan detection:** transcripts for deleted projects, superseded CLI releases, live-session protection.
6. **Analyzer-grade speed:** MFT enumeration plus USN journal for incremental rescans (M6).
7. **Correct sizes:** allocated size, hard links counted once (matters for pnpm stores and WinSxS).
8. **Open, auditable rules** in TOML, plus a local op log.
9. **Later:** a CLI and MCP server so coding agents can *ask* about disk use and propose (never execute) cleanups; a growth monitor ("`~\.codex` grew 12 GB this week").

## 4. Tech stack evaluation

| Option | Footprint | Fit | Verdict |
|---|---|---|---|
| **Tauri 2** (Rust + WebView2) | Typical installers ~3–10 MB; low idle RAM; WebView2 ships with Windows 11 | Rust for Win32/NTFS work; web UI matches Figma closely and shares tokens with the landing page | **Chosen** |
| Electron | 80–200 MB installers, higher RAM | Fast UI dev, but Node isn't ideal for MFT work, and it's a heavy download for a space-saving tool | No |
| WinUI 3 / .NET | Native look, Windows-only | Excellent OS integration; C# P/Invoke for NTFS; no shared code with the web landing | Strong runner-up |
| Slint / egui (Rust-native UI) | Very small | Krokiet proves it works; harder to reach Files-by-Google polish and Figma fidelity | No |

**Windows building blocks (Rust):** `jwalk` for walking; `ntfs-reader` or `usn-journal-rs` for MFT enumeration and USN journal (admin required); the `windows` crate for Restart Manager, `GetFileInformationByHandleEx`, `SHQueryRecycleBinW` and known folders; `trash`, which uses `IFileOperation` with `FOF_ALLOWUNDO` and on Windows can list, restore and purge items; `git2` (vendored libgit2); `blake3` and xxh3 for duplicates.

## 5. Distribution and code signing

| Route | Cost | Notes |
|---|---|---|
| **Microsoft Store (MSIX)** | Free registration for individual developers since Sept 2025 | Store signs, hosts and updates MSIX, so no SmartScreen friction for Store installs. Company accounts still pay a one-time fee |
| **SignPath Foundation** | Free for OSS | Requires an OSI license; the certificate is issued to the SignPath Foundation; MFA and a signing policy are required |
| **Azure Artifact Signing** (formerly Trusted Signing) | ~$9.99/month basic | Public-trust certificates for individuals **only in the USA and Canada**; organizations in the USA, Canada, EU and UK. Reputation still builds over time |
| OV certificate from a CA | ~$150–300+/year | Same reputation build-up |
| Unsigned GitHub Release | Free | Fine for v0.x with clear SmartScreen guidance |

**Recommendation:** ship v0.x unsigned on GitHub Releases. Apply to SignPath early (MIT license qualifies), and publish to the Microsoft Store at 1.0. If the maintainer is outside the USA and Canada, Artifact Signing for individuals isn't available, which makes the Store plus SignPath the practical path. Add winget at 1.0.

## 6. Naming

Goal: short, ownable, and as plain as "Files", meaning *make room, keep what matters*.

| Candidate | Status from quick web checks |
|---|---|
| Winnow | Conflicts with an older Windows freeware called "Winnow Cleaner" |
| Roomy | Taken by Mac storage cleaners |
| Spruce | Taken by a Linux system cleaner on Flathub |
| Whittle | Taken by a photo-cleanup app |
| ebb | Taken by an open-source disk cleaner (with an MCP server) |
| Shedi / Sift | Taken (Mac dev cleaner) / crowded |
| **Pare** | **No storage or cleanup product found.** Meaning: to trim to the essentials ("pare down") |

**Chosen: Pare.** Before 1.0: run a trademark search (USPTO TESS / WIPO Global Brand Database, Nice class 9), check domains (`pare.app`, `getpare.app`, `usepare.com`, `pare.so`), and reserve the GitHub org and Store name.

## Sources

- Files by Google features: https://www.technipages.com/how-to-get-the-most-out-of-google-files-app · https://www.wikiwand.com/en/articles/Files_by_Google · https://droidviews.com/?p=150715
- Windows cleanup recommendations: https://windowscentral.com/how-free-space-windows-11
- PC Manager: https://windowslatest.com/2024/05/11/microsoft-pc-manager-performance-booster-for-windows-11-gets-files-cleanup · https://askwoody.com/?p=2655474
- WizTree / WinDirStat / TreeSize: https://www.foldersizes.com/compare · https://www.foldersizes.com/features/wiztree · https://diskanalyzer.com/
- Czkawka / Krokiet: https://github.com/qarmin/czkawka · https://www.linuxlinks.com/krokiet-graphical-duplicate-file-finder-system-cleaner/
- Kudu: https://usekudu.com/cleaners/claude-code-cache · Mole: https://mole.fit/blog/how-to-clean-up-ai-coding-tools-mac · eclean: https://eclean.gg/compare/eclean-vs-storage-sense/ · Fluent Cleaner: https://www.windowscentral.com/microsoft/windows-11/fluent-cleaner-may-be-the-best-ccleaner-alternative-for-windows-11-users · ebb: https://www.toolcenter.ai/en/tools/ebb
- kondo: https://github.com/tbillington/kondo · npkill: https://flaviocopes.com/npkill.md · claude-code-cleaner: https://github.com/garrickz2/claude-code-cleaner
- Claude Code disk growth: https://github.com/anthropics/claude-code/issues/11646 · https://claudeissues.com/issue/24207-bug-no-disk-space-management-claude-grows-unbounded-cascade-failure-destroys-set · https://claudeissues.com/issue/24486-feature-add-automatic-garbage-collection-for-claude-cache-directories · https://bestagent.dev/claude-code-disk-usage-cleanup
- Claude desktop VM bundles and MSIX versions: https://claudeissues.com/issue/47395-bug-claude-desktop-vm-bundle-uses-11-3-gb-requesting-optimization · https://claudeissues.com/issue/61350-allow-configuring-localcache-cowork-vm-data-location-on-windows-msix-install · https://claudeissues.com/issue/79208-bug-windows-msix-auto-update-doesnt-remove-old-claude-desktop-version-folders
- Codex sessions: https://github.com/openai/codex/issues/34061 · https://braindetox.kr/en/posts/codex_cli_disk_bloat_2026.html · https://aident.ai/blog/fix-codex-session-jsonl-disk-space
- Cursor `state.vscdb`: https://forum.cursor.com/t/cursor-state-vscdb-suddenly-grew-to-77-5-gb-and-globalstorage-reached-100-gb-on-windows/169204
- Docker/WSL VHDX: https://woshub.com/shrink-wsl2-and-docker-desktop-vhdx-files-in-windows/
- Tauri vs Electron: https://tech-insider.org/tauri-vs-electron-desktop-app-framework-2026/ · https://www.youngju.dev/blog/culture/2026-05-14-tauri-2-rust-webview-desktop-mobile-electron-alternative-deep-dive-2026.en
- NTFS crates: https://docs.rs/crate/ntfs-reader/0.4.6 · https://docs.rs/usn-journal-rs · `trash` crate: https://docs.rs/crate/trash/5.2.6/source/src/windows.rs
- Signing: https://learn.microsoft.com/windows/apps/package-and-deploy/code-signing-options · https://www.devclass.com/security/2026/01/14/code-signing-windows-apps-may-be-easier-and-more-secure-with-new-azure-artifact-service/4079554 · https://signpath.org/ · https://signpath.org/terms
- Microsoft Store free individual registration: https://blogs.windows.com/windowsdeveloper/2025/09/10/free-developer-registration-for-individual-developers-on-microsoft-store/ · https://learn.microsoft.com/en-my/windows/apps/publish/whats-new-individual-developer
