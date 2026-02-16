pub mod aggregate;
pub mod arena;
pub mod extensions;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use compact_str::CompactString;

use self::arena::{FileNode, FileTree, NodeId};
use crate::scanner::types::RawFileEntry;

/// Find the common root path for all entries.
/// For drive scans (C:\), returns the drive root.
/// For subdirectory scans, returns the deepest common ancestor.
fn find_common_root(entries: &[RawFileEntry]) -> PathBuf {
    if entries.is_empty() {
        return PathBuf::from("");
    }

    // Check if this is a drive root scan (all paths start with same drive letter)
    let first_path = &entries[0].path;

    // Try to get the drive root (e.g., "C:\")
    if let Some(path_str) = first_path.to_str() {
        if path_str.len() >= 2 && path_str.chars().nth(1) == Some(':') {
            let drive_root = PathBuf::from(format!("{}:\\", path_str.chars().next().unwrap()));

            // Verify all entries start with this drive root
            let all_match = entries.iter().all(|e| {
                e.path.starts_with(&drive_root)
            });

            if all_match {
                tracing::info!("Detected drive root scan: {}", drive_root.display());
                return drive_root;
            }
        }
    }

    // Fallback: find shortest common path
    let mut root = first_path.clone();
    for entry in entries.iter().skip(1).take(100) {  // Sample first 100
        while !entry.path.starts_with(&root) {
            match root.parent() {
                Some(parent) => root = parent.to_path_buf(),
                None => return PathBuf::from(""),
            }
        }
    }

    root
}

/// Build a FileTree from a flat list of RawFileEntry (from the scanner).
pub fn build_tree(entries: &[RawFileEntry]) -> FileTree {
    if entries.is_empty() {
        return FileTree::new("(empty)");
    }

    let dir_count = entries.iter().filter(|e| e.is_dir).count();
    let file_count = entries.iter().filter(|e| !e.is_dir).count();
    tracing::info!(
        "Building tree from {} entries ({} dirs, {} files)",
        entries.len(),
        dir_count,
        file_count
    );

    // Debug: show first few entries
    tracing::debug!("First 5 entries from MFT scanner:");
    for (i, entry) in entries.iter().take(5).enumerate() {
        tracing::debug!(
            "  [{}] {} (dir={}, size={})",
            i,
            entry.path.display(),
            entry.is_dir,
            entry.size
        );
    }

    // Find the common root of all entries
    // For drive scans (C:\), this should be the drive root
    // For subdirectory scans, this should be that subdirectory
    let root_path = find_common_root(entries);

    tracing::info!("Determined root_path: {}", root_path.display());

    let root_name = root_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| root_path.to_string_lossy().to_string());

    tracing::info!("Root node name will be: '{}'", root_name);

    let mut tree = FileTree::new(&root_name);

    // Map from path → NodeId for parent lookups
    let mut path_map: HashMap<std::path::PathBuf, NodeId> = HashMap::new();
    path_map.insert(root_path.clone(), tree.root);

    // First pass: create all directory nodes
    for entry in entries.iter().filter(|e| e.is_dir) {
        if entry.path == root_path {
            continue;
        }
        ensure_node(&mut tree, &mut path_map, &entry.path, true, 0);
    }

    // Second pass: create all file nodes
    for entry in entries.iter().filter(|e| !e.is_dir) {
        let ext = entry
            .path
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();
        let ext_id = tree.intern_extension(&ext);

        let name = entry
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let parent_path = entry
            .path
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();
        let parent_id = ensure_node(&mut tree, &mut path_map, &parent_path, true, 0);

        let node = FileNode {
            name: CompactString::new(&name),
            size: entry.size,
            is_dir: false,
            extension_id: ext_id,
            parent: Some(parent_id),
            first_child: None,
            next_sibling: None,
            depth: 0, // will be set by add_child
        };

        let id = tree.add_child(parent_id, node);
        path_map.insert(entry.path.clone(), id);
    }

    // Aggregate directory sizes
    aggregate::aggregate_sizes(&mut tree);
    // Sort children by size for squarified layout
    aggregate::sort_children_by_size(&mut tree);

    // Debug: count direct children of root
    let root_child_count = tree.children(tree.root).count();
    tracing::info!(
        "Tree built: {} total nodes, {} direct children of root",
        tree.len(),
        root_child_count
    );

    // Debug: show first 10 direct children of root
    tracing::debug!("First 10 direct children of root:");
    for (i, child_id) in tree.children(tree.root).take(10).enumerate() {
        let child = tree.get(child_id);
        tracing::debug!(
            "  [{}] '{}' (dir={}, size={:.2} GB)",
            i,
            child.name,
            child.is_dir,
            child.size as f64 / 1_073_741_824.0
        );
    }

    tree
}

/// Ensure a directory node exists at the given path, creating intermediate nodes as needed.
/// Uses an iterative approach to avoid stack overflow on deep paths.
fn ensure_node(
    tree: &mut FileTree,
    path_map: &mut HashMap<std::path::PathBuf, NodeId>,
    path: &Path,
    is_dir: bool,
    size: u64,
) -> NodeId {
    // Fast path: already exists
    if let Some(&id) = path_map.get(path) {
        return id;
    }

    // Build list of missing ancestors from root to target
    let mut missing = Vec::new();
    let mut current = path.to_path_buf();

    loop {
        if path_map.contains_key(&current) {
            break;
        }
        missing.push(current.clone());

        match current.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => {
                current = parent.to_path_buf();
            }
            _ => break,
        }
    }

    // Reverse to create from root downward
    missing.reverse();

    // Create each missing ancestor
    let mut last_id = tree.root;
    for ancestor in missing {
        let parent_path = ancestor
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();

        let parent_id = path_map.get(&parent_path).copied().unwrap_or(tree.root);

        let name = ancestor
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Determine if this is the target node or an intermediate directory
        let (is_this_dir, this_size) = if ancestor == path {
            (is_dir, size)
        } else {
            (true, 0)
        };

        let node = FileNode {
            name: CompactString::new(&name),
            size: this_size,
            is_dir: is_this_dir,
            extension_id: 0,
            parent: Some(parent_id),
            first_child: None,
            next_sibling: None,
            depth: 0,
        };

        let id = tree.add_child(parent_id, node);
        path_map.insert(ancestor.clone(), id);
        last_id = id;
    }

    last_id
}
