use std::collections::{HashMap, HashSet};
use crate::{Graph, model::EdgeKind};
use crate::geometry::flatten::flatten_cubic;
use crate::geometry::intersect::{intersect_segments, SegIntersection};
use crate::geometry::tolerance::{QUANT_SCALE, EPS_POS, EPS_DENOM};

#[derive(Debug, Clone)]
pub struct Planarized {
    pub verts: Vec<(f32,f32)>,
    pub half_from: Vec<usize>,
    pub half_to: Vec<usize>,
    pub half_eid: Vec<u32>,
}

#[derive(Clone, Copy)]
struct Seg { ax:f32, ay:f32, bx:f32, by:f32, eid:u32 }

fn seg_point(s: &Seg, t: f64) -> (f32,f32) {
    let x = (s.ax as f64) + ((s.bx as f64) - (s.ax as f64)) * t;
    let y = (s.ay as f64) + ((s.by as f64) - (s.ay as f64)) * t;
    (x as f32, y as f32)
}

pub fn planarize_graph(g: &Graph) -> Planarized {
    // 1) Flatten edges into segments
    let mut segs: Vec<Seg> = Vec::new();
    for (eid, e) in g.edges.iter().enumerate() {
        if let Some(e) = e {
            let a = if let Some(n) = g.nodes.get(e.a as usize).and_then(|n| *n) { n } else { continue };
            let b = if let Some(n) = g.nodes.get(e.b as usize).and_then(|n| *n) { n } else { continue };
            match e.kind {
                EdgeKind::Line => {
                    segs.push(Seg{ ax:a.x, ay:a.y, bx:b.x, by:b.y, eid: eid as u32 });
                }
                EdgeKind::Cubic{ha,hb,..} => {
                    let p1x=a.x+ha.x; let p1y=a.y+ha.y; let p2x=b.x+hb.x; let p2y=b.y+hb.y;
                    let mut pts = Vec::new();
                    pts.push(crate::model::Vec2 { x: a.x, y: a.y });
                    flatten_cubic(&mut pts, a.x,a.y, p1x,p1y, p2x,p2y, b.x,b.y, g.flatten_tol, 0);
                    for w in pts.windows(2) {
                        segs.push(Seg{ ax:w[0].x, ay:w[0].y, bx:w[1].x, by:w[1].y, eid: eid as u32 });
                    }
                }
                EdgeKind::Polyline{ ref points } => {
                    let mut prevx = a.x; let mut prevy = a.y;
                    for p in points { segs.push(Seg{ ax:prevx, ay:prevy, bx:p.x, by:p.y, eid: eid as u32 }); prevx=p.x; prevy=p.y; }
                    segs.push(Seg{ ax:prevx, ay:prevy, bx:b.x, by:b.y, eid: eid as u32 });
                }
            }
        }
    }

    // 2) Intersections with uniform grid acceleration
    let n = segs.len();
    let mut splits: Vec<Vec<f64>> = vec![vec![0.0f64, 1.0f64]; n];
    let ep = EPS_POS; let ed = EPS_DENOM;

    // Grid cell size heuristic based on flattening tolerance
    let cell = (g.flatten_tol * 2.0).max(0.5);
    let cell_ix = |x: f32| -> i32 { (x / cell).floor() as i32 };
    let mut buckets: HashMap<(i32,i32), Vec<usize>> = HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        let minx = s.ax.min(s.bx); let maxx = s.ax.max(s.bx);
        let miny = s.ay.min(s.by); let maxy = s.ay.max(s.by);
        let ix0 = cell_ix(minx - ep); let ix1 = cell_ix(maxx + ep);
        let iy0 = cell_ix(miny - ep); let iy1 = cell_ix(maxy + ep);
        for ix in ix0..=ix1 { for iy in iy0..=iy1 { buckets.entry((ix,iy)).or_default().push(i); } }
    }

    let mut tested: HashSet<(usize,usize)> = HashSet::new();
    for (_key, list) in buckets.into_iter() {
        if list.len() < 2 { continue; }
        for a in 0..list.len() {
            let i = list[a];
            for b in (a+1)..list.len() {
                let j = list[b];
                let (lo, hi) = if i < j { (i,j) } else { (j,i) };
                if !tested.insert((lo,hi)) { continue; }

                // Quick bbox reject per pair
                let (ax,ay,bx,by) = (segs[i].ax, segs[i].ay, segs[i].bx, segs[i].by);
                let (cx,cy,dx,dy) = (segs[j].ax, segs[j].ay, segs[j].bx, segs[j].by);
                let minx1 = ax.min(bx); let maxx1 = ax.max(bx); let miny1 = ay.min(by); let maxy1 = ay.max(by);
                let minx2 = cx.min(dx); let maxx2 = cx.max(dx); let miny2 = cy.min(dy); let maxy2 = cy.max(dy);
                if maxx1 < minx2 - ep || maxx2 < minx1 - ep || maxy1 < miny2 - ep || maxy2 < miny1 - ep { continue; }

                match intersect_segments(ax,ay,bx,by,cx,cy,dx,dy, ep, ed) {
                    SegIntersection::None => {}
                    SegIntersection::Proper{t,u,..} => {
                        if t > (ep as f64) && t < 1.0 - (ep as f64) { splits[i].push(t); }
                        if u > (ep as f64) && u < 1.0 - (ep as f64) { splits[j].push(u); }
                    }
                    SegIntersection::Touch{t,u,..} => {
                        if t > (ep as f64) && t < 1.0 - (ep as f64) { splits[i].push(t); }
                        if u > (ep as f64) && u < 1.0 - (ep as f64) { splits[j].push(u); }
                    }
                    SegIntersection::CollinearOverlap{t0,t1,u0,u1} => {
                        for &t in &[t0, t1] { if t > (ep as f64) && t < 1.0 - (ep as f64) { splits[i].push(t); } }
                        for &u in &[u0, u1] { if u > (ep as f64) && u < 1.0 - (ep as f64) { splits[j].push(u); } }
                    }
                }
            }
        }
    }

    // 3) Quantization and vertex creation
    let scale = QUANT_SCALE;
    let mut key_to_vid: HashMap<(i32,i32), usize> = HashMap::new();
    let mut verts: Vec<(f32,f32)> = Vec::new();
    let mut accum: HashMap<usize, (f64,f64,u32)> = HashMap::new();

    let mut half_from: Vec<usize> = Vec::new();
    let mut half_to: Vec<usize> = Vec::new();
    let mut half_eid: Vec<u32> = Vec::new();

    let mut get_vid = |x: f32, y: f32| -> usize {
        let kx = (x * scale).round() as i32;
        let ky = (y * scale).round() as i32;
        if let Some(&vid) = key_to_vid.get(&(kx,ky)) { 
            // accumulate new sample
            let entry = accum.entry(vid).or_insert((0.0,0.0,0));
            entry.0 += x as f64; entry.1 += y as f64; entry.2 += 1;
            return vid;
        }
        let vid = verts.len();
        key_to_vid.insert((kx,ky), vid);
        verts.push((x,y));
        accum.insert(vid, (x as f64, y as f64, 1));
        vid
    };

    for (idx, s) in segs.iter().enumerate() {
        let mut ts = splits[idx].clone();
        ts.sort_by(|a,b| a.partial_cmp(b).unwrap());
        ts.dedup_by(|a,b| (*a - *b).abs() < 1e-12);
        for w in ts.windows(2) {
            let t0 = w[0]; let t1 = w[1];
            if t1 - t0 <= 1e-12 { continue; }
            let (x0,y0) = seg_point(s, t0);
            let (x1,y1) = seg_point(s, t1);
            let dx = x1 - x0; let dy = y1 - y0;
            if dx*dx + dy*dy <= EPS_POS*EPS_POS { continue; }
            let u = get_vid(x0,y0);
            let v = get_vid(x1,y1);
            if u == v { continue; }
            half_from.push(u); half_to.push(v); half_eid.push(s.eid);
            half_from.push(v); half_to.push(u); half_eid.push(s.eid);
        }
    }

    // Average vertex positions per quantization key
    for (vid, (sx,sy,cnt)) in accum.into_iter() {
        if cnt > 0 { verts[vid] = ((sx / (cnt as f64)) as f32, (sy / (cnt as f64)) as f32); }
    }

    Planarized { verts, half_from, half_to, half_eid }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Graph};

    #[test]
    fn planarize_cross() {
        let mut g = Graph::new();
        let a = g.add_node(0.0, 0.0);
        let b = g.add_node(2.0, 2.0);
        let c = g.add_node(0.0, 2.0);
        let d = g.add_node(2.0, 0.0);
        let _e1 = g.add_edge(a, b).unwrap();
        let _e2 = g.add_edge(c, d).unwrap();
        let p = planarize_graph(&g);
        // Two lines crossing become four directed subsegments x 2 = 8 half-edges
        assert!(p.half_from.len() == 8 && p.half_to.len() == 8);
        // Vertices should include intersection plus endpoints (<=5 quantized, but at least 5 unique keys possible)
        assert!(p.verts.len() >= 5);
    }

    #[test]
    fn grid_counts_match() {
        let mut g = Graph::new();
        let v = 12usize; // vertical lines
        let h = 10usize; // horizontal lines
        let x0 = 0.0f32; let x1 = 100.0f32;
        let y0 = 0.0f32; let y1 = 80.0f32;
        // verticals at evenly spaced x, horizontals at evenly spaced y
        for i in 0..v {
            let t = (i as f32 + 1.0) / ((v + 1) as f32);
            let x = x0 + t * (x1 - x0);
            let a = g.add_node(x, y0);
            let b = g.add_node(x, y1);
            g.add_edge(a, b).unwrap();
        }
        for j in 0..h {
            let t = (j as f32 + 1.0) / ((h + 1) as f32);
            let y = y0 + t * (y1 - y0);
            let a = g.add_node(x0, y);
            let b = g.add_node(x1, y);
            g.add_edge(a, b).unwrap();
        }
        let p = planarize_graph(&g);
        let expected_verts = v*h + 2*v + 2*h;
        let expected_undirected = 2*v*h + v + h;
        let expected_half = expected_undirected * 2;
        assert_eq!(p.verts.len(), expected_verts, "vertex count");
        assert_eq!(p.half_from.len(), expected_half, "half-edge count");
        assert_eq!(p.half_to.len(), expected_half, "half-edge count");
    }

    #[test]
    fn random_pairing_and_no_panic() {
        // Deterministic LCG
        fn rng(seed: &mut u64) -> f32 { *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1); (((*seed >> 24) & 0xFFFF_FFFF) as u32) as f32 / (u32::MAX as f32) }
        let mut seed = 0x1234_5678_9ABC_DEF0u64;
        let mut g = Graph::new();
        // Create 150 random segments within a box
        for _ in 0..150 {
            let x1 = 200.0 * rng(&mut seed);
            let y1 = 150.0 * rng(&mut seed);
            let x2 = 200.0 * rng(&mut seed);
            let y2 = 150.0 * rng(&mut seed);
            // Guard degenerate
            if (x1 - x2).abs() + (y1 - y2).abs() < 1e-3 { continue; }
            let a = g.add_node(x1, y1);
            let b = g.add_node(x2, y2);
            g.add_edge(a, b);
        }
        let p = planarize_graph(&g);
        // Every directed edge must have an opposite direction mate
        let mut map: HashMap<(usize,usize), usize> = HashMap::new();
        for i in 0..p.half_from.len() { let u=p.half_from[i]; let v=p.half_to[i]; *map.entry((u,v)).or_insert(0)+=1; }
        for i in 0..p.half_from.len() { let u=p.half_from[i]; let v=p.half_to[i]; let rev = *map.get(&(v,u)).unwrap_or(&0); assert!(rev>=1, "missing reverse half-edge for {}->{}", u, v); }
    }
}
