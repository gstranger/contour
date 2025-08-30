use std::collections::HashMap;
use serde::Serialize;
use crate::{Graph, model::{Vec2, EdgeKind, FillState}};
use crate::geometry::flatten::flatten_cubic;
use crate::geometry::tolerance::{QUANT_SCALE, EPS_FACE_AREA, EPS_ANG};

#[derive(Clone)]
pub(crate) struct Region { pub key: u32, pub points: Vec<Vec2>, pub area: f32 }

fn polygon_area(poly: &Vec<Vec2>) -> f32 { let mut a=0.0; for i in 0..poly.len() { let j=(i+1)%poly.len(); a+= poly[i].x*poly[j].y - poly[j].x*poly[i].y; } 0.5*a }
pub(crate) fn polygon_centroid(poly: &Vec<Vec2>) -> (f32,f32) { let mut cx=0.0; let mut cy=0.0; let mut a=0.0; for i in 0..poly.len(){let j=(i+1)%poly.len(); let cross=poly[i].x*poly[j].y-poly[j].x*poly[i].y; a+=cross; cx+=(poly[i].x+poly[j].x)*cross; cy+=(poly[i].y+poly[j].y)*cross;} let a=a*0.5; if a.abs()<1e-6 { return (poly[0].x, poly[0].y); } (cx/(6.0*a), cy/(6.0*a)) }

fn region_key_from_edges(seq: &Vec<u32>) -> u32 { if seq.is_empty(){return 0;} let mut rev=seq.clone(); rev.reverse(); fn min_rot_u32(seq:&Vec<u32>)->Vec<u32>{ let n=seq.len(); let mut best:Option<Vec<u32>>=None; for s in 0..n{ let mut rot=Vec::with_capacity(n); for k in 0..n { rot.push(seq[(s+k)%n]); } if best.as_ref().map_or(true, |b| rot<*b) { best=Some(rot);} } best.unwrap() } let fwd=min_rot_u32(seq); let bwd=min_rot_u32(&rev); let canon=if fwd<=bwd {fwd} else {bwd}; let mut hash: u32 = 0x811C9DC5; for x in canon { for b in x.to_le_bytes() { hash ^= b as u32; hash = hash.wrapping_mul(0x01000193); } } hash }

impl Graph {
    pub(crate) fn compute_regions(&self) -> Vec<Region> {
        #[derive(Clone,Copy)] struct Pt{ x:f32, y:f32 }
        let mut segs: Vec<(Pt,Pt,u32)> = Vec::new();
        for (eid,e) in self.edges.iter().enumerate() {
            if let Some(e)=e { let a=self.nodes[e.a as usize].unwrap(); let b=self.nodes[e.b as usize].unwrap(); match e.kind {
                EdgeKind::Line => segs.push((Pt{x:a.x,y:a.y}, Pt{x:b.x,y:b.y}, eid as u32)),
                EdgeKind::Cubic{ha,hb,..} => { let p1x=a.x+ha.x; let p1y=a.y+ha.y; let p2x=b.x+hb.x; let p2y=b.y+hb.y; let mut pts=Vec::new(); pts.push(Vec2{x:a.x,y:a.y}); flatten_cubic(&mut pts,a.x,a.y,p1x,p1y,p2x,p2y,b.x,b.y,self.flatten_tol,0); for w in pts.windows(2){ segs.push((Pt{x:w[0].x,y:w[0].y}, Pt{x:w[1].x,y:w[1].y}, eid as u32)); } },
                EdgeKind::Polyline{ ref points } => { let mut prev=Pt{x:a.x,y:a.y}; for p in points { let next=Pt{x:p.x,y:p.y}; segs.push((prev,next,eid as u32)); prev=next; } segs.push((prev, Pt{x:b.x,y:b.y}, eid as u32)); },
            }}
        }
        let scale=QUANT_SCALE; let mut vid_map:HashMap<(i32,i32),usize>=HashMap::new(); let mut verts:Vec<Pt>=Vec::new(); let mut half_from=Vec::new(); let mut half_to=Vec::new(); let mut half_eid=Vec::new();
        for (p,q,eid) in segs { let qx=|v:f32|(v*scale).round() as i32; let k1=(qx(p.x),qx(p.y)); let k2=(qx(q.x),qx(q.y)); let u=*vid_map.entry(k1).or_insert_with(||{let id=verts.len(); verts.push(p); id}); let v=*vid_map.entry(k2).or_insert_with(||{let id=verts.len(); verts.push(q); id}); if u==v { continue; } half_from.push(u); half_to.push(v); half_eid.push(eid); half_from.push(v); half_to.push(u); half_eid.push(eid); }
        let m=half_from.len(); let mut adj:Vec<Vec<(usize,f32)>>=vec![Vec::new(); verts.len()]; for i in 0..m { let u=half_from[i]; let v=half_to[i]; let a=(verts[v].y-verts[u].y).atan2(verts[v].x-verts[u].x); adj[u].push((v,a)); } for lst in &mut adj { lst.sort_by(|x,y| x.1.partial_cmp(&y.1).unwrap()); }
        let mut idx_map:HashMap<(usize,usize),Vec<usize>>=HashMap::new(); for i in 0..m { idx_map.entry((half_from[i], half_to[i])).or_default().push(i); }
        let mut used=vec![false;m]; let mut regions=Vec::new();
        for i_start in 0..m { if used[i_start] { continue; } let mut i_he=i_start; let mut cycle:Vec<usize>=Vec::new(); let mut cycle_eids=Vec::new(); let mut guard=0; loop { used[i_he]=true; let v=half_to[i_he]; let u=half_from[i_he]; cycle.push(u); cycle_eids.push(half_eid[i_he]); let lst=&adj[v]; if lst.is_empty() { break; } let mut rev_idx=None; if let Some(cands)=idx_map.get(&(v,u)) { for &c in cands { if half_from[c]==v && half_to[c]==u { rev_idx=Some(c); break; } } } let _rev_i = if let Some(ix)=rev_idx { ix } else { break }; let ang=(verts[u].y-verts[v].y).atan2(verts[u].x-verts[v].x); let mut idx=0usize; while idx<lst.len() && lst[idx].1 <= ang + EPS_ANG { idx+=1; } let prev = if idx==0 { lst.len()-1 } else { idx-1 }; let (w,_)=lst[prev]; if let Some(list)=idx_map.get(&(v,w)) { let mut found=None; for &cand in list { if !used[cand] { found=Some(cand); break; } } if let Some(nhe)=found { i_he=nhe; } else { break; } } else { break; } guard+=1; if guard>100000 { break; } if i_he==i_start { break; } }
            if cycle.len()>=3 { let mut poly=Vec::new(); for &idx in &cycle { poly.push(Vec2{x:verts[idx].x, y:verts[idx].y}); } let area=polygon_area(&poly); if area.abs() < EPS_FACE_AREA { continue; } let mut seq=Vec::new(); for &e in &cycle_eids { if seq.last().copied()!=Some(e) { seq.push(e);} } if seq.len()>=2 && seq.first()==seq.last() { seq.pop(); } let key=region_key_from_edges(&seq); regions.push(Region{ key, points: poly, area }); }
        }
        if regions.is_empty() { regions = self.find_simple_cycles(); }
        regions
    }

    pub(crate) fn find_simple_cycles(&self) -> Vec<Region> {
        let mut adj:HashMap<u32,Vec<u32>>=HashMap::new();
        for e in self.edges.iter() { if let Some(e)=e { if self.nodes.get(e.a as usize).and_then(|n| *n).is_none() { continue; } if self.nodes.get(e.b as usize).and_then(|n| *n).is_none() { continue; } adj.entry(e.a).or_default().push(e.b); adj.entry(e.b).or_default().push(e.a); } }
        let mut visited:HashMap<u32,bool>=HashMap::new(); let mut regions=Vec::new();
        for (&start, neigh) in adj.iter() { if neigh.len()!=2 { continue; } if visited.get(&start).copied().unwrap_or(false) { continue; }
            let mut cycle_ids=Vec::new(); let mut prev=start; let mut cur=start; let mut guard=0; loop { cycle_ids.push(cur); visited.insert(cur,true); let ns=adj.get(&cur).cloned().unwrap_or_default(); let mut found=None; for n in ns { if n!=prev { found=Some(n); break; } } if let Some(nxt)=found { prev=cur; cur=nxt; } else { break; } guard+=1; if guard>10000 { break; } if cur==start { break; } }
            if cycle_ids.len()>=3 && cur==start { let mut poly=Vec::new(); let mut edge_seq=Vec::new(); for i in 0..cycle_ids.len() { let u=cycle_ids[i]; let v=cycle_ids[(i+1)%cycle_ids.len()]; let nu=self.nodes[u as usize].unwrap(); let nv=self.nodes[v as usize].unwrap(); let mut added=false; for (eid_idx,e) in self.edges.iter().enumerate() { if let Some(e)=e { if (e.a==u && e.b==v) || (e.a==v && e.b==u) { match &e.kind { EdgeKind::Line => { if poly.is_empty(){ poly.push(Vec2{x:nu.x,y:nu.y}); } poly.push(Vec2{x:nv.x,y:nv.y}); }, EdgeKind::Cubic{ha,hb,..} => { let (ax,ay,bx,by,p1x,p1y,p2x,p2y)= if e.a==u { (nu.x,nu.y,nv.x,nv.y,nu.x+ha.x,nu.y+ha.y,nv.x+hb.x,nv.y+hb.y) } else { (nv.x,nv.y,nu.x,nu.y,nv.x+hb.x,nv.y+hb.y,nu.x+ha.x,nu.y+ha.y) }; if poly.is_empty(){ poly.push(Vec2{x:ax,y:ay}); } let mut pts=Vec::new(); flatten_cubic(&mut pts,ax,ay,p1x,p1y,p2x,p2y,bx,by,self.flatten_tol,0); for w in pts.into_iter().skip(1) { poly.push(w); } }, EdgeKind::Polyline{ points } => { if poly.is_empty(){ poly.push(Vec2{x:nu.x,y:nu.y}); } for p in points { poly.push(*p); } poly.push(Vec2{x:nv.x,y:nv.y}); } } edge_seq.push(eid_idx as u32); added=true; break; } } }
                if !added { poly.clear(); break; }
            }
            if poly.len()>=3 { let area=polygon_area(&poly); if area.abs()>=EPS_FACE_AREA { let key=region_key_from_edges(&edge_seq); regions.push(Region{ key, points: poly, area }); } }
        }}
        regions
    }
}

pub fn get_regions_with_fill(g: &mut Graph) -> Vec<serde_json::Value> {
    #[derive(Serialize)] struct RegionSer { key:u32, area:f32, filled:bool, color:Option<[u8;4]>, points:Vec<f32> }
    let regions = g.compute_regions();
    if g.last_geom_ver != g.geom_ver {
        let mut new_fills=HashMap::new(); let mut new_prev=Vec::with_capacity(regions.len());
        for r in &regions { let (cx,cy)=polygon_centroid(&r.points); new_prev.push((r.key,cx,cy)); }
        for (k_new,cx,cy) in &new_prev { let mut best:Option<(u32,f32)>=None; for (k_old,ox,oy) in &g.prev_regions { let dx=cx-ox; let dy=cy-oy; let d2=dx*dx+dy*dy; if best.map_or(true, |(_,bd)| d2<bd) { best=Some((*k_old,d2)); } } let st = if let Some((old_key,d2))=best { if d2<400.0 { g.fills.get(&old_key).copied().unwrap_or(FillState{filled:true,color:None}) } else { g.fills.get(k_new).copied().unwrap_or(FillState{filled:true,color:None}) } } else { g.fills.get(k_new).copied().unwrap_or(FillState{filled:true,color:None}) }; new_fills.insert(*k_new, st); }
        g.fills=new_fills; g.prev_regions=new_prev; g.last_geom_ver=g.geom_ver;
    }
    regions.into_iter().map(|r| {
        let st=g.fills.get(&r.key).copied().unwrap_or(FillState{filled:true,color:None});
        let color=st.color.map(|c| [c.r,c.g,c.b,c.a]);
        let mut pts=Vec::with_capacity(r.points.len()*2); for p in &r.points { pts.push(p.x); pts.push(p.y); }
        serde_json::to_value(RegionSer{ key:r.key, area:r.area, filled:st.filled, color, points:pts }).unwrap()
    }).collect()
}
