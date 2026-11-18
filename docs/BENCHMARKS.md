# Benchmarks

A synthetic million-file volume in CI is impractical, so speed is measured by hand with the protocol below and recorded here. Always note the machine, Windows build, disk, and whether the cache was warm.

## Protocol

**Walker mode (no admin)**

```powershell
cargo build --release -p pare-cli
.\target\release\pare.exe scan --path C:\Users\<you> --no-in-use   # per-phase timings go to stderr
cargo test -p pare-core --release --test scan_perf -- --include-ignored --nocapture   # 200k-file fixture
cargo test -p pare-core --release --test projects -- --nocapture                     # 50 repos
```

**Fast mode (administrator terminal, NTFS)**

```powershell
.\target\release\pare.exe index C:\ --save      # MFT read time, entries, save time, USN catch-up
cargo test -p pare-core --release --test fast_mode -- --ignored --nocapture
```

`fast_mode` creates 1,000 files in a temp folder and times the USN-journal update (target: under 2 s), and checks that an NTFS-compressed file reports its real size on disk.

Run each command twice and record the second (warm) run unless the row says cold.

## Results

Machine A: Windows 11 Home 26200, C: 1.85 TB used, D: 927 GB used, not elevated.

| Measurement | Result | Target | Notes |
|---|---|---|---|
| 200k-file fixture, walker | **0.55 s** | progress ≥ 4/s | release, warm; worst progress sample 6.5 µs |
| User profile, walker, 4.4M files | **67 s** enumerate, 204 s total | — | release; before the M5 fixes below |
| 50 git repos | **0.38 s** | < 3 s | release, warm |
| NSIS installer | **1.71 MiB** | < 15 MB | v0.1.0 build |
| Window visible after launch | **60–180 ms** | < 1 s | release build |
| Fast mode: MFT index, 1M+ files | not yet measured | < 15 s | needs an administrator terminal |
| Fast mode: USN update after 1,000 changes | not yet measured | < 2 s | needs an administrator terminal |

### What the 4.4M-file profile changed

The first real-profile run (204 s) showed where walker mode spends time: enumeration (67 s), filtering `%TEMP%` file by file (64 s), and hard-link checks on the pnpm store (66 s). Fixes that followed:

- Allocation-free, case-insensitive path comparison.
- `prune` commands don't check hard links (their size is informational).
- Large trees sample 256 files for extra hard links before checking every file.
- Whole-drive trees keep folders plus only the files detectors need; the rest fold into per-folder totals (an earlier whole-drive run reached 7 GB of memory).
- One file held open no longer marks a whole rule finding as in use.

Fast mode (MFT + USN journal) is the answer for whole drives; walker mode stays the fallback when Pare isn't elevated or the drive isn't NTFS.
