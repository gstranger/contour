use crate::Graph;
use js_sys::{Float32Array, Uint32Array};
use wasm_bindgen::prelude::*;
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
    pub fn new() -> Graph {
        crate::Graph::rs_new()
    }
    pub fn geom_version(&self) -> u64 {
        self.rs_geom_version()
    }

    // Nodes/Edges basic
    pub fn add_node(&mut self, x: f32, y: f32) -> u32 {
        self.inner.add_node(x, y)
    }
    pub fn add_node_res(&mut self, x: f32, y: f32) -> JsValue {
        if !x.is_finite() {
            return error::non_finite("x");
        }
        if !y.is_finite() {
            return error::non_finite("y");
        }
        error::ok(JsValue::from_f64(self.inner.add_node(x, y) as f64))
    }
    pub fn move_node(&mut self, id: u32, x: f32, y: f32) -> bool {
        self.inner.move_node(id, x, y)
    }
    pub fn move_node_res(&mut self, id: u32, x: f32, y: f32) -> JsValue {
        if !x.is_finite() {
            return error::non_finite("x");
        }
        if !y.is_finite() {
            return error::non_finite("y");
        }
        if self.inner.get_node(id).is_none() {
            return error::invalid_id("node", id);
        }
        let ok = self.inner.move_node(id, x, y);
        error::ok(JsValue::from_bool(ok))
    }
    pub fn get_node(&self, id: u32) -> JsValue {
        if let Some((x, y)) = self.inner.get_node(id) {
            serde_wasm_bindgen::to_value(&vec![x, y]).unwrap()
        } else {
            JsValue::NULL
        }
    }
    pub fn get_node_res(&self, id: u32) -> JsValue {
        if let Some((x, y)) = self.inner.get_node(id) {
            error::ok(serde_wasm_bindgen::to_value(&vec![x, y]).unwrap())
        } else {
            error::invalid_id("node", id)
        }
    }
    pub fn remove_node(&mut self, id: u32) -> bool {
        self.inner.remove_node(id)
    }
    pub fn remove_node_res(&mut self, id: u32) -> JsValue {
        if self.inner.get_node(id).is_none() {
            return error::invalid_id("node", id);
        }
        error::ok(JsValue::from_bool(self.inner.remove_node(id)))
    }
    pub fn node_count(&self) -> u32 {
        self.inner.node_count()
    }
    pub fn add_edge(&mut self, a: u32, b: u32) -> Option<u32> {
        self.inner.add_edge(a, b)
    }
    pub fn add_edge_res(&mut self, a: u32, b: u32) -> JsValue {
        if self.inner.get_node(a).is_none() {
            return error::invalid_id("node", a);
        }
        if self.inner.get_node(b).is_none() {
            return error::invalid_id("node", b);
        }
        if a == b {
            return error::err(
                "invalid_edge",
                "edge endpoints cannot be the same node",
                None,
            );
        }
        match self.inner.add_edge(a, b) {
            Some(eid) => error::ok(JsValue::from_f64(eid as f64)),
            None => error::err("invalid_edge", "failed to add edge", None),
        }
    }
    pub fn remove_edge(&mut self, id: u32) -> bool {
        self.inner.remove_edge(id)
    }
    pub fn remove_edge_res(&mut self, id: u32) -> JsValue {
        if !edge_exists(&self.inner, id) {
            return error::invalid_id("edge", id);
        }
        error::ok(JsValue::from_bool(self.inner.remove_edge(id)))
    }
    pub fn edge_count(&self) -> u32 {
        self.inner.edge_count()
    }

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
        crate::interop::set_kv(
            &obj,
            "endpoints",
            &crate::interop::arr_u32(&ea.endpoints).into(),
        );
        crate::interop::set_kv(&obj, "kinds", &crate::interop::arr_u8(&ea.kinds).into());
        crate::interop::set_kv(
            &obj,
            "stroke_rgba",
            &crate::interop::arr_u8(&ea.stroke_rgba).into(),
        );
        crate::interop::set_kv(
            &obj,
            "stroke_widths",
            &crate::interop::arr_f32(&ea.stroke_widths).into(),
        );
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
        if !x.is_finite() {
            return error::non_finite("x");
        }
        if !y.is_finite() {
            return error::non_finite("y");
        }
        if !tol.is_finite() {
            return error::non_finite("tol");
        }
        if tol < 0.0 {
            return error::out_of_range("tol", 0.0, f32::INFINITY, tol);
        }
        let v = self.pick(x, y, tol);
        if v.is_null() {
            error::ok(JsValue::NULL)
        } else {
            error::ok(v)
        }
    }
    pub fn to_json(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.inner.to_json_value()).unwrap()
    }
    pub fn from_json(&mut self, v: JsValue) -> bool {
        match serde_wasm_bindgen::from_value::<serde_json::Value>(v) {
            Ok(val) => self.inner.from_json_value(val),
            Err(_) => false,
        }
    }
    pub fn from_json_res(&mut self, v: JsValue) -> JsValue {
        match serde_wasm_bindgen::from_value::<serde_json::Value>(v) {
            Ok(val) => match self.inner.from_json_value_strict(val) {
                Ok(ok) => error::ok(JsValue::from_bool(ok)),
                Err((code, msg)) => error::err(code, msg, None),
            },
            Err(e) => error::err("json_parse", format!("{}", e), None),
        }
    }
    pub fn clear(&mut self) {
        self.inner.clear();
    }
    pub fn add_svg_path(&mut self, d: &str) -> u32 {
        self.inner.add_svg_path(d, None)
    }
    pub fn add_svg_path_with_style(
        &mut self,
        d: &str,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
        width: f32,
    ) -> u32 {
        self.inner.add_svg_path(d, Some((r, g, b, a, width)))
    }
    pub fn to_svg_paths(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.inner.to_svg_paths()).unwrap()
    }
    pub fn add_svg_path_res(&mut self, d: &str) -> JsValue {
        let before = self.inner.geom_version();
        let added = self.inner.add_svg_path(d, None);
        if added == 0 {
            return error::err("svg_parse", "no edges parsed from path", None);
        }
        // geom_version should have advanced if edges were added
        let _ = before; // silence unused if optimized
        error::ok(JsValue::from_f64(added as f64))
    }
    pub fn to_svg_paths_res(&self) -> JsValue {
        error::ok(self.to_svg_paths())
    }

    // Regions + fill
    pub fn get_regions(&mut self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.inner.get_regions()).unwrap()
    }
    pub fn get_regions_res(&mut self) -> JsValue {
        error::ok(self.get_regions())
    }
    pub fn toggle_region(&mut self, key: u32) -> bool {
        self.inner.toggle_region(key)
    }
    pub fn toggle_region_res(&mut self, key: u32) -> JsValue {
        if !region_exists(&mut self.inner, key) {
            return error::invalid_id("region", key);
        }
        error::ok(JsValue::from_bool(self.inner.toggle_region(key)))
    }
    pub fn set_region_fill(&mut self, key: u32, filled: bool) {
        self.inner.set_region_fill(key, filled)
    }
    pub fn set_region_fill_res(&mut self, key: u32, filled: bool) -> JsValue {
        if !region_exists(&mut self.inner, key) {
            return error::invalid_id("region", key);
        }
        self.inner.set_region_fill(key, filled);
        error::ok(JsValue::from_bool(true))
    }
    pub fn set_region_color(&mut self, key: u32, r: u8, g: u8, b: u8, a: u8) {
        self.inner.set_region_color(key, r, g, b, a)
    }
    pub fn set_region_color_res(&mut self, key: u32, r: u8, g: u8, b: u8, a: u8) -> JsValue {
        if !region_exists(&mut self.inner, key) {
            return error::invalid_id("region", key);
        }
        self.inner.set_region_color(key, r, g, b, a);
        error::ok(JsValue::from_bool(true))
    }
    pub fn set_flatten_tolerance(&mut self, tol: f32) {
        self.inner.set_flatten_tolerance(tol)
    }
    pub fn set_flatten_tolerance_res(&mut self, tol: f32) -> JsValue {
        if !tol.is_finite() {
            return error::non_finite("tol");
        }
        if tol <= 0.0 || tol > 10.0 {
            return error::out_of_range("tol", 0.01, 10.0, tol);
        }
        self.inner.set_flatten_tolerance(tol);
        error::ok(JsValue::from_bool(true))
    }

    // Styling/handles
    pub fn set_edge_style(&mut self, id: u32, r: u8, g: u8, b: u8, a: u8, width: f32) -> bool {
        self.inner.set_edge_style(id, r, g, b, a, width)
    }
    pub fn set_edge_style_res(
        &mut self,
        id: u32,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
        width: f32,
    ) -> JsValue {
        if !edge_exists(&self.inner, id) {
            return error::invalid_id("edge", id);
        }
        if !width.is_finite() {
            return error::non_finite("width");
        }
        if width <= 0.0 {
            return error::out_of_range("width", 0.0, f32::INFINITY, width);
        }
        error::ok(JsValue::from_bool(
            self.inner.set_edge_style(id, r, g, b, a, width),
        ))
    }
    pub fn get_edge_style(&self, id: u32) -> JsValue {
        if let Some((r, g, b, a, w)) = self.inner.get_edge_style(id) {
            serde_wasm_bindgen::to_value(&vec![r as f32, g as f32, b as f32, a as f32, w]).unwrap()
        } else {
            JsValue::NULL
        }
    }
    pub fn set_edge_cubic(&mut self, id: u32, p1x: f32, p1y: f32, p2x: f32, p2y: f32) -> bool {
        self.inner.set_edge_cubic(id, p1x, p1y, p2x, p2y)
    }
    pub fn set_edge_cubic_res(
        &mut self,
        id: u32,
        p1x: f32,
        p1y: f32,
        p2x: f32,
        p2y: f32,
    ) -> JsValue {
        if !edge_exists(&self.inner, id) {
            return error::invalid_id("edge", id);
        }
        for (n, v) in [("p1x", p1x), ("p1y", p1y), ("p2x", p2x), ("p2y", p2y)] {
            if !v.is_finite() {
                return error::non_finite(n);
            }
        }
        error::ok(JsValue::from_bool(
            self.inner.set_edge_cubic(id, p1x, p1y, p2x, p2y),
        ))
    }
    pub fn set_edge_line(&mut self, id: u32) -> bool {
        self.inner.set_edge_line(id)
    }
    pub fn set_edge_line_res(&mut self, id: u32) -> JsValue {
        if !edge_exists(&self.inner, id) {
            return error::invalid_id("edge", id);
        }
        error::ok(JsValue::from_bool(self.inner.set_edge_line(id)))
    }
    pub fn get_handles(&self, id: u32) -> JsValue {
        if let Some(h) = self.inner.get_handles(id) {
            serde_wasm_bindgen::to_value(&h).unwrap()
        } else {
            JsValue::NULL
        }
    }
    pub fn get_handles_res(&self, id: u32) -> JsValue {
        if !edge_exists(&self.inner, id) {
            return error::invalid_id("edge", id);
        }
        match self.inner.get_handles(id) {
            Some(h) => error::ok(serde_wasm_bindgen::to_value(&h).unwrap()),
            None => error::not_cubic(id),
        }
    }
    pub fn set_handle_pos(&mut self, id: u32, end: u8, x: f32, y: f32) -> bool {
        self.inner.set_handle_pos(id, end, x, y)
    }
    pub fn set_handle_pos_res(&mut self, id: u32, end: u8, x: f32, y: f32) -> JsValue {
        if !edge_exists(&self.inner, id) {
            return error::invalid_id("edge", id);
        }
        if end > 1 {
            return error::err("invalid_end", "end must be 0 or 1", None);
        }
        if !x.is_finite() {
            return error::non_finite("x");
        }
        if !y.is_finite() {
            return error::non_finite("y");
        }
        if self.inner.get_handles(id).is_none() {
            return error::not_cubic(id);
        }
        error::ok(JsValue::from_bool(self.inner.set_handle_pos(id, end, x, y)))
    }
    pub fn set_handle_mode(&mut self, id: u32, mode: u8) -> bool {
        self.inner.set_handle_mode(id, mode)
    }
    pub fn set_handle_mode_res(&mut self, id: u32, mode: u8) -> JsValue {
        if !edge_exists(&self.inner, id) {
            return error::invalid_id("edge", id);
        }
        if mode > 2 {
            return error::invalid_mode(mode);
        }
        if self.inner.get_handles(id).is_none() {
            return error::not_cubic(id);
        }
        error::ok(JsValue::from_bool(self.inner.set_handle_mode(id, mode)))
    }
    pub fn bend_edge_to(&mut self, id: u32, t: f32, tx: f32, ty: f32, stiffness: f32) -> bool {
        self.inner.bend_edge_to(id, t, tx, ty, stiffness)
    }
    pub fn bend_edge_to_res(
        &mut self,
        id: u32,
        t: f32,
        tx: f32,
        ty: f32,
        stiffness: f32,
    ) -> JsValue {
        if !edge_exists(&self.inner, id) {
            return error::invalid_id("edge", id);
        }
        if !t.is_finite() {
            return error::non_finite("t");
        }
        if t < 0.0 || t > 1.0 {
            return error::out_of_range("t", 0.0, 1.0, t);
        }
        for (n, v) in [("tx", tx), ("ty", ty), ("stiffness", stiffness)] {
            if !v.is_finite() {
                return error::non_finite(n);
            }
        }
        if stiffness <= 0.0 {
            return error::out_of_range("stiffness", 0.0, f32::INFINITY, stiffness);
        }
        error::ok(JsValue::from_bool(
            self.inner.bend_edge_to(id, t, tx, ty, stiffness),
        ))
    }

    // Transforms and grouping
    pub fn transform_all(&mut self, s: f32, tx: f32, ty: f32, scale_stroke: bool) {
        self.inner.transform_all(s, tx, ty, scale_stroke)
    }
    pub fn transform_all_res(&mut self, s: f32, tx: f32, ty: f32, scale_stroke: bool) -> JsValue {
        for (n, v) in [("s", s), ("tx", tx), ("ty", ty)] {
            if !v.is_finite() {
                return error::non_finite(n);
            }
        }
        self.inner.transform_all(s, tx, ty, scale_stroke);
        error::ok(JsValue::from_bool(true))
    }
    pub fn translate_nodes(&mut self, node_ids: &Uint32Array, dx: f32, dy: f32) -> u32 {
        let mut v = vec![0u32; node_ids.length() as usize];
        node_ids.copy_to(&mut v);
        self.inner.translate_nodes(&v, dx, dy)
    }
    pub fn translate_nodes_res(&mut self, node_ids: &Uint32Array, dx: f32, dy: f32) -> JsValue {
        if !dx.is_finite() {
            return error::non_finite("dx");
        }
        if !dy.is_finite() {
            return error::non_finite("dy");
        }
        let len = node_ids.length() as usize;
        let mut ids = vec![0u32; len];
        node_ids.copy_to(&mut ids);
        for id in &ids {
            if self.inner.get_node(*id).is_none() {
                return error::invalid_id("node", *id);
            }
        }
        let moved = self.inner.translate_nodes(&ids, dx, dy);
        error::ok(JsValue::from_f64(moved as f64))
    }
    pub fn translate_edges(
        &mut self,
        edge_ids: &Uint32Array,
        dx: f32,
        dy: f32,
        split_shared: bool,
    ) -> u32 {
        let mut v = vec![0u32; edge_ids.length() as usize];
        edge_ids.copy_to(&mut v);
        self.inner.translate_edges(&v, dx, dy, split_shared)
    }
    pub fn translate_edges_res(
        &mut self,
        edge_ids: &Uint32Array,
        dx: f32,
        dy: f32,
        split_shared: bool,
    ) -> JsValue {
        if !dx.is_finite() {
            return error::non_finite("dx");
        }
        if !dy.is_finite() {
            return error::non_finite("dy");
        }
        let len = edge_ids.length() as usize;
        let mut ids = vec![0u32; len];
        edge_ids.copy_to(&mut ids);
        for id in &ids {
            if !edge_exists(&self.inner, *id) {
                return error::invalid_id("edge", *id);
            }
        }
        let moved = self.inner.translate_edges(&ids, dx, dy, split_shared);
        error::ok(JsValue::from_f64(moved as f64))
    }

    // Polylines
    pub fn add_polyline_edge(&mut self, a: u32, b: u32, points: &Float32Array) -> Option<u32> {
        let pts = to_pairs(points);
        self.inner.add_polyline_edge(a, b, &pts)
    }
    pub fn add_polyline_edge_res(&mut self, a: u32, b: u32, points: &Float32Array) -> JsValue {
        if self.inner.get_node(a).is_none() {
            return error::invalid_id("node", a);
        }
        if self.inner.get_node(b).is_none() {
            return error::invalid_id("node", b);
        }
        let len = points.length() as usize;
        if len % 2 == 1 {
            return error::err("invalid_array", "points must have even length", None);
        }
        let mut buf = vec![0.0f32; len];
        points.copy_to(&mut buf);
        if buf.iter().any(|v| !v.is_finite()) {
            return error::non_finite("points");
        }
        let pts: Vec<(f32, f32)> = buf.chunks(2).map(|c| (c[0], c[1])).collect();
        match self.inner.add_polyline_edge(a, b, &pts) {
            Some(eid) => error::ok(JsValue::from_f64(eid as f64)),
            None => error::err("invalid_edge", "failed to add polyline edge", None),
        }
    }
    pub fn set_edge_polyline(&mut self, id: u32, points: &Float32Array) -> bool {
        let pts = to_pairs(points);
        self.inner.set_edge_polyline(id, &pts)
    }
    pub fn set_edge_polyline_res(&mut self, id: u32, points: &Float32Array) -> JsValue {
        if !edge_exists(&self.inner, id) {
            return error::invalid_id("edge", id);
        }
        let len = points.length() as usize;
        if len % 2 == 1 {
            return error::err("invalid_array", "points must have even length", None);
        }
        let mut buf = vec![0.0f32; len];
        points.copy_to(&mut buf);
        if buf.iter().any(|v| !v.is_finite()) {
            return error::non_finite("points");
        }
        let pts: Vec<(f32, f32)> = buf.chunks(2).map(|c| (c[0], c[1])).collect();
        error::ok(JsValue::from_bool(self.inner.set_edge_polyline(id, &pts)))
    }
    pub fn get_polyline_points(&self, id: u32) -> JsValue {
        if let Some(pts) = self.inner.get_polyline_points(id) {
            let mut flat = Vec::with_capacity(pts.len() * 2);
            for (x, y) in pts {
                flat.push(x);
                flat.push(y);
            }
            Float32Array::from(flat.as_slice()).into()
        } else {
            JsValue::NULL
        }
    }
    pub fn get_polyline_points_res(&self, id: u32) -> JsValue {
        if !edge_exists(&self.inner, id) {
            return error::invalid_id("edge", id);
        }
        match self.inner.get_polyline_points(id) {
            Some(pts) => {
                let mut flat = Vec::with_capacity(pts.len() * 2);
                for (x, y) in pts {
                    flat.push(x);
                    flat.push(y);
                }
                error::ok(Float32Array::from(flat.as_slice()).into())
            }
            None => error::not_polyline(id),
        }
    }

    // Freehand fitting
    pub fn add_freehand(&mut self, points: &Float32Array, close: bool) -> js_sys::Uint32Array {
        let pts = to_pairs(points);
        let edges = self.inner.add_freehand(&pts, close);
        crate::interop::arr_u32(&edges)
    }
    pub fn add_freehand_res(&mut self, points: &Float32Array, close: bool) -> JsValue {
        let len = points.length() as usize;
        if len % 2 == 1 || len < 4 {
            return error::err(
                "invalid_array",
                "points must be even length and contain at least 2 points",
                None,
            );
        }
        let mut buf = vec![0.0f32; len];
        points.copy_to(&mut buf);
        if buf.iter().any(|v| !v.is_finite()) {
            return error::non_finite("points");
        }
        let pts: Vec<(f32, f32)> = buf.chunks(2).map(|c| (c[0], c[1])).collect();
        let edges = self.inner.add_freehand(&pts, close);
        error::ok(crate::interop::arr_u32(&edges).into())
    }

    // ========== Layer Management ==========

    /// Create a new layer, returns layer ID
    pub fn create_layer(&mut self, name: &str) -> u32 {
        self.inner.create_layer(name.to_string())
    }

    pub fn create_layer_res(&mut self, name: &str) -> JsValue {
        error::ok(JsValue::from_f64(
            self.inner.create_layer(name.to_string()) as f64,
        ))
    }

    /// Remove a layer (optionally removes its edges)
    pub fn remove_layer(&mut self, id: u32, remove_edges: bool) -> bool {
        self.inner.remove_layer(id, remove_edges)
    }

    pub fn remove_layer_res(&mut self, id: u32, remove_edges: bool) -> JsValue {
        if self.inner.remove_layer(id, remove_edges) {
            error::ok(JsValue::from_bool(true))
        } else {
            error::invalid_id("layer", id)
        }
    }

    /// Get all layers as array of {id, name, z_index, visible, opacity}
    pub fn get_layers(&self) -> JsValue {
        let layers = self.inner.get_layers();
        let arr: Vec<_> = layers
            .into_iter()
            .map(|(id, name, z_index, visible, opacity)| {
                serde_json::json!({
                    "id": id,
                    "name": name,
                    "z_index": z_index,
                    "visible": visible,
                    "opacity": opacity
                })
            })
            .collect();
        serde_wasm_bindgen::to_value(&arr).unwrap()
    }

    /// Rename a layer
    pub fn rename_layer(&mut self, id: u32, name: &str) -> bool {
        self.inner.rename_layer(id, name.to_string())
    }

    pub fn rename_layer_res(&mut self, id: u32, name: &str) -> JsValue {
        if self.inner.rename_layer(id, name.to_string()) {
            error::ok(JsValue::from_bool(true))
        } else {
            error::invalid_id("layer", id)
        }
    }

    /// Set layer visibility
    pub fn set_layer_visibility(&mut self, id: u32, visible: bool) -> bool {
        self.inner.set_layer_visibility(id, visible)
    }

    pub fn set_layer_visibility_res(&mut self, id: u32, visible: bool) -> JsValue {
        if self.inner.set_layer_visibility(id, visible) {
            error::ok(JsValue::from_bool(true))
        } else {
            error::invalid_id("layer", id)
        }
    }

    /// Set layer opacity
    pub fn set_layer_opacity(&mut self, id: u32, opacity: f32) -> bool {
        self.inner.set_layer_opacity(id, opacity)
    }

    pub fn set_layer_opacity_res(&mut self, id: u32, opacity: f32) -> JsValue {
        if !opacity.is_finite() {
            return error::non_finite("opacity");
        }
        if self.inner.set_layer_opacity(id, opacity) {
            error::ok(JsValue::from_bool(true))
        } else {
            error::invalid_id("layer", id)
        }
    }

    /// Set layer z-index
    pub fn set_layer_z_index(&mut self, id: u32, z: i32) -> bool {
        self.inner.set_layer_z_index(id, z)
    }

    pub fn set_layer_z_index_res(&mut self, id: u32, z: i32) -> JsValue {
        if self.inner.set_layer_z_index(id, z) {
            error::ok(JsValue::from_bool(true))
        } else {
            error::invalid_id("layer", id)
        }
    }

    // ========== Group Management ==========

    /// Create a group within a parent group
    pub fn create_group(&mut self, name: &str, parent_id: u32) -> Option<u32> {
        self.inner.create_group(name.to_string(), parent_id)
    }

    pub fn create_group_res(&mut self, name: &str, parent_id: u32) -> JsValue {
        match self.inner.create_group(name.to_string(), parent_id) {
            Some(id) => error::ok(JsValue::from_f64(id as f64)),
            None => error::invalid_id("group", parent_id),
        }
    }

    /// Remove a group (edges/children move to parent)
    pub fn remove_group(&mut self, id: u32) -> bool {
        self.inner.remove_group(id)
    }

    pub fn remove_group_res(&mut self, id: u32) -> JsValue {
        if self.inner.remove_group(id) {
            error::ok(JsValue::from_bool(true))
        } else {
            error::invalid_id("group", id)
        }
    }

    /// Get all groups as array of {id, name, parent, visible, opacity}
    pub fn get_groups(&self) -> JsValue {
        let groups = self.inner.get_groups();
        let arr: Vec<_> = groups
            .into_iter()
            .map(|(id, name, parent, visible, opacity)| {
                serde_json::json!({
                    "id": id,
                    "name": name,
                    "parent": parent,
                    "visible": visible,
                    "opacity": opacity
                })
            })
            .collect();
        serde_wasm_bindgen::to_value(&arr).unwrap()
    }

    /// Rename a group
    pub fn rename_group(&mut self, id: u32, name: &str) -> bool {
        self.inner.rename_group(id, name.to_string())
    }

    pub fn rename_group_res(&mut self, id: u32, name: &str) -> JsValue {
        if self.inner.rename_group(id, name.to_string()) {
            error::ok(JsValue::from_bool(true))
        } else {
            error::invalid_id("group", id)
        }
    }

    /// Set group visibility
    pub fn set_group_visibility(&mut self, id: u32, visible: bool) -> bool {
        self.inner.set_group_visibility(id, visible)
    }

    pub fn set_group_visibility_res(&mut self, id: u32, visible: bool) -> JsValue {
        if self.inner.set_group_visibility(id, visible) {
            error::ok(JsValue::from_bool(true))
        } else {
            error::invalid_id("group", id)
        }
    }

    /// Set group opacity
    pub fn set_group_opacity(&mut self, id: u32, opacity: f32) -> bool {
        self.inner.set_group_opacity(id, opacity)
    }

    pub fn set_group_opacity_res(&mut self, id: u32, opacity: f32) -> JsValue {
        if !opacity.is_finite() {
            return error::non_finite("opacity");
        }
        if self.inner.set_group_opacity(id, opacity) {
            error::ok(JsValue::from_bool(true))
        } else {
            error::invalid_id("group", id)
        }
    }

    /// Add an edge to a specific group
    pub fn add_edge_to_group(&mut self, edge_id: u32, group_id: u32) -> bool {
        self.inner.add_edge_to_group(edge_id, group_id)
    }

    pub fn add_edge_to_group_res(&mut self, edge_id: u32, group_id: u32) -> JsValue {
        if !edge_exists(&self.inner, edge_id) {
            return error::invalid_id("edge", edge_id);
        }
        if self.inner.add_edge_to_group(edge_id, group_id) {
            error::ok(JsValue::from_bool(true))
        } else {
            error::invalid_id("group", group_id)
        }
    }

    /// Get the group containing an edge
    pub fn get_edge_group(&self, edge_id: u32) -> Option<u32> {
        self.inner.get_edge_group(edge_id)
    }

    /// Get the layer containing an edge
    pub fn get_edge_layer(&self, edge_id: u32) -> Option<u32> {
        self.inner.get_edge_layer(edge_id)
    }

    /// Check if an edge is visible
    pub fn is_edge_visible(&self, edge_id: u32) -> bool {
        self.inner.is_edge_visible(edge_id)
    }

    /// Get all visible edge IDs
    pub fn get_visible_edges(&self) -> Uint32Array {
        let edges = self.inner.get_visible_edges();
        crate::interop::arr_u32(&edges)
    }

    /// Get effective opacity for an edge
    pub fn get_edge_opacity(&self, edge_id: u32) -> f32 {
        self.inner.get_edge_opacity(edge_id)
    }

    /// Get the default group ID
    pub fn default_group(&self) -> Option<u32> {
        self.inner.default_group()
    }

    // ========== Gradient Management ==========

    /// Add a linear gradient, returns gradient ID
    /// stops is an array of {offset, r, g, b, a} objects
    pub fn add_linear_gradient(
        &mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stops: &JsValue,
        units: u8,
        spread: u8,
    ) -> u32 {
        let stops_vec = parse_color_stops(stops);
        let units = parse_gradient_units(units);
        let spread = parse_spread_method(spread);
        self.inner.add_linear_gradient(x1, y1, x2, y2, stops_vec, units, spread)
    }

    pub fn add_linear_gradient_res(
        &mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stops: &JsValue,
        units: u8,
        spread: u8,
    ) -> JsValue {
        if !x1.is_finite() || !y1.is_finite() || !x2.is_finite() || !y2.is_finite() {
            return error::non_finite("coordinates");
        }
        let stops_vec = parse_color_stops(stops);
        if stops_vec.is_empty() {
            return error::err("invalid_stops", "gradient must have at least one color stop", None);
        }
        let units = parse_gradient_units(units);
        let spread = parse_spread_method(spread);
        let id = self.inner.add_linear_gradient(x1, y1, x2, y2, stops_vec, units, spread);
        error::ok(JsValue::from_f64(id as f64))
    }

    /// Add a radial gradient, returns gradient ID
    pub fn add_radial_gradient(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        fx: f32,
        fy: f32,
        stops: &JsValue,
        units: u8,
        spread: u8,
    ) -> u32 {
        let stops_vec = parse_color_stops(stops);
        let units = parse_gradient_units(units);
        let spread = parse_spread_method(spread);
        self.inner.add_radial_gradient(cx, cy, r, fx, fy, stops_vec, units, spread)
    }

    pub fn add_radial_gradient_res(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        fx: f32,
        fy: f32,
        stops: &JsValue,
        units: u8,
        spread: u8,
    ) -> JsValue {
        if !cx.is_finite() || !cy.is_finite() || !r.is_finite() || !fx.is_finite() || !fy.is_finite() {
            return error::non_finite("coordinates");
        }
        let stops_vec = parse_color_stops(stops);
        if stops_vec.is_empty() {
            return error::err("invalid_stops", "gradient must have at least one color stop", None);
        }
        let units = parse_gradient_units(units);
        let spread = parse_spread_method(spread);
        let id = self.inner.add_radial_gradient(cx, cy, r, fx, fy, stops_vec, units, spread);
        error::ok(JsValue::from_f64(id as f64))
    }

    /// Remove a gradient
    pub fn remove_gradient(&mut self, id: u32) -> bool {
        self.inner.remove_gradient(id)
    }

    pub fn remove_gradient_res(&mut self, id: u32) -> JsValue {
        if self.inner.remove_gradient(id) {
            error::ok(JsValue::from_bool(true))
        } else {
            error::invalid_id("gradient", id)
        }
    }

    /// Get a gradient by ID
    pub fn get_gradient(&self, id: u32) -> JsValue {
        match self.inner.get_gradient(id) {
            Some(gradient) => serde_wasm_bindgen::to_value(gradient).unwrap_or(JsValue::NULL),
            None => JsValue::NULL,
        }
    }

    /// Get all gradient IDs
    pub fn get_gradient_ids(&self) -> Uint32Array {
        let ids = self.inner.gradient_ids();
        crate::interop::arr_u32(&ids)
    }

    /// Get all gradients
    pub fn get_all_gradients(&self) -> JsValue {
        let gradients = self.inner.get_all_gradients();
        let arr: Vec<_> = gradients
            .into_iter()
            .map(|(id, gradient)| {
                serde_json::json!({
                    "id": id,
                    "gradient": gradient
                })
            })
            .collect();
        serde_wasm_bindgen::to_value(&arr).unwrap_or(JsValue::NULL)
    }

    /// Set region fill to a gradient
    pub fn set_region_gradient(&mut self, key: u32, gradient_id: u32) -> bool {
        self.inner.set_region_gradient(key, gradient_id)
    }

    pub fn set_region_gradient_res(&mut self, key: u32, gradient_id: u32) -> JsValue {
        if self.inner.get_gradient(gradient_id).is_none() {
            return error::invalid_id("gradient", gradient_id);
        }
        if self.inner.set_region_gradient(key, gradient_id) {
            error::ok(JsValue::from_bool(true))
        } else {
            error::err("region_not_found", "region not found", None)
        }
    }

    /// Set edge stroke to a gradient
    pub fn set_edge_stroke_gradient(&mut self, edge_id: u32, gradient_id: u32, width: f32) -> bool {
        self.inner.set_edge_stroke_gradient(edge_id, gradient_id, width)
    }

    pub fn set_edge_stroke_gradient_res(&mut self, edge_id: u32, gradient_id: u32, width: f32) -> JsValue {
        if !edge_exists(&self.inner, edge_id) {
            return error::invalid_id("edge", edge_id);
        }
        if self.inner.get_gradient(gradient_id).is_none() {
            return error::invalid_id("gradient", gradient_id);
        }
        if !width.is_finite() {
            return error::non_finite("width");
        }
        if self.inner.set_edge_stroke_gradient(edge_id, gradient_id, width) {
            error::ok(JsValue::from_bool(true))
        } else {
            error::err("stroke_gradient_failed", "failed to set stroke gradient", None)
        }
    }

    // ========== Shape Management ==========

    /// Create a shape from an array of edge IDs
    pub fn create_shape(&mut self, edge_ids: &Uint32Array, closed: bool) -> Option<u32> {
        let len = edge_ids.length() as usize;
        let mut ids = vec![0u32; len];
        edge_ids.copy_to(&mut ids);
        self.inner.create_shape(&ids, closed)
    }

    pub fn create_shape_res(&mut self, edge_ids: &Uint32Array, closed: bool) -> JsValue {
        let len = edge_ids.length() as usize;
        let mut ids = vec![0u32; len];
        edge_ids.copy_to(&mut ids);

        // Validate all edge IDs exist
        for &id in &ids {
            if !edge_exists(&self.inner, id) {
                return error::invalid_id("edge", id);
            }
        }

        match self.inner.create_shape(&ids, closed) {
            Some(shape_id) => error::ok(JsValue::from_f64(shape_id as f64)),
            None => error::err("shape_creation_failed", "failed to create shape", None),
        }
    }

    /// Delete a shape by ID
    pub fn delete_shape(&mut self, id: u32) -> bool {
        self.inner.delete_shape(id)
    }

    pub fn delete_shape_res(&mut self, id: u32) -> JsValue {
        if self.inner.get_shape(id).is_none() {
            return error::invalid_id("shape", id);
        }
        error::ok(JsValue::from_bool(self.inner.delete_shape(id)))
    }

    /// Get all shape IDs
    pub fn get_shape_ids(&self) -> Uint32Array {
        let ids = self.inner.get_shape_ids();
        crate::interop::arr_u32(&ids)
    }

    /// Get the edge IDs for a shape
    pub fn get_shape_edges(&self, id: u32) -> JsValue {
        match self.inner.get_shape_edges(id) {
            Some(edges) => {
                let arr = crate::interop::arr_u32(edges);
                arr.into()
            }
            None => JsValue::NULL,
        }
    }

    pub fn get_shape_edges_res(&self, id: u32) -> JsValue {
        match self.inner.get_shape_edges(id) {
            Some(edges) => {
                let arr = crate::interop::arr_u32(edges);
                error::ok(arr.into())
            }
            None => error::invalid_id("shape", id),
        }
    }

    /// Get number of shapes
    pub fn shape_count(&self) -> u32 {
        self.inner.shape_count()
    }

    /// Infer shapes from closed loops in the graph
    pub fn infer_shapes(&mut self) -> Uint32Array {
        let ids = self.inner.infer_shapes();
        crate::interop::arr_u32(&ids)
    }

    pub fn infer_shapes_res(&mut self) -> JsValue {
        let ids = self.inner.infer_shapes();
        error::ok(crate::interop::arr_u32(&ids).into())
    }

    /// Set the fill rule for a shape (0 = NonZero, 1 = EvenOdd)
    pub fn set_shape_fill_rule(&mut self, id: u32, rule: u8) -> bool {
        let fill_rule = match rule {
            0 => contour::model::FillRule::NonZero,
            1 => contour::model::FillRule::EvenOdd,
            _ => return false,
        };
        self.inner.set_shape_fill_rule(id, fill_rule)
    }

    pub fn set_shape_fill_rule_res(&mut self, id: u32, rule: u8) -> JsValue {
        if self.inner.get_shape(id).is_none() {
            return error::invalid_id("shape", id);
        }
        if rule > 1 {
            return error::err("invalid_fill_rule", "fill rule must be 0 or 1", None);
        }
        let fill_rule = if rule == 0 {
            contour::model::FillRule::NonZero
        } else {
            contour::model::FillRule::EvenOdd
        };
        error::ok(JsValue::from_bool(
            self.inner.set_shape_fill_rule(id, fill_rule),
        ))
    }

    // ========== Boolean Operations ==========

    /// Perform union of two shapes (A ∪ B)
    pub fn boolean_union(&mut self, shape_a: u32, shape_b: u32) -> JsValue {
        self.boolean_op_impl(shape_a, shape_b, contour::algorithms::boolean::BoolOp::Union)
    }

    pub fn boolean_union_res(&mut self, shape_a: u32, shape_b: u32) -> JsValue {
        self.boolean_op_res_impl(shape_a, shape_b, contour::algorithms::boolean::BoolOp::Union)
    }

    /// Perform intersection of two shapes (A ∩ B)
    pub fn boolean_intersect(&mut self, shape_a: u32, shape_b: u32) -> JsValue {
        self.boolean_op_impl(
            shape_a,
            shape_b,
            contour::algorithms::boolean::BoolOp::Intersect,
        )
    }

    pub fn boolean_intersect_res(&mut self, shape_a: u32, shape_b: u32) -> JsValue {
        self.boolean_op_res_impl(
            shape_a,
            shape_b,
            contour::algorithms::boolean::BoolOp::Intersect,
        )
    }

    /// Perform difference of two shapes (A - B)
    pub fn boolean_difference(&mut self, shape_a: u32, shape_b: u32) -> JsValue {
        self.boolean_op_impl(
            shape_a,
            shape_b,
            contour::algorithms::boolean::BoolOp::Difference,
        )
    }

    pub fn boolean_difference_res(&mut self, shape_a: u32, shape_b: u32) -> JsValue {
        self.boolean_op_res_impl(
            shape_a,
            shape_b,
            contour::algorithms::boolean::BoolOp::Difference,
        )
    }

    /// Perform XOR of two shapes (A ⊕ B)
    pub fn boolean_xor(&mut self, shape_a: u32, shape_b: u32) -> JsValue {
        self.boolean_op_impl(shape_a, shape_b, contour::algorithms::boolean::BoolOp::Xor)
    }

    pub fn boolean_xor_res(&mut self, shape_a: u32, shape_b: u32) -> JsValue {
        self.boolean_op_res_impl(shape_a, shape_b, contour::algorithms::boolean::BoolOp::Xor)
    }

    fn boolean_op_impl(
        &mut self,
        shape_a: u32,
        shape_b: u32,
        op: contour::algorithms::boolean::BoolOp,
    ) -> JsValue {
        match self.inner.boolean_op(shape_a, shape_b, op) {
            Ok(result) => {
                serde_wasm_bindgen::to_value(&serde_json::json!({
                    "shapes": result.shapes,
                    "nodes": result.nodes,
                    "edges": result.edges
                }))
                .unwrap()
            }
            Err(_) => JsValue::NULL,
        }
    }

    fn boolean_op_res_impl(
        &mut self,
        shape_a: u32,
        shape_b: u32,
        op: contour::algorithms::boolean::BoolOp,
    ) -> JsValue {
        // Validate inputs
        if self.inner.get_shape(shape_a).is_none() {
            return error::invalid_id("shape", shape_a);
        }
        if self.inner.get_shape(shape_b).is_none() {
            return error::invalid_id("shape", shape_b);
        }

        match self.inner.boolean_op(shape_a, shape_b, op) {
            Ok(result) => {
                error::ok(
                    serde_wasm_bindgen::to_value(&serde_json::json!({
                        "shapes": result.shapes,
                        "nodes": result.nodes,
                        "edges": result.edges
                    }))
                    .unwrap(),
                )
            }
            Err(e) => {
                let msg = format!("{:?}", e);
                error::err("boolean_op_failed", &msg, None)
            }
        }
    }

    // ========== Text Management ==========

    /// Add a simple text label at the specified position
    pub fn add_text(&mut self, content: &str, x: f32, y: f32) -> u32 {
        self.inner.add_text(content, x, y)
    }

    pub fn add_text_res(&mut self, content: &str, x: f32, y: f32) -> JsValue {
        if !x.is_finite() {
            return error::non_finite("x");
        }
        if !y.is_finite() {
            return error::non_finite("y");
        }
        let id = self.inner.add_text(content, x, y);
        error::ok(JsValue::from_f64(id as f64))
    }

    /// Add a text box with wrapping
    pub fn add_text_box(&mut self, content: &str, x: f32, y: f32, width: f32, height: f32) -> u32 {
        self.inner.add_text_box(content, x, y, width, height)
    }

    pub fn add_text_box_res(&mut self, content: &str, x: f32, y: f32, width: f32, height: f32) -> JsValue {
        for (n, v) in [("x", x), ("y", y), ("width", width), ("height", height)] {
            if !v.is_finite() {
                return error::non_finite(n);
            }
        }
        if width <= 0.0 {
            return error::out_of_range("width", 0.0, f32::INFINITY, width);
        }
        if height <= 0.0 {
            return error::out_of_range("height", 0.0, f32::INFINITY, height);
        }
        let id = self.inner.add_text_box(content, x, y, width, height);
        error::ok(JsValue::from_f64(id as f64))
    }

    /// Add text on a path defined by edge IDs
    pub fn add_text_on_path(&mut self, content: &str, edge_ids: &Uint32Array) -> u32 {
        let len = edge_ids.length() as usize;
        let mut ids = vec![0u32; len];
        edge_ids.copy_to(&mut ids);
        self.inner.add_text_on_path(content, ids)
    }

    pub fn add_text_on_path_res(&mut self, content: &str, edge_ids: &Uint32Array) -> JsValue {
        let len = edge_ids.length() as usize;
        let mut ids = vec![0u32; len];
        edge_ids.copy_to(&mut ids);
        // Validate edge IDs
        for &id in &ids {
            if !edge_exists(&self.inner, id) {
                return error::invalid_id("edge", id);
            }
        }
        let text_id = self.inner.add_text_on_path(content, ids);
        error::ok(JsValue::from_f64(text_id as f64))
    }

    /// Remove a text element
    pub fn remove_text(&mut self, id: u32) -> bool {
        self.inner.remove_text(id)
    }

    pub fn remove_text_res(&mut self, id: u32) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        error::ok(JsValue::from_bool(self.inner.remove_text(id)))
    }

    /// Get all text IDs
    pub fn get_text_ids(&self) -> Uint32Array {
        let ids = self.inner.get_text_ids();
        crate::interop::arr_u32(&ids)
    }

    /// Get text element count
    pub fn text_count(&self) -> u32 {
        self.inner.text_count()
    }

    /// Get text element by ID (returns full JSON object)
    pub fn get_text(&self, id: u32) -> JsValue {
        match self.inner.get_text(id) {
            Some(text) => serde_wasm_bindgen::to_value(text).unwrap_or(JsValue::NULL),
            None => JsValue::NULL,
        }
    }

    pub fn get_text_res(&self, id: u32) -> JsValue {
        match self.inner.get_text(id) {
            Some(text) => error::ok(serde_wasm_bindgen::to_value(text).unwrap_or(JsValue::NULL)),
            None => error::invalid_id("text", id),
        }
    }

    /// Get all text elements as array of JSON objects
    pub fn get_all_texts(&self) -> JsValue {
        let ids = self.inner.get_text_ids();
        let texts: Vec<_> = ids
            .iter()
            .filter_map(|&id| self.inner.get_text(id))
            .collect();
        serde_wasm_bindgen::to_value(&texts).unwrap_or(JsValue::NULL)
    }

    /// Set text content
    pub fn set_text_content(&mut self, id: u32, content: &str) -> bool {
        self.inner.set_text_content(id, content)
    }

    pub fn set_text_content_res(&mut self, id: u32, content: &str) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        error::ok(JsValue::from_bool(self.inner.set_text_content(id, content)))
    }

    /// Set text position
    pub fn set_text_position(&mut self, id: u32, x: f32, y: f32) -> bool {
        self.inner.set_text_position(id, x, y)
    }

    pub fn set_text_position_res(&mut self, id: u32, x: f32, y: f32) -> JsValue {
        if !x.is_finite() {
            return error::non_finite("x");
        }
        if !y.is_finite() {
            return error::non_finite("y");
        }
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        error::ok(JsValue::from_bool(self.inner.set_text_position(id, x, y)))
    }

    /// Set text rotation (in radians)
    pub fn set_text_rotation(&mut self, id: u32, radians: f32) -> bool {
        self.inner.set_text_rotation(id, radians)
    }

    pub fn set_text_rotation_res(&mut self, id: u32, radians: f32) -> JsValue {
        if !radians.is_finite() {
            return error::non_finite("radians");
        }
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        error::ok(JsValue::from_bool(self.inner.set_text_rotation(id, radians)))
    }

    /// Set text alignment (0 = Left, 1 = Center, 2 = Right)
    pub fn set_text_align(&mut self, id: u32, align: u8) -> bool {
        let align = match align {
            0 => contour::model::TextAlign::Left,
            1 => contour::model::TextAlign::Center,
            2 => contour::model::TextAlign::Right,
            _ => return false,
        };
        self.inner.set_text_align(id, align)
    }

    pub fn set_text_align_res(&mut self, id: u32, align: u8) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        if align > 2 {
            return error::err("invalid_align", "align must be 0, 1, or 2", None);
        }
        let text_align = match align {
            0 => contour::model::TextAlign::Left,
            1 => contour::model::TextAlign::Center,
            _ => contour::model::TextAlign::Right,
        };
        error::ok(JsValue::from_bool(self.inner.set_text_align(id, text_align)))
    }

    /// Set font family and size
    pub fn set_text_font(&mut self, id: u32, font_family: &str, font_size: f32) -> bool {
        self.inner.set_text_font(id, font_family, font_size)
    }

    pub fn set_text_font_res(&mut self, id: u32, font_family: &str, font_size: f32) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        if !font_size.is_finite() {
            return error::non_finite("font_size");
        }
        if font_size <= 0.0 {
            return error::out_of_range("font_size", 0.0, f32::INFINITY, font_size);
        }
        error::ok(JsValue::from_bool(
            self.inner.set_text_font(id, font_family, font_size),
        ))
    }

    /// Set font weight (100-900)
    pub fn set_text_font_weight(&mut self, id: u32, weight: u16) -> bool {
        self.inner.set_text_font_weight(id, weight)
    }

    pub fn set_text_font_weight_res(&mut self, id: u32, weight: u16) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        if weight < 100 || weight > 900 {
            return error::err("invalid_weight", "weight must be 100-900", None);
        }
        error::ok(JsValue::from_bool(
            self.inner.set_text_font_weight(id, weight),
        ))
    }

    /// Set font style (0 = Normal, 1 = Italic, 2 = Oblique)
    pub fn set_text_font_style(&mut self, id: u32, style: u8) -> bool {
        let font_style = match style {
            0 => contour::model::FontStyle::Normal,
            1 => contour::model::FontStyle::Italic,
            2 => contour::model::FontStyle::Oblique,
            _ => return false,
        };
        self.inner.set_text_font_style(id, font_style)
    }

    pub fn set_text_font_style_res(&mut self, id: u32, style: u8) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        if style > 2 {
            return error::err("invalid_style", "style must be 0, 1, or 2", None);
        }
        let font_style = match style {
            0 => contour::model::FontStyle::Normal,
            1 => contour::model::FontStyle::Italic,
            _ => contour::model::FontStyle::Oblique,
        };
        error::ok(JsValue::from_bool(
            self.inner.set_text_font_style(id, font_style),
        ))
    }

    /// Set text fill color
    pub fn set_text_fill_color(&mut self, id: u32, r: u8, g: u8, b: u8, a: u8) -> bool {
        self.inner.set_text_fill_color(id, r, g, b, a)
    }

    pub fn set_text_fill_color_res(&mut self, id: u32, r: u8, g: u8, b: u8, a: u8) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        error::ok(JsValue::from_bool(
            self.inner.set_text_fill_color(id, r, g, b, a),
        ))
    }

    /// Clear text fill color (make transparent)
    pub fn clear_text_fill_color(&mut self, id: u32) -> bool {
        self.inner.clear_text_fill_color(id)
    }

    /// Set text stroke color
    pub fn set_text_stroke_color(&mut self, id: u32, r: u8, g: u8, b: u8, a: u8) -> bool {
        self.inner.set_text_stroke_color(id, r, g, b, a)
    }

    pub fn set_text_stroke_color_res(&mut self, id: u32, r: u8, g: u8, b: u8, a: u8) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        error::ok(JsValue::from_bool(
            self.inner.set_text_stroke_color(id, r, g, b, a),
        ))
    }

    /// Set text stroke width
    pub fn set_text_stroke_width(&mut self, id: u32, width: f32) -> bool {
        self.inner.set_text_stroke_width(id, width)
    }

    pub fn set_text_stroke_width_res(&mut self, id: u32, width: f32) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        if !width.is_finite() {
            return error::non_finite("width");
        }
        if width < 0.0 {
            return error::out_of_range("width", 0.0, f32::INFINITY, width);
        }
        error::ok(JsValue::from_bool(
            self.inner.set_text_stroke_width(id, width),
        ))
    }

    /// Set letter spacing (in em units)
    pub fn set_text_letter_spacing(&mut self, id: u32, spacing: f32) -> bool {
        self.inner.set_text_letter_spacing(id, spacing)
    }

    pub fn set_text_letter_spacing_res(&mut self, id: u32, spacing: f32) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        if !spacing.is_finite() {
            return error::non_finite("spacing");
        }
        error::ok(JsValue::from_bool(
            self.inner.set_text_letter_spacing(id, spacing),
        ))
    }

    /// Set line height multiplier
    pub fn set_text_line_height(&mut self, id: u32, line_height: f32) -> bool {
        self.inner.set_text_line_height(id, line_height)
    }

    pub fn set_text_line_height_res(&mut self, id: u32, line_height: f32) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        if !line_height.is_finite() {
            return error::non_finite("line_height");
        }
        if line_height < 0.1 {
            return error::out_of_range("line_height", 0.1, f32::INFINITY, line_height);
        }
        error::ok(JsValue::from_bool(
            self.inner.set_text_line_height(id, line_height),
        ))
    }

    /// Convert text to a text box
    pub fn convert_text_to_box(&mut self, id: u32, width: f32, height: f32) -> bool {
        self.inner.convert_text_to_box(id, width, height)
    }

    pub fn convert_text_to_box_res(&mut self, id: u32, width: f32, height: f32) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        if !width.is_finite() || !height.is_finite() {
            return error::non_finite("dimensions");
        }
        if width <= 0.0 || height <= 0.0 {
            return error::err("invalid_dimensions", "width and height must be positive", None);
        }
        error::ok(JsValue::from_bool(
            self.inner.convert_text_to_box(id, width, height),
        ))
    }

    /// Convert text to text on path
    pub fn convert_text_to_on_path(&mut self, id: u32, edge_ids: &Uint32Array, start_offset: f32) -> bool {
        let len = edge_ids.length() as usize;
        let mut ids = vec![0u32; len];
        edge_ids.copy_to(&mut ids);
        self.inner.convert_text_to_on_path(id, ids, start_offset)
    }

    pub fn convert_text_to_on_path_res(&mut self, id: u32, edge_ids: &Uint32Array, start_offset: f32) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        let len = edge_ids.length() as usize;
        let mut ids = vec![0u32; len];
        edge_ids.copy_to(&mut ids);
        for &eid in &ids {
            if !edge_exists(&self.inner, eid) {
                return error::invalid_id("edge", eid);
            }
        }
        if !start_offset.is_finite() {
            return error::non_finite("start_offset");
        }
        error::ok(JsValue::from_bool(
            self.inner.convert_text_to_on_path(id, ids, start_offset),
        ))
    }

    /// Convert text back to simple label
    pub fn convert_text_to_label(&mut self, id: u32) -> bool {
        self.inner.convert_text_to_label(id)
    }

    pub fn convert_text_to_label_res(&mut self, id: u32) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        error::ok(JsValue::from_bool(self.inner.convert_text_to_label(id)))
    }

    /// Set text box size (only for text box type)
    pub fn set_text_box_size(&mut self, id: u32, width: f32, height: f32) -> bool {
        self.inner.set_text_box_size(id, width, height)
    }

    pub fn set_text_box_size_res(&mut self, id: u32, width: f32, height: f32) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        if !width.is_finite() || !height.is_finite() {
            return error::non_finite("dimensions");
        }
        if width <= 0.0 || height <= 0.0 {
            return error::err("invalid_dimensions", "width and height must be positive", None);
        }
        error::ok(JsValue::from_bool(
            self.inner.set_text_box_size(id, width, height),
        ))
    }

    /// Set text box vertical alignment (0 = Top, 1 = Middle, 2 = Bottom)
    pub fn set_text_box_vertical_align(&mut self, id: u32, align: u8) -> bool {
        let valign = match align {
            0 => contour::model::VerticalAlign::Top,
            1 => contour::model::VerticalAlign::Middle,
            2 => contour::model::VerticalAlign::Bottom,
            _ => return false,
        };
        self.inner.set_text_box_vertical_align(id, valign)
    }

    pub fn set_text_box_vertical_align_res(&mut self, id: u32, align: u8) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        if align > 2 {
            return error::err("invalid_align", "align must be 0, 1, or 2", None);
        }
        let valign = match align {
            0 => contour::model::VerticalAlign::Top,
            1 => contour::model::VerticalAlign::Middle,
            _ => contour::model::VerticalAlign::Bottom,
        };
        error::ok(JsValue::from_bool(
            self.inner.set_text_box_vertical_align(id, valign),
        ))
    }

    /// Set text box overflow behavior (0 = Clip, 1 = Ellipsis, 2 = Visible)
    pub fn set_text_box_overflow(&mut self, id: u32, overflow: u8) -> bool {
        let ovf = match overflow {
            0 => contour::model::TextOverflow::Clip,
            1 => contour::model::TextOverflow::Ellipsis,
            2 => contour::model::TextOverflow::Visible,
            _ => return false,
        };
        self.inner.set_text_box_overflow(id, ovf)
    }

    pub fn set_text_box_overflow_res(&mut self, id: u32, overflow: u8) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        if overflow > 2 {
            return error::err("invalid_overflow", "overflow must be 0, 1, or 2", None);
        }
        let ovf = match overflow {
            0 => contour::model::TextOverflow::Clip,
            1 => contour::model::TextOverflow::Ellipsis,
            _ => contour::model::TextOverflow::Visible,
        };
        error::ok(JsValue::from_bool(
            self.inner.set_text_box_overflow(id, ovf),
        ))
    }

    /// Set text on path start offset (0.0 to 1.0)
    pub fn set_text_path_offset(&mut self, id: u32, offset: f32) -> bool {
        self.inner.set_text_path_offset(id, offset)
    }

    pub fn set_text_path_offset_res(&mut self, id: u32, offset: f32) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        if !offset.is_finite() {
            return error::non_finite("offset");
        }
        error::ok(JsValue::from_bool(
            self.inner.set_text_path_offset(id, offset),
        ))
    }

    /// Set the edges for text on path
    pub fn set_text_path_edges(&mut self, id: u32, edge_ids: &Uint32Array) -> bool {
        let len = edge_ids.length() as usize;
        let mut ids = vec![0u32; len];
        edge_ids.copy_to(&mut ids);
        self.inner.set_text_path_edges(id, ids)
    }

    pub fn set_text_path_edges_res(&mut self, id: u32, edge_ids: &Uint32Array) -> JsValue {
        if self.inner.get_text(id).is_none() {
            return error::invalid_id("text", id);
        }
        let len = edge_ids.length() as usize;
        let mut ids = vec![0u32; len];
        edge_ids.copy_to(&mut ids);
        for &eid in &ids {
            if !edge_exists(&self.inner, eid) {
                return error::invalid_id("edge", eid);
            }
        }
        error::ok(JsValue::from_bool(
            self.inner.set_text_path_edges(id, ids),
        ))
    }

    /// Convert text to vector outlines using glyph data from JavaScript.
    ///
    /// The glyph_data should be an array of objects:
    /// [{char: "A", advance_width: 500, paths: [{commands: [...]}]}]
    ///
    /// Each command can be:
    /// - {type: "moveTo", x: 0, y: 0}
    /// - {type: "lineTo", x: 100, y: 100}
    /// - {type: "quadTo", cx: 50, cy: 50, x: 100, y: 100}
    /// - {type: "cubicTo", c1x: 25, c1y: 25, c2x: 75, c2y: 75, x: 100, y: 100}
    /// - {type: "close"}
    pub fn text_to_outlines(&mut self, text_id: u32, glyph_data: &JsValue) -> JsValue {
        let glyphs = parse_glyph_data(glyph_data);
        match self.inner.text_to_outlines(text_id, &glyphs) {
            Some(result) => serde_wasm_bindgen::to_value(&serde_json::json!({
                "shapes": result.shapes,
                "nodes": result.nodes,
                "edges": result.edges
            }))
            .unwrap_or(JsValue::NULL),
            None => JsValue::NULL,
        }
    }

    pub fn text_to_outlines_res(&mut self, text_id: u32, glyph_data: &JsValue) -> JsValue {
        if self.inner.get_text(text_id).is_none() {
            return error::invalid_id("text", text_id);
        }
        let glyphs = parse_glyph_data(glyph_data);
        match self.inner.text_to_outlines(text_id, &glyphs) {
            Some(result) => error::ok(
                serde_wasm_bindgen::to_value(&serde_json::json!({
                    "shapes": result.shapes,
                    "nodes": result.nodes,
                    "edges": result.edges
                }))
                .unwrap_or(JsValue::NULL),
            ),
            None => error::err("outline_failed", "failed to convert text to outlines", None),
        }
    }

    // ========== Path Operations (for text on path) ==========

    /// Calculate the total length of a path defined by edge IDs.
    pub fn path_length(&self, edge_ids: &Uint32Array) -> f32 {
        let len = edge_ids.length() as usize;
        let mut ids = vec![0u32; len];
        edge_ids.copy_to(&mut ids);
        self.inner.path_length(&ids)
    }

    pub fn path_length_res(&self, edge_ids: &Uint32Array) -> JsValue {
        let len = edge_ids.length() as usize;
        let mut ids = vec![0u32; len];
        edge_ids.copy_to(&mut ids);
        // Validate edges exist
        for &id in &ids {
            if !edge_exists(&self.inner, id) {
                return error::invalid_id("edge", id);
            }
        }
        error::ok(JsValue::from_f64(self.inner.path_length(&ids) as f64))
    }

    /// Get a point at a specific distance along a path.
    /// Returns {x, y, angle} or null if invalid.
    pub fn point_on_path(&self, edge_ids: &Uint32Array, distance: f32) -> JsValue {
        let len = edge_ids.length() as usize;
        let mut ids = vec![0u32; len];
        edge_ids.copy_to(&mut ids);
        match self.inner.point_on_path(&ids, distance) {
            Some(point) => serde_wasm_bindgen::to_value(&serde_json::json!({
                "x": point.x,
                "y": point.y,
                "angle": point.angle
            }))
            .unwrap_or(JsValue::NULL),
            None => JsValue::NULL,
        }
    }

    pub fn point_on_path_res(&self, edge_ids: &Uint32Array, distance: f32) -> JsValue {
        if !distance.is_finite() {
            return error::non_finite("distance");
        }
        let len = edge_ids.length() as usize;
        let mut ids = vec![0u32; len];
        edge_ids.copy_to(&mut ids);
        for &id in &ids {
            if !edge_exists(&self.inner, id) {
                return error::invalid_id("edge", id);
            }
        }
        match self.inner.point_on_path(&ids, distance) {
            Some(point) => error::ok(
                serde_wasm_bindgen::to_value(&serde_json::json!({
                    "x": point.x,
                    "y": point.y,
                    "angle": point.angle
                }))
                .unwrap_or(JsValue::NULL),
            ),
            None => error::err("point_not_found", "could not find point on path", None),
        }
    }

    /// Sample text positions along a path.
    /// char_widths is a Float32Array of character widths.
    /// Returns array of {x, y, angle} for each character.
    pub fn sample_text_positions(
        &self,
        edge_ids: &Uint32Array,
        char_widths: &Float32Array,
        start_offset: f32,
    ) -> JsValue {
        let edge_len = edge_ids.length() as usize;
        let mut ids = vec![0u32; edge_len];
        edge_ids.copy_to(&mut ids);

        let widths_len = char_widths.length() as usize;
        let mut widths = vec![0.0f32; widths_len];
        char_widths.copy_to(&mut widths);

        let positions = self.inner.sample_text_positions(&ids, &widths, start_offset);
        let result: Vec<_> = positions
            .iter()
            .map(|p| {
                serde_json::json!({
                    "x": p.x,
                    "y": p.y,
                    "angle": p.angle
                })
            })
            .collect();
        serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }

    pub fn sample_text_positions_res(
        &self,
        edge_ids: &Uint32Array,
        char_widths: &Float32Array,
        start_offset: f32,
    ) -> JsValue {
        if !start_offset.is_finite() {
            return error::non_finite("start_offset");
        }
        let edge_len = edge_ids.length() as usize;
        let mut ids = vec![0u32; edge_len];
        edge_ids.copy_to(&mut ids);
        for &id in &ids {
            if !edge_exists(&self.inner, id) {
                return error::invalid_id("edge", id);
            }
        }

        let widths_len = char_widths.length() as usize;
        let mut widths = vec![0.0f32; widths_len];
        char_widths.copy_to(&mut widths);
        if widths.iter().any(|w| !w.is_finite()) {
            return error::non_finite("char_widths");
        }

        let positions = self.inner.sample_text_positions(&ids, &widths, start_offset);
        let result: Vec<_> = positions
            .iter()
            .map(|p| {
                serde_json::json!({
                    "x": p.x,
                    "y": p.y,
                    "angle": p.angle
                })
            })
            .collect();
        error::ok(serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL))
    }

    /// Layout text content into a box with line wrapping.
    /// char_widths is a Float32Array of character widths from JS font measurement.
    /// Returns {lines: [{text, x_offset, y_offset, width}], total_height, truncated}
    pub fn layout_text_box(
        &self,
        content: &str,
        width: f32,
        height: f32,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
        char_widths: &Float32Array,
        align: u8,
        vertical_align: u8,
    ) -> JsValue {
        use contour::algorithms::text_layout::layout_text_box;
        use contour::model::{TextAlign, TextStyle, VerticalAlign, FontStyle};

        let style = TextStyle {
            font_family: String::new(),
            font_size,
            font_weight: 400,
            font_style: FontStyle::Normal,
            fill_color: None,
            stroke_color: None,
            stroke_width: 0.0,
            letter_spacing,
            line_height,
        };

        let text_align = match align {
            1 => TextAlign::Center,
            2 => TextAlign::Right,
            _ => TextAlign::Left,
        };

        let vert_align = match vertical_align {
            1 => VerticalAlign::Middle,
            2 => VerticalAlign::Bottom,
            _ => VerticalAlign::Top,
        };

        let widths_len = char_widths.length() as usize;
        let mut widths = vec![0.0f32; widths_len];
        char_widths.copy_to(&mut widths);

        let layout = layout_text_box(content, width, height, &style, &widths, text_align, vert_align);

        let lines: Vec<_> = layout.lines.iter().map(|l| {
            serde_json::json!({
                "text": l.text,
                "x_offset": l.x_offset,
                "y_offset": l.y_offset,
                "width": l.width
            })
        }).collect();

        serde_wasm_bindgen::to_value(&serde_json::json!({
            "lines": lines,
            "total_height": layout.total_height,
            "truncated": layout.truncated
        })).unwrap_or(JsValue::NULL)
    }

    pub fn layout_text_box_res(
        &self,
        content: &str,
        width: f32,
        height: f32,
        font_size: f32,
        line_height: f32,
        letter_spacing: f32,
        char_widths: &Float32Array,
        align: u8,
        vertical_align: u8,
    ) -> JsValue {
        use contour::algorithms::text_layout::layout_text_box;
        use contour::model::{TextAlign, TextStyle, VerticalAlign, FontStyle};

        if !width.is_finite() {
            return error::non_finite("width");
        }
        if !height.is_finite() {
            return error::non_finite("height");
        }
        if !font_size.is_finite() || font_size <= 0.0 {
            return error::err("INVALID_FONT_SIZE", "font_size must be positive and finite", None);
        }
        if !line_height.is_finite() || line_height <= 0.0 {
            return error::err("INVALID_LINE_HEIGHT", "line_height must be positive and finite", None);
        }
        if !letter_spacing.is_finite() {
            return error::non_finite("letter_spacing");
        }

        let style = TextStyle {
            font_family: String::new(),
            font_size,
            font_weight: 400,
            font_style: FontStyle::Normal,
            fill_color: None,
            stroke_color: None,
            stroke_width: 0.0,
            letter_spacing,
            line_height,
        };

        let text_align = match align {
            1 => TextAlign::Center,
            2 => TextAlign::Right,
            _ => TextAlign::Left,
        };

        let vert_align = match vertical_align {
            1 => VerticalAlign::Middle,
            2 => VerticalAlign::Bottom,
            _ => VerticalAlign::Top,
        };

        let widths_len = char_widths.length() as usize;
        let mut widths = vec![0.0f32; widths_len];
        char_widths.copy_to(&mut widths);

        if widths.iter().any(|w| !w.is_finite()) {
            return error::non_finite("char_widths");
        }

        let layout = layout_text_box(content, width, height, &style, &widths, text_align, vert_align);

        let lines: Vec<_> = layout.lines.iter().map(|l| {
            serde_json::json!({
                "text": l.text,
                "x_offset": l.x_offset,
                "y_offset": l.y_offset,
                "width": l.width
            })
        }).collect();

        error::ok(serde_wasm_bindgen::to_value(&serde_json::json!({
            "lines": lines,
            "total_height": layout.total_height,
            "truncated": layout.truncated
        })).unwrap_or(JsValue::NULL))
    }
}

fn to_pairs(arr: &Float32Array) -> Vec<(f32, f32)> {
    let len = arr.length() as usize;
    let mut buf = vec![0.0f32; len];
    arr.copy_to(&mut buf);
    let mut out = Vec::with_capacity(len / 2);
    let mut i = 0;
    while i + 1 < len {
        out.push((buf[i], buf[i + 1]));
        i += 2;
    }
    out
}
fn edge_exists(g: &contour::Graph, id: u32) -> bool {
    let ea = g.get_edge_arrays();
    ea.ids.iter().any(|&x| x == id)
}

fn region_exists(g: &mut contour::Graph, key: u32) -> bool {
    let regs = g.get_regions();
    for v in regs {
        if let Some(k) = v.get("key").and_then(|x| x.as_u64()) {
            if k as u32 == key {
                return true;
            }
        }
    }
    false
}

/// Parse color stops from JsValue array
/// Expected format: [{offset: f32, color: {r: u8, g: u8, b: u8, a: u8}}, ...]
fn parse_color_stops(stops: &JsValue) -> Vec<contour::model::ColorStop> {
    #[derive(serde::Deserialize)]
    struct ColorStopJs {
        offset: f32,
        color: ColorJs,
    }
    #[derive(serde::Deserialize)]
    struct ColorJs {
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    }

    let stops_arr: Vec<ColorStopJs> = serde_wasm_bindgen::from_value(stops.clone()).unwrap_or_default();
    stops_arr
        .into_iter()
        .map(|s| contour::model::ColorStop {
            offset: s.offset.clamp(0.0, 1.0),
            color: contour::model::Color {
                r: s.color.r,
                g: s.color.g,
                b: s.color.b,
                a: s.color.a,
            },
        })
        .collect()
}

/// Parse gradient units from u8 (0 = ObjectBoundingBox, 1 = UserSpaceOnUse)
fn parse_gradient_units(units: u8) -> contour::model::GradientUnits {
    match units {
        1 => contour::model::GradientUnits::UserSpaceOnUse,
        _ => contour::model::GradientUnits::ObjectBoundingBox,
    }
}

/// Parse spread method from u8 (0 = Pad, 1 = Reflect, 2 = Repeat)
fn parse_spread_method(spread: u8) -> contour::model::SpreadMethod {
    match spread {
        1 => contour::model::SpreadMethod::Reflect,
        2 => contour::model::SpreadMethod::Repeat,
        _ => contour::model::SpreadMethod::Pad,
    }
}

/// Parse glyph data from JavaScript for text-to-outlines conversion
fn parse_glyph_data(data: &JsValue) -> Vec<contour::model::GlyphOutline> {
    #[derive(serde::Deserialize)]
    struct GlyphJs {
        char: Option<String>,
        advance_width: f32,
        paths: Vec<PathJs>,
    }
    #[derive(serde::Deserialize)]
    struct PathJs {
        commands: Vec<CommandJs>,
    }
    #[derive(serde::Deserialize)]
    #[serde(tag = "type", rename_all = "camelCase")]
    enum CommandJs {
        MoveTo { x: f32, y: f32 },
        LineTo { x: f32, y: f32 },
        QuadTo { cx: f32, cy: f32, x: f32, y: f32 },
        CubicTo { c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32 },
        Close,
    }

    let glyphs: Vec<GlyphJs> = serde_wasm_bindgen::from_value(data.clone()).unwrap_or_default();

    glyphs
        .into_iter()
        .map(|g| contour::model::GlyphOutline {
            char: g.char.and_then(|s| s.chars().next()).unwrap_or('?'),
            advance_width: g.advance_width,
            paths: g
                .paths
                .into_iter()
                .map(|p| contour::model::GlyphPath {
                    commands: p
                        .commands
                        .into_iter()
                        .map(|c| match c {
                            CommandJs::MoveTo { x, y } => contour::model::PathCommand::MoveTo(x, y),
                            CommandJs::LineTo { x, y } => contour::model::PathCommand::LineTo(x, y),
                            CommandJs::QuadTo { cx, cy, x, y } => {
                                contour::model::PathCommand::QuadTo(cx, cy, x, y)
                            }
                            CommandJs::CubicTo { c1x, c1y, c2x, c2y, x, y } => {
                                contour::model::PathCommand::CubicTo(c1x, c1y, c2x, c2y, x, y)
                            }
                            CommandJs::Close => contour::model::PathCommand::Close,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}
