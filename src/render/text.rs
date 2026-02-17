use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use skrifa::instance::Size;
use skrifa::raw::{FileRef, FontRef};
use skrifa::MetadataProvider;
use vello::peniko::{Blob, FontData};
use vello::Glyph;

pub struct TextRenderer {
    fonts: HashMap<String, FontData>,
}

impl TextRenderer {
    pub fn new() -> Self {
        Self {
            fonts: HashMap::new(),
        }
    }

    pub fn add_font(&mut self, name: String, font: FontData) {
        self.fonts.insert(name, font);
    }

    pub fn load_system_font(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut candidates: Vec<PathBuf> = Vec::new();

        if let Ok(windir) = std::env::var("WINDIR") {
            candidates.push(PathBuf::from(format!("{windir}\\Fonts\\segoeui.ttf")));
            candidates.push(PathBuf::from(format!("{windir}\\Fonts\\arial.ttf")));
        }

        candidates.push(PathBuf::from("C:\\Windows\\Fonts\\segoeui.ttf"));
        candidates.push(PathBuf::from("C:\\Windows\\Fonts\\arial.ttf"));
        candidates.push(PathBuf::from("/mnt/c/Windows/Fonts/segoeui.ttf"));
        candidates.push(PathBuf::from("/mnt/c/Windows/Fonts/arial.ttf"));
        candidates.push(PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"));
        candidates.push(PathBuf::from("/usr/share/fonts/TTF/DejaVuSans.ttf"));

        for path in candidates {
            let Ok(font_data) = std::fs::read(&path) else {
                continue;
            };
            let blob = Blob::new(Arc::new(font_data));
            let font = FontData::new(blob, 0);
            if to_font_ref(&font).is_some() {
                self.fonts.insert(name.to_string(), font);
                tracing::info!("Loaded text font from {}", path.display());
                return Ok(());
            }
        }

        Err("unable to load a system font from known locations".into())
    }

    pub fn load_font_from_path(
        &mut self,
        name: &str,
        path: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let font_data = std::fs::read(path)?;
        let font = FontData::new(Blob::new(Arc::new(font_data)), 0);
        if to_font_ref(&font).is_none() {
            return Err("failed to parse font".into());
        }
        self.fonts.insert(name.to_string(), font);
        tracing::info!("Loaded custom text font from {}", path.display());
        Ok(())
    }

    pub fn render_text(
        &mut self,
        text: &str,
        font_name: &str,
        font_size: f32,
        max_width: Option<f32>,
    ) -> Option<TextRenderResult> {
        let font = self.fonts.get(font_name)?.clone();
        let font_ref = to_font_ref(&font)?;
        let axes = font_ref.axes();
        let var_loc = axes.location(std::iter::empty::<(&str, f32)>());
        let size = Size::new(font_size.max(1.0));
        let glyph_metrics = font_ref.glyph_metrics(size, &var_loc);
        let metrics = font_ref.metrics(size, &var_loc);
        let line_height = (metrics.ascent - metrics.descent + metrics.leading).max(font_size * 1.1);
        let baseline = metrics.ascent.max(font_size * 0.75);
        let charmap = font_ref.charmap();

        let mut glyphs = Vec::with_capacity(text.chars().count());
        let mut pen_x = 0.0_f32;
        let mut pen_y = 0.0_f32;
        let mut max_x = 0.0_f32;
        let mut max_y = line_height;
        let width_limit = max_width.unwrap_or(f32::INFINITY).max(0.0);

        for ch in text.chars() {
            if ch == '\n' {
                pen_x = 0.0;
                pen_y += line_height;
                max_y = max_y.max(pen_y + line_height);
                continue;
            }

            let gid = charmap.map(ch).unwrap_or_default();
            let advance = glyph_metrics
                .advance_width(gid)
                .unwrap_or(font_size * 0.5)
                .max(0.0);

            if pen_x > 0.0 && pen_x + advance > width_limit {
                break;
            }

            glyphs.push(Glyph {
                id: gid.to_u32(),
                x: pen_x,
                y: pen_y + baseline,
            });
            pen_x += advance;
            max_x = max_x.max(pen_x);
        }

        if glyphs.is_empty() {
            return None;
        }

        Some(TextRenderResult {
            font,
            font_size: font_size.max(1.0),
            glyphs,
            width: max_x.ceil() as u32,
            height: max_y.ceil() as u32,
        })
    }
}

pub struct TextRenderResult {
    pub font: FontData,
    pub font_size: f32,
    pub glyphs: Vec<Glyph>,
    pub width: u32,
    pub height: u32,
}

fn to_font_ref(font: &FontData) -> Option<FontRef<'_>> {
    let file_ref = FileRef::new(font.data.as_ref()).ok()?;
    match file_ref {
        FileRef::Font(f) => Some(f),
        FileRef::Collection(c) => c.get(font.index).ok(),
    }
}
