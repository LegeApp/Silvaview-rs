use fontdue::layout::{Layout, CoordinateSystem, GlyphRasterConfig, TextStyle};
use fontdue::Font;
use std::collections::HashMap;
use std::hash::Hash;

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
        // Try to load a common system font
        let font_data = std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf")?;
        let font = Font::from_bytes(font_data, fontdue::FontSettings::default())?;
        self.fonts.insert(name.to_string(), font);
        Ok(())
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
        self.layout.reset(&fontdue::layout::LayoutSettings {
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
