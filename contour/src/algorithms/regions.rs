use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::{
    algorithms::{
        incremental::{ensure_incr_plan, neighbor_edges_for_edges},
        planarize::planarize_graph,
        planarize::Planarized,
        planarize_subset::planarize_subset_with_bbox_guard,
    },
    geometry::{
        flatten::flatten_cubic,
        tolerance::{EPS_ANG, EPS_FACE_AREA, QUANT_SCALE},
    },
    model::{EdgeKind, FillState, Vec2},
    Graph, RegionFaceCache,
};

#[cfg(feature = "region_prof")]
use std::time::Instant;

#[derive(Clone)]
pub(crate) struct Region {
    pub key: u32,
    pub points: Vec<Vec2>,
    pub area: f32,
    pub edges: Vec<u32>,
}

fn polygon_area(poly: &[Vec2]) -> f32 {
    let mut a = 0.0f32;
    for i in 0..poly.len() {
        let j = (i + 1) % poly.len();
        a += poly[i].x * poly[j].y - poly[j].x * poly[i].y;
    }
    0.5 * a
}

pub(crate) fn polygon_centroid(poly: &[Vec2]) -> (f32, f32) {
    let mut cx = 0.0f32;
    let mut cy = 0.0f32;
    let mut a = 0.0f32;
    for i in 0..poly.len() {
        let j = (i + 1) % poly.len();
        let cross = poly[i].x * poly[j].y - poly[j].x * poly[i].y;
        a += cross;
        cx += (poly[i].x + poly[j].x) * cross;
        cy += (poly[i].y + poly[j].y) * cross;
    }
    let a = a * 0.5;
    if a.abs() < EPS_FACE_AREA {
        return (poly[0].x, poly[0].y);
    }
    (cx / (6.0 * a), cy / (6.0 * a))
}

fn region_key_from_edges(seq: &[u32]) -> u32 {
    if seq.is_empty() {
        return 0;
    }
    let mut rev = seq.to_vec();
    rev.reverse();
    fn min_rot_u32(seq: &[u32]) -> Vec<u32> {
        let n = seq.len();
        let mut best: Option<Vec<u32>> = None;
        for s in 0..n {
            let mut rot = Vec::with_capacity(n);
            for k in 0..n {
                rot.push(seq[(s + k) % n]);
            }
            if best.as_ref().map_or(true, |b| rot < *b) {
                best = Some(rot);
            }
        }
        best.unwrap()
    }
    let fwd = min_rot_u32(seq);
    let bwd = min_rot_u32(&rev);
    let canon = if fwd <= bwd { fwd } else { bwd };
    let mut hash: u32 = 0x811C9DC5;
    for x in canon {
        for b in x.to_le_bytes() {
            hash ^= b as u32;
            hash = hash.wrapping_mul(0x0100_0193);
        }
    }
    hash
}

fn polygon_bbox(points: &[Vec2]) -> (f32, f32, f32, f32) {
    let mut minx = f32::INFINITY;
    let mut miny = f32::INFINITY;
    let mut maxx = f32::NEG_INFINITY;
    let mut maxy = f32::NEG_INFINITY;
    for p in points {
        if p.x < minx {
            minx = p.x;
        }
        if p.x > maxx {
            maxx = p.x;
        }
        if p.y < miny {
            miny = p.y;
        }
        if p.y > maxy {
            maxy = p.y;
        }
    }
    (minx, miny, maxx, maxy)
}

fn bbox_union(
    a: Option<(f32, f32, f32, f32)>,
    b: Option<(f32, f32, f32, f32)>,
) -> Option<(f32, f32, f32, f32)> {
    match (a, b) {
        (None, x) => x,
        (x, None) => x,
        (Some((ax0, ay0, ax1, ay1)), Some((bx0, by0, bx1, by1))) => {
            Some((ax0.min(bx0), ay0.min(by0), ax1.max(bx1), ay1.max(by1)))
        }
    }
}

fn bbox_intersects(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    let (ax0, ay0, ax1, ay1) = a;
    let (bx0, by0, bx1, by1) = b;
    !(ax1 < bx0 || bx1 < ax0 || ay1 < by0 || by1 < ay0)
}

fn bbox_pad(b: (f32, f32, f32, f32), pad: f32) -> (f32, f32, f32, f32) {
    (b.0 - pad, b.1 - pad, b.2 + pad, b.3 + pad)
}

fn region_to_cache_face(region: &Region) -> RegionFaceCache {
    RegionFaceCache {
        key: region.key,
        area: region.area,
        bbox: polygon_bbox(&region.points),
        points: region.points.clone(),
        edges: region.edges.clone(),
    }
}

fn cache_faces_to_regions(faces: &[RegionFaceCache]) -> Vec<Region> {
    faces
        .iter()
        .map(|f| Region {
            key: f.key,
            area: f.area,
            points: f.points.clone(),
            edges: f.edges.clone(),
        })
        .collect()
}

#[derive(Clone, Debug)]
pub(crate) struct FlattenIndex {
    pub tol: f32,
    pub cell: f32,
    pub built_ver: u64,
    pub per_edge_cells: HashMap<u32, Vec<(i32, i32)>>,
    pub cells: HashMap<(i32, i32), Vec<u32>>,
}

#[derive(Clone, Debug)]
pub(crate) struct FlattenCache {
    pub tol: f32,
    pub built_ver: u64,
    pub per_edge: HashMap<u32, Vec<Vec2>>, // flattened polyline including endpoints
}

fn choose_cell_size_for_regions(flatten_tol: f32) -> f32 {
    (flatten_tol * 8.0).clamp(4.0, 64.0)
}

fn flatten_points_for_edge(g: &Graph, eid: u32) -> Option<Vec<Vec2>> {
    let e = g.edges.get(eid as usize).and_then(|e| e.as_ref())?;
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
            out.extend(points.iter().copied());
            out.push(Vec2 { x: b.x, y: b.y });
            Some(out)
        }
    }
}

pub(crate) fn ensure_flatten_cache(g: &mut Graph) {
    let mut guard = g.flatten_cache.borrow_mut();
    let rebuild = guard
        .as_ref()
        .map_or(true, |fc| fc.tol != g.flatten_tol || g.dirty.full);
    if rebuild {
        let mut fc = FlattenCache {
            tol: g.flatten_tol,
            built_ver: g.geom_version(),
            per_edge: HashMap::new(),
        };
        for (eid, e_opt) in g.edges.iter().enumerate() {
            if e_opt.is_none() {
                continue;
            }
            let eid = eid as u32;
            if let Some(poly) = flatten_points_for_edge(g, eid) {
                fc.per_edge.insert(eid, poly);
            }
        }
        *guard = Some(fc);
        return;
    }
    if let Some(fc) = guard.as_mut() {
        for eid in g.dirty.edges_removed.iter() {
            fc.per_edge.remove(eid);
        }
        let mut to_update: Vec<u32> = Vec::new();
        to_update.extend(g.dirty.edges_added.iter().copied());
        to_update.extend(g.dirty.edges_modified.iter().copied());
        to_update.sort_unstable();
        to_update.dedup();
        for eid in to_update {
            if let Some(poly) = flatten_points_for_edge(g, eid) {
                fc.per_edge.insert(eid, poly);
            } else {
                fc.per_edge.remove(&eid);
            }
        }
        fc.built_ver = g.geom_version();
    }
}

fn cell_ix(cell: f32, x: f32) -> i32 {
    (x / cell).floor() as i32
}

fn flatten_edge_cells(g: &Graph, eid: u32, cell: f32) -> Vec<(i32, i32)> {
    let mut cells: Vec<(i32, i32)> = Vec::new();
    let push_seg = |cells: &mut Vec<(i32, i32)>, x0: f32, y0: f32, x1: f32, y1: f32| {
        let minx = x0.min(x1);
        let maxx = x0.max(x1);
        let miny = y0.min(y1);
        let maxy = y0.max(y1);
        let ix0 = cell_ix(cell, minx);
        let ix1 = cell_ix(cell, maxx);
        let iy0 = cell_ix(cell, miny);
        let iy1 = cell_ix(cell, maxy);
        for ix in ix0..=ix1 {
            for iy in iy0..=iy1 {
                cells.push((ix, iy));
            }
        }
    };
    if let Some(fc) = g.flatten_cache.borrow().as_ref() {
        if let Some(pts) = fc.per_edge.get(&eid) {
            for w in pts.windows(2) {
                push_seg(&mut cells, w[0].x, w[0].y, w[1].x, w[1].y);
            }
            cells.sort_unstable();
            cells.dedup();
            return cells;
        }
    }
    if let Some(pts) = flatten_points_for_edge(g, eid) {
        for w in pts.windows(2) {
            push_seg(&mut cells, w[0].x, w[0].y, w[1].x, w[1].y);
        }
    }
    cells.sort_unstable();
    cells.dedup();
    cells
}

fn ensure_flatten_index(g: &mut Graph) {
    let cell = choose_cell_size_for_regions(g.flatten_tol);
    let rebuild = {
        let idx_ref = g.flatten_index.borrow();
        idx_ref.as_ref().map_or(true, |idx| {
            idx.tol != g.flatten_tol || (idx.cell - cell).abs() > f32::EPSILON || g.dirty.full
        })
    };
    if rebuild {
        ensure_flatten_cache(g);
        let mut idx = FlattenIndex {
            tol: g.flatten_tol,
            cell,
            built_ver: g.geom_version(),
            per_edge_cells: HashMap::new(),
            cells: HashMap::new(),
        };
        for (eid, e_opt) in g.edges.iter().enumerate() {
            if e_opt.is_none() {
                continue;
            }
            let eid = eid as u32;
            let cells = flatten_edge_cells(g, eid, idx.cell);
            for c in &cells {
                idx.cells.entry(*c).or_default().push(eid);
            }
            idx.per_edge_cells.insert(eid, cells);
        }
        *g.flatten_index.borrow_mut() = Some(idx);
        return;
    }
    ensure_flatten_cache(g);
    if let Some(idx) = g.flatten_index.borrow_mut().as_mut() {
        for eid in g.dirty.edges_removed.iter() {
            if let Some(prev) = idx.per_edge_cells.remove(eid) {
                for c in prev {
                    if let Some(list) = idx.cells.get_mut(&c) {
                        list.retain(|&x| x != *eid);
                    }
                }
            }
        }
        let mut to_update: Vec<u32> = Vec::new();
        to_update.extend(g.dirty.edges_added.iter().copied());
        to_update.extend(g.dirty.edges_modified.iter().copied());
        to_update.sort_unstable();
        to_update.dedup();
        for eid in to_update {
            if let Some(prev) = idx.per_edge_cells.get(&eid).cloned() {
                for c in prev {
                    if let Some(list) = idx.cells.get_mut(&c) {
                        list.retain(|&x| x != eid);
                    }
                }
            }
            let cells = flatten_edge_cells(g, eid, idx.cell);
            for c in &cells {
                idx.cells.entry(*c).or_default().push(eid);
            }
            idx.per_edge_cells.insert(eid, cells);
        }
        idx.built_ver = g.geom_version();
    }
}

fn regions_from_plan(plan: &Planarized) -> Vec<Region> {
    #[derive(Clone, Copy)]
    struct Pt {
        x: f32,
        y: f32,
    }
    let verts: Vec<Pt> = plan
        .verts
        .iter()
        .map(|(x, y)| Pt { x: *x, y: *y })
        .collect();
    let half_from = &plan.half_from;
    let half_to = &plan.half_to;
    let half_eid = &plan.half_eid;

    let m = half_from.len();
    let mut adj: Vec<Vec<(usize, f32, usize)>> = vec![Vec::new(); verts.len()];
    for i in 0..m {
        let u = half_from[i];
        let v = half_to[i];
        let ang = (verts[v].y - verts[u].y).atan2(verts[v].x - verts[u].x);
        adj[u].push((v, ang, i));
    }
    for lst in &mut adj {
        lst.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap()
                .then(a.0.cmp(&b.0))
                .then(a.2.cmp(&b.2))
        });
    }
    let mut idx_map: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for i in 0..m {
        idx_map
            .entry((half_from[i], half_to[i]))
            .or_default()
            .push(i);
    }
    let mut used = vec![false; m];
    let mut regions = Vec::new();
    for i_start in 0..m {
        if used[i_start] {
            continue;
        }
        let mut i_he = i_start;
        let mut cycle: Vec<usize> = Vec::new();
        let mut cycle_eids: Vec<u32> = Vec::new();
        let mut guard = 0usize;
        loop {
            used[i_he] = true;
            let v = half_to[i_he];
            let u = half_from[i_he];
            cycle.push(u);
            cycle_eids.push(half_eid[i_he]);
            let lst = &adj[v];
            if lst.is_empty() {
                break;
            }
            let rev_idx = idx_map.get(&(v, u)).and_then(|cands| {
                cands
                    .iter()
                    .copied()
                    .find(|&c| half_from[c] == v && half_to[c] == u)
            });
            if rev_idx.is_none() {
                break;
            }
            let ang = (verts[u].y - verts[v].y).atan2(verts[u].x - verts[v].x);
            let mut idx = 0usize;
            while idx < lst.len() && lst[idx].1 <= ang + EPS_ANG {
                idx += 1;
            }
            let next = if idx == lst.len() { 0 } else { idx };
            let (w, _, _) = lst[next];
            if let Some(list) = idx_map.get(&(v, w)) {
                if let Some(nhe) = list.iter().copied().find(|cand| !used[*cand]) {
                    i_he = nhe;
                } else {
                    break;
                }
            } else {
                break;
            }
            guard += 1;
            if guard > 100_000 {
                break;
            }
            if i_he == i_start {
                break;
            }
        }
        if cycle.len() >= 3 {
            let mut poly = Vec::new();
            for &idx in &cycle {
                poly.push(Vec2 {
                    x: verts[idx].x,
                    y: verts[idx].y,
                });
            }
            let area = polygon_area(&poly);
            if area.abs() < EPS_FACE_AREA {
                continue;
            }
            let mut seq = Vec::new();
            for &e in &cycle_eids {
                if seq.last().copied() != Some(e) {
                    seq.push(e);
                }
            }
            if seq.len() >= 2 && seq.first() == seq.last() {
                seq.pop();
            }
            let key = region_key_from_edges(&seq);
            regions.push(Region {
                key,
                points: poly,
                area,
                edges: seq,
            });
        }
    }
    regions
}

fn rebuild_regions_full(g: &mut Graph) -> Vec<Region> {
    let mut regs = g.compute_regions();
    if regs.is_empty() {
        regs = g.find_simple_cycles();
    }
    let faces: Vec<RegionFaceCache> = regs.iter().map(region_to_cache_face).collect();
    g.region_cache.borrow_mut().replace(crate::RegionCache {
        faces,
        built_ver: g.geom_version(),
        tol: g.flatten_tol,
    });
    g.clear_dirty_flags();
    regs
}

fn compute_regions_full(g: &mut Graph) -> Vec<Region> {
    #[cfg(feature = "region_prof")]
    let t_all = std::time::Instant::now();
    #[cfg(feature = "region_prof")]
    let t_cache = std::time::Instant::now();
    ensure_flatten_cache(g);
    crate::algorithms::incremental::ensure_incr_plan(g);
    #[cfg(feature = "region_prof")]
    let cache_ms = t_cache.elapsed().as_secs_f64() * 1000.0;

    #[cfg(feature = "region_prof")]
    let t_plan = std::time::Instant::now();
    let plan = planarize_graph(g);
    #[cfg(feature = "region_prof")]
    let plan_ms = t_plan.elapsed().as_secs_f64() * 1000.0;

    #[cfg(feature = "region_prof")]
    let t_faces = std::time::Instant::now();
    let mut regions = regions_from_plan(&plan);
    if regions.is_empty() {
        regions = g.find_simple_cycles();
    }
    #[cfg(feature = "region_prof")]
    let faces_ms = t_faces.elapsed().as_secs_f64() * 1000.0;
    #[cfg(feature = "region_prof")]
    eprintln!(
        "regions_full cache_ms={:.3} planarize_ms={:.3} faces_ms={:.3} total_ms={:.3}",
        cache_ms,
        plan_ms,
        faces_ms,
        t_all.elapsed().as_secs_f64() * 1000.0
    );
    regions
}

pub(crate) fn compute_regions_incremental(g: &mut Graph) -> Vec<Region> {
    #[cfg(feature = "region_prof")]
    let t_all = Instant::now();

    ensure_flatten_cache(g);
    ensure_flatten_index(g);
    ensure_incr_plan(g);

    let need_full = {
        let cache_guard = g.region_cache.borrow();
        cache_guard.is_none()
            || g.dirty.full
            || cache_guard
                .as_ref()
                .map(|c| (c.tol - g.flatten_tol).abs() > f32::EPSILON)
                .unwrap_or(false)
    };
    if need_full {
        let regs = rebuild_regions_full(g);
        #[cfg(feature = "region_prof")]
        eprintln!(
            "regions_full all_ms={:.3}",
            t_all.elapsed().as_secs_f64() * 1000.0
        );
        return regs;
    }

    let nothing_dirty = g.dirty.bbox.is_none()
        && g.dirty.nodes_added.is_empty()
        && g.dirty.nodes_removed.is_empty()
        && g.dirty.nodes_moved.is_empty()
        && g.dirty.edges_added.is_empty()
        && g.dirty.edges_removed.is_empty()
        && g.dirty.edges_modified.is_empty();

    if nothing_dirty {
        if let Some(cache) = g.region_cache.borrow().as_ref() {
            return cache_faces_to_regions(&cache.faces);
        }
        return rebuild_regions_full(g);
    }

    let mut seed_edges: HashSet<u32> = HashSet::new();
    let mut removed_edges: HashSet<u32> = HashSet::new();

    seed_edges.extend(g.dirty.edges_added.iter().copied());
    seed_edges.extend(g.dirty.edges_modified.iter().copied());
    for &eid in g.dirty.edges_removed.iter() {
        seed_edges.insert(eid);
        removed_edges.insert(eid);
    }

    let mut impacted_nodes: HashSet<u32> = HashSet::new();
    impacted_nodes.extend(g.dirty.nodes_added.iter().copied());
    impacted_nodes.extend(g.dirty.nodes_moved.iter().copied());
    impacted_nodes.extend(g.dirty.nodes_removed.iter().copied());
    if !impacted_nodes.is_empty() {
        for (eid, e_opt) in g.edges.iter().enumerate() {
            if let Some(e) = e_opt {
                if impacted_nodes.contains(&e.a) || impacted_nodes.contains(&e.b) {
                    seed_edges.insert(eid as u32);
                }
            }
        }
    }

    let mut seed_vec: Vec<u32> = seed_edges.iter().copied().collect();
    seed_vec.sort_unstable();

    // ensure removed edges not present in graph are tracked
    for &eid in &seed_vec {
        if !matches!(g.edges.get(eid as usize), Some(Some(_))) {
            removed_edges.insert(eid);
        }
    }

    let mut impact_bbox = g.dirty.bbox;
    for &eid in &seed_vec {
        if let Some(edge) = g.edges.get(eid as usize).and_then(|e| e.as_ref()) {
            if let Some(bb) = g.edge_aabb_of(edge) {
                impact_bbox = bbox_union(impact_bbox, Some(bb));
            }
        }
    }
    let pad = (g.flatten_tol * 2.0).max(0.5);
    let clip_bbox = impact_bbox.map(|bb| bbox_pad(bb, pad));

    let neighbor_edges = {
        let plan_ref = g.incr_plan.borrow();
        if let Some(plan) = plan_ref.as_ref() {
            neighbor_edges_for_edges(g, plan, &seed_vec)
        } else {
            seed_vec.clone()
        }
    };

    let mut candidate_set: HashSet<u32> = seed_vec.iter().copied().collect();
    candidate_set.extend(neighbor_edges.iter().copied());

    if let Some(clip) = clip_bbox {
        ensure_flatten_index(g);
        let idx_guard = g.flatten_index.borrow();
        if let Some(idx) = idx_guard.as_ref() {
            let cell = idx.cell;
            let ix0 = cell_ix(cell, clip.0);
            let ix1 = cell_ix(cell, clip.2);
            let iy0 = cell_ix(cell, clip.1);
            let iy1 = cell_ix(cell, clip.3);
            for ix in ix0..=ix1 {
                for iy in iy0..=iy1 {
                    if let Some(list) = idx.cells.get(&(ix, iy)) {
                        candidate_set.extend(list.iter().copied());
                    }
                }
            }
        }
    }

    let mut candidate_edges: Vec<u32> = candidate_set
        .iter()
        .copied()
        .filter(|eid| matches!(g.edges.get(*eid as usize), Some(Some(_))))
        .collect();
    candidate_edges.sort_unstable();

    let total_edges = g.edge_count().max(1) as usize;
    if candidate_edges.len() * 10 > total_edges * 4 || candidate_edges.len() > 1024 {
        let regs = rebuild_regions_full(g);
        #[cfg(feature = "region_prof")]
        eprintln!(
            "regions_full fallback all_ms={:.3}",
            t_all.elapsed().as_secs_f64() * 1000.0
        );
        return regs;
    }

    let mut new_faces: Vec<Region> = Vec::new();
    if !candidate_edges.is_empty() {
        let plan =
            match planarize_subset_with_bbox_guard(g, &candidate_edges, clip_bbox, 200_000, 50_000)
            {
                Some(plan) => plan,
                None => {
                    let regs = rebuild_regions_full(g);
                    #[cfg(feature = "region_prof")]
                    eprintln!(
                        "regions_full guard all_ms={:.3}",
                        t_all.elapsed().as_secs_f64() * 1000.0
                    );
                    return regs;
                }
            };
        new_faces = regions_from_plan(&plan);
        if new_faces.is_empty() {
            new_faces = g.find_simple_cycles();
        }
    }

    let mut seen_keys: HashSet<u32> = HashSet::new();
    new_faces.retain(|face| seen_keys.insert(face.key));
    if let Some(clip) = clip_bbox {
        new_faces.retain(|face| bbox_intersects(polygon_bbox(&face.points), clip));
    }

    let mut cache_guard = g.region_cache.borrow_mut();
    let cache = cache_guard.as_mut().unwrap();
    let mut removal_edges = candidate_set;
    removal_edges.extend(removed_edges.iter().copied());
    let mut new_face_keys: HashSet<u32> = HashSet::new();
    for face in &new_faces {
        new_face_keys.insert(face.key);
    }
    cache.faces.retain(|face| {
        let edge_hit = face.edges.iter().any(|eid| removal_edges.contains(eid));
        let bbox_hit = clip_bbox.map_or(false, |clip| bbox_intersects(face.bbox, clip));
        let replaced = new_face_keys.contains(&face.key);
        !(edge_hit || bbox_hit || replaced)
    });
    for face in &new_faces {
        cache.faces.push(region_to_cache_face(face));
    }
    cache.built_ver = g.geom_version();
    cache.tol = g.flatten_tol;

    drop(cache_guard);
    g.clear_dirty_flags();

    let result = {
        let cache = g.region_cache.borrow();
        let cache = cache.as_ref().unwrap();
        cache_faces_to_regions(&cache.faces)
    };
    #[cfg(feature = "region_prof")]
    eprintln!(
        "regions_inc cand={} ms={:.3}",
        candidate_edges.len(),
        t_all.elapsed().as_secs_f64() * 1000.0
    );
    result
}

pub fn get_regions_with_fill(g: &mut Graph) -> Vec<serde_json::Value> {
    #[derive(Serialize)]
    struct RegionSer {
        key: u32,
        area: f32,
        filled: bool,
        color: Option<[u8; 4]>,
        points: Vec<f32>,
    }

    let mut regions = g.compute_regions_incremental();
    regions.sort_by(|a, b| a.key.cmp(&b.key));

    if g.last_geom_ver != g.geom_ver {
        let mut new_prev: Vec<(u32, i32, i32, f32)> = Vec::with_capacity(regions.len());
        for r in &regions {
            let (cx, cy) = polygon_centroid(&r.points);
            let qx = (cx * QUANT_SCALE).round() as i32;
            let qy = (cy * QUANT_SCALE).round() as i32;
            new_prev.push((r.key, qx, qy, r.area));
        }
        let mut new_fills = HashMap::new();
        let old_prev = g.prev_regions.clone();
        let mut claimed: HashMap<u32, bool> = HashMap::new();
        let mut order: Vec<usize> = (0..new_prev.len()).collect();
        order.sort_by(|&i, &j| {
            new_prev[i]
                .1
                .cmp(&new_prev[j].1)
                .then(new_prev[i].2.cmp(&new_prev[j].2))
                .then(new_prev[i].3.partial_cmp(&new_prev[j].3).unwrap())
                .then(new_prev[i].0.cmp(&new_prev[j].0))
        });
        for idx in order {
            let (k_new, qx, qy, area_new) = new_prev[idx];
            let mut best: Option<(u32, i64, f32)> = None;
            for (k_old, oqx, oqy, area_old) in &old_prev {
                if claimed.get(k_old).copied().unwrap_or(false) {
                    continue;
                }
                let dx = (qx as i64) - (*oqx as i64);
                let dy = (qy as i64) - (*oqy as i64);
                let d2 = dx * dx + dy * dy;
                let ad = (area_new - *area_old).abs();
                best = match best {
                    None => Some((*k_old, d2, ad)),
                    Some((bk, bd, ba)) => {
                        if d2 < bd {
                            Some((*k_old, d2, ad))
                        } else if d2 == bd && ad < ba {
                            Some((*k_old, d2, ad))
                        } else if d2 == bd && (ad - ba).abs() <= f32::EPSILON && *k_old < bk {
                            Some((*k_old, d2, ad))
                        } else {
                            Some((bk, bd, ba))
                        }
                    }
                };
            }
            let state = if let Some((old_key, _, _)) = best {
                claimed.insert(old_key, true);
                g.fills.get(&old_key).copied().unwrap_or(FillState {
                    filled: true,
                    color: None,
                })
            } else {
                g.fills.get(&k_new).copied().unwrap_or(FillState {
                    filled: true,
                    color: None,
                })
            };
            new_fills.insert(k_new, state);
        }
        g.fills = new_fills;
        g.prev_regions = new_prev;
        g.last_geom_ver = g.geom_ver;
    }

    regions
        .into_iter()
        .map(|r| {
            let st = g.fills.get(&r.key).copied().unwrap_or(FillState {
                filled: true,
                color: None,
            });
            let color = st.color.map(|c| [c.r, c.g, c.b, c.a]);
            let mut pts = Vec::with_capacity(r.points.len() * 2);
            for p in &r.points {
                pts.push(p.x);
                pts.push(p.y);
            }
            serde_json::to_value(RegionSer {
                key: r.key,
                area: r.area,
                filled: st.filled,
                color,
                points: pts,
            })
            .unwrap()
        })
        .collect()
}

impl Graph {
    pub(crate) fn compute_regions(&mut self) -> Vec<Region> {
        compute_regions_full(self)
    }

    pub(crate) fn compute_regions_incremental(&mut self) -> Vec<Region> {
        compute_regions_incremental(self)
    }

    pub(crate) fn find_simple_cycles(&self) -> Vec<Region> {
        let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();
        for e in self.edges.iter() {
            if let Some(e) = e {
                if self.nodes.get(e.a as usize).and_then(|n| *n).is_none() {
                    continue;
                }
                if self.nodes.get(e.b as usize).and_then(|n| *n).is_none() {
                    continue;
                }
                adj.entry(e.a).or_default().push(e.b);
                adj.entry(e.b).or_default().push(e.a);
            }
        }
        let mut visited: HashMap<u32, bool> = HashMap::new();
        let mut regions = Vec::new();
        for (&start, neigh) in adj.iter() {
            if neigh.len() != 2 {
                continue;
            }
            if visited.get(&start).copied().unwrap_or(false) {
                continue;
            }
            let mut cycle_ids = Vec::new();
            let mut prev = start;
            let mut cur = start;
            let mut guard = 0usize;
            loop {
                cycle_ids.push(cur);
                visited.insert(cur, true);
                let ns = adj.get(&cur).cloned().unwrap_or_default();
                let mut found = None;
                for n in ns {
                    if n != prev {
                        found = Some(n);
                        break;
                    }
                }
                if let Some(nxt) = found {
                    prev = cur;
                    cur = nxt;
                } else {
                    break;
                }
                guard += 1;
                if guard > 10_000 {
                    break;
                }
                if cur == start {
                    break;
                }
            }
            if cycle_ids.len() >= 3 && cur == start {
                let mut poly = Vec::new();
                let mut edge_seq = Vec::new();
                for i in 0..cycle_ids.len() {
                    let u = cycle_ids[i];
                    let v = cycle_ids[(i + 1) % cycle_ids.len()];
                    let nu = match self.nodes.get(u as usize).and_then(|n| *n) {
                        Some(n) => n,
                        None => {
                            poly.clear();
                            break;
                        }
                    };
                    let nv = match self.nodes.get(v as usize).and_then(|n| *n) {
                        Some(n) => n,
                        None => {
                            poly.clear();
                            break;
                        }
                    };
                    let mut added = false;
                    for (eid_idx, e) in self.edges.iter().enumerate() {
                        if let Some(e) = e {
                            if (e.a == u && e.b == v) || (e.a == v && e.b == u) {
                                match &e.kind {
                                    EdgeKind::Line => {
                                        if poly.is_empty() {
                                            poly.push(Vec2 { x: nu.x, y: nu.y });
                                        }
                                        poly.push(Vec2 { x: nv.x, y: nv.y });
                                    }
                                    EdgeKind::Cubic { ha, hb, .. } => {
                                        let (ax, ay, bx, by, p1x, p1y, p2x, p2y) = if e.a == u {
                                            (
                                                nu.x,
                                                nu.y,
                                                nv.x,
                                                nv.y,
                                                nu.x + ha.x,
                                                nu.y + ha.y,
                                                nv.x + hb.x,
                                                nv.y + hb.y,
                                            )
                                        } else {
                                            (
                                                nv.x,
                                                nv.y,
                                                nu.x,
                                                nu.y,
                                                nv.x + hb.x,
                                                nv.y + hb.y,
                                                nu.x + ha.x,
                                                nu.y + ha.y,
                                            )
                                        };
                                        if poly.is_empty() {
                                            poly.push(Vec2 { x: ax, y: ay });
                                        }
                                        let mut pts = Vec::new();
                                        flatten_cubic(
                                            &mut pts,
                                            ax,
                                            ay,
                                            p1x,
                                            p1y,
                                            p2x,
                                            p2y,
                                            bx,
                                            by,
                                            self.flatten_tol,
                                            0,
                                        );
                                        for w in pts.into_iter().skip(1) {
                                            poly.push(w);
                                        }
                                    }
                                    EdgeKind::Polyline { points } => {
                                        if poly.is_empty() {
                                            poly.push(Vec2 { x: nu.x, y: nu.y });
                                        }
                                        for p in points {
                                            poly.push(*p);
                                        }
                                        poly.push(Vec2 { x: nv.x, y: nv.y });
                                    }
                                }
                                edge_seq.push(eid_idx as u32);
                                added = true;
                                break;
                            }
                        }
                    }
                    if !added {
                        poly.clear();
                        break;
                    }
                }
                if !poly.is_empty() {
                    let area = polygon_area(&poly);
                    if area.abs() >= EPS_FACE_AREA {
                        let key = region_key_from_edges(&edge_seq);
                        regions.push(Region {
                            key,
                            points: poly,
                            area,
                            edges: edge_seq,
                        });
                    }
                }
            }
        }
        regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: &mut u64) -> f32 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (((*seed >> 24) & 0xFFFF_FFFF) as u32) as f32 / (u32::MAX as f32)
    }

    #[test]
    fn square_face_exists() {
        let mut g = Graph::new();
        let n0 = g.add_node(0.0, 0.0);
        let n1 = g.add_node(10.0, 0.0);
        let n2 = g.add_node(10.0, 10.0);
        let n3 = g.add_node(0.0, 10.0);
        g.add_edge(n0, n1);
        g.add_edge(n1, n2);
        g.add_edge(n2, n3);
        g.add_edge(n3, n0);
        let rs = g.compute_regions();
        assert!(!rs.is_empty());
        let mut found = false;
        for r in rs {
            if r.area.abs() > 90.0 && r.area.abs() < 110.0 {
                found = true;
                break;
            }
        }
        assert!(found, "expected ~100 area face");
    }

    #[test]
    fn incremental_matches_full_after_move() {
        let mut g = Graph::new();
        let n0 = g.add_node(0.0, 0.0);
        let n1 = g.add_node(12.0, 0.0);
        let n2 = g.add_node(12.0, 10.0);
        let n3 = g.add_node(0.0, 10.0);
        g.add_edge(n0, n1);
        g.add_edge(n1, n2);
        g.add_edge(n2, n3);
        g.add_edge(n3, n0);

        let _ = g.compute_regions_incremental();

        assert!(g.move_node(n2, 12.0, 13.0));
        let inc_regions = g.compute_regions_incremental();
        let full_regions = g.compute_regions();

        let mut inc_keys: Vec<u32> = inc_regions.iter().map(|r| r.key).collect();
        inc_keys.sort_unstable();
        let mut full_keys: Vec<u32> = full_regions.iter().map(|r| r.key).collect();
        full_keys.sort_unstable();
        assert_eq!(
            inc_keys, full_keys,
            "incremental keys diverged from full recompute"
        );
    }

    #[test]
    fn self_touch_no_crash() {
        let mut g = Graph::new();
        let a = g.add_node(0.0, 0.0);
        let b = g.add_node(10.0, 0.0);
        let c = g.add_node(10.0, 10.0);
        g.add_edge(a, b);
        g.add_edge(b, c);
        let _ = g.compute_regions();
    }

    #[test]
    fn jitter_stability_on_grid() {
        let mut g = Graph::new();
        let v = 6usize;
        let h = 5usize;
        let x0 = 0.0f32;
        let x1 = 120.0f32;
        let y0 = 0.0f32;
        let y1 = 100.0f32;
        let mut nodes: Vec<u32> = Vec::new();
        for i in 0..v {
            let t = (i as f32 + 1.0) / ((v + 1) as f32);
            let x = x0 + t * (x1 - x0);
            let a = g.add_node(x, y0);
            let b = g.add_node(x, y1);
            nodes.push(a);
            nodes.push(b);
            g.add_edge(a, b);
        }
        for j in 0..h {
            let t = (j as f32 + 1.0) / ((h + 1) as f32);
            let y = y0 + t * (y1 - y0);
            let a = g.add_node(x0, y);
            let b = g.add_node(x1, y);
            nodes.push(a);
            nodes.push(b);
            g.add_edge(a, b);
        }
        let mut keys1: Vec<u32> = g.compute_regions().into_iter().map(|r| r.key).collect();
        keys1.sort_unstable();
        let mut seed = 0xCAFEBABE8BADF00Du64;
        for id in 0..g.nodes.len() as u32 {
            if let Some((x, y)) = g.get_node(id) {
                let jx = (lcg(&mut seed) - 0.5) * 0.06;
                let jy = (lcg(&mut seed) - 0.5) * 0.06;
                g.move_node(id, x + jx, y + jy);
            }
        }
        let mut keys2: Vec<u32> = g
            .compute_regions_incremental()
            .into_iter()
            .map(|r| r.key)
            .collect();
        keys2.sort_unstable();
        assert_eq!(
            keys1, keys2,
            "region keys must be stable under small jitter"
        );
    }
}
