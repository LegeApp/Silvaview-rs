use compact_str::CompactString;
use std::path::PathBuf;

use crate::tree::extensions::{category_index, FileCategory, CATEGORY_COUNT};

/// Index into the arena `Vec<FileNode>`. Uses u32 to save memory (supports up to ~4 billion nodes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

impl NodeId {
    pub const NONE: u32 = u32::MAX;

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// A single node in the file tree, stored in a flat arena.
/// Uses sibling-list representation: each node has `first_child` and `next_sibling`.
#[derive(Debug, Clone)]
pub struct FileNode {
    /// File or directory name (not full path)
    pub name: CompactString,
    /// Size in bytes. For files: actual size. For dirs: aggregated sum of children.
    pub size: u64,
    /// Whether this node is a directory
    pub is_dir: bool,
    /// Index into the global extension table (0 = no extension / directory)
    pub extension_id: u16,
    /// File type classification used for coloring and aggregation.
    pub category: FileCategory,
    /// Parent node index (None for root)
    pub parent: Option<NodeId>,
    /// First child node index (None for files / empty dirs)
    pub first_child: Option<NodeId>,
    /// Next sibling node index (None if last child)
    pub next_sibling: Option<NodeId>,
    /// Depth in the tree (root = 0)
    pub depth: u16,
}

/// The file tree stored as a flat arena of nodes.
pub struct FileTree {
    /// All nodes in contiguous memory
    pub nodes: Vec<FileNode>,
    /// Root node index
    pub root: NodeId,
    /// Absolute filesystem path represented by the root node.
    pub root_path: PathBuf,
    /// Deduplicated extension table: index → extension string (e.g., "pdf", "rs", "exe")
    pub extensions: Vec<CompactString>,
    /// Aggregated file category weights for each node.
    pub category_weights: Vec<[u64; CATEGORY_COUNT]>,
}

impl FileTree {
    /// Create an empty tree with a root node.
    pub fn new(root_name: &str, root_path: PathBuf) -> Self {
        let root_node = FileNode {
            name: CompactString::new(root_name),
            size: 0,
            is_dir: true,
            extension_id: 0,
            category: FileCategory::Misc,
            parent: None,
            first_child: None,
            next_sibling: None,
            depth: 0,
        };

        FileTree {
            nodes: vec![root_node],
            root: NodeId(0),
            root_path,
            extensions: vec![CompactString::new("")], // index 0 = no extension
            category_weights: vec![[0; CATEGORY_COUNT]],
        }
    }

    /// Add a child node under the given parent. Returns the new node's ID.
    pub fn add_child(&mut self, parent: NodeId, mut node: FileNode) -> NodeId {
        let new_id = NodeId(self.nodes.len() as u32);
        let node_size = node.size;
        let is_dir = node.is_dir;
        let category = node.category;
        node.parent = Some(parent);
        node.depth = self.nodes[parent.index()].depth + 1;

        // Prepend to parent's child list (O(1))
        node.next_sibling = self.nodes[parent.index()].first_child;
        self.nodes[parent.index()].first_child = Some(new_id);

        self.nodes.push(node);
        let mut weights = [0; CATEGORY_COUNT];
        if !is_dir && node_size > 0 {
            weights[category_index(category)] = node_size;
        }
        self.category_weights.push(weights);
        new_id
    }

    /// Get a node by ID.
    pub fn get(&self, id: NodeId) -> &FileNode {
        &self.nodes[id.index()]
    }

    /// Get a mutable node by ID.
    pub fn get_mut(&mut self, id: NodeId) -> &mut FileNode {
        &mut self.nodes[id.index()]
    }

    /// Total number of nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the tree is empty (only root).
    pub fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    /// Iterate over children of a node.
    pub fn children(&self, parent: NodeId) -> ChildIter<'_> {
        ChildIter {
            tree: self,
            current: self.nodes[parent.index()].first_child,
        }
    }

    /// Get or create an extension ID for the given extension string.
    pub fn intern_extension(&mut self, ext: &str) -> u16 {
        let lower = ext.to_ascii_lowercase();
        if let Some(pos) = self.extensions.iter().position(|e| e.as_str() == lower) {
            pos as u16
        } else {
            let id = self.extensions.len() as u16;
            self.extensions.push(CompactString::new(&lower));
            id
        }
    }
}

/// Iterator over the children of a node.
pub struct ChildIter<'a> {
    tree: &'a FileTree,
    current: Option<NodeId>,
}

impl<'a> Iterator for ChildIter<'a> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let id = self.current?;
        self.current = self.tree.nodes[id.index()].next_sibling;
        Some(id)
    }
}
