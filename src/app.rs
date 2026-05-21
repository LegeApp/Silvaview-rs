use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::time::Instant;

use vello::peniko::ImageData;
use vello::Scene;

use crate::layout::{self, Layout, LayoutConfig};
use crate::render::colors::ColorSettings;
use crate::render::cushion::CushionConfig;
use crate::render::scene::{build_scene, LabelHitRegion};
use crate::render::text::TextRenderer;
use crate::scanner;
use crate::scanner::types::ScanProgress;
use crate::tree::arena::{FileTree, NodeId};
use crate::ui::input::MouseState;
use crate::ui::navigation::NavigationState;
use crate::ui::overlay::{Analytics, SidebarHitId, SidebarHitRegion};
use crate::ui::tooltip::SelectionInfo;

/// Application state machine phases.
#[derive(Debug, PartialEq, Eq)]
pub enum AppPhase {
    /// Waiting for user to select a path to scan
    WaitingForPath,
    /// Scanning the filesystem
    Scanning,
    /// Ready to render the treemap
    Ready,
}

/// Top-level application state.
pub struct App {
    pub phase: AppPhase,
    pub scan_path: PathBuf,

    // Scan state
    pub scan_progress: Option<ScanProgress>,
    scan_rx: Option<mpsc::Receiver<ScanProgress>>,

    // Data
    pub tree: Option<FileTree>,
    pub layout: Option<Layout>,
    pub layout_config: LayoutConfig,
    pub cushion_config: CushionConfig,
    pub color_settings: ColorSettings,
    pub text_renderer: TextRenderer,

    // UI state
    pub navigation: Option<NavigationState>,
    pub mouse: MouseState,
    pub hover_node: Option<NodeId>,
    pub selected_node: Option<NodeId>,
    pub selected_info: Option<SelectionInfo>,
    pub selected_anchor: Option<[f32; 2]>,
    pub selected_popup_bounds: Option<[f32; 4]>,
    pub locked_path: Option<String>,
    clipboard: Option<arboard::Clipboard>,
    pub analytics: Analytics,
    pub show_analytics_panel: bool,
    pub show_text_labels: bool,
    pub label_font_scale: f32,
    pub label_font_path: String,
    pub label_hit_regions: Vec<LabelHitRegion>,
    pub sidebar_hit_regions: Vec<SidebarHitRegion>,
    pub available_drives: Vec<crate::ui::drives::DriveEntry>,
    pub show_hover_info: bool,
    pub vibrancy_dragging: bool,
    pub show_admin_slow_warning: bool,
    pub loading_started: Option<Instant>,

    // Rendering
    pub scene: Scene,
    pub needs_relayout: bool,
    pub viewport_width: f32,
    pub viewport_height: f32,
    /// Cached CPU-rasterized treemap image (only rebuilt on layout changes).
    pub cached_treemap_image: Option<ImageData>,
}

impl App {
    pub fn new(scan_path: PathBuf) -> Self {
        let mut text_renderer = TextRenderer::new();
        if text_renderer.load_system_font("default").is_ok() {
            tracing::info!("Loaded default system font for text overlays");
        } else {
            tracing::warn!("Failed to load system font, text labels will not be available");
        }

        Self {
            phase: AppPhase::WaitingForPath,
            scan_path: scan_path.clone(),
            scan_rx: None,
            scan_progress: None,
            tree: None,
            layout: None,
            layout_config: LayoutConfig::default(),
            cushion_config: CushionConfig::default(),
            color_settings: ColorSettings::default(),
            text_renderer,
            navigation: None,
            mouse: MouseState::default(),
            hover_node: None,
            selected_node: None,
            selected_info: None,
            selected_anchor: None,
            selected_popup_bounds: None,
            locked_path: None,
            clipboard: None,
            analytics: Analytics::default(),
            show_analytics_panel: false, // Keep analytics panel off by default
            show_text_labels: true,      // Enable constrained labels for orientation
            label_font_scale: 1.0,
            label_font_path: String::new(),
            label_hit_regions: Vec::new(),
            sidebar_hit_regions: Vec::new(),
            available_drives: crate::ui::drives::enumerate_drives(),
            show_hover_info: true,
            vibrancy_dragging: false,
            show_admin_slow_warning: false,
            loading_started: None,
            scene: Scene::new(),
            needs_relayout: true,
            viewport_width: 800.0,
            viewport_height: 600.0,
            cached_treemap_image: None,
        }
    }

    /// Start scanning the filesystem in a background thread.
    pub fn start_scan(&mut self) {
        self.phase = AppPhase::Scanning;
        self.loading_started = Some(Instant::now());
        #[cfg(windows)]
        {
            let is_root = self
                .scan_path
                .to_str()
                .map(|s| s.ends_with(":\\"))
                .unwrap_or(false);
            self.show_admin_slow_warning = is_root && !crate::scanner::elevation::is_elevated();
        }
        #[cfg(not(windows))]
        {
            self.show_admin_slow_warning = false;
        }
        let (tx, rx) = mpsc::channel();
        self.scan_rx = Some(rx);

        let path = self.scan_path.clone();
        std::thread::spawn(move || {
            let progress_tx = tx.clone();
            match scanner::scan(&path, scanner::ScanMethod::Auto, progress_tx) {
                Ok(tree) => {
                    tracing::info!("Tree built: {} nodes", tree.len());
                    // Send a final completion signal with the tree
                    // (We'll send the tree via a separate channel in a real impl;
                    //  for now we serialize through progress)
                    let _ = tx.send(ScanProgress::Completed {
                        total_files: tree.len() as u64,
                        total_dirs: 0,
                        total_bytes: tree.get(tree.root).size,
                        elapsed_ms: 0,
                    });

                    // Store tree — in production we'd use a shared Arc<Mutex<>>
                    // For now, we'll use a different approach in the actual event loop
                    SCAN_RESULT.lock().unwrap().replace(tree);
                }
                Err(e) => {
                    tracing::error!("Scan failed: {}", e);
                    let _ = tx.send(ScanProgress::Error {
                        path,
                        message: e.to_string(),
                    });
                }
            }
        });
    }

    /// Start scanning a new path (resets current tree/layout state).
    pub fn start_scan_path(&mut self, path: PathBuf) {
        self.scan_path = path.clone();
        self.tree = None;
        self.layout = None;
        self.navigation = None;
        self.hover_node = None;
        self.selected_node = None;
        self.selected_info = None;
        self.selected_anchor = None;
        self.selected_popup_bounds = None;
        self.locked_path = None;
        self.cached_treemap_image = None;
        self.label_hit_regions.clear();
        self.sidebar_hit_regions.clear();
        self.scan_progress = None;
        self.needs_relayout = true;
        self.start_scan();
    }

    /// Poll for scan completion. Call this from the event loop.
    pub fn poll_scan(&mut self) -> bool {
        if let Some(rx) = &self.scan_rx {
            // Drain all available messages
            while let Ok(progress) = rx.try_recv() {
                if matches!(progress, ScanProgress::Completed { .. }) {
                    if let Some(tree) = SCAN_RESULT.lock().unwrap().take() {
                        let root = tree.root;

                        if tree.len() <= 1 {
                            tracing::error!(
                                "Scan returned no files! Possible causes:\n\
                                 - Scanning C:\\ requires Administrator privileges\n\
                                 - MFT access denied\n\
                                 - Try scanning a different directory (e.g., D:\\Rust-projects)\n\
                                 - Or run as Administrator"
                            );
                        } else {
                            tracing::info!("Tree built: {} nodes", tree.len());
                        }

                        self.tree = Some(tree);
                        self.navigation = Some(NavigationState::new(root));
                        self.phase = AppPhase::Ready;
                        self.loading_started = None;
                        self.needs_relayout = true;
                        self.scan_rx = None;
                        return true;
                    }
                }
                self.scan_progress = Some(progress);
            }
        }
        false
    }

    /// Force a recomputation of the layout for the current viewport.
    pub fn relayout(&mut self) {
        if let (Some(tree), Some(nav)) = (&self.tree, &self.navigation) {
            let [tx, ty, tw, th] = self.treemap_layout_rect();
            let exclusion = self.sidebar_exclusion_rect();
            tracing::info!(
                "Computing layout for tree with {} nodes, root={:?}, viewport={}x{}, treemap={}x{}@{},{} exclusion={:?}",
                tree.len(),
                nav.current_root,
                self.viewport_width,
                self.viewport_height,
                tw,
                th,
                tx,
                ty,
                exclusion
            );

            let computed_layout = layout::compute_layout_lshape(
                tree,
                nav.current_root,
                self.viewport_width,
                self.viewport_height,
                exclusion,
                &self.layout_config,
            );

            tracing::info!(
                "Layout computed: {} rectangles generated",
                computed_layout.rects.len()
            );

            self.layout = Some(computed_layout);

            // Recompute analytics for the current view
            self.analytics = crate::ui::overlay::compute_analytics(tree, nav.current_root);

            self.needs_relayout = false;
        }
    }

    /// Rebuild the Vello scene from the current layout.
    pub fn rebuild_scene(&mut self) {
        if let (Some(tree), Some(layout)) = (&self.tree, &self.layout) {
            self.label_hit_regions = build_scene(
                &mut self.scene,
                self.cached_treemap_image.as_ref(),
                &layout.rects,
                tree,
                self.hover_node,
                self.selected_node,
                &mut self.text_renderer,
                self.show_text_labels,
                self.label_font_scale,
                self.show_hover_info,
            );

            // Add UI overlays
            if self.show_analytics_panel {
                crate::ui::overlay::render_analytics_panel(
                    &mut self.scene,
                    &self.analytics,
                    self.viewport_width,
                    self.viewport_height,
                );
            }

            // DISABLED FOR DEBUGGING - Render tooltip if hovering
            // if let Some(node_id) = self.hover_node {
            //     crate::ui::overlay::render_tooltip(
            //         &mut self.scene,
            //         tree,
            //         node_id,
            //         self.mouse.x,
            //         self.mouse.y,
            //     );
            // }

            // DISABLED FOR DEBUGGING - Breadcrumb at top
            // if let Some(nav) = &self.navigation {
            //     crate::ui::overlay::render_breadcrumb(
            //         &mut self.scene,
            //         tree,
            //         nav.current_root,
            //         self.viewport_width,
            //     );
            // }

            if let Some(info) = &self.selected_info {
                self.selected_popup_bounds = Some(crate::ui::overlay::render_selection_popup(
                    &mut self.scene,
                    &mut self.text_renderer,
                    info,
                    self.selected_anchor.unwrap_or([self.mouse.x, self.mouse.y]),
                    self.viewport_width,
                    self.viewport_height,
                ));
            } else {
                self.selected_popup_bounds = None;
            }
        } else {
            self.scene.reset();
            self.label_hit_regions.clear();
            self.selected_popup_bounds = None;
        }

        let sidebar_path_preview = self.sidebar_path_preview();
        self.sidebar_hit_regions = crate::ui::overlay::render_left_sidebar(
            &mut self.scene,
            &mut self.text_renderer,
            self.viewport_height,
            &self.available_drives,
            &self.scan_path,
            &self.color_settings,
            self.show_hover_info,
            sidebar_path_preview.as_deref(),
            self.locked_path.is_some(),
        );

        if self.phase == AppPhase::Scanning {
            crate::ui::overlay::render_loading_overlay(
                &mut self.scene,
                &mut self.text_renderer,
                self.viewport_width,
                self.viewport_height,
                self.loading_started
                    .map(|t| t.elapsed().as_secs_f32())
                    .unwrap_or(0.0),
                self.show_admin_slow_warning,
            );
        }
    }

    /// Hit-test interactive folder labels (used for label-only drill-down).
    pub fn hit_test_label(&self, x: f32, y: f32) -> Option<NodeId> {
        for region in self.label_hit_regions.iter().rev() {
            let [x1, y1, x2, y2] = region.bounds;
            if x >= x1 && x <= x2 && y >= y1 && y <= y2 {
                return Some(region.node);
            }
        }
        None
    }

    pub fn hit_test_sidebar(&self, x: f32, y: f32) -> Option<SidebarHitId> {
        for region in self.sidebar_hit_regions.iter().rev() {
            let [x1, y1, x2, y2] = region.bounds;
            if x >= x1 && x <= x2 && y >= y1 && y <= y2 {
                return Some(region.id.clone());
            }
        }
        None
    }

    pub fn sidebar_exclusion_rect(&self) -> [f32; 4] {
        crate::ui::overlay::sidebar_panel_bounds(self.viewport_height, self.available_drives.len())
    }

    /// Compute the rectangle available for treemap layout after reserving sidebar space.
    pub fn treemap_layout_rect(&self) -> [f32; 4] {
        let [_sx1, sy1, sx2, sy2] = self.sidebar_exclusion_rect();
        let pad = 8.0;
        let right_x = (sx2 + pad).min(self.viewport_width);
        let right_w = (self.viewport_width - right_x).max(64.0);
        let right_h = self.viewport_height.max(64.0);

        let bottom_y = (sy2 + pad).min(self.viewport_height);
        let bottom_w = self.viewport_width.max(64.0);
        let bottom_h = (self.viewport_height - bottom_y).max(64.0);

        // Prefer reclaiming space below the compact sidebar so we avoid a full-height dead strip.
        let panel_h = (sy2 - sy1).max(0.0);
        if panel_h <= self.viewport_height * 0.75 && bottom_h >= 120.0 {
            [0.0, bottom_y, bottom_w, bottom_h]
        } else if right_w * right_h >= bottom_w * bottom_h {
            [right_x, 0.0, right_w, right_h]
        } else {
            [0.0, bottom_y, bottom_w, bottom_h]
        }
    }

    /// Handle viewport resize.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport_width = width as f32;
        self.viewport_height = height as f32;
        self.needs_relayout = true;
    }

    /// Handle drill-down navigation.
    pub fn drill_down(&mut self, node: NodeId) {
        let did_drill = if let (Some(tree), Some(nav)) = (&self.tree, &mut self.navigation) {
            nav.drill_down(node, tree)
        } else {
            false
        };

        if did_drill {
            self.clear_selection();
            self.needs_relayout = true;
        }
    }

    /// Handle navigate-up.
    pub fn navigate_up(&mut self) {
        let did_navigate = if let Some(nav) = &mut self.navigation {
            nav.navigate_up()
        } else {
            false
        };

        if did_navigate {
            self.clear_selection();
            self.needs_relayout = true;
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_node = None;
        self.selected_info = None;
        self.selected_anchor = None;
        self.selected_popup_bounds = None;
        self.locked_path = None;
    }

    pub fn select_file_node(&mut self, node: NodeId, cursor: [f32; 2]) {
        if let Some(tree) = &self.tree {
            let info = crate::ui::tooltip::build_selection_info(tree, node);
            self.locked_path = Some(info.full_path.clone());
            self.selected_node = Some(node);
            self.selected_info = Some(info);
            self.selected_anchor = Some(cursor);
        }
    }

    pub fn open_selected_in_file_manager(&self) -> bool {
        let (Some(tree), Some(node_id)) = (&self.tree, self.selected_node) else {
            return false;
        };

        let node = tree.get(node_id);
        let full_path = crate::ui::tooltip::build_pathbuf(tree, node_id);
        let directory_path = if node.is_dir {
            full_path.clone()
        } else {
            full_path
                .parent()
                .unwrap_or(full_path.as_path())
                .to_path_buf()
        };

        open_in_file_manager(&full_path, &directory_path, node.is_dir)
    }

    pub fn copy_locked_path_to_clipboard(&mut self) -> bool {
        let Some(path) = self.locked_path.clone() else {
            return false;
        };

        // Retrying helps when the OS clipboard is briefly busy.
        let mut last_err: Option<String> = None;
        for _ in 0..6 {
            if self.clipboard.is_none() {
                self.clipboard = arboard::Clipboard::new().ok();
            }

            if let Some(clipboard) = self.clipboard.as_mut() {
                match clipboard.set_text(path.as_str()) {
                    Ok(()) => return true,
                    Err(e) => {
                        last_err = Some(e.to_string());
                        self.clipboard = None;
                    }
                }
            } else {
                last_err = Some("clipboard backend unavailable".to_string());
            }

            std::thread::sleep(std::time::Duration::from_millis(12));
        }

        if let Some(err) = last_err {
            tracing::warn!("Failed to copy path to clipboard: {}", err);
        }
        false
    }

    pub fn vibrancy_track_bounds(&self) -> Option<[f32; 4]> {
        self.sidebar_hit_regions
            .iter()
            .find(|r| matches!(r.id, crate::ui::overlay::SidebarHitId::VibrancyTrack))
            .map(|r| r.bounds)
    }

    fn sidebar_path_preview(&self) -> Option<String> {
        if let Some(path) = self.locked_path.clone() {
            return Some(path);
        }
        let tree = self.tree.as_ref()?;
        let node = self.hover_node?;
        Some(crate::ui::tooltip::build_path(tree, node))
    }
}

// Temporary: global scan result for cross-thread communication.
// Will be replaced with proper channel-based approach.
use std::sync::Mutex;
static SCAN_RESULT: std::sync::LazyLock<Mutex<Option<FileTree>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

fn open_in_file_manager(
    full_path: &std::path::Path,
    directory_path: &std::path::Path,
    is_dir: bool,
) -> bool {
    #[cfg(target_os = "windows")]
    {
        let status = if is_dir {
            Command::new("explorer").arg(directory_path).status()
        } else {
            let mut select_arg = std::ffi::OsString::from("/select,");
            select_arg.push(full_path.as_os_str());
            Command::new("explorer").arg(select_arg).status()
        };
        return status.map(|s| s.success()).unwrap_or(false);
    }

    #[cfg(target_os = "macos")]
    {
        let status = if is_dir {
            Command::new("open").arg(directory_path).status()
        } else {
            Command::new("open").arg("-R").arg(full_path).status()
        };
        return status.map(|s| s.success()).unwrap_or(false);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return Command::new("xdg-open")
            .arg(directory_path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }

    #[allow(unreachable_code)]
    false
}
