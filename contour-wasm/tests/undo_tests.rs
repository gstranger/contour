#![cfg(target_arch = "wasm32")]

use contour_wasm::Graph;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn undo_res_empty_stack_error() {
    let mut g = Graph::new();
    let result = g.undo_res();
    let ok = js_sys::Reflect::get(&result, &JsValue::from_str("ok")).unwrap();
    assert_eq!(ok, JsValue::from_bool(false));
    let err_obj = js_sys::Reflect::get(&result, &JsValue::from_str("error")).unwrap();
    let code = js_sys::Reflect::get(&err_obj, &JsValue::from_str("code")).unwrap();
    assert_eq!(code.as_string().unwrap(), "undo_empty");
}

#[wasm_bindgen_test]
fn redo_res_empty_stack_error() {
    let mut g = Graph::new();
    let result = g.redo_res();
    let ok = js_sys::Reflect::get(&result, &JsValue::from_str("ok")).unwrap();
    assert_eq!(ok, JsValue::from_bool(false));
    let err_obj = js_sys::Reflect::get(&result, &JsValue::from_str("error")).unwrap();
    let code = js_sys::Reflect::get(&err_obj, &JsValue::from_str("code")).unwrap();
    assert_eq!(code.as_string().unwrap(), "redo_empty");
}

#[wasm_bindgen_test]
fn undo_during_group_errors() {
    let mut g = Graph::new();
    let _ = g.add_node_res(0.0, 0.0);
    g.begin_undo_group_res("test");
    let result = g.undo_res();
    let ok = js_sys::Reflect::get(&result, &JsValue::from_str("ok")).unwrap();
    assert_eq!(ok, JsValue::from_bool(false));
    let err_obj = js_sys::Reflect::get(&result, &JsValue::from_str("error")).unwrap();
    let code = js_sys::Reflect::get(&err_obj, &JsValue::from_str("code")).unwrap();
    assert_eq!(code.as_string().unwrap(), "undo_in_progress");
    g.end_undo_group_res();
}

#[wasm_bindgen_test]
fn end_without_begin_noop() {
    let mut g = Graph::new();
    let result = g.end_undo_group_res();
    let ok = js_sys::Reflect::get(&result, &JsValue::from_str("ok")).unwrap();
    assert_eq!(ok, JsValue::from_bool(true));
    let value = js_sys::Reflect::get(&result, &JsValue::from_str("value")).unwrap();
    let committed = js_sys::Reflect::get(&value, &JsValue::from_str("committed")).unwrap();
    assert_eq!(committed, JsValue::from_bool(false));
}

#[wasm_bindgen_test]
fn label_propagation() {
    let mut g = Graph::new();
    let id_res = g.add_node_res(0.0, 0.0);
    let value = js_sys::Reflect::get(&id_res, &JsValue::from_str("value")).unwrap();
    let _nid = value.as_f64().unwrap() as u32;

    g.begin_undo_group_res("move node");
    let _ = g.move_node_res(_nid, 50.0, 50.0);
    g.end_undo_group_res();

    let undo_result = g.undo_res();
    let val = js_sys::Reflect::get(&undo_result, &JsValue::from_str("value")).unwrap();
    let label = js_sys::Reflect::get(&val, &JsValue::from_str("label")).unwrap();
    assert_eq!(label.as_string().unwrap(), "move node");
}

#[wasm_bindgen_test]
fn depth_remaining_decrements() {
    let mut g = Graph::new();
    let _ = g.add_node_res(100.0, 0.0);
    let _ = g.add_node_res(200.0, 0.0);

    let undo1 = g.undo_res();
    let val1 = js_sys::Reflect::get(&undo1, &JsValue::from_str("value")).unwrap();
    let depth1 = js_sys::Reflect::get(&val1, &JsValue::from_str("depth_remaining")).unwrap();
    assert_eq!(depth1.as_f64().unwrap(), 1.0);

    let undo2 = g.undo_res();
    let val2 = js_sys::Reflect::get(&undo2, &JsValue::from_str("value")).unwrap();
    let depth2 = js_sys::Reflect::get(&val2, &JsValue::from_str("depth_remaining")).unwrap();
    assert_eq!(depth2.as_f64().unwrap(), 0.0);
}
