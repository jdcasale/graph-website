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

        // Clear input deltas (they accumulate from events)
        self.input.clear_deltas();

        // Update camera based on input
        self.camera.update(&self.input, dt);

        // Run physics simulation
        layout::step(&mut self.graph, dt);

        // Determine which node to highlight (and update current_node for zoom target)
        let highlight_node = if self.rail_mode {
            self.current_node.clone()
        } else {
            // Find node closest to camera center (if within reasonable distance)
            let mut closest: Option<(String, f64)> = None;
            let center_threshold = 200.0; // Max distance from center to highlight

            for (id, node) in &self.graph.nodes {
                let dx = node.position.x - self.camera.position.x;
                let dy = node.position.y - self.camera.position.y;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < center_threshold {
                    if closest.is_none() || dist < closest.as_ref().unwrap().1 {
                        closest = Some((id.clone(), dist));
                    }
                }
            }

            let closest_node = closest.map(|(id, _)| id);
            // Update current_node so zoom works in free mode too
            self.current_node = closest_node.clone();
            closest_node
        };

        // Render
        self.renderer
            .render(&self.graph, &self.camera, self.rail_mode, &highlight_node)
            .unwrap_or_else(|e| console_log!("Render error: {:?}", e));
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
            app.camera.go_home();
        }
    });
}

// Called from input module to handle node clicks (when not dragging a node)
pub fn handle_node_click(x: f64, y: f64) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            // Convert screen coords to world coords
            let world_x = x + app.camera.position.x - app.renderer.viewport_width() / 2.0;
            let world_y = y + app.camera.position.y - app.renderer.viewport_height() / 2.0;

            // Check if any node was clicked
            for node in app.graph.nodes.values() {
                let dx = node.position.x - world_x;
                let dy = node.position.y - world_y;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < 20.0 {
                    // Click radius
                    app.camera.glide_to(node.position);
                    break;
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
            let world_x = x + app.camera.position.x - app.renderer.viewport_width() / 2.0;
            let world_y = y + app.camera.position.y - app.renderer.viewport_height() / 2.0;

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
            let world_x = x + app.camera.position.x - app.renderer.viewport_width() / 2.0;
            let world_y = y + app.camera.position.y - app.renderer.viewport_height() / 2.0;

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
                // Entering rail mode - snap to nearest node
                let camera_pos = app.camera.position;
                let mut nearest: Option<(String, f64)> = None;

                for (id, node) in &app.graph.nodes {
                    let dx = node.position.x - camera_pos.x;
                    let dy = node.position.y - camera_pos.y;
                    let dist = (dx * dx + dy * dy).sqrt();

                    if nearest.is_none() || dist < nearest.as_ref().unwrap().1 {
                        nearest = Some((id.clone(), dist));
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

// Zoom in on current/highlighted node (Enter or double-click)
pub fn zoom_in_current() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            // Only zoom if there's a current node to zoom on
            if app.current_node.is_some() {
                app.camera.zoom_in();
            }
        }
    });
}

// Zoom out (Escape)
pub fn zoom_out() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.camera.zoom_out();
        }
    });
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

            // Find all connected nodes
            let mut connected: Vec<String> = Vec::new();

            // Direct connections from current node
            if let Some(node) = app.graph.nodes.get(&current_id) {
                connected.extend(node.connections.clone());
            }

            // Reverse connections (edges pointing to current node)
            for edge in &app.graph.edges {
                if edge.to == current_id && !connected.contains(&edge.from) {
                    connected.push(edge.from.clone());
                }
                if edge.from == current_id && !connected.contains(&edge.to) {
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
