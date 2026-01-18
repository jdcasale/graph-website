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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeType {
    Anchor,
    Content,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub position: Vec2,
    pub velocity: Vec2,
    pub fixed_position: Option<Vec2>,
    pub node_type: NodeType,
    pub title: String,
    pub content: String,
    pub article: Option<String>, // Full article text, visible when zoomed in
    pub connections: Vec<String>,
    pub is_dragged: bool,
    pub parent: Option<String>, // Primary parent for breadcrumb (from directed edge)
}

impl Node {
    pub fn is_fixed(&self) -> bool {
        self.fixed_position.is_some() && !self.is_dragged
    }

    pub fn is_anchor(&self) -> bool {
        self.fixed_position.is_some()
    }

    /// Estimate the bounding box of the text content.
    /// Returns (width, height) in pixels.
    /// The box is positioned to the right of the node point.
    pub fn estimate_bounds(&self) -> (f64, f64) {
        // Estimate based on content (title + body)
        // CSS: max-width 280px, font-size ~14px, line-height 1.5
        let char_width = 8.0; // Approximate monospace character width
        let line_height = 21.0; // 14px * 1.5
        let max_width = 280.0;
        let padding = 12.0; // Some padding around text

        // Calculate title width
        let title_width = (self.title.len() as f64 * char_width).min(max_width);

        // Calculate content dimensions
        let content_lines: Vec<&str> = self.content.lines().collect();
        let max_line_len = content_lines.iter()
            .map(|line| line.len())
            .max()
            .unwrap_or(0);
        let content_width = (max_line_len as f64 * char_width).min(max_width);

        let width = title_width.max(content_width) + padding;
        let height = line_height + (content_lines.len() as f64 * line_height) + padding;

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

    pub fn add_anchor(
        &mut self,
        id: &str,
        position: Vec2,
        title: &str,
        content: &str,
    ) {
        let node = Node {
            id: id.to_string(),
            position,
            velocity: Vec2::zero(),
            fixed_position: Some(position),
            node_type: NodeType::Anchor,
            title: title.to_string(),
            content: content.to_string(),
            article: None,
            connections: Vec::new(),
            is_dragged: false,
            parent: None,
        };
        self.nodes.insert(id.to_string(), node);
    }

    pub fn add_content(
        &mut self,
        id: &str,
        title: &str,
        content: &str,
        article: Option<&str>,
        connected_to: &[&str],
    ) {
        // Start position near first connection (will be adjusted by layout)
        let start_pos = if let Some(first) = connected_to.first() {
            if let Some(anchor) = self.nodes.get(*first) {
                Vec2::new(
                    anchor.position.x + 150.0 + (self.nodes.len() as f64 * 20.0),
                    anchor.position.y + 50.0,
                )
            } else {
                Vec2::zero()
            }
        } else {
            Vec2::zero()
        };

        let node = Node {
            id: id.to_string(),
            position: start_pos,
            velocity: Vec2::zero(),
            fixed_position: None,
            node_type: NodeType::Content,
            title: title.to_string(),
            content: content.to_string(),
            article: article.map(|s| s.to_string()),
            connections: connected_to.iter().map(|s| s.to_string()).collect(),
            is_dragged: false,
            parent: None,
        };
        self.nodes.insert(id.to_string(), node);

        // Add edges (undirected for backwards compatibility)
        for target in connected_to {
            self.edges.push(Edge {
                from: id.to_string(),
                to: target.to_string(),
                edge_type: EdgeType::Undirected,
                label: None,
            });
        }
    }

    pub fn get_node(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// Add a root node (appears at top level, no parent)
    pub fn add_root(
        &mut self,
        id: &str,
        position: Vec2,
        title: &str,
        content: &str,
        article: Option<&str>,
    ) {
        let node = Node {
            id: id.to_string(),
            position,
            velocity: Vec2::zero(),
            fixed_position: Some(position),
            node_type: NodeType::Anchor,
            title: title.to_string(),
            content: content.to_string(),
            article: article.map(|s| s.to_string()),
            connections: Vec::new(),
            is_dragged: false,
            parent: None,
        };
        self.nodes.insert(id.to_string(), node);
    }

    /// Add a child node with directed edge from parent
    pub fn add_child(
        &mut self,
        id: &str,
        parent_id: &str,
        title: &str,
        content: &str,
        article: Option<&str>,
    ) {
        // Start position near parent (will be adjusted by layout)
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
            node_type: NodeType::Content,
            title: title.to_string(),
            content: content.to_string(),
            article: article.map(|s| s.to_string()),
            connections: vec![parent_id.to_string()],
            is_dragged: false,
            parent: Some(parent_id.to_string()),
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
    /// - Plus any undirected associations of those children
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

        // Get undirected associations of children
        for child_id in &children {
            for edge in &self.edges {
                if edge.edge_type == EdgeType::Undirected {
                    let assoc_id = if edge.from == *child_id {
                        Some(&edge.to)
                    } else if edge.to == *child_id {
                        Some(&edge.from)
                    } else {
                        None
                    };

                    if let Some(id) = assoc_id {
                        if !result.contains(id) {
                            result.push(id.clone());
                        }
                    }
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
}

pub fn vec2(x: f64, y: f64) -> Vec2 {
    Vec2::new(x, y)
}
