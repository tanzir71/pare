//! Hard-excluded roots and never-touch user config (CLAUDE.md safety invariant 6).

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use crate::env::Env;
use crate::platform::{eq_component, path_starts_with};
use crate::types::ProtectReason;

/// Folders at the root of *any* drive that Pare never scans or touches.
const DRIVE_ROOT_DIRS: &[&str] = &[
    "Windows",
    "Program Files",
    "Program Files (x86)",
    "ProgramData",
    "WindowsApps",
    "System Volume Information",
    "$Recycle.Bin",
    "Recovery",
    "$WinREAgent",
    "$SysReset",
    "$Windows.~BT",
    "$Windows.~WS",
    "Config.Msi",
    "Boot",
];

/// Files at the root of any drive.
const DRIVE_ROOT_FILES: &[&str] = &[
    "pagefile.sys",
    "hiberfil.sys",
    "swapfile.sys",
    "bootmgr",
    "BOOTNXT",
    "DumpStack.log",
    "DumpStack.log.tmp",
];

/// Never-touch user config relative to `~` (patterns may use `*` in the last component).
const HOME_NEVER_TOUCH: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".aws",
    ".azure",
    ".kube",
    ".claude/settings*.json",
    ".claude/.credentials*",
    ".claude.json",
    ".codex/auth.json",
    ".codex/config.toml",
    ".docker/config.json",
    ".git-credentials",
    ".gitconfig",
    ".npmrc",
    ".pypirc",
    ".netrc",
    ".cargo/credentials*",
];

/// Exceptions inside excluded roots that admin rules may target (M3): `Windows\Temp` and
/// `ProgramData\Microsoft\Windows\WER`.
fn admin_exception(rest: &[&OsStr]) -> bool {
    let is = |i: usize, s: &str| rest.get(i).is_some_and(|c| eq_component(c, OsStr::new(s)));
    (is(0, "Windows") && is(1, "Temp"))
        || (is(0, "ProgramData") && is(1, "Microsoft") && is(2, "Windows") && is(3, "WER"))
}

/// Simple `*` wildcard match, case-insensitive.
pub fn wildcard_match(pattern: &str, name: &str) -> bool {
    fn inner(p: &[u8], n: &[u8]) -> bool {
        match (p.first(), n.first()) {
            (None, None) => true,
            (Some(b'*'), _) => inner(&p[1..], n) || (!n.is_empty() && inner(p, &n[1..])),
            (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => inner(&p[1..], &n[1..]),
            _ => false,
        }
    }
    inner(pattern.as_bytes(), name.as_bytes())
}

/// `Some(reason)` if `path` is (inside) a hard-excluded root or a never-touch file.
/// `allow_admin_exceptions` lets admin rules reach `Windows\Temp` and `ProgramData\…\WER`.
pub fn classify(path: &Path, env: &Env, allow_admin_exceptions: bool) -> Option<ProtectReason> {
    // Pare's own folders.
    if path_starts_with(path, &env.pare_data_dir())
        || path_starts_with(path, &env.pare_config_dir())
    {
        return Some(ProtectReason::SystemPath);
    }

    // Never-touch user config.
    for pat in HOME_NEVER_TOUCH {
        if matches_home_pattern(path, &env.home, pat) {
            return Some(ProtectReason::NeverTouch);
        }
    }

    // Drive-root system folders, on every drive.
    let comps: Vec<Component> = path.components().collect();
    if let Some(Component::Prefix(_)) = comps.first() {
        let rest: Vec<&OsStr> = comps
            .iter()
            .skip(1)
            .filter(|c| !matches!(c, Component::RootDir))
            .map(|c| c.as_os_str())
            .collect();
        if let Some(first) = rest.first() {
            let first_s = first.to_string_lossy();
            let is_dir_hit = DRIVE_ROOT_DIRS
                .iter()
                .any(|d| eq_component(first, OsStr::new(d)))
                || first_s.to_ascii_lowercase().starts_with("program files");
            if is_dir_hit && !(allow_admin_exceptions && admin_exception(&rest)) {
                return Some(ProtectReason::SystemPath);
            }
            if rest.len() == 1
                && DRIVE_ROOT_FILES
                    .iter()
                    .any(|f| eq_component(first, OsStr::new(f)))
            {
                return Some(ProtectReason::SystemPath);
            }
        }
    }

    // Environment-provided system folders (also covers a FakeHome's ProgramData).
    if let Some(sr) = &env.system_root {
        if path_starts_with(path, sr)
            && !(allow_admin_exceptions && path_starts_with(path, &sr.join("Temp")))
        {
            return Some(ProtectReason::SystemPath);
        }
    }
    if path_starts_with(path, &env.program_data) {
        let wer = env
            .program_data
            .join("Microsoft")
            .join("Windows")
            .join("WER");
        if !(allow_admin_exceptions && path_starts_with(path, &wer)) {
            return Some(ProtectReason::SystemPath);
        }
    }
    None
}

fn matches_home_pattern(path: &Path, home: &Path, pattern: &str) -> bool {
    if !path_starts_with(path, home) {
        return false;
    }
    let rel: Vec<&OsStr> = path
        .components()
        .skip(home.components().count())
        .map(|c| c.as_os_str())
        .collect();
    let pat: Vec<&str> = pattern.split('/').collect();
    if rel.len() < pat.len() {
        return false;
    }
    // Path is the pattern itself or inside it.
    pat.iter()
        .zip(rel.iter())
        .all(|(p, c)| wildcard_match(p, &c.to_string_lossy()))
}

/// Does removing `dir` as a whole also remove something hard-excluded or never-touch?
/// (e.g. selecting all of `~\.claude` would take `settings.json` with it).
pub fn contains_never_touch(dir: &Path, env: &Env) -> bool {
    HOME_NEVER_TOUCH.iter().any(|pat| {
        let target: PathBuf = pat.split('/').fold(env.home.clone(), |acc, c| acc.join(c));
        // The pattern's concrete parent (wildcards stripped) lies inside `dir`.
        let concrete: PathBuf = target
            .components()
            .take_while(|c| !c.as_os_str().to_string_lossy().contains('*'))
            .collect();
        path_starts_with(&concrete, dir)
    }) || path_starts_with(&env.pare_data_dir(), dir)
        || path_starts_with(&env.pare_config_dir(), dir)
}

/// Folders Windows or the user needs to exist: never suggested for removal themselves
/// (their contents may be).
pub fn is_essential_folder(path: &Path, env: &Env) -> bool {
    let same =
        |a: &Path| path_starts_with(path, a) && path.components().count() == a.components().count();
    if path.parent().is_none() || path.components().count() <= 2 {
        // A drive root (C:\) or the filesystem root.
        return true;
    }
    [
        &env.home,
        &env.appdata,
        &env.local_appdata,
        &env.temp,
        &env.desktop,
        &env.downloads,
        &env.documents,
        &env.pictures,
        &env.program_data,
    ]
    .iter()
    .any(|p| same(p))
        || env.home.parent().is_some_and(same)
}
