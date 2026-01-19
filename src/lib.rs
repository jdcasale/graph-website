use wasm_bindgen::prelude::*;

mod camera;
mod content;
mod graph;
mod input;
mod layout;
mod renderer;

use camera::Camera;
use graph::Graph;
use input::InputState;
use renderer::Renderer;

use std::cell::RefCell;
use std::rc::Rc;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

pub(crate) use console_log;

struct App {
    graph: Graph,
    camera: Camera,
    input: InputState,
    renderer: Renderer,
    last_time: f64,
    rail_mode: bool,
    current_node: Option<String>,
    depth_context: Option<String>, // ID of node we're "inside" (None = root level)
    depth_stack: Vec<String>,      // Breadcrumb trail for back navigation
    transition_opacity: f64,       // 0.0 = faded out, 1.0 = fully visible
    transition_direction: i8,      // -1 = fading out, 0 = stable, 1 = fading in
    pending_depth_context: Option<Option<String>>, // Depth to switch to after fade out
    dark_mode: bool,
    active_tags: Vec<(String, String)>, // Active tag filters - each shows connections in different style
    prune_mode: bool, // When true, only show nodes connected to current node via active tags
    highlighted_tag_index: Option<usize>, // Currently highlighted tag on current node (for Tab cycling)
    // Rail navigation state (steer-then-drive model)
    rail_edges: Vec<(String, f64)>,       // Cached (neighbor_id, angle) sorted by angle
    rail_selected_index: Option<usize>,   // Currently selected edge (None = no selection)
}

impl App {
    fn new() -> Result<Self, JsValue> {
        let graph = content::build_graph();
        let camera = Camera::new();
        let input = InputState::new();
        let renderer = Renderer::new()?;

        Ok(Self {
            graph,
            camera,
            input,
            renderer,
            last_time: 0.0,
            rail_mode: false,
            current_node: Some("home".to_string()), // Start at home node
            depth_context: None,                     // Start at root level
            depth_stack: Vec::new(),                 // Empty breadcrumb trail
            transition_opacity: 1.0,                 // Start fully visible
            transition_direction: 0,                 // No transition
            pending_depth_context: None,             // No pending change
            dark_mode: false,
            active_tags: Vec::new(),                 // No tags selected
            prune_mode: false,                       // Show all nodes with active tags
            highlighted_tag_index: None,             // No tag highlighted
            // Rail navigation state (steer-then-drive)
            rail_edges: Vec::new(),
            rail_selected_index: None,
        })
    }

    fn tick(&mut self, time: f64) {
        let dt = if self.last_time == 0.0 {
            0.016 // ~60fps default for first frame
        } else {
            (time - self.last_time) / 1000.0 // Convert ms to seconds
        };
        self.last_time = time;

        // Cap dt to prevent huge jumps
        let dt = dt.min(0.1);

        // Handle depth transition animation
        let transition_speed = 4.0; // Speed of fade in/out
        if self.transition_direction == -1 {
            // Fading out
            self.transition_opacity -= transition_speed * dt;
            if self.transition_opacity <= 0.0 {
                self.transition_opacity = 0.0;
                // Switch to pending depth context
                if let Some(new_context) = self.pending_depth_context.take() {
                    self.depth_context = new_context;
                    // Reset camera for new view
                    self.camera.position = graph::Vec2::zero();
                    self.camera.velocity = graph::Vec2::zero();
                    self.camera.target = None;
                    // Snap to first visible node in rail mode
                    if self.rail_mode {
                        self.snap_to_nearest_visible_node();
                    } else {
                        self.current_node = None;
                    }
                }
                // Start fading in
                self.transition_direction = 1;
            }
        } else if self.transition_direction == 1 {
            // Fading in
            self.transition_opacity += transition_speed * dt;
            if self.transition_opacity >= 1.0 {
                self.transition_opacity = 1.0;
                self.transition_direction = 0; // Done
            }
        }

        // Determine visible nodes based on active tags or depth context
        let visible_node_ids: Vec<String> = if !self.active_tags.is_empty() {
            // Tag filter mode: show all nodes with ANY of the active tags
            let mut node_ids: Vec<String> = Vec::new();
            for (key, value) in &self.active_tags {
                for id in self.graph.get_nodes_with_tag(key, value) {
                    if !node_ids.contains(&id) {
                        node_ids.push(id);
                    }
                }
            }
            node_ids
        } else if let Some(ref context_id) = self.depth_context {
            self.graph.get_subgraph_nodes(context_id)
        } else {
            self.graph
                .get_root_nodes()
                .iter()
                .map(|n| n.id.clone())
                .collect()
        };

        // Handle continuous node movement (Shift+hjkl in rail mode)
        if self.rail_mode {
            if let Some(dir) = self.input.move_node_dir {
                if let Some(ref current_id) = self.current_node {
                    let acceleration = 800.0; // Acceleration for node movement
                    let accel_vec = match dir {
                        'h' => graph::Vec2::new(-acceleration, 0.0),
                        'l' => graph::Vec2::new(acceleration, 0.0),
                        'k' => graph::Vec2::new(0.0, -acceleration),
                        'j' => graph::Vec2::new(0.0, acceleration),
                        _ => graph::Vec2::zero(),
                    };

                    if let Some(node) = self.graph.nodes.get_mut(current_id) {
                        // Apply acceleration to node velocity
                        node.velocity = node.velocity + accel_vec * dt;

                        // Cap velocity
                        let max_vel = 400.0;
                        let speed = node.velocity.length();
                        if speed > max_vel {
                            node.velocity = node.velocity * (max_vel / speed);
                        }

                        // Camera follows the node
                        self.camera.position = node.position;
                    }
                }
            }
        }

        // Update camera based on input
        self.camera.update(&self.input, dt);

        // Clear input deltas AFTER camera uses them
        self.input.clear_deltas();

        // Run physics simulation only on visible nodes
        layout::step(&mut self.graph, dt, &visible_node_ids);

        // Determine which node to highlight (only from visible nodes)
        let highlight_node = if self.rail_mode {
            self.current_node.clone()
        } else {
            // Find visible node closest to camera center (if within reasonable distance)
            let mut closest: Option<(String, f64)> = None;
            let center_threshold = 200.0; // Max distance from center to highlight

            for id in &visible_node_ids {
                if let Some(node) = self.graph.nodes.get(id) {
                    let dx = node.position.x - self.camera.position.x;
                    let dy = node.position.y - self.camera.position.y;
                    let dist = (dx * dx + dy * dy).sqrt();

                    if dist < center_threshold {
                        if closest.is_none() || dist < closest.as_ref().unwrap().1 {
                            closest = Some((id.clone(), dist));
                        }
                    }
                }
            }

            let closest_node = closest.map(|(id, _)| id);
            // Update current_node so zoom works in free mode too
            self.current_node = closest_node.clone();
            closest_node
        };

        // Get rail selection for highlighting (computed inline to avoid double borrow)
        let rail_selection = if self.rail_mode {
            self.rail_selected_index
                .and_then(|idx| self.rail_edges.get(idx))
                .map(|(id, _)| id.clone())
        } else {
            None
        };

        // Render with depth context, transition opacity, and active tags
        self.renderer
            .render(
                &self.graph,
                &self.camera,
                self.rail_mode,
                &highlight_node,
                &self.depth_context,
                &visible_node_ids,
                self.transition_opacity,
                self.dark_mode,
                &self.active_tags,
                self.highlighted_tag_index,
                &rail_selection,
            )
            .unwrap_or_else(|e| console_log!("Render error: {:?}", e));
    }

    /// Drill into a node's subgraph (if it has children)
    fn drill_into(&mut self, node_id: &str) {
        if self.graph.has_children(node_id) && self.transition_direction == 0 {
            // Push current context to stack for back navigation
            if let Some(ref ctx) = self.depth_context {
                self.depth_stack.push(ctx.clone());
            }
            // Start fade-out transition, store pending context
            self.pending_depth_context = Some(Some(node_id.to_string()));
            self.transition_direction = -1;
        }
    }

    /// Go up one level in the depth hierarchy
    fn go_up_level(&mut self) -> bool {
        if self.depth_context.is_some() && self.transition_direction == 0 {
            // Get the parent context from the stack
            let parent_context = self.depth_stack.pop();
            // Start fade-out transition, store pending context
            self.pending_depth_context = Some(parent_context);
            self.transition_direction = -1;
            true
        } else {
            false
        }
    }

    /// Get the IDs of currently visible nodes based on active tags or depth context
    fn get_visible_node_ids(&self) -> Vec<String> {
        if !self.active_tags.is_empty() {
            if self.prune_mode {
                // Prune mode: only show nodes connected to current node via active tags
                if let Some(ref current_id) = self.current_node {
                    // Get current node's active tags
                    let current_tags: Vec<(String, String)> = if let Some(node) = self.graph.nodes.get(current_id) {
                        node.tags.iter()
                            .filter(|t| self.active_tags.contains(t))
                            .cloned()
                            .collect()
                    } else {
                        Vec::new()
                    };

                    // Find nodes that share any of current node's active tags
                    let mut node_ids: Vec<String> = vec![current_id.clone()];
                    for (key, value) in &current_tags {
                        for id in self.graph.get_nodes_with_tag(key, value) {
                            if !node_ids.contains(&id) {
                                node_ids.push(id);
                            }
                        }
                    }
                    node_ids
                } else {
                    Vec::new()
                }
            } else {
                // Normal tag filter mode: show all nodes with ANY of the active tags
                let mut node_ids: Vec<String> = Vec::new();
                for (key, value) in &self.active_tags {
                    for id in self.graph.get_nodes_with_tag(key, value) {
                        if !node_ids.contains(&id) {
                            node_ids.push(id);
                        }
                    }
                }
                node_ids
            }
        } else if let Some(ref context_id) = self.depth_context {
            self.graph.get_subgraph_nodes(context_id)
        } else {
            self.graph
                .get_root_nodes()
                .iter()
                .map(|n| n.id.clone())
                .collect()
        }
    }

    /// Snap camera and current_node to the nearest visible node
    fn snap_to_nearest_visible_node(&mut self) {
        let visible_node_ids = self.get_visible_node_ids();
        let camera_pos = self.camera.position;
        let mut nearest: Option<(String, f64)> = None;

        for id in &visible_node_ids {
            if let Some(node) = self.graph.nodes.get(id) {
                let dx = node.position.x - camera_pos.x;
                let dy = node.position.y - camera_pos.y;
                let dist = (dx * dx + dy * dy).sqrt();

                if nearest.is_none() || dist < nearest.as_ref().unwrap().1 {
                    nearest = Some((id.clone(), dist));
                }
            }
        }

        if let Some((id, _)) = nearest {
            if let Some(node) = self.graph.nodes.get(&id) {
                self.camera.position = node.position;
                self.current_node = Some(id);
            }
        }
    }
}

thread_local! {
    static APP: RefCell<Option<App>> = RefCell::new(None);
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    // Set up panic hook for better error messages in browser console
    console_error_panic_hook::set_once();

    console_log!("Initializing Pensieve...");

    // Create app
    let app = App::new()?;

    // Store in thread-local
    APP.with(|cell| {
        *cell.borrow_mut() = Some(app);
    });

    // Set up input listeners
    input::setup_listeners()?;

    // Start animation loop
    start_animation_loop()?;

    // Start intro animation timer
    start_intro_timer()?;

    console_log!("Pensieve initialized!");
    Ok(())
}

fn start_intro_timer() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;

    // After 5.5 seconds (enough time for all quotes to fade in), start fading out
    let fade_out_closure = Closure::once(move || {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(overlay) = document.get_element_by_id("intro-overlay") {
                    // Add fade-out class to trigger CSS transition
                    let _ = overlay.set_attribute("class", "fade-out");

                    // After the fade-out transition (1.5s), hide the element completely
                    let hide_closure = Closure::once(move || {
                        if let Some(window) = web_sys::window() {
                            if let Some(document) = window.document() {
                                if let Some(overlay) = document.get_element_by_id("intro-overlay") {
                                    let _ = overlay.set_attribute("class", "hidden");
                                }
                            }
                        }
                    });

                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        hide_closure.as_ref().unchecked_ref(),
                        1500, // 1.5 seconds for fade-out transition
                    );
                    hide_closure.forget();
                }
            }
        }
    });

    window.set_timeout_with_callback_and_timeout_and_arguments_0(
        fade_out_closure.as_ref().unchecked_ref(),
        5500, // 5.5 seconds to let quotes fade in
    )?;
    fade_out_closure.forget();

    Ok(())
}

/// Check if the intro overlay is currently visible
#[wasm_bindgen]
pub fn is_intro_visible() -> bool {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(overlay) = document.get_element_by_id("intro-overlay") {
                let class = overlay.get_attribute("class").unwrap_or_default();
                // Visible if not hidden and not fading out
                return !class.contains("hidden") && !class.contains("fade-out");
            }
        }
    }
    false
}

/// Dismiss the intro overlay immediately (tap to skip)
#[wasm_bindgen]
pub fn dismiss_intro() {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(overlay) = document.get_element_by_id("intro-overlay") {
                let class = overlay.get_attribute("class").unwrap_or_default();
                if !class.contains("hidden") && !class.contains("fade-out") {
                    let _ = overlay.set_attribute("class", "fade-out");

                    // Hide after transition
                    let hide_closure = Closure::once(move || {
                        if let Some(window) = web_sys::window() {
                            if let Some(document) = window.document() {
                                if let Some(overlay) = document.get_element_by_id("intro-overlay") {
                                    let _ = overlay.set_attribute("class", "hidden");
                                }
                            }
                        }
                    });

                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        hide_closure.as_ref().unchecked_ref(),
                        1500,
                    );
                    hide_closure.forget();
                }
            }
        }
    }
}

fn start_animation_loop() -> Result<(), JsValue> {
    let f: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::new(move |time: f64| {
        APP.with(|cell| {
            if let Some(app) = cell.borrow_mut().as_mut() {
                app.tick(time);
            }
        });

        // Request next frame
        if let Some(window) = web_sys::window() {
            let _ = window.request_animation_frame(
                f.borrow().as_ref().unwrap().as_ref().unchecked_ref(),
            );
        }
    }));

    // Start the loop
    let window = web_sys::window().ok_or("no window")?;
    window.request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref())?;

    Ok(())
}

// Called from input module to update input state
pub fn with_input<F: FnOnce(&mut InputState)>(f: F) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            f(&mut app.input);
        }
    });
}

// Version that returns a value
pub fn with_input_result<T, F: FnOnce(&mut InputState) -> T>(f: F) -> T
where
    T: Default,
{
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            f(&mut app.input)
        } else {
            T::default()
        }
    })
}

// Called from input module for gg command
pub fn go_home() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            // Reset to root level
            app.depth_context = None;
            app.depth_stack.clear();
            app.active_tags.clear(); // Clear any active tag filters
            app.prune_mode = false;
            app.current_node = Some("home".to_string());
            app.camera.go_home();
        }
    });
}

// Called from input module to handle node clicks (when not dragging a node)
// In free roam mode: just select the node (no camera movement)
// In rail mode: skate toward the node
pub fn handle_node_click(x: f64, y: f64) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            // Convert screen coords to world coords (accounting for zoom)
            let zoom = app.camera.zoom;
            let world_x = (x - app.renderer.viewport_width() / 2.0) / zoom + app.camera.position.x;
            let world_y = (y - app.renderer.viewport_height() / 2.0) / zoom + app.camera.position.y;

            // Get visible nodes
            let visible_node_ids = app.get_visible_node_ids();

            // Check if any visible node was clicked
            for node_id in &visible_node_ids {
                if let Some(node) = app.graph.nodes.get(node_id) {
                    let dx = node.position.x - world_x;
                    let dy = node.position.y - world_y;
                    let dist = (dx * dx + dy * dy).sqrt();

                    if dist < 20.0 {
                        // In rail mode: skate toward the clicked node
                        // In free roam: just select it, no camera movement
                        if app.rail_mode {
                            app.camera.skate_toward(node.position);
                        }
                        app.current_node = Some(node_id.clone());
                        // Reset highlighted tag since new node may have different tags
                        app.highlighted_tag_index = None;
                        break;
                    }
                }
            }
        }
    });
}

// Handle double-click on a node - drill into it or zoom to article
pub fn handle_node_double_click(x: f64, y: f64) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            // Convert screen coords to world coords (accounting for zoom)
            let zoom = app.camera.zoom;
            let world_x = (x - app.renderer.viewport_width() / 2.0) / zoom + app.camera.position.x;
            let world_y = (y - app.renderer.viewport_height() / 2.0) / zoom + app.camera.position.y;

            // Get visible nodes
            let visible_node_ids = app.get_visible_node_ids();

            // Find which node was double-clicked
            let mut clicked_node_id: Option<String> = None;
            for node_id in &visible_node_ids {
                if let Some(node) = app.graph.nodes.get(node_id) {
                    let dx = node.position.x - world_x;
                    let dy = node.position.y - world_y;
                    let dist = (dx * dx + dy * dy).sqrt();

                    if dist < 25.0 {
                        // Slightly larger radius for double-click
                        clicked_node_id = Some(node_id.clone());
                        break;
                    }
                }
            }

            // If a node was clicked, navigate into it
            if let Some(node_id) = clicked_node_id {
                if app.graph.has_children(&node_id) {
                    // Drill into subgraph
                    app.drill_into(&node_id);
                } else {
                    // Leaf node - set as current and zoom to article
                    app.current_node = Some(node_id);
                    app.camera.zoom_in();
                }
            }
        }
    });
}

// Try to start dragging a node at screen position (x, y)
// Returns true if a node was found and drag started
pub fn try_start_node_drag(x: f64, y: f64) -> bool {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            let zoom = app.camera.zoom;
            let world_x = (x - app.renderer.viewport_width() / 2.0) / zoom + app.camera.position.x;
            let world_y = (y - app.renderer.viewport_height() / 2.0) / zoom + app.camera.position.y;

            // Find node under cursor
            for node in app.graph.nodes.values_mut() {
                let dx = node.position.x - world_x;
                let dy = node.position.y - world_y;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < 25.0 {
                    // Slightly larger grab radius
                    node.is_dragged = true;
                    node.velocity = graph::Vec2::zero();
                    return true;
                }
            }
        }
        false
    })
}

// Update dragged node position
pub fn update_node_drag(x: f64, y: f64) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            let zoom = app.camera.zoom;
            let world_x = (x - app.renderer.viewport_width() / 2.0) / zoom + app.camera.position.x;
            let world_y = (y - app.renderer.viewport_height() / 2.0) / zoom + app.camera.position.y;

            for node in app.graph.nodes.values_mut() {
                if node.is_dragged {
                    node.position = graph::Vec2::new(world_x, world_y);
                    node.velocity = graph::Vec2::zero();
                }
            }
        }
    });
}

// End node drag and give it a velocity based on mouse movement
pub fn end_node_drag(velocity_x: f64, velocity_y: f64) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            for node in app.graph.nodes.values_mut() {
                if node.is_dragged {
                    node.is_dragged = false;
                    // Give it some momentum from the drag
                    node.velocity = graph::Vec2::new(velocity_x * 0.5, velocity_y * 0.5);
                }
            }
        }
    });
}

// Toggle rail mode (b command)
pub fn toggle_rail_mode() -> bool {
    let entered_rail = APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.rail_mode = !app.rail_mode;

            if app.rail_mode {
                // Entering rail mode - snap to nearest VISIBLE node
                let camera_pos = app.camera.position;
                let visible_node_ids = app.get_visible_node_ids();
                let mut nearest: Option<(String, f64)> = None;

                for id in &visible_node_ids {
                    if let Some(node) = app.graph.nodes.get(id) {
                        let dx = node.position.x - camera_pos.x;
                        let dy = node.position.y - camera_pos.y;
                        let dist = (dx * dx + dy * dy).sqrt();

                        if nearest.is_none() || dist < nearest.as_ref().unwrap().1 {
                            nearest = Some((id.clone(), dist));
                        }
                    }
                }

                if let Some((id, _)) = nearest {
                    if let Some(node) = app.graph.nodes.get(&id) {
                        app.camera.skate_toward(node.position);
                        app.current_node = Some(id);
                    }
                }
                true
            } else {
                // Exiting rail mode - clear selection
                app.rail_edges.clear();
                app.rail_selected_index = None;
                false
            }
        } else {
            false
        }
    });

    // Update edges after releasing the borrow
    if entered_rail {
        update_rail_edges();
    }

    APP.with(|cell| {
        cell.borrow().as_ref().map(|app| app.rail_mode).unwrap_or(false)
    })
}

// Check if rail mode is active
pub fn is_rail_mode() -> bool {
    APP.with(|cell| {
        if let Some(app) = cell.borrow().as_ref() {
            app.rail_mode
        } else {
            false
        }
    })
}

// Toggle dark mode
pub fn toggle_dark_mode() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.dark_mode = !app.dark_mode;
        }
    });
}

// Toggle a tag filter (add if not present, remove if present)
pub fn toggle_tag(key: &str, value: &str) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            let tag = (key.to_string(), value.to_string());
            if let Some(pos) = app.active_tags.iter().position(|t| t == &tag) {
                // Tag is active, remove it
                app.active_tags.remove(pos);
            } else {
                // Tag not active, add it
                app.active_tags.push(tag);
            }
        }
    });
}

// Clear all active tags
pub fn clear_tags() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.active_tags.clear();
        }
    });
}

// Check if there are active tags
pub fn has_active_tags() -> bool {
    APP.with(|cell| {
        if let Some(app) = cell.borrow().as_ref() {
            !app.active_tags.is_empty()
        } else {
            false
        }
    })
}

// Toggle prune mode (p command) - only show nodes connected to current node via active tags
pub fn toggle_prune_mode() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.prune_mode = !app.prune_mode;
        }
    });
}

// Toggle help overlay (? command)
pub fn toggle_help() {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(overlay) = document.get_element_by_id("help-overlay") {
                if let Some(class_list) = overlay.get_attribute("class") {
                    if class_list.contains("hidden") {
                        let _ = overlay.set_attribute("class", "");
                    } else {
                        let _ = overlay.set_attribute("class", "hidden");
                    }
                }
            }
        }
    }
}

// Toggle all tags on current node (tt command)
// If no tags active: activate all tags from the current node (depth 1 expansion)
// If tags active: clear all tags
pub fn toggle_all_tags() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            if !app.active_tags.is_empty() {
                // Tags are active, clear them
                app.active_tags.clear();
                app.prune_mode = false;
            } else {
                // No tags active, activate all tags from the current node
                if let Some(ref node_id) = app.current_node {
                    if let Some(node) = app.graph.nodes.get(node_id) {
                        // Collect all tags from current node
                        let all_tags: Vec<(String, String)> = node.tags
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        app.active_tags = all_tags;
                    }
                }
            }
        }
    });
}

// Cycle to next tag on current node (Tab key)
pub fn cycle_highlighted_tag() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            if let Some(ref node_id) = app.current_node {
                if let Some(node) = app.graph.nodes.get(node_id) {
                    let tag_count = node.tags.len();
                    if tag_count == 0 {
                        app.highlighted_tag_index = None;
                        return;
                    }

                    app.highlighted_tag_index = match app.highlighted_tag_index {
                        None => Some(0),
                        Some(idx) => {
                            let next = idx + 1;
                            if next >= tag_count {
                                Some(0) // Wrap around
                            } else {
                                Some(next)
                            }
                        }
                    };
                }
            }
        }
    });
}

// Toggle the currently highlighted tag (t key)
pub fn toggle_highlighted_tag() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            if let Some(idx) = app.highlighted_tag_index {
                if let Some(node_id) = app.current_node.clone() {
                    if let Some(node) = app.graph.nodes.get(&node_id) {
                        if let Some((key, value)) = node.tags.get(idx) {
                            let tag = (key.clone(), value.clone());
                            if let Some(pos) = app.active_tags.iter().position(|t| t == &tag) {
                                // Tag is active, remove it
                                app.active_tags.remove(pos);
                            } else {
                                // Tag not active, add it
                                app.active_tags.push(tag);
                            }
                        }
                    }
                }
            }
        }
    });
}

// Get the currently highlighted tag index (for rendering)
pub fn get_highlighted_tag_index() -> Option<usize> {
    APP.with(|cell| {
        if let Some(app) = cell.borrow().as_ref() {
            app.highlighted_tag_index
        } else {
            None
        }
    })
}

// Clear highlighted tag (when current node changes)
pub fn clear_highlighted_tag() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.highlighted_tag_index = None;
        }
    });
}

// Zoom in on current/highlighted node (Enter or double-click)
// If node has children, drill into subgraph; if leaf, zoom to article
pub fn zoom_in_current() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            if let Some(ref node_id) = app.current_node.clone() {
                // Check if this node has children
                if app.graph.has_children(node_id) {
                    // Drill into subgraph
                    app.drill_into(node_id);
                } else {
                    // Leaf node - zoom to article (if it has one)
                    app.camera.zoom_in();
                }
            }
        }
    });
}

// Zoom out (Escape) - if zoomed on article, zoom out; otherwise go up a level
pub fn zoom_out() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            if app.camera.is_zoomed_in() {
                // Currently viewing an article, zoom out to graph view
                app.camera.zoom_out();
            } else {
                // In graph view, try to go up a depth level
                app.go_up_level();
            }
        }
    });
}

// Get current camera zoom level
pub fn get_camera_zoom() -> f64 {
    APP.with(|cell| {
        if let Some(app) = cell.borrow().as_ref() {
            app.camera.zoom
        } else {
            1.0
        }
    })
}

// Set camera zoom level directly (for pinch-to-zoom)
pub fn set_camera_zoom(zoom: f64) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            // Clamp zoom between 0.5 and 3.0 for touch (don't allow reading mode via pinch)
            let clamped = zoom.clamp(0.5, 3.0);
            app.camera.zoom = clamped;
            app.camera.target_zoom = None; // Cancel any animated zoom
        }
    });
}

// Apply velocity to camera (for touch momentum)
pub fn apply_camera_velocity(vx: f64, vy: f64) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.camera.velocity = graph::Vec2::new(vx, vy);
            app.camera.target = None; // Cancel any glide target
        }
    });
}

// Check if we're inside a subgraph (not at root level)
pub fn is_in_subgraph() -> bool {
    APP.with(|cell| {
        if let Some(app) = cell.borrow().as_ref() {
            app.depth_context.is_some()
        } else {
            false
        }
    })
}

// Get the current depth context (for breadcrumb display)
pub fn get_depth_context() -> Option<String> {
    APP.with(|cell| {
        if let Some(app) = cell.borrow().as_ref() {
            app.depth_context.clone()
        } else {
            None
        }
    })
}

// Get the breadcrumb path (for display)
pub fn get_breadcrumb_path() -> Vec<String> {
    APP.with(|cell| {
        if let Some(app) = cell.borrow().as_ref() {
            let mut path = app.depth_stack.clone();
            if let Some(ref ctx) = app.depth_context {
                path.push(ctx.clone());
            }
            path
        } else {
            Vec::new()
        }
    })
}

// Check if zoomed in
pub fn is_zoomed_in() -> bool {
    APP.with(|cell| {
        if let Some(app) = cell.borrow().as_ref() {
            app.camera.is_zoomed_in()
        } else {
            false
        }
    })
}

// Move current node in direction (Shift + hjkl in rail mode)
pub fn move_current_node(direction: char) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            if !app.rail_mode {
                return;
            }

            let current_id = match &app.current_node {
                Some(id) => id.clone(),
                None => return,
            };

            let move_amount = 30.0; // pixels per keypress

            if let Some(node) = app.graph.nodes.get_mut(&current_id) {
                let delta = match direction {
                    'h' => graph::Vec2::new(-move_amount, 0.0),
                    'l' => graph::Vec2::new(move_amount, 0.0),
                    'k' => graph::Vec2::new(0.0, -move_amount),
                    'j' => graph::Vec2::new(0.0, move_amount),
                    _ => return,
                };

                node.position = node.position + delta;
                // Also move camera to follow
                app.camera.position = app.camera.position + delta;
            }
        }
    });
}

// ═══════════════════════════════════════════════════════════════
// RAIL MODE: Steer-then-drive navigation
// n/p: cycle through edges clockwise/counter-clockwise
// hjkl: quick aim to edge closest to that direction
// Space: drive to selected edge
// ═══════════════════════════════════════════════════════════════

/// Update the cached list of edges from current node, sorted by angle
pub fn update_rail_edges() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.rail_edges.clear();
            app.rail_selected_index = None;

            let current_id = match &app.current_node {
                Some(id) => id.clone(),
                None => return,
            };

            let current_pos = match app.graph.nodes.get(&current_id) {
                Some(node) => node.position,
                None => return,
            };

            // Get visible nodes
            let visible_node_ids = app.get_visible_node_ids();

            // Collect all connected nodes
            let mut connected: Vec<String> = Vec::new();

            // Direct connections
            if let Some(node) = app.graph.nodes.get(&current_id) {
                for conn in &node.connections {
                    if visible_node_ids.contains(conn) && !connected.contains(conn) {
                        connected.push(conn.clone());
                    }
                }
            }

            // Reverse connections
            for edge in &app.graph.edges {
                if edge.to == current_id && visible_node_ids.contains(&edge.from) && !connected.contains(&edge.from) {
                    connected.push(edge.from.clone());
                }
                if edge.from == current_id && visible_node_ids.contains(&edge.to) && !connected.contains(&edge.to) {
                    connected.push(edge.to.clone());
                }
            }

            // Tag-inferred connections
            if !app.active_tags.is_empty() {
                if let Some(node) = app.graph.nodes.get(&current_id) {
                    for (key, value) in &node.tags {
                        if app.active_tags.contains(&(key.clone(), value.clone())) {
                            for other_id in app.graph.get_nodes_with_tag(key, value) {
                                if other_id != current_id
                                    && visible_node_ids.contains(&other_id)
                                    && !connected.contains(&other_id)
                                {
                                    connected.push(other_id);
                                }
                            }
                        }
                    }
                }
            }

            // Calculate angle for each connected node and store
            for conn_id in connected {
                if let Some(conn_node) = app.graph.nodes.get(&conn_id) {
                    let dx = conn_node.position.x - current_pos.x;
                    let dy = conn_node.position.y - current_pos.y;
                    let angle = dy.atan2(dx); // atan2(y, x) gives angle from positive x-axis
                    app.rail_edges.push((conn_id, angle));
                }
            }

            // Sort by angle (ascending = counter-clockwise from east)
            app.rail_edges.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            // Auto-select first edge if there's only one
            if app.rail_edges.len() == 1 {
                app.rail_selected_index = Some(0);
            }
        }
    });
}

/// Cycle through edges clockwise (n) or counter-clockwise (p)
pub fn cycle_rail_edge(clockwise: bool) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            if !app.rail_mode || app.rail_edges.is_empty() {
                return;
            }

            let len = app.rail_edges.len();
            app.rail_selected_index = Some(match app.rail_selected_index {
                Some(idx) => {
                    if clockwise {
                        (idx + 1) % len
                    } else {
                        (idx + len - 1) % len
                    }
                }
                None => 0,
            });
        }
    });
}

/// Aim at the edge closest to a cardinal direction (hjkl)
/// h=west, l=east, k=north, j=south
/// Applies navigational gravity: biases toward nodes matching more active tags
pub fn aim_rail_edge(direction: char) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            if !app.rail_mode || app.rail_edges.is_empty() {
                return;
            }

            // Target angle for each direction
            let target_angle = match direction {
                'l' => 0.0,                      // East
                'j' => std::f64::consts::FRAC_PI_2,  // South (positive y is down)
                'h' => std::f64::consts::PI,     // West
                'k' => -std::f64::consts::FRAC_PI_2, // North
                _ => return,
            };

            // Find the edge with angle closest to target
            // Use tag match score as a tie-breaker (navigational gravity)
            let mut best_idx = 0;
            let mut best_score = f64::MAX;

            for (idx, (node_id, angle)) in app.rail_edges.iter().enumerate() {
                // Angular difference (handle wraparound)
                let mut angle_diff = (angle - target_angle).abs();
                if angle_diff > std::f64::consts::PI {
                    angle_diff = 2.0 * std::f64::consts::PI - angle_diff;
                }

                // Tag match bonus: better matches reduce the effective angle difference
                // This biases selection toward nodes matching more active tags
                let (matches, total) = app.graph.tag_match_score(node_id, &app.active_tags);
                let tag_bonus = if total > 0 && matches > 0 {
                    // Reduce angle by up to 0.3 radians (~17 degrees) for full match
                    0.3 * (matches as f64 / total as f64)
                } else {
                    0.0
                };

                let score = angle_diff - tag_bonus;
                if score < best_score {
                    best_score = score;
                    best_idx = idx;
                }
            }

            app.rail_selected_index = Some(best_idx);
        }
    });
}

/// Drive to the currently selected edge (Space)
pub fn drive_rail() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            if !app.rail_mode {
                return;
            }

            // If only one edge and none selected, auto-select it
            if app.rail_selected_index.is_none() && app.rail_edges.len() == 1 {
                app.rail_selected_index = Some(0);
            }

            // Get selected edge
            let target_id = match app.rail_selected_index {
                Some(idx) if idx < app.rail_edges.len() => app.rail_edges[idx].0.clone(),
                _ => return, // No selection, do nothing (could add shake feedback)
            };

            // Get target position and move
            if let Some(target_node) = app.graph.nodes.get(&target_id) {
                app.camera.skate_toward(target_node.position);
                app.current_node = Some(target_id);
                // Clear selection and update edges for new position
                app.rail_selected_index = None;
                // Reset highlighted tag since new node may have different tags
                app.highlighted_tag_index = None;
            }
        }
    });

    // Update edges for the new current node
    update_rail_edges();
}

/// Get the currently selected rail destination (for rendering preview)
pub fn get_rail_selection() -> Option<String> {
    APP.with(|cell| {
        if let Some(app) = cell.borrow().as_ref() {
            if !app.rail_mode {
                return None;
            }
            if let Some(idx) = app.rail_selected_index {
                if idx < app.rail_edges.len() {
                    return Some(app.rail_edges[idx].0.clone());
                }
            }
            None
        } else {
            None
        }
    })
}
