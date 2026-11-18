//! Win32 implementation. Every handle is opened with `FILE_FLAG_OPEN_REPARSE_POINT` so links are
//! never followed, and with attribute-only access so cloud placeholders are never hydrated.

use std::ffi::{OsString, c_void};
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_MORE_DATA, ERROR_NO_MORE_FILES, GetLastError, HANDLE,
    INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, DeleteFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, FileIdExtdDirectoryInfo, FileIdExtdDirectoryRestartInfo, GetDiskFreeSpaceExW,
    GetDriveTypeW, GetFileInformationByHandleEx, GetLogicalDriveStringsW, GetVolumeInformationW,
    OPEN_EXISTING, RemoveDirectoryW, SetFileAttributesW,
};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::RestartManager::{
    CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources,
    RmStartSession,
};
use windows_sys::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Pictures,
    SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHEmptyRecycleBinW,
    SHGetKnownFolderPath, SHQUERYRBINFO, SHQueryRecycleBinW,
};
use windows_sys::core::PCWSTR;

use super::{ATTR_DIRECTORY, ATTR_READONLY, Drive, KnownFolder, RawEntry, filetime_to_ms};
use crate::types::ProcessHolder;

const FILE_BASIC_INFO_CLASS: i32 = 0;
const FILE_STANDARD_INFO_CLASS: i32 = 1;
const FILE_ATTRIBUTE_TAG_INFO_CLASS: i32 = 9;
const FILE_ID_INFO_CLASS: i32 = 18;

#[repr(C)]
#[derive(Default)]
struct FileBasicInfo {
    creation_time: i64,
    last_access_time: i64,
    last_write_time: i64,
    change_time: i64,
    file_attributes: u32,
}

#[repr(C)]
#[derive(Default)]
struct FileStandardInfo {
    allocation_size: i64,
    end_of_file: i64,
    number_of_links: u32,
    delete_pending: u8,
    directory: u8,
}

#[repr(C)]
#[derive(Default)]
struct FileAttributeTagInfo {
    file_attributes: u32,
    reparse_tag: u32,
}

#[repr(C)]
#[derive(Default)]
struct FileIdInfo {
    volume_serial_number: u64,
    file_id: [u8; 16],
}

/// Null-terminated wide string with the `\\?\` long-path prefix for absolute paths.
pub fn wide_long(path: &Path) -> Vec<u16> {
    let s: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .map(|c| if c == b'/' as u16 { b'\\' as u16 } else { c })
        .collect();
    let bs = b'\\' as u16;
    let mut out = Vec::with_capacity(s.len() + 9);
    let already = s.len() >= 4
        && s[0] == bs
        && s[1] == bs
        && (s[2] == b'?' as u16 || s[2] == b'.' as u16)
        && s[3] == bs;
    if already {
        out.extend_from_slice(&s);
    } else if s.len() >= 2 && s[0] == bs && s[1] == bs {
        // UNC: \\server\share -> \\?\UNC\server\share
        out.extend("\\\\?\\UNC\\".encode_utf16());
        out.extend_from_slice(&s[2..]);
    } else if s.len() >= 3 && s[1] == b':' as u16 && s[2] == bs {
        out.extend("\\\\?\\".encode_utf16());
        out.extend_from_slice(&s);
    } else {
        out.extend_from_slice(&s);
    }
    out.push(0);
    out
}

/// Null-terminated wide string without any prefix.
pub fn wide_plain(s: &std::ffi::OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle was returned by CreateFileW and is closed exactly once.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn open_no_follow(path: &Path, access: u32) -> io::Result<Handle> {
    let w = wide_long(path);
    // SAFETY: `w` is a valid null-terminated wide string; other pointers are null as allowed.
    let h = unsafe {
        CreateFileW(
            w.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if h == INVALID_HANDLE_VALUE || h.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(Handle(h))
}

fn info<T: Default>(h: &Handle, class: i32) -> io::Result<T> {
    let mut v = T::default();
    // SAFETY: `v` is a properly sized and aligned out-buffer for this information class.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            h.0,
            class,
            &mut v as *mut T as *mut c_void,
            std::mem::size_of::<T>() as u32,
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(v)
    }
}

/// Stat one path without following reparse points or hydrating cloud files.
pub fn stat(path: &Path) -> io::Result<RawEntry> {
    let h = open_no_follow(path, FILE_READ_ATTRIBUTES)?;
    let basic: FileBasicInfo = info(&h, FILE_BASIC_INFO_CLASS)?;
    let std_info: FileStandardInfo = info(&h, FILE_STANDARD_INFO_CLASS)?;
    let tag: FileAttributeTagInfo = info(&h, FILE_ATTRIBUTE_TAG_INFO_CLASS).unwrap_or_default();
    let id: FileIdInfo = info(&h, FILE_ID_INFO_CLASS).unwrap_or_default();
    Ok(RawEntry {
        name: path
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_default(),
        is_dir: basic.file_attributes & ATTR_DIRECTORY != 0,
        attrs: basic.file_attributes,
        reparse_tag: tag.reparse_tag,
        logical: std_info.end_of_file.max(0) as u64,
        allocated: std_info.allocation_size.max(0) as u64,
        file_id: u128::from_le_bytes(id.file_id),
        links: std_info.number_of_links,
        created: filetime_to_ms(basic.creation_time),
        modified: filetime_to_ms(basic.last_write_time),
        accessed: filetime_to_ms(basic.last_access_time),
    })
}

/// Hard-link count and 128-bit file id of a path.
pub fn link_count(path: &Path) -> io::Result<(u32, u128)> {
    let h = open_no_follow(path, FILE_READ_ATTRIBUTES)?;
    let std_info: FileStandardInfo = info(&h, FILE_STANDARD_INFO_CLASS)?;
    let id: FileIdInfo = info(&h, FILE_ID_INFO_CLASS).unwrap_or_default();
    Ok((std_info.number_of_links, u128::from_le_bytes(id.file_id)))
}

/// Serial number of the volume holding `path`.
pub fn volume_serial(path: &Path) -> Option<u64> {
    let h = open_no_follow(path, FILE_READ_ATTRIBUTES).ok()?;
    let id: FileIdInfo = info(&h, FILE_ID_INFO_CLASS).ok()?;
    Some(id.volume_serial_number)
}

#[inline]
unsafe fn rd_u32(p: *const u8, off: usize) -> u32 {
    // SAFETY: caller guarantees `p + off .. p + off + 4` is inside the buffer.
    unsafe { std::ptr::read_unaligned(p.add(off) as *const u32) }
}

#[inline]
unsafe fn rd_i64(p: *const u8, off: usize) -> i64 {
    // SAFETY: caller guarantees `p + off .. p + off + 8` is inside the buffer.
    unsafe { std::ptr::read_unaligned(p.add(off) as *const i64) }
}

/// List a directory in bulk with `FileIdExtdDirectoryInfo`: one handle per directory gives
/// size on disk, 128-bit file ids, attributes, reparse tags and times without opening each file.
pub fn list_dir(path: &Path) -> io::Result<Vec<RawEntry>> {
    let h = open_no_follow(path, FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES)?;
    // 64 KiB, 8-byte aligned (the records contain LARGE_INTEGERs).
    let mut buf = vec![0u64; 8 * 1024];
    let bytes = buf.len() * 8;
    let mut out = Vec::new();
    let mut class = FileIdExtdDirectoryRestartInfo;
    loop {
        // SAFETY: `buf` is a writable, aligned buffer of `bytes` bytes.
        let ok = unsafe {
            GetFileInformationByHandleEx(h.0, class, buf.as_mut_ptr() as *mut c_void, bytes as u32)
        };
        class = FileIdExtdDirectoryInfo;
        if ok == 0 {
            // SAFETY: trivially safe FFI call.
            let err = unsafe { GetLastError() };
            if err == ERROR_NO_MORE_FILES {
                break;
            }
            // FAT/exFAT and some network file systems don't support this class.
            if out.is_empty() && matches!(err, 1 | 50 | 87 | 124) {
                return list_dir_fallback(path);
            }
            return Err(io::Error::from_raw_os_error(err as i32));
        }
        let base = buf.as_ptr() as *const u8;
        let mut off = 0usize;
        loop {
            if off + 88 > bytes {
                break;
            }
            // SAFETY: each record lies inside `buf`; the kernel wrote `NextEntryOffset` chains.
            unsafe {
                let p = base.add(off);
                let next = rd_u32(p, 0) as usize;
                let name_len = rd_u32(p, 60) as usize;
                let name_ptr = p.add(88) as *const u16;
                let n = name_len / 2;
                let mut name_buf = vec![0u16; n];
                std::ptr::copy_nonoverlapping(
                    name_ptr as *const u8,
                    name_buf.as_mut_ptr() as *mut u8,
                    name_len,
                );
                let is_dot = (n == 1 && name_buf[0] == b'.' as u16)
                    || (n == 2 && name_buf[0] == b'.' as u16 && name_buf[1] == b'.' as u16);
                if !is_dot {
                    let attrs = rd_u32(p, 56);
                    let mut id = [0u8; 16];
                    std::ptr::copy_nonoverlapping(p.add(72), id.as_mut_ptr(), 16);
                    out.push(RawEntry {
                        name: OsString::from_wide(&name_buf),
                        is_dir: attrs & ATTR_DIRECTORY != 0,
                        attrs,
                        reparse_tag: rd_u32(p, 68),
                        logical: rd_i64(p, 40).max(0) as u64,
                        allocated: rd_i64(p, 48).max(0) as u64,
                        file_id: u128::from_le_bytes(id),
                        links: 0,
                        created: filetime_to_ms(rd_i64(p, 8)),
                        accessed: filetime_to_ms(rd_i64(p, 16)),
                        modified: filetime_to_ms(rd_i64(p, 24)),
                    });
                }
                if next == 0 {
                    break;
                }
                off += next;
            }
        }
    }
    Ok(out)
}

fn list_dir_fallback(path: &Path) -> io::Result<Vec<RawEntry>> {
    use std::os::windows::fs::MetadataExt;
    let mut out = Vec::new();
    for e in std::fs::read_dir(path)? {
        let e = e?;
        // DirEntry::metadata does not follow links on Windows.
        let Ok(m) = e.metadata() else { continue };
        let attrs = m.file_attributes();
        let len = m.file_size();
        out.push(RawEntry {
            name: e.file_name(),
            is_dir: attrs & ATTR_DIRECTORY != 0,
            attrs,
            reparse_tag: if attrs & super::ATTR_REPARSE_POINT != 0 {
                stat(&e.path())
                    .map(|s| s.reparse_tag)
                    .unwrap_or(super::IO_REPARSE_TAG_SYMLINK)
            } else {
                0
            },
            logical: len,
            allocated: len.div_ceil(4096) * 4096,
            file_id: 0,
            links: 0,
            created: filetime_to_ms(m.creation_time() as i64),
            modified: filetime_to_ms(m.last_write_time() as i64),
            accessed: filetime_to_ms(m.last_access_time() as i64),
        });
    }
    Ok(out)
}

fn clear_readonly(path: &Path) {
    if let Ok(s) = stat(path) {
        if s.attrs & ATTR_READONLY != 0 {
            let w = wide_long(path);
            // SAFETY: valid wide string.
            unsafe {
                SetFileAttributesW(w.as_ptr(), s.attrs & !ATTR_READONLY);
            }
        }
    }
}

/// Delete a file, or a file symlink itself (never its target).
pub fn remove_file(path: &Path) -> io::Result<()> {
    let w = wide_long(path);
    // SAFETY: valid wide string.
    if unsafe { DeleteFileW(w.as_ptr()) } != 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32) {
        clear_readonly(path);
        // SAFETY: valid wide string.
        if unsafe { DeleteFileW(w.as_ptr()) } != 0 {
            return Ok(());
        }
        return Err(io::Error::last_os_error());
    }
    Err(err)
}

/// Remove an empty directory, or a junction / directory symlink itself (never its target).
pub fn remove_dir(path: &Path) -> io::Result<()> {
    let w = wide_long(path);
    // SAFETY: valid wide string.
    if unsafe { RemoveDirectoryW(w.as_ptr()) } != 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32) {
        clear_readonly(path);
        // SAFETY: valid wide string.
        if unsafe { RemoveDirectoryW(w.as_ptr()) } != 0 {
            return Ok(());
        }
        return Err(io::Error::last_os_error());
    }
    Err(err)
}

pub fn known_folder(kind: KnownFolder) -> Option<PathBuf> {
    let id = match kind {
        KnownFolder::Desktop => FOLDERID_Desktop,
        KnownFolder::Downloads => FOLDERID_Downloads,
        KnownFolder::Documents => FOLDERID_Documents,
        KnownFolder::Pictures => FOLDERID_Pictures,
    };
    let mut p: *mut u16 = null_mut();
    // SAFETY: `id` is a valid GUID; `p` receives a CoTaskMemAlloc'd string we free below.
    let hr = unsafe { SHGetKnownFolderPath(&id, 0, null_mut(), &mut p) };
    if hr != 0 || p.is_null() {
        return None;
    }
    // SAFETY: `p` is a valid null-terminated wide string until freed.
    let s = unsafe {
        let mut len = 0;
        while *p.add(len) != 0 {
            len += 1;
        }
        OsString::from_wide(std::slice::from_raw_parts(p, len))
    };
    // SAFETY: allocated by the shell with CoTaskMemAlloc.
    unsafe { CoTaskMemFree(p as *const c_void) };
    Some(PathBuf::from(s))
}

/// Size and item count of one drive's Recycle Bin (e.g. `C:\`).
pub fn recycle_bin_query(root: &Path) -> io::Result<(u64, u64)> {
    let w = wide_plain(root.as_os_str());
    let mut info = SHQUERYRBINFO {
        cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
        i64Size: 0,
        i64NumItems: 0,
    };
    // SAFETY: valid wide string and out-struct with cbSize set.
    let hr = unsafe { SHQueryRecycleBinW(w.as_ptr(), &mut info) };
    if hr != 0 {
        return Err(io::Error::other(format!(
            "SHQueryRecycleBinW failed: 0x{hr:08X}"
        )));
    }
    Ok((info.i64Size.max(0) as u64, info.i64NumItems.max(0) as u64))
}

/// Empty one drive's Recycle Bin. Only ever called for an explicit user selection.
pub fn recycle_bin_empty(root: &Path) -> io::Result<()> {
    let w = wide_plain(root.as_os_str());
    // SAFETY: valid wide string; no owner window.
    let hr = unsafe {
        SHEmptyRecycleBinW(
            null_mut(),
            w.as_ptr(),
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
        )
    };
    // S_OK, or E_UNEXPECTED when the bin is already empty.
    if hr == 0 || hr == 0x8000_FFFFu32 as i32 {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "SHEmptyRecycleBinW failed: 0x{hr:08X}"
        )))
    }
}

/// Processes holding any of `paths` open, via the Windows Restart Manager.
pub fn processes_holding(paths: &[PathBuf]) -> io::Result<Vec<ProcessHolder>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut session = 0u32;
    let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    // SAFETY: out-params are valid.
    let rc = unsafe { RmStartSession(&mut session, 0, key.as_mut_ptr()) };
    if rc != 0 {
        return Err(io::Error::from_raw_os_error(rc as i32));
    }
    struct Session(u32);
    impl Drop for Session {
        fn drop(&mut self) {
            // SAFETY: ends the session we started.
            unsafe {
                RmEndSession(self.0);
            }
        }
    }
    let _guard = Session(session);
    for chunk in paths.chunks(256) {
        let wides: Vec<Vec<u16>> = chunk.iter().map(|p| wide_plain(p.as_os_str())).collect();
        let ptrs: Vec<PCWSTR> = wides.iter().map(|w| w.as_ptr()).collect();
        // SAFETY: `ptrs` points at null-terminated wide strings that outlive the call.
        let rc = unsafe {
            RmRegisterResources(
                session,
                ptrs.len() as u32,
                ptrs.as_ptr(),
                0,
                null(),
                0,
                null(),
            )
        };
        if rc != 0 {
            return Err(io::Error::from_raw_os_error(rc as i32));
        }
    }
    let mut list: Vec<RM_PROCESS_INFO> = Vec::new();
    for _ in 0..5 {
        let mut needed = 0u32;
        let mut count = list.len() as u32;
        let mut reasons = 0u32;
        // SAFETY: `list` has room for `count` entries.
        let rc = unsafe {
            RmGetList(
                session,
                &mut needed,
                &mut count,
                if list.is_empty() {
                    null_mut()
                } else {
                    list.as_mut_ptr()
                },
                &mut reasons,
            )
        };
        if rc == 0 {
            list.truncate(count as usize);
            break;
        }
        if rc == ERROR_MORE_DATA {
            list = vec![RM_PROCESS_INFO::default(); needed as usize];
            continue;
        }
        return Err(io::Error::from_raw_os_error(rc as i32));
    }
    Ok(list
        .iter()
        .map(|p| {
            let n = p
                .strAppName
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(p.strAppName.len());
            ProcessHolder {
                pid: p.Process.dwProcessId,
                name: String::from_utf16_lossy(&p.strAppName[..n]),
            }
        })
        .collect())
}

/// Fixed and removable drives with free space.
pub fn list_drives() -> Vec<Drive> {
    let mut buf = [0u16; 512];
    // SAFETY: buffer is valid for its length.
    let n = unsafe { GetLogicalDriveStringsW(buf.len() as u32, buf.as_mut_ptr()) } as usize;
    let mut out = Vec::new();
    for root in buf[..n.min(buf.len())]
        .split(|&c| c == 0)
        .filter(|s| !s.is_empty())
    {
        let mut w = root.to_vec();
        w.push(0);
        // SAFETY: valid wide string.
        let kind = unsafe { GetDriveTypeW(w.as_ptr()) };
        // 2 = removable, 3 = fixed.
        if kind != 2 && kind != 3 {
            continue;
        }
        let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
        // SAFETY: valid wide string and out-params.
        if unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, &mut total, &mut free) } == 0 {
            continue;
        }
        let mut label = [0u16; 261];
        let mut fs = [0u16; 261];
        let (mut serial, mut maxc, mut flags) = (0u32, 0u32, 0u32);
        // SAFETY: valid buffers.
        unsafe {
            GetVolumeInformationW(
                w.as_ptr(),
                label.as_mut_ptr(),
                label.len() as u32,
                &mut serial,
                &mut maxc,
                &mut flags,
                fs.as_mut_ptr(),
                fs.len() as u32,
            );
        }
        let s = |b: &[u16]| {
            let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
            String::from_utf16_lossy(&b[..n])
        };
        let root_s = String::from_utf16_lossy(root);
        let fs_name = s(&fs);
        out.push(Drive {
            letter: root_s.trim_end_matches('\\').to_string(),
            root: PathBuf::from(&root_s),
            label: s(&label),
            is_ntfs: fs_name.eq_ignore_ascii_case("NTFS"),
            fs: fs_name,
            total,
            free,
            is_fixed: kind == 3,
        });
    }
    out
}

/// Is this process running elevated (as administrator)?
pub fn is_elevated() -> bool {
    use windows_sys::Win32::Security::{
        GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    let mut token: HANDLE = null_mut();
    // SAFETY: valid out-pointer; the pseudo handle from GetCurrentProcess needs no closing.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return false;
    }
    let _guard = Handle(token);
    let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut len = 0u32;
    // SAFETY: the buffer is a TOKEN_ELEVATION of the size we pass.
    let ok = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut TOKEN_ELEVATION as *mut c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
    };
    ok != 0 && elevation.TokenIsElevated != 0
}

/// Display names (and versions) of installed apps, from the Uninstall registry keys.
pub fn installed_apps() -> Vec<(String, Option<String>)> {
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, RRF_RT_REG_SZ,
        RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW,
    };
    let path: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\0"
        .encode_utf16()
        .collect();
    let mut out = Vec::new();
    let read = |key: HKEY, sub: &[u16], name: &str| -> Option<String> {
        let value: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut buf = [0u16; 512];
        let mut size = (buf.len() * 2) as u32;
        // SAFETY: valid key, wide strings and buffer of `size` bytes.
        let rc = unsafe {
            RegGetValueW(
                key,
                sub.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                null_mut(),
                buf.as_mut_ptr() as *mut c_void,
                &mut size,
            )
        };
        if rc != 0 {
            return None;
        }
        let n = (size as usize / 2).saturating_sub(1).min(buf.len());
        let s = String::from_utf16_lossy(&buf[..n]);
        (!s.trim().is_empty()).then_some(s)
    };
    for (root, flags) in [
        (HKEY_CURRENT_USER, 0),
        (HKEY_LOCAL_MACHINE, 0),
        (HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY),
    ] {
        let mut key: HKEY = null_mut();
        // SAFETY: valid root, null-terminated path, out-param.
        if unsafe { RegOpenKeyExW(root, path.as_ptr(), 0, KEY_READ | flags, &mut key) } != 0 {
            continue;
        }
        for i in 0..4096u32 {
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            // SAFETY: buffer of `len` wide chars.
            let rc = unsafe {
                RegEnumKeyExW(
                    key,
                    i,
                    name.as_mut_ptr(),
                    &mut len,
                    null(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                )
            };
            if rc != 0 {
                break;
            }
            let mut sub: Vec<u16> = name[..len as usize].to_vec();
            sub.push(0);
            if let Some(display) = read(key, &sub, "DisplayName") {
                out.push((display, read(key, &sub, "DisplayVersion")));
            }
        }
        // SAFETY: opened above.
        unsafe { RegCloseKey(key) };
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_path_prefixing() {
        let w = |p: &str| {
            String::from_utf16_lossy(&wide_long(Path::new(p))[..])
                .trim_end_matches('\0')
                .to_string()
        };
        assert_eq!(w(r"C:\Users\ana"), r"\\?\C:\Users\ana");
        assert_eq!(w(r"\\server\share\x"), r"\\?\UNC\server\share\x");
        assert_eq!(w(r"\\?\C:\x"), r"\\?\C:\x");
        assert_eq!(w("C:/a/b"), r"\\?\C:\a\b");
    }

    #[test]
    fn dir_info_layout_matches_windows_headers() {
        use windows_sys::Win32::Storage::FileSystem::FILE_ID_EXTD_DIR_INFO;
        assert_eq!(std::mem::offset_of!(FILE_ID_EXTD_DIR_INFO, FileName), 88);
        assert_eq!(std::mem::offset_of!(FILE_ID_EXTD_DIR_INFO, FileId), 72);
        assert_eq!(
            std::mem::offset_of!(FILE_ID_EXTD_DIR_INFO, AllocationSize),
            48
        );
        assert_eq!(
            std::mem::offset_of!(FILE_ID_EXTD_DIR_INFO, ReparsePointTag),
            68
        );
    }
}
