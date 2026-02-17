use crate::layout::LayoutRect;
use crate::render::colors;
use crate::render::colors::ColorSettings;
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
    /// Fast approximate lighting mode (avoids per-pixel normal normalization).
    pub fast_lighting: bool,
}

impl Default for CushionConfig {
    fn default() -> Self {
        // Light direction from paper: (1, 2, 10), normalized
        let (lx, ly, lz) = (1.0_f32, 2.0, 10.0);
        let len = (lx * lx + ly * ly + lz * lz).sqrt();
        Self {
            // Lower ambient + stronger diffuse gives better visual separation.
            ambient: 0.26,
            diffuse: 0.92,
            light: [lx / len, ly / len, lz / len],
            // Prioritize visual fidelity by default; fast mode remains optional.
            fast_lighting: false,
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
    color_settings: &ColorSettings,
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

    // Normalize light once per rasterization pass (never per-pixel).
    let [mut lx, mut ly, mut lz] = config.light;
    let light_len = (lx * lx + ly * ly + lz * lz).sqrt();
    if light_len > 1e-6 {
        lx /= light_len;
        ly /= light_len;
        lz /= light_len;
    } else {
        lx = 0.09759001;
        ly = 0.19518003;
        lz = 0.9759001;
    }

    // Iterate rects in order: parents before children.
    // Children overwrite parent pixels, so deeper structure shows through.
    for rect in layout_rects {
        let node = tree.get(rect.node);

        // Base color
        let base = if node.is_dir {
            colors::directory_color(&node.name, rect.depth, color_settings)
        } else {
            let ext = if node.extension_id > 0 {
                tree.extensions
                    .get(node.extension_id as usize)
                    .map(|s| s.as_str())
                    .unwrap_or("")
            } else {
                ""
            };
            colors::extension_color(ext, color_settings)
        };

        let [sx1, sx2, sy1, sy2] = rect.surface;

        // Pixel bounds (clamped to buffer)
        let px0 = (rect.x as usize).min(w);
        let py0 = (rect.y as usize).min(h);
        let px1 = ((rect.x + rect.w).ceil() as usize).min(w);
        let py1 = ((rect.y + rect.h).ceil() as usize).min(h);

        if config.fast_lighting {
            // Approximate mode: skip per-pixel normal normalization for speed.
            for py in py0..py1 {
                let py_f = py as f32 + 0.5;
                let ny = -(2.0 * sy2 * py_f + sy1);

                let row_offset = py * w;
                for px in px0..px1 {
                    let px_f = px as f32 + 0.5;
                    let nx = -(2.0 * sx2 * px_f + sx1);

                    // Fast path: approximate normalization with reciprocal sqrt.
                    let lambert = (nx * lx + ny * ly + lz).max(0.0);
                    let inv_len = (nx * nx + ny * ny + 1.0).max(1e-5).sqrt().recip();
                    let ndotl = lambert * inv_len;
                    let intensity = (config.ambient + config.diffuse * ndotl)
                        .clamp(0.0, 1.0)
                        .powf(1.22);

                    let idx = (row_offset + px) * 4;
                    buf[idx] = (base.r * intensity * 255.0) as u8;
                    buf[idx + 1] = (base.g * intensity * 255.0) as u8;
                    buf[idx + 2] = (base.b * intensity * 255.0) as u8;
                }
            }
        } else {
            // Full Lambert mode: normalize per-pixel normal for higher fidelity.
            for py in py0..py1 {
                let py_f = py as f32 + 0.5;
                let ny = -(2.0 * sy2 * py_f + sy1);

                let row_offset = py * w;
                for px in px0..px1 {
                    let px_f = px as f32 + 0.5;
                    let nx = -(2.0 * sx2 * px_f + sx1);

                    let dot = nx * lx + ny * ly + lz;
                    let n_len = (nx * nx + ny * ny + 1.0).sqrt();
                    let cos_theta = (dot / n_len).max(0.0);
                    let intensity = (config.ambient + config.diffuse * cos_theta)
                        .clamp(0.0, 1.0)
                        .powf(1.22);

                    let idx = (row_offset + px) * 4;
                    buf[idx] = (base.r * intensity * 255.0) as u8;
                    buf[idx + 1] = (base.g * intensity * 255.0) as u8;
                    buf[idx + 2] = (base.b * intensity * 255.0) as u8;
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
