use crate::camera::Camera;
use crate::graph::{Graph, NodeType, Vec2};
use pulldown_cmark::{html, Parser};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

const NODE_RADIUS: f64 = 6.0;
const HIGHLIGHT_RADIUS: f64 = 14.0;
const CONTENT_MARGIN: f64 = 500.0; // Render nodes within this margin of viewport

pub struct Renderer {
    canvas: web_sys::HtmlCanvasElement,
    ctx: web_sys::CanvasRenderingContext2d,
    content_container: web_sys::HtmlElement,
    content_elements: HashMap<String, web_sys::HtmlElement>,
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
    ) -> Result<(), JsValue> {
        // Update canvas size if window resized
        self.update_size()?;

        // Clear canvas
        self.ctx.set_fill_style_str("#ffffff");
        self.ctx.fill_rect(0.0, 0.0, self.width, self.height);

        let zoom = camera.zoom;
        let reading_mode = camera.is_reading_mode();

        // Calculate graph opacity - fade out as we zoom in
        // Start fading at zoom 2.0, fully transparent by zoom 10.0
        let graph_opacity = if zoom <= 2.0 {
            1.0
        } else if zoom >= 10.0 {
            0.0
        } else {
            1.0 - (zoom - 2.0) / 8.0
        };

        // Calculate camera offset (center the camera position in viewport)
        let offset_x = self.width / 2.0 - camera.position.x * zoom;
        let offset_y = self.height / 2.0 - camera.position.y * zoom;

        // Draw graph elements with opacity (skip if fully transparent)
        if graph_opacity > 0.01 {
            // Draw edges with opacity
            let edge_color = format!("rgba(204, 204, 204, {})", graph_opacity);
            self.ctx.set_stroke_style_str(&edge_color);
            self.ctx.set_line_width(zoom.max(1.0));

            for edge in &graph.edges {
                if let (Some(from), Some(to)) = (graph.get_node(&edge.from), graph.get_node(&edge.to)) {
                    let from_screen = Vec2::new(from.position.x * zoom + offset_x, from.position.y * zoom + offset_y);
                    let to_screen = Vec2::new(to.position.x * zoom + offset_x, to.position.y * zoom + offset_y);

                    // Cull if both ends are way off screen
                    if !self.is_line_visible(from_screen, to_screen) {
                        continue;
                    }

                    self.ctx.begin_path();
                    self.ctx.move_to(from_screen.x, from_screen.y);
                    self.ctx.line_to(to_screen.x, to_screen.y);
                    self.ctx.stroke();
                }
            }
        }

        // Draw nodes
        for node in graph.nodes.values() {
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
                    // Use different style for rail mode vs free mode
                    let highlight_color = if rail_mode {
                        format!("rgba(0, 0, 0, {})", graph_opacity)
                    } else {
                        format!("rgba(153, 153, 153, {})", graph_opacity)
                    };
                    self.ctx.set_stroke_style_str(&highlight_color);
                    self.ctx.set_line_width(if rail_mode { 2.0 * zoom } else { 1.5 * zoom });
                    self.ctx.begin_path();
                    self.ctx
                        .arc(screen_pos.x, screen_pos.y, HIGHLIGHT_RADIUS * zoom, 0.0, std::f64::consts::TAU)?;
                    self.ctx.stroke();
                }

                // Draw node circle with opacity
                let color = match node.node_type {
                    NodeType::Anchor => format!("rgba(51, 51, 51, {})", graph_opacity),
                    NodeType::Content => format!("rgba(102, 102, 102, {})", graph_opacity),
                };

                self.ctx.set_fill_style_str(&color);
                self.ctx.begin_path();
                self.ctx
                    .arc(screen_pos.x, screen_pos.y, NODE_RADIUS * zoom, 0.0, std::f64::consts::TAU)?;
                self.ctx.fill();
            }

            // Update or create content element
            self.update_content_element(node, screen_pos, is_current, reading_mode, graph_opacity)?;
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

    fn update_content_element(
        &mut self,
        node: &crate::graph::Node,
        screen_pos: Vec2,
        is_current: bool,
        reading_mode: bool,
        graph_opacity: f64,
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
        if reading_mode && is_current && node.article.is_some() {
            let article = node.article.as_ref().unwrap();

            // Parse markdown to HTML
            let parser = Parser::new(article);
            let mut html_output = String::new();
            html::push_html(&mut html_output, parser);

            el.set_inner_html(&format!(
                "<article class=\"reading-article\">{}</article>",
                html_output
            ));
            el.set_class_name("node-content node-reading");

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
        let title_class = match node.node_type {
            NodeType::Anchor => "node-title anchor-title",
            NodeType::Content => "node-title",
        };

        // Normal mode - show title and summary
        el.set_inner_html(&format!(
            "<div class=\"{}\">{}</div><div class=\"node-body\">{}</div>",
            title_class, node.title, node.content
        ));

        // Update class based on current state
        let class_name = if is_current {
            "node-content node-current"
        } else {
            "node-content"
        };
        el.set_class_name(class_name);

        // Update position - positioned relative to node
        style.set_property("display", "block")?;
        style.set_property("position", "absolute")?;
        style.set_property("left", &format!("{}px", screen_pos.x + 15.0))?;
        style.set_property("top", &format!("{}px", screen_pos.y - 10.0))?;
        style.set_property("transform", "none")?;
        style.set_property("transform-origin", "top left")?;

        // Apply opacity to non-current nodes during zoom transition
        if is_current {
            style.set_property("opacity", "1")?;
        } else {
            style.set_property("opacity", &format!("{}", graph_opacity))?;
        }

        Ok(())
    }
}
