//! Integration tests for boolean operations on shapes.

use contour::algorithms::boolean::BoolOp;
use contour::Graph;

/// Helper to create a square shape centered at (cx, cy) with half-width hw.
fn create_square(g: &mut Graph, cx: f32, cy: f32, hw: f32) -> u32 {
    let n0 = g.add_node(cx - hw, cy - hw);
    let n1 = g.add_node(cx + hw, cy - hw);
    let n2 = g.add_node(cx + hw, cy + hw);
    let n3 = g.add_node(cx - hw, cy + hw);

    let e0 = g.add_edge(n0, n1).unwrap();
    let e1 = g.add_edge(n1, n2).unwrap();
    let e2 = g.add_edge(n2, n3).unwrap();
    let e3 = g.add_edge(n3, n0).unwrap();

    g.create_shape(&[e0, e1, e2, e3], true).unwrap()
}

#[test]
fn test_union_overlapping_squares() {
    let mut g = Graph::new();

    // Two overlapping squares
    let shape_a = create_square(&mut g, 0.0, 0.0, 50.0);
    let shape_b = create_square(&mut g, 40.0, 0.0, 50.0);

    let result = g.boolean_op(shape_a, shape_b, BoolOp::Union);
    assert!(result.is_ok());

    let res = result.unwrap();
    assert!(!res.shapes.is_empty(), "Union should produce at least one shape");
}

#[test]
fn test_intersect_overlapping_squares() {
    let mut g = Graph::new();

    // Two overlapping squares
    let shape_a = create_square(&mut g, 0.0, 0.0, 50.0);
    let shape_b = create_square(&mut g, 40.0, 0.0, 50.0);

    let result = g.boolean_op(shape_a, shape_b, BoolOp::Intersect);
    assert!(result.is_ok());

    let res = result.unwrap();
    assert!(!res.shapes.is_empty(), "Intersect should produce at least one shape");
}

#[test]
fn test_difference_overlapping_squares() {
    let mut g = Graph::new();

    // Two overlapping squares
    let shape_a = create_square(&mut g, 0.0, 0.0, 50.0);
    let shape_b = create_square(&mut g, 40.0, 0.0, 50.0);

    let result = g.boolean_op(shape_a, shape_b, BoolOp::Difference);
    assert!(result.is_ok());

    let res = result.unwrap();
    assert!(!res.shapes.is_empty(), "Difference should produce at least one shape");
}

#[test]
fn test_xor_overlapping_squares() {
    let mut g = Graph::new();

    // Two overlapping squares
    let shape_a = create_square(&mut g, 0.0, 0.0, 50.0);
    let shape_b = create_square(&mut g, 40.0, 0.0, 50.0);

    let result = g.boolean_op(shape_a, shape_b, BoolOp::Xor);
    assert!(result.is_ok());

    let res = result.unwrap();
    assert!(!res.shapes.is_empty(), "XOR should produce at least one shape");
}

#[test]
fn test_union_disjoint_squares() {
    let mut g = Graph::new();

    // Two non-overlapping squares
    let shape_a = create_square(&mut g, 0.0, 0.0, 50.0);
    let shape_b = create_square(&mut g, 200.0, 0.0, 50.0);

    let result = g.boolean_op(shape_a, shape_b, BoolOp::Union);
    assert!(result.is_ok());

    let res = result.unwrap();
    // Disjoint union should produce two shapes
    assert_eq!(res.shapes.len(), 2, "Disjoint union should produce two shapes");
}

#[test]
fn test_intersect_disjoint_squares() {
    let mut g = Graph::new();

    // Two non-overlapping squares
    let shape_a = create_square(&mut g, 0.0, 0.0, 50.0);
    let shape_b = create_square(&mut g, 200.0, 0.0, 50.0);

    let result = g.boolean_op(shape_a, shape_b, BoolOp::Intersect);
    assert!(result.is_ok());

    let res = result.unwrap();
    // Disjoint intersection should produce no shapes
    assert!(res.shapes.is_empty(), "Disjoint intersection should be empty");
}

#[test]
fn test_contained_square() {
    let mut g = Graph::new();

    // Large square containing small square
    let shape_a = create_square(&mut g, 0.0, 0.0, 100.0);
    let shape_b = create_square(&mut g, 0.0, 0.0, 30.0);

    // Union of A containing B should just be A
    let union = g.boolean_op(shape_a, shape_b, BoolOp::Union).unwrap();
    assert_eq!(union.shapes.len(), 1, "Union with contained shape should produce one shape");

    // Intersection should be B
    let mut g2 = Graph::new();
    let a2 = create_square(&mut g2, 0.0, 0.0, 100.0);
    let b2 = create_square(&mut g2, 0.0, 0.0, 30.0);
    let inter = g2.boolean_op(a2, b2, BoolOp::Intersect).unwrap();
    assert_eq!(inter.shapes.len(), 1, "Intersection should produce one shape");
}

#[test]
fn test_shape_management() {
    let mut g = Graph::new();

    // Create nodes and edges
    let n0 = g.add_node(0.0, 0.0);
    let n1 = g.add_node(100.0, 0.0);
    let n2 = g.add_node(100.0, 100.0);
    let n3 = g.add_node(0.0, 100.0);

    let e0 = g.add_edge(n0, n1).unwrap();
    let e1 = g.add_edge(n1, n2).unwrap();
    let e2 = g.add_edge(n2, n3).unwrap();
    let e3 = g.add_edge(n3, n0).unwrap();

    // Create shape
    let shape_id = g.create_shape(&[e0, e1, e2, e3], true);
    assert!(shape_id.is_some());
    let sid = shape_id.unwrap();

    // Verify shape exists
    assert!(g.get_shape(sid).is_some());
    assert_eq!(g.shape_count(), 1);

    // Get shape edges
    let edges = g.get_shape_edges(sid);
    assert!(edges.is_some());
    assert_eq!(edges.unwrap().len(), 4);

    // Delete shape
    assert!(g.delete_shape(sid));
    assert!(g.get_shape(sid).is_none());
    assert_eq!(g.shape_count(), 0);
}

#[test]
fn test_infer_shapes() {
    let mut g = Graph::new();

    // Create a closed triangle
    let n0 = g.add_node(0.0, 0.0);
    let n1 = g.add_node(100.0, 0.0);
    let n2 = g.add_node(50.0, 86.6);

    g.add_edge(n0, n1);
    g.add_edge(n1, n2);
    g.add_edge(n2, n0);

    // Infer shapes
    let shapes = g.infer_shapes();
    assert_eq!(shapes.len(), 1, "Should infer one triangle shape");
}

#[test]
fn test_invalid_shape_operations() {
    let mut g = Graph::new();

    // Try to create shape with non-existent edges
    let result = g.create_shape(&[999, 1000], true);
    assert!(result.is_none());

    // Try boolean op on non-existent shapes
    let result = g.boolean_op(999, 1000, BoolOp::Union);
    assert!(result.is_err());
}
