use std::collections::HashMap;
use crate::model::{Vec2, EdgeKind};
use crate::geometry::flatten::flatten_cubic;

use crate::Graph;

#[derive(Clone)]
pub(crate) struct Region { pub key: u32, pub points: Vec<Vec2>, pub area: f32 }

impl Graph {
    pub(crate) fn compute_regions(&self) -> Vec<Region> {
        #[derive(Clone, Copy, Debug)]
        struct Pt { x: f32, y: f32 }
        // Flattened segments tagged with originating edge id
        let mut segs: Vec<(Pt, Pt, u32)> = Vec::new();
        for (eid, e) in self.edges.iter().enumerate() {
            if let Some(edge) = e {
                let a = match self.nodes.get(edge.a as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                let b = match self.nodes.get(edge.b as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                match edge.kind {
                    EdgeKind::Line => {
                        segs.push((Pt { x: a.x, y: a.y }, Pt { x: b.x, y: b.y }, eid as u32));
                    }
                    EdgeKind::Cubic { ha, hb, .. } => {
                        let p1x = a.x + ha.x; let p1y = a.y + ha.y;
                        let p2x = b.x + hb.x; let p2y = b.y + hb.y;
                        // flatten with configured tolerance
                        let mut pts: Vec<Vec2> = Vec::new();
                        pts.push(Vec2 { x: a.x, y: a.y });
                        flatten_cubic(&mut pts, a.x, a.y, p1x, p1y, p2x, p2y, b.x, b.y, self.flatten_tol, 0);
                        for w in pts.windows(2) {
                            let p = Pt { x: w[0].x, y: w[0].y };
                            let q = Pt { x: w[1].x, y: w[1].y };
                            segs.push((p, q, eid as u32));
                        }
                    }
                    EdgeKind::Polyline { ref points } => {
                        let mut prev = Pt { x: a.x, y: a.y };
                        for pt in points {
                            let next = Pt { x: pt.x, y: pt.y };
                            segs.push((prev, next, eid as u32));
                            prev = next;
                        }
                        let endp = Pt { x: b.x, y: b.y };
                        segs.push((prev, endp, eid as u32));
                    }
                }
            }
        }

        // quantize and build vertices
        let scale: f32 = 10.0; // 0.1 px grid
        let mut vid_map: HashMap<(i32, i32), usize> = HashMap::new();
        let mut verts: Vec<Pt> = Vec::new();
        let mut half_from: Vec<usize> = Vec::new();
        let mut half_to: Vec<usize> = Vec::new();
        let mut half_eid: Vec<u32> = Vec::new();
        for (p, q, eid) in segs {
            let qx = |v: f32| (v*scale).round() as i32;
            let k1 = (qx(p.x), qx(p.y));
            let k2 = (qx(q.x), qx(q.y));
            let u = *vid_map.entry(k1).or_insert_with(|| { let id = verts.len(); verts.push(p); id });
            let v = *vid_map.entry(k2).or_insert_with(|| { let id = verts.len(); verts.push(q); id });
            if u == v { continue; }
            half_from.push(u); half_to.push(v); half_eid.push(eid);
            half_from.push(v); half_to.push(u); half_eid.push(eid);
        }

        let m = half_from.len();
        let mut adj: Vec<Vec<(usize, f32)>> = vec![Vec::new(); verts.len()];
        for i in 0..m {
            let u = half_from[i]; let v = half_to[i];
            let a = (verts[v].y - verts[u].y).atan2(verts[v].x - verts[u].x);
            adj[u].push((v, a));
        }
        for lst in &mut adj { lst.sort_by(|x, y| x.1.partial_cmp(&y.1).unwrap()); }

        // map for halfedge lookup
        let mut idx_map: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
        for i in 0..m { idx_map.entry((half_from[i], half_to[i])).or_default().push(i); }

        let mut used: Vec<bool> = vec![false; m];
        let mut regions: Vec<Region> = Vec::new();
        for i_start in 0..m {
            if used[i_start] { continue; }
            let mut i_he = i_start;
            let mut cycle: Vec<usize> = Vec::new();
            let mut cycle_eids: Vec<u32> = Vec::new();
            let mut guard = 0;
            loop {
                used[i_he] = true;
                let v = half_to[i_he];
                let u = half_from[i_he];
                cycle.push(u);
                cycle_eids.push(half_eid[i_he]);
                // find next half-edge: turn CCW at vertex v
                let lst = &adj[v];
                if lst.is_empty() { break; }
                // find reverse half-edge index of current edge (v->u)
                let mut rev_idx = None;
                if let Some(cands) = idx_map.get(&(v, u)) {
                    for &c in cands { if half_from[c] == v && half_to[c] == u { rev_idx = Some(c); break; } }
                }
                let rev_i = if let Some(ix) = rev_idx { ix } else { break };
                // In adj list at v, find (u) angle and take previous (CCW next)
                let ang = (verts[u].y - verts[v].y).atan2(verts[u].x - verts[v].x);
                let mut k = 0usize; while k < lst.len() && (lst[k].1 - ang).abs() > f32::EPSILON { k += 1; }
                if k == 0 { k = lst.len(); }
                let (w, _angw) = lst[k-1];
                // find half-edge index (v->w)
                if let Some(list) = idx_map.get(&(v, w)) {
                    let mut found = None;
                    for &cand in list { if !used[cand] { found = Some(cand); break; } }
                    if let Some(nhe) = found { i_he = nhe; } else { break; }
                } else { break; }
                guard += 1; if guard > 100000 { break; }
                if i_he == i_start { break; }
            }
            if cycle.len() >= 3 {
                let mut poly: Vec<Vec2> = Vec::new();
                for &idx in &cycle { poly.push(Vec2 { x: verts[idx].x, y: verts[idx].y }); }
                let area = polygon_area(&poly);
                if area.abs() < 1e-2 { continue; }
                // Build canonical edge id sequence (compress consecutive duplicates)
                let mut seq: Vec<u32> = Vec::new();
                for &e in &cycle_eids { if seq.last().copied() != Some(e) { seq.push(e); } }
                if seq.len() >= 2 && seq.first() == seq.last() { seq.pop(); }
                let key = region_key_from_edges(&seq);
                regions.push(Region { key, points: poly, area });
            }
        }
        if regions.is_empty() {
            let fallback = self.find_simple_cycles();
            if !fallback.is_empty() { return fallback; }
        }
        regions
    }
}

impl Graph {
    pub(crate) fn find_simple_cycles(&self) -> Vec<Region> {
        // Build adjacency of node ids
        let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();
        for e in self.edges.iter() {
            if let Some(edge) = e {
                if self.nodes.get(edge.a as usize).and_then(|n| *n).is_none() { continue; }
                if self.nodes.get(edge.b as usize).and_then(|n| *n).is_none() { continue; }
                adj.entry(edge.a).or_default().push(edge.b);
                adj.entry(edge.b).or_default().push(edge.a);
            }
        }
        // Filter candidate nodes with degree 2
        let mut visited: HashMap<u32, bool> = HashMap::new();
        let mut regions: Vec<Region> = Vec::new();
        for (&start, neigh) in adj.iter() {
            if neigh.len() != 2 { continue; }
            if visited.get(&start).copied().unwrap_or(false) { continue; }
            // Walk cycle
            let mut cycle_ids: Vec<u32> = Vec::new();
            let mut prev: u32 = start;
            let mut cur: u32 = start;
            let mut guard = 0;
            loop {
                cycle_ids.push(cur);
                visited.insert(cur, true);
                // choose next neighbor different from prev
                let ns = adj.get(&cur).cloned().unwrap_or_default();
                let mut found_next = None;
                for n in ns {
                    if n != prev { found_next = Some(n); break; }
                }
                if let Some(nxt) = found_next { prev = cur; cur = nxt; } else { break; }
                guard += 1; if guard > 10000 { break; }
                if cur == start { break; }
            }
            if cycle_ids.len() >= 3 && cur == start {
                let mut poly: Vec<Vec2> = Vec::new();
                let mut edge_seq: Vec<u32> = Vec::new();
                for i in 0..cycle_ids.len() {
                    let u = cycle_ids[i];
                    let v = cycle_ids[(i + 1) % cycle_ids.len()];
                    let nu = match self.nodes.get(u as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                    let nv = match self.nodes.get(v as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                    // find an edge between u and v
                    let mut added_any = false;
                    for (eid_idx, e) in self.edges.iter().enumerate() {
                        if let Some(e) = e {
                            if (e.a == u && e.b == v) || (e.a == v && e.b == u) {
                                match &e.kind {
                                    EdgeKind::Line => {
                                        if poly.is_empty() { poly.push(Vec2 { x: nu.x, y: nu.y }); }
                                        poly.push(Vec2 { x: nv.x, y: nv.y });
                                    }
                                    EdgeKind::Cubic { ha, hb, .. } => {
                                        let (ax, ay, bx, by, p1x, p1y, p2x, p2y) = if e.a == u {
                                            (nu.x, nu.y, nv.x, nv.y, nu.x + ha.x, nu.y + ha.y, nv.x + hb.x, nv.y + hb.y)
                                        } else {
                                            (nv.x, nv.y, nu.x, nu.y, nv.x + hb.x, nv.y + hb.y, nu.x + ha.x, nu.y + ha.y)
                                        };
                                        if poly.is_empty() { poly.push(Vec2 { x: ax, y: ay }); }
                                        let mut pts: Vec<Vec2> = Vec::new();
                                        flatten_cubic(&mut pts, ax, ay, p1x, p1y, p2x, p2y, bx, by, self.flatten_tol, 0);
                                        for p in pts { poly.push(p); }
                                    }
                                    EdgeKind::Polyline { points } => {
                                        if poly.is_empty() { poly.push(Vec2 { x: nu.x, y: nu.y }); }
                                        if e.a == u {
                                            for p in points.iter() { poly.push(Vec2 { x: p.x, y: p.y }); }
                                            poly.push(Vec2 { x: nv.x, y: nv.y });
                                        } else {
                                            poly.push(Vec2 { x: nv.x, y: nv.y });
                                            for p in points.iter().rev() { poly.push(Vec2 { x: p.x, y: p.y }); }
                                        }
                                    }
                                }
                                added_any = true;
                                if edge_seq.last().copied() != Some(eid_idx as u32) { edge_seq.push(eid_idx as u32); }
                                break;
                            }
                        }
                    }
                    if !added_any {
                        if poly.is_empty() { poly.push(Vec2 { x: nu.x, y: nu.y }); }
                        poly.push(Vec2 { x: nv.x, y: nv.y });
                    }
                }
                let area = polygon_area(&poly).abs();
                if area > 1e-2 {
                    if edge_seq.len() >= 2 && edge_seq.first() == edge_seq.last() { edge_seq.pop(); }
                    let key = region_key_from_edges(&edge_seq);
                    regions.push(Region { key, points: poly, area });
                }
            }
        }
        regions
    }
}

pub(crate) fn polygon_area(poly: &Vec<Vec2>) -> f32 {
    let mut a = 0.0;
    for i in 0..poly.len() {
        let j = (i + 1) % poly.len();
        a += poly[i].x * poly[j].y - poly[j].x * poly[i].y;
    }
    0.5 * a
}

pub(crate) fn polygon_centroid(poly: &Vec<Vec2>) -> (f32, f32) {
    let mut cx = 0.0; let mut cy = 0.0; let mut a = 0.0;
    for i in 0..poly.len() {
        let j = (i + 1) % poly.len();
        let cross = poly[i].x * poly[j].y - poly[j].x * poly[i].y;
        a += cross;
        cx += (poly[i].x + poly[j].x) * cross;
        cy += (poly[i].y + poly[j].y) * cross;
    }
    let a = a * 0.5;
    if a.abs() < 1e-6 { return (poly[0].x, poly[0].y); }
    (cx / (6.0 * a), cy / (6.0 * a))
}

fn region_key_from_edges(seq: &Vec<u32>) -> u32 {
    if seq.is_empty() { return 0; }
    // Consider both directions
    let mut rev = seq.clone(); rev.reverse();
    fn min_rot_u32(seq: &Vec<u32>) -> Vec<u32> {
        let n = seq.len();
        let mut best: Option<Vec<u32>> = None;
        for s in 0..n {
            let mut rot = Vec::with_capacity(n);
            for k in 0..n { rot.push(seq[(s+k)%n]); }
            if best.as_ref().map_or(true, |b| rot < *b) { best = Some(rot); }
        }
        best.unwrap()
    }
    let fwd = min_rot_u32(seq);
    let bwd = min_rot_u32(&rev);
    let canon = if fwd <= bwd { fwd } else { bwd };
    // FNV-1a hash over u32 ids
    let mut hash: u32 = 0x811C9DC5;
    for x in canon {
        for b in x.to_le_bytes() { hash ^= b as u32; hash = hash.wrapping_mul(0x01000193); }
    }
    hash
}

