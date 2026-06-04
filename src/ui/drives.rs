use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DriveEntry {
    pub label: String,
    pub path: PathBuf,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

pub fn default_scan_path() -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from("C:\\")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/")
    }
}

#[cfg(windows)]
pub fn enumerate_drives() -> Vec<DriveEntry> {
    enumerate_drives_windows()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn enumerate_drives() -> Vec<DriveEntry> {
    enumerate_drives_unix()
}

#[cfg(all(not(windows), not(target_os = "linux"), not(target_os = "macos")))]
pub fn enumerate_drives() -> Vec<DriveEntry> {
    Vec::new()
}

#[cfg(windows)]
fn enumerate_drives_windows() -> Vec<DriveEntry> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut entries: Vec<DriveEntry> = disks
        .iter()
        .map(|d| {
            let mount = d.mount_point().to_path_buf();
            let label = mount.to_string_lossy().to_string();
            DriveEntry {
                label,
                path: mount,
                total_bytes: d.total_space(),
                available_bytes: d.available_space(),
            }
        })
        .collect();

    if entries.is_empty() {
        entries.push(DriveEntry {
            label: "C:\\".to_string(),
            path: default_scan_path(),
            total_bytes: 0,
            available_bytes: 0,
        });
    }

    entries.sort_by(|a, b| a.label.cmp(&b.label));
    entries
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn enumerate_drives_unix() -> Vec<DriveEntry> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut entries: Vec<DriveEntry> = disks
        .iter()
        .filter_map(|d| {
            let mount = d.mount_point().to_path_buf();
            if !mount.is_absolute() {
                return None;
            }
            let label = mount.to_string_lossy().to_string();
            Some(DriveEntry {
                label,
                path: mount,
                total_bytes: d.total_space(),
                available_bytes: d.available_space(),
            })
        })
        .collect();

    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries.dedup_by(|a, b| a.path == b.path);

    let default_path = default_scan_path();
    if !entries.iter().any(|entry| entry.path == default_path) {
        entries.push(DriveEntry {
            label: "/".to_string(),
            path: default_path,
            total_bytes: 0,
            available_bytes: 0,
        });
    }

    entries.sort_by(|a, b| a.label.cmp(&b.label));
    entries
}
