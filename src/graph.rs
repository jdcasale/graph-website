use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Default)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn normalized(&self) -> Self {
        let len = self.length();
        if len == 0.0 {
            Self::zero()
        } else {
            Self {
                x: self.x / len,
                y: self.y / len,
            }
        }
    }
}

impl std::ops::Add for Vec2 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl std::ops::AddAssign for Vec2 {
    fn add_assign(&mut self, other: Self) {
        self.x += other.x;
        self.y += other.y;
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

impl std::ops::Mul<f64> for Vec2 {
    type Output = Self;
    fn mul(self, scalar: f64) -> Self {
        Self {
            x: self.x * scalar,
            y: self.y * scalar,
        }
    }
}

impl std::ops::MulAssign<f64> for Vec2 {
    fn mul_assign(&mut self, scalar: f64) {
        self.x *= scalar;
        self.y *= scalar;
    }
}

/// The kind of node - determines what content it can have
#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    /// A collection node that contains other nodes (no article)
    Collection,
    /// A post/article node with full content
    Post { article: String },
    // Future types:
    // Media { url: String, media_type: MediaType },
    // Link { url: String, description: String },
}

impl NodeKind {
    /// Returns the article content if this is a Post node
    pub fn article(&self) -> Option<&str> {
        match self {
            NodeKind::Post { article } => Some(article),
            _ => None,
        }
    }

    /// Returns true if this node has children (is a collection)
    pub fn is_collection(&self) -> bool {
        matches!(self, NodeKind::Collection)
    }

    /// Returns the type name for display/tags
    pub fn type_name(&self) -> &'static str {
        match self {
            NodeKind::Collection => "collection",
            NodeKind::Post { .. } => "post",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub position: Vec2,
    pub velocity: Vec2,
    pub fixed_position: Option<Vec2>,
    pub kind: NodeKind,
    pub title: String,
    pub summary: String, // Brief description shown in graph view
    pub connections: Vec<String>,
    pub is_dragged: bool,
    pub parent: Option<String>, // Primary parent for breadcrumb (from directed edge)
    pub tags: Vec<(String, String)>, // Key-value tags for inferring connections
}

impl Node {
    pub fn is_fixed(&self) -> bool {
        self.fixed_position.is_some() && !self.is_dragged
    }

    /// Returns the article content if this is a Post node
    pub fn article(&self) -> Option<&str> {
        self.kind.article()
    }

    /// Estimate the bounding box of the text content.
    /// Returns (width, height) in pixels.
    /// The box is positioned to the right of the node point.
    pub fn estimate_bounds(&self) -> (f64, f64) {
        // Estimate based on content (title + summary)
        // CSS: max-width 280px, font-size ~14px, line-height 1.5
        let char_width = 8.0; // Approximate monospace character width
        let line_height = 21.0; // 14px * 1.5
        let max_width = 280.0;
        let padding = 12.0; // Some padding around text

        // Calculate title width
        let title_width = (self.title.len() as f64 * char_width).min(max_width);

        // Calculate summary dimensions
        let summary_lines: Vec<&str> = self.summary.lines().collect();
        let max_line_len = summary_lines.iter()
            .map(|line| line.len())
            .max()
            .unwrap_or(0);
        let summary_width = (max_line_len as f64 * char_width).min(max_width);

        let width = title_width.max(summary_width) + padding;
        let height = line_height + (summary_lines.len() as f64 * line_height) + padding;

        (width, height)
    }

    /// Get the bounding box corners relative to node position.
    /// Returns (min_x, min_y, max_x, max_y) offsets from node.position.
    /// The text box is positioned to the right and slightly above the node point.
    pub fn bounds_offsets(&self) -> (f64, f64, f64, f64) {
        let (width, height) = self.estimate_bounds();
        let offset_x = 15.0; // Text is offset to the right of node
        let offset_y = -10.0; // Text is offset slightly up

        (offset_x, offset_y, offset_x + width, offset_y + height)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EdgeType {
    Directed,   // Parent→child relationship (hierarchy)
    Undirected, // Association/relationship (topic, vibe, etc.)
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
    pub label: Option<String>, // Optional: "topic", "uses-language", etc.
}

#[derive(Debug, Clone)]
pub struct Graph {
    pub nodes: HashMap<String, Node>,
    pub edges: Vec<Edge>,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }

    pub fn get_node(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    // ═══════════════════════════════════════════════════════════════
    // TYPE-SAFE NODE CREATION
    // ═══════════════════════════════════════════════════════════════

    /// Add a root-level collection (no parent, contains children)
    pub fn add_collection(
        &mut self,
        id: &str,
        position: Vec2,
        title: &str,
        summary: &str,
    ) {
        let node = Node {
            id: id.to_string(),
            position,
            velocity: Vec2::zero(),
            fixed_position: Some(position),
            kind: NodeKind::Collection,
            title: title.to_string(),
            summary: summary.to_string(),
            connections: Vec::new(),
            is_dragged: false,
            parent: None,
            tags: Vec::new(),
        };
        self.nodes.insert(id.to_string(), node);
    }

    /// Add a child collection under a parent (contains children)
    pub fn add_child_collection(
        &mut self,
        id: &str,
        parent_id: &str,
        title: &str,
        summary: &str,
    ) {
        let start_pos = if let Some(parent) = self.nodes.get(parent_id) {
            Vec2::new(
                parent.position.x + 150.0 + (self.nodes.len() as f64 * 20.0),
                parent.position.y + 50.0,
            )
        } else {
            Vec2::zero()
        };

        let node = Node {
            id: id.to_string(),
            position: start_pos,
            velocity: Vec2::zero(),
            fixed_position: None,
            kind: NodeKind::Collection,
            title: title.to_string(),
            summary: summary.to_string(),
            connections: vec![parent_id.to_string()],
            is_dragged: false,
            parent: Some(parent_id.to_string()),
            tags: Vec::new(),
        };
        self.nodes.insert(id.to_string(), node);

        // Add directed edge from parent to child
        self.edges.push(Edge {
            from: parent_id.to_string(),
            to: id.to_string(),
            edge_type: EdgeType::Directed,
            label: None,
        });
    }

    /// Add a root-level post (no parent, has article content)
    pub fn add_post(
        &mut self,
        id: &str,
        position: Vec2,
        title: &str,
        summary: &str,
        article: &str,
    ) {
        let node = Node {
            id: id.to_string(),
            position,
            velocity: Vec2::zero(),
            fixed_position: Some(position),
            kind: NodeKind::Post { article: article.to_string() },
            title: title.to_string(),
            summary: summary.to_string(),
            connections: Vec::new(),
            is_dragged: false,
            parent: None,
            tags: Vec::new(),
        };
        self.nodes.insert(id.to_string(), node);
    }

    /// Add a child post under a parent (has article content)
    pub fn add_child_post(
        &mut self,
        id: &str,
        parent_id: &str,
        title: &str,
        summary: &str,
        article: &str,
    ) {
        let start_pos = if let Some(parent) = self.nodes.get(parent_id) {
            Vec2::new(
                parent.position.x + 150.0 + (self.nodes.len() as f64 * 20.0),
                parent.position.y + 50.0,
            )
        } else {
            Vec2::zero()
        };

        let node = Node {
            id: id.to_string(),
            position: start_pos,
            velocity: Vec2::zero(),
            fixed_position: None,
            kind: NodeKind::Post { article: article.to_string() },
            title: title.to_string(),
            summary: summary.to_string(),
            connections: vec![parent_id.to_string()],
            is_dragged: false,
            parent: Some(parent_id.to_string()),
            tags: Vec::new(),
        };
        self.nodes.insert(id.to_string(), node);

        // Add directed edge from parent to child
        self.edges.push(Edge {
            from: parent_id.to_string(),
            to: id.to_string(),
            edge_type: EdgeType::Directed,
            label: None,
        });
    }

    /// Add an undirected association edge between two nodes
    pub fn add_association(&mut self, node_a: &str, node_b: &str, label: Option<&str>) {
        // Add to connections list of both nodes
        if let Some(node) = self.nodes.get_mut(node_a) {
            if !node.connections.contains(&node_b.to_string()) {
                node.connections.push(node_b.to_string());
            }
        }
        if let Some(node) = self.nodes.get_mut(node_b) {
            if !node.connections.contains(&node_a.to_string()) {
                node.connections.push(node_a.to_string());
            }
        }

        self.edges.push(Edge {
            from: node_a.to_string(),
            to: node_b.to_string(),
            edge_type: EdgeType::Undirected,
            label: label.map(|s| s.to_string()),
        });
    }

    /// Get nodes connected by directed edges FROM this node (children)
    pub fn get_children(&self, node_id: &str) -> Vec<&Node> {
        self.edges
            .iter()
            .filter(|e| e.from == node_id && e.edge_type == EdgeType::Directed)
            .filter_map(|e| self.nodes.get(&e.to))
            .collect()
    }

    /// Get nodes connected by undirected edges to this node
    pub fn get_associations(&self, node_id: &str) -> Vec<&Node> {
        self.edges
            .iter()
            .filter(|e| e.edge_type == EdgeType::Undirected)
            .filter_map(|e| {
                if e.from == node_id {
                    self.nodes.get(&e.to)
                } else if e.to == node_id {
                    self.nodes.get(&e.from)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get parent via directed edge TO this node
    pub fn get_parent(&self, node_id: &str) -> Option<&Node> {
        self.nodes
            .get(node_id)
            .and_then(|n| n.parent.as_ref())
            .and_then(|parent_id| self.nodes.get(parent_id))
    }

    /// Get root nodes - nodes with no directed edges pointing to them
    pub fn get_root_nodes(&self) -> Vec<&Node> {
        self.nodes
            .values()
            .filter(|n| n.parent.is_none())
            .collect()
    }

    /// Check if a node has children (directed edges FROM it)
    pub fn has_children(&self, node_id: &str) -> bool {
        self.edges
            .iter()
            .any(|e| e.from == node_id && e.edge_type == EdgeType::Directed)
    }

    /// Get nodes to display in a subgraph context:
    /// - The context node's children (directed edges)
    /// - Plus any nodes that share tags with those children (inferred associations)
    pub fn get_subgraph_nodes(&self, context_id: &str) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();

        // Get direct children
        let children: Vec<String> = self
            .edges
            .iter()
            .filter(|e| e.from == context_id && e.edge_type == EdgeType::Directed)
            .map(|e| e.to.clone())
            .collect();

        result.extend(children.clone());

        // Get nodes that share tags with children (inferred associations)
        for child_id in &children {
            let shared_tag_nodes = self.get_nodes_with_shared_tags(child_id);
            for node_id in shared_tag_nodes {
                if !result.contains(&node_id) {
                    result.push(node_id);
                }
            }
        }

        result
    }

    /// Get edges that connect any of the given visible nodes
    pub fn get_visible_edges(&self, visible_node_ids: &[String]) -> Vec<&Edge> {
        self.edges
            .iter()
            .filter(|e| visible_node_ids.contains(&e.from) && visible_node_ids.contains(&e.to))
            .collect()
    }

    // ═══════════════════════════════════════════════════════════════
    // TAG-BASED METHODS
    // ═══════════════════════════════════════════════════════════════

    /// Add a tag to a node
    pub fn tag(&mut self, node_id: &str, key: &str, value: &str) {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.tags.push((key.to_string(), value.to_string()));
        }
    }

    /// Calculate how many of the active tags this node matches
    /// Returns (matches, total_active) for computing emphasis
    pub fn tag_match_score(&self, node_id: &str, active_tags: &[(String, String)]) -> (usize, usize) {
        if active_tags.is_empty() {
            return (0, 0);
        }
        let node = match self.nodes.get(node_id) {
            Some(n) => n,
            None => return (0, active_tags.len()),
        };
        let matches = active_tags.iter()
            .filter(|(k, v)| node.tags.iter().any(|(nk, nv)| nk == k && nv == v))
            .count();
        (matches, active_tags.len())
    }

    /// Check if two nodes share any tags (same key AND value)
    pub fn nodes_share_tag(&self, node_a: &str, node_b: &str) -> Option<(String, String)> {
        let a = self.nodes.get(node_a)?;
        let b = self.nodes.get(node_b)?;

        for (key_a, val_a) in &a.tags {
            for (key_b, val_b) in &b.tags {
                if key_a == key_b && val_a == val_b {
                    return Some((key_a.clone(), val_a.clone()));
                }
            }
        }
        None
    }

    /// Get all inferred edges from shared tags among visible nodes.
    /// Returns tuples of (node_a, node_b, tag_key, tag_value) for ALL shared tags.
    pub fn get_inferred_edges(&self, visible_node_ids: &[String]) -> Vec<(String, String, String, String)> {
        let mut edges: Vec<(String, String, String, String)> = Vec::new();

        for i in 0..visible_node_ids.len() {
            for j in (i + 1)..visible_node_ids.len() {
                let id_a = &visible_node_ids[i];
                let id_b = &visible_node_ids[j];

                // Get ALL shared tags between these nodes, not just the first
                let shared_tags = self.get_all_shared_tags(id_a, id_b);
                for (key, value) in shared_tags {
                    edges.push((id_a.clone(), id_b.clone(), key, value));
                }
            }
        }

        edges
    }

    /// Get all tags shared between two nodes (not just the first one)
    fn get_all_shared_tags(&self, node_a: &str, node_b: &str) -> Vec<(String, String)> {
        let a = match self.nodes.get(node_a) {
            Some(n) => n,
            None => return Vec::new(),
        };
        let b = match self.nodes.get(node_b) {
            Some(n) => n,
            None => return Vec::new(),
        };

        let mut shared: Vec<(String, String)> = Vec::new();
        for (key_a, val_a) in &a.tags {
            for (key_b, val_b) in &b.tags {
                if key_a == key_b && val_a == val_b {
                    shared.push((key_a.clone(), val_a.clone()));
                }
            }
        }
        shared
    }

    /// Get all nodes that have a specific tag (key, value)
    pub fn get_nodes_with_tag(&self, key: &str, value: &str) -> Vec<String> {
        self.nodes
            .iter()
            .filter(|(_, node)| node.tags.contains(&(key.to_string(), value.to_string())))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Get nodes that share any tag with the given node
    pub fn get_nodes_with_shared_tags(&self, node_id: &str) -> Vec<String> {
        let node = match self.nodes.get(node_id) {
            Some(n) => n,
            None => return Vec::new(),
        };

        let mut result: Vec<String> = Vec::new();

        for (other_id, other_node) in &self.nodes {
            if other_id == node_id {
                continue;
            }

            // Check if any tags match
            for (key, value) in &node.tags {
                if other_node.tags.contains(&(key.clone(), value.clone())) {
                    if !result.contains(other_id) {
                        result.push(other_id.clone());
                    }
                    break;
                }
            }
        }

        result
    }
}

pub fn vec2(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}
