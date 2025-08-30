use wasm_bindgen_test::*;
use contour_wasm::Graph;
use wasm_bindgen::JsValue;
use js_sys::{Reflect, Float32Array, Uint32Array};

wasm_bindgen_test_configure!(run_in_browser);

fn is_ok(v: &JsValue) -> bool { Reflect::get(v, &JsValue::from_str("ok")).ok().and_then(|x| x.as_bool()).unwrap_or(false) }
fn is_err_code(v: &JsValue, code: &str) -> bool {
    if is_ok(v) { return false; }
    if let Ok(err) = Reflect::get(v, &JsValue::from_str("error")) {
        if let Ok(c) = Reflect::get(&err, &JsValue::from_str("code")) { return c.as_string().map_or(false, |s| s==code); }
    }
    false
}

#[wasm_bindgen_test]
fn fuzz_strict_methods_no_abort() {
    let mut g = Graph::new();
    // Seed some nodes and edges
    let a = g.add_node(0.0, 0.0); let b = g.add_node(100.0, 0.0);
    let e = g.add_edge(a,b).unwrap();

    // Simple LCG
    let mut seed: u64 = 0x1234_5678_ABCD_EF01;
    let mut rnd = || { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1); (seed >> 16) as u32 };

    for _ in 0..500u32 {
        let op = rnd() % 14;
        let ver_before = g.geom_version();
        let res = match op {
            0 => g.add_node_res(f32::from_bits(rnd()), f32::from_bits(rnd())),
            1 => { let id = if rnd()%2==0 { a } else { 99_999 }; g.move_node_res(id, f32::from_bits(rnd()), f32::from_bits(rnd())) }
            2 => { g.get_node_res(99_999) }
            3 => { g.remove_edge_res(77_777) }
            4 => { g.add_edge_res(a, a) } // invalid endpoints
            5 => { g.set_handle_pos_res(e, 2, 0.0, 0.0) } // invalid end
            6 => { g.set_handle_mode_res(e, 99) } // invalid mode
            7 => { g.bend_edge_to_res(e, -0.5, 0.0, 0.0, 1.0) } // t out of range
            8 => { g.pick_res(0.0, 0.0, -1.0) } // tol out of range
            9 => { g.set_flatten_tolerance_res(-0.1) }
            10 => { // polyline invalid array
                let arr = Float32Array::from(&[0.0f32, 1.0, 2.0][..]); g.add_polyline_edge_res(a,b,&arr)
            }
            11 => { // region invalid key
                g.toggle_region_res(0xFFFF_FFFF)
            }
            12 => { // style invalid width
                g.set_edge_style_res(e, 0,0,0,255, -1.0)
            }
            13 => { // translate invalid id
                let ids = Uint32Array::from(&[a, 0xFFFF_FFFF][..]); g.translate_nodes_res(&ids, 1.0, 2.0)
            }
            _ => unreachable!()
        };
        // No aborts and no state mutation on error paths
        if !is_ok(&res) { assert_eq!(g.geom_version(), ver_before); }
    }

    // Valid parity checks (a few calls)
    let v = g.add_node_res(10.0, 20.0); assert!(is_ok(&v));
    let id = Reflect::get(&Reflect::get(&v, &JsValue::from_str("value")).unwrap(), &JsValue::from_str("value")).ok(); let _ = id;
    let p = g.pick_res(50.0, 0.0, 10.0); assert!(is_ok(&p));
}

