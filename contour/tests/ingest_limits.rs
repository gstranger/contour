use contour::Graph;
use serde_json::json;

#[test]
fn json_caps_exceeded_nodes() {
    let mut g = Graph::new();
    let too_many = 200_001usize;
    let nodes: Vec<_> = (0..too_many)
        .map(|i| json!({"id": i, "x": 0.0, "y": 0.0}))
        .collect();
    let v = json!({"version":1, "nodes": nodes, "edges": [], "fills": []});
    let ok = g.from_json_value(v);
    assert!(!ok, "expected failure on nodes cap");
}

#[test]
fn json_invalid_numbers() {
    let mut g = Graph::new();
    let v = json!({"version":1, "nodes":[{"id":0,"x":1.0e38,"y":0.0}], "edges":[], "fills": []});
    assert!(!g.from_json_value(v));
}

#[test]
fn svg_overlong_d_returns_zero() {
    let mut g = Graph::new();
    let long = "M 0 0 L 1 1 ".repeat(1_000_000); // exceeds token cap
    let added = g.add_svg_path(&long, None);
    assert_eq!(added, 0);
}
