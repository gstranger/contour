//! End-to-end tests for SVG `A`/`a` arc command parsing.

use contour::Graph;

#[test]
fn arc_command_adds_multiple_cubic_edges() {
    // A semicircle becomes a chain of two cubic edges.
    let mut g = Graph::new();
    let added = g.add_svg_path("M 0 0 A 50 50 0 0 1 100 0", None);
    assert_eq!(
        added, 2,
        "expected 2 cubic edges for a semicircle, got {}",
        added
    );
    assert_eq!(g.edge_count(), 2);
    // Three nodes: start, intermediate, end.
    assert_eq!(g.node_count(), 3);
}

#[test]
fn relative_arc_translates_endpoint() {
    // `a` (lowercase) makes the endpoint relative to current position.
    let mut g = Graph::new();
    g.add_svg_path("M 10 20 a 50 50 0 0 1 100 0", None);
    // The end node must sit at the absolute (110, 20).
    let mut saw_start = false;
    let mut saw_end = false;
    for id in 0..(g.node_count() as u32) {
        if let Some((x, y)) = g.get_node(id) {
            if (x - 10.0).abs() < 0.01 && (y - 20.0).abs() < 0.01 {
                saw_start = true;
            }
            if (x - 110.0).abs() < 0.01 && (y - 20.0).abs() < 0.01 {
                saw_end = true;
            }
        }
    }
    assert!(saw_start && saw_end);
}

#[test]
fn flag_parsing_without_separator() {
    // `A50,50 0 0150,0` is legal: rx=50 ry=50 phi=0 large=0 sweep=1 x=50 y=0.
    let mut g1 = Graph::new();
    let n1 = g1.add_svg_path("M 0 0 A50,50 0 0 1 50,0", None);
    let mut g2 = Graph::new();
    let n2 = g2.add_svg_path("M 0 0 A50,50 0 0150,0", None);
    assert!(n1 > 0);
    assert_eq!(
        n1, n2,
        "compact and spaced flag forms should parse identically"
    );
    assert_eq!(g1.node_count(), g2.node_count());
    assert_eq!(g1.edge_count(), g2.edge_count());
}

#[test]
fn zero_radius_arc_emits_line() {
    // With rx=0, the spec says the arc collapses to a line. One edge.
    let mut g = Graph::new();
    let added = g.add_svg_path("M 0 0 A 0 50 0 0 1 100 0", None);
    assert_eq!(added, 1);
    assert_eq!(g.node_count(), 2);
}

#[test]
fn coincident_endpoints_produce_no_edge() {
    // start == end with non-zero radii: spec says no arc.
    let mut g = Graph::new();
    let added = g.add_svg_path("M 50 50 A 30 30 0 0 1 50 50", None);
    assert_eq!(added, 0);
}

#[test]
fn multiple_arcs_chain() {
    // Two A commands sharing implicit cursor position.
    let mut g = Graph::new();
    let added = g.add_svg_path("M 0 0 A 50 50 0 0 1 100 0 A 50 50 0 0 1 200 0", None);
    // Each semicircle splits into 2 cubics, so 4 edges total.
    assert_eq!(added, 4);
}

#[test]
fn implicit_repeat_of_arc_command() {
    // After `A`, additional 7-tuples should be treated as more arcs
    // without re-stating the command letter.
    let mut g = Graph::new();
    let added = g.add_svg_path("M 0 0 A 50 50 0 0 1 100 0 50 50 0 0 1 200 0", None);
    assert_eq!(added, 4);
}

#[test]
fn arc_endpoint_is_quantized_consistently() {
    // The final cubic's end node must coincide with the explicit endpoint,
    // not drift due to floating-point in the conversion. We verify by
    // chaining an arc into a line and confirming the line shares a node.
    let mut g = Graph::new();
    g.add_svg_path("M 0 0 A 50 50 0 0 1 100 0 L 150 0", None);
    // 2 cubics + 1 line = 3 edges, 4 nodes.
    assert_eq!(g.edge_count(), 3);
    assert_eq!(g.node_count(), 4);
}

#[test]
fn close_after_arc_returns_to_subpath_start() {
    // M ... A ... Z should produce an arc and a closing edge back to (0, 0).
    let mut g = Graph::new();
    let added = g.add_svg_path("M 0 0 A 50 50 0 0 1 100 0 Z", None);
    // 2 cubics + 1 closing line.
    assert_eq!(added, 3);
}

#[test]
fn rotated_arc_parses_without_panic() {
    // Non-zero phi parameter. Just checking the parser doesn't choke.
    let mut g = Graph::new();
    let added = g.add_svg_path("M 0 0 A 30 50 45 0 1 80 30", None);
    assert!(added > 0);
}
