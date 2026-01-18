use crate::graph::Vec2;
use crate::{end_node_drag, go_home, handle_node_click, is_rail_mode, is_zoomed_in, navigate_rail, toggle_rail_mode, try_start_node_drag, update_node_drag, with_input, with_input_result, zoom_in_current, zoom_out};
use std::collections::HashSet;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

pub struct InputState {
    pub keys_held: HashSet<char>,
    pub shift_held: bool,
    pub move_node_dir: Option<char>, // Direction to move current node (shift+hjkl)
    pub arrow_left: bool,
    pub arrow_right: bool,
    pub arrow_up: bool,
    pub arrow_down: bool,
    pub mouse_position: Vec2,
    pub is_dragging: bool,
    pub is_dragging_node: bool,
    pub drag_delta: Vec2,
    pub drag_velocity: Vec2,
    pub scroll_delta: Vec2,
    last_mouse: Vec2,
    last_g_time: f64,
    last_b_time: f64,
    last_click_time: f64,
    last_click_pos: Vec2,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            keys_held: HashSet::new(),
            shift_held: false,
            move_node_dir: None,
            arrow_left: false,
            arrow_right: false,
            arrow_up: false,
            arrow_down: false,
            mouse_position: Vec2::zero(),
            is_dragging: false,
            is_dragging_node: false,
            drag_delta: Vec2::zero(),
            drag_velocity: Vec2::zero(),
            scroll_delta: Vec2::zero(),
            last_mouse: Vec2::zero(),
            last_g_time: 0.0,
            last_b_time: 0.0,
            last_click_time: 0.0,
            last_click_pos: Vec2::zero(),
        }
    }

    pub fn key_held(&self, key: char) -> bool {
        self.keys_held.contains(&key)
    }

    pub fn clear_deltas(&mut self) {
        self.drag_delta = Vec2::zero();
        self.scroll_delta = Vec2::zero();
    }
}

pub fn setup_listeners() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;

    // Keydown
    let keydown_closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::KeyboardEvent| {
        let key = event.key();
        let shift = event.shift_key();

        // Prevent default for our navigation keys
        if matches!(key.as_str(), "h" | "j" | "k" | "l" | "g" | "b" | "Enter" | "Escape" | "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown") {
            event.prevent_default();
        }

        // Track shift state
        if key == "Shift" {
            with_input(|input| input.shift_held = true);
            return;
        }

        // Handle Escape - zoom out
        if key == "Escape" {
            if is_zoomed_in() {
                zoom_out();
            }
            return;
        }

        // Handle Enter - zoom in on current/highlighted node
        if key == "Enter" {
            if !is_zoomed_in() {
                zoom_in_current();
            }
            return;
        }

        // Handle gg (go home) separately to avoid borrow issues
        if key == "g" {
            let should_go_home = with_input_result(|input| {
                let now = js_sys::Date::now();
                let result = now - input.last_g_time < 500.0;
                input.last_g_time = now;
                result
            });
            if should_go_home {
                go_home();
            }
            return;
        }

        // Handle bb (toggle rail mode) separately
        if key == "b" {
            let should_toggle = with_input_result(|input| {
                let now = js_sys::Date::now();
                let result = now - input.last_b_time < 500.0;
                input.last_b_time = now;
                result
            });
            if should_toggle {
                toggle_rail_mode();
            }
            return;
        }

        // Check if we're in rail mode for hjkl
        let rail_mode = is_rail_mode();

        // In rail mode with Shift, track direction for continuous movement
        if rail_mode && shift && matches!(key.as_str(), "h" | "j" | "k" | "l" | "H" | "J" | "K" | "L") {
            let dir = key.to_lowercase().chars().next().unwrap_or('h');
            with_input(|input| input.move_node_dir = Some(dir));
            return;
        }

        // In rail mode, hjkl navigate between connected nodes
        if rail_mode && matches!(key.as_str(), "h" | "j" | "k" | "l") {
            match key.as_str() {
                "h" => navigate_rail('h'),
                "j" => navigate_rail('j'),
                "k" => navigate_rail('k'),
                "l" => navigate_rail('l'),
                _ => {}
            }
            return;
        }

        // Normal mode - hjkl pan camera
        with_input(|input| {
            match key.as_str() {
                "h" => { input.keys_held.insert('h'); }
                "j" => { input.keys_held.insert('j'); }
                "k" => { input.keys_held.insert('k'); }
                "l" => { input.keys_held.insert('l'); }
                "ArrowLeft" => { input.arrow_left = true; }
                "ArrowRight" => { input.arrow_right = true; }
                "ArrowUp" => { input.arrow_up = true; }
                "ArrowDown" => { input.arrow_down = true; }
                _ => {}
            }
        });
    });

    document.add_event_listener_with_callback("keydown", keydown_closure.as_ref().unchecked_ref())?;
    keydown_closure.forget();

    // Keyup
    let keyup_closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::KeyboardEvent| {
        let key = event.key();
        with_input(|input| {
            match key.as_str() {
                "Shift" => {
                    input.shift_held = false;
                    input.move_node_dir = None; // Stop node movement when shift released
                }
                "h" | "j" | "k" | "l" => {
                    input.keys_held.remove(&key.chars().next().unwrap());
                    // Clear move_node_dir if this was the direction being held
                    if input.move_node_dir == key.chars().next() {
                        input.move_node_dir = None;
                    }
                }
                "ArrowLeft" => { input.arrow_left = false; }
                "ArrowRight" => { input.arrow_right = false; }
                "ArrowUp" => { input.arrow_up = false; }
                "ArrowDown" => { input.arrow_down = false; }
                _ => {}
            }
        });
    });

    document.add_event_listener_with_callback("keyup", keyup_closure.as_ref().unchecked_ref())?;
    keyup_closure.forget();

    // Mouse down - try to grab a node first, otherwise start camera drag
    let mousedown_closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
        let x = event.client_x() as f64;
        let y = event.client_y() as f64;

        // First try to grab a node
        let grabbed_node = try_start_node_drag(x, y);

        with_input(|input| {
            input.is_dragging = true;
            input.is_dragging_node = grabbed_node;
            input.last_mouse = Vec2::new(x, y);
            input.drag_velocity = Vec2::zero();
        });
    });

    document.add_event_listener_with_callback("mousedown", mousedown_closure.as_ref().unchecked_ref())?;
    mousedown_closure.forget();

    // Mouse move - either drag node or pan camera
    let mousemove_closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
        let x = event.client_x() as f64;
        let y = event.client_y() as f64;

        let (is_dragging, is_dragging_node) = with_input_result(|input| {
            let current = Vec2::new(x, y);
            input.mouse_position = current;

            if input.is_dragging {
                let delta = current - input.last_mouse;
                input.drag_delta = delta;
                input.drag_velocity = delta; // Track velocity for release
                input.last_mouse = current;
            }

            (input.is_dragging, input.is_dragging_node)
        });

        // If dragging a node, update its position
        if is_dragging && is_dragging_node {
            update_node_drag(x, y);
        }
    });

    document.add_event_listener_with_callback("mousemove", mousemove_closure.as_ref().unchecked_ref())?;
    mousemove_closure.forget();

    // Mouse up - release node or handle click/double-click
    let mouseup_closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
        let x = event.client_x() as f64;
        let y = event.client_y() as f64;
        let now = js_sys::Date::now();

        // Get state and clear drag, check for double-click
        let (was_dragging_node, drag_velocity, total_drag, is_double_click) = with_input_result(|input| {
            let pos = Vec2::new(x, y);
            let time_diff = now - input.last_click_time;
            let pos_diff = (pos - input.last_click_pos).length();

            // Double-click if within 400ms and 20px of last click
            let is_dbl = time_diff < 400.0 && pos_diff < 20.0;

            let result = (
                input.is_dragging_node,
                input.drag_velocity,
                input.drag_delta.length(),
                is_dbl,
            );

            input.is_dragging = false;
            input.is_dragging_node = false;
            input.drag_delta = Vec2::zero();
            input.drag_velocity = Vec2::zero();
            input.last_click_time = now;
            input.last_click_pos = pos;

            result
        });

        if was_dragging_node {
            // Release the node with velocity
            end_node_drag(drag_velocity.x * 60.0, drag_velocity.y * 60.0);
        } else if total_drag < 5.0 {
            if is_double_click && !is_zoomed_in() {
                // Double-click - zoom in on highlighted node
                zoom_in_current();
            } else if !is_double_click {
                // Single click - jump to clicked node
                handle_node_click(x, y);
            }
        }
    });

    document.add_event_listener_with_callback("mouseup", mouseup_closure.as_ref().unchecked_ref())?;
    mouseup_closure.forget();

    // Wheel (trackpad/scroll)
    let wheel_closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::WheelEvent| {
        event.prevent_default();
        with_input(|input| {
            input.scroll_delta = Vec2::new(event.delta_x(), event.delta_y());
        });
    });

    let options = web_sys::AddEventListenerOptions::new();
    options.set_passive(false);
    document.add_event_listener_with_callback_and_add_event_listener_options(
        "wheel",
        wheel_closure.as_ref().unchecked_ref(),
        &options,
    )?;
    wheel_closure.forget();

    Ok(())
}
