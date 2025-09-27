use contour::Graph;
use serde_json::json;

#[test]
fn json_edges_with_missing_nodes_are_ignored() {
    let mut g = Graph::new();
    // Construct a JSON doc with one valid node and one edge referencing a missing node
    let doc = json!({
        "version": 1,
        "nodes": [ {"id":0, "x":0.0, "y":0.0} ],
        "edges": [ {"id":0, "a":0, "b":42, "kind":"line"} ],
        "fills": []
    });
    let ok = g.from_json_value(doc);
    assert!(ok, "from_json_value should succeed and not panic");
    // Should have 1 node, 0 edges effectively usable
    assert_eq!(g.node_count(), 1);
    assert_eq!(g.edge_count(), 0);
    // Downstream calls should not panic
    let _ = g.get_regions();
    let _ = g.to_svg_paths();
}

#[test]
fn set_handle_pos_invalid_end_is_noop() {
    let mut g = Graph::new();
    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(100.0, 0.0);
    let e = g.add_edge(a, b).unwrap();
    // Convert to cubic
    assert!(g.set_edge_cubic(e, 25.0, 0.0, 75.0, 0.0));
    // Invalid end should return false and not panic
    assert_eq!(g.set_handle_pos(e, 2, 10.0, 10.0), false);
}
