use crate::layout::LayoutRect;
use crate::render::colors;
use crate::tree::arena::FileTree;
use vello::kurbo::Rect;

/// Cushion shading parameters (van Wijk & van de Wetering 1999).
#[derive(Clone, Copy)]
pub struct CushionConfig {
    /// Ambient light intensity (paper default: ~0.16 = 40/255)
    pub ambient: f32,
    /// Diffuse light intensity (paper default: ~0.84 = 215/255)
    pub diffuse: f32,
    /// Normalized light direction [x, y, z]
    pub light: [f32; 3],
}

impl Default for CushionConfig {
    fn default() -> Self {
        // Light direction from paper: (1, 2, 10), normalized
        let (lx, ly, lz) = (1.0_f32, 2.0, 10.0);
        let len = (lx * lx + ly * ly + lz * lz).sqrt();
        Self {
            ambient: 0.4,   // Much brighter ambient (40% base brightness)
            diffuse: 0.6,   // Strong diffuse for good shading contrast
            light: [lx / len, ly / len, lz / len],
        }
    }
}

/// CPU-rasterize the cushion treemap into an RGBA pixel buffer.
///
/// Each pixel's color is determined by the deepest (last-drawn) rectangle
/// containing it. The surface normal is derived from the accumulated
/// cushion coefficients and shaded with Lambertian reflectance.
pub fn rasterize_cushions(
    width: u32,
    height: u32,
    layout_rects: &[LayoutRect],
    tree: &FileTree,
    config: &CushionConfig,
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut buf = vec![0u8; w * h * 4];

    // Initialize to dark neutral background, fully opaque.
    for pixel in buf.chunks_exact_mut(4) {
        pixel[0] = 20;
        pixel[1] = 22;
        pixel[2] = 28;
        pixel[3] = 255;
    }

    let [lx, ly, lz] = config.light;

    // Iterate rects in order: parents before children.
    // Children overwrite parent pixels, so deeper structure shows through.
    for rect in layout_rects {
        let node = tree.get(rect.node);

        // Base color
        let base = if node.is_dir {
            colors::directory_color(&node.name, rect.depth)
        } else {
            let ext = if node.extension_id > 0 {
                tree.extensions
                    .get(node.extension_id as usize)
                    .map(|s| s.as_str())
                    .unwrap_or("")
            } else {
                ""
            };
            colors::extension_color(ext)
        };

        let [sx1, sx2, sy1, sy2] = rect.surface;

        // Pixel bounds (clamped to buffer)
        let px0 = (rect.x as usize).min(w);
        let py0 = (rect.y as usize).min(h);
        let px1 = ((rect.x + rect.w).ceil() as usize).min(w);
        let py1 = ((rect.y + rect.h).ceil() as usize).min(h);

        for py in py0..py1 {
            let py_f = py as f32 + 0.5;
            // Precompute Y component of normal
            let ny = -(2.0 * sy2 * py_f + sy1);

            let row_offset = py * w;
            for px in px0..px1 {
                let px_f = px as f32 + 0.5;

                // Surface normal from accumulated parabolic coefficients
                let nx = -(2.0 * sx2 * px_f + sx1);
                // nz = 1.0 (implicit)

                // Lambertian shading: I = ambient + diffuse * max(0, dot(n, light) / |n|)
                let dot = nx * lx + ny * ly + lz;
                let n_len = (nx * nx + ny * ny + 1.0).sqrt();
                let cos_theta = (dot / n_len).max(0.0);
                let intensity = config.ambient + config.diffuse * cos_theta;

                let r = (base.r * intensity).min(1.0).max(0.0);
                let g = (base.g * intensity).min(1.0).max(0.0);
                let b = (base.b * intensity).min(1.0).max(0.0);

                let idx = (row_offset + px) * 4;
                buf[idx] = (r * 255.0) as u8;
                buf[idx + 1] = (g * 255.0) as u8;
                buf[idx + 2] = (b * 255.0) as u8;
                // alpha stays 255

                // Debug: Check for unexpected black pixels
                if buf[idx] == 0 && buf[idx + 1] == 0 && buf[idx + 2] == 0 && px < 5 && py < 5 {
                    tracing::warn!(
                        "Black pixel at ({}, {}) for node '{}' (base: {:.2},{:.2},{:.2}, intensity: {:.2})",
                        px, py, node.name, base.r, base.g, base.b, intensity
                    );
                }
            }
        }
    }

    buf
}

/// Get the bounding rect for a layout rect (as a vello kurbo Rect).
pub fn layout_to_rect(rect: &LayoutRect) -> Rect {
    Rect::new(
        rect.x as f64,
        rect.y as f64,
        (rect.x + rect.w) as f64,
        (rect.y + rect.h) as f64,
    )
}
