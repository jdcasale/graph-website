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

        // Determine visible nodes based on depth context
        let visible_node_ids: Vec<String> = if let Some(ref context_id) = self.depth_context {
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

        // Render with depth context and transition opacity
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

    /// Get the IDs of currently visible nodes based on depth context
    fn get_visible_node_ids(&self) -> Vec<String> {
        if let Some(ref context_id) = self.depth_context {
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
    console_log!("Initializing personal site...");

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

    console_log!("Personal site initialized!");
    Ok(())
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
            app.current_node = Some("home".to_string());
            app.camera.go_home();
        }
    });
}

// Called from input module to handle node clicks (when not dragging a node)
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
                        // Click radius
                        app.camera.glide_to(node.position);
                        app.current_node = Some(node_id.clone());
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

// Toggle rail mode (bb command)
pub fn toggle_rail_mode() -> bool {
    APP.with(|cell| {
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
                        app.camera.glide_to(node.position);
                        app.current_node = Some(id);
                    }
                }
            }

            app.rail_mode
        } else {
            false
        }
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

// Navigate to a connected node in the given direction
// direction: 'h' (left), 'l' (right), 'k' (up), 'j' (down)
pub fn navigate_rail(direction: char) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            if !app.rail_mode {
                return;
            }

            let current_id = match &app.current_node {
                Some(id) => id.clone(),
                None => return,
            };

            let current_pos = match app.graph.nodes.get(&current_id) {
                Some(node) => node.position,
                None => return,
            };

            // Get visible nodes - we can only navigate to these
            let visible_node_ids = app.get_visible_node_ids();

            // Find all connected nodes that are VISIBLE
            let mut connected: Vec<String> = Vec::new();

            // Direct connections from current node (filtered to visible)
            if let Some(node) = app.graph.nodes.get(&current_id) {
                for conn in &node.connections {
                    if visible_node_ids.contains(conn) && !connected.contains(conn) {
                        connected.push(conn.clone());
                    }
                }
            }

            // Reverse connections (edges pointing to current node, filtered to visible)
            for edge in &app.graph.edges {
                if edge.to == current_id && visible_node_ids.contains(&edge.from) && !connected.contains(&edge.from) {
                    connected.push(edge.from.clone());
                }
                if edge.from == current_id && visible_node_ids.contains(&edge.to) && !connected.contains(&edge.to) {
                    connected.push(edge.to.clone());
                }
            }

            // Find the best node in the given direction
            let mut best: Option<(String, f64)> = None;

            for conn_id in connected {
                let conn_node = match app.graph.nodes.get(&conn_id) {
                    Some(n) => n,
                    None => continue,
                };

                let dx = conn_node.position.x - current_pos.x;
                let dy = conn_node.position.y - current_pos.y;

                // Check if this node is in the right direction
                let is_valid_direction = match direction {
                    'h' => dx < -20.0,  // Left
                    'l' => dx > 20.0,   // Right
                    'k' => dy < -20.0,  // Up
                    'j' => dy > 20.0,   // Down
                    _ => false,
                };

                if !is_valid_direction {
                    continue;
                }

                // Score by how well it matches the direction (prefer more aligned nodes)
                let dist = (dx * dx + dy * dy).sqrt();
                let score = match direction {
                    'h' | 'l' => (dx.abs() / dist) / dist, // Prefer horizontal, closer
                    'k' | 'j' => (dy.abs() / dist) / dist, // Prefer vertical, closer
                    _ => 0.0,
                };

                if best.is_none() || score > best.as_ref().unwrap().1 {
                    best = Some((conn_id, score));
                }
            }

            // Navigate to the best node
            if let Some((target_id, _)) = best {
                if let Some(target_node) = app.graph.nodes.get(&target_id) {
                    app.camera.glide_to(target_node.position);
                    app.current_node = Some(target_id);
                }
            }
        }
    });
}
