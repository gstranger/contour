use crate::algorithms::planarize::Planarized;
use crate::geometry::flatten::flatten_cubic;
use crate::geometry::intersect::{intersect_segments, SegIntersection};
use crate::geometry::tolerance::{EPS_DENOM, EPS_POS, QUANT_SCALE};
use crate::{model::EdgeKind, Graph};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy)]
struct Seg {
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
    eid: u32,
}

fn seg_point(s: &Seg, t: f64) -> (f32, f32) {
    let x = (s.ax as f64) + ((s.bx as f64) - (s.ax as f64)) * t;
    let y = (s.ay as f64) + ((s.by as f64) - (s.ay as f64)) * t;
    (x as f32, y as f32)
}

fn aabb_intersects(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    let (ax0, ay0, ax1, ay1) = a;
    let (bx0, by0, bx1, by1) = b;
    !(ax1 < bx0 || bx1 < ax0 || ay1 < by0 || by1 < ay0)
}

pub fn planarize_subset_with_bbox(
    g: &Graph,
    edges: &[u32],
    clip: Option<(f32, f32, f32, f32)>,
) -> Planarized {
    // 1) Flatten only the selected edges
    let mut segs: Vec<Seg> = Vec::new();
    for &eid in edges {
        if let Some(e) = g.edges.get(eid as usize).and_then(|e| e.as_ref()) {
            let a = if let Some(n) = g.nodes.get(e.a as usize).and_then(|n| *n) {
                n
            } else {
                continue;
            };
            let b = if let Some(n) = g.nodes.get(e.b as usize).and_then(|n| *n) {
                n
            } else {
                continue;
            };
            // Use flattened cache if available for all kinds
            if let Some(fc) = g.flatten_cache.borrow().as_ref() {
                if let Some(pts) = fc.per_edge.get(&eid) {
                    for w in pts.windows(2) {
                        let seg_aabb = (
                            w[0].x.min(w[1].x),
                            w[0].y.min(w[1].y),
                            w[0].x.max(w[1].x),
                            w[0].y.max(w[1].y),
                        );
                        if clip.map_or(true, |c| aabb_intersects(seg_aabb, c)) {
                            segs.push(Seg {
                                ax: w[0].x,
                                ay: w[0].y,
                                bx: w[1].x,
                                by: w[1].y,
                                eid,
                            });
                        }
                    }
                    continue;
                }
            }
            match &e.kind {
                EdgeKind::Line => {
                    let seg_aabb = (a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
                    if clip.map_or(true, |c| aabb_intersects(seg_aabb, c)) {
                        segs.push(Seg {
                            ax: a.x,
                            ay: a.y,
                            bx: b.x,
                            by: b.y,
                            eid,
                        });
                    }
                }
                EdgeKind::Cubic { ha, hb, .. } => {
                    let p1x = a.x + ha.x;
                    let p1y = a.y + ha.y;
                    let p2x = b.x + hb.x;
                    let p2y = b.y + hb.y;
                    let edge_aabb = (
                        a.x.min(p1x).min(p2x).min(b.x),
                        a.y.min(p1y).min(p2y).min(b.y),
                        a.x.max(p1x).max(p2x).max(b.x),
                        a.y.max(p1y).max(p2y).max(b.y),
                    );
                    if let Some(c) = clip {
                        if !aabb_intersects(edge_aabb, c) {
                            continue;
                        }
                    }
                    let mut pts = Vec::new();
                    pts.push(crate::model::Vec2 { x: a.x, y: a.y });
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
                    for w in pts.windows(2) {
                        let seg_aabb = (
                            w[0].x.min(w[1].x),
                            w[0].y.min(w[1].y),
                            w[0].x.max(w[1].x),
                            w[0].y.max(w[1].y),
                        );
                        if clip.map_or(true, |c| aabb_intersects(seg_aabb, c)) {
                            segs.push(Seg {
                                ax: w[0].x,
                                ay: w[0].y,
                                bx: w[1].x,
                                by: w[1].y,
                                eid,
                            });
                        }
                    }
                }
                EdgeKind::Polyline { points } => {
                    let mut prevx = a.x;
                    let mut prevy = a.y;
                    for p in points {
                        let seg_aabb = (
                            prevx.min(p.x),
                            prevy.min(p.y),
                            prevx.max(p.x),
                            prevy.max(p.y),
                        );
                        if clip.map_or(true, |c| aabb_intersects(seg_aabb, c)) {
                            segs.push(Seg {
                                ax: prevx,
                                ay: prevy,
                                bx: p.x,
                                by: p.y,
                                eid,
                            });
                        }
                        prevx = p.x;
                        prevy = p.y;
                    }
                    let seg_aabb = (
                        prevx.min(b.x),
                        prevy.min(b.y),
                        prevx.max(b.x),
                        prevy.max(b.y),
                    );
                    if clip.map_or(true, |c| aabb_intersects(seg_aabb, c)) {
                        segs.push(Seg {
                            ax: prevx,
                            ay: prevy,
                            bx: b.x,
                            by: b.y,
                            eid,
                        });
                    }
                }
            }
        }
    }

    // 2) Intersections with uniform grid acceleration + orientation bucketing
    let n = segs.len();
    let mut splits: Vec<Vec<f64>> = vec![vec![0.0f64, 1.0f64]; n];
    // Orientation: 0 = horiz-ish, 1 = vert-ish
    let mut orient: Vec<u8> = Vec::with_capacity(n);
    for s in &segs {
        let dx = (s.bx - s.ax).abs();
        let dy = (s.by - s.ay).abs();
        orient.push(if dx >= dy { 0 } else { 1 });
    }
    let ep = EPS_POS;
    let ed = EPS_DENOM;

    // Grid cell size heuristic tuned for subset
    let cell = (g.flatten_tol * 1.5).max(0.4);
    let cell_ix = |x: f32| -> i32 { (x / cell).floor() as i32 };
    let mut buckets: HashMap<(i32, i32), Vec<usize>> = HashMap::with_capacity(segs.len() * 2 + 16);
    for (i, s) in segs.iter().enumerate() {
        let minx = s.ax.min(s.bx);
        let maxx = s.ax.max(s.bx);
        let miny = s.ay.min(s.by);
        let maxy = s.ay.max(s.by);
        let ix0 = cell_ix(minx - ep);
        let ix1 = cell_ix(maxx + ep);
        let iy0 = cell_ix(miny - ep);
        let iy1 = cell_ix(maxy + ep);
        for ix in ix0..=ix1 {
            for iy in iy0..=iy1 {
                buckets.entry((ix, iy)).or_default().push(i);
            }
        }
    }

    let mut tested: HashSet<(usize, usize)> = HashSet::new();
    for (_key, list) in buckets.into_iter() {
        if list.len() < 2 {
            continue;
        }
        // Split by orientation for pair pruning
        let mut horiz: Vec<usize> = Vec::new();
        let mut vert: Vec<usize> = Vec::new();
        for &idx in &list {
            if orient[idx] == 0 {
                horiz.push(idx);
            } else {
                vert.push(idx);
            }
        }
        // Cross pairs: horiz vs vert
        for &i in &horiz {
            for &j in &vert {
                let (lo, hi) = if i < j { (i, j) } else { (j, i) };
                if !tested.insert((lo, hi)) {
                    continue;
                }
                let (ax, ay, bx, by) = (segs[i].ax, segs[i].ay, segs[i].bx, segs[i].by);
                let (cx, cy, dx, dy) = (segs[j].ax, segs[j].ay, segs[j].bx, segs[j].by);
                let minx1 = ax.min(bx);
                let maxx1 = ax.max(bx);
                let miny1 = ay.min(by);
                let maxy1 = ay.max(by);
                let minx2 = cx.min(dx);
                let maxx2 = cx.max(dx);
                let miny2 = cy.min(dy);
                let maxy2 = cy.max(dy);
                if maxx1 < minx2 - ep
                    || maxx2 < minx1 - ep
                    || maxy1 < miny2 - ep
                    || maxy2 < miny1 - ep
                {
                    continue;
                }
                match intersect_segments(ax, ay, bx, by, cx, cy, dx, dy, ep, ed) {
                    SegIntersection::None => {}
                    SegIntersection::Proper { t, u, .. } | SegIntersection::Touch { t, u, .. } => {
                        if t > (ep as f64) && t < 1.0 - (ep as f64) {
                            splits[i].push(t);
                        }
                        if u > (ep as f64) && u < 1.0 - (ep as f64) {
                            splits[j].push(u);
                        }
                    }
                    SegIntersection::CollinearOverlap { t0, t1, u0, u1 } => {
                        for &t in &[t0, t1] {
                            if t > (ep as f64) && t < 1.0 - (ep as f64) {
                                splits[i].push(t);
                            }
                        }
                        for &u in &[u0, u1] {
                            if u > (ep as f64) && u < 1.0 - (ep as f64) {
                                splits[j].push(u);
                            }
                        }
                    }
                }
            }
        }
        // Same-orientation pairs: only when near-collinear and ranges overlap strongly
        // Horizontal-ish
        for a in 0..horiz.len() {
            let i = horiz[a];
            for b in (a + 1)..horiz.len() {
                let j = horiz[b];
                let (lo, hi) = if i < j { (i, j) } else { (j, i) };
                if !tested.insert((lo, hi)) {
                    continue;
                }
                let (ax, ay, bx, by) = (segs[i].ax, segs[i].ay, segs[i].bx, segs[i].by);
                let (cx, cy, dx, dy) = (segs[j].ax, segs[j].ay, segs[j].bx, segs[j].by);
                // y proximity and x-range overlap
                let ymid1 = (ay + by) * 0.5;
                let ymid2 = (cy + dy) * 0.5;
                if (ymid1 - ymid2).abs() > ep {
                    continue;
                }
                let minx1 = ax.min(bx);
                let maxx1 = ax.max(bx);
                let minx2 = cx.min(dx);
                let maxx2 = cx.max(dx);
                if maxx1 < minx2 - ep || maxx2 < minx1 - ep {
                    continue;
                }
                match intersect_segments(ax, ay, bx, by, cx, cy, dx, dy, ep, ed) {
                    _ => {}
                }
            }
        }
        // Vertical-ish
        for a in 0..vert.len() {
            let i = vert[a];
            for b in (a + 1)..vert.len() {
                let j = vert[b];
                let (lo, hi) = if i < j { (i, j) } else { (j, i) };
                if !tested.insert((lo, hi)) {
                    continue;
                }
                let (ax, ay, bx, by) = (segs[i].ax, segs[i].ay, segs[i].bx, segs[i].by);
                let (cx, cy, dx, dy) = (segs[j].ax, segs[j].ay, segs[j].bx, segs[j].by);
                // x proximity and y-range overlap
                let xmid1 = (ax + bx) * 0.5;
                let xmid2 = (cx + dx) * 0.5;
                if (xmid1 - xmid2).abs() > ep {
                    continue;
                }
                let miny1 = ay.min(by);
                let maxy1 = ay.max(by);
                let miny2 = cy.min(dy);
                let maxy2 = cy.max(dy);
                if maxy1 < miny2 - ep || maxy2 < miny1 - ep {
                    continue;
                }
                match intersect_segments(ax, ay, bx, by, cx, cy, dx, dy, ep, ed) {
                    _ => {}
                }
            }
        }
    }

    // 3) Quantization and vertex creation
    let scale = QUANT_SCALE;
    let mut key_to_vid: HashMap<(i32, i32), usize> = HashMap::new();
    let mut verts: Vec<(f32, f32)> = Vec::new();
    let mut accum: HashMap<usize, (f64, f64, u32)> = HashMap::new();

    let mut half_from: Vec<usize> = Vec::new();
    let mut half_to: Vec<usize> = Vec::new();
    let mut half_eid: Vec<u32> = Vec::new();

    let mut get_vid = |x: f32, y: f32| -> usize {
        let kx = (x * scale).round() as i32;
        let ky = (y * scale).round() as i32;
        if let Some(&vid) = key_to_vid.get(&(kx, ky)) {
            let entry = accum.entry(vid).or_insert((0.0, 0.0, 0));
            entry.0 += x as f64;
            entry.1 += y as f64;
            entry.2 += 1;
            return vid;
        }
        let vid = verts.len();
        key_to_vid.insert((kx, ky), vid);
        verts.push((x, y));
        accum.insert(vid, (x as f64, y as f64, 1));
        vid
    };

    for (idx, s) in segs.iter().enumerate() {
        let mut ts = splits[idx].clone();
        ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        for w in ts.windows(2) {
            let t0 = w[0];
            let t1 = w[1];
            if t1 - t0 <= 1e-12 {
                continue;
            }
            let (x0, y0) = seg_point(s, t0);
            let (x1, y1) = seg_point(s, t1);
            let dx = x1 - x0;
            let dy = y1 - y0;
            if dx * dx + dy * dy <= EPS_POS * EPS_POS {
                continue;
            }
            let u = get_vid(x0, y0);
            let v = get_vid(x1, y1);
            if u == v {
                continue;
            }
            half_from.push(u);
            half_to.push(v);
            half_eid.push(s.eid);
            half_from.push(v);
            half_to.push(u);
            half_eid.push(s.eid);
        }
    }

    for (vid, (sx, sy, cnt)) in accum.into_iter() {
        if cnt > 0 {
            verts[vid] = ((sx / (cnt as f64)) as f32, (sy / (cnt as f64)) as f32);
        }
    }

    Planarized {
        verts,
        half_from,
        half_to,
        half_eid,
    }
}

pub fn planarize_subset(g: &Graph, edges: &[u32]) -> Planarized {
    planarize_subset_with_bbox(g, edges, None)
}

/// Like `planarize_subset_with_bbox`, but returns None early when the estimated
/// bucket pair count or segment count exceed provided limits. This prevents
/// pathological O(k^2) intersection explosions during incremental updates.
pub fn planarize_subset_with_bbox_guard(
    g: &Graph,
    edges: &[u32],
    clip: Option<(f32, f32, f32, f32)>,
    pairs_limit: usize,
    seg_limit: usize,
) -> Option<Planarized> {
    // 1) Flatten only the selected edges
    let mut segs: Vec<Seg> = Vec::new();
    for &eid in edges {
        if let Some(e) = g.edges.get(eid as usize).and_then(|e| e.as_ref()) {
            let a = if let Some(n) = g.nodes.get(e.a as usize).and_then(|n| *n) {
                n
            } else {
                continue;
            };
            let b = if let Some(n) = g.nodes.get(e.b as usize).and_then(|n| *n) {
                n
            } else {
                continue;
            };
            if let Some(fc) = g.flatten_cache.borrow().as_ref() {
                if let Some(pts) = fc.per_edge.get(&eid) {
                    for w in pts.windows(2) {
                        let seg_aabb = (
                            w[0].x.min(w[1].x),
                            w[0].y.min(w[1].y),
                            w[0].x.max(w[1].x),
                            w[0].y.max(w[1].y),
                        );
                        if clip.map_or(true, |c| aabb_intersects(seg_aabb, c)) {
                            segs.push(Seg {
                                ax: w[0].x,
                                ay: w[0].y,
                                bx: w[1].x,
                                by: w[1].y,
                                eid,
                            });
                        }
                    }
                    continue;
                }
            }
            match &e.kind {
                EdgeKind::Line => {
                    let seg_aabb = (a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
                    if clip.map_or(true, |c| aabb_intersects(seg_aabb, c)) {
                        segs.push(Seg {
                            ax: a.x,
                            ay: a.y,
                            bx: b.x,
                            by: b.y,
                            eid,
                        });
                    }
                }
                EdgeKind::Cubic { ha, hb, .. } => {
                    let p1x = a.x + ha.x;
                    let p1y = a.y + ha.y;
                    let p2x = b.x + hb.x;
                    let p2y = b.y + hb.y;
                    let edge_aabb = (
                        a.x.min(p1x).min(p2x).min(b.x),
                        a.y.min(p1y).min(p2y).min(b.y),
                        a.x.max(p1x).max(p2x).max(b.x),
                        a.y.max(p1y).max(p2y).max(b.y),
                    );
                    if let Some(c) = clip {
                        if !aabb_intersects(edge_aabb, c) {
                            continue;
                        }
                    }
                    let mut pts = Vec::new();
                    pts.push(crate::model::Vec2 { x: a.x, y: a.y });
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
                    for w in pts.windows(2) {
                        let seg_aabb = (
                            w[0].x.min(w[1].x),
                            w[0].y.min(w[1].y),
                            w[0].x.max(w[1].x),
                            w[0].y.max(w[1].y),
                        );
                        if clip.map_or(true, |c| aabb_intersects(seg_aabb, c)) {
                            segs.push(Seg {
                                ax: w[0].x,
                                ay: w[0].y,
                                bx: w[1].x,
                                by: w[1].y,
                                eid,
                            });
                        }
                    }
                }
                EdgeKind::Polyline { points } => {
                    let mut prevx = a.x;
                    let mut prevy = a.y;
                    for p in points {
                        let seg_aabb = (
                            prevx.min(p.x),
                            prevy.min(p.y),
                            prevx.max(p.x),
                            prevy.max(p.y),
                        );
                        if clip.map_or(true, |c| aabb_intersects(seg_aabb, c)) {
                            segs.push(Seg {
                                ax: prevx,
                                ay: prevy,
                                bx: p.x,
                                by: p.y,
                                eid,
                            });
                        }
                        prevx = p.x;
                        prevy = p.y;
                    }
                    let seg_aabb = (
                        prevx.min(b.x),
                        prevy.min(b.y),
                        prevx.max(b.x),
                        prevy.max(b.y),
                    );
                    if clip.map_or(true, |c| aabb_intersects(seg_aabb, c)) {
                        segs.push(Seg {
                            ax: prevx,
                            ay: prevy,
                            bx: b.x,
                            by: b.y,
                            eid,
                        });
                    }
                }
            }
        }
        if segs.len() > seg_limit {
            return None;
        }
    }
    if segs.len() > seg_limit {
        return None;
    }

    // 2) Intersections with uniform grid and budget guard
    let n = segs.len();
    let mut splits: Vec<Vec<f64>> = vec![vec![0.0f64, 1.0f64]; n];
    let ep = EPS_POS;
    let ed = EPS_DENOM;
    let cell = (g.flatten_tol * 1.5).max(0.4);
    let cell_ix = |x: f32| -> i32 { (x / cell).floor() as i32 };
    let mut buckets: HashMap<(i32, i32), Vec<usize>> = HashMap::with_capacity(segs.len() * 2 + 16);
    for (i, s) in segs.iter().enumerate() {
        let minx = s.ax.min(s.bx);
        let maxx = s.ax.max(s.bx);
        let miny = s.ay.min(s.by);
        let maxy = s.ay.max(s.by);
        let ix0 = cell_ix(minx - ep);
        let ix1 = cell_ix(maxx + ep);
        let iy0 = cell_ix(miny - ep);
        let iy1 = cell_ix(maxy + ep);
        for ix in ix0..=ix1 {
            for iy in iy0..=iy1 {
                buckets.entry((ix, iy)).or_default().push(i);
            }
        }
    }
    // Estimate pair budget; bail early if too high (overestimates are fine)
    let mut est_pairs: usize = 0;
    for (_k, list) in buckets.iter() {
        let m = list.len();
        if m >= 2 {
            // m choose 2
            est_pairs = est_pairs.saturating_add(m.saturating_sub(1) * m / 2);
            if est_pairs > pairs_limit {
                return None;
            }
        }
    }

    let mut tested: HashSet<(usize, usize)> = HashSet::new();
    for (_key, list) in buckets.into_iter() {
        if list.len() < 2 {
            continue;
        }
        for a in 0..list.len() {
            let i = list[a];
            for b in (a + 1)..list.len() {
                let j = list[b];
                let (lo, hi) = if i < j { (i, j) } else { (j, i) };
                if !tested.insert((lo, hi)) {
                    continue;
                }
                let (ax, ay, bx, by) = (segs[i].ax, segs[i].ay, segs[i].bx, segs[i].by);
                let (cx, cy, dx, dy) = (segs[j].ax, segs[j].ay, segs[j].bx, segs[j].by);
                let minx1 = ax.min(bx);
                let maxx1 = ax.max(bx);
                let miny1 = ay.min(by);
                let maxy1 = ay.max(by);
                let minx2 = cx.min(dx);
                let maxx2 = cx.max(dx);
                let miny2 = cy.min(dy);
                let maxy2 = cy.max(dy);
                if maxx1 < minx2 - ep
                    || maxx2 < minx1 - ep
                    || maxy1 < miny2 - ep
                    || maxy2 < miny1 - ep
                {
                    continue;
                }
                match intersect_segments(ax, ay, bx, by, cx, cy, dx, dy, ep, ed) {
                    SegIntersection::None => {}
                    SegIntersection::Proper { t, u, .. } | SegIntersection::Touch { t, u, .. } => {
                        if t > (ep as f64) && t < 1.0 - (ep as f64) {
                            splits[i].push(t);
                        }
                        if u > (ep as f64) && u < 1.0 - (ep as f64) {
                            splits[j].push(u);
                        }
                    }
                    SegIntersection::CollinearOverlap { t0, t1, u0, u1 } => {
                        for &t in &[t0, t1] {
                            if t > (ep as f64) && t < 1.0 - (ep as f64) {
                                splits[i].push(t);
                            }
                        }
                        for &u in &[u0, u1] {
                            if u > (ep as f64) && u < 1.0 - (ep as f64) {
                                splits[j].push(u);
                            }
                        }
                    }
                }
            }
        }
    }

    // 3) Quantization and vertex creation
    let scale = QUANT_SCALE;
    let mut key_to_vid: HashMap<(i32, i32), usize> = HashMap::new();
    let mut verts: Vec<(f32, f32)> = Vec::new();
    let mut accum: HashMap<usize, (f64, f64, u32)> = HashMap::new();
    let mut half_from: Vec<usize> = Vec::new();
    let mut half_to: Vec<usize> = Vec::new();
    let mut half_eid: Vec<u32> = Vec::new();
    let mut get_vid = |x: f32, y: f32| -> usize {
        let kx = (x * scale).round() as i32;
        let ky = (y * scale).round() as i32;
        if let Some(&vid) = key_to_vid.get(&(kx, ky)) {
            let entry = accum.entry(vid).or_insert((0.0, 0.0, 0));
            entry.0 += x as f64;
            entry.1 += y as f64;
            entry.2 += 1;
            return vid;
        }
        let vid = verts.len();
        key_to_vid.insert((kx, ky), vid);
        verts.push((x, y));
        accum.insert(vid, (x as f64, y as f64, 1));
        vid
    };
    for (idx, s) in segs.iter().enumerate() {
        let mut ts = splits[idx].clone();
        ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        for w in ts.windows(2) {
            let t0 = w[0];
            let t1 = w[1];
            if t1 - t0 <= 1e-12 {
                continue;
            }
            let (x0, y0) = seg_point(s, t0);
            let (x1, y1) = seg_point(s, t1);
            let dx = x1 - x0;
            let dy = y1 - y0;
            if dx * dx + dy * dy <= EPS_POS * EPS_POS {
                continue;
            }
            let u = get_vid(x0, y0);
            let v = get_vid(x1, y1);
            if u == v {
                continue;
            }
            half_from.push(u);
            half_to.push(v);
            half_eid.push(s.eid);
            half_from.push(v);
            half_to.push(u);
            half_eid.push(s.eid);
        }
    }
    for (vid, (sx, sy, cnt)) in accum.into_iter() {
        if cnt > 0 {
            verts[vid] = ((sx / (cnt as f64)) as f32, (sy / (cnt as f64)) as f32);
        }
    }
    Some(Planarized {
        verts,
        half_from,
        half_to,
        half_eid,
    })
}

/// Planarize a subset with pair pruning: only compute intersections for pairs where
/// at least one segment belongs to a "primary" edge. This is intended for incremental
/// updates where `primary_edges` are the changed edges, and `all_edges` is `primary ∪ neighbors`.
pub fn planarize_subset_pruned(
    g: &Graph,
    primary_edges: &[u32],
    all_edges: &[u32],
    clip: Option<(f32, f32, f32, f32)>,
) -> Planarized {
    use std::collections::HashSet;
    let prim_set: HashSet<u32> = primary_edges.iter().copied().collect();
    // 1) Flatten edges for `all_edges`; mark per-segment whether it is primary
    let mut segs: Vec<Seg> = Vec::new();
    let mut is_primary: Vec<bool> = Vec::new();
    for &eid in all_edges {
        if let Some(e) = g.edges.get(eid as usize).and_then(|e| e.as_ref()) {
            let a = if let Some(n) = g.nodes.get(e.a as usize).and_then(|n| *n) {
                n
            } else {
                continue;
            };
            let b = if let Some(n) = g.nodes.get(e.b as usize).and_then(|n| *n) {
                n
            } else {
                continue;
            };
            let prim = prim_set.contains(&eid);
            // Try flattened cache
            if let Some(fc) = g.flatten_cache.borrow().as_ref() {
                if let Some(pts) = fc.per_edge.get(&eid) {
                    for w in pts.windows(2) {
                        let seg_aabb = (
                            w[0].x.min(w[1].x),
                            w[0].y.min(w[1].y),
                            w[0].x.max(w[1].x),
                            w[0].y.max(w[1].y),
                        );
                        if clip.map_or(true, |c| aabb_intersects(seg_aabb, c)) {
                            segs.push(Seg {
                                ax: w[0].x,
                                ay: w[0].y,
                                bx: w[1].x,
                                by: w[1].y,
                                eid,
                            });
                            is_primary.push(prim);
                        }
                    }
                    continue;
                }
            }
            match &e.kind {
                EdgeKind::Line => {
                    let seg_aabb = (a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
                    if clip.map_or(true, |c| aabb_intersects(seg_aabb, c)) {
                        segs.push(Seg {
                            ax: a.x,
                            ay: a.y,
                            bx: b.x,
                            by: b.y,
                            eid,
                        });
                        is_primary.push(prim);
                    }
                }
                EdgeKind::Cubic { ha, hb, .. } => {
                    let p1x = a.x + ha.x;
                    let p1y = a.y + ha.y;
                    let p2x = b.x + hb.x;
                    let p2y = b.y + hb.y;
                    let edge_aabb = (
                        a.x.min(p1x).min(p2x).min(b.x),
                        a.y.min(p1y).min(p2y).min(b.y),
                        a.x.max(p1x).max(p2x).max(b.x),
                        a.y.max(p1y).max(p2y).max(b.y),
                    );
                    if let Some(c) = clip {
                        if !aabb_intersects(edge_aabb, c) {
                            continue;
                        }
                    }
                    let mut pts = Vec::new();
                    pts.push(crate::model::Vec2 { x: a.x, y: a.y });
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
                    for w in pts.windows(2) {
                        let seg_aabb = (
                            w[0].x.min(w[1].x),
                            w[0].y.min(w[1].y),
                            w[0].x.max(w[1].x),
                            w[0].y.max(w[1].y),
                        );
                        if clip.map_or(true, |c| aabb_intersects(seg_aabb, c)) {
                            segs.push(Seg {
                                ax: w[0].x,
                                ay: w[0].y,
                                bx: w[1].x,
                                by: w[1].y,
                                eid,
                            });
                            is_primary.push(prim);
                        }
                    }
                }
                EdgeKind::Polyline { points } => {
                    let mut prevx = a.x;
                    let mut prevy = a.y;
                    for p in points {
                        let seg_aabb = (
                            prevx.min(p.x),
                            prevy.min(p.y),
                            prevx.max(p.x),
                            prevy.max(p.y),
                        );
                        if clip.map_or(true, |c| aabb_intersects(seg_aabb, c)) {
                            segs.push(Seg {
                                ax: prevx,
                                ay: prevy,
                                bx: p.x,
                                by: p.y,
                                eid,
                            });
                            is_primary.push(prim);
                        }
                        prevx = p.x;
                        prevy = p.y;
                    }
                    let seg_aabb = (
                        prevx.min(b.x),
                        prevy.min(b.y),
                        prevx.max(b.x),
                        prevy.max(b.y),
                    );
                    if clip.map_or(true, |c| aabb_intersects(seg_aabb, c)) {
                        segs.push(Seg {
                            ax: prevx,
                            ay: prevy,
                            bx: b.x,
                            by: b.y,
                            eid,
                        });
                        is_primary.push(prim);
                    }
                }
            }
        }
    }

    // 2) Intersections with grid; pairs only if at least one segment is primary
    let n = segs.len();
    let mut splits: Vec<Vec<f64>> = vec![vec![0.0f64, 1.0f64]; n];
    let ep = EPS_POS;
    let ed = EPS_DENOM;
    let cell = (g.flatten_tol * 1.5).max(0.4);
    let cell_ix = |x: f32| -> i32 { (x / cell).floor() as i32 };
    let mut buckets: HashMap<(i32, i32), Vec<usize>> = HashMap::with_capacity(segs.len() * 2 + 16);
    for (i, s) in segs.iter().enumerate() {
        let minx = s.ax.min(s.bx);
        let maxx = s.ax.max(s.bx);
        let miny = s.ay.min(s.by);
        let maxy = s.ay.max(s.by);
        let ix0 = cell_ix(minx - ep);
        let ix1 = cell_ix(maxx + ep);
        let iy0 = cell_ix(miny - ep);
        let iy1 = cell_ix(maxy + ep);
        for ix in ix0..=ix1 {
            for iy in iy0..=iy1 {
                buckets.entry((ix, iy)).or_default().push(i);
            }
        }
    }
    let mut tested: HashSet<(usize, usize)> = HashSet::new();
    for (_key, list) in buckets.into_iter() {
        if list.len() < 2 {
            continue;
        }
        for a in 0..list.len() {
            let i = list[a];
            for b in (a + 1)..list.len() {
                let j = list[b];
                if !is_primary[i] && !is_primary[j] {
                    continue;
                } // prune neighbor-neighbor only
                let (lo, hi) = if i < j { (i, j) } else { (j, i) };
                if !tested.insert((lo, hi)) {
                    continue;
                }
                let (ax, ay, bx, by) = (segs[i].ax, segs[i].ay, segs[i].bx, segs[i].by);
                let (cx, cy, dx, dy) = (segs[j].ax, segs[j].ay, segs[j].bx, segs[j].by);
                let minx1 = ax.min(bx);
                let maxx1 = ax.max(bx);
                let miny1 = ay.min(by);
                let maxy1 = ay.max(by);
                let minx2 = cx.min(dx);
                let maxx2 = cx.max(dx);
                let miny2 = cy.min(dy);
                let maxy2 = cy.max(dy);
                if maxx1 < minx2 - ep
                    || maxx2 < minx1 - ep
                    || maxy1 < miny2 - ep
                    || maxy2 < miny1 - ep
                {
                    continue;
                }
                match intersect_segments(ax, ay, bx, by, cx, cy, dx, dy, ep, ed) {
                    SegIntersection::None => {}
                    SegIntersection::Proper { t, u, .. } | SegIntersection::Touch { t, u, .. } => {
                        if t > (ep as f64) && t < 1.0 - (ep as f64) {
                            splits[i].push(t);
                        }
                        if u > (ep as f64) && u < 1.0 - (ep as f64) {
                            splits[j].push(u);
                        }
                    }
                    SegIntersection::CollinearOverlap { t0, t1, u0, u1 } => {
                        for &t in &[t0, t1] {
                            if t > (ep as f64) && t < 1.0 - (ep as f64) {
                                splits[i].push(t);
                            }
                        }
                        for &u in &[u0, u1] {
                            if u > (ep as f64) && u < 1.0 - (ep as f64) {
                                splits[j].push(u);
                            }
                        }
                    }
                }
            }
        }
    }

    // 3) Quantization and vertex creation
    let scale = QUANT_SCALE;
    let mut key_to_vid: HashMap<(i32, i32), usize> = HashMap::new();
    let mut verts: Vec<(f32, f32)> = Vec::new();
    let mut accum: HashMap<usize, (f64, f64, u32)> = HashMap::new();
    let mut half_from: Vec<usize> = Vec::new();
    let mut half_to: Vec<usize> = Vec::new();
    let mut half_eid: Vec<u32> = Vec::new();
    let mut get_vid = |x: f32, y: f32| -> usize {
        let kx = (x * scale).round() as i32;
        let ky = (y * scale).round() as i32;
        if let Some(&vid) = key_to_vid.get(&(kx, ky)) {
            let entry = accum.entry(vid).or_insert((0.0, 0.0, 0));
            entry.0 += x as f64;
            entry.1 += y as f64;
            entry.2 += 1;
            return vid;
        }
        let vid = verts.len();
        key_to_vid.insert((kx, ky), vid);
        verts.push((x, y));
        accum.insert(vid, (x as f64, y as f64, 1));
        vid
    };

    for (idx, s) in segs.iter().enumerate() {
        let mut ts = splits[idx].clone();
        ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        for w in ts.windows(2) {
            let t0 = w[0];
            let t1 = w[1];
            if t1 - t0 <= 1e-12 {
                continue;
            }
            let (x0, y0) = seg_point(s, t0);
            let (x1, y1) = seg_point(s, t1);
            let dx = x1 - x0;
            let dy = y1 - y0;
            if dx * dx + dy * dy <= EPS_POS * EPS_POS {
                continue;
            }
            let u = get_vid(x0, y0);
            let v = get_vid(x1, y1);
            if u == v {
                continue;
            }
            half_from.push(u);
            half_to.push(v);
            half_eid.push(s.eid);
            half_from.push(v);
            half_to.push(u);
            half_eid.push(s.eid);
        }
    }
    for (vid, (sx, sy, cnt)) in accum.into_iter() {
        if cnt > 0 {
            verts[vid] = ((sx / (cnt as f64)) as f32, (sy / (cnt as f64)) as f32);
        }
    }
    Planarized {
        verts,
        half_from,
        half_to,
        half_eid,
    }
}
