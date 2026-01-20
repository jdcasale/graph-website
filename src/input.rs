use crate::graph::Vec2;
use crate::{apply_camera_velocity, console_log, log, cycle_highlighted_tag, cycle_rail_edge, drive_rail, end_node_drag, exit_rail_mode, get_camera_zoom, go_home, handle_node_click, handle_node_double_click, is_rail_mode, is_zoomed_in, set_camera_zoom, toggle_all_tags, toggle_dark_mode, toggle_help, toggle_highlighted_tag, toggle_prune_mode, toggle_rail_mode, toggle_tag, try_start_node_drag, update_node_drag, with_input, with_input_result, zoom_in_current, zoom_out};
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
    last_t_time: f64,
    last_click_time: f64,
    last_click_pos: Vec2,
    // Touch state
    pub touch_count: u32,
    pub pinch_distance: Option<f64>,
    pub pinch_center: Vec2,
    last_tap_time: f64,
    last_tap_pos: Vec2,
    touch_start_pos: Vec2,  // Where the current touch started (for tap detection)
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
            last_t_time: 0.0,
            last_click_time: 0.0,
            last_click_pos: Vec2::zero(),
            // Touch state
            touch_count: 0,
            pinch_distance: None,
            pinch_center: Vec2::zero(),
            last_tap_time: 0.0,
            last_tap_pos: Vec2::zero(),
            touch_start_pos: Vec2::zero(),
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
        if matches!(key.as_str(), "h" | "j" | "k" | "l" | "g" | "b" | "d" | "t" | "p" | "n" | "m" | " " | "?" | "Tab" | "Enter" | "Escape" | "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown") {
            event.prevent_default();
        }

        // Track shift state
        if key == "Shift" {
            with_input(|input| input.shift_held = true);
            return;
        }

        // Handle Escape - zoom out or go up a level
        if key == "Escape" {
            zoom_out();
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

        // Handle b (toggle rail mode)
        if key == "b" {
            toggle_rail_mode();
            return;
        }

        // Handle d (toggle dark mode)
        if key == "d" {
            toggle_dark_mode();
            return;
        }

        // Handle Tab (cycle through tags on current node)
        if key == "Tab" {
            cycle_highlighted_tag();
            return;
        }

        // Handle t (single = toggle highlighted tag, double tt = toggle all tags)
        if key == "t" {
            let should_toggle_all = with_input_result(|input| {
                let now = js_sys::Date::now();
                let result = now - input.last_t_time < 500.0;
                input.last_t_time = now;
                result
            });
            if should_toggle_all {
                toggle_all_tags();
            } else {
                // Single t - toggle the highlighted tag
                toggle_highlighted_tag();
            }
            return;
        }

        // Handle p (toggle prune mode - only show nodes connected to current via active tags)
        if key == "p" {
            toggle_prune_mode();
            return;
        }

        // n/m = cycle edges (enters rail mode if not already in it)
        if key == "n" {
            if !is_rail_mode() {
                toggle_rail_mode(); // Enter rail mode
            }
            cycle_rail_edge(true);
            return;
        }
        if key == "m" {
            if !is_rail_mode() {
                toggle_rail_mode(); // Enter rail mode
            }
            cycle_rail_edge(false);
            return;
        }

        // Space = drive to selected edge (only in rail mode)
        if key == " " && is_rail_mode() {
            drive_rail();
            return;
        }

        // Handle ? (toggle help overlay)
        if key == "?" {
            toggle_help();
            return;
        }

        // Shift+hjkl moves the current node (if in rail mode)
        if shift && matches!(key.as_str(), "h" | "j" | "k" | "l" | "H" | "J" | "K" | "L") {
            if is_rail_mode() {
                let dir = key.to_lowercase().chars().next().unwrap_or('h');
                with_input(|input| input.move_node_dir = Some(dir));
                return;
            }
        }

        // hjkl = exit rail mode and pan camera
        if matches!(key.as_str(), "h" | "j" | "k" | "l") {
            exit_rail_mode(); // Seamlessly exit rail mode
        }

        // hjkl pan camera (closest node auto-selected as you move)
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
        // Check if click was on a tag chip - if so, handle it and return
        if let Some(target) = event.target() {
            if let Ok(element) = target.dyn_into::<web_sys::HtmlElement>() {
                let class_name = element.class_name();
                if class_name.contains("tag-chip") {
                    // Handle tag chip click
                    if let (Some(key), Some(value)) = (
                        element.get_attribute("data-tag-key"),
                        element.get_attribute("data-tag-value"),
                    ) {
                        console_log!("mouseup on tag-chip: {}:{}", key, value);
                        toggle_tag(&key, &value);
                    } else {
                        console_log!("tag-chip missing data attributes, class={}", class_name);
                    }
                    with_input(|input| {
                        input.is_dragging = false;
                        input.is_dragging_node = false;
                        input.drag_delta = Vec2::zero();
                        input.drag_velocity = Vec2::zero();
                    });
                    return;
                }
            }
        }

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
                // Double-click - navigate into the clicked node
                handle_node_double_click(x, y);
            } else if !is_double_click {
                // Single click - jump to clicked node
                handle_node_click(x, y);
            }
        }
    });

    document.add_event_listener_with_callback("mouseup", mouseup_closure.as_ref().unchecked_ref())?;
    mouseup_closure.forget();

    // Wheel (trackpad/scroll) - only capture when not in reading mode
    let wheel_closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::WheelEvent| {
        // In reading mode (zoomed in on article), let the browser handle scrolling
        if is_zoomed_in() {
            return;
        }
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

    // ═══════════════════════════════════════════════════════════════
    // TAG CHIP CLICK HANDLER
    // ═══════════════════════════════════════════════════════════════

    // Click handler for tag chips (uses event delegation)
    let tag_click_closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
        // Check if the click target is a tag chip
        if let Some(target) = event.target() {
            if let Ok(element) = target.dyn_into::<web_sys::HtmlElement>() {
                // Check if element has "tag-chip" class
                let class_name = element.class_name();
                console_log!("click on element with class: {}", class_name);
                if class_name.contains("tag-chip") {
                    // Get tag key and value from data attributes
                    let key = element.get_attribute("data-tag-key");
                    let value = element.get_attribute("data-tag-value");
                    console_log!("tag-chip clicked: key={:?}, value={:?}", key, value);
                    if let (Some(key), Some(value)) = (key, value) {
                        event.prevent_default();
                        event.stop_propagation();
                        console_log!("toggling tag: {}:{}", key, value);
                        toggle_tag(&key, &value);
                    }
                }
            }
        }
    });

    document.add_event_listener_with_callback("click", tag_click_closure.as_ref().unchecked_ref())?;
    tag_click_closure.forget();

    // ═══════════════════════════════════════════════════════════════
    // TOUCH HANDLERS (on document, like mouse handlers)
    // ═══════════════════════════════════════════════════════════════

    // Touch start - begin drag or pinch
    let touchstart_closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::TouchEvent| {
        event.prevent_default();
        event.stop_propagation();

        let touches = event.touches();
        let touch_count = touches.length();

        console_log!("touchstart: {} fingers", touch_count);

        if touch_count == 1 {
            // Single touch - start drag
            if let Some(touch) = touches.get(0) {
                let x = touch.client_x() as f64;
                let y = touch.client_y() as f64;
                let pos = Vec2::new(x, y);

                // Try to grab a node first
                let grabbed_node = try_start_node_drag(x, y);

                with_input(|input| {
                    input.touch_count = 1;
                    input.is_dragging = true;
                    input.is_dragging_node = grabbed_node;
                    input.last_mouse = pos;
                    input.touch_start_pos = pos;  // Remember where touch started
                    input.drag_velocity = Vec2::zero();
                    input.pinch_distance = None;
                });
            }
        } else if touch_count == 2 {
            // Two fingers - start pinch
            if let (Some(t1), Some(t2)) = (touches.get(0), touches.get(1)) {
                let p1 = Vec2::new(t1.client_x() as f64, t1.client_y() as f64);
                let p2 = Vec2::new(t2.client_x() as f64, t2.client_y() as f64);
                let dist = (p2 - p1).length();
                let center = Vec2::new((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0);

                with_input(|input| {
                    input.touch_count = 2;
                    input.is_dragging = false;
                    input.is_dragging_node = false;
                    input.pinch_distance = Some(dist);
                    input.pinch_center = center;
                });

                // End any node drag
                end_node_drag(0.0, 0.0);
            }
        }
    });

    let touch_options = web_sys::AddEventListenerOptions::new();
    touch_options.set_passive(false);
    document.add_event_listener_with_callback_and_add_event_listener_options(
        "touchstart",
        touchstart_closure.as_ref().unchecked_ref(),
        &touch_options,
    )?;
    touchstart_closure.forget();

    // Touch move - drag or pinch
    let touchmove_closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::TouchEvent| {
        event.prevent_default();
        event.stop_propagation();

        let touches = event.touches();
        let touch_count = touches.length();

        if touch_count == 1 {
            // Single touch - drag
            if let Some(touch) = touches.get(0) {
                let x = touch.client_x() as f64;
                let y = touch.client_y() as f64;

                let (is_dragging, is_dragging_node) = with_input_result(|input| {
                    if input.touch_count != 1 {
                        return (false, false);
                    }

                    let current = Vec2::new(x, y);
                    input.mouse_position = current;

                    if input.is_dragging {
                        let delta = current - input.last_mouse;
                        input.drag_delta = delta;
                        input.drag_velocity = delta;
                        input.last_mouse = current;
                    }

                    (input.is_dragging, input.is_dragging_node)
                });

                // If dragging a node, update its position
                if is_dragging && is_dragging_node {
                    update_node_drag(x, y);
                }
            }
        } else if touch_count >= 2 {
            // Two or more fingers - pinch zoom
            if let (Some(t1), Some(t2)) = (touches.get(0), touches.get(1)) {
                let p1 = Vec2::new(t1.client_x() as f64, t1.client_y() as f64);
                let p2 = Vec2::new(t2.client_x() as f64, t2.client_y() as f64);
                let new_dist = (p2 - p1).length();

                with_input(|input| {
                    // Update touch state in case we transitioned from 1 finger
                    input.touch_count = 2;
                    input.is_dragging = false;
                    input.is_dragging_node = false;

                    if let Some(old_dist) = input.pinch_distance {
                        if old_dist > 0.0 && new_dist > 0.0 {
                            let scale_factor = new_dist / old_dist;
                            let current_zoom = get_camera_zoom();
                            let new_zoom = current_zoom * scale_factor;
                            console_log!("pinch: old={:.1} new={:.1} scale={:.2} zoom={:.2}",
                                old_dist, new_dist, scale_factor, new_zoom);
                            set_camera_zoom(new_zoom);
                        }
                    } else {
                        console_log!("pinch: initializing distance={:.1}", new_dist);
                    }
                    // Always update pinch_distance (initializes on first 2-finger move)
                    input.pinch_distance = Some(new_dist);
                });
            }
        }
    });

    let touchmove_options = web_sys::AddEventListenerOptions::new();
    touchmove_options.set_passive(false);
    document.add_event_listener_with_callback_and_add_event_listener_options(
        "touchmove",
        touchmove_closure.as_ref().unchecked_ref(),
        &touchmove_options,
    )?;
    touchmove_closure.forget();

    // Touch end - handle tap/double-tap or release drag
    let touchend_closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::TouchEvent| {
        event.prevent_default();
        event.stop_propagation();

        let changed_touches = event.changed_touches();
        let remaining_touches = event.touches().length();

        console_log!("touchend: {} remaining", remaining_touches);

        if remaining_touches == 0 {
            // All fingers lifted
            if let Some(touch) = changed_touches.get(0) {
                let x = touch.client_x() as f64;
                let y = touch.client_y() as f64;
                let pos = Vec2::new(x, y);
                let now = js_sys::Date::now();

                let (was_dragging_node, drag_velocity, total_drag, was_single_touch) = with_input_result(|input| {
                    let was_single = input.touch_count == 1;
                    // Calculate total distance from where touch started (not just last delta)
                    let total_distance = (pos - input.touch_start_pos).length();
                    let result = (
                        input.is_dragging_node,
                        input.drag_velocity,
                        total_distance,
                        was_single,
                    );

                    input.is_dragging = false;
                    input.is_dragging_node = false;
                    input.drag_delta = Vec2::zero();
                    input.touch_count = 0;
                    input.pinch_distance = None;
                    // Don't clear drag_velocity yet - we need it for momentum

                    result
                });

                console_log!("touchend: was_node={}, was_single={}, total_drag={:.1}",
                    was_dragging_node, was_single_touch, total_drag);

                if was_dragging_node {
                    // Release the node with velocity
                    console_log!("releasing node with velocity");
                    end_node_drag(drag_velocity.x * 60.0, drag_velocity.y * 60.0);
                } else if was_single_touch && total_drag < 15.0 {
                    // This was a tap (not a drag)
                    let is_double_tap = with_input_result(|input| {
                        let time_diff = now - input.last_tap_time;
                        let pos_diff = (pos - input.last_tap_pos).length();

                        // Double-tap if within 400ms and 30px of last tap
                        let is_dbl = time_diff < 400.0 && pos_diff < 30.0;

                        console_log!("tap check: time_diff={:.0}, pos_diff={:.1}, is_dbl={}",
                            time_diff, pos_diff, is_dbl);

                        input.last_tap_time = now;
                        input.last_tap_pos = pos;

                        is_dbl
                    });

                    if is_double_tap && !is_zoomed_in() {
                        // Double-tap - navigate into node
                        console_log!("DOUBLE TAP - calling handle_node_double_click");
                        handle_node_double_click(x, y);
                    } else if !is_double_tap {
                        // Single tap - select node
                        console_log!("SINGLE TAP - calling handle_node_click");
                        handle_node_click(x, y);
                    }
                } else if was_single_touch {
                    // Was a drag (not a tap) - apply momentum to camera
                    // Velocity is in pixels per frame, scale up for smooth momentum
                    apply_camera_velocity(-drag_velocity.x * 60.0, -drag_velocity.y * 60.0);
                }

                // Now clear the velocity
                with_input(|input| {
                    input.drag_velocity = Vec2::zero();
                });
            }
        } else if remaining_touches == 1 {
            // Went from 2 fingers to 1 - transition back to single-touch drag
            if let Some(touch) = event.touches().get(0) {
                let x = touch.client_x() as f64;
                let y = touch.client_y() as f64;

                with_input(|input| {
                    input.touch_count = 1;
                    input.is_dragging = true;
                    input.is_dragging_node = false;
                    input.last_mouse = Vec2::new(x, y);
                    input.pinch_distance = None;
                });
            }
        }
    });

    let touchend_options = web_sys::AddEventListenerOptions::new();
    touchend_options.set_passive(false);
    document.add_event_listener_with_callback_and_add_event_listener_options(
        "touchend",
        touchend_closure.as_ref().unchecked_ref(),
        &touchend_options,
    )?;
    touchend_closure.forget();

    // Touch cancel - same as touch end
    let touchcancel_closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::TouchEvent| {
        event.prevent_default();
        with_input(|input| {
            if input.is_dragging_node {
                end_node_drag(0.0, 0.0);
            }
            input.is_dragging = false;
            input.is_dragging_node = false;
            input.drag_delta = Vec2::zero();
            input.drag_velocity = Vec2::zero();
            input.touch_count = 0;
            input.pinch_distance = None;
        });
    });

    let touchcancel_options = web_sys::AddEventListenerOptions::new();
    touchcancel_options.set_passive(false);
    document.add_event_listener_with_callback_and_add_event_listener_options(
        "touchcancel",
        touchcancel_closure.as_ref().unchecked_ref(),
        &touchcancel_options,
    )?;
    touchcancel_closure.forget();

    Ok(())
}
