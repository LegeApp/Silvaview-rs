#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod layout;
mod render;
mod scanner;
mod tree;
mod ui;

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{CursorIcon, Window, WindowAttributes, WindowId};

use app::App;
use app::AppPhase;
use render::RenderState;
use ui::input;
use ui::overlay::SidebarHitId;

/// Main application handler for winit's event loop.
struct SilvaViewApp {
    app: App,
    render_state: Option<RenderState>,
    window: Option<Arc<Window>>,
}

impl SilvaViewApp {
    fn new(scan_path: PathBuf) -> Self {
        Self {
            app: App::new(scan_path),
            render_state: None,
            window: None,
        }
    }

    fn update_window_title(&self) {
        let Some(window) = &self.window else {
            return;
        };
        if let (Some(tree), Some(nav)) = (&self.app.tree, &self.app.navigation) {
            let path = ui::tooltip::build_path(tree, nav.current_root);
            window.set_title(&format!("SilvaView-rs — {}", path));
        } else {
            window.set_title("SilvaView-rs — Disk Space Visualizer");
        }
    }
}

impl ApplicationHandler for SilvaViewApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("SilvaView-rs — Disk Space Visualizer")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 800));

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create window"),
        );
        self.window = Some(window.clone());

        // Initialize GPU rendering
        let render_state = pollster::block_on(RenderState::new(window.clone()));
        match render_state {
            Ok(state) => {
                let size = window.inner_size();
                let scale = window.scale_factor();
                tracing::info!(
                    "Window initialized: scale_factor={:.3}, physical_size={}x{}",
                    scale,
                    size.width,
                    size.height
                );
                self.app.viewport_width = size.width as f32;
                self.app.viewport_height = size.height as f32;
                self.app.cached_treemap_image = Some(state.treemap_image().clone());
                self.render_state = Some(state);
                window.request_redraw();
            }
            Err(e) => {
                tracing::error!("Failed to initialize GPU: {}", e);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                if let Some(render) = &mut self.render_state {
                    render.resize(size.width, size.height);
                    self.app.cached_treemap_image = Some(render.treemap_image().clone());
                    self.app.resize(size.width, size.height);
                }
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                tracing::info!("Scale factor changed: {:.3}", scale_factor);
                if let (Some(render), Some(window)) = (&mut self.render_state, &self.window) {
                    let size = window.inner_size();
                    render.resize(size.width, size.height);
                    self.app.cached_treemap_image = Some(render.treemap_image().clone());
                    self.app.resize(size.width, size.height);
                    window.request_redraw();
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.app.mouse.x = position.x as f32;
                self.app.mouse.y = position.y as f32;
                if self.app.vibrancy_dragging {
                    if let Some(track) = self
                        .app
                        .sidebar_hit_regions
                        .iter()
                        .find(|r| matches!(r.id, SidebarHitId::VibrancyTrack))
                        .map(|r| r.bounds)
                    {
                        self.app.color_settings.vibrancy =
                            ui::overlay::vibrancy_value_from_track_x(self.app.mouse.x, track);
                        self.app.needs_relayout = true;
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }
                }

                // Update hover state
                let new_hover = if let Some(layout) = &self.app.layout {
                    input::hit_test(
                        &layout.rects,
                        self.app.mouse.x,
                        self.app.mouse.y,
                    )
                } else {
                    None
                };
                if new_hover != self.app.hover_node {
                    self.app.hover_node = new_hover;
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if button == winit::event::MouseButton::Left {
                    self.app.mouse.left_pressed = state == ElementState::Pressed;
                    if state == ElementState::Released {
                        self.app.vibrancy_dragging = false;
                    }
                }

                if state == ElementState::Pressed && button == winit::event::MouseButton::Left {
                    if let Some(hit) = self.app.hit_test_sidebar(self.app.mouse.x, self.app.mouse.y) {
                        match hit {
                            SidebarHitId::SelectDrive(path) => {
                                self.app.start_scan_path(path);
                                self.update_window_title();
                            }
                            SidebarHitId::CycleColorMode => {
                                use crate::render::colors::ColorMode;
                                self.app.color_settings.mode = match self.app.color_settings.mode {
                                    ColorMode::Category => ColorMode::CategoryExtension,
                                    ColorMode::CategoryExtension => ColorMode::ExtensionHash,
                                    ColorMode::ExtensionHash => ColorMode::Category,
                                };
                                self.app.needs_relayout = true;
                            }
                            SidebarHitId::VibrancyDown => {
                                self.app.color_settings.vibrancy =
                                    (self.app.color_settings.vibrancy - 0.08).clamp(0.6, 2.0);
                                self.app.needs_relayout = true;
                            }
                            SidebarHitId::VibrancyUp => {
                                self.app.color_settings.vibrancy =
                                    (self.app.color_settings.vibrancy + 0.08).clamp(0.6, 2.0);
                                self.app.needs_relayout = true;
                            }
                            SidebarHitId::VibrancyTrack => {
                                if let Some(track) = self
                                    .app
                                    .sidebar_hit_regions
                                    .iter()
                                    .find(|r| matches!(r.id, SidebarHitId::VibrancyTrack))
                                    .map(|r| r.bounds)
                                {
                                    self.app.color_settings.vibrancy =
                                        ui::overlay::vibrancy_value_from_track_x(self.app.mouse.x, track);
                                    self.app.vibrancy_dragging = true;
                                    self.app.needs_relayout = true;
                                }
                            }
                            SidebarHitId::ToggleHoverInfo => {
                                self.app.show_hover_info = !self.app.show_hover_info;
                            }
                        }
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                        return;
                    }

                    if let Some(node) = self.app.hit_test_label(self.app.mouse.x, self.app.mouse.y) {
                        self.app.drill_down(node);
                        self.update_window_title();
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                        return;
                    }

                    // Fallback: allow clicking a directory rectangle to drill down.
                    // Sidebar hit-testing already returned above, so this only applies to treemap tiles.
                    if let (Some(layout), Some(tree)) = (&self.app.layout, &self.app.tree) {
                        if let Some(node) = input::hit_test(&layout.rects, self.app.mouse.x, self.app.mouse.y) {
                            if tree.get(node).is_dir {
                                self.app.drill_down(node);
                                self.update_window_title();
                                if let Some(window) = &self.window {
                                    window.request_redraw();
                                }
                            }
                        }
                    }
                    return;
                }

                let action = if let Some(layout) = &self.app.layout {
                    input::process_mouse_button(
                        button,
                        state,
                        &self.app.mouse,
                        &layout.rects,
                    )
                } else {
                    input::InputAction::None
                };
                self.handle_action(action);
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if matches!(event.logical_key.as_ref(), Key::Named(NamedKey::F2)) {
                        let settings = ui::config_dialog::run_config_dialog(
                            "SilvaView-rs — Settings",
                            ui::config_dialog::DialogResult {
                                scan_path: self.app.scan_path.clone(),
                                layout: self.app.layout_config.clone(),
                                cushion: self.app.cushion_config,
                                show_labels: self.app.show_text_labels,
                                label_font_scale: self.app.label_font_scale,
                                label_font_path: self.app.label_font_path.clone(),
                            },
                            false,
                        );
                        if let Some(settings) = settings {
                            self.app.layout_config = settings.layout;
                            self.app.cushion_config = settings.cushion;
                            self.app.show_text_labels = settings.show_labels;
                            self.app.label_font_scale = settings.label_font_scale;
                            self.app.label_font_path = settings.label_font_path.clone();
                            if !settings.label_font_path.trim().is_empty() {
                                if let Err(e) = self.app.text_renderer.load_font_from_path(
                                    "default",
                                    Path::new(settings.label_font_path.trim()),
                                ) {
                                    tracing::warn!(
                                        "Failed to load custom font '{}': {}",
                                        settings.label_font_path,
                                        e
                                    );
                                }
                            }
                            self.app.needs_relayout = true;
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                        return;
                    }

                    let action = input::process_key(event.logical_key.clone(), event.state);
                    self.handle_action(action);
                }
            }

            WindowEvent::RedrawRequested => {
                if let Some(window) = &self.window {
                    if self.app.phase == app::AppPhase::Scanning {
                        window.set_cursor(CursorIcon::Progress);
                    } else {
                        window.set_cursor(CursorIcon::Default);
                    }
                }

                // Poll for scan completion
                if self.app.phase == app::AppPhase::Scanning {
                    if self.app.poll_scan() {
                        self.update_window_title();
                    }
                }

                // Recompute layout if needed
                if self.app.needs_relayout && self.app.phase == AppPhase::Ready {
                    self.app.relayout();
                    if let (Some(render), Some(layout), Some(tree)) =
                        (&mut self.render_state, &self.app.layout, &self.app.tree)
                    {
                        render.update_cushion_treemap(
                            &layout.rects,
                            tree,
                            &self.app.cushion_config,
                            &self.app.color_settings,
                            self.app.sidebar_exclusion_rect(),
                        );
                        self.app.cached_treemap_image = Some(render.treemap_image().clone());
                        tracing::info!(
                            "Cushion treemap rasterized (WGSL): {}x{}",
                            render.surface_config.width,
                            render.surface_config.height
                        );
                    }
                }

                // Build and render the scene
                self.app.rebuild_scene();

                if let Some(render) = &mut self.render_state {
                    if let Err(e) = render.render(&self.app.scene) {
                        tracing::error!("Render error: {}", e);
                    }
                }

                // Request continuous redraws during scanning for progress updates
                if self.app.phase == app::AppPhase::Scanning {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }

            _ => {}
        }
    }
}

impl SilvaViewApp {
    fn handle_action(&mut self, action: input::InputAction) {
        match action {
            input::InputAction::DrillDown { node } => {
                self.app.drill_down(node);
                self.update_window_title();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            input::InputAction::NavigateUp => {
                self.app.navigate_up();
                self.update_window_title();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            input::InputAction::Resize { width, height } => {
                self.app.resize(width, height);
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("Silvaview_rs=info".parse().unwrap()),
        )
        .init();

    // Parse command line: optional path argument, defaults to C:\
    let scan_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            #[cfg(windows)]
            {
                PathBuf::from("C:\\")
            }
            #[cfg(not(windows))]
            {
                PathBuf::from("/")
            }
        });

    // Check for admin privileges if scanning a drive root
    #[cfg(windows)]
    if scan_path.to_str().map(|s| s.ends_with(":\\")).unwrap_or(false) {
        if !scanner::elevation::is_elevated() {
            tracing::warn!("Not running with Administrator privileges. MFT scanning unavailable.");
            tracing::info!("For 10x faster scanning, run with 'Run as Administrator' or the app will auto-prompt.");
            // Note: With the manifest approach above, Windows will auto-prompt on launch
            // With runtime elevation, you could do: scanner::elevation::request_elevation()?;
        } else {
            tracing::info!("Running with Administrator privileges - MFT scanning enabled!");
        }
    }

    tracing::info!("SilvaView-rs starting, scan path: {:?}", scan_path);

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = SilvaViewApp::new(scan_path);
    event_loop.run_app(&mut app)?;

    Ok(())
}
