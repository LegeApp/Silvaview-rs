/// Debug CLI tool for testing the filesystem scanner without GUI
/// Usage: cargo run --bin debug-scan -- C:\

use std::path::PathBuf;
use std::sync::mpsc;

use anyhow::Result;
use sequoiaview_rs::scanner::{self, types::ScanProgress};
use sequoiaview_rs::tree;
use sequoiaview_rs::tree::extensions::{categorize_extension, FileCategory};

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sequoiaview_rs=debug".parse().unwrap()),
        )
        .init();

    // Parse command line
    let scan_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\"));

    println!("═══════════════════════════════════════════════════════");
    println!("  SequoiaView-rs Debug Scanner");
    println!("═══════════════════════════════════════════════════════");
    println!();
    println!("Scan path: {}", scan_path.display());

    // Check privileges
    #[cfg(windows)]
    {
        use sequoiaview_rs::scanner::elevation;
        if elevation::is_elevated() {
            println!("Privileges: Administrator ✓ (MFT scanning enabled)");
        } else {
            println!("Privileges: User (jwalk fallback)");
        }
    }

    println!();
    println!("Starting scan...");
    println!();

    let (progress_tx, progress_rx) = mpsc::channel();

    // Scan in background thread
    let scan_path_clone = scan_path.clone();
    let scan_thread = std::thread::spawn(move || {
        scanner::scan(&scan_path_clone, scanner::ScanMethod::Auto, progress_tx)
    });

    // Monitor progress
    let mut last_update = std::time::Instant::now();
    while let Ok(progress) = progress_rx.recv() {
        match progress {
            ScanProgress::Started { .. } => {
                print!("Scanning");
                std::io::Write::flush(&mut std::io::stdout()).ok();
            }
            ScanProgress::Progress {
                files_scanned,
                dirs_scanned,
                total_bytes,
            } => {
                if last_update.elapsed().as_millis() > 500 {
                    print!("\rScanning: {} files, {} dirs, {:.2} GB",
                        files_scanned,
                        dirs_scanned,
                        total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
                    );
                    std::io::Write::flush(&mut std::io::stdout()).ok();
                    last_update = std::time::Instant::now();
                }
            }
            ScanProgress::Completed {
                total_files,
                total_dirs,
                total_bytes,
                elapsed_ms,
            } => {
                println!();
                println!();
                println!("Scan complete!");
                println!("  Files:     {}", total_files);
                println!("  Dirs:      {}", total_dirs);
                println!("  Total:     {}", total_files + total_dirs);
                println!("  Size:      {:.2} GB", total_bytes as f64 / (1024.0 * 1024.0 * 1024.0));
                println!("  Duration:  {:.2}s", elapsed_ms as f64 / 1000.0);
                println!();
                break;
            }
            ScanProgress::Error { path, message } => {
                eprintln!("\nError scanning {:?}: {}", path, message);
            }
        }
    }

    // Wait for scan to finish
    let entries = scan_thread.join().unwrap()?;

    println!("Building file tree...");
    let tree = tree::build_tree(&entries);
    println!("Tree built: {} nodes", tree.len());
    println!();

    // Analyze by file type
    println!("═══════════════════════════════════════════════════════");
    println!("  File Type Analysis");
    println!("═══════════════════════════════════════════════════════");
    println!();

    let mut category_stats: std::collections::HashMap<FileCategory, (u64, u64)> =
        std::collections::HashMap::new();

    // Walk tree
    for node_id in 0..tree.len() {
        let node = tree.get(tree::arena::NodeId(node_id as u32));
        if !node.is_dir && node.size > 0 {
            let ext = if node.extension_id > 0 {
                tree.extensions
                    .get(node.extension_id as usize)
                    .map(|s| s.as_str())
                    .unwrap_or("")
            } else {
                ""
            };
            let category = categorize_extension(ext);
            let entry = category_stats.entry(category).or_insert((0, 0));
            entry.0 += 1; // count
            entry.1 += node.size; // bytes
        }
    }

    // Sort by size
    let mut stats: Vec<_> = category_stats.into_iter().collect();
    stats.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));

    let total_size: u64 = stats.iter().map(|(_, (_, size))| size).sum();

    println!("{:<15} {:>12} {:>12} {:>8}", "Category", "Files", "Size", "Percent");
    println!("{:-<50}", "");

    for (category, (count, size)) in stats {
        let percent = if total_size > 0 {
            (size as f64 / total_size as f64) * 100.0
        } else {
            0.0
        };

        println!(
            "{:<15} {:>12} {:>12} {:>7.1}%",
            format!("{:?}", category),
            format_number(count),
            format_size(size),
            percent
        );
    }

    println!("{:-<50}", "");
    println!("{:<15} {:>12} {:>12} {:>8}",
        "TOTAL",
        "",
        format_size(total_size),
        "100.0%"
    );

    println!();
    println!("═══════════════════════════════════════════════════════");
    println!("  Largest Files");
    println!("═══════════════════════════════════════════════════════");
    println!();

    // Find largest files
    let mut files: Vec<_> = (0..tree.len())
        .map(|i| tree.get(tree::arena::NodeId(i as u32)))
        .filter(|node| !node.is_dir)
        .collect();

    files.sort_by(|a, b| b.size.cmp(&a.size));

    for (i, node) in files.iter().take(20).enumerate() {
        println!("{:2}. {} - {}",
            i + 1,
            format_size(node.size),
            node.name
        );
    }

    println!();
    println!("Done!");

    Ok(())
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, c);
    }
    result
}
