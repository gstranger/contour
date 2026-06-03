use contour::Graph;

#[test]
fn add_node_undo_redo() {
    let mut g = Graph::new();
    let id = g.add_node(100.0, 200.0);
    assert!(g.get_node(id).is_some());

    let (label, remaining) = g.undo().unwrap();
    assert_eq!(label, "add node");
    assert_eq!(remaining, 0);
    assert!(g.get_node(id).is_none());

    let (label2, _) = g.redo().unwrap();
    assert_eq!(label2, "add node");
    let node = g.get_node(id).unwrap();
    assert_eq!(node, (100.0, 200.0));
}

#[test]
fn remove_node_cascades_edges() {
    let mut g = Graph::new();
    let center = g.add_node(50.0, 50.0);
    let n1 = g.add_node(0.0, 0.0);
    let n2 = g.add_node(100.0, 0.0);
    let n3 = g.add_node(100.0, 100.0);
    let _ = g.add_edge(center, n1).unwrap();
    let _ = g.add_edge(center, n2).unwrap();
    let _ = g.add_edge(center, n3).unwrap();

    assert_eq!(g.edge_count(), 3);
    assert!(g.remove_node(center));
    assert_eq!(g.edge_count(), 0);

    // Undo restore
    let _ = g.undo();
    assert!(g.get_node(center).is_some());
    assert_eq!(g.edge_count(), 3);
    let all_edges = g.get_edge_arrays();
    assert_eq!(all_edges.ids.len(), 3);
}

#[test]
fn gesture_group_merges() {
    let mut g = Graph::new();
    let id = g.add_node(0.0, 0.0);

    g.begin_undo_group("drag".into());
    for i in 1..=10 {
        g.move_node(id, i as f32 * 10.0, i as f32 * 10.0);
    }
    assert!(g.end_undo_group());

    let (x, _) = g.get_node(id).unwrap();
    assert_eq!(x, 100.0);

    let (label, remaining) = g.undo().unwrap();
    assert_eq!(label, "drag");
    assert_eq!(remaining, 1); // "add node" command still in stack

    let (x2, _) = g.get_node(id).unwrap();
    assert_eq!(x2, 0.0);
}

#[test]
fn nested_groups_merge() {
    let mut g = Graph::new();
    let id = g.add_node(10.0, 10.0);

    g.begin_undo_group("outer".into());
    g.move_node(id, 20.0, 20.0);
    g.begin_undo_group("inner".into());
    g.move_node(id, 30.0, 30.0);
    assert!(!g.end_undo_group());
    g.move_node(id, 40.0, 40.0);
    assert!(g.end_undo_group());

    let _ = g.undo();
    let (x, _) = g.get_node(id).unwrap();
    assert_eq!(x, 10.0);

    let _ = g.redo();
    let (x2, _) = g.get_node(id).unwrap();
    assert_eq!(x2, 40.0);
}

#[test]
fn gestural_then_structural_undo() {
    let mut g = Graph::new();
    let id = g.add_node(0.0, 0.0);

    g.begin_undo_group("drag".into());
    g.move_node(id, 50.0, 50.0);
    g.end_undo_group();

    let _ = g.undo();
    let (x, _) = g.get_node(id).unwrap();
    assert_eq!(x, 0.0);

    let _ = g.undo();
    assert!(g.get_node(id).is_none());
}

#[test]
fn redo_after_mutation_clears_redo() {
    let mut g = Graph::new();
    let id = g.add_node(0.0, 0.0);
    g.move_node(id, 50.0, 50.0);
    let _ = g.undo();
    let _ = g.undo();
    let _ = g.add_node(100.0, 100.0);
    assert!(g.redo().is_none());
}

#[test]
fn depth_cap_evicts_oldest() {
    let mut g = Graph::new();
    for i in 0..(contour::undo::MAX_UNDO_DEPTH + 10) {
        let _ = g.add_node(i as f32 * 10.0, 0.0);
    }

    let mut undo_count = 0;
    while g.undo().is_some() {
        undo_count += 1;
    }
    assert_eq!(undo_count, contour::undo::MAX_UNDO_DEPTH);
}

#[test]
fn undo_during_group_returns_none() {
    let mut g = Graph::new();
    let _ = g.add_node(0.0, 0.0);
    g.begin_undo_group("thing".into());
    assert!(g.undo().is_none());
    g.end_undo_group();
}

#[test]
fn geom_ver_bumps_on_undo() {
    let mut g = Graph::new();
    let v1 = g.geom_version();
    let _ = g.add_node(0.0, 0.0);
    let v2 = g.geom_version();
    assert!(v2 > v1);
    let _ = g.undo();
    let v3 = g.geom_version();
    assert!(v3 > v2);
}

#[test]
fn clear_resets_undo() {
    let mut g = Graph::new();
    let _ = g.add_node(0.0, 0.0);
    let _ = g.undo();
    let _ = g.redo();
    g.clear();
    assert!(g.undo().is_none());
    assert!(g.redo().is_none());
}

#[test]
fn json_load_resets_undo() {
    let mut g = Graph::new();
    let _ = g.add_node(0.0, 0.0);

    g.from_json_value(serde_json::json!({"version": 1, "nodes": [], "edges": []}));
    assert!(g.undo().is_none());
}

#[test]
fn snapshot_redo_captures_current() {
    let mut g = Graph::new();
    let id = g.add_node(0.0, 0.0);

    g.begin_undo_group("drag".into());
    g.move_node(id, 50.0, 50.0);
    assert!(g.end_undo_group());

    let _ = g.undo();
    let (x, _) = g.get_node(id).unwrap();
    assert_eq!(x, 0.0);

    let _ = g.redo();
    let (x2, _) = g.get_node(id).unwrap();
    assert_eq!(x2, 50.0);
}

#[test]
fn empty_gesture_group_not_committed() {
    let mut g = Graph::new();
    g.begin_undo_group("empty".into());
    assert!(!g.end_undo_group());
    assert!(g.undo().is_none());
}

#[test]
fn edge_add_undo_redo() {
    let mut g = Graph::new();
    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(100.0, 0.0);
    let _eid = g.add_edge(a, b).unwrap();
    assert_eq!(g.edge_count(), 1);

    let _ = g.undo(); // undo add edge
    assert_eq!(g.edge_count(), 0);

    let _ = g.undo(); // undo add node b
    let _ = g.undo(); // undo add node a
    assert_eq!(g.node_count(), 0);

    // Redo all three
    let _ = g.redo(); // redo add node a
    let _ = g.redo(); // redo add node b
    let _ = g.redo(); // redo add edge
    assert_eq!(g.node_count(), 2);
    assert_eq!(g.edge_count(), 1);
}
