use std::sync::Arc;

use vello::kurbo::{Affine, Rect};
use vello::peniko::{Blob, Color, Fill, Image, ImageFormat};
use vello::Scene;

use super::text::{TextRenderer, TextRenderResult};
use crate::layout::LayoutRect;
use crate::tree::arena::{FileTree, NodeId};

/// Build a Vello scene from the cached treemap image + overlays.
pub fn build_scene(
    scene: &mut Scene,
    treemap_image: Option<&Image>,
    layout_rects: &[LayoutRect],
    tree: &FileTree,
    hover_node: Option<NodeId>,
    text_renderer: &mut TextRenderer,
    show_text_labels: bool,
) {
    scene.reset();

    // Draw the cached CPU-rasterized treemap as a single image
    if let Some(image) = treemap_image {
        tracing::info!(
            "Drawing treemap image: {}x{} ({} bytes)",
            image.width,
            image.height,
            image.data.as_ref().len()
        );
        scene.draw_image(image, Affine::IDENTITY);
    } else {
        tracing::warn!("No treemap image to draw!");
    }

    if show_text_labels {
        let viewport_area = layout_rects
            .first()
            .map(|r| (r.w * r.h).max(1.0))
            .unwrap_or(1.0);
        let min_label_area = (viewport_area * 0.003).max(8_000.0);
        let mut candidates: Vec<&LayoutRect> = layout_rects
            .iter()
            .filter(|r| {
                let area = r.w * r.h;
                area >= min_label_area && r.w >= 70.0 && r.h >= 20.0 && r.depth <= 5
            })
            .collect();
        candidates.sort_by(|a, b| (b.w * b.h).partial_cmp(&(a.w * a.h)).unwrap());

        let max_labels = 24;
        let mut placed_bounds: Vec<[f32; 4]> = Vec::with_capacity(max_labels);
        let mut drawn = 0usize;

        for rect in candidates {
            if drawn >= max_labels {
                break;
            }

            let node = tree.get(rect.node);
            let pad = 3.0;
            let max_text_w = rect.w - pad * 2.0;
            if max_text_w <= 24.0 {
                continue;
            }

            let font_size = (rect.h * 0.18).clamp(10.0, 16.0);
            let label = truncate_label(&node.name, max_text_w, font_size);
            if label.is_empty() {
                continue;
            }

            if let Some(text_result) =
                text_renderer.render_text(&label, "default", font_size, Some(max_text_w))
            {
                let text_w = text_result.width as f32;
                let text_h = text_result.height as f32;
                if text_w <= 1.0 || text_h <= 1.0 || text_h > rect.h - pad * 2.0 {
                    continue;
                }

                let tx = rect.x + pad;
                let ty = rect.y + pad;
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
                drawn += 1;
            }
        }
    }

    // DISABLED FOR DEBUGGING - Draw hover highlight overlay
    // if let Some(hover_id) = hover_node {
    //     for rect in layout_rects {
    //         if rect.node == hover_id {
    //             let shape = cushion::layout_to_rect(rect);
    //             let highlight = Color::new([1.0f32, 1.0, 1.0, 0.2]);
    //             scene.fill(Fill::NonZero, Affine::IDENTITY, highlight, None, &shape);
    //             break;
    //         }
    //     }
    // }
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
