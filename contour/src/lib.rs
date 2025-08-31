pub mod model;
pub mod geometry { pub mod math; pub mod flatten; pub mod tolerance; pub mod intersect; }
pub mod algorithms { pub mod picking; pub mod regions; pub mod planarize; }
mod json;
mod svg;

use serde::{Serialize, Deserialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use model::{Color, FillState, Node, HandleMode, Vec2, EdgeKind, Edge};

pub struct Graph {
    pub(crate) nodes: Vec<Option<Node>>, // id is index
    pub(crate) edges: Vec<Option<Edge>>, // id is index
    pub(crate) fills: HashMap<u32, FillState>, // region key -> fill
    pub(crate) geom_ver: u64,
    pub(crate) last_geom_ver: u64,
    pub(crate) prev_regions: Vec<(u32, f32, f32)>, // (key, cx, cy)
    pub(crate) flatten_tol: f32,
    // Picking spatial index: (built_geom_ver, index)
    pub(crate) pick_index: RefCell<Option<(u64, crate::algorithms::picking::PickIndex)>>,
}

pub struct EdgeArrays { pub ids: Vec<u32>, pub endpoints: Vec<u32>, pub kinds: Vec<u8>, pub stroke_rgba: Vec<u8>, pub stroke_widths: Vec<f32> }

#[derive(Serialize, Deserialize)]
pub enum Pick {
    #[serde(rename = "node")] Node { id: u32, dist: f32 },
    #[serde(rename = "edge")] Edge { id: u32, t: f32, dist: f32 },
    #[serde(rename = "handle")] Handle { edge: u32, end: u8, dist: f32 },
}

impl Graph {
    // Enforce handle constraints after edits. If changed_end is Some(0|1), we
    // preserve that end's length for Aligned, and mirror the other to equal length for Mirrored.
    fn enforce_handle_constraints(ha: Vec2, hb: Vec2, mode: HandleMode, changed_end: Option<u8>) -> (Vec2, Vec2) {
        use geometry::tolerance::EPS_LEN;
        match mode {
            HandleMode::Free => (ha, hb),
            HandleMode::Mirrored => {
                // Opposite directions; equal lengths.
                let la = (ha.x*ha.x + ha.y*ha.y).sqrt();
                let lb = (hb.x*hb.x + hb.y*hb.y).sqrt();
                if la <= EPS_LEN && lb <= EPS_LEN { return (Vec2{x:0.0,y:0.0}, Vec2{x:0.0,y:0.0}); }
                // Choose target length. If a specific end changed, use its length; else average.
                let target_len = match changed_end { Some(0) => la, Some(1) => lb, _ => 0.5*(la.max(0.0)+lb.max(0.0)) };
                // Direction from ha defines both (hb opposite).
                let (ux,uy) = if la > EPS_LEN { (ha.x/la, ha.y/la) } else if lb > EPS_LEN { (-hb.x/lb, -hb.y/lb) } else { (0.0,0.0) };
                let nha = Vec2 { x: ux * target_len, y: uy * target_len };
                let nhb = Vec2 { x: -ux * target_len, y: -uy * target_len };
                (nha, nhb)
            }
            HandleMode::Aligned => {
                // Opposite directions; preserve opposite length for the changed end if provided.
                let la = (ha.x*ha.x + ha.y*ha.y).sqrt();
                let lb = (hb.x*hb.x + hb.y*hb.y).sqrt();
                // Direction from ha if available, else opposite of hb.
                let (ux,uy, ref_len_b) = if changed_end == Some(0) {
                    let (ux,uy) = if la>EPS_LEN { (ha.x/la, ha.y/la) } else { (0.0,0.0) };
                    (ux,uy, lb)
                } else if changed_end == Some(1) {
                    // Use hb direction to infer ha; preserve ha length
                    let (vx,vy) = if lb>EPS_LEN { (hb.x/lb, hb.y/lb) } else { (0.0,0.0) };
                    (-vx,-vy, la)
                } else {
                    // Both changed: align by ha's direction and preserve hb length.
                    let (ux,uy) = if la>EPS_LEN { (ha.x/la, ha.y/la) } else if lb>EPS_LEN { (-hb.x/lb, -hb.y/lb) } else { (0.0,0.0) };
                    (ux,uy, lb)
                };
                let nha = if la>EPS_LEN { Vec2 { x: ux*la, y: uy*la } } else { ha };
                let nhb = if ref_len_b>EPS_LEN { Vec2 { x: -ux*ref_len_b, y: -uy*ref_len_b } } else { Vec2{x:0.0,y:0.0} };
                (nha, nhb)
            }
        }
    }
    pub fn new() -> Self { Graph { nodes: Vec::new(), edges: Vec::new(), fills: HashMap::new(), geom_ver: 1, last_geom_ver: 0, prev_regions: Vec::new(), flatten_tol: 0.25, pick_index: RefCell::new(None) } }
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
    // set_edge_cubic defined below with guards
    pub fn set_edge_line(&mut self, id: u32) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) { edge.kind = EdgeKind::Line; self.bump(); return true; }
        false
    }
    pub fn get_handles(&self, id: u32) -> Option<[f32;4]> {
        if let Some(Some(e)) = self.edges.get(id as usize) {
            let a = match self.nodes.get(e.a as usize).and_then(|n| *n) { Some(n)=>n, None=>return None };
            let b = match self.nodes.get(e.b as usize).and_then(|n| *n) { Some(n)=>n, None=>return None };
            if let EdgeKind::Cubic { ha, hb, .. } = e.kind { return Some([a.x+ha.x, a.y+ha.y, b.x+hb.x, b.y+hb.y]); }
        }
        None
    }
    pub fn get_handle_mode(&self, id: u32) -> Option<u8> {
        if let Some(Some(e)) = self.edges.get(id as usize) {
            if let EdgeKind::Cubic { mode, .. } = e.kind {
                return Some(match mode { HandleMode::Free=>0, HandleMode::Mirrored=>1, HandleMode::Aligned=>2 });
            }
        }
        None
    }
    pub fn set_handle_pos(&mut self, id: u32, end: u8, x: f32, y: f32) -> bool {
        if end != 0 && end != 1 { return false; }
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            let (mut ha, mut hb, mode) = match edge.kind { EdgeKind::Cubic { ha, hb, mode } => (ha,hb,mode), _ => return false };
            let a = match self.nodes.get(edge.a as usize).and_then(|n| *n) { Some(n)=>n, None=>return false };
            let b = match self.nodes.get(edge.b as usize).and_then(|n| *n) { Some(n)=>n, None=>return false };
            if end==0 { ha = Vec2 { x: x-a.x, y: y-a.y }; }
            else { hb = Vec2 { x: x-b.x, y: y-b.y }; }
            let (ha, hb) = Self::enforce_handle_constraints(ha, hb, mode, Some(end));
            edge.kind = EdgeKind::Cubic { ha, hb, mode };
            self.bump(); return true;
        }
        false
    }
    pub fn set_handle_mode(&mut self, id: u32, mode: u8) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            let (ha, hb, _) = match edge.kind { EdgeKind::Cubic { ha, hb, mode } => (ha,hb,mode), _ => return false };
            let m = match mode {1=>HandleMode::Mirrored,2=>HandleMode::Aligned,_=>HandleMode::Free};
            let (ha, hb) = Self::enforce_handle_constraints(ha, hb, m, None);
            edge.kind = EdgeKind::Cubic { ha, hb, mode: m }; self.bump(); return true;
        }
        false
    }
    pub fn set_edge_cubic(&mut self, id: u32, p1x: f32, p1y: f32, p2x: f32, p2y: f32) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            let a = match self.nodes.get(edge.a as usize).and_then(|n| *n) { Some(n)=>n, None=>return false };
            let b = match self.nodes.get(edge.b as usize).and_then(|n| *n) { Some(n)=>n, None=>return false };
            let ha = Vec2 { x: p1x - a.x, y: p1y - a.y };
            let hb = Vec2 { x: p2x - b.x, y: p2y - b.y };
            // If both handles collapse to anchors, keep as line (no-op cubic)
            let ha_l = (ha.x*ha.x + ha.y*ha.y).sqrt();
            let hb_l = (hb.x*hb.x + hb.y*hb.y).sqrt();
            if ha_l <= geometry::tolerance::EPS_LEN && hb_l <= geometry::tolerance::EPS_LEN {
                edge.kind = EdgeKind::Line;
            } else {
                edge.kind = EdgeKind::Cubic { ha, hb, mode: HandleMode::Free };
            }
            self.bump(); return true;
        }
        false
    }
    pub fn bend_edge_to(&mut self, id: u32, t: f32, tx: f32, ty: f32, stiffness: f32) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            let a = if let Some(n)=self.nodes.get(edge.a as usize).and_then(|n| *n) { n } else { return true };
            let b = if let Some(n)=self.nodes.get(edge.b as usize).and_then(|n| *n) { n } else { return true };
            let t = geometry::tolerance::clamp01(t);
            let (mut ha, mut hb, mode) = match edge.kind {
                EdgeKind::Cubic{ha,hb,mode} => (ha,hb,mode),
                EdgeKind::Line => {
                    // Convert to a simple cubic aligned with the segment unless degenerate.
                    let dx=b.x-a.x; let dy=b.y-a.y; let len=(dx*dx+dy*dy).sqrt();
                    if len < geometry::tolerance::EPS_LEN { return true; } // no-op on zero-length
                    let k=0.3*len; let ux=dx/len; let uy=dy/len;
                    (Vec2{x:ux*k, y:uy*k}, Vec2{x:-ux*k, y:-uy*k}, HandleMode::Free)
                }
                EdgeKind::Polyline{..} => return false,
            };
            let p1x=a.x+ha.x; let p1y=a.y+ha.y; let p2x=b.x+hb.x; let p2y=b.y+hb.y;
            let (cx,cy)=geometry::math::cubic_point(t,a.x,a.y,p1x,p1y,p2x,p2y,b.x,b.y);
            let dx=tx-cx; let dy=ty-cy; let c1=3.0*(1.0-t).powi(2)*t; let c2=3.0*(1.0-t)*t.powi(2);
            let l1=1.0; let l2=stiffness.max(geometry::tolerance::EPS_LEN);
            let denom=c1*c1/l1 + c2*c2/l2;
            if denom <= geometry::tolerance::EPS_DENOM {
                // Treat as no-op to avoid instability
                return true;
            }
            let ax=dx/denom; let ay=dy/denom; let d1x=(c1/l1)*ax; let d1y=(c1/l1)*ay; let d2x=(c2/l2)*ax; let d2y=(c2/l2)*ay;
            // Apply LS update then enforce constraints
            ha.x+=d1x; ha.y+=d1y; hb.x+=d2x; hb.y+=d2y;
            let changed = if t <= 0.5 { Some(0) } else { Some(1) };
            let (ha, hb) = Self::enforce_handle_constraints(ha, hb, mode, changed);
            // Commit cubic kind (including line->cubic conversion)
            edge.kind=EdgeKind::Cubic{ha,hb,mode}; self.bump(); return true;
        }
        false
    }

    // Freehand fitting: convert a sampled polyline into a chain of cubic edges
    pub fn add_freehand(&mut self, points: &[(f32,f32)], close: bool) -> Vec<u32> {
        fn rdp(points: &[(f32,f32)], eps: f32) -> Vec<(f32,f32)> {
            if points.len() <= 2 { return points.to_vec(); }
            let eps2 = eps*eps;
            fn perp_dist2(p: (f32,f32), a:(f32,f32), b:(f32,f32)) -> f32 {
                let (px,py) = p; let (x1,y1)=a; let (x2,y2)=b;
                let vx=x2-x1; let vy=y2-y1; let wx=px-x1; let wy=py-y1;
                let vv = vx*vx+vy*vy; if vv==0.0 { return wx*wx+wy*wy; }
                let t = (wx*vx+wy*vy)/vv; let t = if t<0.0 {0.0} else if t>1.0 {1.0} else {t};
                let sx = x1 + t*vx; let sy = y1 + t*vy; let dx=px-sx; let dy=py-sy; dx*dx+dy*dy
            }
            fn rec(slice:&[(f32,f32)], eps2:f32, out:&mut Vec<(f32,f32)>) {
                let n=slice.len(); if n<=2 { out.push(slice[0]); return; }
                let a=slice[0]; let b=slice[n-1];
                let mut idx=0usize; let mut md2=0.0f32;
                for i in 1..(n-1) { let d2=perp_dist2(slice[i], a, b); if d2>md2 { md2=d2; idx=i; } }
                if md2>eps2 { rec(&slice[..=idx], eps2, out); rec(&slice[idx..], eps2, out); }
                else { out.push(a); }
            }
            let mut out=Vec::new(); rec(points, eps2, &mut out); out.push(*points.last().unwrap()); out
        }

        fn angle_between(a:(f32,f32), b:(f32,f32)) -> f32 {
            let (ax,ay)=a; let (bx,by)=b; let da=(ax*ax+ay*ay).sqrt(); let db=(bx*bx+by*by).sqrt();
            if da==0.0 || db==0.0 { return 0.0; }
            let mut c = (ax*bx+ay*by)/(da*db); if c>1.0 { c=1.0; } else if c < -1.0 { c=-1.0; }
            c.acos().to_degrees()
        }

        fn resample_even(points: &[(f32,f32)], step: f32, close: bool) -> Vec<(f32,f32)> {
            let n = points.len();
            if n == 0 { return Vec::new(); }
            if n == 1 { return points.to_vec(); }
            let mut out: Vec<(f32,f32)> = Vec::new();
            let mut prev = points[0];
            out.push(prev);
            let mut carry = step;
            let mut seg_iter = 0usize;
            let total_segs = if close { n } else { n-1 };
            let mut i = 0usize;
            while seg_iter < total_segs {
                let j = if i+1 < n { i+1 } else { 0 };
                let (x1,y1) = prev;
                let (x2,y2) = points[j];
                let dx = x2 - prev.0; let dy = y2 - prev.1; // from prev to segment end
                let seg_len = (dx*dx + dy*dy).sqrt();
                if seg_len >= carry && carry > 0.0 {
                    let t = carry / seg_len;
                    let nx = x1 + t * (x2 - x1);
                    let ny = y1 + t * (y2 - y1);
                    out.push((nx, ny));
                    prev = (nx, ny);
                    // stay on same segment with reduced remaining length
                    carry = step;
                    continue;
                } else {
                    // move to next segment
                    carry -= seg_len;
                    prev = points[j];
                    i = j;
                    seg_iter += 1;
                }
            }
            if !close { if *out.last().unwrap() != *points.last().unwrap() { out.push(*points.last().unwrap()); } }
            out
        }

        let mut pts: Vec<(f32,f32)> = points.iter().copied().collect();
        // Basic guard and sampling sanity
        {
            use crate::geometry::tolerance::EPS_POS;
            pts.dedup_by(|a,b| (a.0-b.0).abs()<EPS_POS && (a.1-b.1).abs()<EPS_POS);
        }
        if pts.len()<2 { return Vec::new(); }
        // Simplify then resample to even spacing (~24 px)
        // Strong simplify first
        let rough = if pts.len()>4 { rdp(&pts, 4.0) } else { pts.clone() };
        // Target a small fixed number of anchors across the whole stroke
        let mut total_len = 0.0f32;
        for i in 0..(rough.len().saturating_sub(1)) {
            let (x1,y1)=rough[i]; let (x2,y2)=rough[i+1];
            total_len += ((x2-x1)*(x2-x1)+(y2-y1)*(y2-y1)).sqrt();
        }
        if close && rough.len()>=2 { let (x1,y1)=rough[rough.len()-1]; let (x2,y2)=rough[0]; total_len += ((x2-x1)*(x2-x1)+(y2-y1)*(y2-y1)).sqrt(); }
        let target_anchors = if close { 8usize } else { 6usize }; // very sparse by default
        let step = if close {
            if target_anchors>0 { (total_len / (target_anchors as f32)).max(40.0) } else { total_len.max(40.0) }
        } else {
            if target_anchors>1 { (total_len / ((target_anchors-1) as f32)).max(40.0) } else { total_len.max(40.0) }
        };
        let mut simp = resample_even(&rough, step, close);
        // Uniformly downsample to hard cap desired anchors
        let desired = target_anchors.max( if close { 3 } else { 2 } );
        if !close {
            if simp.len() > desired {
                let mut reduced: Vec<(f32,f32)> = Vec::with_capacity(desired);
                for k in 0..desired {
                    let idx = if desired>1 { ((k as f32) * ((simp.len()-1) as f32) / ((desired-1) as f32)).round() as usize } else { 0 };
                    reduced.push(simp[idx.min(simp.len()-1)]);
                }
                simp = reduced;
            }
        } else {
            if simp.len() > desired {
                let mut reduced: Vec<(f32,f32)> = Vec::with_capacity(desired);
                for k in 0..desired {
                    let idx = ((k as f32) * ((simp.len()) as f32) / (desired as f32)).round() as usize % simp.len();
                    reduced.push(simp[idx]);
                }
                simp = reduced;
            }
        }
        let n = simp.len(); if n<2 { return Vec::new(); }

        // Create nodes
        let mut node_ids: Vec<u32> = Vec::with_capacity(n);
        for &(x,y) in &simp { node_ids.push(self.add_node(x,y)); }

        // Segment-wise Catmull–Rom fit with clamping and cusp detection
        let cusp_deg = 160.0f32; // be more conservative about creating corners
        let clamp_factor = 0.8f32; // allow longer handles for smoother shapes
        let mut created_edges: Vec<u32> = Vec::new();
        let seg_count = if close { n } else { n-1 };
        for i in 0..seg_count {
            let i0 = i % n;
            let i1 = (i+1)%n;
            let im1 = if i0>0 { i0-1 } else { if close { n-1 } else { i0 } };
            let ip2 = if i1+1<n { i1+1 } else { if close { (i1+1)%n } else { i1 } };
            let p0 = simp[im1];
            let p1 = simp[i0];
            let p2 = simp[i1];
            let p3 = simp[ip2];
            // Catmull–Rom tangents
            let t1x = 0.5*(p2.0 - p0.0); let t1y = 0.5*(p2.1 - p0.1);
            let t2x = 0.5*(p3.0 - p1.0); let t2y = 0.5*(p3.1 - p1.1);
            // Initial controls
            let mut c1x = p1.0 + t1x/3.0; let mut c1y = p1.1 + t1y/3.0;
            let mut c2x = p2.0 - t2x/3.0; let mut c2y = p2.1 - t2y/3.0;
            // Cusp detection at p1 and p2
            let v_in = (p1.0 - p0.0, p1.1 - p0.1);
            let v_out = (p2.0 - p1.0, p2.1 - p1.1);
            if angle_between(v_in, v_out) >= cusp_deg { c1x=p1.0; c1y=p1.1; }
            let v_in2 = (p2.0 - p1.0, p2.1 - p1.1);
            let v_out2 = (p3.0 - p2.0, p3.1 - p2.1);
            if angle_between(v_in2, v_out2) >= cusp_deg { c2x=p2.0; c2y=p2.1; }
            // Clamp to segment length
            let seg_len = ((p2.0-p1.0)*(p2.0-p1.0)+(p2.1-p1.1)*(p2.1-p1.1)).sqrt();
            let max_len = clamp_factor * seg_len;
            let mut hx = c1x - p1.0; let mut hy = c1y - p1.1; let hl=(hx*hx+hy*hy).sqrt();
            if hl>max_len && hl>0.0 { hx*=max_len/hl; hy*=max_len/hl; c1x=p1.0+hx; c1y=p1.1+hy; }
            let mut kx = c2x - p2.0; let mut ky = c2y - p2.1; let kl=(kx*kx+ky*ky).sqrt();
            if kl>max_len && kl>0.0 { kx*=max_len/kl; ky*=max_len/kl; c2x=p2.0+kx; c2y=p2.1+ky; }
            // Create edge
            if let Some(eid) = self.add_edge(node_ids[i0], node_ids[i1]) {
                // Set cubic with mirrored handles for smoothness
                if let Some(Some(edge)) = self.edges.get_mut(eid as usize) {
                    let ha = Vec2 { x: c1x - p1.0, y: c1y - p1.1 };
                    let hb = Vec2 { x: c2x - p2.0, y: c2y - p2.1 };
                    edge.kind = EdgeKind::Cubic { ha, hb, mode: HandleMode::Mirrored };
                    self.bump();
                }
                created_edges.push(eid);
            }
        }
        created_edges
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
