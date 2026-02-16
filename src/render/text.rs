use fontdue::layout::{CoordinateSystem, GlyphRasterConfig, Layout, LayoutSettings, TextStyle};
use fontdue::Font;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct TextRenderer {
    fonts: HashMap<String, Font>,
    layout: Layout,
}

impl TextRenderer {
    pub fn new() -> Self {
        Self {
            fonts: HashMap::new(),
            layout: Layout::new(CoordinateSystem::PositiveYDown),
        }
    }

    pub fn add_font(&mut self, name: String, font: Font) {
        self.fonts.insert(name, font);
    }

    pub fn load_system_font(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut candidates: Vec<PathBuf> = Vec::new();

        if let Ok(windir) = std::env::var("WINDIR") {
            candidates.push(PathBuf::from(format!("{windir}\\Fonts\\segoeui.ttf")));
            candidates.push(PathBuf::from(format!("{windir}\\Fonts\\arial.ttf")));
        }

        // Native Windows paths
        candidates.push(PathBuf::from("C:\\Windows\\Fonts\\segoeui.ttf"));
        candidates.push(PathBuf::from("C:\\Windows\\Fonts\\arial.ttf"));

        // WSL/Linux fallback paths (for Linux builds scanning Windows drives)
        candidates.push(PathBuf::from("/mnt/c/Windows/Fonts/segoeui.ttf"));
        candidates.push(PathBuf::from("/mnt/c/Windows/Fonts/arial.ttf"));
        candidates.push(PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"));
        candidates.push(PathBuf::from("/usr/share/fonts/TTF/DejaVuSans.ttf"));

        for path in candidates {
            let Ok(font_data) = std::fs::read(&path) else {
                continue;
            };
            if let Ok(font) = Font::from_bytes(font_data, fontdue::FontSettings::default()) {
                self.fonts.insert(name.to_string(), font);
                tracing::info!("Loaded text font from {}", path.display());
                return Ok(());
            }
        }

        Err("unable to load a system font from known locations".into())
    }

    pub fn render_text(
        &mut self,
        text: &str,
        font_name: &str,
        font_size: f32,
        max_width: Option<f32>,
    ) -> Option<TextRenderResult> {
        let font = self.fonts.get(font_name)?;

        // Reset layout
        self.layout.reset(&LayoutSettings {
            max_width,
            ..Default::default()
        });

        // Add text to layout
        self.layout.append(
            &[font],
            &TextStyle::new(text, font_size, 0), // font_index should be usize
        );

        // Rasterize glyphs
        let mut glyphs = Vec::new();
        let mut width: f32 = 0.0;
        let mut height: f32 = 0.0;

        for glyph in self.layout.glyphs() {
            let (metrics, bitmap) = font.rasterize_config(GlyphRasterConfig {
                glyph_index: glyph.key.glyph_index,
                px: font_size,
                font_hash: 0, // Use 0 for now since font.hash() is private
            });

            // Convert grayscale bitmap to RGBA (white text with alpha)
            let mut rgba_bitmap = Vec::with_capacity(bitmap.len() * 4);
            for &gray in &bitmap {
                rgba_bitmap.push(255); // R
                rgba_bitmap.push(255); // G  
                rgba_bitmap.push(255); // B
                rgba_bitmap.push(gray); // A
            }

            let glyph_data = TextGlyph {
                x: glyph.x,
                y: glyph.y,
                width: metrics.width,
                height: metrics.height,
                bitmap: rgba_bitmap,
            };

            glyphs.push(glyph_data);
            
            // Update bounds
            let right = glyph.x + metrics.width as f32;
            let bottom = glyph.y + metrics.height as f32;
            width = width.max(right);
            height = height.max(bottom);
        }

        if glyphs.is_empty() {
            return None;
        }

        Some(TextRenderResult {
            glyphs,
            width: width.ceil() as u32,
            height: height.ceil() as u32,
        })
    }
}

pub struct TextRenderResult {
    pub glyphs: Vec<TextGlyph>,
    pub width: u32,
    pub height: u32,
}

pub struct TextGlyph {
    pub x: f32,
    pub y: f32,
    pub width: usize,
    pub height: usize,
    pub bitmap: Vec<u8>, // RGBA format
}
