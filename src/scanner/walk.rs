use std::path::Path;
use std::sync::mpsc;

use anyhow::Result;
use jwalk::WalkDirGeneric;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt;

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
    let root_metadata = std::fs::metadata(root)?;
    let root_is_dir = root_metadata.is_dir();
    let root_file_size = (!root_is_dir).then_some(root_metadata.len()).unwrap_or(0);

    // Root scans on Linux otherwise cross into pseudo filesystems and mounted
    // volumes. Keep subdirectory scans unrestricted so explicitly selected
    // mounts still work, and do not apply this on macOS where the data volume
    // is commonly mounted separately from `/`.
    #[cfg(target_os = "linux")]
    let filter_linux_root = is_linux_root_scan(root);
    #[cfg(not(target_os = "linux"))]
    let filter_linux_root = false;
    #[cfg(target_os = "linux")]
    let root_device = filter_linux_root.then_some(root_metadata.dev());

    for entry in WalkDirGeneric::<((), Option<u64>)>::new(root)
        .follow_links(false)
        .skip_hidden(false)
        .sort(false)
        .process_read_dir(move |_, _, _, children| {
            for child in children.iter_mut() {
                let Ok(entry) = child else {
                    continue;
                };

                if entry.path_is_symlink() {
                    entry.read_children_path = None;
                }

                #[cfg(target_os = "linux")]
                if filter_linux_root && is_linux_virtual_path(&entry.path()) {
                    entry.read_children_path = None;
                    continue;
                }

                let is_dir = entry.file_type().is_dir();
                if !is_dir || filter_linux_root {
                    let metadata = match entry.metadata() {
                        Ok(metadata) => metadata,
                        Err(_) => {
                            entry.read_children_path = None;
                            continue;
                        }
                    };

                    #[cfg(target_os = "linux")]
                    if entry.depth() > 0 && root_device.is_some_and(|dev| metadata.dev() != dev) {
                        entry.read_children_path = None;
                        continue;
                    }

                    if !is_dir {
                        entry.client_state = Some(metadata.len());
                    }
                }
            }
        })
    {
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
        let is_root = entry.depth() == 0;
        let is_dir = if is_root {
            root_is_dir
        } else {
            entry.file_type().is_dir()
        };
        let size = if is_root {
            root_file_size
        } else {
            entry.client_state.unwrap_or(0)
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

#[cfg(target_os = "linux")]
fn is_linux_virtual_path(path: &Path) -> bool {
    path.starts_with("/proc") || path.starts_with("/sys") || path.starts_with("/dev")
}

#[cfg(target_os = "linux")]
fn is_linux_root_scan(path: &Path) -> bool {
    fs::canonicalize(path).is_ok_and(|path| path == Path::new("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "silvaview-walk-{name}-{}-{unique}",
            std::process::id()
        ))
    }

    #[test]
    fn reports_directories_and_file_sizes() {
        let root = temp_path("tree");
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("data.bin"), [1_u8, 2, 3, 4]).unwrap();

        let (tx, _rx) = mpsc::channel();
        let entries = scan_walkdir(&root, tx).unwrap();

        assert!(entries
            .iter()
            .any(|entry| entry.path == nested && entry.is_dir));
        assert!(entries
            .iter()
            .any(|entry| entry.path.ends_with("data.bin") && entry.size == 4 && !entry.is_dir));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn classifies_a_file_scan_root_as_a_file() {
        let root = temp_path("file");
        std::fs::write(&root, [1_u8, 2, 3]).unwrap();

        let (tx, _rx) = mpsc::channel();
        let entries = scan_walkdir(&root, tx).unwrap();

        assert_eq!(entries.len(), 1);
        assert!(!entries[0].is_dir);
        assert_eq!(entries[0].size, 3);

        std::fs::remove_file(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_child_symlinks() {
        use std::os::unix::fs::symlink;

        let root = temp_path("symlink-tree");
        let target = temp_path("symlink-target");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("outside.bin"), [1_u8]).unwrap();
        let link = root.join("link");
        symlink(&target, &link).unwrap();

        let (tx, _rx) = mpsc::channel();
        let entries = scan_walkdir(&root, tx).unwrap();

        assert!(entries
            .iter()
            .any(|entry| entry.path == link && !entry.is_dir));
        assert!(!entries
            .iter()
            .any(|entry| entry.path.starts_with(&link) && entry.path != link));

        std::fs::remove_file(link).unwrap();
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(target).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn treats_a_symlinked_directory_root_as_a_directory() {
        use std::os::unix::fs::symlink;

        let target = temp_path("root-link-target");
        let link = temp_path("root-link");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("inside.bin"), [1_u8]).unwrap();
        symlink(&target, &link).unwrap();

        let (tx, _rx) = mpsc::channel();
        let entries = scan_walkdir(&link, tx).unwrap();

        assert!(entries
            .iter()
            .any(|entry| entry.path == link && entry.is_dir));
        assert!(entries
            .iter()
            .any(|entry| entry.path == link.join("inside.bin")));

        std::fs::remove_file(link).unwrap();
        std::fs::remove_dir_all(target).unwrap();
    }
}
