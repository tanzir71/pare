//! Non-Windows fallback so the platform-independent logic builds and tests on Linux CI.
//! Pare only ships for Windows.

use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use super::{
    ATTR_DIRECTORY, ATTR_HIDDEN, ATTR_REPARSE_POINT, Drive, IO_REPARSE_TAG_SYMLINK, KnownFolder,
    RawEntry, system_time_to_ms,
};
use crate::types::ProcessHolder;

fn entry_from(name: std::ffi::OsString, m: &std::fs::Metadata) -> RawEntry {
    let ft = m.file_type();
    let mut attrs = 0;
    if ft.is_dir() {
        attrs |= ATTR_DIRECTORY;
    }
    if ft.is_symlink() {
        attrs |= ATTR_REPARSE_POINT;
    }
    if name.to_string_lossy().starts_with('.') {
        attrs |= ATTR_HIDDEN;
    }
    let modified = m.modified().map(system_time_to_ms).unwrap_or(0);
    RawEntry {
        name,
        is_dir: ft.is_dir(),
        attrs,
        reparse_tag: if ft.is_symlink() {
            IO_REPARSE_TAG_SYMLINK
        } else {
            0
        },
        logical: m.len(),
        allocated: m.blocks() * 512,
        file_id: ((m.dev() as u128) << 64) | m.ino() as u128,
        links: m.nlink() as u32,
        created: m.created().map(system_time_to_ms).unwrap_or(modified),
        modified,
        accessed: m.accessed().map(system_time_to_ms).unwrap_or(0),
    }
}

pub fn stat(path: &Path) -> io::Result<RawEntry> {
    let m = std::fs::symlink_metadata(path)?;
    Ok(entry_from(
        path.file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_default(),
        &m,
    ))
}

pub fn list_dir(path: &Path) -> io::Result<Vec<RawEntry>> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(path)? {
        let e = e?;
        let Ok(m) = std::fs::symlink_metadata(e.path()) else {
            continue;
        };
        out.push(entry_from(e.file_name(), &m));
    }
    Ok(out)
}

pub fn link_count(path: &Path) -> io::Result<(u32, u128)> {
    let s = stat(path)?;
    Ok((s.links, s.file_id))
}

pub fn volume_serial(path: &Path) -> Option<u64> {
    std::fs::symlink_metadata(path).ok().map(|m| m.dev())
}

pub fn remove_file(path: &Path) -> io::Result<()> {
    std::fs::remove_file(path)
}

pub fn remove_dir(path: &Path) -> io::Result<()> {
    let m = std::fs::symlink_metadata(path)?;
    if m.file_type().is_symlink() {
        std::fs::remove_file(path)
    } else {
        std::fs::remove_dir(path)
    }
}

pub fn known_folder(_kind: KnownFolder) -> Option<PathBuf> {
    None
}

pub fn recycle_bin_query(_root: &Path) -> io::Result<(u64, u64)> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Recycle Bin is Windows-only",
    ))
}

pub fn recycle_bin_empty(_root: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Recycle Bin is Windows-only",
    ))
}

pub fn processes_holding(_paths: &[PathBuf]) -> io::Result<Vec<ProcessHolder>> {
    Ok(Vec::new())
}

pub fn list_drives() -> Vec<Drive> {
    use sysinfo::Disks;
    let Ok(disks) = Disks::new_with_refreshed_list() else {
        return Vec::new();
    };
    disks
        .iter()
        .map(|d| Drive {
            root: d.mount_point().to_path_buf(),
            letter: d.mount_point().to_string_lossy().into_owned(),
            label: d.name().to_string_lossy().into_owned(),
            fs: d.file_system().to_string_lossy().into_owned(),
            total: d.total_space(),
            free: d.available_space(),
            is_ntfs: false,
            is_fixed: !d.is_removable(),
        })
        .collect()
}
