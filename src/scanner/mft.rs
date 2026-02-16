use std::collections::HashMap;
use std::mem;
use std::path::PathBuf;
use std::sync::mpsc;

use anyhow::{Context, Result};

use super::types::{RawFileEntry, ScanProgress};

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, SetFilePointerEx, FILE_BEGIN, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
#[cfg(windows)]
use windows::Win32::System::IO::DeviceIoControl;
#[cfg(windows)]
use windows::Win32::System::Ioctl::FSCTL_GET_NTFS_VOLUME_DATA;
#[cfg(windows)]
use windows::core::PCWSTR;

/// NTFS_VOLUME_DATA_BUFFER structure
#[cfg(windows)]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct NtfsVolumeData {
    volume_serial_number: i64,
    number_sectors: i64,
    total_clusters: i64,
    free_clusters: i64,
    total_reserved: i64,
    bytes_per_sector: u32,
    bytes_per_cluster: u32,
    bytes_per_file_record_segment: u32,
    clusters_per_file_record_segment: u32,
    mft_valid_data_length: i64,
    mft_start_lcn: i64,
    mft2_start_lcn: i64,
    mft_zone_start: i64,
    mft_zone_end: i64,
}

/// A contiguous extent of MFT data on disk
#[derive(Debug, Clone)]
struct MftExtent {
    /// Byte offset on the volume where this extent starts
    disk_offset: i64,
    /// Number of bytes in this extent
    length: u64,
}

const ATTR_TYPE_FILE_NAME: u32 = 0x30;
const ATTR_TYPE_DATA: u32 = 0x80;
const ATTR_TYPE_ATTRIBUTE_LIST: u32 = 0x20;
const ATTR_TYPE_END: u32 = 0xFFFFFFFF;

/// Filename namespace constants
const FILENAME_NAMESPACE_POSIX: u8 = 0;
const FILENAME_NAMESPACE_WIN32: u8 = 1;
const FILENAME_NAMESPACE_WIN32_AND_DOS: u8 = 3;

/// Scan an NTFS volume by directly parsing the Master File Table.
#[cfg(windows)]
pub fn scan_mft(
    drive_letter: char,
    progress_tx: mpsc::Sender<ScanProgress>,
) -> Result<Vec<RawFileEntry>> {
    use windows::Win32::Foundation::GENERIC_READ;

    let volume_path = format!("\\\\.\\{}:", drive_letter);
    let root_path = PathBuf::from(format!("{}:\\", drive_letter));

    let _ = progress_tx.send(ScanProgress::Started {
        root: root_path.clone(),
    });

    tracing::info!("Opening NTFS volume: {}", volume_path);

    let wide_path: Vec<u16> = volume_path.encode_utf16().chain(std::iter::once(0)).collect();

    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide_path.as_ptr()),
            GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )?
    };

    if handle == INVALID_HANDLE_VALUE {
        anyhow::bail!("Failed to open volume. Administrator privileges required.");
    }

    let result = scan_mft_with_handle(handle, root_path, progress_tx);

    unsafe {
        let _ = CloseHandle(handle);
    }

    result
}

#[cfg(windows)]
fn scan_mft_with_handle(
    handle: HANDLE,
    root_path: PathBuf,
    progress_tx: mpsc::Sender<ScanProgress>,
) -> Result<Vec<RawFileEntry>> {
    let start = std::time::Instant::now();

    // Get NTFS volume data to find MFT location
    let mut volume_data: NtfsVolumeData = unsafe { mem::zeroed() };
    let mut bytes_returned: u32 = 0;

    unsafe {
        DeviceIoControl(
            handle,
            FSCTL_GET_NTFS_VOLUME_DATA,
            None,
            0,
            Some(&mut volume_data as *mut _ as *mut _),
            mem::size_of::<NtfsVolumeData>() as u32,
            Some(&mut bytes_returned),
            None,
        )
    }
    .context("Failed to get NTFS volume data. Not an NTFS volume?")?;

    let bytes_per_record = volume_data.bytes_per_file_record_segment as usize;
    let bytes_per_cluster = volume_data.bytes_per_cluster as u64;
    let mft_start_offset = volume_data.mft_start_lcn * bytes_per_cluster as i64;

    tracing::info!(
        "MFT start: cluster {}, offset {}, record size: {} bytes, cluster size: {} bytes",
        volume_data.mft_start_lcn,
        mft_start_offset,
        bytes_per_record,
        bytes_per_cluster
    );

    let estimated_records = (volume_data.mft_valid_data_length / bytes_per_record as i64) as u64;
    tracing::info!(
        "Estimated MFT records: {} (MFT valid data length: {} bytes)",
        estimated_records,
        volume_data.mft_valid_data_length
    );

    // ========================================================================
    // PHASE 1: Read MFT record 0 ($MFT itself) to get the full extent list.
    //
    // The MFT is a file and can be fragmented. We must parse its data runs
    // to know where all the MFT fragments are on disk.
    // ========================================================================
    let mft_extents = read_mft_extents(handle, mft_start_offset, bytes_per_record, bytes_per_cluster)?;

    tracing::info!(
        "MFT has {} extents covering {} bytes",
        mft_extents.len(),
        mft_extents.iter().map(|e| e.length).sum::<u64>()
    );

    // ========================================================================
    // PHASE 2: Read all MFT records – single-pass incremental path building
    // ========================================================================

    // Map from MFT record number → resolved path (seeded with root = record 5)
    let mut record_paths: HashMap<u64, PathBuf> = HashMap::new();
    record_paths.insert(5, root_path.clone());

    let mut entries: Vec<RawFileEntry> = Vec::new();
    // Track entries that need $DATA size resolved from extension records
    let mut needs_size_resolution: HashMap<u64, usize> = HashMap::new();

    let mut files_scanned: u64 = 0;
    let mut dirs_scanned: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut records_processed: u64 = 0;
    let mut records_skipped: u64 = 0;

    // Storage for ATTRIBUTE_LIST extension record resolution
    let mut base_to_extensions: HashMap<u64, Vec<Vec<u8>>> = HashMap::new();

    // Deferred records: records whose parent path isn't known yet
    // (parent record number, record number, name, size, is_dir, has_attribute_list)
    let mut deferred: Vec<(u64, u64, String, u64, bool, bool)> = Vec::new();

    let mut global_record_number: u64 = 0;
    let mft_valid_bytes = volume_data.mft_valid_data_length as u64;
    let mut mft_bytes_read_total: u64 = 0;

    const BATCH_SIZE: usize = 1024;
    let batch_bytes = bytes_per_record * BATCH_SIZE;
    let mut buffer = vec![0u8; batch_bytes];

    for extent in &mft_extents {
        let mut extent_bytes_read: u64 = 0;

        while extent_bytes_read < extent.length {
            if mft_bytes_read_total >= mft_valid_bytes {
                break;
            }

            let disk_pos = extent.disk_offset + extent_bytes_read as i64;
            let remaining_in_extent = extent.length - extent_bytes_read;
            let remaining_valid = mft_valid_bytes - mft_bytes_read_total;
            let to_read = (batch_bytes as u64)
                .min(remaining_in_extent)
                .min(remaining_valid) as usize;

            if to_read < bytes_per_record {
                break;
            }

            unsafe {
                SetFilePointerEx(handle, disk_pos, None, FILE_BEGIN)?;
            }

            let mut bytes_read: u32 = 0;
            let read_result = unsafe {
                ReadFile(
                    handle,
                    Some(&mut buffer[..to_read]),
                    Some(&mut bytes_read),
                    None,
                )
            };

            if read_result.is_err() || bytes_read == 0 {
                tracing::warn!(
                    "Read failed at disk offset {}, extent_offset {}, skipping rest of extent",
                    disk_pos,
                    extent_bytes_read
                );
                break;
            }

            let records_in_batch = (bytes_read as usize) / bytes_per_record;

            for i in 0..records_in_batch {
                let record_data = &buffer[i * bytes_per_record..(i + 1) * bytes_per_record];
                records_processed += 1;
                let record_number = global_record_number;
                global_record_number += 1;

                // Quick header checks
                if record_data.len() < 42 || &record_data[0..4] != b"FILE" {
                    records_skipped += 1;
                    continue;
                }

                let base_record_ref = read_u48_le(record_data, 0x20);

                if base_record_ref != 0 {
                    // EXTENSION RECORD → store for later $ATTRIBUTE_LIST resolution
                    base_to_extensions
                        .entry(base_record_ref)
                        .or_insert_with(Vec::new)
                        .push(record_data.to_vec());
                    records_skipped += 1;
                    continue;
                }

                // BASE RECORD
                let mut record_copy = record_data.to_vec();
                apply_fixups(&mut record_copy);
                let record = &record_copy;

                let flags = read_u16_le(record, 22);
                let in_use = (flags & 0x01) != 0;
                if !in_use {
                    records_skipped += 1;
                    continue;
                }

                let is_directory = (flags & 0x02) != 0;

                let (best_name, any_name, parent_record, data_size, has_attribute_list, file_name_size) =
                    parse_mft_attributes(record, is_directory);

                // Use best_name, falling back to any_name (which includes DOS names)
                let name = best_name.or(any_name);

                if let (Some(name), Some(parent)) = (name, parent_record) {
                    // Skip system metafiles
                    if name.starts_with('$') && record_number < 24 {
                        records_skipped += 1;
                        continue;
                    }

                    let final_size = if is_directory {
                        0
                    } else {
                        data_size.unwrap_or(file_name_size)
                    };

                    // Build path incrementally from parent
                    if let Some(parent_path) = record_paths.get(&parent).cloned() {
                        let full_path = parent_path.join(&name);

                        // Register directory paths so children can find them
                        if is_directory {
                            record_paths.insert(record_number, full_path.clone());
                        }

                        let entry = RawFileEntry {
                            path: full_path,
                            size: final_size,
                            is_dir: is_directory,
                            parent: Some(parent_path),
                            mft_record: Some(record_number),
                        };

                        let entry_idx = entries.len();
                        entries.push(entry);

                        // Track for $ATTRIBUTE_LIST resolution
                        if !is_directory && data_size.is_none() && has_attribute_list {
                            needs_size_resolution.insert(record_number, entry_idx);
                        }
                    } else {
                        // Parent not yet seen — defer for later
                        deferred.push((parent, record_number, name, final_size, is_directory, !is_directory && data_size.is_none() && has_attribute_list));
                    }

                    if is_directory {
                        dirs_scanned += 1;
                    } else {
                        files_scanned += 1;
                        total_bytes += final_size;
                    }
                } else {
                    records_skipped += 1;
                }
            }

            let actual_read = (records_in_batch * bytes_per_record) as u64;
            extent_bytes_read += actual_read;
            mft_bytes_read_total += actual_read;

            // Progress updates
            if records_processed % 50_000 == 0 {
                let _ = progress_tx.send(ScanProgress::Progress {
                    files_scanned,
                    dirs_scanned,
                    total_bytes,
                });
            }
        }

        if mft_bytes_read_total >= mft_valid_bytes {
            break;
        }
    }

    // ==================== Resolve deferred records ====================
    // Records whose parents weren't seen yet during the main scan pass.
    // Iterate multiple times until no more progress is made.
    let mut prev_deferred_count = deferred.len() + 1;
    while !deferred.is_empty() && deferred.len() < prev_deferred_count {
        prev_deferred_count = deferred.len();
        let mut still_deferred = Vec::new();

        for (parent, record_number, name, final_size, is_directory, needs_attr_resolve) in deferred {
            if let Some(parent_path) = record_paths.get(&parent).cloned() {
                let full_path = parent_path.join(&name);

                if is_directory {
                    record_paths.insert(record_number, full_path.clone());
                }

                let entry = RawFileEntry {
                    path: full_path,
                    size: final_size,
                    is_dir: is_directory,
                    parent: Some(parent_path),
                    mft_record: Some(record_number),
                };

                let entry_idx = entries.len();
                entries.push(entry);

                if needs_attr_resolve {
                    needs_size_resolution.insert(record_number, entry_idx);
                }
            } else {
                still_deferred.push((parent, record_number, name, final_size, is_directory, needs_attr_resolve));
            }
        }

        deferred = still_deferred;
    }

    if !deferred.is_empty() {
        tracing::warn!(
            "{} records could not be resolved (orphaned parent references), attaching to root",
            deferred.len()
        );
        for (_, record_number, name, final_size, is_directory, needs_attr_resolve) in deferred {
            let full_path = root_path.join(&name);

            if is_directory {
                record_paths.insert(record_number, full_path.clone());
            }

            let entry = RawFileEntry {
                path: full_path,
                size: final_size,
                is_dir: is_directory,
                parent: Some(root_path.clone()),
                mft_record: Some(record_number),
            };

            let entry_idx = entries.len();
            entries.push(entry);

            if needs_attr_resolve {
                needs_size_resolution.insert(record_number, entry_idx);
            }
        }
    }

    // ==================== $ATTRIBUTE_LIST extension resolution ====================
    let mut resolved_count = 0u64;
    let mut recovered_bytes: u64 = 0;

    for (base_ref, extensions) in &base_to_extensions {
        if let Some(&idx) = needs_size_resolution.get(base_ref) {
            // Look for $DATA in any of the extension records
            let mut data_size_from_ext: Option<u64> = None;
            for ext_data in extensions {
                let mut ext_copy = ext_data.clone();
                apply_fixups(&mut ext_copy);
                if let Some(size) = parse_data_size_from_record(&ext_copy) {
                    data_size_from_ext = Some(size);
                    break;
                }
            }

            if let Some(new_size) = data_size_from_ext {
                let entry = &mut entries[idx];
                let old_size = entry.size;
                if new_size > old_size {
                    recovered_bytes += new_size - old_size;
                    entry.size = new_size;
                    total_bytes = total_bytes - old_size + new_size;
                    resolved_count += 1;
                }
            }
        }
    }

    tracing::info!(
        "Resolved $ATTRIBUTE_LIST for {} files → recovered {:.2} GB",
        resolved_count,
        recovered_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    );

    let elapsed = start.elapsed();
    let total_gb = total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

    tracing::info!(
        "MFT scan complete: {} files, {} dirs, {:.2} GB in {:.2}s",
        files_scanned,
        dirs_scanned,
        total_gb,
        elapsed.as_secs_f64()
    );

    tracing::info!(
        "MFT statistics: processed {} records, skipped {}, yielded {} entries, read {} bytes of MFT",
        records_processed,
        records_skipped,
        files_scanned + dirs_scanned,
        mft_bytes_read_total
    );

    let _ = progress_tx.send(ScanProgress::Completed {
        total_files: files_scanned,
        total_dirs: dirs_scanned,
        total_bytes,
        elapsed_ms: elapsed.as_millis() as u64,
    });

    Ok(entries)
}

/// Read MFT record 0 and parse data runs to get the full MFT extent list.
///
/// The $MFT file's own MFT record tells us where all MFT fragments are on disk.
#[cfg(windows)]
fn read_mft_extents(
    handle: HANDLE,
    mft_start_offset: i64,
    bytes_per_record: usize,
    bytes_per_cluster: u64,
) -> Result<Vec<MftExtent>> {
    // Read MFT record 0 (the $MFT file itself)
    let mut record0 = vec![0u8; bytes_per_record];

    unsafe {
        SetFilePointerEx(handle, mft_start_offset, None, FILE_BEGIN)?;
    }

    let mut bytes_read: u32 = 0;
    unsafe {
        ReadFile(
            handle,
            Some(&mut record0),
            Some(&mut bytes_read),
            None,
        )
    }
    .context("Failed to read MFT record 0")?;

    if bytes_read < bytes_per_record as u32 {
        anyhow::bail!("Short read on MFT record 0: got {} bytes", bytes_read);
    }

    if &record0[0..4] != b"FILE" {
        anyhow::bail!("MFT record 0 has invalid signature");
    }

    // Apply fixups
    apply_fixups(&mut record0);

    // Find the unnamed $DATA attribute in record 0
    let first_attr_offset = read_u16_le(&record0, 20) as usize;
    let mut offset = first_attr_offset;

    while offset + 16 <= record0.len() {
        let attr_type = read_u32_le(&record0, offset);

        if attr_type == ATTR_TYPE_END {
            break;
        }

        let attr_length = read_u32_le(&record0, offset + 4) as usize;
        if attr_length == 0 || attr_length < 16 || offset + attr_length > record0.len() {
            break;
        }

        let non_resident = record0[offset + 8];
        let name_length = record0[offset + 9] as usize;

        // We want the unnamed (name_length == 0) $DATA attribute, which must be non-resident
        if attr_type == ATTR_TYPE_DATA && non_resident != 0 && name_length == 0 {
            // Parse data runs from the non-resident $DATA attribute
            let data_runs_offset_in_attr = read_u16_le(&record0, offset + 32) as usize;
            let data_runs_start = offset + data_runs_offset_in_attr;

            let extents = parse_data_runs(&record0[data_runs_start..], bytes_per_cluster)?;
            tracing::info!("Parsed {} data runs from $MFT record 0", extents.len());
            return Ok(extents);
        }

        offset += attr_length;
    }

    // Fallback: if we couldn't parse data runs, treat MFT as a single extent
    // This is the old behavior - it will only work if the MFT isn't fragmented
    tracing::warn!("Could not find $DATA attribute in $MFT record 0, assuming contiguous MFT");
    Ok(vec![MftExtent {
        disk_offset: mft_start_offset,
        length: u64::MAX, // will be clamped by mft_valid_data_length during reading
    }])
}

/// Parse NTFS data runs (also called "run list") from a non-resident attribute.
///
/// Data runs encode a series of (length, offset) pairs in a compact variable-length format.
/// Each run starts with a header byte: low nibble = bytes for length, high nibble = bytes for offset.
/// A header byte of 0x00 terminates the list.
fn parse_data_runs(data: &[u8], bytes_per_cluster: u64) -> Result<Vec<MftExtent>> {
    let mut extents = Vec::new();
    let mut pos = 0;
    let mut current_lcn: i64 = 0; // Running LCN (offsets are relative to previous)

    while pos < data.len() {
        let header = data[pos];
        if header == 0 {
            break; // End of data runs
        }
        pos += 1;

        let length_bytes = (header & 0x0F) as usize;
        let offset_bytes = ((header >> 4) & 0x0F) as usize;

        if length_bytes == 0 || pos + length_bytes + offset_bytes > data.len() {
            break;
        }

        // Read run length (unsigned)
        let mut run_length: u64 = 0;
        for i in 0..length_bytes {
            run_length |= (data[pos + i] as u64) << (i * 8);
        }
        pos += length_bytes;

        // Read run offset (signed, relative to previous LCN)
        if offset_bytes == 0 {
            // Sparse run - no physical location, skip
            // (This shouldn't happen for $MFT but handle it gracefully)
            continue;
        }

        let mut run_offset: i64 = 0;
        for i in 0..offset_bytes {
            run_offset |= (data[pos + i] as i64) << (i * 8);
        }
        // Sign-extend if the high bit is set
        if offset_bytes > 0 && (data[pos + offset_bytes - 1] & 0x80) != 0 {
            for i in offset_bytes..8 {
                run_offset |= 0xFFi64 << (i * 8);
            }
        }
        pos += offset_bytes;

        current_lcn += run_offset;

        extents.push(MftExtent {
            disk_offset: current_lcn * bytes_per_cluster as i64,
            length: run_length * bytes_per_cluster,
        });
    }

    if extents.is_empty() {
        anyhow::bail!("No data runs found in $MFT $DATA attribute");
    }

    Ok(extents)
}

/// Apply Update Sequence Array fixups to an MFT record.
/// NTFS stores fixup values at sector boundaries to detect corruption.
fn apply_fixups(record: &mut [u8]) -> bool {
    if record.len() < 512 {
        return false;
    }

    let usa_offset = read_u16_le(record, 0x04) as usize;
    let usa_count = read_u16_le(record, 0x06) as usize;

    if usa_count == 0 || usa_offset + usa_count * 2 > record.len() {
        return false;
    }

    let update_seq = read_u16_le(record, usa_offset);

    let bytes_per_sector = 512;
    for i in 1..usa_count {
        let sector_offset = i * bytes_per_sector - 2;
        if sector_offset + 2 > record.len() {
            break;
        }

        let sector_signature = read_u16_le(record, sector_offset);
        if sector_signature != update_seq {
            continue;
        }

        let fixup_value = read_u16_le(record, usa_offset + i * 2);
        record[sector_offset] = (fixup_value & 0xFF) as u8;
        record[sector_offset + 1] = (fixup_value >> 8) as u8;
    }

    true
}

// ─── ATTRIBUTE_LIST helper functions ───────────────────────────────────────

/// Parse MFT attributes to extract file information, detecting $ATTRIBUTE_LIST presence.
/// Returns: (best_name, any_name, parent_record, data_size, has_attribute_list, file_name_size)
///
/// `best_name` excludes DOS-only names (namespace 2) for display purposes.
/// `any_name` accepts ALL namespaces including DOS, so DOS-only directories are never dropped.
fn parse_mft_attributes(
    record: &[u8],
    is_directory: bool,
) -> (Option<String>, Option<String>, Option<u64>, Option<u64>, bool, u64) {
    let mut best_file_name: Option<String> = None;
    let mut best_namespace: u8 = 255;
    let mut any_name: Option<String> = None;
    let mut any_name_parent: Option<u64> = None;
    let mut parent_record: Option<u64> = None;
    let mut file_name_size: u64 = 0;
    let mut data_size: Option<u64> = None;
    let mut has_attribute_list: bool = false;

    let first_attr_offset = read_u16_le(record, 20) as usize;
    let mut offset = first_attr_offset;

    while offset + 16 <= record.len() {
        let attr_type = read_u32_le(record, offset);
        if attr_type == ATTR_TYPE_END {
            break;
        }

        let attr_length = read_u32_le(record, offset + 4) as usize;
        if attr_length == 0 || attr_length < 16 || offset + attr_length > record.len() {
            break;
        }

        let non_resident = record[offset + 8];
        let attr_name_length = record[offset + 9] as usize;

        if attr_type == ATTR_TYPE_ATTRIBUTE_LIST {
            has_attribute_list = true;
        } else if attr_type == ATTR_TYPE_FILE_NAME && non_resident == 0 {
            let value_offset_in_attr = read_u16_le(record, offset + 20) as usize;
            let value_offset = offset + value_offset_in_attr;

            if value_offset + 0x42 <= record.len() {
                let parent_ref = read_u48_le(record, value_offset);
                let name_length = record[value_offset + 0x40] as usize;
                let namespace = record[value_offset + 0x41];

                let name_offset = value_offset + 0x42;
                let name_bytes_len = name_length * 2;

                if name_offset + name_bytes_len <= record.len() {
                    let name_u16: Vec<u16> = (0..name_length)
                        .map(|i| read_u16_le(record, name_offset + i * 2))
                        .collect();
                    let name = String::from_utf16_lossy(&name_u16);

                    // Always track any_name (first name seen, or update if we get a non-DOS name)
                    if any_name.is_none() {
                        any_name = Some(name.clone());
                        any_name_parent = Some(parent_ref);
                    }

                    let priority = match namespace {
                        FILENAME_NAMESPACE_WIN32_AND_DOS => 0,
                        FILENAME_NAMESPACE_WIN32 => 1,
                        FILENAME_NAMESPACE_POSIX => 2,
                        _ => 255, // DOS-only or unknown — excluded from best_name
                    };

                    if priority < best_namespace {
                        best_namespace = priority;
                        best_file_name = Some(name.clone());
                        parent_record = Some(parent_ref);

                        // Also update any_name to prefer the better name
                        any_name = Some(name);
                        any_name_parent = Some(parent_ref);

                        if !is_directory && value_offset + 0x38 <= record.len() {
                            file_name_size = read_u64_le(record, value_offset + 0x30);
                        }
                    }
                }
            }
        } else if attr_type == ATTR_TYPE_DATA && attr_name_length == 0 && !is_directory {
            if non_resident != 0 {
                if offset + 56 <= record.len() {
                    data_size = Some(read_u64_le(record, offset + 48));
                }
            } else if offset + 20 <= record.len() {
                data_size = Some(read_u32_le(record, offset + 16) as u64);
            }
        }

        offset += attr_length;
    }

    // If best_name is None but any_name exists, use any_name's parent too
    if parent_record.is_none() {
        parent_record = any_name_parent;
    }

    (best_file_name, any_name, parent_record, data_size, has_attribute_list, file_name_size)
}

/// Parse $DATA size from a record (used for extension records in Pass 2).
fn parse_data_size_from_record(record: &[u8]) -> Option<u64> {
    if record.len() < 42 {
        return None;
    }

    let flags = read_u16_le(record, 22);
    let is_directory = (flags & 0x02) != 0;
    if is_directory {
        return None;
    }

    let first_attr_offset = read_u16_le(record, 20) as usize;
    let mut offset = first_attr_offset;

    while offset + 16 <= record.len() {
        let attr_type = read_u32_le(record, offset);
        if attr_type == ATTR_TYPE_END {
            break;
        }

        let attr_length = read_u32_le(record, offset + 4) as usize;
        if attr_length == 0 || attr_length < 16 || offset + attr_length > record.len() {
            break;
        }

        let non_resident = record[offset + 8];
        let attr_name_length = record[offset + 9] as usize;

        if attr_type == ATTR_TYPE_DATA && attr_name_length == 0 {
            if non_resident != 0 {
                if offset + 56 <= record.len() {
                    return Some(read_u64_le(record, offset + 48));
                }
            } else if offset + 20 <= record.len() {
                return Some(read_u32_le(record, offset + 16) as u64);
            }
        }

        offset += attr_length;
    }
    None
}

// ─── Helper readers ─────────────────────────────────────────────────────────

/// Read 6 bytes (48-bit) as a little-endian u64, used for MFT record references.
/// NTFS record references are 8 bytes: low 6 = record number, high 2 = sequence.
#[inline]
fn read_u48_le(data: &[u8], offset: usize) -> u64 {
    read_u64_le(data, offset) & 0x0000_FFFF_FFFF_FFFF
}

#[inline]
fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

#[inline]
fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

#[inline]
fn read_u64_le(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
        data[offset + 4],
        data[offset + 5],
        data[offset + 6],
        data[offset + 7],
    ])
}

// ─── Non-Windows stubs ──────────────────────────────────────────────────────

#[cfg(not(windows))]
pub fn scan_mft(
    drive_letter: char,
    progress_tx: mpsc::Sender<ScanProgress>,
) -> Result<Vec<RawFileEntry>> {
    let root = PathBuf::from(format!("{}:\\", drive_letter));
    tracing::warn!("MFT scanning only available on Windows, falling back to jwalk");
    super::walk::scan_walkdir(&root, progress_tx)
}

#[cfg(windows)]
pub fn is_mft_available(drive_letter: char) -> bool {
    use windows::Win32::Foundation::GENERIC_READ;

    let volume_path = format!("\\\\.\\{}:", drive_letter);
    let wide_path: Vec<u16> = volume_path.encode_utf16().chain(std::iter::once(0)).collect();

    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide_path.as_ptr()),
            GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
    };

    match handle {
        Ok(h) if h != INVALID_HANDLE_VALUE => {
            unsafe {
                let _ = CloseHandle(h);
            }
            true
        }
        _ => false,
    }
}

#[cfg(not(windows))]
pub fn is_mft_available(_drive_letter: char) -> bool {
    false
}
