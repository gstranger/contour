use contour::Graph;

#[test]
fn caller_up_to_date_returns_empty_diff() {
    let mut g = Graph::new();
    let _ = g.add_node(0.0, 0.0);
    let ver = g.geom_version();

    let diff = g.get_dirty(ver);
    assert_eq!(diff.current_ver, ver);
    assert_eq!(diff.since_ver, ver);
    assert!(!diff.full);
    assert!(diff.nodes_added.is_empty());
    assert!(diff.edges_added.is_empty());
}

#[test]
fn diff_collects_changes_since_a_covered_version() {
    let mut g = Graph::new();
    let before = g.get_dirty(0).current_ver;
    g.dirty_reset();
    let v0 = g.geom_version();

    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(10.0, 0.0);
    let c = g.add_node(5.0, 10.0);
    let e = g.add_edge(a, b).expect("edge add");
    g.add_edge(b, c);
    g.move_node(a, 1.0, 1.0);

    let diff = g.get_dirty(v0);
    assert!(!diff.full, "window should cover v0; got full=true");
    assert!(diff.current_ver > v0);
    assert_eq!(diff.since_ver, v0);
    assert_eq!(diff.nodes_added, vec![a, b, c]);
    assert_eq!(diff.nodes_moved, vec![a]);
    assert_eq!(diff.edges_added.len(), 2);
    assert!(diff.edges_added.contains(&e));
    assert!(diff.bbox.is_some(), "bbox should be populated after mutations");
    let _ = before;
}

#[test]
fn stale_since_returns_full_true() {
    let mut g = Graph::new();
    let _ = g.add_node(0.0, 0.0);
    // Roll the window forward by running a region pass — clear_dirty_flags
    // bumps the internal since_ver to current geom_ver.
    let _ = g.get_regions();
    let new_since = g.get_dirty(0).since_ver;
    assert!(new_since > 0, "internal since_ver should advance past 0");

    // Caller's stale checkpoint (0) is now before the window — must signal full.
    let diff = g.get_dirty(0);
    assert!(diff.full, "stale since should produce full=true");
    assert_eq!(diff.since_ver, new_since);
}

#[test]
fn dirty_reset_clears_accumulated_changes() {
    let mut g = Graph::new();
    g.dirty_reset();
    let checkpoint = g.geom_version();

    let a = g.add_node(0.0, 0.0);
    let _ = g.add_node(1.0, 1.0);

    let pre = g.get_dirty(checkpoint);
    assert!(!pre.full);
    assert!(pre.nodes_added.contains(&a));

    g.dirty_reset();
    let new_checkpoint = g.geom_version();
    let after = g.get_dirty(new_checkpoint);
    assert!(!after.full);
    assert!(after.nodes_added.is_empty());
    assert!(after.nodes_moved.is_empty());
    assert_eq!(after.since_ver, new_checkpoint);
}

#[test]
fn edge_modifications_are_recorded() {
    let mut g = Graph::new();
    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(10.0, 0.0);
    let e = g.add_edge(a, b).expect("edge add");
    let v0 = g.geom_version();

    // Convert to cubic and bend — both should mark the edge modified.
    g.set_edge_cubic(e, 2.0, 1.0, 8.0, 1.0);
    g.bend_edge_to(e, 0.5, 5.0, 3.0, 1.0);

    let diff = g.get_dirty(v0);
    assert!(!diff.full);
    assert!(
        diff.edges_modified.contains(&e),
        "expected edge {} in edges_modified; got {:?}",
        e,
        diff.edges_modified
    );
}
