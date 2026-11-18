# Distribution (M7)

Everything here needs the maintainer's own accounts or secrets, so it's prepared but not switched on. Do these in order; each step says what to set and where.

## 1. Updater signing key (GitHub builds)

The in-app update check is opt-in and only exists in builds signed for it.

1. Generate a key pair on your machine (keep the private key out of the repo):
   ```powershell
   pnpm --filter desktop tauri signer generate -w $HOME\.pare-updater.key
   ```
2. In the GitHub repo settings:
   - Secret `TAURI_SIGNING_PRIVATE_KEY`: the contents of `.pare-updater.key`
   - Secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: its password (empty if none)
   - Variable `PARE_UPDATER_PUBKEY`: the public key it printed
3. The release workflow then builds with `createUpdaterArtifacts`, uploads `latest.json`, and compiles the public key into the app. Builds without the variable have no updater at all.

## 2. Code signing

Unsigned v0.x builds trigger SmartScreen; release notes explain "More info → Run anyway".

- **SignPath Foundation** (free for OSI-licensed projects): apply at signpath.org with the repo URL. After approval, add secret `SIGNPATH_API_TOKEN` and variables `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG`, `SIGNPATH_SIGNING_POLICY_SLUG`, then replace the placeholder "Sign installer (SignPath)" step in `.github/workflows/release.yml` with `signpath/github-action-submit-signing-request` (upload the NSIS artifact, wait for completion, download the signed file before checksums).
- **Azure Artifact Signing** (~$9.99/month) is only open to individuals in the USA and Canada.

Either way, SmartScreen reputation builds up over time.

## 3. Microsoft Store (MSIX)

1. Register a free individual developer account in Partner Center and reserve the name "Pare".
2. Build the app (`pnpm --filter desktop tauri build`) and package `target/release/pare-desktop.exe` plus the WebView2 bootstrapper with the MSIX Packaging Tool or `winapp`/`makeappx`. Use the Store tiles from `apps/desktop/src-tauri/icons/Square*Logo.png` and `StoreLogo.png`.
3. Submit; the Store signs and updates the package. Store builds must not include the updater (leave `PARE_UPDATER_PUBKEY` unset for that build).

## 4. winget

After v1.0.0 is published:

`packaging/winget/manifests/t/tanzir71/Pare/0.1.0/` is ready for the published v0.1.0 release (hash from its `SHA256SUMS.txt`). For later versions:

1. Copy `SHA256SUMS.txt`'s hash for the installer into `packaging/winget/manifests/t/tanzir71/Pare/<version>/tanzir71.Pare.installer.yaml`.
2. Validate: `winget validate packaging/winget/manifests/t/tanzir71/Pare/1.0.0`.
3. Open a PR to `microsoft/winget-pkgs` with that folder (or use `wingetcreate submit`).

## 5. Release checklist

- [ ] CI green on `main`
- [ ] Bump `version` in `apps/desktop/src-tauri/tauri.conf.json` and `apps/desktop/src-tauri/Cargo.toml`
- [ ] Update `CHANGELOG.md`
- [ ] Tag `vX.Y.Z` and push the tag; review the draft release, then publish it
- [ ] Landing: set `RELEASED = true` in `apps/web/src/site.ts` (switches every CTA to Download) and update the announcement pill
