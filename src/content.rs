use crate::graph::Graph;
use crate::layout;

// Include generated code from build.rs
include!(concat!(env!("OUT_DIR"), "/content_generated.rs"));

/// Build the content graph with all nodes and edges.
/// Content is defined in content/config.toml and content/*.md files.
pub fn build_graph() -> Graph {
    let mut graph = Graph::new();

    // Call generated function that populates the graph
    populate_graph(&mut graph);

    // Run physics layout
    layout::initialize(&mut graph, 200);

    graph
}
