use contour::algorithms::planarize::planarize_graph;
use contour::geometry::tolerance::EPS_FACE_AREA;
use contour::Graph;
use proptest::prelude::*;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
enum Op {
    AddNode {
        x: i16,
        y: i16,
    },
    MoveNode {
        idx: u16,
        dx: i8,
        dy: i8,
    },
    RemoveNode {
        idx: u16,
    },
    AddEdge {
        a: u16,
        b: u16,
    },
    RemoveEdge {
        idx: u16,
    },
    BendEdge {
        idx: u16,
        t_num: u8,
        tx: i8,
        ty: i8,
    },
    SetHandleMode {
        idx: u16,
        mode: u8,
    },
    SetEdgeCubic {
        idx: u16,
        p1x: i8,
        p1y: i8,
        p2x: i8,
        p2y: i8,
    },
    SetHandlePos {
        idx: u16,
        end: u8,
        x: i8,
        y: i8,
    },
    AddSvgPath {
        kind: u8,
    },
    AddFreehand {
        seed: u8,
        n: u8,
        close: bool,
    },
    Pick {
        x: i16,
        y: i16,
        tol_num: u8,
    },
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        (any::<i16>(), any::<i16>()).prop_map(|(x, y)| Op::AddNode { x, y }),
        (any::<u16>(), any::<i8>(), any::<i8>()).prop_map(|(idx, dx, dy)| Op::MoveNode {
            idx,
            dx,
            dy,
        }),
        any::<u16>().prop_map(|idx| Op::RemoveNode { idx }),
        (any::<u16>(), any::<u16>()).prop_map(|(a, b)| Op::AddEdge { a, b }),
        any::<u16>().prop_map(|idx| Op::RemoveEdge { idx }),
        (any::<u16>(), any::<u8>(), any::<i8>(), any::<i8>())
            .prop_map(|(idx, t_num, tx, ty)| Op::BendEdge { idx, t_num, tx, ty },),
        (any::<u16>(), (0u8..=2u8)).prop_map(|(idx, mode)| Op::SetHandleMode { idx, mode }),
        (
            any::<u16>(),
            any::<i8>(),
            any::<i8>(),
            any::<i8>(),
            any::<i8>()
        )
            .prop_map(|(idx, p1x, p1y, p2x, p2y)| Op::SetEdgeCubic {
                idx,
                p1x,
                p1y,
                p2x,
                p2y,
            }),
        (any::<u16>(), (0u8..=1u8), any::<i8>(), any::<i8>())
            .prop_map(|(idx, end, x, y)| Op::SetHandlePos { idx, end, x, y }),
        (0u8..=3u8).prop_map(|kind| Op::AddSvgPath { kind }),
        (any::<u8>(), any::<u8>(), any::<bool>()).prop_map(|(seed, n, close)| Op::AddFreehand {
            seed,
            n,
            close
        }),
        (any::<i16>(), any::<i16>(), any::<u8>()).prop_map(|(x, y, tol_num)| Op::Pick {
            x,
            y,
            tol_num
        }),
    ]
}

#[derive(Default)]
struct ModelState {
    nodes: Vec<u32>,
    edges: Vec<u32>,
}

fn sync_state(g: &Graph, state: &mut ModelState) {
    let (node_ids, _) = g.get_node_arrays();
    state.nodes = node_ids;
    let edge_arrays = g.get_edge_arrays();
    state.edges = edge_arrays.ids;
}

fn pick_svg(kind: u8) -> &'static str {
    match kind % 4 {
        0 => "M0 0 L10 0 L10 10 L0 10 Z",
        1 => "M0 0 C5 5 10 5 10 0 L10 10 L0 10 Z",
        2 => "M-5 -5 L5 5 M5 -5 L-5 5",
        _ => "M0 0 L1 0 L1 1 Z M5 5 L6 5 L6 6 Z",
    }
}

fn freehand_points(seed: u8, n: u8) -> Vec<(f32, f32)> {
    let n = (n as usize % 32) + 2;
    let s = seed as f32;
    (0..n)
        .map(|i| {
            let t = i as f32 * 0.3 + s * 0.07;
            (t.cos() * 5.0, t.sin() * 5.0)
        })
        .collect()
}

fn apply_op(g: &mut Graph, state: &ModelState, op: Op) {
    match op {
        Op::AddNode { x, y } => {
            let _ = g.add_node(x as f32 * 0.1, y as f32 * 0.1);
        }
        Op::MoveNode { idx, dx, dy } => {
            if state.nodes.is_empty() {
                return;
            }
            let nid = state.nodes[(idx as usize) % state.nodes.len()];
            if let Some((x, y)) = g.get_node(nid) {
                let nx = x + (dx as f32 * 0.05);
                let ny = y + (dy as f32 * 0.05);
                let _ = g.move_node(nid, nx, ny);
            }
        }
        Op::RemoveNode { idx } => {
            if state.nodes.is_empty() {
                return;
            }
            let nid = state.nodes[(idx as usize) % state.nodes.len()];
            let _ = g.remove_node(nid);
        }
        Op::AddEdge { a, b } => {
            if state.nodes.len() < 2 {
                return;
            }
            let aid = state.nodes[(a as usize) % state.nodes.len()];
            let bid = state.nodes[(b as usize) % state.nodes.len()];
            if aid == bid {
                return;
            }
            let _ = g.add_edge(aid, bid);
        }
        Op::RemoveEdge { idx } => {
            if state.edges.is_empty() {
                return;
            }
            let eid = state.edges[(idx as usize) % state.edges.len()];
            let _ = g.remove_edge(eid);
        }
        Op::BendEdge { idx, t_num, tx, ty } => {
            if state.edges.is_empty() {
                return;
            }
            let eid = state.edges[(idx as usize) % state.edges.len()];
            let t = (t_num as f32 / 255.0).clamp(0.05, 0.95);
            if let Some((cx, cy)) = centroid_of_edge(g, eid) {
                let target_x = cx + (tx as f32 * 0.1);
                let target_y = cy + (ty as f32 * 0.1);
                let _ = g.bend_edge_to(eid, t, target_x, target_y, 1.0);
            }
        }
        Op::SetHandleMode { idx, mode } => {
            if state.edges.is_empty() {
                return;
            }
            let eid = state.edges[(idx as usize) % state.edges.len()];
            let _ = g.set_handle_mode(eid, mode);
        }
        Op::SetEdgeCubic {
            idx,
            p1x,
            p1y,
            p2x,
            p2y,
        } => {
            if state.edges.is_empty() {
                return;
            }
            let eid = state.edges[(idx as usize) % state.edges.len()];
            let _ = g.set_edge_cubic(
                eid,
                p1x as f32 * 0.1,
                p1y as f32 * 0.1,
                p2x as f32 * 0.1,
                p2y as f32 * 0.1,
            );
        }
        Op::SetHandlePos { idx, end, x, y } => {
            if state.edges.is_empty() {
                return;
            }
            let eid = state.edges[(idx as usize) % state.edges.len()];
            let _ = g.set_handle_pos(eid, end, x as f32 * 0.1, y as f32 * 0.1);
        }
        Op::AddSvgPath { kind } => {
            let _ = g.add_svg_path(pick_svg(kind), None);
        }
        Op::AddFreehand { seed, n, close } => {
            let pts = freehand_points(seed, n);
            let _ = g.add_freehand(&pts, close);
        }
        Op::Pick { x, y, tol_num } => {
            let tol = (tol_num as f32 / 255.0) * 5.0;
            let _ = g.pick(x as f32 * 0.1, y as f32 * 0.1, tol);
        }
    }
}

fn centroid_of_edge(g: &Graph, eid: u32) -> Option<(f32, f32)> {
    let arrays = g.get_edge_arrays();
    for (i, id) in arrays.ids.iter().enumerate() {
        if *id == eid {
            let a = arrays.endpoints[2 * i];
            let b = arrays.endpoints[2 * i + 1];
            let (ax, ay) = g.get_node(a)?;
            let (bx, by) = g.get_node(b)?;
            return Some(((ax + bx) * 0.5, (ay + by) * 0.5));
        }
    }
    None
}

fn assert_invariants(g: &mut Graph, prev_ver: u64) {
    // Counts consistent with array accessors
    let (node_ids, node_pos) = g.get_node_arrays();
    prop_assert_local(
        node_ids.len() == g.node_count() as usize,
        "node_count mismatch",
    );
    prop_assert_local(
        node_pos.len() == node_ids.len() * 2,
        "node positions array shape mismatch",
    );

    let edge_arrays = g.get_edge_arrays();
    prop_assert_local(
        edge_arrays.ids.len() == g.edge_count() as usize,
        "edge_count mismatch",
    );
    prop_assert_local(
        edge_arrays.endpoints.len() == edge_arrays.ids.len() * 2,
        "endpoints array shape mismatch",
    );
    prop_assert_local(
        edge_arrays.kinds.len() == edge_arrays.ids.len(),
        "kinds array shape mismatch",
    );

    // geom_version is monotonic non-decreasing
    prop_assert_local(g.geom_version() >= prev_ver, "geom_version went backwards");

    // No dangling references; no self-loops
    let node_set: HashSet<u32> = node_ids.iter().copied().collect();
    for i in 0..edge_arrays.ids.len() {
        let a = edge_arrays.endpoints[2 * i];
        let b = edge_arrays.endpoints[2 * i + 1];
        prop_assert_local(node_set.contains(&a), "edge endpoint missing in node set");
        prop_assert_local(node_set.contains(&b), "edge endpoint missing in node set");
        prop_assert_local(a != b, "edge connects identical nodes");
    }

    // Half-edge pairing in planarized output
    let plan = planarize_graph(g);
    let mut counts: HashMap<(usize, usize), i32> = HashMap::new();
    for i in 0..plan.half_from.len() {
        let key = (plan.half_from[i], plan.half_to[i]);
        *counts.entry(key).or_insert(0) += 1;
    }
    for i in 0..plan.half_from.len() {
        let u = plan.half_from[i];
        let v = plan.half_to[i];
        let rev = counts.get(&(v, u)).copied().unwrap_or(0);
        prop_assert_local(rev > 0, "missing reverse half-edge");
    }

    // Faces close: no tiny degenerate areas in returned regions
    let regions = g.get_regions();
    for region in &regions {
        if let Some(area) = region
            .get("area")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
        {
            if area.abs() > 0.0 {
                prop_assert_local(area.abs() >= EPS_FACE_AREA, "degenerate face area");
            }
        }
    }

    // Determinism: a second region pass on an unmutated graph yields the same key multiset
    let ver_before = g.geom_version();
    let regions2 = g.get_regions();
    prop_assert_local(
        g.geom_version() == ver_before,
        "get_regions mutated geom_version",
    );
    let keys1: Vec<u64> = regions
        .iter()
        .filter_map(|r| r.get("key").and_then(|v| v.as_u64()))
        .collect();
    let keys2: Vec<u64> = regions2
        .iter()
        .filter_map(|r| r.get("key").and_then(|v| v.as_u64()))
        .collect();
    let mut k1 = keys1.clone();
    let mut k2 = keys2.clone();
    k1.sort_unstable();
    k2.sort_unstable();
    prop_assert_local(k1 == k2, "region keys non-deterministic across calls");

    // JSON round-trip: counts preserved when reloading into a fresh graph
    let json = g.to_json_value();
    let mut g2 = Graph::new();
    let loaded = g2.from_json_value(json);
    prop_assert_local(loaded, "from_json_value rejected own to_json output");
    prop_assert_local(
        g2.node_count() == g.node_count(),
        "node_count drift after json round-trip",
    );
    prop_assert_local(
        g2.edge_count() == g.edge_count(),
        "edge_count drift after json round-trip",
    );
}

// proptest macros expect TestCaseError-returning calls; assert_invariants is shared
// helper code, so use a local thin wrapper that panics with context.
fn prop_assert_local(cond: bool, msg: &'static str) {
    assert!(cond, "{}", msg);
}

fn sequence_strategy() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(op_strategy(), 5..30)
}

proptest! {
    // Default to 1024 cases (~30s locally). Override via env for the v1 acceptance
    // run: `PROPTEST_CASES=10000 cargo test -p contour --test property_graph`.
    #![proptest_config(ProptestConfig { cases: 1024, .. ProptestConfig::default() })]
    #[test]
    fn graph_edit_invariants(seq in sequence_strategy()) {
        let mut graph = Graph::new();
        let mut state = ModelState::default();
        let mut prev_ver = graph.geom_version();
        for op in seq {
            sync_state(&graph, &mut state);
            apply_op(&mut graph, &state, op);
            prop_assert!(graph.geom_version() >= prev_ver, "geom_version went backwards mid-sequence");
            prev_ver = graph.geom_version();
        }
        assert_invariants(&mut graph, 0);
    }
}

// Focused strict-API check: invalid inputs must not mutate state or bump geom_version.
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, .. ProptestConfig::default() })]
    #[test]
    fn strict_api_rejects_without_mutating(
        x in -1000i16..1000i16,
        y in -1000i16..1000i16,
    ) {
        let mut g = Graph::new();
        let a = g.add_node(x as f32 * 0.1, y as f32 * 0.1);
        let ver = g.geom_version();
        let n_before = g.node_count();
        let e_before = g.edge_count();

        // Self-edge is invalid; legacy returns None, no mutation.
        let res = g.add_edge(a, a);
        prop_assert!(res.is_none(), "add_edge(a, a) should return None");
        prop_assert_eq!(g.geom_version(), ver, "geom_version changed on rejected self-edge");
        prop_assert_eq!(g.node_count(), n_before, "node_count changed on rejected self-edge");
        prop_assert_eq!(g.edge_count(), e_before, "edge_count changed on rejected self-edge");

        // move_node on unknown id is a no-op.
        let bogus = u32::MAX - 1;
        let moved = g.move_node(bogus, 1.0, 1.0);
        prop_assert!(!moved, "move_node on bogus id should return false");
        prop_assert_eq!(g.geom_version(), ver, "geom_version changed on bogus move_node");

        // remove_edge on unknown id is a no-op.
        let removed = g.remove_edge(bogus);
        prop_assert!(!removed, "remove_edge on bogus id should return false");
        prop_assert_eq!(g.geom_version(), ver, "geom_version changed on bogus remove_edge");
    }
}
