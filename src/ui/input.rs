use winit::event::{ElementState, MouseButton};
use winit::keyboard::{Key, NamedKey};

use crate::layout::LayoutRect;
use crate::tree::arena::NodeId;

/// Mouse state tracking.
#[derive(Debug, Default)]
pub struct MouseState {
    pub x: f32,
    pub y: f32,
    pub left_pressed: bool,
    pub right_pressed: bool,
}

/// Hit-test: find which layout rectangle contains the given point.
/// Returns the topmost (deepest) rectangle at that point.
pub fn hit_test(layout_rects: &[LayoutRect], x: f32, y: f32) -> Option<NodeId> {
    // Iterate in reverse since deeper nodes are added later
    for rect in layout_rects.iter().rev() {
        if x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h {
            return Some(rect.node);
        }
    }
    None
}

/// Input action produced from raw input events.
#[derive(Debug)]
pub enum InputAction {
    /// Mouse moved to new position
    Hover { x: f32, y: f32 },
    /// Left click on a node (drill down)
    DrillDown { node: NodeId },
    /// Right click or backspace (navigate up)
    NavigateUp,
    /// Scroll for zoom
    Zoom { delta: f32, x: f32, y: f32 },
    /// Window resized
    Resize { width: u32, height: u32 },
    /// No action
    None,
}

/// Process a mouse button event.
pub fn process_mouse_button(
    button: MouseButton,
    state: ElementState,
    mouse: &MouseState,
    layout_rects: &[LayoutRect],
) -> InputAction {
    if state != ElementState::Pressed {
        return InputAction::None;
    }

    match button {
        MouseButton::Left => {
            if let Some(node) = hit_test(layout_rects, mouse.x, mouse.y) {
                InputAction::DrillDown { node }
            } else {
                InputAction::None
            }
        }
        MouseButton::Back | MouseButton::Right => InputAction::NavigateUp,
        _ => InputAction::None,
    }
}

/// Process a keyboard event.
pub fn process_key(key: Key, state: ElementState) -> InputAction {
    if state != ElementState::Pressed {
        return InputAction::None;
    }

    match key.as_ref() {
        Key::Named(NamedKey::Backspace) | Key::Named(NamedKey::Escape) => {
            InputAction::NavigateUp
        }
        _ => InputAction::None,
    }
}
