mod app;
mod layout;
mod render;
mod scanner;
mod tree;
mod ui;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use app::App;
use render::RenderState;
use ui::input;

/// Main application handler for winit's event loop.
struct SequoiaViewApp {
    app: App,
    render_state: Option<RenderState>,
    window: Option<Arc<Window>>,
}

impl SequoiaViewApp {
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
            window.set_title(&format!("SequoiaView-rs — {}", path));
        } else {
            window.set_title("SequoiaView-rs — Disk Space Visualizer");
        }
    }
}

impl ApplicationHandler for SequoiaViewApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("SequoiaView-rs — Disk Space Visualizer")
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
                self.app.viewport_width = size.width as f32;
                self.app.viewport_height = size.height as f32;
                self.render_state = Some(state);

                // Start scanning immediately
                self.app.start_scan();
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
                    self.app.resize(size.width, size.height);
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.app.mouse.x = position.x as f32;
                self.app.mouse.y = position.y as f32;

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
                if state == ElementState::Pressed && button == winit::event::MouseButton::Left {
                    // Navigation is intentionally label-only: clicking data blocks is reserved
                    // for future file inspection interactions.
                    if let Some(node) = self.app.hit_test_label(self.app.mouse.x, self.app.mouse.y) {
                        self.app.drill_down(node);
                        self.update_window_title();
                        if let Some(window) = &self.window {
                            window.request_redraw();
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
                    let action = input::process_key(event.logical_key.clone(), event.state);
                    self.handle_action(action);
                }
            }

            WindowEvent::RedrawRequested => {
                // Poll for scan completion
                if self.app.phase == app::AppPhase::Scanning {
                    if self.app.poll_scan() {
                        self.update_window_title();
                    }
                }

                // Recompute layout if needed
                if self.app.needs_relayout && self.app.phase == app::AppPhase::Ready {
                    self.app.relayout();
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

impl SequoiaViewApp {
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
                .add_directive("sequoiaview_rs=info".parse().unwrap()),
        )
        .init();

    // Parse command line: optional path argument, defaults to C:\
    let scan_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\"));

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

    tracing::info!("SequoiaView-rs starting, scan path: {:?}", scan_path);

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = SequoiaViewApp::new(scan_path);
    event_loop.run_app(&mut app)?;

    Ok(())
}
