use crate::tree::extensions::FileCategory;
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
/// Vibrant colors on dark background for modern aesthetic.
pub fn category_color(category: FileCategory) -> AppColor {
    match category {
        FileCategory::Image => hsv_to_rgb(190.0 / 360.0, 0.68, 0.92),
        FileCategory::Video => hsv_to_rgb(15.0 / 360.0, 0.75, 0.90),
        FileCategory::Audio => hsv_to_rgb(280.0 / 360.0, 0.70, 0.88),
        FileCategory::Document => hsv_to_rgb(220.0 / 360.0, 0.62, 0.90),
        FileCategory::Ebook => hsv_to_rgb(165.0 / 360.0, 0.58, 0.82),
        FileCategory::Archive => hsv_to_rgb(40.0 / 360.0, 0.78, 0.92),
        FileCategory::Code => hsv_to_rgb(130.0 / 360.0, 0.66, 0.87),
        FileCategory::Executable => hsv_to_rgb(0.0, 0.80, 0.82),
        FileCategory::Config => hsv_to_rgb(55.0 / 360.0, 0.76, 0.92),
        FileCategory::Font => hsv_to_rgb(330.0 / 360.0, 0.55, 0.92),
        FileCategory::Installer => hsv_to_rgb(30.0 / 360.0, 0.82, 0.90),
        FileCategory::Asset3D => hsv_to_rgb(95.0 / 360.0, 0.72, 0.86),
        FileCategory::Backup => hsv_to_rgb(25.0 / 360.0, 0.40, 0.70),
        FileCategory::Database => hsv_to_rgb(245.0 / 360.0, 0.45, 0.82),
        FileCategory::DiskImage => hsv_to_rgb(205.0 / 360.0, 0.64, 0.82),
        FileCategory::Other => AppColor::new(0.50, 0.50, 0.55),
    }
}

/// Get color for a node based on its extension.
pub fn extension_color(ext: &str, settings: &ColorSettings) -> AppColor {
    let category = crate::tree::extensions::categorize_extension(ext);
    let base = category_color(category);
    let ext_norm = ext.trim_start_matches('.').to_ascii_lowercase();
    let adjusted = match settings.mode {
        ColorMode::Category => base,
        ColorMode::CategoryExtension => {
            let hue_jitter = hash01(&ext_norm) * 0.08 - 0.04;
            shift_hsv(base, hue_jitter, 1.0)
        }
        ColorMode::ExtensionHash => {
            let h = hash01(&ext_norm);
            hsv_to_rgb(h, 0.72, 0.84)
        }
    };
    apply_vibrancy(adjusted, settings.vibrancy)
}

/// Directory colors are intentionally muted but varied by name hash.
/// This keeps hierarchy readable without making directories all identical gray.
pub fn directory_color(name: &str, depth: u16, settings: &ColorSettings) -> AppColor {
    let mut h: u32 = 2166136261;
    for &b in name.as_bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    let r = 0.36 + (((h >> 0) & 0xFF) as f32 / 255.0) * 0.26;
    let g = 0.34 + (((h >> 8) & 0xFF) as f32 / 255.0) * 0.24;
    let b = 0.38 + (((h >> 16) & 0xFF) as f32 / 255.0) * 0.22;
    let fade = (depth as f32 * 0.01).min(0.10);
    apply_vibrancy(
        AppColor::new((r - fade).max(0.20), (g - fade).max(0.20), (b - fade).max(0.22)),
        settings.vibrancy * 0.85,
    )
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
