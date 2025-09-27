use contour_wasm::Graph;
use js_sys::{Float32Array, Reflect, Uint32Array};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

fn is_err(v: &JsValue, code: &str) -> bool {
    if let Ok(ok) =
        Reflect::get(v, &JsValue::from_str("ok")).and_then(|x| x.as_bool().ok_or(JsValue::NULL))
    {
        if ok {
            return false;
        }
        if let Ok(err) = Reflect::get(v, &JsValue::from_str("error")) {
            if let Ok(c) = Reflect::get(&err, &JsValue::from_str("code")) {
                return c.as_string().map_or(false, |s| s == code);
            }
        }
    }
    false
}

#[wasm_bindgen_test]
fn invalid_ids_and_ranges_return_typed_errors() {
    let mut g = Graph::new();
    let ver = g.geom_version();
    // invalid node id
    let r = g.move_node_res(12345, 0.0, 0.0);
    assert!(is_err(&r, "invalid_id"));
    assert_eq!(g.geom_version(), ver, "state mutated on error");

    // invalid edge id
    let r2 = g.remove_edge_res(9999);
    assert!(is_err(&r2, "invalid_id"));
    assert_eq!(g.geom_version(), ver);

    // out of range t
    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(10.0, 0.0);
    let e = g.add_edge(a, b).unwrap();
    let r3 = g.bend_edge_to_res(e, -0.1, 0.0, 0.0, 1.0);
    assert!(is_err(&r3, "out_of_range"));
    assert!(is_err(&g.pick_res(0.0, 0.0, -1.0), "out_of_range"));
}

#[wasm_bindgen_test]
fn handle_api_strict_errors() {
    let mut g = Graph::new();
    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(10.0, 0.0);
    let e = g.add_edge(a, b).unwrap();
    // Not cubic
    let r = g.get_handles_res(e);
    assert!(is_err(&r, "not_cubic"));
    // Bad end
    let r2 = g.set_handle_pos_res(e, 2, 0.0, 0.0);
    assert!(is_err(&r2, "invalid_end"));
}

#[wasm_bindgen_test]
fn region_toggle_strict_errors() {
    let mut g = Graph::new();
    // Random key should fail
    let r = g.toggle_region_res(123456);
    assert!(is_err(&r, "invalid_id"));
}

#[wasm_bindgen_test]
fn transforms_and_style_strict_errors() {
    let mut g = Graph::new();
    let ver = g.geom_version();
    // Non-finite transform
    let r = g.transform_all_res(f32::NAN, 0.0, 0.0, false);
    assert!(is_err(&r, "non_finite"));
    assert_eq!(g.geom_version(), ver);
    // Style with invalid width
    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(10.0, 0.0);
    let e = g.add_edge(a, b).unwrap();
    let r2 = g.set_edge_style_res(e, 0, 0, 0, 255, -1.0);
    assert!(is_err(&r2, "out_of_range"));
}

#[wasm_bindgen_test]
fn translate_res_validation() {
    let mut g = Graph::new();
    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(10.0, 0.0);
    let ids = Uint32Array::from(&[a, 999_999][..]);
    let r = g.translate_nodes_res(&ids, 1.0, 2.0);
    assert!(is_err(&r, "invalid_id"));
}

#[wasm_bindgen_test]
fn polyline_strict_errors() {
    let mut g = Graph::new();
    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(10.0, 0.0);
    // Odd-length points
    let arr = Float32Array::from(&[0.0f32, 1.0, 2.0][..]);
    let r = g.add_polyline_edge_res(a, b, &arr);
    assert!(is_err(&r, "invalid_array"));
}

#[wasm_bindgen_test]
fn svg_strict_errors_and_success() {
    let mut g = Graph::new();
    // Malformed path should yield svg_parse (no edges)
    let bad = g.add_svg_path_res("XYZ 10 10");
    assert!(is_err(&bad, "svg_parse"));
    // Simple valid rectangle
    let ok = g.add_svg_path_res("M 0 0 L 10 0 L 10 10 L 0 10 Z");
    // ok should be { ok:true, value:number }
    use js_sys::Reflect;
    let is_ok = Reflect::get(&ok, &JsValue::from_str("ok"))
        .unwrap()
        .as_bool()
        .unwrap();
    assert!(is_ok);
}
