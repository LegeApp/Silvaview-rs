use crate::tree::extensions::{category_index, FileCategory, CATEGORY_COUNT, CATEGORY_ORDER};
use vello::peniko::color::{DynamicColor, Srgb};
use vello::peniko::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Category,
    CategoryExtension,
    ExtensionHash,
}

#[derive(Debug, Clone, Copy)]
pub struct ColorSettings {
    pub mode: ColorMode,
    pub vibrancy: f32,
}

impl Default for ColorSettings {
    fn default() -> Self {
        Self {
            mode: ColorMode::CategoryExtension,
            vibrancy: 1.20,
        }
    }
}

/// Our custom color representation for easy manipulation.
#[derive(Debug, Clone, Copy)]
pub struct AppColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl AppColor {
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Convert to vello's peniko Color (AlphaColor<Srgb>).
    pub fn to_peniko(self) -> Color {
        Color::new([self.r, self.g, self.b, self.a])
    }

    /// Convert to DynamicColor for gradient stops.
    pub fn to_dynamic(self) -> DynamicColor {
        DynamicColor::from_alpha_color::<Srgb>(self.to_peniko())
    }

    /// Create a lighter version (for cushion highlight).
    pub fn lighten(self, amount: f32) -> Self {
        Self {
            r: (self.r + amount).min(1.0),
            g: (self.g + amount).min(1.0),
            b: (self.b + amount).min(1.0),
            a: self.a,
        }
    }

    /// Create a darker version (for cushion shadow).
    pub fn darken(self, amount: f32) -> Self {
        Self {
            r: (self.r - amount).max(0.0),
            g: (self.g - amount).max(0.0),
            b: (self.b - amount).max(0.0),
            a: self.a,
        }
    }
}

/// Dark mode color palette for file categories.
/// Categories follow a readable spectrum:
/// docs -> code -> images -> video -> audio -> archives -> system -> misc.
pub fn category_color(category: FileCategory) -> AppColor {
    let (h, s, v) = match category {
        FileCategory::Document => (215.0 / 360.0, 0.72, 0.95),
        FileCategory::Code => (175.0 / 360.0, 0.72, 0.91),
        FileCategory::Image => (132.0 / 360.0, 0.80, 0.95),
        FileCategory::Video => (42.0 / 360.0, 0.84, 0.97),
        FileCategory::Audio => (336.0 / 360.0, 0.76, 0.94),
        FileCategory::Archive => (282.0 / 360.0, 0.66, 0.90),
        FileCategory::System => return AppColor::new(0.56, 0.60, 0.68),
        FileCategory::Misc => (22.0 / 360.0, 0.48, 0.88),
    };
    hsv_to_rgb(h, s, v)
}

pub fn file_color(category: FileCategory, ext: &str, settings: &ColorSettings) -> AppColor {
    let base = category_color(category);
    let ext_norm = ext.trim_start_matches('.').to_ascii_lowercase();
    let adjusted = match settings.mode {
        ColorMode::Category => base,
        ColorMode::CategoryExtension => {
            let hue_jitter = hash01(&ext_norm) * 0.11 - 0.055;
            let sat_mul = 0.94 + hash01(&(ext_norm.clone() + "#sat")) * 0.16;
            shift_hsv(base, hue_jitter, sat_mul)
        }
        ColorMode::ExtensionHash => {
            let h = hash01(&ext_norm);
            hsv_to_rgb(h, 0.78, 0.88)
        }
    };
    apply_vibrancy(adjusted, settings.vibrancy)
}

/// Backward-compatible helper for file color lookups by extension.
pub fn extension_color(ext: &str, settings: &ColorSettings) -> AppColor {
    let category = crate::tree::extensions::categorize_extension(ext);
    file_color(category, ext, settings)
}

/// Directories inherit their palette from the file types they contain so color remains
/// stable as you move between overview and drill-down levels.
pub fn directory_color(weights: &[u64; CATEGORY_COUNT], settings: &ColorSettings) -> AppColor {
    let total: u64 = weights.iter().sum();
    if total == 0 {
        return apply_vibrancy(category_color(FileCategory::Misc), settings.vibrancy * 0.9);
    }

    let top = top_categories(weights, 4);
    let mut r = 0.0;
    let mut g = 0.0;
    let mut b = 0.0;
    let mut sum = 0.0;
    for (category, weight) in top.iter().copied() {
        let share = weight as f32 / total as f32;
        let emphasis = share.powf(0.72);
        let c = category_color(category);
        r += c.r * emphasis;
        g += c.g * emphasis;
        b += c.b * emphasis;
        sum += emphasis;
    }

    if sum <= 1e-6 {
        return apply_vibrancy(category_color(FileCategory::Misc), settings.vibrancy * 0.9);
    }

    let dominant_share = top
        .first()
        .map(|(_, weight)| *weight as f32 / total as f32)
        .unwrap_or(1.0);
    let stability = 0.92 - (1.0 - dominant_share).min(0.45) * 0.25;
    apply_vibrancy(
        AppColor::new(r / sum, g / sum, b / sum),
        settings.vibrancy * stability,
    )
}

pub fn top_categories(weights: &[u64; CATEGORY_COUNT], limit: usize) -> Vec<(FileCategory, u64)> {
    let mut entries: Vec<(FileCategory, u64)> = CATEGORY_ORDER
        .iter()
        .copied()
        .filter_map(|category| {
            let weight = weights[category_index(category)];
            (weight > 0).then_some((category, weight))
        })
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries.truncate(limit.min(entries.len()));
    entries
}

pub fn mode_name(mode: ColorMode) -> &'static str {
    match mode {
        ColorMode::Category => "Category",
        ColorMode::CategoryExtension => "Cat+Ext",
        ColorMode::ExtensionHash => "Ext Hash",
    }
}

fn apply_vibrancy(color: AppColor, vibrancy: f32) -> AppColor {
    let (mut h, mut s, v) = rgb_to_hsv(color);
    let _ = &mut h;
    s = (s * vibrancy.clamp(0.6, 2.0)).clamp(0.0, 1.0);
    hsv_to_rgb(h, s, v)
}

fn shift_hsv(color: AppColor, hue_delta: f32, sat_mul: f32) -> AppColor {
    let (mut h, mut s, v) = rgb_to_hsv(color);
    h = (h + hue_delta).rem_euclid(1.0);
    s = (s * sat_mul).clamp(0.0, 1.0);
    hsv_to_rgb(h, s, v)
}

fn rgb_to_hsv(c: AppColor) -> (f32, f32, f32) {
    let max = c.r.max(c.g.max(c.b));
    let min = c.r.min(c.g.min(c.b));
    let d = max - min;
    let h = if d <= 1e-6 {
        0.0
    } else if (max - c.r).abs() <= 1e-6 {
        ((c.g - c.b) / d).rem_euclid(6.0) / 6.0
    } else if (max - c.g).abs() <= 1e-6 {
        (((c.b - c.r) / d) + 2.0) / 6.0
    } else {
        (((c.r - c.g) / d) + 4.0) / 6.0
    };
    let s = if max <= 1e-6 { 0.0 } else { d / max };
    (h, s, max)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> AppColor {
    let h6 = (h * 6.0).rem_euclid(6.0);
    let i = h6.floor() as i32;
    let f = h6 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    AppColor { r, g, b, a: 1.0 }
}

fn hash01(s: &str) -> f32 {
    let mut h: u32 = 2166136261;
    for &b in s.as_bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    ((h >> 8) as f32) / ((u32::MAX >> 8) as f32)
}
