use wasm_bindgen::prelude::*;
use js_sys::{Float32Array, Object, Uint32Array, Uint8Array, Reflect};
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Color { r: u8, g: u8, b: u8, a: u8 }

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct FillState { filled: bool, color: Option<Color> }

#[derive(Clone, Copy, Debug)]
struct Node { x: f32, y: f32 }

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum HandleMode { Free = 0, Mirrored = 1, Aligned = 2 }

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Vec2 { x: f32, y: f32 }

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum EdgeKind {
    Line,
    Cubic { ha: Vec2, hb: Vec2, mode: HandleMode },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Edge {
    a: u32,
    b: u32,
    kind: EdgeKind,
    // Optional stroke style for rendering
    stroke: Option<Color>,
    stroke_width: f32,
}

#[wasm_bindgen]
pub struct Graph {
    nodes: Vec<Option<Node>>, // id is index
    edges: Vec<Option<Edge>>, // id is index
    fills: HashMap<u32, FillState>, // region fill state keyed by region key
    geom_ver: u64, // increments on geometry edits
    last_geom_ver: u64,
    prev_regions: Vec<(u32, f32, f32)>, // (key, cx, cy)
    flatten_tol: f32, // curve flattening tolerance in pixels
}

#[wasm_bindgen]
impl Graph {
    /// Monotonic geometry version; increments on node/edge/handle edits
    pub fn geom_version(&self) -> u64 { self.geom_ver }
    #[wasm_bindgen(constructor)]
    pub fn new() -> Graph {
        Graph { nodes: Vec::new(), edges: Vec::new(), fills: HashMap::new(), geom_ver: 1, last_geom_ver: 0, prev_regions: Vec::new(), flatten_tol: 0.25 }
    }

    // Nodes
    pub fn add_node(&mut self, x: f32, y: f32) -> u32 {
        let id = self.nodes.len() as u32;
        self.nodes.push(Some(Node { x, y }));
        self.geom_ver = self.geom_ver.wrapping_add(1);
        id
    }

    pub fn move_node(&mut self, id: u32, x: f32, y: f32) -> bool {
        match self.nodes.get_mut(id as usize) {
            Some(slot) if slot.is_some() => {
                if let Some(n) = slot.as_mut() { n.x = x; n.y = y; }
                self.geom_ver = self.geom_ver.wrapping_add(1);
                true
            }
            _ => false,
        }
    }

    pub fn get_node(&self, id: u32) -> JsValue {
        if let Some(Some(n)) = self.nodes.get(id as usize) {
            let arr = vec![n.x, n.y];
            serde_wasm_bindgen::to_value(&arr).unwrap_or(JsValue::NULL)
        } else {
            JsValue::NULL
        }
    }

    pub fn remove_node(&mut self, id: u32) -> bool {
        if let Some(slot) = self.nodes.get_mut(id as usize) {
            if slot.is_some() {
                *slot = None;
                // Also remove edges incident to this node
                for e in self.edges.iter_mut() {
                    if let Some(edge) = e.as_ref() {
                        if edge.a == id || edge.b == id { *e = None; }
                    }
                }
                self.geom_ver = self.geom_ver.wrapping_add(1);
                return true;
            }
        }
        false
    }

    pub fn node_count(&self) -> u32 {
        self.nodes.iter().filter(|n| n.is_some()).count() as u32
    }

    // Edges (undirected lines)
    pub fn add_edge(&mut self, a: u32, b: u32) -> Option<u32> {
        if a == b { return None; }
        if self.nodes.get(a as usize).and_then(|n| n.as_ref()).is_none() { return None; }
        if self.nodes.get(b as usize).and_then(|n| n.as_ref()).is_none() { return None; }
        let id = self.edges.len() as u32;
        self.edges.push(Some(Edge { a, b, kind: EdgeKind::Line, stroke: None, stroke_width: 2.0 }));
        self.geom_ver = self.geom_ver.wrapping_add(1);
        Some(id)
    }

    pub fn remove_edge(&mut self, id: u32) -> bool {
        if let Some(slot) = self.edges.get_mut(id as usize) {
            if slot.is_some() { *slot = None; self.geom_ver = self.geom_ver.wrapping_add(1); return true; }
        }
        false
    }

    pub fn edge_count(&self) -> u32 {
        self.edges.iter().filter(|e| e.is_some()).count() as u32
    }

    // Typed array getters for efficient rendering
    pub fn get_node_data(&self) -> JsValue {
        let mut ids: Vec<u32> = Vec::new();
        let mut positions: Vec<f32> = Vec::new();
        for (i, n) in self.nodes.iter().enumerate() {
            if let Some(n) = n {
                ids.push(i as u32);
                positions.push(n.x);
                positions.push(n.y);
            }
        }
        let ids_arr = Uint32Array::from(ids.as_slice());
        let pos_arr = Float32Array::from(positions.as_slice());
        let obj = Object::new();
        let _ = Reflect::set(&obj, &JsValue::from_str("ids"), &ids_arr);
        let _ = Reflect::set(&obj, &JsValue::from_str("positions"), &pos_arr);
        obj.into()
    }

    pub fn get_edge_data(&self) -> JsValue {
        let mut ids: Vec<u32> = Vec::new();
        let mut endpoints: Vec<u32> = Vec::new();
        let mut kinds: Vec<u8> = Vec::new();
        let mut stroke_rgba: Vec<u8> = Vec::new(); // 4 per edge
        let mut stroke_widths: Vec<f32> = Vec::new();
        for (i, e) in self.edges.iter().enumerate() {
            if let Some(e) = e {
                ids.push(i as u32);
                endpoints.push(e.a);
                endpoints.push(e.b);
                kinds.push(match e.kind { EdgeKind::Line => 0, EdgeKind::Cubic { .. } => 1 });
                if let Some(c) = e.stroke {
                    stroke_rgba.push(c.r); stroke_rgba.push(c.g); stroke_rgba.push(c.b); stroke_rgba.push(c.a);
                    stroke_widths.push(e.stroke_width);
                } else {
                    // 0 alpha and 0 width indicate "unset" to consumers
                    stroke_rgba.push(0); stroke_rgba.push(0); stroke_rgba.push(0); stroke_rgba.push(0);
                    stroke_widths.push(0.0);
                }
            }
        }
        let ids_arr = Uint32Array::from(ids.as_slice());
        let ep_arr = Uint32Array::from(endpoints.as_slice());
        let kinds_arr = Uint8Array::from(kinds.as_slice());
        let rgba_arr = Uint8Array::from(stroke_rgba.as_slice());
        let width_arr = Float32Array::from(stroke_widths.as_slice());
        let obj = Object::new();
        let _ = Reflect::set(&obj, &JsValue::from_str("ids"), &ids_arr);
        let _ = Reflect::set(&obj, &JsValue::from_str("endpoints"), &ep_arr);
        let _ = Reflect::set(&obj, &JsValue::from_str("kinds"), &kinds_arr);
        let _ = Reflect::set(&obj, &JsValue::from_str("stroke_rgba"), &rgba_arr);
        let _ = Reflect::set(&obj, &JsValue::from_str("stroke_widths"), &width_arr);
        obj.into()
    }

    // Picking: returns { kind: "node"|"edge", id, dist, t? }
    pub fn pick(&self, x: f32, y: f32, tol: f32) -> JsValue {
        let tol2 = tol * tol;
        // Prefer nodes when within tolerance
        let mut best_node: Option<(u32, f32)> = None; // (id, dist2)
        for (i, n) in self.nodes.iter().enumerate() {
            if let Some(n) = n {
                let dx = n.x - x; let dy = n.y - y;
                let d2 = dx*dx + dy*dy;
                if d2 <= tol2 {
                    if best_node.map_or(true, |(_, bd2)| d2 < bd2) {
                        best_node = Some((i as u32, d2));
                    }
                }
            }
        }
        if let Some((id, d2)) = best_node {
            let mut obj = Object::new();
            let _ = Reflect::set(&obj, &JsValue::from_str("kind"), &JsValue::from_str("node"));
            let _ = Reflect::set(&obj, &JsValue::from_str("id"), &JsValue::from_f64(id as f64));
            let _ = Reflect::set(&obj, &JsValue::from_str("dist"), &JsValue::from_f64(d2.sqrt() as f64));
            return obj.into();
        }

        // Prefer handles over edges when within tolerance
        // Handles
        let mut best_handle: Option<(u32, u8, f32)> = None; // (edge_id, end 0|1, dist2)
        for (i, e) in self.edges.iter().enumerate() {
            if let Some(e) = e {
                match e.kind {
                    EdgeKind::Cubic { ha, hb, .. } => {
                        let a = match self.nodes.get(e.a as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                        let b = match self.nodes.get(e.b as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                        let p1x = a.x + ha.x; let p1y = a.y + ha.y;
                        let p2x = b.x + hb.x; let p2y = b.y + hb.y;
                        let d1 = (p1x - x).powi(2) + (p1y - y).powi(2);
                        if d1 <= tol2 && best_handle.map_or(true, |(_,_,bd)| d1 < bd) {
                            best_handle = Some((i as u32, 0, d1));
                        }
                        let d2 = (p2x - x).powi(2) + (p2y - y).powi(2);
                        if d2 <= tol2 && best_handle.map_or(true, |(_,_,bd)| d2 < bd) {
                            best_handle = Some((i as u32, 1, d2));
                        }
                    }
                    _ => {}
                }
            }
        }
        if let Some((edge_id, end, d2)) = best_handle {
            let mut obj = Object::new();
            let _ = Reflect::set(&obj, &JsValue::from_str("kind"), &JsValue::from_str("handle"));
            let _ = Reflect::set(&obj, &JsValue::from_str("edge"), &JsValue::from_f64(edge_id as f64));
            let _ = Reflect::set(&obj, &JsValue::from_str("end"), &JsValue::from_f64(end as f64));
            let _ = Reflect::set(&obj, &JsValue::from_str("dist"), &JsValue::from_f64(d2.sqrt() as f64));
            return obj.into();
        }

        // Else check edges
        let mut best_edge: Option<(u32, f32, f32)> = None; // (id, dist2, t)
        for (i, e) in self.edges.iter().enumerate() {
            if let Some(e) = e {
                match e.kind {
                    EdgeKind::Line => {
                        let a = match self.nodes.get(e.a as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                        let b = match self.nodes.get(e.b as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                        let (d2, t) = seg_distance_sq(x, y, a.x, a.y, b.x, b.y);
                        if d2 <= tol2 {
                            if best_edge.map_or(true, |(_, bd2, _)| d2 < bd2) {
                                best_edge = Some((i as u32, d2, t));
                            }
                        }
                    }
                    EdgeKind::Cubic { ha, hb, .. } => {
                        let a = match self.nodes.get(e.a as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                        let b = match self.nodes.get(e.b as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                        let p1x = a.x + ha.x; let p1y = a.y + ha.y;
                        let p2x = b.x + hb.x; let p2y = b.y + hb.y;
                        let (d2, t) = cubic_distance_sq(x, y, a.x, a.y, p1x, p1y, p2x, p2y, b.x, b.y);
                        if d2 <= tol2 {
                            if best_edge.map_or(true, |(_, bd2, _)| d2 < bd2) {
                                best_edge = Some((i as u32, d2, t));
                            }
                        }
                    }
                }
            }
        }
        if let Some((id, d2, t)) = best_edge {
            let mut obj = Object::new();
            let _ = Reflect::set(&obj, &JsValue::from_str("kind"), &JsValue::from_str("edge"));
            let _ = Reflect::set(&obj, &JsValue::from_str("id"), &JsValue::from_f64(id as f64));
            let _ = Reflect::set(&obj, &JsValue::from_str("t"), &JsValue::from_f64(t as f64));
            let _ = Reflect::set(&obj, &JsValue::from_str("dist"), &JsValue::from_f64(d2.sqrt() as f64));
            return obj.into();
        }

        JsValue::UNDEFINED
    }

    // Serialize graph to JSON object: { nodes:[{id,x,y}], edges:[{id,a,b}] }
    pub fn to_json(&self) -> JsValue {
        #[derive(Serialize)]
        struct NodeSer { id: u32, x: f32, y: f32 }
        #[derive(Serialize)]
        #[serde(tag = "kind", rename_all = "lowercase")]
        enum EdgeSerKind { Line, Cubic { ha: Vec2, hb: Vec2, mode: HandleMode } }
        #[derive(Serialize)]
        struct EdgeSer { id: u32, a: u32, b: u32, #[serde(flatten)] kind: EdgeSerKind, stroke: Option<Color>, width: f32 }
        #[derive(Serialize)]
        struct FillSer { key: u32, filled: bool, color: Option<Color> }
        #[derive(Serialize)]
        struct DocSer { version: u32, nodes: Vec<NodeSer>, edges: Vec<EdgeSer>, fills: Vec<FillSer> }

        let mut nodes = Vec::new();
        for (i, n) in self.nodes.iter().enumerate() {
            if let Some(n) = n { nodes.push(NodeSer { id: i as u32, x: n.x, y: n.y }); }
        }
        let mut edges = Vec::new();
        for (i, e) in self.edges.iter().enumerate() {
            if let Some(e) = e {
                let kind = match e.kind {
                    EdgeKind::Line => EdgeSerKind::Line,
                    EdgeKind::Cubic { ha, hb, mode } => EdgeSerKind::Cubic { ha, hb, mode },
                };
                edges.push(EdgeSer { id: i as u32, a: e.a, b: e.b, kind, stroke: e.stroke, width: e.stroke_width });
            }
        }
        let mut fills: Vec<FillSer> = Vec::new();
        for (k, v) in self.fills.iter() { fills.push(FillSer { key: *k, filled: v.filled, color: v.color }); }
        serde_wasm_bindgen::to_value(&DocSer { version: 1, nodes, edges, fills }).unwrap_or(JsValue::NULL)
    }

    // Load from JSON object with the same shape. Returns true on success.
    pub fn from_json(&mut self, v: JsValue) -> bool {
        #[derive(Deserialize)]
        struct NodeDe { id: u32, x: f32, y: f32 }
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "lowercase")]
        enum EdgeDeKind { Line, Cubic { ha: Vec2, hb: Vec2, mode: Option<HandleMode> } }
        #[derive(Deserialize)]
        struct EdgeDe { id: u32, a: u32, b: u32, #[serde(flatten)] kind: Option<EdgeDeKind>, stroke: Option<Color>, width: Option<f32> }
        #[derive(Deserialize)]
        struct FillDe { key: u32, filled: bool, color: Option<Color> }
        #[derive(Deserialize)]
        struct DocDe { version: Option<u32>, nodes: Vec<NodeDe>, edges: Vec<EdgeDe>, fills: Option<Vec<FillDe>> }

        let parsed: Result<DocDe, _> = serde_wasm_bindgen::from_value(v);
        if let Ok(doc) = parsed {
            let max_node = doc.nodes.iter().map(|n| n.id).max().unwrap_or(0);
            let max_edge = doc.edges.iter().map(|e| e.id).max().unwrap_or(0);
            self.nodes = vec![None; (max_node as usize) + 1];
            self.edges = vec![None; (max_edge as usize) + 1];
            self.fills.clear();
            for n in doc.nodes { self.nodes[n.id as usize] = Some(Node { x: n.x, y: n.y }); }
            for e in doc.edges {
                let kind = match e.kind.unwrap_or(EdgeDeKind::Line) {
                    EdgeDeKind::Line => EdgeKind::Line,
                    EdgeDeKind::Cubic { ha, hb, mode } => EdgeKind::Cubic { ha, hb, mode: mode.unwrap_or(HandleMode::Free) },
                };
                self.edges[e.id as usize] = Some(Edge { a: e.a, b: e.b, kind, stroke: e.stroke, stroke_width: e.width.unwrap_or(2.0) });
            }
            if let Some(fills) = doc.fills { for f in fills { self.fills.insert(f.key, FillState { filled: f.filled, color: f.color }); } }
            self.geom_ver = self.geom_ver.wrapping_add(1);
            true
        } else {
            false
        }
    }

    // Clear the graph
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
        self.fills.clear();
        self.geom_ver = self.geom_ver.wrapping_add(1);
    }
}

fn seg_distance_sq(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> (f32, f32) {
    let vx = x2 - x1; let vy = y2 - y1;
    let wx = px - x1; let wy = py - y1;
    let vv = vx*vx + vy*vy;
    let mut t = if vv > 0.0 { (wx*vx + wy*vy) / vv } else { 0.0 };
    if t < 0.0 { t = 0.0; } else if t > 1.0 { t = 1.0; }
    let projx = x1 + t * vx; let projy = y1 + t * vy;
    let dx = px - projx; let dy = py - projy;
    (dx*dx + dy*dy, t)
}

fn cubic_point(t: f32, x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) -> (f32, f32) {
    let u = 1.0 - t;
    let tt = t*t; let uu = u*u;
    let uuu = uu*u; let ttt = tt*t;
    let x = uuu*x0 + 3.0*uu*t*x1 + 3.0*u*tt*x2 + ttt*x3;
    let y = uuu*y0 + 3.0*uu*t*y1 + 3.0*u*tt*y2 + ttt*y3;
    (x, y)
}

fn dist_point_to_seg_sq(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let (d2, _) = seg_distance_sq(px, py, x1, y1, x2, y2);
    d2
}

fn flatten_cubic(points: &mut Vec<Vec2>,
    x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32,
    tol: f32, depth: u32)
{
    // Flatness: max distance of control points to line (x0,y0)-(x3,y3)
    let d1 = dist_point_to_seg_sq(x1, y1, x0, y0, x3, y3);
    let d2 = dist_point_to_seg_sq(x2, y2, x0, y0, x3, y3);
    let tol2 = tol * tol;
    if d1.max(d2) <= tol2 || depth > 16 {
        points.push(Vec2 { x: x3, y: y3 });
        return;
    }
    // Subdivide at t=0.5 (de Casteljau)
    let x01 = 0.5*(x0 + x1); let y01 = 0.5*(y0 + y1);
    let x12 = 0.5*(x1 + x2); let y12 = 0.5*(y1 + y2);
    let x23 = 0.5*(x2 + x3); let y23 = 0.5*(y2 + y3);
    let x012 = 0.5*(x01 + x12); let y012 = 0.5*(y01 + y12);
    let x123 = 0.5*(x12 + x23); let y123 = 0.5*(y12 + y23);
    let x0123 = 0.5*(x012 + x123); let y0123 = 0.5*(y012 + y123);
    flatten_cubic(points, x0, y0, x01, y01, x012, y012, x0123, y0123, tol, depth+1);
    flatten_cubic(points, x0123, y0123, x123, y123, x23, y23, x3, y3, tol, depth+1);
}

fn cubic_distance_sq(px: f32, py: f32,
    x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) -> (f32, f32) {
    // Approximate by sampling
    let mut best_d2 = f32::INFINITY;
    let mut best_t = 0.0;
    let n = 32;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let (x, y) = cubic_point(t, x0,y0,x1,y1,x2,y2,x3,y3);
        let dx = px - x; let dy = py - y; let d2 = dx*dx + dy*dy;
        if d2 < best_d2 { best_d2 = d2; best_t = t; }
    }
    (best_d2, best_t)
}

#[derive(Clone)]
struct Region { key: u32, points: Vec<Vec2>, area: f32 }

impl Graph {
    fn compute_regions(&self) -> Vec<Region> {
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
            let mut cycle: Vec<usize> = Vec::new();
            let mut cycle_eids: Vec<u32> = Vec::new();
            let mut i_he = i_start;
            let mut guard = 0;
            loop {
                if used[i_he] { break; }
                used[i_he] = true;
                let u = half_from[i_he];
                let v = half_to[i_he];
                cycle.push(u);
                cycle_eids.push(half_eid[i_he]);
                let rev_angle = (verts[u].y - verts[v].y).atan2(verts[u].x - verts[v].x);
                let lst = &adj[v];
                if lst.is_empty() { break; }
                let mut j = 0usize;
                for k in 0..lst.len() {
                    if lst[k].1 > rev_angle { j = if k == 0 { lst.len()-1 } else { k-1 }; break; }
                    if k == lst.len()-1 { j = k; }
                }
                let w = lst[j].0;
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
            // Fallback: simple degree-2 cycles on the original graph
            let mut fallback = self.find_simple_cycles();
            if !fallback.is_empty() { return fallback; }
        }
        regions
    }
}

impl Graph {
    fn find_simple_cycles(&self) -> Vec<Region> {
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
            // next will be chosen from neighbors each step
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
                // Build polygon following actual edge geometry (lines or cubics)
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
                                match e.kind {
                                    EdgeKind::Line => {
                                        // push endpoint v
                                        if poly.is_empty() { poly.push(Vec2 { x: nu.x, y: nu.y }); }
                                        poly.push(Vec2 { x: nv.x, y: nv.y });
                                    }
                                    EdgeKind::Cubic { ha, hb, .. } => {
                                        // flatten along the curve from u->v in correct direction
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
                                }
                                added_any = true;
                                if edge_seq.last().copied() != Some(eid_idx as u32) { edge_seq.push(eid_idx as u32); }
                                break;
                            }
                        }
                    }
                    if !added_any {
                        // fallback straight segment
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

fn polygon_area(poly: &Vec<Vec2>) -> f32 {
    let mut a = 0.0;
    for i in 0..poly.len() {
        let j = (i + 1) % poly.len();
        a += poly[i].x * poly[j].y - poly[j].x * poly[i].y;
    }
    0.5 * a
}

fn polygon_centroid(poly: &Vec<Vec2>) -> (f32, f32) {
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
    let n = seq.len();
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

/// Optional: better error messages in the browser console
#[wasm_bindgen]
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

// === Cubic API ===
#[wasm_bindgen]
impl Graph {
    /// Convert an edge to cubic with absolute handle positions (world space)
    pub fn set_edge_cubic(&mut self, id: u32, p1x: f32, p1y: f32, p2x: f32, p2y: f32) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            let a = match self.nodes.get(edge.a as usize).and_then(|n| *n) { Some(n) => n, None => return false };
            let b = match self.nodes.get(edge.b as usize).and_then(|n| *n) { Some(n) => n, None => return false };
            let ha = Vec2 { x: p1x - a.x, y: p1y - a.y };
            let hb = Vec2 { x: p2x - b.x, y: p2y - b.y };
            edge.kind = EdgeKind::Cubic { ha, hb, mode: HandleMode::Free };
            self.geom_ver = self.geom_ver.wrapping_add(1);
            return true;
        }
        false
    }

    pub fn set_edge_line(&mut self, id: u32) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            edge.kind = EdgeKind::Line; self.geom_ver = self.geom_ver.wrapping_add(1); return true;
        }
        false
    }

    /// Set edge stroke style (color rgba 0-255, width in px)
    pub fn set_edge_style(&mut self, id: u32, r: u8, g: u8, b: u8, a: u8, width: f32) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            edge.stroke = Some(Color { r, g, b, a });
            edge.stroke_width = if width.is_finite() && width > 0.0 { width } else { 2.0 };
            return true;
        }
        false
    }

    /// Get edge stroke style as [r,g,b,a,width] or null if none
    pub fn get_edge_style(&self, id: u32) -> JsValue {
        if let Some(Some(edge)) = self.edges.get(id as usize) {
            if let Some(c) = edge.stroke {
                let arr = vec![c.r as f32, c.g as f32, c.b as f32, c.a as f32, edge.stroke_width];
                return serde_wasm_bindgen::to_value(&arr).unwrap_or(JsValue::NULL);
            }
        }
        JsValue::NULL
    }

    /// Get absolute handle positions as [x1,y1,x2,y2], or null if not cubic
    pub fn get_handles(&self, id: u32) -> JsValue {
        if let Some(Some(edge)) = self.edges.get(id as usize) {
            let a = match self.nodes.get(edge.a as usize).and_then(|n| *n) { Some(n) => n, None => return JsValue::NULL };
            let b = match self.nodes.get(edge.b as usize).and_then(|n| *n) { Some(n) => n, None => return JsValue::NULL };
            if let EdgeKind::Cubic { ha, hb, .. } = edge.kind {
                let arr = vec![a.x + ha.x, a.y + ha.y, b.x + hb.x, b.y + hb.y];
                return serde_wasm_bindgen::to_value(&arr).unwrap_or(JsValue::NULL);
            }
        }
        JsValue::NULL
    }

    /// Set absolute handle position for end 0(a) or 1(b); applies current mode constraints
    pub fn set_handle_pos(&mut self, id: u32, end: u8, x: f32, y: f32) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            let (mut ha, mut hb, mode);
            let a = match self.nodes.get(edge.a as usize).and_then(|n| *n) { Some(n) => n, None => return false };
            let b = match self.nodes.get(edge.b as usize).and_then(|n| *n) { Some(n) => n, None => return false };
            match edge.kind {
                EdgeKind::Cubic { ha: _ha, hb: _hb, mode: m } => { ha = _ha; hb = _hb; mode = m; }
                EdgeKind::Line => { return false; }
            }
            if end == 0 {
                ha = Vec2 { x: x - a.x, y: y - a.y };
                match mode {
                    HandleMode::Free => {}
                    HandleMode::Mirrored => { hb = Vec2 { x: -ha.x, y: -ha.y }; }
                    HandleMode::Aligned => {
                        let len = (hb.x*hb.x + hb.y*hb.y).sqrt();
                        let mut vx = -ha.x; let mut vy = -ha.y;
                        let vlen = (vx*vx + vy*vy).sqrt();
                        if vlen > 0.0 { vx /= vlen; vy /= vlen; }
                        hb = Vec2 { x: vx*len, y: vy*len };
                    }
                }
            } else {
                hb = Vec2 { x: x - b.x, y: y - b.y };
                match mode {
                    HandleMode::Free => {}
                    HandleMode::Mirrored => { ha = Vec2 { x: -hb.x, y: -hb.y }; }
                    HandleMode::Aligned => {
                        let len = (ha.x*ha.x + ha.y*ha.y).sqrt();
                        let mut vx = -hb.x; let mut vy = -hb.y;
                        let vlen = (vx*vx + vy*vy).sqrt();
                        if vlen > 0.0 { vx /= vlen; vy /= vlen; }
                        ha = Vec2 { x: vx*len, y: vy*len };
                    }
                }
            }
            edge.kind = EdgeKind::Cubic { ha, hb, mode };
            self.geom_ver = self.geom_ver.wrapping_add(1);
            return true;
        }
        false
    }

    pub fn set_handle_mode(&mut self, id: u32, mode: u8) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            let (ha, hb) = match edge.kind { EdgeKind::Cubic { ha, hb, .. } => (ha, hb), EdgeKind::Line => return false };
            let new_mode = match mode { 1 => HandleMode::Mirrored, 2 => HandleMode::Aligned, _ => HandleMode::Free };
            edge.kind = EdgeKind::Cubic { ha, hb, mode: new_mode };
            self.geom_ver = self.geom_ver.wrapping_add(1);
            return true;
        }
        false
    }

    /// Bend an edge by dragging a point on the curve to target (tx,ty).
    /// t is the parameter (0..1) of the picked point. stiffness >= 0 weights how much P2 moves vs P1 (1.0 = equal).
    pub fn bend_edge_to(&mut self, id: u32, t: f32, tx: f32, ty: f32, stiffness: f32) -> bool {
        if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            let a = match self.nodes.get(edge.a as usize).and_then(|n| *n) { Some(n) => n, None => return false };
            let b = match self.nodes.get(edge.b as usize).and_then(|n| *n) { Some(n) => n, None => return false };

            // Ensure cubic
            if let EdgeKind::Line = edge.kind {
                // initialize default handles ~30% along the line
                let dx = b.x - a.x; let dy = b.y - a.y;
                let len = (dx*dx + dy*dy).sqrt();
                let k = if len > 0.0 { 0.3 * len } else { 0.0 };
                let ux = if len > 0.0 { dx/len } else { 1.0 };
                let uy = if len > 0.0 { dy/len } else { 0.0 };
                let ha = Vec2 { x: ux * k, y: uy * k };
                let hb = Vec2 { x: -ux * k, y: -uy * k };
                edge.kind = EdgeKind::Cubic { ha, hb, mode: HandleMode::Free };
            }

            if let EdgeKind::Cubic { mut ha, mut hb, mode } = edge.kind {
                // Absolute handle points
                let p1x = a.x + ha.x; let p1y = a.y + ha.y;
                let p2x = b.x + hb.x; let p2y = b.y + hb.y;
                let u = 1.0 - t;
                let c1 = 3.0 * u * u * t;
                let c2 = 3.0 * u * t * t;
                // Current curve point from control points
                let c0x = u*u*u * a.x + t*t*t * b.x;
                let c0y = u*u*u * a.y + t*t*t * b.y;
                let curx = c0x + c1 * p1x + c2 * p2x;
                let cury = c0y + c1 * p1y + c2 * p2y;
                let dx = tx - curx; let dy = ty - cury;

                // Least-squares minimal-change update under constraint c1*dP1 + c2*dP2 = d
                let lam1 = 1.0f32; let lam2 = if stiffness > 0.0 { stiffness } else { 1.0 };
                let s = (c1*c1)/lam1 + (c2*c2)/lam2;
                if s > 1e-6 {
                    let k1 = (c1/lam1) / s;
                    let k2 = (c2/lam2) / s;
                    let d1x = k1 * dx; let d1y = k1 * dy;
                    let d2x = k2 * dx; let d2y = k2 * dy;
                    let mut new_p1x = p1x + d1x; let mut new_p1y = p1y + d1y;
                    let mut new_p2x = p2x + d2x; let mut new_p2y = p2y + d2y;

                    // Enforce handle modes
                    match mode {
                        HandleMode::Free => {}
                        HandleMode::Mirrored => {
                            let v1x = new_p1x - a.x; let v1y = new_p1y - a.y;
                            new_p2x = b.x - v1x; new_p2y = b.y - v1y;
                        }
                        HandleMode::Aligned => {
                            // Keep directions opposite; preserve lengths
                            let v1x = new_p1x - a.x; let v1y = new_p1y - a.y;
                            let len2 = ((p2x - b.x).hypot(p2y - b.y)).max(1e-6);
                            let mut vx = -v1x; let mut vy = -v1y;
                            let vlen = (vx*vx + vy*vy).sqrt().max(1e-6);
                            vx /= vlen; vy /= vlen;
                            new_p2x = b.x + vx * len2; new_p2y = b.y + vy * len2;
                        }
                    }

                    ha = Vec2 { x: new_p1x - a.x, y: new_p1y - a.y };
                    hb = Vec2 { x: new_p2x - b.x, y: new_p2y - b.y };
                    edge.kind = EdgeKind::Cubic { ha, hb, mode };
                    self.geom_ver = self.geom_ver.wrapping_add(1);
                    return true;
                }
            }
        }
        false
    }

    /// Set curve flattening tolerance in pixels (default 0.25). Lower values hug edges more tightly.
    pub fn set_flatten_tolerance(&mut self, tol: f32) {
        let t = if tol.is_finite() && tol > 0.01 { tol } else { 0.01 };
        self.flatten_tol = t.min(2.0);
    }

    /// Uniformly transform all geometry: scale, then translate.
    /// - Scales node positions by `s` and handle offsets by `s` to preserve shape
    /// - Translates node positions by (tx, ty)
    /// - Optionally scales stroke widths when `scale_stroke` is true
    pub fn transform_all(&mut self, s: f32, tx: f32, ty: f32, scale_stroke: bool) {
        let s = if s.is_finite() { s } else { 1.0 };
        for n in self.nodes.iter_mut() {
            if let Some(n) = n.as_mut() {
                n.x = n.x * s + tx;
                n.y = n.y * s + ty;
            }
        }
        for e in self.edges.iter_mut() {
            if let Some(edge) = e.as_mut() {
                if let EdgeKind::Cubic { ha, hb, mode } = edge.kind {
                    edge.kind = EdgeKind::Cubic { ha: Vec2 { x: ha.x * s, y: ha.y * s }, hb: Vec2 { x: hb.x * s, y: hb.y * s }, mode };
                }
                if scale_stroke {
                    if edge.stroke_width.is_finite() { edge.stroke_width *= s.max(0.0); }
                }
            }
        }
        self.geom_ver = self.geom_ver.wrapping_add(1);
    }

    /// Translate a set of nodes by (dx, dy). Returns number of nodes moved.
    pub fn translate_nodes(&mut self, node_ids: &Uint32Array, dx: f32, dy: f32) -> u32 {
        let ids: Vec<u32> = node_ids.to_vec();
        if ids.is_empty() { return 0; }
        let set: HashSet<u32> = ids.into_iter().collect();
        let mut moved = 0u32;
        for (i, n) in self.nodes.iter_mut().enumerate() {
            if set.contains(&(i as u32)) {
                if let Some(nn) = n.as_mut() {
                    nn.x += dx; nn.y += dy; moved += 1;
                }
            }
        }
        if moved > 0 { self.geom_ver = self.geom_ver.wrapping_add(1); }
        moved
    }

    /// Translate all nodes incident to the given edges by (dx, dy).
    /// When `split_shared` is true, nodes that are shared with edges not in `edge_ids`
    /// are duplicated and the grouped edges are rewired to the duplicates before moving.
    /// Returns number of nodes moved.
    pub fn translate_edges(&mut self, edge_ids: &Uint32Array, dx: f32, dy: f32, split_shared: bool) -> u32 {
        let eids_vec: Vec<u32> = edge_ids.to_vec();
        if eids_vec.is_empty() { return 0; }
        let edge_set: HashSet<u32> = eids_vec.iter().copied().collect();

        // Gather nodes touched by the edge set
        let mut nodes_in_group: HashSet<u32> = HashSet::new();
        for (i, e) in self.edges.iter().enumerate() {
            let id = i as u32;
            if !edge_set.contains(&id) { continue; }
            if let Some(edge) = e.as_ref() {
                nodes_in_group.insert(edge.a);
                nodes_in_group.insert(edge.b);
            }
        }
        if nodes_in_group.is_empty() { return 0; }

        // If requested, split nodes that are shared with outside edges
        let mut split_map: HashMap<u32, u32> = HashMap::new(); // old_node -> new_node
        if split_shared {
            // Determine which nodes are incident to any edge not in the set
            let mut shared: HashSet<u32> = HashSet::new();
            for (i, e) in self.edges.iter().enumerate() {
                let id = i as u32;
                if let Some(edge) = e.as_ref() {
                    if nodes_in_group.contains(&edge.a) && !edge_set.contains(&id) { shared.insert(edge.a); }
                    if nodes_in_group.contains(&edge.b) && !edge_set.contains(&id) { shared.insert(edge.b); }
                }
            }
            // Create duplicates for shared nodes
            for n_id in shared.iter().copied() {
                if let Some(Some(n)) = self.nodes.get(n_id as usize) {
                    let new_id = self.nodes.len() as u32;
                    self.nodes.push(Some(Node { x: n.x, y: n.y }));
                    split_map.insert(n_id, new_id);
                }
            }
            // Rewire grouped edges to use duplicated nodes where applicable
            if !split_map.is_empty() {
                for (i, e) in self.edges.iter_mut().enumerate() {
                    let id = i as u32;
                    if !edge_set.contains(&id) { continue; }
                    if let Some(edge) = e.as_mut() {
                        if let Some(&na) = split_map.get(&edge.a) { edge.a = na; }
                        if let Some(&nb) = split_map.get(&edge.b) { edge.b = nb; }
                    }
                }
                // Update nodes_in_group to include duplicates and remove originals where rewired
                for (old, new_) in split_map.iter() {
                    if nodes_in_group.contains(old) {
                        nodes_in_group.insert(*new_);
                    }
                }
            }
        }

        // Recompute the final set of nodes to move: endpoints of edges in the set after rewiring
        let mut nodes_to_move: HashSet<u32> = HashSet::new();
        for (i, e) in self.edges.iter().enumerate() {
            let id = i as u32;
            if !edge_set.contains(&id) { continue; }
            if let Some(edge) = e.as_ref() {
                nodes_to_move.insert(edge.a);
                nodes_to_move.insert(edge.b);
            }
        }

        // Apply translation
        let mut moved = 0u32;
        for nid in nodes_to_move.iter() {
            if let Some(Some(n)) = self.nodes.get_mut(*nid as usize) {
                n.x += dx; n.y += dy; moved += 1;
            }
        }
        if moved > 0 { self.geom_ver = self.geom_ver.wrapping_add(1); }
        moved
    }
}

#[wasm_bindgen]
impl Graph {
    /// Return detected regions as JS array of { key, area, filled, points:[x,y,...] }
    pub fn get_regions(&mut self) -> JsValue {
        #[derive(Serialize)]
        struct RegionSer { key: u32, area: f32, filled: bool, color: Option<[u8;4]>, points: Vec<f32> }
        let regions = self.compute_regions();
        // If geometry changed, remap fills by nearest centroid from previous regions
        if self.last_geom_ver != self.geom_ver {
            let mut new_fills: HashMap<u32, FillState> = HashMap::new();
            // Build new centroids
            let mut new_prev: Vec<(u32, f32, f32)> = Vec::with_capacity(regions.len());
            for r in &regions {
                let (cx, cy) = polygon_centroid(&r.points);
                new_prev.push((r.key, cx, cy));
            }
            for (k_new, cx, cy) in &new_prev {
                // find nearest previous centroid
                let mut best: Option<(u32, f32)> = None; // (old_key, dist2)
                for (k_old, ox, oy) in &self.prev_regions {
                    let dx = cx - ox; let dy = cy - oy; let d2 = dx*dx + dy*dy;
                    if best.map_or(true, |(_, bd2)| d2 < bd2) { best = Some((*k_old, d2)); }
                }
                let st = if let Some((old_key, d2)) = best {
                    if d2 < 400.0 { // within ~20px
                        self.fills.get(&old_key).copied().unwrap_or(FillState { filled: true, color: None })
                    } else { self.fills.get(k_new).copied().unwrap_or(FillState { filled: true, color: None }) }
                } else { self.fills.get(k_new).copied().unwrap_or(FillState { filled: true, color: None }) };
                new_fills.insert(*k_new, st);
            }
            self.fills = new_fills;
            self.prev_regions = new_prev;
            self.last_geom_ver = self.geom_ver;
        }
        let out: Vec<RegionSer> = regions.into_iter().map(|r| {
            let st = self.fills.get(&r.key).copied().unwrap_or(FillState { filled: true, color: None });
            let color = st.color.map(|c| [c.r, c.g, c.b, c.a]);
            let mut pts: Vec<f32> = Vec::with_capacity(r.points.len() * 2);
            for p in &r.points { pts.push(p.x); pts.push(p.y); }
            RegionSer { key: r.key, area: r.area, filled: st.filled, color, points: pts }
        }).collect();
        serde_wasm_bindgen::to_value(&out).unwrap_or(JsValue::NULL)
    }

    /// Toggle fill for a region identified by its key; returns new state
    pub fn toggle_region(&mut self, key: u32) -> bool {
        let current = self.fills.get(&key).copied().unwrap_or(FillState { filled: true, color: None });
        let next = !current.filled;
        self.fills.insert(key, FillState { filled: next, color: current.color });
        next
    }

    /// Explicitly set region fill on/off
    pub fn set_region_fill(&mut self, key: u32, filled: bool) {
        let color = self.fills.get(&key).and_then(|st| st.color);
        self.fills.insert(key, FillState { filled, color });
    }

    /// Set region fill color as RGBA 0-255
    pub fn set_region_color(&mut self, key: u32, r: u8, g: u8, b: u8, a: u8) {
        let filled = self.fills.get(&key).map(|st| st.filled).unwrap_or(true);
        self.fills.insert(key, FillState { filled, color: Some(Color { r, g, b, a }) });
    }

    // === SVG import/export (MVP) ===
    /// Append geometry from an SVG path `d` (supports M/m, L/l, C/c, Z/z). Returns number of edges added.
    pub fn add_svg_path(&mut self, d: &str) -> u32 {
        let mut i = 0usize;
        let bytes = d.as_bytes();
        let mut cur = (0.0f32, 0.0f32);
        let mut start_sub = (0.0f32, 0.0f32);
        let mut last_cmd = b'M';
        let mut edges_added = 0u32;

        // node cache by quantized coord for sharing
        let mut node_cache: HashMap<(i32, i32), u32> = HashMap::new();
        let q = |x: f32, y: f32| ((x * 100.0).round() as i32, (y * 100.0).round() as i32);
        let mut get_node = |x: f32, y: f32, this: &mut Graph| -> u32 {
            let key = q(x, y);
            if let Some(&id) = node_cache.get(&key) { return id; }
            let id = this.add_node(x, y);
            node_cache.insert(key, id);
            id
        };

        fn skip_ws(bytes: &[u8], i: &mut usize) {
            while *i < bytes.len() {
                let c = bytes[*i];
                if c == b' ' || c == b'\n' || c == b'\t' || c == b',' { *i += 1; } else { break; }
            }
        }
        fn parse_num(bytes: &[u8], i: &mut usize) -> Option<f32> {
            skip_ws(bytes, i);
            let start = *i;
            let mut had = false;
            while *i < bytes.len() {
                let c = bytes[*i];
                if (c as char).is_ascii_digit() || c == b'.' || c == b'-' || c == b'+' || c == b'e' || c == b'E' {
                    had = true; *i += 1;
                } else { break; }
            }
            if !had { return None; }
            let s = std::str::from_utf8(&bytes[start..*i]).ok()?;
            s.parse::<f32>().ok()
        }

        while i < bytes.len() {
            skip_ws(bytes, &mut i);
            if i >= bytes.len() { break; }
            let c = bytes[i];
            let is_cmd = matches!(c, b'M'|b'm'|b'L'|b'l'|b'C'|b'c'|b'Z'|b'z');
            let cmd = if is_cmd { i += 1; c } else { last_cmd };
            last_cmd = cmd;

            match cmd {
                b'M'|b'm' => {
                    let mut x = parse_num(bytes, &mut i).unwrap_or(cur.0);
                    let mut y = parse_num(bytes, &mut i).unwrap_or(cur.1);
                    if cmd == b'm' { x += cur.0; y += cur.1; }
                    cur = (x, y); start_sub = cur;
                    // Subsequent coordinate pairs are implicit L
                    loop {
                        skip_ws(bytes, &mut i);
                        if i >= bytes.len() { break; }
                        let peek = bytes[i];
                        if matches!(peek, b'M'|b'm'|b'L'|b'l'|b'C'|b'c'|b'Z'|b'z') { break; }
                        let mut nx = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        let mut ny = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        if cmd == b'm' { nx += cur.0; ny += cur.1; }
                        let a = get_node(cur.0, cur.1, self);
                        let b = get_node(nx, ny, self);
                        if let Some(_eid) = self.add_edge(a, b) { edges_added += 1; }
                        cur = (nx, ny);
                    }
                }
                b'L'|b'l' => {
                    loop {
                        let mut x = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        let mut y = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        if cmd == b'l' { x += cur.0; y += cur.1; }
                        let a = get_node(cur.0, cur.1, self);
                        let b = get_node(x, y, self);
                        if let Some(_eid) = self.add_edge(a, b) { edges_added += 1; }
                        cur = (x, y);
                        skip_ws(bytes, &mut i);
                        if i >= bytes.len() { break; }
                        let peek = bytes[i];
                        if matches!(peek, b'M'|b'm'|b'L'|b'l'|b'C'|b'c'|b'Z'|b'z') { break; }
                    }
                }
                b'C'|b'c' => {
                    loop {
                        let mut x1 = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        let mut y1 = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        let mut x2 = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        let mut y2 = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        let mut x = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        let mut y = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        if cmd == b'c' { x1 += cur.0; y1 += cur.1; x2 += cur.0; y2 += cur.1; x += cur.0; y += cur.1; }
                        let a = get_node(cur.0, cur.1, self);
                        let b = get_node(x, y, self);
                        if let Some(eid) = self.add_edge(a, b) {
                            self.set_edge_cubic(eid, x1, y1, x2, y2);
                            edges_added += 1;
                        }
                        cur = (x, y);
                        skip_ws(bytes, &mut i);
                        if i >= bytes.len() { break; }
                        let peek = bytes[i];
                        if matches!(peek, b'M'|b'm'|b'L'|b'l'|b'C'|b'c'|b'Z'|b'z') { break; }
                    }
                }
                b'Z'|b'z' => {
                    // close current subpath
                    let a = get_node(cur.0, cur.1, self);
                    let b = get_node(start_sub.0, start_sub.1, self);
                    if a != b { if let Some(_eid) = self.add_edge(a, b) { edges_added += 1; } }
                    cur = start_sub;
                }
                _ => { /* ignore */ }
            }
        }
        if edges_added > 0 { self.geom_ver = self.geom_ver.wrapping_add(1); }
        edges_added
    }

    /// Append geometry from an SVG path with a stroke style applied to created edges.
    /// Returns number of edges added.
    pub fn add_svg_path_with_style(&mut self, d: &str, r: u8, g: u8, b: u8, a: u8, width: f32) -> u32 {
        let mut i = 0usize;
        let bytes = d.as_bytes();
        let mut cur = (0.0f32, 0.0f32);
        let mut start_sub = (0.0f32, 0.0f32);
        let mut last_cmd = b'M';
        let mut edges_added = 0u32;

        let mut node_cache: HashMap<(i32, i32), u32> = HashMap::new();
        let q = |x: f32, y: f32| ((x * 100.0).round() as i32, (y * 100.0).round() as i32);
        let mut get_node = |x: f32, y: f32, this: &mut Graph| -> u32 {
            let key = q(x, y);
            if let Some(&id) = node_cache.get(&key) { return id; }
            let id = this.add_node(x, y);
            node_cache.insert(key, id);
            id
        };

        fn skip_ws(bytes: &[u8], i: &mut usize) {
            while *i < bytes.len() {
                let c = bytes[*i];
                if c == b' ' || c == b'\n' || c == b'\t' || c == b',' { *i += 1; } else { break; }
            }
        }
        fn parse_num(bytes: &[u8], i: &mut usize) -> Option<f32> {
            skip_ws(bytes, i);
            let start = *i;
            let mut had = false;
            while *i < bytes.len() {
                let c = bytes[*i];
                if (c as char).is_ascii_digit() || c == b'.' || c == b'-' || c == b'+' || c == b'e' || c == b'E' {
                    had = true; *i += 1;
                } else { break; }
            }
            if !had { return None; }
            let s = std::str::from_utf8(&bytes[start..*i]).ok()?;
            s.parse::<f32>().ok()
        }

        while i < bytes.len() {
            skip_ws(bytes, &mut i);
            if i >= bytes.len() { break; }
            let c = bytes[i];
            let is_cmd = matches!(c, b'M'|b'm'|b'L'|b'l'|b'C'|b'c'|b'Z'|b'z');
            let cmd = if is_cmd { i += 1; c } else { last_cmd };
            last_cmd = cmd;

            match cmd {
                b'M'|b'm' => {
                    let mut x = parse_num(bytes, &mut i).unwrap_or(cur.0);
                    let mut y = parse_num(bytes, &mut i).unwrap_or(cur.1);
                    if cmd == b'm' { x += cur.0; y += cur.1; }
                    cur = (x, y); start_sub = cur;
                    loop {
                        skip_ws(bytes, &mut i);
                        if i >= bytes.len() { break; }
                        let peek = bytes[i];
                        if matches!(peek, b'M'|b'm'|b'L'|b'l'|b'C'|b'c'|b'Z'|b'z') { break; }
                        let mut nx = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        let mut ny = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        if cmd == b'm' { nx += cur.0; ny += cur.1; }
                        let a_id = get_node(cur.0, cur.1, self);
                        let b_id = get_node(nx, ny, self);
                        if let Some(eid) = self.add_edge(a_id, b_id) { self.set_edge_style(eid, r, g, b, a, width); edges_added += 1; }
                        cur = (nx, ny);
                    }
                }
                b'L'|b'l' => {
                    loop {
                        let mut x = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        let mut y = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        if cmd == b'l' { x += cur.0; y += cur.1; }
                        let a_id = get_node(cur.0, cur.1, self);
                        let b_id = get_node(x, y, self);
                        if let Some(eid) = self.add_edge(a_id, b_id) { self.set_edge_style(eid, r, g, b, a, width); edges_added += 1; }
                        cur = (x, y);
                        skip_ws(bytes, &mut i);
                        if i >= bytes.len() { break; }
                        let peek = bytes[i];
                        if matches!(peek, b'M'|b'm'|b'L'|b'l'|b'C'|b'c'|b'Z'|b'z') { break; }
                    }
                }
                b'C'|b'c' => {
                    loop {
                        let mut x1 = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        let mut y1 = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        let mut x2 = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        let mut y2 = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        let mut x = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        let mut y = match parse_num(bytes, &mut i) { Some(v) => v, None => break };
                        if cmd == b'c' { x1 += cur.0; y1 += cur.1; x2 += cur.0; y2 += cur.1; x += cur.0; y += cur.1; }
                        let a_id = get_node(cur.0, cur.1, self);
                        let b_id = get_node(x, y, self);
                        if let Some(eid) = self.add_edge(a_id, b_id) {
                            self.set_edge_cubic(eid, x1, y1, x2, y2);
                            self.set_edge_style(eid, r, g, b, a, width);
                            edges_added += 1;
                        }
                        cur = (x, y);
                        skip_ws(bytes, &mut i);
                        if i >= bytes.len() { break; }
                        let peek = bytes[i];
                        if matches!(peek, b'M'|b'm'|b'L'|b'l'|b'C'|b'c'|b'Z'|b'z') { break; }
                    }
                }
                b'Z'|b'z' => {
                    let a_id = get_node(cur.0, cur.1, self);
                    let b_id = get_node(start_sub.0, start_sub.1, self);
                    if a_id != b_id { if let Some(eid) = self.add_edge(a_id, b_id) { self.set_edge_style(eid, r, g, b, a, width); edges_added += 1; } }
                    cur = start_sub;
                }
                _ => {}
            }
        }
        if edges_added > 0 { self.geom_ver = self.geom_ver.wrapping_add(1); }
        edges_added
    }

    /// Export edges as independent SVG path fragments (M/L or M/C). Returns JS array of strings.
    pub fn to_svg_paths(&self) -> JsValue {
        let mut paths: Vec<String> = Vec::new();
        for e in self.edges.iter() {
            if let Some(edge) = e {
                let a = match self.nodes.get(edge.a as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                let b = match self.nodes.get(edge.b as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                match edge.kind {
                    EdgeKind::Line => {
                        paths.push(format!("M {} {} L {} {}", a.x, a.y, b.x, b.y));
                    }
                    EdgeKind::Cubic { ha, hb, .. } => {
                        let p1x = a.x + ha.x; let p1y = a.y + ha.y;
                        let p2x = b.x + hb.x; let p2y = b.y + hb.y;
                        paths.push(format!("M {} {} C {} {}, {} {}, {} {}", a.x, a.y, p1x, p1y, p2x, p2y, b.x, b.y));
                    }
                }
            }
        }
        serde_wasm_bindgen::to_value(&paths).unwrap_or(JsValue::NULL)
    }
}
