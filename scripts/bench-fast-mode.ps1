# Fast-mode (M6) benchmark. Run from the repo root in an ADMINISTRATOR PowerShell.
# Read-only: it reads the NTFS file table and USN journal; the test's 1,000 files go to %TEMP%.
# Record the numbers in docs/BENCHMARKS.md.
$ErrorActionPreference = 'Stop'
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  Write-Error 'Run this from an administrator terminal (fast mode reads the MFT).'
}
cargo build --release -p pare-cli
foreach ($i in 1..2) { .\target\release\pare.exe index C:\ }   # second run is the warm number
cargo test -p pare-core --release --test fast_mode -- --ignored --nocapture --test-threads 1
