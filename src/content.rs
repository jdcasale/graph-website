use crate::graph::{vec2, Graph};
use crate::layout;

/// Build the content graph with all nodes and edges.
/// This is where you define your site's content structure.
pub fn build_graph() -> Graph {
    let mut graph = Graph::new();

    // ═══════════════════════════════════════════════════════════════
    // ROOT LEVEL - Main categories (these appear at top level)
    // ═══════════════════════════════════════════════════════════════

    graph.add_root(
        "home",
        vec2(0.0, 0.0),
        "Welcome",
        "Navigate with h/j/k/l or arrow keys.\nClick a node to jump to it.\nPress gg to return home.\nPress b for rail mode.\nPress d for dark mode.\nPress Enter to drill into nodes.",
        None,
    );

    graph.add_root(
        "projects",
        vec2(600.0, -200.0),
        "Projects",
        "Things I've built.\nEnter to explore.",
        None,
    );

    graph.add_root(
        "writing",
        vec2(-500.0, 200.0),
        "Writing",
        "Essays and notes.\nEnter to explore.",
        None,
    );

    graph.add_root(
        "about",
        vec2(400.0, 400.0),
        "About",
        "Who I am.",
        Some(r#"# CLAUDE PLACEHOLDER ARTICLE: About

I'm a developer who enjoys building tools that help people think.

This website is an experiment in non-linear navigation—a way to explore ideas spatially rather than through traditional menus and links.

## Contact

Feel free to reach out if you'd like to chat about any of the projects or ideas here."#),
    );

    // Undirected edges between root nodes (associations)
    graph.add_association("projects", "writing", Some("topic: programming"));

    // ═══════════════════════════════════════════════════════════════
    // CHILDREN OF "projects" - Drill into "projects" to see these
    // ═══════════════════════════════════════════════════════════════

    graph.add_child(
        "project-alpha",
        "projects",
        "Project Alpha",
        "A cool thing I made.\nIt does interesting stuff.",
        Some(r#"# CLAUDE PLACEHOLDER ARTICLE: Project Alpha

This is a longer description of Project Alpha that becomes visible when you zoom in.

The project started as an experiment in building interactive visualizations. What began as a weekend hack turned into something much more interesting.

## Key Features

- Real-time data processing
- WebGL rendering pipeline
- Custom physics simulation

The most challenging part was getting the performance right. Early versions would choke on anything more than a few hundred elements. After profiling and optimizing the hot paths, it now handles thousands smoothly.

## Lessons Learned

1. Profile before optimizing
2. The browser is faster than you think
3. But the DOM is slower than you think"#),
    );

    graph.add_child(
        "project-beta",
        "projects",
        "Project Beta",
        "A CLI tool with sub-features.\nEnter to explore.",
        None, // No article - has children instead
    );

    graph.add_child(
        "project-gamma",
        "projects",
        "Project Gamma",
        "Yet another thing.\nStill pretty neat.",
        Some(r#"# CLAUDE PLACEHOLDER ARTICLE: Project Gamma

A cross-platform utility for automating repetitive tasks.

Built with simplicity in mind—no configuration files, just command-line flags and sensible defaults."#),
    );

    // ═══════════════════════════════════════════════════════════════
    // CHILDREN OF "project-beta" - Drill into "project-beta" to see these
    // ═══════════════════════════════════════════════════════════════

    graph.add_child(
        "beta-search",
        "project-beta",
        "Search",
        "Full-text search feature.",
        Some(r#"# CLAUDE PLACEHOLDER ARTICLE: Project Beta: Search

The search feature uses a custom inverted index for fast full-text search across all your notes.

## How It Works

1. Text is tokenized and normalized
2. Tokens are stored in an inverted index
3. Queries are parsed and matched against the index
4. Results are ranked by relevance

The index is persistent and updates incrementally as you add or modify notes."#),
    );

    graph.add_child(
        "beta-graph",
        "project-beta",
        "Graph View",
        "Visualize connections.",
        Some(r#"# CLAUDE PLACEHOLDER ARTICLE: Project Beta: Graph View

The graph view shows how your notes connect to each other.

Connections are detected automatically based on:
- Explicit links between notes
- Shared tags and keywords
- Semantic similarity (using embeddings)

This visualization was the inspiration for this website's design."#),
    );

    graph.add_child(
        "beta-export",
        "project-beta",
        "Export",
        "Export to various formats.",
        Some(r#"# CLAUDE PLACEHOLDER ARTICLE: Project Beta: Export

Export your knowledge base to different formats:

- **Markdown**: Plain text, portable
- **HTML**: Static site generation
- **JSON**: For programmatic access
- **PDF**: For printing or sharing

All exports preserve links and structure."#),
    );

    // ═══════════════════════════════════════════════════════════════
    // CHILDREN OF "writing" - Drill into "writing" to see these
    // ═══════════════════════════════════════════════════════════════

    graph.add_child(
        "essay-minimalism",
        "writing",
        "On Minimalism",
        "Less is more, usually.\nSometimes less is just less.",
        Some(r#"# CLAUDE PLACEHOLDER ARTICLE: On Minimalism

There's a certain kind of simplicity that's actually complexity in disguise. A "minimal" interface that hides essential controls. A "clean" API that forces you to write boilerplate elsewhere.

**True minimalism isn't about having less. It's about having exactly what you need—no more, no less.**

The best tools disappear. You don't think about the hammer, you think about the nail. You don't think about the text editor, you think about the text.

## This Website

This principle guided the design of this website:

- No navigation menus because the content *is* the navigation
- No search box because you explore by moving through space
- The interface is the content

Of course, this approach has tradeoffs. Discoverability suffers. New visitors might feel lost. But for the kind of slow, exploratory reading I want to encourage, getting a little lost is part of the point."#),
    );

    graph.add_child(
        "essay-tools",
        "writing",
        "Tools for Thought",
        "The tools we use shape how we think.\nChoose wisely.",
        Some(r#"# CLAUDE PLACEHOLDER ARTICLE: Tools for Thought

> "We shape our tools and thereafter our tools shape us."
> — Marshall McLuhan

Nowhere is this more true than in software.

The spreadsheet didn't just make accounting faster—it changed what questions we ask. When everything looks like a grid of numbers, you start to see the world that way.

The same is true for text editors, note-taking apps, and programming languages. Each one encodes assumptions about how ideas should be structured, connected, and transformed.

## What Would an Exploration Tool Look Like?

I've been thinking about what a tool for thought optimized for exploration would look like:

- Not hierarchical like a file system
- Not linear like a document
- Something more like a garden—or a map

This website is an experiment in that direction."#),
    );

    graph.add_child(
        "note-rust",
        "writing",
        "Notes on Rust",
        "The borrow checker is your friend.\nEventually.",
        Some(r#"# CLAUDE PLACEHOLDER ARTICLE: Notes on Rust

Learning Rust felt like learning to program all over again.

The borrow checker is infamous for rejecting code that "obviously" works. But after a while, you start to see what it sees. Those "obviously correct" programs often had subtle bugs waiting to happen.

## The Mental Model Shift

- Stop thinking about objects with identity
- Start thinking about values with ownership
- Data flows through your program like water

Once it clicks, you find yourself writing better code in other languages too. You notice when you're holding references too long, when ownership is unclear, when data races could occur.

## A Mathematical Aside

Ownership can be thought of formally. If we let $O(v)$ represent the owner of value $v$, then:

$$\forall v : |O(v)| = 1$$

That is, every value has exactly one owner at any given time. The borrow checker enforces this invariant at compile time, which is why Rust can guarantee memory safety without a garbage collector.

The complexity of the borrow checker is roughly $O(n \cdot m)$ where $n$ is the number of variables and $m$ is the number of lifetimes.

## The Bigger Lesson

Rust didn't teach me how to write Rust. It taught me how to think about memory, concurrency, and program structure in a deeper way.

*This website is written in Rust, compiled to WebAssembly.*"#),
    );

    // ═══════════════════════════════════════════════════════════════
    // UNDIRECTED ASSOCIATIONS - Cross-hierarchy connections
    // ═══════════════════════════════════════════════════════════════

    // project-alpha is associated with note-rust (both about Rust/programming)
    graph.add_association("project-alpha", "note-rust", Some("language: rust"));

    // beta-graph is associated with essay-tools (both about visualization/tools)
    graph.add_association("beta-graph", "essay-tools", Some("topic: visualization"));

    // project-gamma connects writing and projects
    graph.add_association("project-gamma", "essay-minimalism", Some("topic: simplicity"));

    // ═══════════════════════════════════════════════════════════════
    // RUN INITIAL LAYOUT
    // ═══════════════════════════════════════════════════════════════

    layout::initialize(&mut graph, 200);

    graph
}
