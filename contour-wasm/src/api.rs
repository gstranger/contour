use wasm_bindgen::prelude::*;
use js_sys::{Uint32Array, Float32Array};
use crate::Graph;
type JsValue = wasm_bindgen::JsValue;

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
    pub fn move_node(&mut self, id: u32, x: f32, y: f32) -> bool { self.inner.move_node(id, x, y) }
    pub fn get_node(&self, id: u32) -> JsValue { if let Some((x,y))=self.inner.get_node(id) { serde_wasm_bindgen::to_value(&vec![x,y]).unwrap() } else { JsValue::NULL } }
    pub fn remove_node(&mut self, id: u32) -> bool { self.inner.remove_node(id) }
    pub fn node_count(&self) -> u32 { self.inner.node_count() }
    pub fn add_edge(&mut self, a: u32, b: u32) -> Option<u32> { self.inner.add_edge(a, b) }
    pub fn remove_edge(&mut self, id: u32) -> bool { self.inner.remove_edge(id) }
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
    pub fn pick(&self, x: f32, y: f32, tol: f32) -> JsValue { if let Some(p)=self.inner.pick(x,y,tol) { serde_wasm_bindgen::to_value(&p).unwrap() } else { JsValue::NULL } }
    pub fn to_json(&self) -> JsValue { serde_wasm_bindgen::to_value(&self.inner.to_json_value()).unwrap() }
    pub fn from_json(&mut self, v: JsValue) -> bool { match serde_wasm_bindgen::from_value::<serde_json::Value>(v) { Ok(val)=> self.inner.from_json_value(val), Err(_)=> false } }
    pub fn clear(&mut self) { self.inner.clear(); }
    pub fn add_svg_path(&mut self, d: &str) -> u32 { self.inner.add_svg_path(d, None) }
    pub fn add_svg_path_with_style(&mut self, d: &str, r: u8, g: u8, b: u8, a: u8, width: f32) -> u32 { self.inner.add_svg_path(d, Some((r,g,b,a,width))) }
    pub fn to_svg_paths(&self) -> JsValue { serde_wasm_bindgen::to_value(&self.inner.to_svg_paths()).unwrap() }

    // Regions + fill
    pub fn get_regions(&mut self) -> JsValue { serde_wasm_bindgen::to_value(&self.inner.get_regions()).unwrap() }
    pub fn toggle_region(&mut self, key: u32) -> bool { self.inner.toggle_region(key) }
    pub fn set_region_fill(&mut self, key: u32, filled: bool) { self.inner.set_region_fill(key, filled) }
    pub fn set_region_color(&mut self, key: u32, r: u8, g: u8, b: u8, a: u8) { self.inner.set_region_color(key, r, g, b, a) }
    pub fn set_flatten_tolerance(&mut self, tol: f32) { self.inner.set_flatten_tolerance(tol) }

    // Styling/handles
    pub fn set_edge_style(&mut self, id: u32, r: u8, g: u8, b: u8, a: u8, width: f32) -> bool { self.inner.set_edge_style(id, r, g, b, a, width) }
    pub fn get_edge_style(&self, id: u32) -> JsValue { if let Some((r,g,b,a,w))=self.inner.get_edge_style(id) { serde_wasm_bindgen::to_value(&vec![r as f32,g as f32,b as f32,a as f32,w]).unwrap() } else { JsValue::NULL } }
    pub fn set_edge_cubic(&mut self, id: u32, p1x: f32, p1y: f32, p2x: f32, p2y: f32) -> bool { self.inner.set_edge_cubic(id, p1x, p1y, p2x, p2y) }
    pub fn set_edge_line(&mut self, id: u32) -> bool { self.inner.set_edge_line(id) }
    pub fn get_handles(&self, id: u32) -> JsValue { if let Some(h)=self.inner.get_handles(id) { serde_wasm_bindgen::to_value(&h).unwrap() } else { JsValue::NULL } }
    pub fn set_handle_pos(&mut self, id: u32, end: u8, x: f32, y: f32) -> bool { self.inner.set_handle_pos(id, end, x, y) }
    pub fn set_handle_mode(&mut self, id: u32, mode: u8) -> bool { self.inner.set_handle_mode(id, mode) }
    pub fn bend_edge_to(&mut self, id: u32, t: f32, tx: f32, ty: f32, stiffness: f32) -> bool { self.inner.bend_edge_to(id, t, tx, ty, stiffness) }

    // Transforms and grouping
    pub fn transform_all(&mut self, s: f32, tx: f32, ty: f32, scale_stroke: bool) { self.inner.transform_all(s, tx, ty, scale_stroke) }
    pub fn translate_nodes(&mut self, node_ids: &Uint32Array, dx: f32, dy: f32) -> u32 { let mut v=vec![0u32; node_ids.length() as usize]; node_ids.copy_to(&mut v); self.inner.translate_nodes(&v, dx, dy) }
    pub fn translate_edges(&mut self, edge_ids: &Uint32Array, dx: f32, dy: f32, split_shared: bool) -> u32 { let mut v=vec![0u32; edge_ids.length() as usize]; edge_ids.copy_to(&mut v); self.inner.translate_edges(&v, dx, dy, split_shared) }

    // Polylines
    pub fn add_polyline_edge(&mut self, a: u32, b: u32, points: &Float32Array) -> Option<u32> { let pts=to_pairs(points); self.inner.add_polyline_edge(a,b,&pts) }
    pub fn set_edge_polyline(&mut self, id: u32, points: &Float32Array) -> bool { let pts=to_pairs(points); self.inner.set_edge_polyline(id, &pts) }
    pub fn get_polyline_points(&self, id: u32) -> JsValue { if let Some(pts)=self.inner.get_polyline_points(id) { let mut flat=Vec::with_capacity(pts.len()*2); for (x,y) in pts { flat.push(x); flat.push(y); } Float32Array::from(flat.as_slice()).into() } else { JsValue::NULL } }
}

fn to_pairs(arr: &Float32Array) -> Vec<(f32,f32)> { let len=arr.length() as usize; let mut buf=vec![0.0f32; len]; arr.copy_to(&mut buf); let mut out=Vec::with_capacity(len/2); let mut i=0; while i+1<len { out.push((buf[i],buf[i+1])); i+=2; } out }
