//! Thin OS layer. Windows is the real target; the Unix fallback exists so CI on Ubuntu can run
//! the platform-independent logic and tests.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use crate::types::Timestamp;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
pub use self::unix::*;

// FILE_ATTRIBUTE_* values (same numbers on every platform; Unix synthesizes the ones it can).
pub const ATTR_READONLY: u32 = 0x1;
pub const ATTR_HIDDEN: u32 = 0x2;
pub const ATTR_SYSTEM: u32 = 0x4;
pub const ATTR_DIRECTORY: u32 = 0x10;
pub const ATTR_REPARSE_POINT: u32 = 0x400;
pub const ATTR_COMPRESSED: u32 = 0x800;
pub const ATTR_OFFLINE: u32 = 0x1000;
pub const ATTR_RECALL_ON_OPEN: u32 = 0x40000;
pub const ATTR_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;

pub const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xA000_0003;
pub const IO_REPARSE_TAG_SYMLINK: u32 = 0xA000_000C;
/// Bit set on every "name surrogate" tag: the reparse point redirects to another path.
pub const REPARSE_TAG_NAME_SURROGATE: u32 = 0x2000_0000;

/// One directory entry (or a stat of a single path). Never follows reparse points.
#[derive(Clone, Debug)]
pub struct RawEntry {
    pub name: OsString,
    pub is_dir: bool,
    pub attrs: u32,
    /// Valid only when `attrs` has `ATTR_REPARSE_POINT`.
    pub reparse_tag: u32,
    pub logical: u64,
    /// Size on disk (compression/sparse aware where the OS reports it).
    pub allocated: u64,
    /// File identity on its volume (0 = unknown).
    pub file_id: u128,
    /// Hard-link count (0 = unknown; query [`link_count`] when it matters).
    pub links: u32,
    pub created: Timestamp,
    pub modified: Timestamp,
    pub accessed: Timestamp,
}

impl RawEntry {
    pub fn is_reparse(&self) -> bool {
        self.attrs & ATTR_REPARSE_POINT != 0
    }

    /// A cloud-sync reparse point (OneDrive and other Cloud Files providers).
    pub fn is_cloud_tag(&self) -> bool {
        self.is_reparse() && is_cloud_reparse_tag(self.reparse_tag)
    }

    /// Junction, symlink or another name-surrogate: a link to somewhere else.
    pub fn is_link(&self) -> bool {
        self.is_reparse() && (self.reparse_tag & REPARSE_TAG_NAME_SURROGATE != 0)
    }

    /// Safe to descend into: a real directory, or a cloud-sync folder (which is not a redirect).
    /// Every other reparse point (junctions, symlinks, mount points, unknown tags) is a leaf.
    pub fn is_traversable_dir(&self) -> bool {
        self.is_dir && (!self.is_reparse() || self.is_cloud_tag())
    }

    /// Reading this file's content would download it from the cloud.
    pub fn is_cloud_placeholder(&self) -> bool {
        is_cloud_attrs(self.attrs)
    }
}

pub fn is_cloud_attrs(attrs: u32) -> bool {
    attrs & (ATTR_RECALL_ON_DATA_ACCESS | ATTR_RECALL_ON_OPEN | ATTR_OFFLINE) != 0
}

/// IO_REPARSE_TAG_CLOUD and its 16 variants: 0x9000001A, 0x9000101A, … 0x9000F01A.
pub fn is_cloud_reparse_tag(tag: u32) -> bool {
    tag & 0xFFFF_0FFF == 0x9000_001A
}

/// Lexically normalize an absolute path: resolve `.` and `..`, unify separators.
/// (Win32 `\\?\` paths skip the OS's own normalization, so we must do it.)
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Case-insensitive (Windows) path prefix test.
pub fn path_starts_with(path: &Path, prefix: &Path) -> bool {
    let mut a = path.components();
    for pc in prefix.components() {
        match a.next() {
            Some(c) if eq_component(c.as_os_str(), pc.as_os_str()) => {}
            _ => return false,
        }
    }
    true
}

pub fn eq_component(a: &std::ffi::OsStr, b: &std::ffi::OsStr) -> bool {
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

/// Known user folders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownFolder {
    Desktop,
    Downloads,
    Documents,
    Pictures,
}

/// A fixed or removable drive.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Drive {
    /// e.g. `C:\`
    pub root: PathBuf,
    pub letter: String,
    pub label: String,
    pub fs: String,
    pub total: u64,
    pub free: u64,
    pub is_ntfs: bool,
    pub is_fixed: bool,
}

impl Drive {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.free)
    }
}

/// Convert a Windows FILETIME (100 ns ticks since 1601) to Unix milliseconds.
pub fn filetime_to_ms(ft: i64) -> Timestamp {
    if ft <= 0 {
        return 0;
    }
    (ft - 116_444_736_000_000_000) / 10_000
}

pub fn system_time_to_ms(t: std::time::SystemTime) -> Timestamp {
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_millis() as i64,
        Err(e) => -(e.duration().as_millis() as i64),
    }
}

pub fn now_ms() -> Timestamp {
    system_time_to_ms(std::time::SystemTime::now())
}
