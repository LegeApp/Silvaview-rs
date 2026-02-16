use std::path::Path;
use std::sync::mpsc;

use anyhow::Result;
use jwalk::WalkDir;

use super::types::{RawFileEntry, ScanProgress};

/// Scan a directory tree using jwalk (parallel filesystem walker).
/// This is the fallback scanner that works on any filesystem without admin privileges.
pub fn scan_walkdir(
    root: &Path,
    progress_tx: mpsc::Sender<ScanProgress>,
) -> Result<Vec<RawFileEntry>> {
    let _ = progress_tx.send(ScanProgress::Started {
        root: root.to_path_buf(),
    });

    let start = std::time::Instant::now();
    let mut entries = Vec::with_capacity(100_000);
    let mut files_scanned: u64 = 0;
    let mut dirs_scanned: u64 = 0;
    let mut total_bytes: u64 = 0;

    for entry in WalkDir::new(root).skip_hidden(false).sort(false) {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                let _ = progress_tx.send(ScanProgress::Error {
                    path: root.to_path_buf(),
                    message: e.to_string(),
                });
                continue;
            }
        };

        let path = entry.path();
        let is_dir = entry.file_type().is_dir();
        let size = if is_dir {
            0
        } else {
            entry.metadata().map(|m| m.len()).unwrap_or(0)
        };

        let parent = path.parent().map(|p| p.to_path_buf());

        entries.push(RawFileEntry {
            path,
            size,
            is_dir,
            parent,
            mft_record: None,
        });

        if is_dir {
            dirs_scanned += 1;
        } else {
            files_scanned += 1;
            total_bytes += size;
        }

        // Send progress every 10,000 entries
        if (files_scanned + dirs_scanned) % 10_000 == 0 {
            let _ = progress_tx.send(ScanProgress::Progress {
                files_scanned,
                dirs_scanned,
                total_bytes,
            });
        }
    }

    let elapsed = start.elapsed();
    let _ = progress_tx.send(ScanProgress::Completed {
        total_files: files_scanned,
        total_dirs: dirs_scanned,
        total_bytes,
        elapsed_ms: elapsed.as_millis() as u64,
    });

    Ok(entries)
}
