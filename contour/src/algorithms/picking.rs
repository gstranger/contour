use crate::{Graph, model::EdgeKind};
use crate::geometry::math::{seg_distance_sq, cubic_distance_sq};
use crate::geometry::tolerance::clamp01;

pub fn pick_impl(g: &Graph, x: f32, y: f32, tol: f32) -> Option<crate::Pick> {
    let tol2 = tol*tol;
    // Nodes first
    let mut best_node: Option<(u32,f32)> = None;
    for (i, n) in g.nodes.iter().enumerate() {
        if let Some(n) = n { let dx=n.x-x; let dy=n.y-y; let d2=dx*dx+dy*dy; if d2<=tol2 { if best_node.map_or(true, |(_,bd)| d2<bd) { best_node=Some((i as u32, d2)); } } }
    }
    if let Some((id,d2))=best_node { return Some(crate::Pick::Node{ id, dist: d2.sqrt() }); }
    // Handles
    let mut best_handle: Option<(u32,u8,f32)> = None;
    for (i,e) in g.edges.iter().enumerate() {
        if let Some(e)=e { if let EdgeKind::Cubic{ha,hb,..}=e.kind {
            let a=g.nodes[e.a as usize].unwrap(); let b=g.nodes[e.b as usize].unwrap();
            let p1x=a.x+ha.x; let p1y=a.y+ha.y; let p2x=b.x+hb.x; let p2y=b.y+hb.y;
            let d1=(p1x-x).powi(2)+(p1y-y).powi(2); if d1<=tol2 && best_handle.map_or(true, |(_,_,bd)| d1<bd) { best_handle=Some((i as u32, 0, d1)); }
            let d2=(p2x-x).powi(2)+(p2y-y).powi(2); if d2<=tol2 && best_handle.map_or(true, |(_,_,bd)| d2<bd) { best_handle=Some((i as u32, 1, d2)); }
        }}
    }
    if let Some((edge,end,d2))=best_handle { return Some(crate::Pick::Handle{ edge, end, dist: d2.sqrt() }); }
    // Edges
    let mut best_edge: Option<(u32,f32,f32)> = None;
    for (i,e) in g.edges.iter().enumerate() {
        if let Some(e)=e { match e.kind {
            EdgeKind::Line => {
                let a=g.nodes[e.a as usize].unwrap(); let b=g.nodes[e.b as usize].unwrap();
                let (d2,t)=seg_distance_sq(x,y,a.x,a.y,b.x,b.y); if d2<=tol2 { if best_edge.map_or(true, |(_,bd,_)| d2<bd) { best_edge=Some((i as u32, d2, t)); } }
            }
            EdgeKind::Cubic{ha,hb,..} => {
                let a=g.nodes[e.a as usize].unwrap(); let b=g.nodes[e.b as usize].unwrap();
                let p1x=a.x+ha.x; let p1y=a.y+ha.y; let p2x=b.x+hb.x; let p2y=b.y+hb.y;
                let (d2,t)=cubic_distance_sq(x,y,a.x,a.y,p1x,p1y,p2x,p2y,b.x,b.y); if d2<=tol2 { if best_edge.map_or(true, |(_,bd,_)| d2<bd) { best_edge=Some((i as u32, d2, clamp01(t))); } }
            }
            EdgeKind::Polyline{ ref points } => {
                let a=g.nodes[e.a as usize].unwrap(); let b=g.nodes[e.b as usize].unwrap();
                let mut prevx=a.x; let mut prevy=a.y; let mut length=0.0; let mut segs=Vec::new();
                for p in points { let x2=p.x; let y2=p.y; let seg_len=((x2-prevx).powi(2)+(y2-prevy).powi(2)).sqrt(); if seg_len>0.0 { segs.push((prevx,prevy,x2,y2,seg_len)); length+=seg_len; } prevx=x2; prevy=y2; }
                let seg_len=((b.x-prevx).powi(2)+(b.y-prevy).powi(2)).sqrt(); if seg_len>0.0 { segs.push((prevx,prevy,b.x,b.y,seg_len)); length+=seg_len; }
                let mut acc=0.0; for (x1,y1,x2,y2,sl) in segs.into_iter() { let (d2,ts)=seg_distance_sq(x,y,x1,y1,x2,y2); if d2<=tol2 { let t_along=if length>0.0 {(acc+ts*sl)/length}else{0.0}; if best_edge.map_or(true, |(_,bd,_)| d2<bd) { best_edge=Some((i as u32, d2, t_along)); } } acc+=sl; }
            }
        }}
    }
    if let Some((id,d2,t))=best_edge { return Some(crate::Pick::Edge{ id, t, dist: d2.sqrt() }); }
    None
}
