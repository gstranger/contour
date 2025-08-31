use wasm_bindgen::prelude::*;
use js_sys::{Uint32Array, Float32Array};
use crate::Graph;
type JsValue = wasm_bindgen::JsValue;
use crate::error;

#[wasm_bindgen]
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
impl Graph {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Graph { crate::Graph::rs_new() }
    pub fn geom_version(&self) -> u64 { self.rs_geom_version() }

    // Nodes/Edges basic
    pub fn add_node(&mut self, x: f32, y: f32) -> u32 { self.inner.add_node(x, y) }
    pub fn add_node_res(&mut self, x: f32, y: f32) -> JsValue {
        if !x.is_finite() { return error::non_finite("x"); }
        if !y.is_finite() { return error::non_finite("y"); }
        error::ok(JsValue::from_f64(self.inner.add_node(x, y) as f64))
    }
    pub fn move_node(&mut self, id: u32, x: f32, y: f32) -> bool { self.inner.move_node(id, x, y) }
    pub fn move_node_res(&mut self, id: u32, x: f32, y: f32) -> JsValue {
        if !x.is_finite() { return error::non_finite("x"); }
        if !y.is_finite() { return error::non_finite("y"); }
        if self.inner.get_node(id).is_none() { return error::invalid_id("node", id); }
        let ok = self.inner.move_node(id, x, y);
        error::ok(JsValue::from_bool(ok))
    }
    pub fn get_node(&self, id: u32) -> JsValue { if let Some((x,y))=self.inner.get_node(id) { serde_wasm_bindgen::to_value(&vec![x,y]).unwrap() } else { JsValue::NULL } }
    pub fn get_node_res(&self, id: u32) -> JsValue { if let Some((x,y))=self.inner.get_node(id) { error::ok(serde_wasm_bindgen::to_value(&vec![x,y]).unwrap()) } else { error::invalid_id("node", id) } }
    pub fn remove_node(&mut self, id: u32) -> bool { self.inner.remove_node(id) }
    pub fn remove_node_res(&mut self, id: u32) -> JsValue { if self.inner.get_node(id).is_none() { return error::invalid_id("node", id); } error::ok(JsValue::from_bool(self.inner.remove_node(id))) }
    pub fn node_count(&self) -> u32 { self.inner.node_count() }
    pub fn add_edge(&mut self, a: u32, b: u32) -> Option<u32> { self.inner.add_edge(a, b) }
    pub fn add_edge_res(&mut self, a: u32, b: u32) -> JsValue {
        if self.inner.get_node(a).is_none() { return error::invalid_id("node", a); }
        if self.inner.get_node(b).is_none() { return error::invalid_id("node", b); }
        if a==b { return error::err("invalid_edge", "edge endpoints cannot be the same node", None); }
        match self.inner.add_edge(a, b) { Some(eid)=> error::ok(JsValue::from_f64(eid as f64)), None=> error::err("invalid_edge", "failed to add edge", None) }
    }
    pub fn remove_edge(&mut self, id: u32) -> bool { self.inner.remove_edge(id) }
    pub fn remove_edge_res(&mut self, id: u32) -> JsValue { if !edge_exists(&self.inner, id) { return error::invalid_id("edge", id); } error::ok(JsValue::from_bool(self.inner.remove_edge(id))) }
    pub fn edge_count(&self) -> u32 { self.inner.edge_count() }

    // Typed arrays getters
    pub fn get_node_data(&self) -> JsValue {
        let (ids, pos) = self.inner.get_node_arrays();
        let ids_arr = crate::interop::arr_u32(&ids);
        let pos_arr = crate::interop::arr_f32(&pos);
        let obj = crate::interop::new_obj();
        crate::interop::set_kv(&obj, "ids", &ids_arr.into());
        crate::interop::set_kv(&obj, "positions", &pos_arr.into());
        obj.into()
    }
    pub fn get_edge_data(&self) -> JsValue {
        let ea = self.inner.get_edge_arrays();
        let obj = crate::interop::new_obj();
        crate::interop::set_kv(&obj, "ids", &crate::interop::arr_u32(&ea.ids).into());
        crate::interop::set_kv(&obj, "endpoints", &crate::interop::arr_u32(&ea.endpoints).into());
        crate::interop::set_kv(&obj, "kinds", &crate::interop::arr_u8(&ea.kinds).into());
        crate::interop::set_kv(&obj, "stroke_rgba", &crate::interop::arr_u8(&ea.stroke_rgba).into());
        crate::interop::set_kv(&obj, "stroke_widths", &crate::interop::arr_f32(&ea.stroke_widths).into());
        obj.into()
    }

    // Picking + JSON + SVG
    pub fn pick(&self, x: f32, y: f32, tol: f32) -> JsValue {
        if let Some(p) = self.inner.pick(x, y, tol) {
            // Flatten to { kind: 'node'|'edge'|'handle', ... }
            let obj = crate::interop::new_obj();
            match p {
                contour::Pick::Node { id, dist } => {
                    crate::interop::set_kv(&obj, "kind", &JsValue::from_str("node"));
                    crate::interop::set_kv(&obj, "id", &JsValue::from_f64(id as f64));
                    crate::interop::set_kv(&obj, "dist", &JsValue::from_f64(dist as f64));
                }
                contour::Pick::Edge { id, t, dist } => {
                    crate::interop::set_kv(&obj, "kind", &JsValue::from_str("edge"));
                    crate::interop::set_kv(&obj, "id", &JsValue::from_f64(id as f64));
                    crate::interop::set_kv(&obj, "t", &JsValue::from_f64(t as f64));
                    crate::interop::set_kv(&obj, "dist", &JsValue::from_f64(dist as f64));
                }
                contour::Pick::Handle { edge, end, dist } => {
                    crate::interop::set_kv(&obj, "kind", &JsValue::from_str("handle"));
                    crate::interop::set_kv(&obj, "edge", &JsValue::from_f64(edge as f64));
                    crate::interop::set_kv(&obj, "end", &JsValue::from_f64(end as f64));
                    crate::interop::set_kv(&obj, "dist", &JsValue::from_f64(dist as f64));
                }
            }
            obj.into()
        } else {
            JsValue::NULL
        }
    }
    pub fn pick_res(&self, x: f32, y: f32, tol: f32) -> JsValue {
        if !x.is_finite() { return error::non_finite("x"); }
        if !y.is_finite() { return error::non_finite("y"); }
        if !tol.is_finite() { return error::non_finite("tol"); }
        if tol < 0.0 { return error::out_of_range("tol", 0.0, f32::INFINITY, tol); }
        let v = self.pick(x,y,tol);
        if v.is_null() { error::ok(JsValue::NULL) } else { error::ok(v) }
    }
    pub fn to_json(&self) -> JsValue { serde_wasm_bindgen::to_value(&self.inner.to_json_value()).unwrap() }
    pub fn from_json(&mut self, v: JsValue) -> bool { match serde_wasm_bindgen::from_value::<serde_json::Value>(v) { Ok(val)=> self.inner.from_json_value(val), Err(_)=> false } }
    pub fn from_json_res(&mut self, v: JsValue) -> JsValue {
        match serde_wasm_bindgen::from_value::<serde_json::Value>(v) {
            Ok(val) => match self.inner.from_json_value_strict(val) {
                Ok(ok) => error::ok(JsValue::from_bool(ok)),
                Err((code,msg)) => error::err(code, msg, None)
            },
            Err(e) => error::err("json_parse", format!("{}", e), None)
        }
    }
    pub fn clear(&mut self) { self.inner.clear(); }
    pub fn add_svg_path(&mut self, d: &str) -> u32 { self.inner.add_svg_path(d, None) }
    pub fn add_svg_path_with_style(&mut self, d: &str, r: u8, g: u8, b: u8, a: u8, width: f32) -> u32 { self.inner.add_svg_path(d, Some((r,g,b,a,width))) }
    pub fn to_svg_paths(&self) -> JsValue { serde_wasm_bindgen::to_value(&self.inner.to_svg_paths()).unwrap() }
    pub fn add_svg_path_res(&mut self, d: &str) -> JsValue {
        let before = self.inner.geom_version();
        let added = self.inner.add_svg_path(d, None);
        if added == 0 { return error::err("svg_parse", "no edges parsed from path", None); }
        // geom_version should have advanced if edges were added
        let _ = before; // silence unused if optimized
        error::ok(JsValue::from_f64(added as f64))
    }
    pub fn to_svg_paths_res(&self) -> JsValue { error::ok(self.to_svg_paths()) }

    // Regions + fill
    pub fn get_regions(&mut self) -> JsValue { serde_wasm_bindgen::to_value(&self.inner.get_regions()).unwrap() }
    pub fn get_regions_res(&mut self) -> JsValue { error::ok(self.get_regions()) }
    pub fn toggle_region(&mut self, key: u32) -> bool { self.inner.toggle_region(key) }
    pub fn toggle_region_res(&mut self, key: u32) -> JsValue {
        if !region_exists(&mut self.inner, key) { return error::invalid_id("region", key); }
        error::ok(JsValue::from_bool(self.inner.toggle_region(key)))
    }
    pub fn set_region_fill(&mut self, key: u32, filled: bool) { self.inner.set_region_fill(key, filled) }
    pub fn set_region_fill_res(&mut self, key: u32, filled: bool) -> JsValue {
        if !region_exists(&mut self.inner, key) { return error::invalid_id("region", key); }
        self.inner.set_region_fill(key, filled); error::ok(JsValue::from_bool(true))
    }
    pub fn set_region_color(&mut self, key: u32, r: u8, g: u8, b: u8, a: u8) { self.inner.set_region_color(key, r, g, b, a) }
    pub fn set_region_color_res(&mut self, key: u32, r: u8, g: u8, b: u8, a: u8) -> JsValue {
        if !region_exists(&mut self.inner, key) { return error::invalid_id("region", key); }
        self.inner.set_region_color(key, r,g,b,a); error::ok(JsValue::from_bool(true))
    }
    pub fn set_flatten_tolerance(&mut self, tol: f32) { self.inner.set_flatten_tolerance(tol) }
    pub fn set_flatten_tolerance_res(&mut self, tol: f32) -> JsValue {
        if !tol.is_finite() { return error::non_finite("tol"); }
        if tol <= 0.0 || tol > 10.0 { return error::out_of_range("tol", 0.01, 10.0, tol); }
        self.inner.set_flatten_tolerance(tol);
        error::ok(JsValue::from_bool(true))
    }

    // Styling/handles
    pub fn set_edge_style(&mut self, id: u32, r: u8, g: u8, b: u8, a: u8, width: f32) -> bool { self.inner.set_edge_style(id, r, g, b, a, width) }
    pub fn set_edge_style_res(&mut self, id: u32, r: u8, g: u8, b: u8, a: u8, width: f32) -> JsValue {
        if !edge_exists(&self.inner, id) { return error::invalid_id("edge", id); }
        if !width.is_finite() { return error::non_finite("width"); }
        if width <= 0.0 { return error::out_of_range("width", 0.0, f32::INFINITY, width); }
        error::ok(JsValue::from_bool(self.inner.set_edge_style(id, r, g, b, a, width)))
    }
    pub fn get_edge_style(&self, id: u32) -> JsValue { if let Some((r,g,b,a,w))=self.inner.get_edge_style(id) { serde_wasm_bindgen::to_value(&vec![r as f32,g as f32,b as f32,a as f32,w]).unwrap() } else { JsValue::NULL } }
    pub fn set_edge_cubic(&mut self, id: u32, p1x: f32, p1y: f32, p2x: f32, p2y: f32) -> bool { self.inner.set_edge_cubic(id, p1x, p1y, p2x, p2y) }
    pub fn set_edge_cubic_res(&mut self, id: u32, p1x: f32, p1y: f32, p2x: f32, p2y: f32) -> JsValue {
        if !edge_exists(&self.inner, id) { return error::invalid_id("edge", id); }
        for (n,v) in [("p1x",p1x),("p1y",p1y),("p2x",p2x),("p2y",p2y)] { if !v.is_finite() { return error::non_finite(n); } }
        error::ok(JsValue::from_bool(self.inner.set_edge_cubic(id, p1x, p1y, p2x, p2y)))
    }
    pub fn set_edge_line(&mut self, id: u32) -> bool { self.inner.set_edge_line(id) }
    pub fn set_edge_line_res(&mut self, id: u32) -> JsValue { if !edge_exists(&self.inner, id) { return error::invalid_id("edge", id); } error::ok(JsValue::from_bool(self.inner.set_edge_line(id))) }
    pub fn get_handles(&self, id: u32) -> JsValue { if let Some(h)=self.inner.get_handles(id) { serde_wasm_bindgen::to_value(&h).unwrap() } else { JsValue::NULL } }
    pub fn get_handles_res(&self, id: u32) -> JsValue {
        if !edge_exists(&self.inner, id) { return error::invalid_id("edge", id); }
        match self.inner.get_handles(id) { Some(h)=> error::ok(serde_wasm_bindgen::to_value(&h).unwrap()), None=> error::not_cubic(id) }
    }
    pub fn set_handle_pos(&mut self, id: u32, end: u8, x: f32, y: f32) -> bool { self.inner.set_handle_pos(id, end, x, y) }
    pub fn set_handle_pos_res(&mut self, id: u32, end: u8, x: f32, y: f32) -> JsValue {
        if !edge_exists(&self.inner, id) { return error::invalid_id("edge", id); }
        if end>1 { return error::err("invalid_end", "end must be 0 or 1", None); }
        if !x.is_finite() { return error::non_finite("x"); }
        if !y.is_finite() { return error::non_finite("y"); }
        if self.inner.get_handles(id).is_none() { return error::not_cubic(id); }
        error::ok(JsValue::from_bool(self.inner.set_handle_pos(id, end, x, y)))
    }
    pub fn set_handle_mode(&mut self, id: u32, mode: u8) -> bool { self.inner.set_handle_mode(id, mode) }
    pub fn set_handle_mode_res(&mut self, id: u32, mode: u8) -> JsValue {
        if !edge_exists(&self.inner, id) { return error::invalid_id("edge", id); }
        if mode>2 { return error::invalid_mode(mode); }
        if self.inner.get_handles(id).is_none() { return error::not_cubic(id); }
        error::ok(JsValue::from_bool(self.inner.set_handle_mode(id, mode)))
    }
    pub fn bend_edge_to(&mut self, id: u32, t: f32, tx: f32, ty: f32, stiffness: f32) -> bool { self.inner.bend_edge_to(id, t, tx, ty, stiffness) }
    pub fn bend_edge_to_res(&mut self, id: u32, t: f32, tx: f32, ty: f32, stiffness: f32) -> JsValue {
        if !edge_exists(&self.inner, id) { return error::invalid_id("edge", id); }
        if !t.is_finite() { return error::non_finite("t"); }
        if t < 0.0 || t > 1.0 { return error::out_of_range("t", 0.0, 1.0, t); }
        for (n,v) in [("tx",tx),("ty",ty),("stiffness",stiffness)] { if !v.is_finite() { return error::non_finite(n); } }
        if stiffness <= 0.0 { return error::out_of_range("stiffness", 0.0, f32::INFINITY, stiffness); }
        error::ok(JsValue::from_bool(self.inner.bend_edge_to(id, t, tx, ty, stiffness)))
    }

    // Transforms and grouping
    pub fn transform_all(&mut self, s: f32, tx: f32, ty: f32, scale_stroke: bool) { self.inner.transform_all(s, tx, ty, scale_stroke) }
    pub fn transform_all_res(&mut self, s: f32, tx: f32, ty: f32, scale_stroke: bool) -> JsValue {
        for (n,v) in [("s",s),("tx",tx),("ty",ty)] { if !v.is_finite() { return error::non_finite(n); } }
        self.inner.transform_all(s, tx, ty, scale_stroke);
        error::ok(JsValue::from_bool(true))
    }
    pub fn translate_nodes(&mut self, node_ids: &Uint32Array, dx: f32, dy: f32) -> u32 { let mut v=vec![0u32; node_ids.length() as usize]; node_ids.copy_to(&mut v); self.inner.translate_nodes(&v, dx, dy) }
    pub fn translate_nodes_res(&mut self, node_ids: &Uint32Array, dx: f32, dy: f32) -> JsValue {
        if !dx.is_finite() { return error::non_finite("dx"); }
        if !dy.is_finite() { return error::non_finite("dy"); }
        let len = node_ids.length() as usize; let mut ids=vec![0u32; len]; node_ids.copy_to(&mut ids);
        for id in &ids { if self.inner.get_node(*id).is_none() { return error::invalid_id("node", *id); } }
        let moved = self.inner.translate_nodes(&ids, dx, dy);
        error::ok(JsValue::from_f64(moved as f64))
    }
    pub fn translate_edges(&mut self, edge_ids: &Uint32Array, dx: f32, dy: f32, split_shared: bool) -> u32 { let mut v=vec![0u32; edge_ids.length() as usize]; edge_ids.copy_to(&mut v); self.inner.translate_edges(&v, dx, dy, split_shared) }
    pub fn translate_edges_res(&mut self, edge_ids: &Uint32Array, dx: f32, dy: f32, split_shared: bool) -> JsValue {
        if !dx.is_finite() { return error::non_finite("dx"); }
        if !dy.is_finite() { return error::non_finite("dy"); }
        let len = edge_ids.length() as usize; let mut ids=vec![0u32; len]; edge_ids.copy_to(&mut ids);
        for id in &ids { if !edge_exists(&self.inner, *id) { return error::invalid_id("edge", *id); } }
        let moved = self.inner.translate_edges(&ids, dx, dy, split_shared);
        error::ok(JsValue::from_f64(moved as f64))
    }

    // Polylines
    pub fn add_polyline_edge(&mut self, a: u32, b: u32, points: &Float32Array) -> Option<u32> { let pts=to_pairs(points); self.inner.add_polyline_edge(a,b,&pts) }
    pub fn add_polyline_edge_res(&mut self, a: u32, b: u32, points: &Float32Array) -> JsValue {
        if self.inner.get_node(a).is_none() { return error::invalid_id("node", a); }
        if self.inner.get_node(b).is_none() { return error::invalid_id("node", b); }
        let len = points.length() as usize; if len%2==1 { return error::err("invalid_array", "points must have even length", None); }
        let mut buf=vec![0.0f32; len]; points.copy_to(&mut buf); if buf.iter().any(|v| !v.is_finite()) { return error::non_finite("points"); }
        let pts: Vec<(f32,f32)> = buf.chunks(2).map(|c|(c[0],c[1])).collect();
        match self.inner.add_polyline_edge(a,b,&pts) { Some(eid)=> error::ok(JsValue::from_f64(eid as f64)), None=> error::err("invalid_edge", "failed to add polyline edge", None) }
    }
    pub fn set_edge_polyline(&mut self, id: u32, points: &Float32Array) -> bool { let pts=to_pairs(points); self.inner.set_edge_polyline(id, &pts) }
    pub fn set_edge_polyline_res(&mut self, id: u32, points: &Float32Array) -> JsValue {
        if !edge_exists(&self.inner, id) { return error::invalid_id("edge", id); }
        let len = points.length() as usize; if len%2==1 { return error::err("invalid_array", "points must have even length", None); }
        let mut buf=vec![0.0f32; len]; points.copy_to(&mut buf); if buf.iter().any(|v| !v.is_finite()) { return error::non_finite("points"); }
        let pts: Vec<(f32,f32)> = buf.chunks(2).map(|c|(c[0],c[1])).collect();
        error::ok(JsValue::from_bool(self.inner.set_edge_polyline(id, &pts)))
    }
    pub fn get_polyline_points(&self, id: u32) -> JsValue { if let Some(pts)=self.inner.get_polyline_points(id) { let mut flat=Vec::with_capacity(pts.len()*2); for (x,y) in pts { flat.push(x); flat.push(y); } Float32Array::from(flat.as_slice()).into() } else { JsValue::NULL } }
    pub fn get_polyline_points_res(&self, id: u32) -> JsValue {
        if !edge_exists(&self.inner, id) { return error::invalid_id("edge", id); }
        match self.inner.get_polyline_points(id) { Some(pts)=> { let mut flat=Vec::with_capacity(pts.len()*2); for (x,y) in pts { flat.push(x); flat.push(y); } error::ok(Float32Array::from(flat.as_slice()).into()) }, None=> error::not_polyline(id) }
    }

    // Freehand fitting
    pub fn add_freehand(&mut self, points: &Float32Array, close: bool) -> js_sys::Uint32Array {
        let pts = to_pairs(points);
        let edges = self.inner.add_freehand(&pts, close);
        crate::interop::arr_u32(&edges)
    }
    pub fn add_freehand_res(&mut self, points: &Float32Array, close: bool) -> JsValue {
        let len = points.length() as usize; if len%2==1 || len<4 { return error::err("invalid_array", "points must be even length and contain at least 2 points", None); }
        let mut buf=vec![0.0f32; len]; points.copy_to(&mut buf); if buf.iter().any(|v| !v.is_finite()) { return error::non_finite("points"); }
        let pts: Vec<(f32,f32)> = buf.chunks(2).map(|c|(c[0],c[1])).collect();
        let edges = self.inner.add_freehand(&pts, close);
        error::ok(crate::interop::arr_u32(&edges).into())
    }
}

fn to_pairs(arr: &Float32Array) -> Vec<(f32,f32)> { let len=arr.length() as usize; let mut buf=vec![0.0f32; len]; arr.copy_to(&mut buf); let mut out=Vec::with_capacity(len/2); let mut i=0; while i+1<len { out.push((buf[i],buf[i+1])); i+=2; } out }
fn edge_exists(g: &contour::Graph, id: u32) -> bool { let ea = g.get_edge_arrays(); ea.ids.iter().any(|&x| x==id) }

fn region_exists(g: &mut contour::Graph, key: u32) -> bool {
    let regs = g.get_regions();
    for v in regs { if let Some(k)=v.get("key").and_then(|x| x.as_u64()) { if k as u32 == key { return true; } } }
    false
}
