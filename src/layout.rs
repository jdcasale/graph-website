use crate::graph::{Graph, Vec2};

// Physics tuning constants
const REPULSION_STRENGTH: f64 = 400000.0;   // Repulsion between all nodes (higher = more spread)
const ATTRACTION_STRENGTH: f64 = 0.005;     // Spring attraction along edges (lower = looser connections)
const IDEAL_EDGE_LENGTH: f64 = 280.0;       // Target distance for connected nodes
const ANCHOR_ATTRACTION: f64 = 0.012;       // Pull anchors back to their home position
const VELOCITY_DAMPING: f64 = 0.96;         // Velocity decay per frame (higher = more momentum)
const MIN_DISTANCE: f64 = 60.0;             // Minimum distance for repulsion calc
const MAX_FORCE: f64 = 1000.0;              // Cap force magnitude
const VELOCITY_THRESHOLD: f64 = 0.05;       // Stop when velocity below this

/// Run one step of the physics simulation.
/// Call this every frame for continuous movement.
pub fn step(graph: &mut Graph, dt: f64) {
    let node_ids: Vec<String> = graph.nodes.keys().cloned().collect();

    // Calculate forces for each node
    let mut forces: Vec<(String, Vec2)> = Vec::new();

    for id in &node_ids {
        let node = match graph.nodes.get(id) {
            Some(n) => n,
            None => continue,
        };

        // Skip nodes being dragged (they follow the mouse)
        if node.is_dragged {
            continue;
        }

        let mut force = Vec2::zero();

        // Repulsion from all other nodes (based on bounding box distance)
        for other_id in &node_ids {
            if other_id == id {
                continue;
            }

            let other = match graph.nodes.get(other_id) {
                Some(n) => n,
                None => continue,
            };

            // Get bounding boxes in world coordinates
            let (a_min_x, a_min_y, a_max_x, a_max_y) = node.bounds_offsets();
            let (b_min_x, b_min_y, b_max_x, b_max_y) = other.bounds_offsets();

            // Convert to absolute positions
            let a_left = node.position.x + a_min_x;
            let a_right = node.position.x + a_max_x;
            let a_top = node.position.y + a_min_y;
            let a_bottom = node.position.y + a_max_y;

            let b_left = other.position.x + b_min_x;
            let b_right = other.position.x + b_max_x;
            let b_top = other.position.y + b_min_y;
            let b_bottom = other.position.y + b_max_y;

            // Calculate separation distance between boxes (negative if overlapping)
            let x_gap = if a_right < b_left {
                b_left - a_right  // A is to the left of B
            } else if b_right < a_left {
                a_left - b_right  // B is to the left of A
            } else {
                0.0  // Overlapping in X
            };

            let y_gap = if a_bottom < b_top {
                b_top - a_bottom  // A is above B
            } else if b_bottom < a_top {
                a_top - b_bottom  // B is above A
            } else {
                0.0  // Overlapping in Y
            };

            // Distance between boxes (0 if overlapping or touching)
            let box_dist = if x_gap > 0.0 && y_gap > 0.0 {
                (x_gap * x_gap + y_gap * y_gap).sqrt()  // Corner distance
            } else if x_gap > 0.0 {
                x_gap  // Horizontal gap only
            } else if y_gap > 0.0 {
                y_gap  // Vertical gap only
            } else {
                // Boxes overlap - calculate overlap amount for stronger repulsion
                let x_overlap = a_right.min(b_right) - a_left.max(b_left);
                let y_overlap = a_bottom.min(b_bottom) - a_top.max(b_top);
                -(x_overlap.min(y_overlap)).max(1.0)  // Negative distance for overlap
            };

            // Direction from other to this node (center to center)
            let diff = node.position - other.position;
            let direction = if diff.length() > 0.1 {
                diff.normalized()
            } else {
                // If centers are nearly identical, push in a random-ish direction
                Vec2::new(1.0, 0.0)
            };

            // Inverse square repulsion based on box distance
            // Add MIN_DISTANCE to prevent division issues and ensure smooth falloff
            let effective_dist = (box_dist + MIN_DISTANCE).max(MIN_DISTANCE);
            let strength = REPULSION_STRENGTH / (effective_dist * effective_dist);
            let repulsion = direction * strength;
            force = force + repulsion;
        }

        // Attraction along edges (to connected nodes)
        for conn_id in &node.connections {
            let connected = match graph.nodes.get(conn_id) {
                Some(n) => n,
                None => continue,
            };

            let diff = connected.position - node.position;
            let dist = diff.length();

            // Spring force: pull toward ideal distance
            let displacement = dist - IDEAL_EDGE_LENGTH;
            let attraction = diff.normalized() * (displacement * ATTRACTION_STRENGTH);
            force = force + attraction;
        }

        // Also handle reverse edges (where this node is the target)
        for edge in &graph.edges {
            if &edge.to == id && !node.connections.contains(&edge.from) {
                let connected = match graph.nodes.get(&edge.from) {
                    Some(n) => n,
                    None => continue,
                };

                let diff = connected.position - node.position;
                let dist = diff.length();
                let displacement = dist - IDEAL_EDGE_LENGTH;
                let attraction = diff.normalized() * (displacement * ATTRACTION_STRENGTH);
                force = force + attraction;
            }
        }

        // Anchors get pulled back to their home position
        if let Some(home) = node.fixed_position {
            let diff = home - node.position;
            let pull = diff * ANCHOR_ATTRACTION;
            force = force + pull;
        }

        // Cap force magnitude to prevent explosions
        let force_mag = force.length();
        if force_mag > MAX_FORCE {
            force = force.normalized() * MAX_FORCE;
        }

        forces.push((id.clone(), force));
    }

    // Apply forces to velocities, then velocities to positions
    for (id, force) in forces {
        if let Some(node) = graph.nodes.get_mut(&id) {
            if node.is_dragged {
                continue;
            }

            // Apply force to velocity
            node.velocity = node.velocity + force * dt;

            // Apply damping
            node.velocity *= VELOCITY_DAMPING;

            // Stop if very slow
            if node.velocity.length() < VELOCITY_THRESHOLD {
                node.velocity = Vec2::zero();
            }

            // Update position
            node.position = node.position + node.velocity * dt;
        }
    }
}

/// Run initial layout iterations to get a stable starting state.
/// Call this once at startup.
pub fn initialize(graph: &mut Graph, iterations: usize) {
    for _ in 0..iterations {
        step(graph, 0.016); // Simulate at ~60fps
    }

    // Clear velocities after initial layout
    for node in graph.nodes.values_mut() {
        node.velocity = Vec2::zero();
    }
}
