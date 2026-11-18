//! Fast mode: read a volume's MFT directly (needs an elevated process). Sizes on disk come from
//! the record (resident data takes no clusters; otherwise the size rounded up to the cluster),
//! except compressed and sparse files, which are asked of Windows one by one.

use std::path::Path;
use std::time::Instant;

use ntfs_reader::{Journal, JournalOptions, Mft, NextUsn, NtfsAttributeType, ROOT_RECORD, Volume};
use rayon::prelude::*;

use super::{IndexEntry, JournalPos, VolumeIndex};
use crate::platform::{self, ATTR_COMPRESSED, ATTR_DIRECTORY};

const ATTR_SPARSE: u32 = 0x200;
const UNKNOWN: u64 = u64::MAX;

fn ms(t: time::OffsetDateTime) -> i64 {
    (t.unix_timestamp_nanos() / 1_000_000) as i64
}

/// `\\.\C:` for `C:\`.
pub fn device_path(root: &Path) -> Option<String> {
    let s = root.to_string_lossy();
    let letter = s.chars().next().filter(|c| c.is_ascii_alphabetic())?;
    Some(format!(r"\\.\{}:", letter.to_ascii_uppercase()))
}

pub fn build(root: &Path) -> Result<VolumeIndex, String> {
    let start = Instant::now();
    let dev = device_path(root).ok_or("not a drive root")?;
    let volume = Volume::new(&dev).map_err(|e| e.to_string())?;
    let cluster = volume.cluster_size().max(512);
    // Remember where the journal is now, so the next rescan reads only what changed after.
    let journal = Volume::new(&dev)
        .ok()
        .and_then(|v| {
            Journal::new(
                v,
                JournalOptions {
                    next_usn: NextUsn::Next,
                    ..JournalOptions::default()
                },
            )
            .ok()
        })
        .map(|j| JournalPos {
            journal_id: j.journal_id(),
            usn: j.next_usn(),
        });
    let mft = Mft::new_compact(volume).map_err(|e| e.to_string())?;
    let mut entries: Vec<IndexEntry> = Vec::with_capacity(mft.record_count() as usize);

    let root_rec = mft.record(ROOT_RECORD).ok_or("no root directory record")?;
    let root_id = root_rec.reference();
    entries.push(IndexEntry {
        id: root_id,
        parent: root_id,
        name: Default::default(),
        is_dir: true,
        size: 0,
        allocated: 0,
        attrs: ATTR_DIRECTORY,
        links: 1,
        created: 0,
        modified: 0,
        accessed: 0,
    });

    for file in mft.files() {
        let id = file.reference();
        let is_dir = file.is_directory();
        let si = file.standard_information();
        let attrs = si.as_ref().map(|s| s.file_attributes()).unwrap_or(0)
            | if is_dir { ATTR_DIRECTORY } else { 0 };
        let (created, modified, accessed) = si
            .as_ref()
            .map(|s| (ms(s.created()), ms(s.modified()), ms(s.accessed())))
            .unwrap_or((0, 0, 0));
        let (size, resident) = if is_dir {
            (0, true)
        } else {
            file.attributes()
                .find(|a| a.attribute_type() == Some(NtfsAttributeType::Data) && a.name().is_none())
                .map(|a| (a.value_size().unwrap_or(0), a.is_resident()))
                .unwrap_or((0, true))
        };
        let allocated = if is_dir || resident {
            0
        } else if attrs & (ATTR_COMPRESSED | ATTR_SPARSE) != 0 {
            UNKNOWN
        } else {
            size.div_ceil(cluster) * cluster
        };
        let names: Vec<_> = file.hard_links().collect();
        let links = names.len().max(1) as u32;
        for n in names {
            entries.push(IndexEntry {
                id,
                parent: n.parent_reference(),
                name: n.to_os_string(),
                is_dir,
                size,
                allocated,
                attrs,
                links,
                created,
                modified,
                accessed,
            });
        }
    }

    let mut idx = VolumeIndex {
        root: root.to_path_buf(),
        root_id,
        serial: platform::volume_serial(root).unwrap_or(0),
        entries,
        journal,
        built: platform::now_ms(),
        build_ms: 0,
    };
    // Compressed and sparse files: ask Windows for the real size on disk.
    let by_id = idx.by_id();
    let unknown: Vec<usize> = idx
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.allocated == UNKNOWN)
        .map(|(i, _)| i)
        .collect();
    let sizes: Vec<(usize, u64)> = unknown
        .par_iter()
        .map(|&i| {
            let a = idx
                .path_of(i, &by_id)
                .and_then(|p| platform::stat(&p).ok())
                .map(|s| s.allocated)
                .unwrap_or(idx.entries[i].size.div_ceil(cluster) * cluster);
            (i, a)
        })
        .collect();
    for (i, a) in sizes {
        idx.entries[i].allocated = a;
    }
    idx.build_ms = start.elapsed().as_millis() as u64;
    Ok(idx)
}

/// Read journal records since the index was built and apply them. Returns how many files changed.
pub fn update(idx: &mut VolumeIndex) -> Result<usize, String> {
    let Some(pos) = idx.journal else {
        return Err("no journal position".into());
    };
    let dev = device_path(&idx.root).ok_or("not a drive root")?;
    let volume = Volume::new(&dev).map_err(|e| e.to_string())?;
    let mut journal = Journal::new(
        volume,
        JournalOptions {
            next_usn: NextUsn::Custom(ntfs_reader::JournalPosition {
                journal_id: pos.journal_id,
                usn: pos.usn,
            }),
            ..JournalOptions::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let mut changes = Vec::new();
    loop {
        let r = journal.read().map_err(|e| e.to_string())?;
        for rec in r.records {
            changes.push(super::usn::Change {
                id: rec.file_id.as_reference().unwrap_or(0),
                parent: rec.parent_id.as_reference().unwrap_or(0),
                name: rec.name.clone(),
                deleted: rec.reason.contains(ntfs_reader::Reason::FILE_DELETE),
                attrs: rec.file_attributes,
            });
        }
        if r.caught_up {
            break;
        }
    }
    let n = super::usn::apply(idx, &changes, &|p| platform::stat(p).ok());
    idx.journal = Some(JournalPos {
        journal_id: journal.journal_id(),
        usn: journal.next_usn(),
    });
    Ok(n)
}
