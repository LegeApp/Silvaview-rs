use crate::tree::arena::{FileTree, NodeId};
use crate::tree::extensions::category_label;
use chrono::{DateTime, Local};
use std::fs;
use std::path::{Path, PathBuf};

/// Information to display in the tooltip when hovering over a node.
#[derive(Debug, Clone)]
pub struct TooltipInfo {
    pub name: String,
    pub full_path: String,
    pub size_display: String,
    pub category: String,
    pub is_dir: bool,
    pub child_count: Option<usize>,
}

/// Richer information shown for a selected node.
#[derive(Debug, Clone)]
pub struct SelectionInfo {
    pub name: String,
    pub full_path: String,
    pub directory_path: String,
    pub size_display: String,
    pub category: String,
    pub is_dir: bool,
    pub child_count: Option<usize>,
    pub modified_display: String,
}

/// Build tooltip info for a node.
pub fn build_tooltip(tree: &FileTree, node_id: NodeId) -> TooltipInfo {
    let node = tree.get(node_id);

    let category = if node.is_dir {
        "Directory".to_string()
    } else {
        category_label(node.category).to_string()
    };

    let child_count = if node.is_dir {
        Some(tree.children(node_id).count())
    } else {
        None
    };

    // Build full path by walking up the tree
    let full_path = build_path(tree, node_id);

    TooltipInfo {
        name: node.name.to_string(),
        full_path,
        size_display: format_size(node.size),
        category,
        is_dir: node.is_dir,
        child_count,
    }
}

pub fn build_selection_info(tree: &FileTree, node_id: NodeId) -> SelectionInfo {
    let node = tree.get(node_id);
    let full_path_buf = build_pathbuf(tree, node_id);
    let full_path = full_path_buf.to_string_lossy().to_string();
    let directory_path = if node.is_dir {
        full_path.clone()
    } else {
        full_path_buf
            .parent()
            .unwrap_or(full_path_buf.as_path())
            .to_string_lossy()
            .to_string()
    };

    let category = if node.is_dir {
        "Directory".to_string()
    } else {
        category_label(node.category).to_string()
    };

    SelectionInfo {
        name: node.name.to_string(),
        full_path,
        directory_path,
        size_display: format_size(node.size),
        category,
        is_dir: node.is_dir,
        child_count: node.is_dir.then(|| tree.children(node_id).count()),
        modified_display: format_modified_time(&full_path_buf),
    }
}

/// Format bytes into human-readable size string.
pub fn format_size(bytes: u64) -> String {
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

/// Build the full path of a node by walking up the tree.
pub fn build_path(tree: &FileTree, node_id: NodeId) -> String {
    build_pathbuf(tree, node_id).to_string_lossy().to_string()
}

pub fn build_pathbuf(tree: &FileTree, node_id: NodeId) -> PathBuf {
    let mut parts = Vec::new();
    let mut current = Some(node_id);

    while let Some(id) = current {
        let node = tree.get(id);
        if id != tree.root {
            parts.push(node.name.to_string());
        }
        current = node.parent;
    }

    parts.reverse();
    let mut path = if tree.root_path.as_os_str().is_empty() {
        let root_name = tree.get(tree.root).name.to_string();
        if root_name.is_empty() {
            PathBuf::new()
        } else {
            PathBuf::from(root_name)
        }
    } else {
        tree.root_path.clone()
    };
    for part in parts {
        path.push(part);
    }
    path
}

fn format_modified_time(path: &Path) -> String {
    match fs::metadata(path).and_then(|metadata| metadata.modified()) {
        Ok(modified) => {
            let local: DateTime<Local> = modified.into();
            local.format("%Y-%m-%d %H:%M:%S").to_string()
        }
        Err(_) => "Unavailable".to_string(),
    }
}
