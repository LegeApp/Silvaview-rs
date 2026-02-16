use std::path::PathBuf;
use std::sync::mpsc;

use vello::peniko::Image;
use vello::Scene;

use crate::layout::{self, Layout, LayoutConfig};
use crate::render::cushion::{self, CushionConfig};
use crate::render::scene::{build_scene, image_from_rgba, LabelHitRegion};
use crate::render::text::TextRenderer;
use crate::scanner;
use crate::scanner::types::ScanProgress;
use crate::tree::arena::{FileTree, NodeId};
use crate::ui::input::MouseState;
use crate::ui::navigation::NavigationState;
use crate::ui::overlay::Analytics;

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
    pub text_renderer: TextRenderer,

    // UI state
    pub navigation: Option<NavigationState>,
    pub mouse: MouseState,
    pub hover_node: Option<NodeId>,
    pub analytics: Analytics,
    pub show_analytics_panel: bool,
    pub show_text_labels: bool,
    pub label_hit_regions: Vec<LabelHitRegion>,

    // Rendering
    pub scene: Scene,
    pub needs_relayout: bool,
    pub viewport_width: f32,
    pub viewport_height: f32,
    /// Cached CPU-rasterized treemap image (only rebuilt on layout changes).
    pub cached_treemap_image: Option<Image>,
}

impl App {
    pub fn new(scan_path: PathBuf) -> Self {
        let mut text_renderer = TextRenderer::new();
        // Try to load a system font
        if let Err(_) = text_renderer.load_system_font("default") {
            tracing::warn!("Failed to load system font, text labels will not be available");
        } else {
            tracing::info!("Loaded default system font for text overlays");
        }

        Self {
            phase: AppPhase::WaitingForPath,
            scan_path,
            scan_rx: None,
            scan_progress: None,
            tree: None,
            layout: None,
            layout_config: LayoutConfig::default(),
            cushion_config: CushionConfig::default(),
            text_renderer,
            navigation: None,
            mouse: MouseState::default(),
            hover_node: None,
            analytics: Analytics::default(),
            show_analytics_panel: false,  // Keep analytics panel off by default
            show_text_labels: true,       // Enable constrained labels for orientation
            label_hit_regions: Vec::new(),
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
        let (tx, rx) = mpsc::channel();
        self.scan_rx = Some(rx);

        let path = self.scan_path.clone();
        std::thread::spawn(move || {
            let progress_tx = tx.clone();
            match scanner::scan(&path, scanner::ScanMethod::Auto, progress_tx) {
                Ok(entries) => {
                    let tree = crate::tree::build_tree(&entries);
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

    /// Poll for scan completion. Call this from the event loop.
    pub fn poll_scan(&mut self) -> bool {
        if let Some(rx) = &self.scan_rx {
            // Drain all available messages
            while let Ok(progress) = rx.try_recv() {
                match &progress {
                    ScanProgress::Completed { .. } => {
                        // Check if the tree is ready
                        if let Some(tree) = SCAN_RESULT.lock().unwrap().take() {
                            let root = tree.root;

                            // Validate tree has actual data
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
                            self.needs_relayout = true;
                            self.scan_rx = None;
                            return true;
                        }
                    }
                    _ => {}
                }
                self.scan_progress = Some(progress);
            }
        }
        false
    }

    /// Force a recomputation of the layout for the current viewport.
    pub fn relayout(&mut self) {
        self.cached_treemap_image = None;  // Invalidate cache
        if let (Some(tree), Some(nav)) = (&self.tree, &self.navigation) {
            tracing::info!(
                "Computing layout for tree with {} nodes, root={:?}, viewport={}x{}",
                tree.len(),
                nav.current_root,
                self.viewport_width,
                self.viewport_height
            );

            let computed_layout = layout::compute_layout(
                tree,
                nav.current_root,
                self.viewport_width,
                self.viewport_height,
                &self.layout_config,
            );

            tracing::info!("Layout computed: {} rectangles generated", computed_layout.rects.len());

            // CPU-rasterize the cushion treemap and cache the image
            let w = self.viewport_width as u32;
            let h = self.viewport_height as u32;
            if w > 0 && h > 0 {
                let buf = cushion::rasterize_cushions(
                    w,
                    h,
                    &computed_layout.rects,
                    tree,
                    &self.cushion_config,
                );
                self.cached_treemap_image = Some(image_from_rgba(buf, w, h));
                tracing::info!("Cushion treemap rasterized: {}x{}", w, h);
            }

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
                &mut self.text_renderer,
                self.show_text_labels,
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
        } else {
            self.label_hit_regions.clear();
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

    /// Handle viewport resize.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport_width = width as f32;
        self.viewport_height = height as f32;
        self.needs_relayout = true;
    }

    /// Handle drill-down navigation.
    pub fn drill_down(&mut self, node: NodeId) {
        if let (Some(tree), Some(nav)) = (&self.tree, &mut self.navigation) {
            if nav.drill_down(node, tree) {
                self.needs_relayout = true;
            }
        }
    }

    /// Handle navigate-up.
    pub fn navigate_up(&mut self) {
        if let Some(nav) = &mut self.navigation {
            if nav.navigate_up() {
                self.needs_relayout = true;
            }
        }
    }
}

// Temporary: global scan result for cross-thread communication.
// Will be replaced with proper channel-based approach.
use std::sync::Mutex;
static SCAN_RESULT: std::sync::LazyLock<Mutex<Option<FileTree>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));
