use std::sync::Arc;

use vello::kurbo::{Affine, Rect};
use vello::peniko::{Blob, Color, Fill, Image, ImageFormat};
use vello::Scene;

use super::cushion;
use super::text::{TextRenderer, TextRenderResult};
use crate::layout::LayoutRect;
use crate::tree::arena::{FileTree, NodeId};
use crate::ui::tooltip::format_size;

#[derive(Debug, Clone, Copy)]
pub struct LabelHitRegion {
    pub node: NodeId,
    pub bounds: [f32; 4], // [x1, y1, x2, y2]
}

/// Build a Vello scene from the cached treemap image + overlays.
pub fn build_scene(
    scene: &mut Scene,
    treemap_image: Option<&Image>,
    layout_rects: &[LayoutRect],
    tree: &FileTree,
    hover_node: Option<NodeId>,
    text_renderer: &mut TextRenderer,
    show_text_labels: bool,
) -> Vec<LabelHitRegion> {
    scene.reset();
    let mut label_hit_regions = Vec::new();

    // Draw the cached CPU-rasterized treemap as a single image
    if let Some(image) = treemap_image {
        tracing::debug!(
            "Drawing treemap image: {}x{} ({} bytes)",
            image.width,
            image.height,
            image.data.as_ref().len()
        );
        scene.draw_image(image, Affine::IDENTITY);
    } else {
        tracing::debug!("No treemap image to draw yet");
    }

    // Draw lightweight directory frame/header overlays so hierarchy reads as nested containers.
    for rect in layout_rects {
        let node = tree.get(rect.node);
        if !node.is_dir || rect.depth == 0 || rect.w < 24.0 || rect.h < 20.0 {
            continue;
        }

        let (frame, header) = directory_frame_params(rect.depth);
        let inner_w = rect.w - frame * 2.0;
        let inner_h = rect.h - frame * 2.0;
        if inner_w <= 2.0 || inner_h <= 2.0 {
            continue;
        }

        // Subtle top header band where directory labels live.
        let header_h = header.min((inner_h - 1.0).max(0.0));
        if header_h > 1.0 {
            let header_rect = Rect::new(
                (rect.x + frame) as f64,
                (rect.y + frame) as f64,
                (rect.x + rect.w - frame) as f64,
                (rect.y + frame + header_h) as f64,
            );
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                Color::new([0.0, 0.0, 0.0, 0.22]),
                None,
                &header_rect,
            );
        }

        // Frame border strips.
        let border = Color::new([1.0, 1.0, 1.0, 0.08]);
        let x0 = rect.x;
        let y0 = rect.y;
        let x1 = rect.x + rect.w;
        let y1 = rect.y + rect.h;

        let top = Rect::new(x0 as f64, y0 as f64, x1 as f64, (y0 + frame) as f64);
        let bottom = Rect::new(x0 as f64, (y1 - frame) as f64, x1 as f64, y1 as f64);
        let left = Rect::new(x0 as f64, y0 as f64, (x0 + frame) as f64, y1 as f64);
        let right = Rect::new((x1 - frame) as f64, y0 as f64, x1 as f64, y1 as f64);
        scene.fill(Fill::NonZero, Affine::IDENTITY, border, None, &top);
        scene.fill(Fill::NonZero, Affine::IDENTITY, border, None, &bottom);
        scene.fill(Fill::NonZero, Affine::IDENTITY, border, None, &left);
        scene.fill(Fill::NonZero, Affine::IDENTITY, border, None, &right);
    }

    if show_text_labels {
        let viewport_area = layout_rects
            .first()
            .map(|r| (r.w * r.h).max(1.0))
            .unwrap_or(1.0);
        let min_label_area = (viewport_area * 0.0004).max(1_200.0);
        let mut candidates: Vec<&LayoutRect> = layout_rects
            .iter()
            .filter(|r| {
                let node = tree.get(r.node);
                let area = r.w * r.h;
                node.is_dir && r.depth >= 1 && area >= min_label_area && r.w >= 64.0 && r.h >= 18.0 && r.depth <= 10
            })
            .collect();
        candidates.sort_by(|a, b| (b.w * b.h).partial_cmp(&(a.w * a.h)).unwrap());

        let max_labels = 80;
        let candidate_count = candidates.len();
        let mut placed_bounds: Vec<[f32; 4]> = Vec::with_capacity(max_labels);
        let mut drawn = 0usize;

        for rect in candidates {
            if drawn >= max_labels {
                break;
            }
            let node = tree.get(rect.node);
            let (frame, header) = directory_frame_params(rect.depth);
            let pad_x = (frame + 3.0).max(4.0);
            let pad_y = (frame + 2.0).max(3.0);
            let max_text_w = rect.w - pad_x * 2.0;
            if max_text_w <= 24.0 {
                continue;
            }

            let label_band_h = header.min((rect.h - pad_y - 1.0).max(0.0));
            if label_band_h <= 10.0 {
                continue;
            }

            let font_size = (label_band_h * 0.62).clamp(9.0, 14.0);
            let base = format!("{}  {}", node.name, format_size(node.size));
            let label = truncate_label(&base, max_text_w, font_size);
            if label.is_empty() {
                continue;
            }

            if let Some(text_result) =
                text_renderer.render_text(&label, "default", font_size, Some(max_text_w))
            {
                let text_w = text_result.width as f32;
                let text_h = text_result.height as f32;
                if text_w <= 1.0 || text_h <= 1.0 || text_h > label_band_h {
                    continue;
                }

                let tx = rect.x + pad_x;
                let ty = rect.y + pad_y;
                let bounds = [tx, ty, tx + text_w + 2.0, ty + text_h + 2.0];
                if placed_bounds.iter().any(|b| rects_overlap(*b, bounds)) {
                    continue;
                }

                let bg = Rect::new(
                    bounds[0] as f64,
                    bounds[1] as f64,
                    bounds[2] as f64,
                    bounds[3] as f64,
                );
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    Color::new([0.0, 0.0, 0.0, 0.35]),
                    None,
                    &bg,
                );
                draw_text_to_scene(scene, text_result, tx + 1.0, ty + 1.0);
                placed_bounds.push(bounds);
                label_hit_regions.push(LabelHitRegion {
                    node: rect.node,
                    bounds,
                });
                drawn += 1;
            }
        }

        tracing::debug!(
            "Text overlays: candidates={}, drawn={}, hover={:?}",
            candidate_count,
            drawn,
            hover_node
        );
    }

    // Hover highlight helps orient which rectangle is under the cursor.
    if let Some(hover_id) = hover_node {
        for rect in layout_rects {
            if rect.node == hover_id {
                let shape = cushion::layout_to_rect(rect);
                let highlight = Color::new([1.0f32, 1.0, 1.0, 0.20]);
                scene.fill(Fill::NonZero, Affine::IDENTITY, highlight, None, &shape);
                break;
            }
        }
    }

    label_hit_regions
}

/// Draw rendered text to a Vello scene.
fn draw_text_to_scene(scene: &mut Scene, text_result: TextRenderResult, x: f32, y: f32) {
    for glyph in text_result.glyphs {
        if glyph.bitmap.is_empty() {
            continue;
        }

        // Create image from glyph bitmap
        let glyph_image = Image::new(
            Blob::new(Arc::new(glyph.bitmap)),
            ImageFormat::Rgba8,
            glyph.width as u32,
            glyph.height as u32,
        );

        // Draw glyph at position
        let transform = Affine::translate((x as f64 + glyph.x as f64, y as f64 + glyph.y as f64));
        scene.draw_image(&glyph_image, transform);
    }
}

fn rects_overlap(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
}

fn directory_frame_params(depth: u16) -> (f32, f32) {
    if depth == 0 {
        return (0.0, 0.0);
    }
    let scale = 0.92_f32.powi((depth as i32 - 1).max(0));
    let frame = (2.0 * scale).max(1.0);
    let header = (16.0 * scale).max(8.0);
    (frame, header)
}

fn truncate_label(name: &str, max_width: f32, font_size: f32) -> String {
    let approx_char_w = (font_size * 0.58).max(1.0);
    let max_chars = (max_width / approx_char_w) as usize;
    if max_chars < 3 {
        return String::new();
    }
    if name.chars().count() <= max_chars {
        return name.to_string();
    }
    if max_chars <= 3 {
        return "...".to_string();
    }
    let keep = max_chars - 3;
    let truncated: String = name.chars().take(keep).collect();
    format!("{}...", truncated)
}

/// Create a `peniko::Image` from an RGBA pixel buffer.
pub fn image_from_rgba(buf: Vec<u8>, width: u32, height: u32) -> Image {
    let data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(buf);
    Image::new(Blob::new(data), ImageFormat::Rgba8, width, height)
}
