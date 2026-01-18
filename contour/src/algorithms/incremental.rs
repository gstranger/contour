use crate::geometry::flatten::flatten_cubic;
use crate::{
    model::{EdgeKind, Vec2},
    Graph,
};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct IncrPlan {
    pub cell: f32,
    pub seg_cells: HashMap<(i32, i32), Vec<(u32, usize)>>, // (cell) -> list of (edge_id, seg_index)
    pub edge_segments: HashMap<u32, Vec<(f32, f32, f32, f32)>>, // edge_id -> list of segments (ax,ay,bx,by)
}

fn cell_ix(cell: f32, x: f32) -> i32 {
    (x / cell).floor() as i32
}

/// Maximum cells a segment can span in one dimension before we skip grid insertion.
/// This prevents memory explosion from segments with extreme coordinate ranges.
const MAX_CELL_SPAN: i32 = 256;

fn choose_cell_size(flatten_tol: f32) -> f32 {
    (flatten_tol * 8.0).clamp(4.0, 64.0)
}

fn flatten_points_for_edge(g: &Graph, eid: u32) -> Option<Vec<Vec2>> {
    // Prefer cached flattened data if available
    if let Some(fc) = g.flatten_cache.borrow().as_ref() {
        if let Some(pts) = fc.per_edge.get(&eid) {
            return Some(pts.clone());
        }
    }
    // Compute on the fly
    let e = g.edges.get(eid as usize).and_then(|x| x.as_ref())?;
    let a = g.nodes.get(e.a as usize).and_then(|n| *n)?;
    let b = g.nodes.get(e.b as usize).and_then(|n| *n)?;
    match &e.kind {
        EdgeKind::Line => Some(vec![Vec2 { x: a.x, y: a.y }, Vec2 { x: b.x, y: b.y }]),
        EdgeKind::Cubic { ha, hb, .. } => {
            let p1x = a.x + ha.x;
            let p1y = a.y + ha.y;
            let p2x = b.x + hb.x;
            let p2y = b.y + hb.y;
            let mut pts = Vec::new();
            pts.push(Vec2 { x: a.x, y: a.y });
            flatten_cubic(
                &mut pts,
                a.x,
                a.y,
                p1x,
                p1y,
                p2x,
                p2y,
                b.x,
                b.y,
                g.flatten_tol,
                0,
            );
            Some(pts)
        }
        EdgeKind::Polyline { points } => {
            let mut out = Vec::with_capacity(points.len() + 2);
            out.push(Vec2 { x: a.x, y: a.y });
            for p in points {
                out.push(*p);
            }
            out.push(Vec2 { x: b.x, y: b.y });
            Some(out)
        }
    }
}

pub fn build_from_graph(g: &Graph) -> IncrPlan {
    let cell = choose_cell_size(g.flatten_tol);
    let mut seg_cells: HashMap<(i32, i32), Vec<(u32, usize)>> = HashMap::new();
    let mut edge_segments: HashMap<u32, Vec<(f32, f32, f32, f32)>> = HashMap::new();
    for (eid, e_opt) in g.edges.iter().enumerate() {
        if e_opt.is_none() {
            continue;
        }
        let eid = eid as u32;
        if let Some(pts) = flatten_points_for_edge(g, eid) {
            let mut segs: Vec<(f32, f32, f32, f32)> = Vec::new();
            for (idx, w) in pts.windows(2).enumerate() {
                let (ax, ay, bx, by) = (w[0].x, w[0].y, w[1].x, w[1].y);
                segs.push((ax, ay, bx, by));
                let minx = ax.min(bx);
                let maxx = ax.max(bx);
                let miny = ay.min(by);
                let maxy = ay.max(by);
                let ix0 = cell_ix(cell, minx);
                let ix1 = cell_ix(cell, maxx);
                let iy0 = cell_ix(cell, miny);
                let iy1 = cell_ix(cell, maxy);
                // Skip grid insertion for segments spanning too many cells
                if (ix1 - ix0) > MAX_CELL_SPAN || (iy1 - iy0) > MAX_CELL_SPAN {
                    continue;
                }
                for ix in ix0..=ix1 {
                    for iy in iy0..=iy1 {
                        seg_cells.entry((ix, iy)).or_default().push((eid, idx));
                    }
                }
            }
            edge_segments.insert(eid, segs);
        }
    }
    IncrPlan {
        cell,
        seg_cells,
        edge_segments,
    }
}

pub fn update_for_dirty(g: &Graph, plan: &mut IncrPlan, edge_ids: &[u32]) {
    // Remove old segments of dirty edges from cells
    for &eid in edge_ids {
        if let Some(segs) = plan.edge_segments.remove(&eid) {
            for (idx, (ax, ay, bx, by)) in segs.into_iter().enumerate() {
                let minx = ax.min(bx);
                let maxx = ax.max(bx);
                let miny = ay.min(by);
                let maxy = ay.max(by);
                let ix0 = cell_ix(plan.cell, minx);
                let ix1 = cell_ix(plan.cell, maxx);
                let iy0 = cell_ix(plan.cell, miny);
                let iy1 = cell_ix(plan.cell, maxy);
                for ix in ix0..=ix1 {
                    for iy in iy0..=iy1 {
                        if let Some(v) = plan.seg_cells.get_mut(&(ix, iy)) {
                            v.retain(|&(e, j)| !(e == eid && j == idx));
                        }
                    }
                }
            }
        }
    }
    // Add new segments for dirty edges
    for &eid in edge_ids {
        if let Some(pts) = flatten_points_for_edge(g, eid) {
            let mut segs: Vec<(f32, f32, f32, f32)> = Vec::new();
            for (idx, w) in pts.windows(2).enumerate() {
                let (ax, ay, bx, by) = (w[0].x, w[0].y, w[1].x, w[1].y);
                segs.push((ax, ay, bx, by));
                let minx = ax.min(bx);
                let maxx = ax.max(bx);
                let miny = ay.min(by);
                let maxy = ay.max(by);
                let ix0 = cell_ix(plan.cell, minx);
                let ix1 = cell_ix(plan.cell, maxx);
                let iy0 = cell_ix(plan.cell, miny);
                let iy1 = cell_ix(plan.cell, maxy);
                // Skip grid insertion for segments spanning too many cells
                if (ix1 - ix0) > MAX_CELL_SPAN || (iy1 - iy0) > MAX_CELL_SPAN {
                    continue;
                }
                for ix in ix0..=ix1 {
                    for iy in iy0..=iy1 {
                        plan.seg_cells.entry((ix, iy)).or_default().push((eid, idx));
                    }
                }
            }
            plan.edge_segments.insert(eid, segs);
        }
    }
}

/// Ensure that the incremental planarization cache exists and is up to date.
/// Rebuilds the entire cache if:
/// - No cache exists yet, or
/// - The chosen cell size changed (e.g., flatten tolerance changed), or
/// - A full rebuild was requested via `g.dirty.full`.
/// Otherwise, only updates entries for the union of added/removed/modified edges.
pub fn ensure_incr_plan(g: &mut Graph) {
    // Keep flatten cache current; incremental plan prefers to reuse it.
    crate::algorithms::regions::ensure_flatten_cache(g);

    let desired_cell = choose_cell_size(g.flatten_tol);
    let mut guard = g.incr_plan.borrow_mut();
    let need_rebuild = guard
        .as_ref()
        .map_or(true, |p| (p.cell - desired_cell).abs() > f32::EPSILON)
        || g.dirty.full;

    if need_rebuild {
        *guard = Some(build_from_graph(g));
        return;
    }

    // Incremental update for dirty edges
    if let Some(plan) = guard.as_mut() {
        let mut dirty_edges: Vec<u32> = Vec::new();
        dirty_edges.extend(g.dirty.edges_removed.iter().copied());
        dirty_edges.extend(g.dirty.edges_added.iter().copied());
        dirty_edges.extend(g.dirty.edges_modified.iter().copied());
        if !dirty_edges.is_empty() {
            dirty_edges.sort_unstable();
            dirty_edges.dedup();
            update_for_dirty(g, plan, &dirty_edges);
        }
    }
}

/// Given a small set of changed edges, return a superset of edges that are
/// spatial neighbors by consulting the segment-to-cell index from the plan.
/// This is intended to bound planarization to a small local neighborhood.
pub fn neighbor_edges_for_edges(g: &Graph, plan: &IncrPlan, edges: &[u32]) -> Vec<u32> {
    fn push_cells_for_seg(
        plan: &IncrPlan,
        out: &mut std::collections::HashSet<u32>,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
    ) {
        let cell = plan.cell;
        let minx = ax.min(bx);
        let maxx = ax.max(bx);
        let miny = ay.min(by);
        let maxy = ay.max(by);
        let ix0 = cell_ix(cell, minx);
        let ix1 = cell_ix(cell, maxx);
        let iy0 = cell_ix(cell, miny);
        let iy1 = cell_ix(cell, maxy);
        for ix in ix0..=ix1 {
            for iy in iy0..=iy1 {
                if let Some(list) = plan.seg_cells.get(&(ix, iy)) {
                    for &(eid, _seg_idx) in list {
                        out.insert(eid);
                    }
                }
            }
        }
    }
    let mut out: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for &eid in edges {
        if let Some(segs) = plan.edge_segments.get(&eid) {
            for &(ax, ay, bx, by) in segs {
                push_cells_for_seg(plan, &mut out, ax, ay, bx, by);
            }
        } else if let Some(pts) = flatten_points_for_edge(g, eid) {
            for w in pts.windows(2) {
                push_cells_for_seg(plan, &mut out, w[0].x, w[0].y, w[1].x, w[1].y);
            }
        }
        out.insert(eid);
    }
    let mut v: Vec<u32> = out.into_iter().collect();
    v.sort_unstable();
    v
}
