use vello::kurbo::{Affine, Circle, Rect};
use vello::peniko::{Brush, Color, Fill};
use vello::Scene;

use crate::render::colors::{mode_name, ColorSettings};
use crate::render::text::{TextRenderResult, TextRenderer};
use crate::tree::arena::{FileTree, NodeId};
use crate::tree::extensions::FileCategory;
use crate::ui::drives::DriveEntry;
use crate::ui::tooltip::{SelectionInfo, TooltipInfo};

/// Analytics data for the file type breakdown panel.
#[derive(Debug, Default)]
pub struct Analytics {
    /// Total bytes per category
    pub category_sizes: Vec<(FileCategory, u64)>,
    /// Total size of all files
    pub total_size: u64,
}

#[derive(Debug, Clone)]
pub enum SidebarHitId {
    SelectDrive(std::path::PathBuf),
    CycleColorMode,
    VibrancyDown,
    VibrancyUp,
    VibrancyTrack,
    ToggleHoverInfo,
}

#[derive(Debug, Clone)]
pub struct SidebarHitRegion {
    pub id: SidebarHitId,
    pub bounds: [f32; 4],
}

pub fn sidebar_panel_bounds(viewport_height: f32, drive_count: usize) -> [f32; 4] {
    let visible_drives = drive_count.min(12);
    let panel_h = sidebar_height(visible_drives).min((viewport_height - 8.0).max(32.0));
    [8.0, 8.0, 196.0, 8.0 + panel_h]
}

pub fn vibrancy_value_from_track_x(x: f32, track: [f32; 4]) -> f32 {
    let t = ((x - track[0]) / (track[2] - track[0]).max(1.0)).clamp(0.0, 1.0);
    0.6 + t * (2.0 - 0.6)
}

/// Compute analytics for the current view.
pub fn compute_analytics(tree: &FileTree, root: NodeId) -> Analytics {
    let mut category_map = std::collections::HashMap::new();
    let mut total_size = 0u64;

    // Traverse all descendants
    let mut stack = vec![root];
    while let Some(node_id) = stack.pop() {
        let node = tree.get(node_id);

        if !node.is_dir {
            *category_map.entry(node.category).or_insert(0u64) += node.size;
            total_size += node.size;
        }

        // Add children to stack
        for child_id in tree.children(node_id) {
            stack.push(child_id);
        }
    }

    // Sort by size descending
    let mut category_sizes: Vec<_> = category_map.into_iter().collect();
    category_sizes.sort_by(|a, b| b.1.cmp(&a.1));

    Analytics {
        category_sizes,
        total_size,
    }
}

/// Render the analytics panel on the right side.
pub fn render_analytics_panel(
    scene: &mut Scene,
    analytics: &Analytics,
    viewport_width: f32,
    viewport_height: f32,
) {
    let panel_width = 250.0;
    let panel_x = viewport_width - panel_width;

    // Semi-transparent dark background
    let bg_rect = Rect::new(
        panel_x as f64,
        0.0,
        viewport_width as f64,
        viewport_height as f64,
    );
    let bg_brush = Brush::Solid(Color::new([0.1, 0.1, 0.12, 0.9]));
    scene.fill(Fill::NonZero, Affine::IDENTITY, &bg_brush, None, &bg_rect);

    // TODO: Add text rendering using parley or a simple glyph renderer
    // For now, just draw colored bars for each category

    let bar_start_y = 40.0;
    let bar_height = 24.0;
    let bar_spacing = 4.0;
    let bar_max_width = panel_width - 40.0;

    for (i, (category, size)) in analytics.category_sizes.iter().enumerate() {
        let y = bar_start_y + (i as f32) * (bar_height + bar_spacing);
        if y + bar_height > viewport_height {
            break;
        }

        let percentage = if analytics.total_size > 0 {
            (*size as f64) / (analytics.total_size as f64)
        } else {
            0.0
        };
        let bar_width = (percentage * bar_max_width as f64) as f32;

        // Category color bar
        let bar_rect = Rect::new(
            (panel_x + 20.0) as f64,
            y as f64,
            (panel_x + 20.0 + bar_width) as f64,
            (y + bar_height) as f64,
        );
        let color = crate::render::colors::category_color(*category);
        let bar_brush = Brush::Solid(color.to_peniko());
        scene.fill(Fill::NonZero, Affine::IDENTITY, &bar_brush, None, &bar_rect);
    }
}

/// Render hover tooltip for a file.
pub fn render_hover_popup(
    scene: &mut Scene,
    text_renderer: &mut TextRenderer,
    info: &TooltipInfo,
    anchor: [f32; 2],
    viewport_width: f32,
    viewport_height: f32,
    copied: bool,
) -> [f32; 4] {
    let panel_w = (viewport_width * 0.30).clamp(300.0, 420.0);
    let panel_h = 116.0;
    let offset = 18.0;
    let panel_x = if anchor[0] + panel_w + offset <= viewport_width - 12.0 {
        anchor[0] + offset
    } else {
        (anchor[0] - panel_w - offset).max(212.0)
    };
    let panel_y = (anchor[1] + 14.0).clamp(12.0, viewport_height - panel_h - 12.0);
    let panel = Rect::new(
        panel_x as f64,
        panel_y as f64,
        (panel_x + panel_w) as f64,
        (panel_y + panel_h) as f64,
    );
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &Color::new([0.05, 0.07, 0.09, 0.94]),
        None,
        &panel,
    );

    let accent = Rect::new(
        panel_x as f64,
        panel_y as f64,
        (panel_x + 4.0) as f64,
        (panel_y + panel_h) as f64,
    );
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &Color::new([0.24, 0.72, 1.0, 0.96]),
        None,
        &accent,
    );

    let title = format!(
        "{}: {}",
        if info.is_dir { "Folder" } else { "File" },
        info.name
    );
    draw_label_with_width(
        scene,
        text_renderer,
        &title,
        panel_x + 16.0,
        panel_y + 12.0,
        panel_w - 32.0,
    );
    let meta = if let Some(child_count) = info.child_count {
        format!("{}  |  {} items", info.size_display, child_count)
    } else {
        format!("{}  |  {}", info.category, info.size_display)
    };
    draw_label_with_width(
        scene,
        text_renderer,
        &meta,
        panel_x + 16.0,
        panel_y + 34.0,
        panel_w - 32.0,
    );
    draw_label_with_width(
        scene,
        text_renderer,
        &info.full_path,
        panel_x + 16.0,
        panel_y + 58.0,
        panel_w - 32.0,
    );
    let action = if copied {
        "Path copied"
    } else {
        "Click to copy path  |  Double-click to open"
    };
    draw_label_with_width(
        scene,
        text_renderer,
        action,
        panel_x + 16.0,
        panel_y + 86.0,
        panel_w - 32.0,
    );

    [panel_x, panel_y, panel_x + panel_w, panel_y + panel_h]
}

/// Render breadcrumb navigation path at the top.
pub fn render_breadcrumb(
    scene: &mut Scene,
    tree: &FileTree,
    current_root: NodeId,
    viewport_width: f32,
) {
    let breadcrumb_height = 32.0;

    // Background bar
    let bg_rect = Rect::new(0.0, 0.0, viewport_width as f64, breadcrumb_height as f64);
    let bg_brush = Brush::Solid(Color::new([0.12, 0.12, 0.14, 0.85]));
    scene.fill(Fill::NonZero, Affine::IDENTITY, &bg_brush, None, &bg_rect);

    // Build path
    let _path = crate::ui::tooltip::build_path(tree, current_root);

    // TODO: Render text path using parley
    // For Phase 2, we'll keep it simple without text initially
}

pub fn render_left_sidebar(
    scene: &mut Scene,
    text_renderer: &mut TextRenderer,
    viewport_height: f32,
    drives: &[DriveEntry],
    selected_scan_path: &std::path::Path,
    color_settings: &ColorSettings,
    show_hover_info: bool,
) -> Vec<SidebarHitRegion> {
    let [x1, y1, x2, y2] = sidebar_panel_bounds(viewport_height, drives.len());
    let visible_drives = drives.len().min(12);
    let mut hits = Vec::new();
    let panel = Rect::new(x1 as f64, y1 as f64, x2 as f64, y2 as f64);
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &Color::new([0.10, 0.11, 0.13, 0.86]),
        None,
        &panel,
    );

    let mut y = y1 + 8.0;
    draw_label(scene, text_renderer, "Drives", 14.0, y);
    y += 22.0;

    let selected = selected_scan_path.to_string_lossy().to_lowercase();
    for drive in drives.iter().take(visible_drives) {
        let row_h = 26.0_f32;
        let bx1 = 10.0_f32;
        let bx2 = x2 - 10.0;
        let by1 = y;
        let by2 = y + row_h;
        let path_s = drive.path.to_string_lossy().to_lowercase();
        let active = selected.starts_with(&path_s);
        let fill = if active {
            Color::new([0.23, 0.30, 0.42, 0.86])
        } else {
            Color::new([0.16, 0.17, 0.20, 0.70])
        };
        let r = Rect::new(bx1 as f64, by1 as f64, bx2 as f64, by2 as f64);
        scene.fill(Fill::NonZero, Affine::IDENTITY, &fill, None, &r);
        draw_label_centered(
            scene,
            text_renderer,
            &drive.label,
            bx1 + 8.0,
            by1,
            14.0,
            row_h,
        );
        hits.push(SidebarHitRegion {
            id: SidebarHitId::SelectDrive(drive.path.clone()),
            bounds: [bx1, by1, bx2, by2],
        });
        y += row_h + 6.0;
    }

    y += 8.0;
    draw_label(scene, text_renderer, "Appearance", 14.0, y);
    y += 24.0;

    let mode_text = format!("Mode: {}", mode_name(color_settings.mode));
    let mode_r = Rect::new(10.0, y as f64, (x2 - 10.0) as f64, (y + 28.0) as f64);
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &Color::new([0.16, 0.17, 0.20, 0.78]),
        None,
        &mode_r,
    );
    draw_label(scene, text_renderer, &mode_text, 18.0, y + 7.0);
    hits.push(SidebarHitRegion {
        id: SidebarHitId::CycleColorMode,
        bounds: [10.0, y, x2 - 10.0, y + 28.0],
    });
    y += 38.0;

    draw_label(scene, text_renderer, "Vibrancy", 14.0, y);
    let vib_text = format!("{:.2}", color_settings.vibrancy);
    draw_label(scene, text_renderer, &vib_text, x2 - 70.0, y);
    y += 18.0;
    let minus = Rect::new(10.0, y as f64, 42.0, (y + 26.0) as f64);
    let plus = Rect::new((x2 - 42.0) as f64, y as f64, x2 as f64, (y + 26.0) as f64);
    let track = [50.0_f32, y, x2 - 50.0, y + 26.0];
    let track_rect = Rect::new(
        track[0] as f64,
        track[1] as f64,
        track[2] as f64,
        track[3] as f64,
    );
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &Color::new([0.16, 0.17, 0.20, 0.78]),
        None,
        &minus,
    );
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &Color::new([0.16, 0.17, 0.20, 0.78]),
        None,
        &plus,
    );
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &Color::new([0.20, 0.22, 0.26, 0.86]),
        None,
        &track_rect,
    );
    let t = ((color_settings.vibrancy - 0.6) / (2.0 - 0.6)).clamp(0.0, 1.0);
    let thumb_x = track[0] + (track[2] - track[0]) * t;
    let thumb = Rect::new(
        (thumb_x - 4.0) as f64,
        (y + 2.0) as f64,
        (thumb_x + 4.0) as f64,
        (y + 24.0) as f64,
    );
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &Color::new([0.78, 0.82, 0.92, 0.95]),
        None,
        &thumb,
    );
    draw_label(scene, text_renderer, "-", 24.0, y + 3.0);
    draw_label(scene, text_renderer, "+", x2 - 30.0, y + 3.0);
    hits.push(SidebarHitRegion {
        id: SidebarHitId::VibrancyDown,
        bounds: [10.0, y, 42.0, y + 26.0],
    });
    hits.push(SidebarHitRegion {
        id: SidebarHitId::VibrancyUp,
        bounds: [x2 - 42.0, y, x2, y + 26.0],
    });
    hits.push(SidebarHitRegion {
        id: SidebarHitId::VibrancyTrack,
        bounds: track,
    });
    y += 36.0;

    let hover_r = Rect::new(10.0, y as f64, (x2 - 10.0) as f64, (y + 28.0) as f64);
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &Color::new([0.16, 0.17, 0.20, 0.78]),
        None,
        &hover_r,
    );
    let hover_text = if show_hover_info {
        "Hover Info: On"
    } else {
        "Hover Info: Off"
    };
    draw_label(scene, text_renderer, hover_text, 18.0, y + 7.0);
    hits.push(SidebarHitRegion {
        id: SidebarHitId::ToggleHoverInfo,
        bounds: [10.0, y, x2 - 10.0, y + 28.0],
    });
    hits
}

pub fn render_loading_overlay(
    scene: &mut Scene,
    text_renderer: &mut TextRenderer,
    viewport_width: f32,
    viewport_height: f32,
    elapsed_seconds: f32,
    show_admin_warning: bool,
) {
    let panel_w = (viewport_width * 0.54).clamp(420.0, 760.0);
    let panel_h = if show_admin_warning { 126.0 } else { 92.0 };
    let x = (viewport_width - panel_w) * 0.5;
    let y = (viewport_height - panel_h) * 0.5;
    let panel = Rect::new(
        x as f64,
        y as f64,
        (x + panel_w) as f64,
        (y + panel_h) as f64,
    );

    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &Color::new([0.07, 0.08, 0.10, 0.84]),
        None,
        &panel,
    );

    // Center-justified loading line with spinner directly above it.
    let text_result = text_renderer.render_text(
        "Loading drive data...",
        "default",
        14.0,
        Some(panel_w - 32.0),
    );
    let text_y = if let Some(rendered) = text_result {
        let tx = x + ((panel_w - rendered.width as f32) * 0.5).max(16.0);
        let ty = y + 47.0;
        draw_text(scene, rendered, tx, ty);
        ty
    } else {
        y + 47.0
    };

    let spinner_cx = x + panel_w * 0.5;
    let spinner_cy = text_y - 16.0;
    let spinner_r = 7.0;
    let step = ((elapsed_seconds * 10.0) as i32).rem_euclid(12) as usize;
    for i in 0..12usize {
        let angle = (i as f32 / 12.0) * std::f32::consts::TAU;
        let px = spinner_cx + angle.cos() * spinner_r;
        let py = spinner_cy + angle.sin() * spinner_r;
        let dist = ((12 + i as i32 - step as i32) % 12) as f32;
        let alpha = (1.0 - dist / 12.0) * 0.9 + 0.08;
        let dot = Circle::new((px as f64, py as f64), 1.7);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &Color::new([0.88, 0.90, 0.95, alpha]),
            None,
            &dot,
        );
    }

    if show_admin_warning {
        draw_label_with_width(
            scene,
            text_renderer,
            "Program not started with administrator permissions, loading will be 10x slower.",
            x + 14.0,
            text_y + 28.0,
            panel_w - 32.0,
        );
    }
}

pub fn render_selection_popup(
    scene: &mut Scene,
    text_renderer: &mut TextRenderer,
    info: &SelectionInfo,
    anchor: [f32; 2],
    viewport_width: f32,
    viewport_height: f32,
) -> [f32; 4] {
    let panel_w = (viewport_width * 0.30).clamp(300.0, 420.0);
    let mut panel_h = 150.0;
    if info.child_count.is_some() {
        panel_h += 18.0;
    }
    let cursor_offset = 18.0;
    let panel_x = if anchor[0] + panel_w + cursor_offset <= viewport_width - 12.0 {
        anchor[0] + cursor_offset
    } else {
        (anchor[0] - panel_w - cursor_offset).max(212.0)
    };
    let panel_y = (anchor[1] + 14.0).clamp(12.0, viewport_height - panel_h - 12.0);
    let panel = Rect::new(
        panel_x as f64,
        panel_y as f64,
        (panel_x + panel_w) as f64,
        (panel_y + panel_h) as f64,
    );

    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &Color::new([0.05, 0.07, 0.09, 0.92]),
        None,
        &panel,
    );

    let accent = Rect::new(
        panel_x as f64,
        panel_y as f64,
        (panel_x + 4.0) as f64,
        (panel_y + panel_h) as f64,
    );
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &Color::new([0.24, 0.72, 1.0, 0.96]),
        None,
        &accent,
    );

    let title = if info.is_dir {
        "Selected Folder"
    } else {
        "Selected File"
    };
    draw_label(scene, text_renderer, title, panel_x + 16.0, panel_y + 14.0);
    draw_label_with_width(
        scene,
        text_renderer,
        &info.name,
        panel_x + 16.0,
        panel_y + 36.0,
        panel_w - 32.0,
    );

    let mut y = panel_y + 60.0;
    let meta = if let Some(child_count) = info.child_count {
        format!(
            "{}  |  {}  |  {} items",
            info.category, info.size_display, child_count
        )
    } else {
        format!("{}  |  {}", info.category, info.size_display)
    };
    draw_label_with_width(
        scene,
        text_renderer,
        &meta,
        panel_x + 16.0,
        y,
        panel_w - 32.0,
    );
    y += 20.0;

    let modified = format!("Modified: {}", info.modified_display);
    draw_label_with_width(
        scene,
        text_renderer,
        &modified,
        panel_x + 16.0,
        y,
        panel_w - 32.0,
    );
    y += 24.0;

    draw_label(scene, text_renderer, "Directory", panel_x + 16.0, y);
    y += 16.0;
    draw_label_with_width(
        scene,
        text_renderer,
        &info.directory_path,
        panel_x + 16.0,
        y,
        panel_w - 32.0,
    );
    y += 28.0;

    draw_label(scene, text_renderer, "Path", panel_x + 16.0, y);
    y += 16.0;
    draw_label_with_width(
        scene,
        text_renderer,
        &info.full_path,
        panel_x + 16.0,
        y,
        panel_w - 32.0,
    );

    [panel_x, panel_y, panel_x + panel_w, panel_y + panel_h]
}

fn draw_text(scene: &mut Scene, text_result: TextRenderResult, x: f32, y: f32) {
    let tx = x.round();
    let ty = y.round();
    let transform = Affine::translate((tx as f64, ty as f64));
    scene
        .draw_glyphs(&text_result.font)
        .font_size(text_result.font_size)
        .transform(transform)
        .brush(Color::WHITE)
        .hint(true)
        .draw(
            Fill::NonZero,
            text_result.glyphs.into_iter().map(|mut glyph| {
                glyph.x = glyph.x.round();
                glyph.y = glyph.y.round();
                glyph
            }),
        );
}

fn draw_label(scene: &mut Scene, text_renderer: &mut TextRenderer, text: &str, x: f32, y: f32) {
    draw_label_with_width(scene, text_renderer, text, x, y, 210.0);
}

fn draw_label_with_width(
    scene: &mut Scene,
    text_renderer: &mut TextRenderer,
    text: &str,
    x: f32,
    y: f32,
    max_width: f32,
) {
    if let Some(rendered) = text_renderer.render_text(text, "default", 14.0, Some(max_width)) {
        draw_text(scene, rendered, x, y);
    }
}

fn sidebar_height(visible_drives: usize) -> f32 {
    let drives_h = visible_drives as f32 * (26.0 + 6.0);
    // Header + section padding + appearance controls + hover toggle.
    14.0 + 22.0 + drives_h + 8.0 + 24.0 + 38.0 + 18.0 + 36.0 + 36.0 + 38.0 + 8.0
}

fn draw_label_centered(
    scene: &mut Scene,
    text_renderer: &mut TextRenderer,
    text: &str,
    x: f32,
    row_y: f32,
    font_size: f32,
    row_h: f32,
) {
    if let Some(rendered) = text_renderer.render_text(text, "default", font_size, Some(210.0)) {
        let y = row_y + ((row_h - rendered.height as f32) * 0.5).max(0.0);
        draw_text(scene, rendered, x, y);
    }
}
