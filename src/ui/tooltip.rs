use crate::tree::arena::{FileTree, NodeId};
use crate::tree::extensions::categorize_extension;

/// Information to display in the tooltip when hovering over a node.
#[derive(Debug)]
pub struct TooltipInfo {
    pub name: String,
    pub full_path: String,
    pub size_display: String,
    pub category: String,
    pub is_dir: bool,
    pub child_count: Option<usize>,
}

/// Build tooltip info for a node.
pub fn build_tooltip(tree: &FileTree, node_id: NodeId) -> TooltipInfo {
    let node = tree.get(node_id);

    let ext = if node.extension_id > 0 {
        tree.extensions
            .get(node.extension_id as usize)
            .map(|s| s.as_str())
            .unwrap_or("")
    } else {
        ""
    };

    let category = if node.is_dir {
        "Directory".to_string()
    } else {
        format!("{:?}", categorize_extension(ext))
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
    let mut parts = Vec::new();
    let mut current = Some(node_id);

    while let Some(id) = current {
        let node = tree.get(id);
        parts.push(node.name.to_string());
        current = node.parent;
    }

    parts.reverse();
    parts.join("\\")
}
