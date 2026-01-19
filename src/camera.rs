use crate::graph::Vec2;
use crate::input::InputState;

// Physics tuning constants
const ACCELERATION: f64 = 1800.0; // px/s² when key held (lower = more gradual buildup)
const FRICTION: f64 = 0.94; // velocity multiplier per frame (higher = less friction, longer coast)
const MAX_VELOCITY: f64 = 2000.0; // px/s cap (higher top speed)
const GLIDE_SPEED: f64 = 6.0; // lerp factor for click-to-center
const VELOCITY_THRESHOLD: f64 = 1.0; // Stop when velocity below this

pub struct Camera {
    pub position: Vec2,
    pub velocity: Vec2,
    pub target: Option<Vec2>,
    pub zoom: f64,
    pub target_zoom: Option<f64>,
}

const ZOOM_SPEED: f64 = 4.0; // How fast zoom animates

impl Camera {
    pub fn new() -> Self {
        Self {
            position: Vec2::zero(),
            velocity: Vec2::zero(),
            target: None,
            zoom: 1.0,
            target_zoom: None,
        }
    }

    pub fn update(&mut self, input: &InputState, dt: f64) {
        // If we have a glide target, lerp toward it
        if let Some(target) = self.target {
            let diff = target - self.position;
            let dist = diff.length();

            if dist < 1.0 {
                // Arrived
                self.position = target;
                self.target = None;
                self.velocity = Vec2::zero();
            } else {
                // Lerp toward target
                self.position = self.position + diff * (GLIDE_SPEED * dt).min(1.0);
            }
            return; // Skip normal input while gliding
        }

        // Apply acceleration from vim keys
        let mut accel = Vec2::zero();

        if input.key_held('h') {
            accel.x -= ACCELERATION;
        }
        if input.key_held('l') {
            accel.x += ACCELERATION;
        }
        if input.key_held('k') {
            accel.y -= ACCELERATION;
        }
        if input.key_held('j') {
            accel.y += ACCELERATION;
        }

        // Arrow keys as alternative
        if input.arrow_left {
            accel.x -= ACCELERATION;
        }
        if input.arrow_right {
            accel.x += ACCELERATION;
        }
        if input.arrow_up {
            accel.y -= ACCELERATION;
        }
        if input.arrow_down {
            accel.y += ACCELERATION;
        }

        // Apply acceleration
        self.velocity = self.velocity + accel * dt;

        // Apply friction
        self.velocity *= FRICTION.powf(dt * 60.0); // Normalize friction to ~60fps

        // Clamp velocity
        let speed = self.velocity.length();
        if speed > MAX_VELOCITY {
            self.velocity = self.velocity.normalized() * MAX_VELOCITY;
        }

        // Stop if very slow
        if speed < VELOCITY_THRESHOLD {
            self.velocity = Vec2::zero();
        }

        // Update position
        self.position = self.position + self.velocity * dt;

        // Apply mouse drag delta (only if not dragging a node)
        if input.is_dragging && !input.is_dragging_node {
            self.position = self.position - input.drag_delta;
        }

        // Apply scroll/wheel delta
        self.position = self.position + input.scroll_delta;

        // Animate zoom
        if let Some(target_zoom) = self.target_zoom {
            let diff = target_zoom - self.zoom;
            if diff.abs() < 0.01 {
                self.zoom = target_zoom;
                self.target_zoom = None;
            } else {
                self.zoom += diff * ZOOM_SPEED * dt;
            }
        }
    }

    pub fn glide_to(&mut self, target: Vec2) {
        self.target = Some(target);
        self.velocity = Vec2::zero();
    }

    /// Push velocity toward a target - feels like "skating" rather than teleporting
    pub fn skate_toward(&mut self, target: Vec2) {
        let diff = target - self.position;
        let dist = diff.length();
        if dist < 1.0 {
            return;
        }
        // Set velocity in the direction of the target
        // Speed scales with distance but caps well below MAX_VELOCITY for a leisurely glide
        let speed = (dist * 1.5).min(600.0);
        self.velocity = diff.normalized() * speed;
        // Clear any hard target so physics takes over
        self.target = None;
    }

    pub fn go_home(&mut self) {
        self.glide_to(Vec2::zero());
        self.zoom_to(1.0);
    }

    pub fn zoom_to(&mut self, zoom: f64) {
        self.target_zoom = Some(zoom);
    }

    pub fn zoom_in(&mut self) {
        self.target_zoom = Some(12.0); // High zoom for reading - graph disappears
    }

    pub fn zoom_out(&mut self) {
        self.target_zoom = Some(1.0);
    }

    pub fn is_zoomed_in(&self) -> bool {
        self.zoom > 3.0 || self.target_zoom.map(|z| z > 3.0).unwrap_or(false)
    }

    pub fn is_reading_mode(&self) -> bool {
        self.zoom > 8.0
    }
}
