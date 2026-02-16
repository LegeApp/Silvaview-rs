use crate::tree::arena::{FileTree, NodeId};

/// Navigation state: tracks the current view root and history.
pub struct NavigationState {
    /// Stack of view roots (for back navigation)
    history: Vec<NodeId>,
    /// Current view root
    pub current_root: NodeId,
}

impl NavigationState {
    pub fn new(root: NodeId) -> Self {
        Self {
            history: Vec::new(),
            current_root: root,
        }
    }

    /// Drill down into a directory node.
    /// Returns true if navigation happened.
    pub fn drill_down(&mut self, node: NodeId, tree: &FileTree) -> bool {
        let target = tree.get(node);

        // If it's a file, drill into its parent directory instead
        let target_id = if target.is_dir {
            node
        } else {
            match target.parent {
                Some(parent) if parent != self.current_root => parent,
                _ => return false,
            }
        };

        // Don't drill into the same node
        if target_id == self.current_root {
            return false;
        }

        self.history.push(self.current_root);
        self.current_root = target_id;
        true
    }

    /// Navigate up one level.
    /// Returns true if navigation happened.
    pub fn navigate_up(&mut self) -> bool {
        if let Some(prev) = self.history.pop() {
            self.current_root = prev;
            true
        } else {
            false
        }
    }

    /// Navigate to the absolute root.
    pub fn navigate_home(&mut self, root: NodeId) {
        self.history.clear();
        self.current_root = root;
    }

    /// Current depth in navigation history.
    pub fn depth(&self) -> usize {
        self.history.len()
    }
}
