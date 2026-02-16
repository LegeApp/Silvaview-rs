use std::path::PathBuf;

/// Raw file entry collected during scanning, before tree construction.
#[derive(Debug, Clone)]
pub struct RawFileEntry {
    /// Full path to the file or directory
    pub path: PathBuf,
    /// File size in bytes (0 for directories)
    pub size: u64,
    /// Whether this entry is a directory
    pub is_dir: bool,
    /// Parent directory path
    pub parent: Option<PathBuf>,
    /// MFT record number. Only populated by the MFT scanner and used
    /// internally to resolve $ATTRIBUTE_LIST attributes that live in
    /// extension records.
    pub mft_record: Option<u64>,
}

/// Progress updates emitted during scanning.
#[derive(Debug, Clone)]
pub enum ScanProgress {
    /// Starting scan of a drive/path
    Started { root: PathBuf },
    /// Periodic progress update
    Progress {
        files_scanned: u64,
        dirs_scanned: u64,
        total_bytes: u64,
    },
    /// Scan completed
    Completed {
        total_files: u64,
        total_dirs: u64,
        total_bytes: u64,
        elapsed_ms: u64,
    },
    /// Error encountered (non-fatal)
    Error { path: PathBuf, message: String },
}
