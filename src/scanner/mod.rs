pub mod elevation;
pub mod mft;
pub mod types;
pub mod walk;

use std::path::Path;
use std::sync::mpsc;

use anyhow::Result;

use self::types::{RawFileEntry, ScanProgress};

/// The scanning strategy to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMethod {
    /// Direct MFT parsing (fast, requires admin, NTFS only)
    Mft,
    /// Parallel directory walk via jwalk (universal fallback)
    WalkDir,
    /// Auto-detect: try MFT first, fall back to WalkDir
    Auto,
}

/// Scan a path using the specified method.
pub fn scan(
    path: &Path,
    method: ScanMethod,
    progress_tx: mpsc::Sender<ScanProgress>,
) -> Result<Vec<RawFileEntry>> {
    match method {
        ScanMethod::Mft => {
            let drive_letter = extract_drive_letter(path)?;
            mft::scan_mft(drive_letter, progress_tx)
        }
        ScanMethod::WalkDir => walk::scan_walkdir(path, progress_tx),
        ScanMethod::Auto => {
            if let Some(letter) = try_extract_drive_letter(path) {
                if mft::is_mft_available(letter) {
                    return mft::scan_mft(letter, progress_tx);
                }
            }
            walk::scan_walkdir(path, progress_tx)
        }
    }
}

fn extract_drive_letter(path: &Path) -> Result<char> {
    try_extract_drive_letter(path)
        .ok_or_else(|| anyhow::anyhow!("Cannot extract drive letter from path: {:?}", path))
}

fn try_extract_drive_letter(path: &Path) -> Option<char> {
    let s = path.to_str()?;
    let mut chars = s.chars();
    let letter = chars.next()?;
    if letter.is_ascii_alphabetic() && chars.next() == Some(':') {
        Some(letter.to_ascii_uppercase())
    } else {
        None
    }
}
