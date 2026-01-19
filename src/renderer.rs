use crate::camera::Camera;
use crate::graph::{EdgeType, Graph, NodeKind, Vec2};
use pulldown_cmark::{html, Parser};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = renderLatex)]
    fn render_latex(element: &web_sys::HtmlElement);
}

const NODE_RADIUS: f64 = 6.0;
const NODE_WITH_CHILDREN_RADIUS: f64 = 9.0; // Slightly larger for nodes with children
const HIGHLIGHT_RADIUS: f64 = 14.0;
const CONTENT_MARGIN: f64 = 500.0; // Render nodes within this margin of viewport

pub struct Renderer {
    canvas: web_sys::HtmlCanvasElement,
    ctx: web_sys::CanvasRenderingContext2d,
    content_container: web_sys::HtmlElement,
    content_elements: HashMap<String, web_sys::HtmlElement>,
    breadcrumb_element: web_sys::HtmlElement,
    rail_indicator: web_sys::HtmlElement,
    width: f64,
    height: f64,
}

impl Renderer {
    pub fn new() -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;

        // Get or create canvas
        let canvas = document
            .get_element_by_id("canvas")
            .ok_or("no canvas element")?
            .dyn_into::<web_sys::HtmlCanvasElement>()?;

        let ctx = canvas
            .get_context("2d")?
            .ok_or("no 2d context")?
            .dyn_into::<web_sys::CanvasRenderingContext2d>()?;

        // Get content container
        let content_container = document
            .get_element_by_id("content")
            .ok_or("no content element")?
            .dyn_into::<web_sys::HtmlElement>()?;

        // Create or get breadcrumb element
        let breadcrumb_element = if let Some(el) = document.get_element_by_id("breadcrumb") {
            el.dyn_into::<web_sys::HtmlElement>()?
        } else {
            let el = document
                .create_element("div")?
                .dyn_into::<web_sys::HtmlElement>()?;
            el.set_id("breadcrumb");
            el.set_class_name("breadcrumb");
            document.body().ok_or("no body")?.append_child(&el)?;
            el
        };

        // Create or get rail mode indicator
        let rail_indicator = if let Some(el) = document.get_element_by_id("rail-indicator") {
            el.dyn_into::<web_sys::HtmlElement>()?
        } else {
            let el = document
                .create_element("div")?
                .dyn_into::<web_sys::HtmlElement>()?;
            el.set_id("rail-indicator");
            el.set_class_name("rail-indicator");
            el.set_inner_html("RAIL MODE <span class=\"rail-hint\">b to exit</span>");
            document.body().ok_or("no body")?.append_child(&el)?;
            el
        };

        // Get initial size
        let width = window.inner_width()?.as_f64().unwrap_or(800.0);
        let height = window.inner_height()?.as_f64().unwrap_or(600.0);

        // Set canvas size
        canvas.set_width(width as u32);
        canvas.set_height(height as u32);

        Ok(Self {
            canvas,
            ctx,
            content_container,
            content_elements: HashMap::new(),
            breadcrumb_element,
            rail_indicator,
            width,
            height,
        })
    }

    pub fn viewport_width(&self) -> f64 {
        self.width
    }

    pub fn viewport_height(&self) -> f64 {
        self.height
    }

    pub fn render(
        &mut self,
        graph: &Graph,
        camera: &Camera,
        rail_mode: bool,
        current_node: &Option<String>,
        depth_context: &Option<String>,
        visible_node_ids: &[String],
        transition_opacity: f64,
        dark_mode: bool,
        active_tags: &[(String, String)],
        highlighted_tag_index: Option<usize>,
    ) -> Result<(), JsValue> {
        // Update canvas size if window resized
        self.update_size()?;

        // Set dark mode class on body for CSS styling
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let body = document.body().ok_or("no body")?;
        if dark_mode {
            body.set_class_name("dark-mode");
        } else {
            body.set_class_name("");
        }

        // Clear canvas with appropriate background
        let bg_color = if dark_mode { "#111111" } else { "#ffffff" };
        self.ctx.set_fill_style_str(bg_color);
        self.ctx.fill_rect(0.0, 0.0, self.width, self.height);

        let zoom = camera.zoom;
        let reading_mode = camera.is_reading_mode();

        // Calculate graph opacity - fade out as we zoom in
        // Start fading at zoom 2.0, fully transparent by zoom 10.0
        let zoom_opacity = if zoom <= 2.0 {
            1.0
        } else if zoom >= 10.0 {
            0.0
        } else {
            1.0 - (zoom - 2.0) / 8.0
        };

        // Combined opacity: zoom-based and transition-based
        let graph_opacity = zoom_opacity * transition_opacity;

        // Show/hide rail mode indicator
        if rail_mode {
            self.rail_indicator.style().set_property("display", "block")?;
        } else {
            self.rail_indicator.style().set_property("display", "none")?;
        }

        // Calculate camera offset (center the camera position in viewport)
        let offset_x = self.width / 2.0 - camera.position.x * zoom;
        let offset_y = self.height / 2.0 - camera.position.y * zoom;

        // Draw graph elements with opacity (skip if fully transparent)
        if graph_opacity > 0.01 {
            // Draw only directed edges between visible nodes
            let visible_edges = graph.get_visible_edges(visible_node_ids);

            for edge in visible_edges {
                // Only draw directed edges here (hierarchy)
                if edge.edge_type != EdgeType::Directed {
                    continue;
                }

                if let (Some(from), Some(to)) = (graph.get_node(&edge.from), graph.get_node(&edge.to)) {
                    let from_screen = Vec2::new(from.position.x * zoom + offset_x, from.position.y * zoom + offset_y);
                    let to_screen = Vec2::new(to.position.x * zoom + offset_x, to.position.y * zoom + offset_y);

                    // Cull if both ends are way off screen
                    if !self.is_line_visible(from_screen, to_screen) {
                        continue;
                    }

                    // Solid line for hierarchy
                    let edge_color = if dark_mode {
                        format!("rgba(180, 180, 180, {})", graph_opacity)
                    } else {
                        format!("rgba(102, 102, 102, {})", graph_opacity)
                    };
                    self.ctx.set_stroke_style_str(&edge_color);
                    self.ctx.set_line_width(zoom.max(1.0) * 1.5);
                    self.ctx.set_line_dash(&js_sys::Array::new())?; // Solid line

                    self.ctx.begin_path();
                    self.ctx.move_to(from_screen.x, from_screen.y);
                    self.ctx.line_to(to_screen.x, to_screen.y);
                    self.ctx.stroke();

                    // Draw arrow at the end
                    self.draw_arrow(from_screen, to_screen, zoom, graph_opacity, dark_mode)?;
                }
            }

            // Draw inferred edges from shared tags (only when tags are active)
            if !active_tags.is_empty() {
                let inferred_edges = graph.get_inferred_edges(visible_node_ids);

                // Color palette for different tags
                let tag_colors: Vec<(&str, &str)> = vec![
                    ("rgba(59, 130, 246, {})", "rgba(96, 165, 250, {})"),   // Blue
                    ("rgba(16, 185, 129, {})", "rgba(52, 211, 153, {})"),   // Green
                    ("rgba(245, 158, 11, {})", "rgba(251, 191, 36, {})"),   // Amber
                    ("rgba(139, 92, 246, {})", "rgba(167, 139, 250, {})"),  // Purple
                ];

                for (from_id, to_id, key, value) in inferred_edges {
                    // Only draw if this tag is active
                    let tag_index = active_tags.iter().position(|(k, v)| k == &key && v == &value);
                    if tag_index.is_none() {
                        continue;
                    }
                    let tag_idx = tag_index.unwrap() % tag_colors.len();

                    if let (Some(from), Some(to)) = (graph.get_node(&from_id), graph.get_node(&to_id)) {
                        let from_screen = Vec2::new(from.position.x * zoom + offset_x, from.position.y * zoom + offset_y);
                        let to_screen = Vec2::new(to.position.x * zoom + offset_x, to.position.y * zoom + offset_y);

                        // Cull if both ends are way off screen
                        if !self.is_line_visible(from_screen, to_screen) {
                            continue;
                        }

                        // Colored line for tag-inferred associations
                        let (light_color, dark_color) = tag_colors[tag_idx];
                        let edge_color = if dark_mode {
                            dark_color.replace("{}", &format!("{}", graph_opacity * 0.8))
                        } else {
                            light_color.replace("{}", &format!("{}", graph_opacity * 0.8))
                        };
                        self.ctx.set_stroke_style_str(&edge_color);
                        self.ctx.set_line_width(2.0 * zoom.max(1.0));

                        // Different dash patterns for different tags
                        let dash_array = js_sys::Array::new();
                        match tag_idx {
                            0 => { // Solid
                            }
                            1 => { // Dashed
                                dash_array.push(&JsValue::from_f64(8.0 * zoom));
                                dash_array.push(&JsValue::from_f64(4.0 * zoom));
                            }
                            2 => { // Dotted
                                dash_array.push(&JsValue::from_f64(2.0 * zoom));
                                dash_array.push(&JsValue::from_f64(4.0 * zoom));
                            }
                            _ => { // Dash-dot
                                dash_array.push(&JsValue::from_f64(8.0 * zoom));
                                dash_array.push(&JsValue::from_f64(4.0 * zoom));
                                dash_array.push(&JsValue::from_f64(2.0 * zoom));
                                dash_array.push(&JsValue::from_f64(4.0 * zoom));
                            }
                        }
                        self.ctx.set_line_dash(&dash_array)?;

                        self.ctx.begin_path();
                        self.ctx.move_to(from_screen.x, from_screen.y);
                        self.ctx.line_to(to_screen.x, to_screen.y);
                        self.ctx.stroke();

                        // Reset dash pattern
                        self.ctx.set_line_dash(&js_sys::Array::new())?;
                    }
                }
            }
        }

        // Draw only visible nodes
        for node_id in visible_node_ids {
            let node = match graph.get_node(node_id) {
                Some(n) => n,
                None => continue,
            };

            let screen_pos = Vec2::new(node.position.x * zoom + offset_x, node.position.y * zoom + offset_y);

            // Check if this is the highlighted node (works in both rail and free mode)
            let is_current = current_node
                .as_ref()
                .map(|id| id == &node.id)
                .unwrap_or(false);

            // In reading mode, only show the current node
            if reading_mode && !is_current {
                if let Some(el) = self.content_elements.get(&node.id) {
                    let _ = el.style().set_property("display", "none");
                }
                continue;
            }

            // Cull if off screen (only in non-reading mode)
            if !reading_mode && !self.is_point_visible(screen_pos, CONTENT_MARGIN) {
                // Hide content element if exists
                if let Some(el) = self.content_elements.get(&node.id) {
                    let _ = el.style().set_property("display", "none");
                }
                continue;
            }

            // Draw graph elements with opacity (skip if fully transparent)
            if graph_opacity > 0.01 {
                // Draw highlight ring if this is the current/closest node
                if is_current {
                    // Use different style for rail mode vs free mode, inverted for dark mode
                    let highlight_color = if rail_mode {
                        if dark_mode {
                            format!("rgba(255, 255, 255, {})", graph_opacity)
                        } else {
                            format!("rgba(0, 0, 0, {})", graph_opacity)
                        }
                    } else {
                        if dark_mode {
                            format!("rgba(180, 180, 180, {})", graph_opacity)
                        } else {
                            format!("rgba(153, 153, 153, {})", graph_opacity)
                        }
                    };
                    self.ctx.set_stroke_style_str(&highlight_color);
                    self.ctx.set_line_width(if rail_mode { 2.0 * zoom } else { 1.5 * zoom });
                    self.ctx.begin_path();
                    self.ctx
                        .arc(screen_pos.x, screen_pos.y, HIGHLIGHT_RADIUS * zoom, 0.0, std::f64::consts::TAU)?;
                    self.ctx.stroke();
                }

                // Determine node radius - larger for nodes with children
                let has_children = graph.has_children(&node.id);
                let radius = if has_children {
                    NODE_WITH_CHILDREN_RADIUS
                } else {
                    NODE_RADIUS
                };

                // Draw node circle with opacity (inverted for dark mode)
                let color = match &node.kind {
                    NodeKind::Collection => if dark_mode {
                        format!("rgba(220, 220, 220, {})", graph_opacity)
                    } else {
                        format!("rgba(51, 51, 51, {})", graph_opacity)
                    },
                    NodeKind::Post { .. } => if dark_mode {
                        format!("rgba(180, 180, 180, {})", graph_opacity)
                    } else {
                        format!("rgba(102, 102, 102, {})", graph_opacity)
                    },
                };

                self.ctx.set_fill_style_str(&color);
                self.ctx.begin_path();
                self.ctx
                    .arc(screen_pos.x, screen_pos.y, radius * zoom, 0.0, std::f64::consts::TAU)?;
                self.ctx.fill();

                // Draw small "+" indicator for nodes with children
                if has_children {
                    let plus_color = if dark_mode {
                        format!("rgba(17, 17, 17, {})", graph_opacity)
                    } else {
                        format!("rgba(255, 255, 255, {})", graph_opacity)
                    };
                    self.ctx.set_stroke_style_str(&plus_color);
                    self.ctx.set_line_width(zoom.max(1.0));

                    // Horizontal line of +
                    self.ctx.begin_path();
                    self.ctx.move_to(screen_pos.x - 3.0 * zoom, screen_pos.y);
                    self.ctx.line_to(screen_pos.x + 3.0 * zoom, screen_pos.y);
                    self.ctx.stroke();

                    // Vertical line of +
                    self.ctx.begin_path();
                    self.ctx.move_to(screen_pos.x, screen_pos.y - 3.0 * zoom);
                    self.ctx.line_to(screen_pos.x, screen_pos.y + 3.0 * zoom);
                    self.ctx.stroke();
                }
            }

            // Calculate tooltip offset direction (away from connected nodes)
            let tooltip_offset = self.calculate_tooltip_offset(graph, &node.id, visible_node_ids);

            // Update or create content element
            self.update_content_element(node, screen_pos, is_current, reading_mode, graph_opacity, active_tags, tooltip_offset, highlighted_tag_index)?;
        }

        // Hide content elements for non-visible nodes
        for (node_id, el) in &self.content_elements {
            if !visible_node_ids.contains(node_id) {
                let _ = el.style().set_property("display", "none");
            }
        }

        // Update breadcrumb display
        self.update_breadcrumb(graph, depth_context)?;

        Ok(())
    }

    /// Draw an arrow at the end of a directed edge
    fn draw_arrow(&self, from: Vec2, to: Vec2, zoom: f64, opacity: f64, dark_mode: bool) -> Result<(), JsValue> {
        let arrow_size = 8.0 * zoom;

        // Calculate direction
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.01 {
            return Ok(());
        }

        let dir_x = dx / len;
        let dir_y = dy / len;

        // Arrow point is slightly before the "to" position (to account for node radius)
        let node_radius = NODE_RADIUS * zoom;
        let arrow_tip_x = to.x - dir_x * node_radius;
        let arrow_tip_y = to.y - dir_y * node_radius;

        // Perpendicular direction
        let perp_x = -dir_y;
        let perp_y = dir_x;

        // Arrow base points
        let base_x = arrow_tip_x - dir_x * arrow_size;
        let base_y = arrow_tip_y - dir_y * arrow_size;

        let left_x = base_x + perp_x * arrow_size * 0.5;
        let left_y = base_y + perp_y * arrow_size * 0.5;

        let right_x = base_x - perp_x * arrow_size * 0.5;
        let right_y = base_y - perp_y * arrow_size * 0.5;

        // Draw filled arrow (inverted for dark mode)
        let arrow_color = if dark_mode {
            format!("rgba(180, 180, 180, {})", opacity)
        } else {
            format!("rgba(102, 102, 102, {})", opacity)
        };
        self.ctx.set_fill_style_str(&arrow_color);
        self.ctx.begin_path();
        self.ctx.move_to(arrow_tip_x, arrow_tip_y);
        self.ctx.line_to(left_x, left_y);
        self.ctx.line_to(right_x, right_y);
        self.ctx.close_path();
        self.ctx.fill();

        Ok(())
    }

    /// Update the breadcrumb display
    fn update_breadcrumb(&self, graph: &Graph, depth_context: &Option<String>) -> Result<(), JsValue> {
        if let Some(ref context_id) = depth_context {
            // Build breadcrumb path
            let mut path_titles: Vec<String> = Vec::new();

            // Walk up the parent chain
            let mut current = Some(context_id.clone());
            while let Some(ref id) = current {
                if let Some(node) = graph.get_node(id) {
                    path_titles.push(node.title.clone());
                    current = node.parent.clone();
                } else {
                    break;
                }
            }

            // Reverse to get root-first order
            path_titles.reverse();

            // Format breadcrumb
            let breadcrumb_html = format!(
                "<span class=\"breadcrumb-back\">← Esc</span> {}",
                path_titles.join(" › ")
            );

            self.breadcrumb_element.set_inner_html(&breadcrumb_html);
            self.breadcrumb_element.style().set_property("display", "block")?;
        } else {
            self.breadcrumb_element.style().set_property("display", "none")?;
        }

        Ok(())
    }

    fn update_size(&mut self) -> Result<(), JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let width = window.inner_width()?.as_f64().unwrap_or(800.0);
        let height = window.inner_height()?.as_f64().unwrap_or(600.0);

        if (width - self.width).abs() > 1.0 || (height - self.height).abs() > 1.0 {
            self.width = width;
            self.height = height;
            self.canvas.set_width(width as u32);
            self.canvas.set_height(height as u32);
        }

        Ok(())
    }

    fn is_point_visible(&self, pos: Vec2, margin: f64) -> bool {
        pos.x > -margin
            && pos.x < self.width + margin
            && pos.y > -margin
            && pos.y < self.height + margin
    }

    fn is_line_visible(&self, from: Vec2, to: Vec2) -> bool {
        // Simple bounding box check
        let min_x = from.x.min(to.x);
        let max_x = from.x.max(to.x);
        let min_y = from.y.min(to.y);
        let max_y = from.y.max(to.y);

        max_x > -CONTENT_MARGIN
            && min_x < self.width + CONTENT_MARGIN
            && max_y > -CONTENT_MARGIN
            && min_y < self.height + CONTENT_MARGIN
    }

    /// Calculate the direction to offset a tooltip away from connected nodes
    fn calculate_tooltip_offset(&self, graph: &Graph, node_id: &str, visible_node_ids: &[String]) -> Vec2 {
        let node = match graph.get_node(node_id) {
            Some(n) => n,
            None => return Vec2::new(1.0, 0.0), // Default to right
        };

        // Collect all connected node positions
        let mut connected_positions: Vec<Vec2> = Vec::new();

        // Get edges involving this node
        for edge in &graph.edges {
            let connected_id = if edge.from == node_id {
                &edge.to
            } else if edge.to == node_id {
                &edge.from
            } else {
                continue;
            };

            // Only consider visible nodes
            if !visible_node_ids.contains(connected_id) {
                continue;
            }

            if let Some(connected_node) = graph.get_node(connected_id) {
                connected_positions.push(connected_node.position);
            }
        }

        // Also consider tag-inferred connections
        for (key, value) in &node.tags {
            for other_id in graph.get_nodes_with_tag(key, value) {
                if other_id == node_id || !visible_node_ids.contains(&other_id) {
                    continue;
                }
                if let Some(other_node) = graph.get_node(&other_id) {
                    // Avoid duplicates
                    let already_added = connected_positions.iter().any(|p| {
                        (p.x - other_node.position.x).abs() < 0.1 && (p.y - other_node.position.y).abs() < 0.1
                    });
                    if !already_added {
                        connected_positions.push(other_node.position);
                    }
                }
            }
        }

        // If no connections, default to right
        if connected_positions.is_empty() {
            return Vec2::new(1.0, 0.0);
        }

        // Calculate average direction FROM connected nodes TO this node
        let mut avg_direction = Vec2::zero();
        for pos in &connected_positions {
            let dx = node.position.x - pos.x;
            let dy = node.position.y - pos.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len > 0.1 {
                avg_direction.x += dx / len;
                avg_direction.y += dy / len;
            }
        }

        // Normalize the result
        let len = (avg_direction.x * avg_direction.x + avg_direction.y * avg_direction.y).sqrt();
        if len > 0.1 {
            Vec2::new(avg_direction.x / len, avg_direction.y / len)
        } else {
            // If directions cancel out (node is surrounded), default to right
            Vec2::new(1.0, 0.0)
        }
    }

    fn update_content_element(
        &mut self,
        node: &crate::graph::Node,
        screen_pos: Vec2,
        is_current: bool,
        reading_mode: bool,
        graph_opacity: f64,
        active_tags: &[(String, String)],
        tooltip_offset: Vec2, // Direction to offset tooltip (away from connected nodes)
        highlighted_tag_index: Option<usize>, // Currently highlighted tag (Tab cycling)
    ) -> Result<(), JsValue> {
        let el = if let Some(el) = self.content_elements.get(&node.id) {
            el.clone()
        } else {
            // Create new element
            let document = web_sys::window()
                .ok_or("no window")?
                .document()
                .ok_or("no document")?;

            let el = document
                .create_element("div")?
                .dyn_into::<web_sys::HtmlElement>()?;

            el.set_attribute("data-node-id", &node.id)?;

            self.content_container.append_child(&el)?;
            self.content_elements.insert(node.id.clone(), el.clone());
            el
        };

        let style = el.style();

        // In reading mode, show full markdown article centered on screen
        if reading_mode && is_current && node.article().is_some() {
            let article = node.article().unwrap();

            // Parse markdown to HTML
            let parser = Parser::new(article);
            let mut html_output = String::new();
            html::push_html(&mut html_output, parser);

            el.set_inner_html(&format!(
                "<article class=\"reading-article\">{}</article>",
                html_output
            ));
            el.set_class_name("node-content node-reading");

            // Render LaTeX math expressions
            render_latex(&el);

            // Center the article on screen
            style.set_property("display", "block")?;
            style.set_property("position", "fixed")?;
            style.set_property("left", "50%")?;
            style.set_property("top", "50%")?;
            style.set_property("transform", "translate(-50%, -50%)")?;
            style.set_property("transform-origin", "center center")?;

            return Ok(());
        }

        // Determine what content to show based on zoom level
        let title_class = match &node.kind {
            NodeKind::Collection => "node-title anchor-title",
            NodeKind::Post { .. } => "node-title",
        };

        // Build tag chips HTML for highlighted node
        let tags_html = if is_current && !node.tags.is_empty() {
            // Color classes for different tags
            let tag_colors = ["tag-blue", "tag-green", "tag-amber", "tag-purple"];

            let chips: Vec<String> = node.tags.iter().enumerate().map(|(idx, (key, value))| {
                // Check if this tag is active
                let is_active = active_tags.iter().any(|(k, v)| k == key && v == value);
                let active_class = if is_active { " tag-active" } else { "" };

                // Check if this tag is highlighted (Tab cycling)
                let is_highlighted = highlighted_tag_index == Some(idx);
                let highlighted_class = if is_highlighted { " tag-highlighted" } else { "" };

                // Find color index if active
                let color_idx = if is_active {
                    active_tags.iter().position(|(k, v)| k == key && v == value).unwrap_or(0) % tag_colors.len()
                } else {
                    0
                };
                let color_class = if is_active { tag_colors[color_idx] } else { "" };

                format!(
                    "<span class=\"tag-chip{}{}{}\" data-tag-key=\"{}\" data-tag-value=\"{}\">{}: {}</span>",
                    active_class, highlighted_class, if is_active { format!(" {}", color_class) } else { String::new() },
                    key, value, key, value
                )
            }).collect();

            format!("<div class=\"tag-chips\">{}</div>", chips.join(""))
        } else {
            String::new()
        };

        // Normal mode - show title, summary, and tags
        el.set_inner_html(&format!(
            "<div class=\"{}\">{}</div><div class=\"node-body\">{}</div>{}",
            title_class, node.title, node.summary, tags_html
        ));

        // Update class based on current state
        let class_name = if is_current {
            "node-content node-current"
        } else {
            "node-content"
        };
        el.set_class_name(class_name);

        // Update position - positioned relative to node, offset away from connected nodes
        style.set_property("display", "block")?;
        style.set_property("position", "absolute")?;

        // Calculate position and transform based on offset direction
        let offset_distance = 20.0; // Base distance from node
        let offset_x = tooltip_offset.x * offset_distance;
        let offset_y = tooltip_offset.y * offset_distance;

        // Determine which side of the node the tooltip should be on
        // and adjust transform-origin accordingly
        let (transform, transform_origin) = if tooltip_offset.x < -0.3 {
            // Tooltip goes to the left
            ("translate(-100%, -50%)", "right center")
        } else if tooltip_offset.x > 0.3 {
            // Tooltip goes to the right
            ("translate(0%, -50%)", "left center")
        } else if tooltip_offset.y < 0.0 {
            // Tooltip goes above
            ("translate(-50%, -100%)", "center bottom")
        } else {
            // Tooltip goes below
            ("translate(-50%, 0%)", "center top")
        };

        style.set_property("left", &format!("{}px", screen_pos.x + offset_x))?;
        style.set_property("top", &format!("{}px", screen_pos.y + offset_y))?;
        style.set_property("transform", transform)?;
        style.set_property("transform-origin", transform_origin)?;

        // Apply opacity to non-current nodes during zoom transition
        if is_current {
            style.set_property("opacity", "1")?;
        } else {
            style.set_property("opacity", &format!("{}", graph_opacity))?;
        }

        Ok(())
    }
}
