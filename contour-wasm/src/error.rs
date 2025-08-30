use wasm_bindgen::prelude::*;
use js_sys::{Object, Reflect};

fn set_kv(obj: &Object, k: &str, v: &JsValue) { let _ = Reflect::set(obj, &JsValue::from_str(k), v); }

fn new_obj() -> Object { Object::new() }

pub fn ok(v: JsValue) -> JsValue {
    let o = new_obj();
    set_kv(&o, "ok", &JsValue::from_bool(true));
    set_kv(&o, "value", &v);
    o.into()
}

pub fn err(code: &'static str, message: impl Into<String>, data: Option<JsValue>) -> JsValue {
    let root = new_obj();
    set_kv(&root, "ok", &JsValue::from_bool(false));
    let e = new_obj();
    set_kv(&e, "code", &JsValue::from_str(code));
    set_kv(&e, "message", &JsValue::from_str(&message.into()));
    if let Some(d) = data { set_kv(&e, "data", &d); }
    set_kv(&root, "error", &e.into());
    root.into()
}

#[inline]
pub fn non_finite(param: &str) -> JsValue {
    let d = new_obj(); set_kv(&d, "param", &JsValue::from_str(param));
    err("non_finite", format!("parameter '{}' must be finite", param), Some(d.into()))
}

#[inline]
pub fn out_of_range(param: &str, min: f32, max: f32, got: f32) -> JsValue {
    let d = new_obj();
    set_kv(&d, "param", &JsValue::from_str(param));
    set_kv(&d, "min", &JsValue::from_f64(min as f64));
    set_kv(&d, "max", &JsValue::from_f64(max as f64));
    set_kv(&d, "got", &JsValue::from_f64(got as f64));
    err("out_of_range", format!("parameter '{}' out of range", param), Some(d.into()))
}

#[inline]
pub fn invalid_id(kind: &str, id: u32) -> JsValue {
    let d = new_obj();
    set_kv(&d, "kind", &JsValue::from_str(kind));
    set_kv(&d, "id", &JsValue::from_f64(id as f64));
    err("invalid_id", format!("invalid {} id", kind), Some(d.into()))
}

#[inline]
pub fn invalid_mode(got: u8) -> JsValue {
    let d = new_obj(); set_kv(&d, "got", &JsValue::from_f64(got as f64));
    err("invalid_mode", "mode must be 0:Free, 1:Mirrored, 2:Aligned", Some(d.into()))
}

#[inline]
pub fn not_cubic(edge: u32) -> JsValue { invalid_kind("not_cubic", "edge is not cubic", edge) }

#[inline]
pub fn not_polyline(edge: u32) -> JsValue { invalid_kind("not_polyline", "edge is not polyline", edge) }

fn invalid_kind(code: &'static str, msg: &str, edge: u32) -> JsValue {
    let d = new_obj(); set_kv(&d, "edge", &JsValue::from_f64(edge as f64));
    err(code, msg.to_string(), Some(d.into()))
}

