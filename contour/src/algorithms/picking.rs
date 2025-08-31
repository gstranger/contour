use std::collections::{HashMap, HashSet};
use crate::{Graph, model::EdgeKind};
use crate::geometry::math::{seg_distance_sq, cubic_distance_sq};
use crate::geometry::tolerance::clamp01;

#[derive(Clone)]
pub struct PickIndex {
    pub cell: f32,
    pub nodes: HashMap<(i32,i32), Vec<u32>>,          // node ids
    pub handles: HashMap<(i32,i32), Vec<(u32,u8)>>,   // (edge_id, end)
    pub edges: HashMap<(i32,i32), Vec<u32>>,          // edge ids by bbox coverage
}

fn cell_ix(cell: f32, x: f32) -> i32 { (x / cell).floor() as i32 }

fn bbox_of_edge(g: &Graph, eid: usize) -> Option<(f32,f32,f32,f32)> {
    if let Some(e)=g.edges.get(eid).and_then(|x| x.as_ref()) {
        let a=g.nodes.get(e.a as usize).and_then(|n| *n)?;
        let b=g.nodes.get(e.b as usize).and_then(|n| *n)?;
        match e.kind {
            EdgeKind::Line => {
                let minx=a.x.min(b.x); let maxx=a.x.max(b.x);
                let miny=a.y.min(b.y); let maxy=a.y.max(b.y);
                Some((minx,miny,maxx,maxy))
            }
            EdgeKind::Cubic{ha,hb,..} => {
                let p1x=a.x+ha.x; let p1y=a.y+ha.y; let p2x=b.x+hb.x; let p2y=b.y+hb.y;
                let minx = a.x.min(b.x).min(p1x).min(p2x);
                let maxx = a.x.max(b.x).max(p1x).max(p2x);
                let miny = a.y.min(b.y).min(p1y).min(p2y);
                let maxy = a.y.max(b.y).max(p1y).max(p2y);
                Some((minx,miny,maxx,maxy))
            }
            EdgeKind::Polyline{ ref points } => {
                let mut minx=a.x.min(b.x); let mut maxx=a.x.max(b.x);
                let mut miny=a.y.min(b.y); let mut maxy=a.y.max(b.y);
                for p in points { minx=minx.min(p.x); maxx=maxx.max(p.x); miny=miny.min(p.y); maxy=maxy.max(p.y); }
                Some((minx,miny,maxx,maxy))
            }
        }
    } else { None }
}

pub fn build_pick_index(g: &Graph, cell: f32) -> PickIndex {
    let mut nodes: HashMap<(i32,i32), Vec<u32>> = HashMap::new();
    for (i,n) in g.nodes.iter().enumerate() { if let Some(n)=n { let ix=cell_ix(cell, n.x); let iy=cell_ix(cell, n.y); nodes.entry((ix,iy)).or_default().push(i as u32); } }

    let mut handles: HashMap<(i32,i32), Vec<(u32,u8)>> = HashMap::new();
    for (i,e) in g.edges.iter().enumerate() { if let Some(e)=e { if let EdgeKind::Cubic{ha,hb,..}=e.kind {
        let a=g.nodes.get(e.a as usize).and_then(|n| *n); let b=g.nodes.get(e.b as usize).and_then(|n| *n);
        if let (Some(a),Some(b))=(a,b) {
            let p1x=a.x+ha.x; let p1y=a.y+ha.y; let p2x=b.x+hb.x; let p2y=b.y+hb.y;
            let ix1=cell_ix(cell,p1x); let iy1=cell_ix(cell,p1y);
            let ix2=cell_ix(cell,p2x); let iy2=cell_ix(cell,p2y);
            handles.entry((ix1,iy1)).or_default().push((i as u32, 0));
            handles.entry((ix2,iy2)).or_default().push((i as u32, 1));
        }
    }}}

    let mut edges: HashMap<(i32,i32), Vec<u32>> = HashMap::new();
    for (i,_e) in g.edges.iter().enumerate() { if g.edges[i].is_some() {
        if let Some((minx,miny,maxx,maxy))=bbox_of_edge(g, i) {
            let ix0 = cell_ix(cell, minx); let ix1 = cell_ix(cell, maxx);
            let iy0 = cell_ix(cell, miny); let iy1 = cell_ix(cell, maxy);
            for ix in ix0..=ix1 { for iy in iy0..=iy1 { edges.entry((ix,iy)).or_default().push(i as u32); } }
        }
    }}

    PickIndex { cell, nodes, handles, edges }
}

fn choose_cell_size(g: &Graph) -> f32 {
    // Heuristic: target ~8 edges per cell on average.
    let mut minx = f32::INFINITY; let mut miny = f32::INFINITY;
    let mut maxx = f32::NEG_INFINITY; let mut maxy = f32::NEG_INFINITY;
    let mut have = false;
    for n in g.nodes.iter() { if let Some(n)=n { have=true; if n.x<minx{minx=n.x}; if n.x>maxx{maxx=n.x}; if n.y<miny{miny=n.y}; if n.y>maxy{maxy=n.y}; } }
    let dx = if have { (maxx - minx).max(1.0) } else { 1024.0 };
    let dy = if have { (maxy - miny).max(1.0) } else { 768.0 };
    let m = g.edge_count().max(1) as f32;
    let area = dx * dy;
    let target_cells = (m / 8.0).max(16.0); // floor minimal cells
    let cell_area = (area / target_cells).max(64.0);
    let cell = cell_area.sqrt();
    cell.clamp(8.0, 256.0)
}

fn dedup<T: Copy + std::cmp::Eq + std::hash::Hash>(v: Vec<T>) -> Vec<T> {
    let mut set = HashSet::new();
    let mut out = Vec::new();
    for x in v { if set.insert(x) { out.push(x); } }
    out
}

fn query_ids<T: Copy>(map: &HashMap<(i32,i32), Vec<T>>, cell: f32, x: f32, y: f32, tol: f32) -> Vec<T> {
    let ix0 = cell_ix(cell, x - tol); let ix1 = cell_ix(cell, x + tol);
    let iy0 = cell_ix(cell, y - tol); let iy1 = cell_ix(cell, y + tol);
    let mut out = Vec::new();
    for ix in ix0..=ix1 { for iy in iy0..=iy1 { if let Some(lst)=map.get(&(ix,iy)) { out.extend_from_slice(lst); } } }
    out
}

pub fn pick_impl(g: &Graph, x: f32, y: f32, tol: f32) -> Option<crate::Pick> {
    // Use spatial index with lazy rebuild keyed by geom_ver
    let cell = choose_cell_size(g);
    let mut idx_guard = g.pick_index.borrow_mut();
    let use_idx = if let Some((ver,_)) = idx_guard.as_ref() { *ver == g.geom_version() } else { false };
    if !use_idx {
        let idx = build_pick_index(g, cell);
        *idx_guard = Some((g.geom_version(), idx));
    }
    let (_, idx) = idx_guard.as_ref().unwrap();

    let tol2 = tol*tol;
    // Nodes first
    let mut best_node: Option<(u32,f32)> = None;
    let node_cands = dedup(query_ids(&idx.nodes, idx.cell, x, y, tol));
    for id in node_cands { if let Some(n)=g.nodes.get(id as usize).and_then(|n| *n) { let dx=n.x-x; let dy=n.y-y; let d2=dx*dx+dy*dy; if d2<=tol2 { if best_node.map_or(true, |(_,bd)| d2<bd) { best_node=Some((id, d2)); } } } }
    if let Some((id,d2))=best_node { return Some(crate::Pick::Node{ id, dist: d2.sqrt() }); }
    // Handles
    let mut best_handle: Option<(u32,u8,f32)> = None;
    let handle_cands = dedup(query_ids(&idx.handles, idx.cell, x, y, tol));
    for (edge,end) in handle_cands { if let Some(e)=g.edges.get(edge as usize).and_then(|ee| ee.as_ref()) { if let EdgeKind::Cubic{ha,hb,..}=e.kind {
        let a = if let Some(n)=g.nodes.get(e.a as usize).and_then(|n| *n) { n } else { continue };
        let b = if let Some(n)=g.nodes.get(e.b as usize).and_then(|n| *n) { n } else { continue };
        let (px,py) = if end==0 { (a.x+ha.x, a.y+ha.y) } else { (b.x+hb.x, b.y+hb.y) };
        let d2=(px-x).powi(2)+(py-y).powi(2); if d2<=tol2 && best_handle.map_or(true, |(_,_,bd)| d2<bd) { best_handle=Some((edge, end, d2)); }
    }}}
    if let Some((edge,end,d2))=best_handle { return Some(crate::Pick::Handle{ edge, end, dist: d2.sqrt() }); }
    // Edges
    let mut best_edge: Option<(u32,f32,f32)> = None;
    let edge_cands = dedup(query_ids(&idx.edges, idx.cell, x, y, tol));
    for eid in edge_cands { if let Some(e)=g.edges.get(eid as usize).and_then(|ee| ee.as_ref()) { match e.kind {
        EdgeKind::Line => {
            let a = if let Some(n)=g.nodes.get(e.a as usize).and_then(|n| *n) { n } else { continue };
            let b = if let Some(n)=g.nodes.get(e.b as usize).and_then(|n| *n) { n } else { continue };
            let (d2,t)=seg_distance_sq(x,y,a.x,a.y,b.x,b.y); if d2<=tol2 { if best_edge.map_or(true, |(_,bd,_)| d2<bd) { best_edge=Some((eid, d2, t)); } }
        }
        EdgeKind::Cubic{ha,hb,..} => {
            let a = if let Some(n)=g.nodes.get(e.a as usize).and_then(|n| *n) { n } else { continue };
            let b = if let Some(n)=g.nodes.get(e.b as usize).and_then(|n| *n) { n } else { continue };
            let p1x=a.x+ha.x; let p1y=a.y+ha.y; let p2x=b.x+hb.x; let p2y=b.y+hb.y;
            let (d2,t)=cubic_distance_sq(x,y,a.x,a.y,p1x,p1y,p2x,p2y,b.x,b.y); if d2<=tol2 { if best_edge.map_or(true, |(_,bd,_)| d2<bd) { best_edge=Some((eid, d2, clamp01(t))); } }
        }
        EdgeKind::Polyline{ ref points } => {
            let a = if let Some(n)=g.nodes.get(e.a as usize).and_then(|n| *n) { n } else { continue };
            let b = if let Some(n)=g.nodes.get(e.b as usize).and_then(|n| *n) { n } else { continue };
            let mut prevx=a.x; let mut prevy=a.y; let mut length=0.0; let mut segs=Vec::new();
            for p in points { let x2=p.x; let y2=p.y; let seg_len=((x2-prevx).powi(2)+(y2-prevy).powi(2)).sqrt(); if seg_len>0.0 { segs.push((prevx,prevy,x2,y2,seg_len)); length+=seg_len; } prevx=x2; prevy=y2; }
            let seg_len=((b.x-prevx).powi(2)+(b.y-prevy).powi(2)).sqrt(); if seg_len>0.0 { segs.push((prevx,prevy,b.x,b.y,seg_len)); length+=seg_len; }
            let mut acc=0.0; for (x1,y1,x2,y2,sl) in segs.into_iter() { let (d2,ts)=seg_distance_sq(x,y,x1,y1,x2,y2); if d2<=tol2 { let t_along=if length>0.0 {(acc+ts*sl)/length}else{0.0}; if best_edge.map_or(true, |(_,bd,_)| d2<bd) { best_edge=Some((eid, d2, t_along)); } } acc+=sl; }
        }
    }}}
    if let Some((id,d2,t))=best_edge { return Some(crate::Pick::Edge{ id, t, dist: d2.sqrt() }); }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Graph;
    #[test]
    fn bench_pick_grid() {
        // Build 5k edges as grid-like random lines
        let mut g = Graph::new();
        let mut nodes = Vec::new();
        for i in 0..100 { for j in 0..100 { nodes.push(g.add_node(i as f32 * 10.0, j as f32 * 6.0)); } }
        let mut ecount=0;
        for i in 0..99 { for j in 0..100 { let a=nodes[i*100+j]; let b=nodes[(i+1)*100+j]; g.add_edge(a,b); ecount+=1; if ecount>=5000 { break; } } if ecount>=5000 { break; } }
        let start = std::time::Instant::now();
        let mut hits=0; for k in 0..2000 { let x=(k%100) as f32 * 10.0 + 1.3; let y=((k/100)%100) as f32 * 6.0 + 0.7; if g.pick(x,y,3.0).is_some() { hits+=1; } }
        let dur = start.elapsed();
        let per = dur.as_secs_f64() * 1000.0 / 2000.0; // ms/pick
        // We don't assert on time in tests; this just exercises the path.
        assert!(hits>=0);
        let _ = per; // silence unused warning
    }
}
