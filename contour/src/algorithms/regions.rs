use std::collections::HashMap;
use serde::Serialize;
use crate::{Graph, model::{Vec2, EdgeKind, FillState}};
use crate::geometry::flatten::flatten_cubic;
use crate::geometry::tolerance::{QUANT_SCALE, EPS_FACE_AREA, EPS_ANG};
use crate::algorithms::planarize::planarize_graph;

#[derive(Clone)]
pub(crate) struct Region { pub key: u32, pub points: Vec<Vec2>, pub area: f32 }

fn polygon_area(poly: &Vec<Vec2>) -> f32 { let mut a=0.0; for i in 0..poly.len() { let j=(i+1)%poly.len(); a+= poly[i].x*poly[j].y - poly[j].x*poly[i].y; } 0.5*a }
pub(crate) fn polygon_centroid(poly: &Vec<Vec2>) -> (f32,f32) {
    let mut cx=0.0; let mut cy=0.0; let mut a=0.0;
    for i in 0..poly.len(){let j=(i+1)%poly.len(); let cross=poly[i].x*poly[j].y-poly[j].x*poly[i].y; a+=cross; cx+=(poly[i].x+poly[j].x)*cross; cy+=(poly[i].y+poly[j].y)*cross;}
    let a=a*0.5;
    if a.abs() < EPS_FACE_AREA { return (poly[0].x, poly[0].y); }
    (cx/(6.0*a), cy/(6.0*a))
}

fn region_key_from_edges(seq: &Vec<u32>) -> u32 { if seq.is_empty(){return 0;} let mut rev=seq.clone(); rev.reverse(); fn min_rot_u32(seq:&Vec<u32>)->Vec<u32>{ let n=seq.len(); let mut best:Option<Vec<u32>>=None; for s in 0..n{ let mut rot=Vec::with_capacity(n); for k in 0..n { rot.push(seq[(s+k)%n]); } if best.as_ref().map_or(true, |b| rot<*b) { best=Some(rot);} } best.unwrap() } let fwd=min_rot_u32(seq); let bwd=min_rot_u32(&rev); let canon=if fwd<=bwd {fwd} else {bwd}; let mut hash: u32 = 0x811C9DC5; for x in canon { for b in x.to_le_bytes() { hash ^= b as u32; hash = hash.wrapping_mul(0x01000193); } } hash }

impl Graph {
    pub(crate) fn compute_regions(&self) -> Vec<Region> {
        #[derive(Clone,Copy)] struct Pt{ x:f32, y:f32 }
        let plan = planarize_graph(self);
        let verts: Vec<Pt> = plan.verts.iter().map(|(x,y)| Pt{ x:*x, y:*y }).collect();
        let half_from = plan.half_from;
        let half_to = plan.half_to;
        let half_eid = plan.half_eid;

        let m=half_from.len();
        let mut adj:Vec<Vec<(usize,f32,usize)>>=vec![Vec::new(); verts.len()];
        for i in 0..m { let u=half_from[i]; let v=half_to[i]; let a=(verts[v].y-verts[u].y).atan2(verts[v].x-verts[u].x); adj[u].push((v,a,i)); }
        for lst in &mut adj { lst.sort_by(|x,y| { let c=x.1.partial_cmp(&y.1).unwrap(); if c!=std::cmp::Ordering::Equal { c } else { x.0.cmp(&y.0).then(x.2.cmp(&y.2)) } }); }
        let mut idx_map:HashMap<(usize,usize),Vec<usize>>=HashMap::new(); for i in 0..m { idx_map.entry((half_from[i], half_to[i])).or_default().push(i); }
        let mut used=vec![false;m]; let mut regions=Vec::new();
        for i_start in 0..m { if used[i_start] { continue; } let mut i_he=i_start; let mut cycle:Vec<usize>=Vec::new(); let mut cycle_eids=Vec::new(); let mut guard=0; loop { used[i_he]=true; let v=half_to[i_he]; let u=half_from[i_he]; cycle.push(u); cycle_eids.push(half_eid[i_he]); let lst=&adj[v]; if lst.is_empty() { break; } let mut rev_idx=None; if let Some(cands)=idx_map.get(&(v,u)) { for &c in cands { if half_from[c]==v && half_to[c]==u { rev_idx=Some(c); break; } } } let _rev_i = if let Some(ix)=rev_idx { ix } else { break }; let ang=(verts[u].y-verts[v].y).atan2(verts[u].x-verts[v].x); let mut idx=0usize; while idx<lst.len() && lst[idx].1 <= ang + EPS_ANG { idx+=1; } let next = if idx==lst.len() { 0 } else { idx }; let (w,_,_)=lst[next]; if let Some(list)=idx_map.get(&(v,w)) { let mut found=None; for &cand in list { if !used[cand] { found=Some(cand); break; } } if let Some(nhe)=found { i_he=nhe; } else { break; } } else { break; } guard+=1; if guard>100000 { break; } if i_he==i_start { break; } }
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
            if cycle_ids.len()>=3 && cur==start { let mut poly=Vec::new(); let mut edge_seq=Vec::new(); for i in 0..cycle_ids.len() { let u=cycle_ids[i]; let v=cycle_ids[(i+1)%cycle_ids.len()]; let nu = if let Some(n)=self.nodes.get(u as usize).and_then(|n| *n) { n } else { poly.clear(); break; }; let nv = if let Some(n)=self.nodes.get(v as usize).and_then(|n| *n) { n } else { poly.clear(); break; }; let mut added=false; for (eid_idx,e) in self.edges.iter().enumerate() { if let Some(e)=e { if (e.a==u && e.b==v) || (e.a==v && e.b==u) { match &e.kind { EdgeKind::Line => { if poly.is_empty(){ poly.push(Vec2{x:nu.x,y:nu.y}); } poly.push(Vec2{x:nv.x,y:nv.y}); }, EdgeKind::Cubic{ha,hb,..} => { let (ax,ay,bx,by,p1x,p1y,p2x,p2y)= if e.a==u { (nu.x,nu.y,nv.x,nv.y,nu.x+ha.x,nu.y+ha.y,nv.x+hb.x,nv.y+hb.y) } else { (nv.x,nv.y,nu.x,nu.y,nv.x+hb.x,nv.y+hb.y,nu.x+ha.x,nu.y+ha.y) }; if poly.is_empty(){ poly.push(Vec2{x:ax,y:ay}); } let mut pts=Vec::new(); flatten_cubic(&mut pts,ax,ay,p1x,p1y,p2x,p2y,bx,by,self.flatten_tol,0); for w in pts.into_iter().skip(1) { poly.push(w); } }, EdgeKind::Polyline{ points } => { if poly.is_empty(){ poly.push(Vec2{x:nu.x,y:nu.y}); } for p in points { poly.push(*p); } poly.push(Vec2{x:nv.x,y:nv.y}); } } edge_seq.push(eid_idx as u32); added=true; break; } } }
                if !added { poly.clear(); break; }
            }
            if poly.len()>=3 { let area=polygon_area(&poly); if area.abs()>=EPS_FACE_AREA { let key=region_key_from_edges(&edge_seq); regions.push(Region{ key, points: poly, area }); } }
        }}
        regions
    }
}

pub fn get_regions_with_fill(g: &mut Graph) -> Vec<serde_json::Value> {
    #[derive(Serialize)] struct RegionSer { key:u32, area:f32, filled:bool, color:Option<[u8;4]>, points:Vec<f32> }
    let mut regions = g.compute_regions();
    // Deterministic output: sort by key
    regions.sort_by(|a,b| a.key.cmp(&b.key));
    if g.last_geom_ver != g.geom_ver {
        let mut new_prev: Vec<(u32,i32,i32,f32)> = Vec::with_capacity(regions.len());
        for r in &regions { let (cx,cy)=polygon_centroid(&r.points); let qx=(cx*QUANT_SCALE).round() as i32; let qy=(cy*QUANT_SCALE).round() as i32; new_prev.push((r.key,qx,qy,r.area)); }
        // Deterministic remap: greedy nearest with tie-breakers
        let mut new_fills=HashMap::new();
        let old_prev = g.prev_regions.clone();
        let mut claimed: HashMap<u32,bool> = HashMap::new();
        let mut order: Vec<usize> = (0..new_prev.len()).collect();
        order.sort_by(|&i,&j| new_prev[i].1.cmp(&new_prev[j].1)
            .then(new_prev[i].2.cmp(&new_prev[j].2))
            .then(new_prev[i].3.partial_cmp(&new_prev[j].3).unwrap())
            .then(new_prev[i].0.cmp(&new_prev[j].0))
        );
        for idx in order {
            let (k_new,qx,qy,area_new) = new_prev[idx];
            let mut best: Option<(u32, i64, f32)> = None;
            for (k_old,oqx,oqy,area_old) in &old_prev {
                if claimed.get(k_old).copied().unwrap_or(false) { continue; }
                let dx = (qx as i64) - (*oqx as i64); let dy = (qy as i64) - (*oqy as i64);
                let d2 = dx*dx + dy*dy;
                let ad = (area_new - *area_old).abs();
                best = match best { None => Some((*k_old,d2,ad)), Some((bk,bd,ba)) => {
                    if d2 < bd { Some((*k_old,d2,ad)) }
                    else if d2 == bd && ad < ba { Some((*k_old,d2,ad)) }
                    else if d2 == bd && (ad - ba).abs() <= f32::EPSILON && *k_old < bk { Some((*k_old,d2,ad)) } else { Some((bk,bd,ba)) }
                } };
            }
            let st = if let Some((old_key,_,_))=best { claimed.insert(old_key,true); g.fills.get(&old_key).copied().unwrap_or(FillState{filled:true,color:None}) } else { g.fills.get(&k_new).copied().unwrap_or(FillState{filled:true,color:None}) };
            new_fills.insert(k_new, st);
        }
        g.fills=new_fills; g.prev_regions=new_prev; g.last_geom_ver=g.geom_ver;
    }
    regions.into_iter().map(|r| {
        let st=g.fills.get(&r.key).copied().unwrap_or(FillState{filled:true,color:None});
        let color=st.color.map(|c| [c.r,c.g,c.b,c.a]);
        let mut pts=Vec::with_capacity(r.points.len()*2); for p in &r.points { pts.push(p.x); pts.push(p.y); }
        serde_json::to_value(RegionSer{ key:r.key, area:r.area, filled:st.filled, color, points:pts }).unwrap()
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn lcg(seed: &mut u64) -> f32 { *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1); (((*seed >> 24) & 0xFFFF_FFFF) as u32) as f32 / (u32::MAX as f32) }

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
        let mut found=false; for r in rs { if r.area.abs() > 90.0 && r.area.abs() < 110.0 { found=true; break; } }
        assert!(found, "expected ~100 area face");
    }

    #[test]
    fn self_touch_no_crash() {
        let mut g = Graph::new();
        let a = g.add_node(0.0, 0.0);
        let b = g.add_node(10.0, 0.0);
        let c = g.add_node(10.0, 10.0);
        g.add_edge(a, b);
        g.add_edge(b, c); // touches at b
        let _ = g.compute_regions();
        // No assertion on faces; just ensure no panic and consistent return
    }

    #[test]
    fn jitter_stability_on_grid() {
        let mut g = Graph::new();
        let v = 6usize; let h = 5usize;
        let x0 = 0.0f32; let x1 = 120.0f32; let y0 = 0.0f32; let y1 = 100.0f32;
        let mut nodes: Vec<u32> = Vec::new();
        // Build verticals
        for i in 0..v {
            let t = (i as f32 + 1.0) / ((v + 1) as f32); let x = x0 + t*(x1-x0);
            let a = g.add_node(x, y0); let b = g.add_node(x, y1); nodes.push(a); nodes.push(b);
            g.add_edge(a,b);
        }
        // Build horizontals
        for j in 0..h {
            let t = (j as f32 + 1.0) / ((h + 1) as f32); let y = y0 + t*(y1-y0);
            let a = g.add_node(x0, y); let b = g.add_node(x1, y); nodes.push(a); nodes.push(b);
            g.add_edge(a,b);
        }
        let mut keys1: Vec<u32> = g.compute_regions().into_iter().map(|r| r.key).collect();
        keys1.sort_unstable();
        // Jitter nodes below quantization cell size (0.1 px) to test stability
        let mut seed = 0xCAFEBABE8BADF00Du64;
        for id in 0..g.nodes.len() as u32 {
            if let Some((_x,_y)) = g.get_node(id) {
                let jx = (lcg(&mut seed) - 0.5) * 0.06; // +/-0.03 px
                let jy = (lcg(&mut seed) - 0.5) * 0.06;
                let (x,y) = g.get_node(id).unwrap();
                g.move_node(id, x + jx, y + jy);
            }
        }
        let mut keys2: Vec<u32> = g.compute_regions().into_iter().map(|r| r.key).collect();
        keys2.sort_unstable();
        assert_eq!(keys1, keys2, "region keys must be stable under small jitter");
    }

    #[test]
    fn grid_face_count() {
        let mut g = Graph::new();
        let v = 7usize; // vertical lines (interior)
        let h = 6usize; // horizontal lines (interior)
        let x0 = 0.0f32; let x1 = 140.0f32; let y0 = 0.0f32; let y1 = 120.0f32;
        // Build verticals
        for i in 0..v {
            let t = (i as f32 + 1.0) / ((v + 1) as f32);
            let x = x0 + t * (x1 - x0);
            let a = g.add_node(x, y0);
            let b = g.add_node(x, y1);
            g.add_edge(a, b);
        }
        // Build horizontals
        for j in 0..h {
            let t = (j as f32 + 1.0) / ((h + 1) as f32);
            let y = y0 + t * (y1 - y0);
            let a = g.add_node(x0, y);
            let b = g.add_node(x1, y);
            g.add_edge(a, b);
        }
        let regions = g.compute_regions();
        // Count only interior rectangles: all vertices strictly interior (not touching boundary coords)
        let mut count = 0usize;
        for r in regions.iter() {
            if r.points.is_empty() { continue; }
            let mut interior = true;
            for p in &r.points {
                if (p.x - x0).abs() < 1e-5 || (p.x - x1).abs() < 1e-5 || (p.y - y0).abs() < 1e-5 || (p.y - y1).abs() < 1e-5 {
                    interior = false; break;
                }
            }
            if interior { count += 1; }
        }
        let expected = (v-1)*(h-1);
        assert_eq!(count, expected, "expected {} bounded interior faces, got {}", expected, count);
    }
}
