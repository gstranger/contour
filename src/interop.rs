use wasm_bindgen::prelude::*;
use js_sys::{Float32Array, Object, Uint32Array, Uint8Array, Reflect};

pub fn new_obj() -> Object { Object::new() }

pub fn set_kv(obj: &Object, key: &str, val: &JsValue) {
    let _ = Reflect::set(obj, &JsValue::from_str(key), val);
}

pub fn arr_u32(data: &[u32]) -> Uint32Array { Uint32Array::from(data) }
pub fn arr_f32(data: &[f32]) -> Float32Array { Float32Array::from(data) }
pub fn arr_u8(data: &[u8]) -> Uint8Array { Uint8Array::from(data) }

