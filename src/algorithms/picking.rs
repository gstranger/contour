use wasm_bindgen::JsValue;
use js_sys::Object;
use crate::interop::{new_obj, set_kv};
use crate::model::{EdgeKind};
use crate::geometry::math::{seg_distance_sq, cubic_distance_sq};

use crate::Graph;

pub(crate) fn pick_impl(g: &Graph, x: f32, y: f32, tol: f32) -> JsValue {
    let tol2 = tol * tol;
    // Prefer nodes when within tolerance
    let mut best_node: Option<(u32, f32)> = None; // (id, dist2)
    for (i, n) in g.nodes.iter().enumerate() {
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
        let obj = new_obj();
        set_kv(&obj, "kind", &JsValue::from_str("node"));
        set_kv(&obj, "id", &JsValue::from_f64(id as f64));
        set_kv(&obj, "dist", &JsValue::from_f64(d2.sqrt() as f64));
        return obj.into();
    }

    // Prefer handles over edges when within tolerance
    // Handles
    let mut best_handle: Option<(u32, u8, f32)> = None; // (edge_id, end 0|1, dist2)
    for (i, e) in g.edges.iter().enumerate() {
        if let Some(e) = e {
            match e.kind {
                EdgeKind::Cubic { ha, hb, .. } => {
                    let a = match g.nodes.get(e.a as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                    let b = match g.nodes.get(e.b as usize).and_then(|n| *n) { Some(n) => n, None => continue };
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
        let obj = new_obj();
        set_kv(&obj, "kind", &JsValue::from_str("handle"));
        set_kv(&obj, "edge", &JsValue::from_f64(edge_id as f64));
        set_kv(&obj, "end", &JsValue::from_f64(end as f64));
        set_kv(&obj, "dist", &JsValue::from_f64(d2.sqrt() as f64));
        return obj.into();
    }

    // Else check edges
    let mut best_edge: Option<(u32, f32, f32)> = None; // (id, dist2, t)
    for (i, e) in g.edges.iter().enumerate() {
        if let Some(e) = e {
            match e.kind {
                EdgeKind::Line => {
                    let a = match g.nodes.get(e.a as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                    let b = match g.nodes.get(e.b as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                    let (d2, t) = seg_distance_sq(x, y, a.x, a.y, b.x, b.y);
                    if d2 <= tol2 {
                        if best_edge.map_or(true, |(_, bd2, _)| d2 < bd2) {
                            best_edge = Some((i as u32, d2, t));
                        }
                    }
                }
                EdgeKind::Cubic { ha, hb, .. } => {
                    let a = match g.nodes.get(e.a as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                    let b = match g.nodes.get(e.b as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                    let p1x = a.x + ha.x; let p1y = a.y + ha.y;
                    let p2x = b.x + hb.x; let p2y = b.y + hb.y;
                    let (d2, t) = cubic_distance_sq(x, y, a.x, a.y, p1x, p1y, p2x, p2y, b.x, b.y);
                    if d2 <= tol2 {
                        if best_edge.map_or(true, |(_, bd2, _)| d2 < bd2) {
                            best_edge = Some((i as u32, d2, t));
                        }
                    }
                }
                EdgeKind::Polyline { ref points } => {
                    let a = match g.nodes.get(e.a as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                    let b = match g.nodes.get(e.b as usize).and_then(|n| *n) { Some(n) => n, None => continue };
                    // Build chain: A -> points... -> B
                    let mut prevx = a.x; let mut prevy = a.y;
                    let mut length = 0.0f32;
                    let mut segs: Vec<(f32,f32,f32,f32,f32)> = Vec::new(); // (x1,y1,x2,y2,segLen)
                    for p in points {
                        let x2 = p.x; let y2 = p.y;
                        let seg_len = ((x2 - prevx).powi(2) + (y2 - prevy).powi(2)).sqrt();
                        if seg_len > 0.0 { segs.push((prevx, prevy, x2, y2, seg_len)); length += seg_len; }
                        prevx = x2; prevy = y2;
                    }
                    let seg_len = ((b.x - prevx).powi(2) + (b.y - prevy).powi(2)).sqrt();
                    if seg_len > 0.0 { segs.push((prevx, prevy, b.x, b.y, seg_len)); length += seg_len; }
                    let mut acc = 0.0f32;
                    for (x1,y1,x2,y2,seg_len) in segs.into_iter() {
                        let (d2, tseg) = seg_distance_sq(x, y, x1, y1, x2, y2);
                        if d2 <= tol2 {
                            let t_along = if length > 0.0 { (acc + tseg*seg_len) / length } else { 0.0 };
                            if best_edge.map_or(true, |(_, bd2, _)| d2 < bd2) { best_edge = Some((i as u32, d2, t_along)); }
                        }
                        acc += seg_len;
                    }
                }
            }
        }
    }
    if let Some((id, d2, t)) = best_edge {
        let obj = new_obj();
        set_kv(&obj, "kind", &JsValue::from_str("edge"));
        set_kv(&obj, "id", &JsValue::from_f64(id as f64));
        set_kv(&obj, "t", &JsValue::from_f64(t as f64));
        set_kv(&obj, "dist", &JsValue::from_f64(d2.sqrt() as f64));
        return obj.into();
    }
    JsValue::UNDEFINED
}
