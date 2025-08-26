use js_sys::{Object, Reflect, Uint32Array, Float32Array, Uint8Array};
use wasm_bindgen::JsValue;

pub fn new_obj() -> Object { Object::new() }
pub fn set_kv(obj: &Object, k: &str, v: &JsValue) {
    let _ = Reflect::set(obj, &JsValue::from_str(k), v);
}
pub fn arr_u32(slice: &[u32]) -> Uint32Array {
    let arr = Uint32Array::new_with_length(slice.len() as u32);
    arr.copy_from(slice); arr
}
pub fn arr_f32(slice: &[f32]) -> Float32Array {
    let arr = Float32Array::new_with_length(slice.len() as u32);
    arr.copy_from(slice); arr
}
pub fn arr_u8(slice: &[u8]) -> Uint8Array {
    let arr = Uint8Array::new_with_length(slice.len() as u32);
    arr.copy_from(slice); arr
}

