//! M2 "Quick Clean" detectors: large files, Downloads (installers, extracted archives, partial
//! and old downloads), empty folders and the Recycle Bin.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::DetectCtx;
use crate::format::{age_phrase, format_bytes};
use crate::i18n::{tr, trf};
use crate::platform::{self, eq_component};
use crate::safety::protected_paths;
use crate::scan::measure::Candidate;
use crate::scan::tree::{Leaf, NO_NODE, NodeId, Tree};
use crate::types::{CategoryId, ComesBack, Disposal, MS_PER_DAY, Tier};

const INSTALLER_EXTS: &[&str] = &["exe", "msi", "msix", "msixbundle", "appx", "appxbundle"];
const ARCHIVE_EXTS: &[&str] = &["zip", "7z", "rar"];
const PARTIAL_EXTS: &[&str] = &["crdownload", "part", "partial", "opdownload"];
/// Never suggested as "large files": VM disks and mail stores need their own tools.
const LARGE_FILE_SKIP_EXTS: &[&str] = &[
    "vhd", "vhdx", "avhdx", "vmdk", "vdi", "qcow2", "pst", "ost", "sys",
];
/// Empty folders apps recreate or rely on.
const EMPTY_FOLDER_SKIP: &[&str] = &[
    "Camera Roll",
    "Saved Pictures",
    "Screenshots",
    "Captures",
    "Custom Office Templates",
    "My Music",
    "My Pictures",
    "My Videos",
    "Sound Recordings",
    "OneNote Notebooks",
    "WindowsPowerShell",
    "PowerShell",
];

fn days(ms: i64) -> i64 {
    ms / MS_PER_DAY
}

fn is_hidden_name(name: &OsStr) -> bool {
    let s = name.to_string_lossy();
    s.starts_with('.')
        || s.eq_ignore_ascii_case("desktop.ini")
        || s.eq_ignore_ascii_case("thumbs.db")
}

/// Node ids for folders whose subtrees the quick detectors leave to the rule engine.
fn app_data_nodes(ctx: &DetectCtx) -> HashSet<NodeId> {
    [
        ctx.env.home.join("AppData"),
        ctx.env.appdata.clone(),
        ctx.env.local_appdata.clone(),
        ctx.env.program_data.clone(),
    ]
    .iter()
    .filter_map(|p| ctx.tree.find(p))
    .collect()
}

/// Depth-first over the tree, skipping subtrees `skip` rejects. Calls `visit` for each node
/// that isn't skipped; `visit` returns false to stop descending into that node.
fn walk_filtered(
    tree: &Tree,
    skip: &dyn Fn(NodeId) -> bool,
    visit: &mut dyn FnMut(NodeId) -> bool,
) {
    let mut stack: Vec<NodeId> = tree.roots.iter().map(|r| r.node).collect();
    while let Some(id) = stack.pop() {
        if skip(id) {
            continue;
        }
        if !visit(id) {
            continue;
        }
        stack.extend(tree.children(id));
    }
}

fn root_label(ctx: &DetectCtx, id: NodeId) -> (String, PathBuf) {
    let root = ctx.tree.root_of(id);
    let label = ctx
        .root_labels
        .iter()
        .find(|(n, _)| *n == root)
        .map(|(_, l)| l.clone())
        .unwrap_or_else(|| ctx.tree.path(root).to_string_lossy().into_owned());
    (label, ctx.tree.path(root))
}

/// Downloads: partial downloads, installers, extracted archives, then old downloads.
/// Returns the candidates and the nodes they claim (so "large files" doesn't repeat them).
pub fn downloads(ctx: &DetectCtx) -> (Vec<Candidate>, HashSet<NodeId>) {
    let mut out = Vec::new();
    let mut claimed: HashSet<NodeId> = HashSet::new();
    let Some(dl) = ctx.tree.find(&ctx.env.downloads) else {
        return (out, claimed);
    };
    let tree = ctx.tree;
    let dl_path = tree.path(dl);
    let th = ctx.thresholds;

    let base = |rule: &str, title: String, id: NodeId, source: &str| {
        let mut c = Candidate::new(rule, CategoryId::Downloads, title);
        c.nodes = vec![id];
        c.source = source.to_string();
        c.source_path = Some(dl_path.clone());
        c
    };

    for id in tree.descendants(dl).skip(1) {
        let n = tree.node(id);
        if n.is_dir || n.is_hidden_or_system() || is_hidden_name(&n.name) {
            continue;
        }
        let Some(ext) = n.extension_lower() else {
            continue;
        };
        let age = days(ctx.now - n.modified);
        let name = n.name.to_string_lossy().into_owned();
        if PARTIAL_EXTS.contains(&ext.as_str()) {
            if age >= th.partial_days as i64 {
                let mut c = base("downloads.partial", name, id, &tr("Unfinished downloads"));
                c.tier = Tier::Safe;
                c.disposal = Disposal::Delete;
                c.comes_back = ComesBack::OnDemand;
                c.why = trf(
                    "A download that never finished. It was last written {when}.",
                    &[("when", &age_phrase(ctx.now, n.modified))],
                );
                c.if_removed = tr(
                    "The unfinished file is gone. Start the download again if you still need it.",
                );
                c.note = Some(age_phrase(ctx.now, n.modified));
                claimed.insert(id);
                out.push(c);
            }
        } else if INSTALLER_EXTS.contains(&ext.as_str()) {
            if age >= th.installer_days as i64 {
                let installed = installed_match(&name, &ctx.installed);
                let mut c = base("downloads.installers", name.clone(), id, &tr("Installers"));
                c.tier = Tier::Review;
                c.disposal = Disposal::Recycle;
                c.comes_back = ComesBack::OnDemand;
                c.why = trf(
                    "An installer you downloaded {when}. Once an app is installed, the installer is rarely needed.",
                    &[("when", &age_phrase(ctx.now, n.modified))],
                );
                c.if_removed = tr(
                    "It goes to the Recycle Bin. Download it again from the publisher if you need to reinstall.",
                );
                c.note = Some(age_phrase(ctx.now, n.modified));
                if let Some((app, version)) = installed {
                    let v = version
                        .as_deref()
                        .map(|v| format!(" {v}"))
                        .unwrap_or_default();
                    c.why = trf(
                        "An installer you downloaded {when}. {app} is already installed on this PC, so you likely don't need it.",
                        &[
                            ("when", &age_phrase(ctx.now, n.modified)),
                            ("app", &format!("{app}{v}")),
                        ],
                    );
                    c.note = Some(trf("Installed: {app}", &[("app", app)]));
                    c.facts.push((tr("Installed app"), format!("{app}{v}")));
                }
                claimed.insert(id);
                out.push(c);
            }
        } else if ARCHIVE_EXTS.contains(&ext.as_str()) {
            let stem = Path::new(&*n.name)
                .file_stem()
                .map(|s| s.to_os_string())
                .unwrap_or_default();
            let parent = n.parent;
            if parent == NO_NODE {
                continue;
            }
            let sibling = tree.children(parent).find(|&s| {
                let sn = tree.node(s);
                sn.is_dir && sn.leaf == Leaf::No && eq_component(&sn.name, &stem)
            });
            if let Some(sib) = sibling {
                let folder = tree.node(sib).name.to_string_lossy().into_owned();
                let mut c = base(
                    "downloads.extracted-archives",
                    name,
                    id,
                    &tr("Archives already extracted"),
                );
                c.tier = Tier::Review;
                c.disposal = Disposal::Recycle;
                c.comes_back = ComesBack::No;
                c.why = trf(
                    "You already extracted this archive into the “{folder}” folder next to it.",
                    &[("folder", &folder)],
                );
                c.if_removed =
                    tr("The extracted folder stays. The archive goes to the Recycle Bin.");
                c.note = Some(trf("Extracted to {folder}", &[("folder", &folder)]));
                claimed.insert(id);
                out.push(c);
            }
        }
    }

    // Old downloads: top-level items untouched for a long time.
    let contains_claimed = |id: NodeId| tree.descendants(id).any(|d| claimed.contains(&d));
    let mut old = Vec::new();
    for id in tree.children(dl) {
        let n = tree.node(id);
        if n.is_hidden_or_system() || is_hidden_name(&n.name) || contains_claimed(id) {
            continue;
        }
        if n.is_dir && n.leaf != Leaf::No {
            continue;
        }
        let newest = n.sub_newest;
        if days(ctx.now - newest) < th.old_download_days as i64 {
            continue;
        }
        let name = n.name.to_string_lossy().into_owned();
        let mut c = base("downloads.old", name, id, &tr("Old downloads"));
        c.tier = Tier::Review;
        c.disposal = Disposal::Recycle;
        c.comes_back = ComesBack::No;
        c.why = trf(
            "Downloaded and not changed for {age}.",
            &[("age", &crate::format::duration_phrase(ctx.now - newest))],
        );
        c.if_removed =
            tr("It goes to the Recycle Bin, so you can restore it until the bin is emptied.");
        c.note = Some(trf(
            "Changed {when}",
            &[("when", &age_phrase(ctx.now, newest))],
        ));
        old.push((id, c));
    }
    for (id, c) in old {
        claimed.insert(id);
        out.push(c);
    }
    (out, claimed)
}

/// Large files: ≥ 500 MB (setting), not modified for 90+ days (setting).
pub fn large_files(ctx: &DetectCtx, claimed: &HashSet<NodeId>) -> Vec<Candidate> {
    let tree = ctx.tree;
    let th = ctx.thresholds;
    let app = app_data_nodes(ctx);
    let min_bytes = th.large_file_mb.saturating_mul(1_000_000);
    let mut out = Vec::new();
    let skip = |id: NodeId| {
        let n = tree.node(id);
        claimed.contains(&id)
            || app.contains(&id)
            || (n.parent != NO_NODE
                && n.is_dir
                && (n.is_hidden_or_system() || is_hidden_name(&n.name)))
    };
    let mut hits = Vec::new();
    walk_filtered(tree, &skip, &mut |id| {
        let n = tree.node(id);
        if n.is_dir {
            return true;
        }
        if n.logical >= min_bytes
            && days(ctx.now - n.modified) >= th.large_file_days as i64
            && !n.is_cloud_placeholder()
            && !n
                .extension_lower()
                .is_some_and(|e| LARGE_FILE_SKIP_EXTS.contains(&e.as_str()))
        {
            hits.push(id);
        }
        false
    });
    for id in hits {
        let n = tree.node(id);
        let (label, root_path) = root_label(ctx, id);
        let mut c = Candidate::new(
            "quick.large-files",
            CategoryId::LargeFiles,
            n.name.to_string_lossy().into_owned(),
        );
        c.nodes = vec![id];
        c.source = label;
        c.source_path = Some(root_path);
        c.tier = Tier::Review;
        c.disposal = Disposal::Recycle;
        c.comes_back = ComesBack::No;
        c.why = trf(
            "A {size} file you haven't changed for {age}.",
            &[
                ("size", &format_bytes(n.allocated.max(n.logical))),
                ("age", &crate::format::duration_phrase(ctx.now - n.modified)),
            ],
        );
        c.if_removed =
            tr("It goes to the Recycle Bin, so you can restore it until the bin is emptied.");
        c.note = Some(trf(
            "Changed {when}",
            &[("when", &age_phrase(ctx.now, n.modified))],
        ));
        out.push(c);
    }
    out
}

/// Empty folders (recursively empty), top-most only, in your own folders.
pub fn empty_folders(ctx: &DetectCtx) -> Vec<Candidate> {
    let tree = ctx.tree;
    let app = app_data_nodes(ctx);
    let is_repo = |id: NodeId| {
        tree.children(id)
            .any(|c| eq_component(&tree.node(c).name, OsStr::new(".git")))
    };
    let skip = |id: NodeId| {
        let n = tree.node(id);
        !n.is_dir
            || app.contains(&id)
            || (n.parent != NO_NODE && (n.is_hidden_or_system() || is_hidden_name(&n.name)))
            || n.leaf != Leaf::No
            || is_repo(id)
    };
    let mut hits = Vec::new();
    walk_filtered(tree, &skip, &mut |id| {
        let n = tree.node(id);
        if n.parent == NO_NODE {
            return true; // never the scan root itself
        }
        if n.sub_files == 0 && n.sub_opaque == 0 {
            let path = tree.path(id);
            let named_skip = EMPTY_FOLDER_SKIP
                .iter()
                .any(|s| eq_component(&n.name, OsStr::new(s)))
                || n.name.to_string_lossy().starts_with("Visual Studio");
            if named_skip {
                return false;
            }
            if protected_paths::is_essential_folder(&path, ctx.env)
                || protected_paths::classify(&path, ctx.env, false).is_some()
            {
                return true; // keep looking inside (e.g. an empty Documents)
            }
            hits.push(id);
            return false; // top-most only
        }
        true
    });
    hits.into_iter()
        .map(|id| {
            let n = tree.node(id);
            let (label, root_path) = root_label(ctx, id);
            let mut c = Candidate::new(
                "quick.empty-folders",
                CategoryId::EmptyFolders,
                n.name.to_string_lossy().into_owned(),
            );
            c.nodes = vec![id];
            c.source = label;
            c.source_path = Some(root_path);
            c.tier = Tier::Safe;
            c.disposal = Disposal::Delete;
            c.comes_back = ComesBack::No;
            c.why = tr("This folder (and any folders inside it) contains no files.");
            c.if_removed = tr("Nothing is lost: the folder is empty.");
            c.note = Some(tr("Empty"));
            c
        })
        .collect()
}

/// One Recycle Bin finding per drive that has something in it.
pub fn recycle_bin(drives: &[PathBuf]) -> Vec<Candidate> {
    let mut out = Vec::new();
    for d in drives {
        let Ok((bytes, items)) = platform::recycle_bin_query(d) else {
            continue;
        };
        if bytes == 0 && items == 0 {
            continue;
        }
        let letter = d.to_string_lossy().trim_end_matches('\\').to_string();
        let mut c = Candidate::new(
            "windows.recycle-bin",
            CategoryId::RecycleBin,
            trf("Recycle Bin on {drive}", &[("drive", &letter)]),
        );
        c.source = tr("Recycle Bin");
        c.source_path = Some(d.clone());
        c.fixed_size = Some((bytes, items));
        c.tier = Tier::Safe;
        c.disposal = Disposal::EmptyRecycleBin {
            drive: d.to_string_lossy().into_owned(),
        };
        c.comes_back = ComesBack::No;
        c.why = trf(
            "Files you already deleted. They keep using space on {drive} until the Recycle Bin is emptied.",
            &[("drive", &letter)],
        );
        c.if_removed =
            tr("They're gone for good; you can't restore them from the Recycle Bin anymore.");
        c.note = Some(trf("{n} items", &[("n", &items.to_string())]));
        out.push(c);
    }
    out
}

const GENERIC: &[&str] = &[
    "setup",
    "installer",
    "install",
    "x64",
    "x86",
    "amd64",
    "arm64",
    "win",
    "win64",
    "win32",
    "windows",
    "user",
    "full",
    "latest",
    "release",
    "stable",
    "msi",
    "exe",
    "online",
    "offline",
    "web",
    "64bit",
    "32bit",
    "bit",
];

/// Lower-case words of a name, split on punctuation and camelCase, without versions.
pub fn name_tokens(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for ch in name.chars() {
        if ch.is_alphanumeric() {
            if ch.is_uppercase() && prev_lower && !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            prev_lower = ch.is_lowercase() || ch.is_ascii_digit();
            cur.extend(ch.to_lowercase());
        } else {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    // Drop versions ("3", "131", "v2").
    words.retain(|w| {
        let digits = |x: &str| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit());
        !(digits(w) || (w.starts_with('v') && digits(&w[1..])))
    });
    // Common installer-name spellings of app names.
    for w in &mut words {
        if w == "vscode" {
            *w = "code".into();
        }
    }
    words
}

/// The installed app an installer file most likely installed, if any: every significant word
/// of the file name (3+ letters, not "setup"/"x64"/…) appears in the app's name.
pub fn installed_match<'a>(
    installer: &str,
    apps: &'a [(String, Option<String>)],
) -> Option<&'a (String, Option<String>)> {
    let stem = Path::new(installer)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let words: Vec<String> = name_tokens(&stem)
        .into_iter()
        .filter(|w| w.len() >= 3 && !GENERIC.contains(&w.as_str()))
        .collect();
    if words.is_empty() || !words.iter().any(|w| w.len() >= 4) {
        return None;
    }
    apps.iter().find(|(name, _)| {
        let have = name_tokens(name);
        words.iter().all(|w| have.iter().any(|h| h == w))
    })
}

#[cfg(test)]
mod installer_tests {
    use super::*;

    #[test]
    fn matches_installers_to_apps() {
        let apps = vec![
            ("Google Chrome".to_string(), Some("131.0".to_string())),
            ("Docker Desktop".to_string(), None),
            ("Python 3.13.0 (64-bit)".to_string(), None),
            ("Microsoft Visual Studio Code (User)".to_string(), None),
        ];
        assert_eq!(
            installed_match("ChromeSetup.exe", &apps).map(|a| a.0.as_str()),
            Some("Google Chrome")
        );
        assert_eq!(
            installed_match("Docker Desktop Installer.exe", &apps).map(|a| a.0.as_str()),
            Some("Docker Desktop")
        );
        assert_eq!(
            installed_match("python-3.13.0-amd64.exe", &apps).map(|a| a.0.as_str()),
            Some("Python 3.13.0 (64-bit)")
        );
        assert_eq!(
            installed_match("VSCodeUserSetup-x64-1.95.3.exe", &apps).map(|a| a.0.as_str()),
            Some("Microsoft Visual Studio Code (User)")
        );
        assert!(installed_match("setup.exe", &apps).is_none());
        assert!(installed_match("RandomTool-2.0.msi", &apps).is_none());
    }
}
