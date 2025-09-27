use contour::algorithms::planarize::planarize_graph;
use contour::geometry::tolerance::EPS_FACE_AREA;
use contour::Graph;
use proptest::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Debug)]
enum Op {
    AddNode { x: i16, y: i16 },
    MoveNode { idx: u16, dx: i8, dy: i8 },
    RemoveNode { idx: u16 },
    AddEdge { a: u16, b: u16 },
    RemoveEdge { idx: u16 },
    BendEdge { idx: u16, t_num: u8, tx: i8, ty: i8 },
    SetHandleMode { idx: u16, mode: u8 },
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
        (any::<u16>(), any::<u8>(), any::<i8>(), any::<i8>()).prop_map(
            |(idx, t_num, tx, ty)| Op::BendEdge {
                idx,
                t_num,
                tx,
                ty,
            },
        ),
        (any::<u16>(), (0u8..=2u8)).prop_map(|(idx, mode)| Op::SetHandleMode { idx, mode }),
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

fn assert_invariants(g: &mut Graph) {
    // No dangling references
    let edge_arrays = g.get_edge_arrays();
    for i in 0..edge_arrays.ids.len() {
        let a = edge_arrays.endpoints[2 * i];
        let b = edge_arrays.endpoints[2 * i + 1];
        assert!(g.get_node(a).is_some(), "edge {} missing node {}", i, a);
        assert!(g.get_node(b).is_some(), "edge {} missing node {}", i, b);
        assert_ne!(a, b, "edge {} connects identical nodes", i);
    }

    // Half-edge pairing
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
        assert!(rev > 0, "missing reverse half-edge for {} -> {}", u, v);
    }

    // Faces close (non-degenerate regions)
    let regions = g.get_regions();
    for region in regions {
        if let Some(area) = region.get("area").and_then(|v| v.as_f64()).map(|v| v as f32) {
            if area.abs() > 0.0 {
                assert!(
                    area.abs() >= EPS_FACE_AREA,
                    "degenerate face area {}", area
                );
            }
        }
    }
}

fn sequence_strategy() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(op_strategy(), 5..30)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 10_000, .. ProptestConfig::default() })]
    #[test]
    fn graph_edit_invariants(seq in sequence_strategy()) {
        let mut graph = Graph::new();
        let mut state = ModelState::default();
        for op in seq {
            sync_state(&graph, &mut state);
            apply_op(&mut graph, &state, op);
        }
        assert_invariants(&mut graph);
    }
}
