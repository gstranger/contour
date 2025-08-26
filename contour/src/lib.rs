pub mod model;
pub mod geometry { pub mod math; pub mod flatten; }
pub mod algorithms { pub mod picking; pub mod regions; }
mod json;
mod svg;

use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use model::{Color, FillState, Node, HandleMode, Vec2, EdgeKind, Edge};

#[derive(Default)]
pub struct Graph {
    pub(crate) nodes: Vec<Option<Node>>, // id is index
    pub(crate) edges: Vec<Option<Edge>>, // id is index
    pub(crate) fills: HashMap<u32, FillState>, // region key -> fill
    pub(crate) geom_ver: u64,
    pub(crate) last_geom_ver: u64,
    pub(crate) prev_regions: Vec<(u32, f32, f32)>, // (key, cx, cy)
    pub(crate) flatten_tol: f32,
}

pub struct EdgeArrays { pub ids: Vec<u32>, pub endpoints: Vec<u32>, pub kinds: Vec<u8>, pub stroke_rgba: Vec<u8>, pub stroke_widths: Vec<f32> }

#[derive(Serialize, Deserialize)]
pub enum Pick {
    #[serde(rename = "node")] Node { id: u32, dist: f32 },
    #[serde(rename = "edge")] Edge { id: u32, t: f32, dist: f32 },
    #[serde(rename = "handle")] Handle { edge: u32, end: u8, dist: f32 },
}

impl Graph {
    pub fn new() -> Self {
        Graph { nodes: Vec::new(), edges: Vec::new(), fills: HashMap::new(), geom_ver: 1, last_geom_ver: 0, prev_regions: Vec::new(), flatten_tol: 0.25 }
    }
    pub fn geom_version(&self) -> u64 { self.geom_ver }

    // Nodes
    pub fn add_node(&mut self, x: f32, y: f32) -> u32 {
        let id = self.nodes.len() as u32;
        self.nodes.push(Some(Node { x, y }));
        self.bump();
        id
    }
    pub fn move_node(&mut self, id: u32, x: f32, y: f32) -> bool {
        if let Some(Some(n)) = self.nodes.get_mut(id as usize) { n.x = x; n.y = y; self.bump(); return true; }
        false
    }
    pub fn get_node(&self, id: u32) -> Option<(f32,f32)> {
        self.nodes.get(id as usize).and_then(|n| *n).map(|n| (n.x, n.y))
    }
    pub fn remove_node(&mut self, id: u32) -> bool {
        if let Some(slot) = self.nodes.get_mut(id as usize) {
            if slot.is_some() {
                *slot = None;
                for e in self.edges.iter_mut() {
                    if let Some(edge) = e.as_ref() { if edge.a == id || edge.b == id { *e = None; } }
                }
                self.bump();
                return true;
            }
        }
        false
    }
    pub fn node_count(&self) -> u32 { self.nodes.iter().filter(|n| n.is_some()).count() as u32 }

    // Edges
    pub fn add_edge(&mut self, a: u32, b: u32) -> Option<u32> {
        if a == b { return None; }
        if self.nodes.get(a as usize).and_then(|n| n.as_ref()).is_none() { return None; }
        if self.nodes.get(b as usize).and_then(|n| n.as_ref()).is_none() { return None; }
        let id = self.edges.len() as u32;
        self.edges.push(Some(Edge { a, b, kind: EdgeKind::Line, stroke: None, stroke_width: 2.0 }));
        self.bump();
        Some(id)
    }
    pub fn remove_edge(&mut self, id: u32) -> bool {
        if let Some(slot) = self.edges.get_mut(id as usize) { if slot.is_some() { *slot = None; self.bump(); return true; } }
        false
    }
    pub fn edge_count(&self) -> u32 { self.edges.iter().filter(|e| e.is_some()).count() as u32 }

    pub fn get_node_arrays(&self) -> (Vec<u32>, Vec<f32>) {
        let mut ids = Vec::new();
        let mut pos = Vec::new();
        for (i, n) in self.nodes.iter().enumerate() { if let Some(n) = n { ids.push(i as u32); pos.push(n.x); pos.push(n.y); } }
        (ids, pos)
    }
    pub fn get_edge_arrays(&self) -> EdgeArrays {
        let mut ids = Vec::new();
        let mut ep = Vec::new();
        let mut kinds = Vec::new();
        let mut rgba = Vec::new();
        let mut widths = Vec::new();
        for (i, e) in self.edges.iter().enumerate() {
            if let Some(e) = e {
                ids.push(i as u32);
                ep.push(e.a); ep.push(e.b);
                kinds.push(match e.kind { EdgeKind::Line => 0, EdgeKind::Cubic {..} => 1, EdgeKind::Polyline {..} => 2 });
                if let Some(c) = e.stroke { rgba.extend_from_slice(&[c.r, c.g, c.b, c.a]); widths.push(e.stroke_width); }
                else { rgba.extend_from_slice(&[0,0,0,0]); widths.push(0.0); }
            }
        }
        EdgeArrays { ids, endpoints: ep, kinds, stroke_rgba: rgba, stroke_widths: widths }
    }

    // Picking return
    pub fn pick(&self, x: f32, y: f32, tol: f32) -> Option<Pick> {
        algorithms::picking::pick_impl(self, x, y, tol)
    }

    // JSON
    pub fn to_json_value(&self) -> serde_json::Value { json::to_json_impl(self) }
    pub fn from_json_value(&mut self, v: serde_json::Value) -> bool { json::from_json_impl(self, v) }

    // Clear
    pub fn clear(&mut self) { self.nodes.clear(); self.edges.clear(); self.fills.clear(); self.bump(); }

    // Styles and handles
    pub fn set_edge_style(&mut self, id: u32, r: u8, g: u8, b: u8, a: u8, width: f32) -> bool {
        if let Some(Some(e)) = self.edges.get_mut(id as usize) { e.stroke = Some(Color{r,g,b,a}); e.stroke_width = if width>0.0 { width } else { 2.0 }; return true; }
        false
    }
    pub fn get_edge_style(&self, id: u32) -> Option<(u8,u8,u8,u8,f32)> {
        if let Some(Some(e)) = self.edges.get(id as usize) { if let Some(c)=e.stroke { return Some((c.r,c.g,c.b,c.a,e.stroke_width)); } }
        None
    }
    pub fn set_edge_cubic(&mut self, id: u32, p1x: f32, p1y: f32, p2x: f32, p2y: f32) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            let a = self.nodes[edge.a as usize].unwrap();
            let b = self.nodes[edge.b as usize].unwrap();
            let ha = Vec2 { x: p1x - a.x, y: p1y - a.y };
            let hb = Vec2 { x: p2x - b.x, y: p2y - b.y };
            edge.kind = EdgeKind::Cubic { ha, hb, mode: HandleMode::Free };
            self.bump(); return true;
        }
        false
    }
    pub fn set_edge_line(&mut self, id: u32) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) { edge.kind = EdgeKind::Line; self.bump(); return true; }
        false
    }
    pub fn get_handles(&self, id: u32) -> Option<[f32;4]> {
        if let Some(Some(e)) = self.edges.get(id as usize) {
            let a = self.nodes[e.a as usize]?; let b = self.nodes[e.b as usize]?;
            if let EdgeKind::Cubic { ha, hb, .. } = e.kind { return Some([a.x+ha.x, a.y+ha.y, b.x+hb.x, b.y+hb.y]); }
        }
        None
    }
    pub fn set_handle_pos(&mut self, id: u32, end: u8, x: f32, y: f32) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            let (mut ha, mut hb, mode) = match edge.kind { EdgeKind::Cubic { ha, hb, mode } => (ha,hb,mode), _ => return false };
            let a = self.nodes[edge.a as usize].unwrap();
            let b = self.nodes[edge.b as usize].unwrap();
            if end==0 { ha = Vec2 { x: x-a.x, y: y-a.y }; match mode { HandleMode::Free=>{}, HandleMode::Mirrored=>{ hb = Vec2{x:-ha.x,y:-ha.y}; }, HandleMode::Aligned=>{ let len=(hb.x*hb.x+hb.y*hb.y).sqrt(); let mut vx=-ha.x; let mut vy=-ha.y; let vlen=(vx*vx+vy*vy).sqrt(); if vlen>0.0 {vx/=vlen; vy/=vlen;} hb=Vec2{x:vx*len,y:vy*len}; } } }
            else { hb = Vec2 { x: x-b.x, y: y-b.y }; match mode { HandleMode::Free=>{}, HandleMode::Mirrored=>{ ha = Vec2{x:-hb.x,y:-hb.y}; }, HandleMode::Aligned=>{ let len=(ha.x*ha.x+ha.y*ha.y).sqrt(); let mut vx=-hb.x; let mut vy=-hb.y; let vlen=(vx*vx+vy*vy).sqrt(); if vlen>0.0 {vx/=vlen; vy/=vlen;} ha=Vec2{x:vx*len,y:vy*len}; } } }
            edge.kind = EdgeKind::Cubic { ha, hb, mode };
            self.bump(); return true;
        }
        false
    }
    pub fn set_handle_mode(&mut self, id: u32, mode: u8) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            let (ha, hb) = match edge.kind { EdgeKind::Cubic { ha, hb, .. } => (ha,hb), _ => return false };
            let m = match mode {1=>HandleMode::Mirrored,2=>HandleMode::Aligned,_=>HandleMode::Free};
            edge.kind = EdgeKind::Cubic { ha, hb, mode: m }; self.bump(); return true;
        }
        false
    }
    pub fn bend_edge_to(&mut self, id: u32, t: f32, tx: f32, ty: f32, stiffness: f32) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            let a = self.nodes[edge.a as usize].unwrap();
            let b = self.nodes[edge.b as usize].unwrap();
            let (mut ha, mut hb, mode) = match edge.kind {
                EdgeKind::Cubic{ha,hb,mode} => (ha,hb,mode),
                EdgeKind::Line => {
                    let dx=b.x-a.x; let dy=b.y-a.y; let len=(dx*dx+dy*dy).sqrt().max(1.0); let k=0.3*len;
                    (Vec2{x:a.x+(dx/len)*k-a.x, y:a.y+(dy/len)*k-a.y}, Vec2{x:b.x-(dx/len)*k-b.x, y:b.y-(dy/len)*k-b.y}, HandleMode::Free)
                }
                EdgeKind::Polyline{..} => return false,
            };
            let p1x=a.x+ha.x; let p1y=a.y+ha.y; let p2x=b.x+hb.x; let p2y=b.y+hb.y;
            let (cx,cy)=geometry::math::cubic_point(t,a.x,a.y,p1x,p1y,p2x,p2y,b.x,b.y);
            let dx=tx-cx; let dy=ty-cy; let c1=3.0*(1.0-t).powi(2)*t; let c2=3.0*(1.0-t)*t.powi(2);
            let l1=1.0; let l2=stiffness.max(0.0001); let denom=c1*c1/l1 + c2*c2/l2; if denom<=0.0 { return false; }
            let ax=dx/denom; let ay=dy/denom; let d1x=(c1/l1)*ax; let d1y=(c1/l1)*ay; let d2x=(c2/l2)*ax; let d2y=(c2/l2)*ay;
            match mode { HandleMode::Free => { ha.x+=d1x; ha.y+=d1y; hb.x+=d2x; hb.y+=d2y; }
                HandleMode::Mirrored => { let mhx=ha.x-d1x; let mhy=ha.y-d1y; let len=(mhx*mhx+mhy*mhy).sqrt(); if len>0.0 { let vx=-mhx/len; let vy=-mhy/len; let tlen=((hb.x+d2x).powi(2)+(hb.y+d2y).powi(2)).sqrt(); hb.x=vx*tlen; hb.y=vy*tlen; } ha.x+=d1x; ha.y+=d1y; }
                HandleMode::Aligned => { ha.x+=d1x; ha.y+=d1y; let len=(hb.x*hb.x+hb.y*hb.y).sqrt(); let mut vx=-ha.x; let mut vy=-ha.y; let vlen=(vx*vx+vy*vy).sqrt(); if vlen>0.0 {vx/=vlen; vy/=vlen;} hb.x=vx*len; hb.y=vy*len; } }
            edge.kind=EdgeKind::Cubic{ha,hb,mode}; self.bump(); return true;
        }
        false
    }

    // Regions & fills
    pub fn set_flatten_tolerance(&mut self, tol: f32) { self.flatten_tol = tol.max(0.01).min(10.0); }
    pub fn get_regions(&mut self) -> Vec<serde_json::Value> { algorithms::regions::get_regions_with_fill(self) }
    pub fn toggle_region(&mut self, key: u32) -> bool { let cur=self.fills.get(&key).copied().unwrap_or(FillState{filled:true,color:None}); let next=!cur.filled; self.fills.insert(key, FillState{filled:next,color:cur.color}); next }
    pub fn set_region_fill(&mut self, key: u32, filled: bool) { let color=self.fills.get(&key).and_then(|st| st.color); self.fills.insert(key, FillState{filled, color}); }
    pub fn set_region_color(&mut self, key: u32, r:u8,g:u8,b:u8,a:u8) { let filled=self.fills.get(&key).map(|st| st.filled).unwrap_or(true); self.fills.insert(key, FillState{filled, color:Some(Color{r,g,b,a})}); }

    // Polyline
    pub fn set_edge_polyline(&mut self, id: u32, points: &[(f32,f32)]) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            edge.kind = EdgeKind::Polyline { points: points.iter().map(|(x,y)| Vec2{x:*x,y:*y}).collect() };
            self.bump(); return true;
        }
        false
    }
    pub fn get_polyline_points(&self, id: u32) -> Option<Vec<(f32,f32)>> {
        if let Some(Some(edge)) = self.edges.get(id as usize) { if let EdgeKind::Polyline{points}= &edge.kind { return Some(points.iter().map(|p|(p.x,p.y)).collect()); } }
        None
    }
    pub fn add_polyline_edge(&mut self, a: u32, b: u32, points: &[(f32,f32)]) -> Option<u32> {
        if a==b { return None; }
        if self.nodes.get(a as usize).and_then(|n| n.as_ref()).is_none() { return None; }
        if self.nodes.get(b as usize).and_then(|n| n.as_ref()).is_none() { return None; }
        let id = self.edges.len() as u32;
        let pts = points.iter().map(|(x,y)| Vec2{x:*x,y:*y}).collect();
        self.edges.push(Some(Edge { a, b, kind: EdgeKind::Polyline { points: pts }, stroke: None, stroke_width: 2.0 }));
        self.bump(); Some(id)
    }

    // SVG
    pub fn add_svg_path(&mut self, d: &str, style: Option<(u8,u8,u8,u8,f32)>) -> u32 { svg::add_svg_path_impl(self, d, style) }
    pub fn to_svg_paths(&self) -> Vec<String> { svg::to_svg_paths_impl(self) }

    fn bump(&mut self) { self.geom_ver = self.geom_ver.wrapping_add(1); }
}

// Transforms and grouping moves
impl Graph {
    pub fn transform_all(&mut self, s: f32, tx: f32, ty: f32, scale_stroke: bool) {
        for n in self.nodes.iter_mut() { if let Some(n)=n { n.x = n.x * s + tx; n.y = n.y * s + ty; } }
        for e in self.edges.iter_mut() {
            if let Some(e)=e { match &mut e.kind { EdgeKind::Line=>{}, EdgeKind::Cubic{ha,hb,..} => { ha.x*=s; ha.y*=s; hb.x*=s; hb.y*=s; }, EdgeKind::Polyline{points} => { for p in points { p.x = p.x * s + tx; p.y = p.y * s + ty; } } } if scale_stroke { e.stroke_width *= s; } }
        }
        self.bump();
    }
    pub fn translate_nodes(&mut self, ids:&[u32], dx:f32, dy:f32) -> u32 {
        let mut moved=0; for &id in ids { if let Some(Some(n))=self.nodes.get_mut(id as usize) { n.x+=dx; n.y+=dy; moved+=1; } }
        if moved>0 { self.bump(); }
        moved
    }
    pub fn translate_edges(&mut self, edge_ids:&[u32], dx:f32, dy:f32, split_shared: bool) -> u32 {
        let mut nodes_to_move: HashSet<u32> = HashSet::new();
        for &eid in edge_ids { if let Some(e)=self.edges.get(eid as usize).and_then(|e| e.as_ref()) { nodes_to_move.insert(e.a); nodes_to_move.insert(e.b); } }
        if split_shared {
            let selected: HashSet<u32> = edge_ids.iter().copied().collect();
            let mut remap: HashMap<u32,u32>=HashMap::new();
            for nid in nodes_to_move.clone().into_iter() {
                let mut used_elsewhere=false; for (i,e) in self.edges.iter().enumerate() { if let Some(e)=e { if (e.a==nid || e.b==nid) && !selected.contains(&(i as u32)) { used_elsewhere=true; break; } } }
                if used_elsewhere { let (x,y)=self.get_node(nid).unwrap(); let new_id=self.add_node(x,y); remap.insert(nid,new_id); }
            }
            if !remap.is_empty() { for &eid in edge_ids { if let Some(Some(e))=self.edges.get_mut(eid as usize) { if let Some(&na)=remap.get(&e.a) { e.a=na; } if let Some(&nb)=remap.get(&e.b) { e.b=nb; } } } nodes_to_move = remap.values().copied().collect(); }
        }
        let mut moved=0; for nid in nodes_to_move { if let Some((x,y))=self.get_node(nid) { if self.move_node(nid, x+dx, y+dy) { moved+=1; } } }
        moved
    }
}
