use crate::tree::extensions::FileCategory;
use vello::peniko::color::{DynamicColor, Srgb};
use vello::peniko::Color;

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
        FileCategory::Image => AppColor::new(0.90, 0.45, 0.65),      // Pink
        FileCategory::Video => AppColor::new(0.85, 0.35, 0.35),      // Red
        FileCategory::Audio => AppColor::new(0.95, 0.60, 0.30),      // Orange
        FileCategory::Document => AppColor::new(0.40, 0.70, 0.95),   // Blue
        FileCategory::Archive => AppColor::new(0.95, 0.80, 0.25),    // Yellow
        FileCategory::Code => AppColor::new(0.40, 0.85, 0.55),       // Green
        FileCategory::Executable => AppColor::new(0.70, 0.40, 0.90), // Purple
        FileCategory::Config => AppColor::new(0.55, 0.75, 0.80),     // Teal
        FileCategory::Font => AppColor::new(0.75, 0.65, 0.85),       // Lavender
        FileCategory::Database => AppColor::new(0.50, 0.60, 0.80),   // Steel blue
        FileCategory::DiskImage => AppColor::new(0.80, 0.55, 0.45),  // Copper
        FileCategory::Other => AppColor::new(0.50, 0.50, 0.55),      // Gray
    }
}

/// Get color for a node based on its extension.
pub fn extension_color(ext: &str) -> AppColor {
    let category = crate::tree::extensions::categorize_extension(ext);
    category_color(category)
}

/// Directory colors are intentionally muted but varied by name hash.
/// This keeps hierarchy readable without making directories all identical gray.
pub fn directory_color(name: &str, depth: u16) -> AppColor {
    let mut h: u32 = 2166136261;
    for &b in name.as_bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    let r = 0.30 + (((h >> 0) & 0xFF) as f32 / 255.0) * 0.20;
    let g = 0.30 + (((h >> 8) & 0xFF) as f32 / 255.0) * 0.20;
    let b = 0.34 + (((h >> 16) & 0xFF) as f32 / 255.0) * 0.18;
    let fade = (depth as f32 * 0.01).min(0.10);
    AppColor::new((r - fade).max(0.20), (g - fade).max(0.20), (b - fade).max(0.22))
}
