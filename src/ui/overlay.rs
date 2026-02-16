use vello::kurbo::{Affine, Rect};
use vello::peniko::{Brush, Color, Fill};
use vello::Scene;

use crate::tree::arena::{FileTree, NodeId};
use crate::tree::extensions::FileCategory;
use crate::ui::tooltip;

/// Analytics data for the file type breakdown panel.
#[derive(Debug, Default)]
pub struct Analytics {
    /// Total bytes per category
    pub category_sizes: Vec<(FileCategory, u64)>,
    /// Total size of all files
    pub total_size: u64,
}

/// Compute analytics for the current view.
pub fn compute_analytics(tree: &FileTree, root: NodeId) -> Analytics {
    let mut category_map = std::collections::HashMap::new();
    let mut total_size = 0u64;

    // Traverse all descendants
    let mut stack = vec![root];
    while let Some(node_id) = stack.pop() {
        let node = tree.get(node_id);

        if !node.is_dir {
            // It's a file - categorize it
            let ext = if node.extension_id > 0 {
                tree.extensions
                    .get(node.extension_id as usize)
                    .map(|s| s.as_str())
                    .unwrap_or("")
            } else {
                ""
            };
            let category = crate::tree::extensions::categorize_extension(ext);
            *category_map.entry(category).or_insert(0u64) += node.size;
            total_size += node.size;
        }

        // Add children to stack
        for child_id in tree.children(node_id) {
            stack.push(child_id);
        }
    }

    // Sort by size descending
    let mut category_sizes: Vec<_> = category_map.into_iter().collect();
    category_sizes.sort_by(|a, b| b.1.cmp(&a.1));

    Analytics {
        category_sizes,
        total_size,
    }
}

/// Render the analytics panel on the right side.
pub fn render_analytics_panel(
    scene: &mut Scene,
    analytics: &Analytics,
    viewport_width: f32,
    viewport_height: f32,
) {
    let panel_width = 250.0;
    let panel_x = viewport_width - panel_width;

    // Semi-transparent dark background
    let bg_rect = Rect::new(
        panel_x as f64,
        0.0,
        viewport_width as f64,
        viewport_height as f64,
    );
    let bg_brush = Brush::Solid(Color::new([0.1, 0.1, 0.12, 0.9]));
    scene.fill(Fill::NonZero, Affine::IDENTITY, &bg_brush, None, &bg_rect);

    // TODO: Add text rendering using parley or a simple glyph renderer
    // For now, just draw colored bars for each category

    let bar_start_y = 40.0;
    let bar_height = 24.0;
    let bar_spacing = 4.0;
    let bar_max_width = panel_width - 40.0;

    for (i, (category, size)) in analytics.category_sizes.iter().enumerate() {
        let y = bar_start_y + (i as f32) * (bar_height + bar_spacing);
        if y + bar_height > viewport_height {
            break;
        }

        let percentage = if analytics.total_size > 0 {
            (*size as f64) / (analytics.total_size as f64)
        } else {
            0.0
        };
        let bar_width = (percentage * bar_max_width as f64) as f32;

        // Category color bar
        let bar_rect = Rect::new(
            (panel_x + 20.0) as f64,
            y as f64,
            (panel_x + 20.0 + bar_width) as f64,
            (y + bar_height) as f64,
        );
        let color = crate::render::colors::category_color(*category);
        let bar_brush = Brush::Solid(color.to_peniko());
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &bar_brush,
            None,
            &bar_rect,
        );
    }
}

/// Render hover tooltip for a file.
pub fn render_tooltip(
    scene: &mut Scene,
    tree: &FileTree,
    node_id: NodeId,
    mouse_x: f32,
    mouse_y: f32,
) {
    let info = tooltip::build_tooltip(tree, node_id);

    // Tooltip background
    let tooltip_width = 300.0;
    let tooltip_height = 80.0;
    let mut tooltip_x = mouse_x + 15.0;
    let tooltip_y = mouse_y + 15.0;

    // Keep tooltip on screen
    if tooltip_x + tooltip_width > 1280.0 {
        tooltip_x = mouse_x - tooltip_width - 15.0;
    }

    let tooltip_rect = Rect::new(
        tooltip_x as f64,
        tooltip_y as f64,
        (tooltip_x + tooltip_width) as f64,
        (tooltip_y + tooltip_height) as f64,
    );

    // Dark background with slight transparency
    let bg_brush = Brush::Solid(Color::new([0.15, 0.15, 0.18, 0.95]));
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &bg_brush,
        None,
        &tooltip_rect,
    );

    // Border
    // TODO: Add stroke rendering when we add text support

    // For now, just render the background
    // Text will be added when we integrate parley
}

/// Render breadcrumb navigation path at the top.
pub fn render_breadcrumb(
    scene: &mut Scene,
    tree: &FileTree,
    current_root: NodeId,
    viewport_width: f32,
) {
    let breadcrumb_height = 32.0;

    // Background bar
    let bg_rect = Rect::new(0.0, 0.0, viewport_width as f64, breadcrumb_height as f64);
    let bg_brush = Brush::Solid(Color::new([0.12, 0.12, 0.14, 0.85]));
    scene.fill(Fill::NonZero, Affine::IDENTITY, &bg_brush, None, &bg_rect);

    // Build path
    let _path = tooltip::build_path(tree, current_root);

    // TODO: Render text path using parley
    // For Phase 2, we'll keep it simple without text initially
}
