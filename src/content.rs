use crate::graph::{vec2, Graph};
use crate::layout;

/// Build the content graph with all nodes and edges.
/// This is where you define your site's content structure.
pub fn build_graph() -> Graph {
    let mut graph = Graph::new();

    // ═══════════════════════════════════════════════════════════════
    // ANCHOR NODES (categories with fixed positions)
    // ═══════════════════════════════════════════════════════════════

    graph.add_anchor(
        "home",
        vec2(0.0, 0.0),
        "Welcome",
        "Navigate with h/j/k/l or arrow keys.\nClick a node to jump to it.\nPress gg to return home.\nPress bb for rail mode.",
    );

    graph.add_anchor(
        "projects",
        vec2(600.0, -200.0),
        "Projects",
        "Things I've built.",
    );

    graph.add_anchor(
        "writing",
        vec2(-500.0, 200.0),
        "Writing",
        "Essays and notes.",
    );

    graph.add_anchor(
        "about",
        vec2(400.0, 400.0),
        "About",
        "Who I am.",
    );

    // ═══════════════════════════════════════════════════════════════
    // CONTENT NODES (positioned relative to anchors by layout)
    // ═══════════════════════════════════════════════════════════════

    graph.add_content(
        "project-alpha",
        "Project Alpha",
        "A cool thing I made.\nIt does interesting stuff.",
        Some(r#"# Project Alpha

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
        &["projects"],
    );

    graph.add_content(
        "project-beta",
        "Project Beta",
        "Another project.\nThis one is different.",
        Some(r#"# Project Beta

A command-line tool for managing personal knowledge bases.

I built this because I was frustrated with existing note-taking apps. They either lock you into proprietary formats or require too much manual organization.

This tool takes a different approach: plain text files with minimal markup, automatic linking based on content similarity, and a fast full-text search.

## Usage

```bash
$ beta add "Today I learned about..."
$ beta search "that thing from last week"
$ beta graph --output=viz.html
```

The graph visualization was actually the seed for this website's design."#),
        &["projects"],
    );

    graph.add_content(
        "project-gamma",
        "Project Gamma",
        "Yet another thing.\nStill pretty neat.",
        None,
        &["projects", "writing"],
    );

    graph.add_content(
        "essay-minimalism",
        "On Minimalism",
        "Less is more, usually.\nSometimes less is just less.",
        Some(r#"# On Minimalism

There's a certain kind of simplicity that's actually complexity in disguise. A "minimal" interface that hides essential controls. A "clean" API that forces you to write boilerplate elsewhere.

**True minimalism isn't about having less. It's about having exactly what you need—no more, no less.**

The best tools disappear. You don't think about the hammer, you think about the nail. You don't think about the text editor, you think about the text.

## This Website

This principle guided the design of this website:

- No navigation menus because the content *is* the navigation
- No search box because you explore by moving through space
- The interface is the content

Of course, this approach has tradeoffs. Discoverability suffers. New visitors might feel lost. But for the kind of slow, exploratory reading I want to encourage, getting a little lost is part of the point."#),
        &["writing"],
    );

    graph.add_content(
        "essay-tools",
        "Tools for Thought",
        "The tools we use shape how we think.\nChoose wisely.",
        Some(r#"# Tools for Thought

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
        &["writing"],
    );

    graph.add_content(
        "note-rust",
        "Notes on Rust",
        "The borrow checker is your friend.\nEventually.",
        Some(r#"# Notes on Rust

Learning Rust felt like learning to program all over again.

The borrow checker is infamous for rejecting code that "obviously" works. But after a while, you start to see what it sees. Those "obviously correct" programs often had subtle bugs waiting to happen.

## The Mental Model Shift

- Stop thinking about objects with identity
- Start thinking about values with ownership
- Data flows through your program like water

Once it clicks, you find yourself writing better code in other languages too. You notice when you're holding references too long, when ownership is unclear, when data races could occur.

## The Bigger Lesson

Rust didn't teach me how to write Rust. It taught me how to think about memory, concurrency, and program structure in a deeper way.

*This website is written in Rust, compiled to WebAssembly.*"#),
        &["writing", "projects"],
    );

    // ═══════════════════════════════════════════════════════════════
    // RUN INITIAL LAYOUT
    // ═══════════════════════════════════════════════════════════════

    layout::initialize(&mut graph, 200);

    graph
}
