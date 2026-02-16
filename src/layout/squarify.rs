use crate::tree::arena::{FileTree, NodeId};
use std::collections::HashMap;

/// A positioned rectangle in the treemap layout.
#[derive(Debug, Clone, Copy)]
pub struct LayoutRect {
    pub node: NodeId,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub depth: u16,
    /// Accumulated cushion surface coefficients [sx1, sx2, sy1, sy2]
    /// (linear_x, quad_x, linear_y, quad_y) from van Wijk & van de Wetering 1999.
    pub surface: [f32; 4],
}

/// The full layout result (rects + fast lookup).
#[derive(Debug)]
pub struct Layout {
    /// All visible rectangles (files + directories for interaction)
    pub rects: Vec<LayoutRect>,
    /// node → index into `rects` (O(1) hover, tooltip, highlighting)
    pub node_to_rect: HashMap<NodeId, usize>,
}

/// Configuration for treemap layout.
#[derive(Clone)]
pub struct LayoutConfig {
    /// Minimum screen area (px²) to render a node (LOD culling)
    pub min_area: f32,
    /// Base padding between siblings (px)
    pub padding: f32,
    /// How much padding shrinks per nesting level (0.0 = constant, 0.7 = nice taper)
    pub padding_falloff: f32,
    /// Maximum recursion depth (safety + performance)
    pub max_depth: u16,
    /// Target aspect ratio for squarified layout (1.0 = square-ish)
    pub aspect_tolerance: f64,
    /// Initial cushion ridge height (paper default: 0.5)
    pub cushion_height: f32,
    /// Per-level height decay factor (paper default: 0.75)
    pub cushion_falloff: f32,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            min_area: 1.0,        // Reduced from 4.0 to show more small files
            padding: 1.5,         // Slightly reduced padding
            padding_falloff: 0.75,
            max_depth: 64,
            aspect_tolerance: 1.0,
            cushion_height: 0.8, // Increased from 0.5 for more visible cushion effect
            cushion_falloff: 0.75,
        }
    }
}

/// Add a ridge to the cushion surface coefficients along one axis.
/// Matches the CTM procedure from van Wijk & van de Wetering 1999.
fn add_ridge(x1: f32, x2: f32, h: f32, s1: &mut f32, s2: &mut f32) {
    let denom = x2 - x1;
    if denom.abs() < 1e-6 {
        return;
    }
    *s1 += 4.0 * h * (x2 + x1) / denom;
    *s2 -= 4.0 * h / denom;
}

/// Compute layout for any subtree (root can be any directory for drill-down).
pub fn compute_layout(
    tree: &FileTree,
    root: NodeId,
    viewport_w: f32,
    viewport_h: f32,
    config: &LayoutConfig,
) -> Layout {
    let mut rects = Vec::with_capacity(tree.len() / 4); // rough estimate
    let mut node_to_rect = HashMap::with_capacity(rects.capacity());

    let root_rect = LayoutRect {
        node: root,
        x: 0.0,
        y: 0.0,
        w: viewport_w,
        h: viewport_h,
        depth: 0,
        surface: [0.0; 4],
    };

    rects.push(root_rect);
    node_to_rect.insert(root, 0);

    if tree.get(root).is_dir {
        layout_children(
            tree,
            root,
            0.0,
            0.0,
            viewport_w,
            viewport_h,
            0,
            [0.0; 4],
            config.cushion_height,
            config,
            &mut rects,
            &mut node_to_rect,
        );
    }

    Layout { rects, node_to_rect }
}

/// Recursively layout children (now with sorting + better culling).
fn layout_children(
    tree: &FileTree,
    parent: NodeId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    depth: u16,
    parent_surface: [f32; 4],
    cushion_h: f32,
    config: &LayoutConfig,
    rects: &mut Vec<LayoutRect>,
    node_to_rect: &mut HashMap<NodeId, usize>,
) {
    if depth >= config.max_depth {
        return;
    }

    // Dynamic padding that tapers with depth
    let pad = if depth == 0 {
        0.0
    } else {
        config.padding * config.padding_falloff.powi(depth as i32)
    };
    let inner_x = x + pad;
    let inner_y = y + pad;
    let inner_w = (w - 2.0 * pad).max(0.0);
    let inner_h = (h - 2.0 * pad).max(0.0);

    if inner_w * inner_h < config.min_area {
        return;
    }

    let parent_node = tree.get(parent);
    let parent_size = parent_node.size as f64;
    if parent_size <= 0.0 {
        tracing::debug!(
            "Skipping layout for parent {:?} '{}' with zero size at depth {}",
            parent,
            parent_node.name,
            depth
        );
        return;
    }

    // Collect + sort children by size descending (critical for good squarified layout)
    let mut children: Vec<NodeId> = tree.children(parent).collect();
    children.sort_by_key(|&id| std::cmp::Reverse(tree.get(id).size));

    if children.is_empty() {
        return;
    }

    if depth == 0 {
        tracing::info!(
            "Laying out {} children of root '{}' (size={:.2} GB) in {:.0}x{:.0} area",
            children.len(),
            parent_node.name,
            parent_size / 1_073_741_824.0,
            inner_w,
            inner_h
        );
    }

    // Normalized areas
    let total_area = (inner_w as f64) * (inner_h as f64);
    let areas: Vec<f64> = children
        .iter()
        .map(|&id| (tree.get(id).size as f64 / parent_size) * total_area)
        .collect();

    // Squarified layout
    let positioned = squarify(&areas, inner_x as f64, inner_y as f64, inner_w as f64, inner_h as f64);

    for (i, pos) in positioned.iter().enumerate() {
        let child_id = children[i];
        let child_depth = depth + 1;

        let area = (pos.w * pos.h) as f32;
        if area < config.min_area {
            continue;
        }

        let cx = pos.x as f32;
        let cy = pos.y as f32;
        let cw = pos.w as f32;
        let ch = pos.h as f32;

        // Accumulate cushion ridges from parent
        let [mut sx1, mut sx2, mut sy1, mut sy2] = parent_surface;
        add_ridge(cx, cx + cw, cushion_h, &mut sx1, &mut sx2);
        add_ridge(cy, cy + ch, cushion_h, &mut sy1, &mut sy2);
        let surface = [sx1, sx2, sy1, sy2];

        let rect = LayoutRect {
            node: child_id,
            x: cx,
            y: cy,
            w: cw,
            h: ch,
            depth: child_depth,
            surface,
        };

        let idx = rects.len();
        rects.push(rect);
        node_to_rect.insert(child_id, idx);

        // Recurse only into directories
        if tree.get(child_id).is_dir {
            layout_children(
                tree,
                child_id,
                cx,
                cy,
                cw,
                ch,
                child_depth,
                surface,
                cushion_h * config.cushion_falloff,
                config,
                rects,
                node_to_rect,
            );
        }
    }
}

/// Improved squarified layout (tries multiple row lengths for better aspect ratios).
fn squarify(areas: &[f64], mut x: f64, mut y: f64, mut w: f64, mut h: f64) -> Vec<Positioned> {
    let mut result = Vec::with_capacity(areas.len());
    let mut remaining: Vec<f64> = areas.to_vec();
    remaining.sort_by(|a, b| b.partial_cmp(a).unwrap()); // descending

    while !remaining.is_empty() {
        // Guard against degenerate cases
        if w <= 1e-6 || h <= 1e-6 {
            break;
        }

        let horizontal = w >= h;
        let short = if horizontal { h } else { w };

        // Find best row length
        let mut best_score = f64::INFINITY;
        let mut best_k = 1;
        let mut row_sum = 0.0;

        for k in 1..=remaining.len().min(20) { // cap for speed
            let sum: f64 = remaining[0..k].iter().sum();
            let score = worst_aspect_ratio(&remaining[0..k], sum, short);
            if score < best_score {
                best_score = score;
                best_k = k;
                row_sum = sum;
            } else if k > 3 {
                break; // diminishing returns
            }
        }

        let row = &remaining[0..best_k];
        // If we are laying out a horizontal row, thickness consumes height and is
        // computed against available width. For a vertical column, vice versa.
        let long = if horizontal { w } else { h };
        let thickness = row_sum / long.max(1e-8);

        let mut offset = 0.0;
        for &area in row {
            let length = area / thickness.max(1e-8);

            // Validate dimensions before creating the positioned rect
            if !length.is_finite() || !thickness.is_finite() || length <= 0.0 || thickness <= 0.0 {
                tracing::warn!(
                    "Squarify: invalid dimensions (length={}, thickness={}, area={}, short={}), skipping",
                    length, thickness, area, short
                );
                continue;
            }

            let pos = if horizontal {
                Positioned {
                    x: x + offset,
                    y,
                    w: length,
                    h: thickness,
                }
            } else {
                Positioned {
                    x,
                    y: y + offset,
                    w: thickness,
                    h: length,
                }
            };
            result.push(pos);
            offset += length;
        }

        // Shrink remaining space
        if horizontal {
            y += thickness;
            h = (h - thickness).max(0.0);
        } else {
            x += thickness;
            w = (w - thickness).max(0.0);
        }

        remaining.drain(0..best_k);
    }

    result
}

#[derive(Debug, Clone, Copy)]
struct Positioned {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

fn worst_aspect_ratio(row: &[f64], sum: f64, side: f64) -> f64 {
    if row.is_empty() || sum <= 0.0 || side <= 0.0 {
        return f64::MAX;
    }
    let side_sq = side * side;
    let sum_sq = sum * sum;
    let max_r = row.iter().copied().fold(0.0, f64::max);
    let min_r = row.iter().copied().fold(f64::INFINITY, f64::min);
    let a = (side_sq * max_r) / sum_sq;
    let b = sum_sq / (side_sq * min_r);
    a.max(b)
}

#[cfg(test)]
mod tests {
    use super::squarify;

    #[test]
    fn single_item_fills_viewport_without_axis_swap() {
        let rects = squarify(&[1920.0 * 1080.0], 0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(rects.len(), 1);
        let r = rects[0];
        assert!((r.w - 1920.0).abs() < 1e-6);
        assert!((r.h - 1080.0).abs() < 1e-6);
    }

    #[test]
    fn layout_preserves_area_for_simple_case() {
        let areas = [400.0, 300.0, 200.0, 100.0];
        let rects = squarify(&areas, 0.0, 0.0, 50.0, 20.0);
        let total_in: f64 = areas.iter().sum();
        let total_out: f64 = rects.iter().map(|r| r.w * r.h).sum();
        assert!((total_in - total_out).abs() < 1e-6);
    }
}
