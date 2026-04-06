pub mod elevation;
pub mod mft;
pub mod types;
pub mod walk;

use std::path::Path;
use std::sync::mpsc;

use anyhow::Result;

use self::types::ScanProgress;
use crate::tree::arena::FileTree;

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
) -> Result<FileTree> {
    match method {
        ScanMethod::Mft => {
            anyhow::ensure!(
                is_drive_root(path),
                "MFT scanning requires a drive root path like D:\\"
            );
            let drive_letter = extract_drive_letter(path)?;
            mft::scan_mft(drive_letter, progress_tx)
        }
        ScanMethod::WalkDir => {
            let entries = walk::scan_walkdir(path, progress_tx)?;
            Ok(crate::tree::build_tree(&entries))
        }
        ScanMethod::Auto => {
            if is_drive_root(path) {
                if let Some(letter) = try_extract_drive_letter(path) {
                    if mft::is_mft_available(letter) {
                        match mft::scan_mft(letter, progress_tx.clone()) {
                            Ok(tree) => return Ok(tree),
                            Err(err) => {
                                tracing::warn!(
                                    "MFT scan failed for {}:, falling back to walk scan: {}",
                                    letter,
                                    err
                                );
                            }
                        }
                    }
                }
            }
            let entries = walk::scan_walkdir(path, progress_tx)?;
            Ok(crate::tree::build_tree(&entries))
        }
    }
}

fn is_drive_root(path: &Path) -> bool {
    let Some(s) = path.to_str() else {
        return false;
    };

    let mut chars = s.chars();
    let Some(letter) = chars.next() else {
        return false;
    };

    if !letter.is_ascii_alphabetic() || chars.next() != Some(':') {
        return false;
    }

    matches!(chars.next(), Some('\\') | Some('/')) && chars.next().is_none()
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
