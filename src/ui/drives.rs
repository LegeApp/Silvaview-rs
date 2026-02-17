use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DriveEntry {
    pub label: String,
    pub path: PathBuf,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[cfg(windows)]
pub fn enumerate_drives() -> Vec<DriveEntry> {
    enumerate_drives_windows()
}

#[cfg(target_os = "linux")]
pub fn enumerate_drives() -> Vec<DriveEntry> {
    enumerate_drives_linux()
}

#[cfg(all(not(windows), not(target_os = "linux")))]
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
            path: PathBuf::from("C:\\"),
            total_bytes: 0,
            available_bytes: 0,
        });
    }

    entries.sort_by(|a, b| a.label.cmp(&b.label));
    entries
}

#[cfg(target_os = "linux")]
fn enumerate_drives_linux() -> Vec<DriveEntry> {
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

    if entries.is_empty() {
        entries.push(DriveEntry {
            label: "/".to_string(),
            path: PathBuf::from("/"),
            total_bytes: 0,
            available_bytes: 0,
        });
    }

    entries.sort_by(|a, b| a.label.cmp(&b.label));
    entries
}
