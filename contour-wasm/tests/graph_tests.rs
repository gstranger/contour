use contour_wasm::Graph;
use js_sys::{Float32Array, Reflect, Uint32Array};
use serde::Deserialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn nodes_and_edges_basic() {
    let mut g = Graph::new();
    let a = g.add_node(10.0, 20.0);
    let b = g.add_node(30.0, 40.0);
    assert_eq!(g.node_count(), 2);

    // get_node returns [x,y]
    let na = g.get_node(a);
    let va: Vec<f32> = serde_wasm_bindgen::from_value(na).unwrap();
    assert_eq!(va, vec![10.0, 20.0]);

    // move_node
    assert!(g.move_node(b, 35.0, 45.0));
    let vb: Vec<f32> = serde_wasm_bindgen::from_value(g.get_node(b)).unwrap();
    assert_eq!(vb, vec![35.0, 45.0]);

    // add_edge
    let e = g.add_edge(a, b).expect("edge id");
    assert_eq!(g.edge_count(), 1);

    // typed arrays
    let nd = g.get_node_data();
    let n_ids = Uint32Array::new(&Reflect::get(&nd, &JsValue::from_str("ids")).unwrap());
    let n_pos = Float32Array::new(&Reflect::get(&nd, &JsValue::from_str("positions")).unwrap());
    assert_eq!(n_ids.length(), 2);
    assert_eq!(n_pos.length(), 4);

    let ed = g.get_edge_data();
    let e_ids = Uint32Array::new(&Reflect::get(&ed, &JsValue::from_str("ids")).unwrap());
    let e_ep = Uint32Array::new(&Reflect::get(&ed, &JsValue::from_str("endpoints")).unwrap());
    assert_eq!(e_ids.length(), 1);
    assert_eq!(e_ep.length(), 2);

    // remove_edge
    assert!(g.remove_edge(e));
    assert_eq!(g.edge_count(), 0);
}

#[wasm_bindgen_test]
fn pick_node_and_edge() {
    let mut g = Graph::new();
    let a = g.add_node(100.0, 100.0);
    let b = g.add_node(200.0, 100.0);
    g.add_edge(a, b).unwrap();

    // pick near node a
    let p = g.pick(102.0, 98.0, 10.0);
    #[derive(Deserialize)]
    struct Pick {
        kind: String,
        id: f64,
    }
    let pn: Pick = serde_wasm_bindgen::from_value(p).unwrap();
    assert_eq!(pn.kind, "node");
    assert_eq!(pn.id as u32, a);

    // pick near edge (midpoint)
    let p2 = g.pick(150.0, 100.0, 5.0);
    let pe: Pick = serde_wasm_bindgen::from_value(p2).unwrap();
    assert_eq!(pe.kind, "edge");
}

#[wasm_bindgen_test]
fn json_roundtrip_and_clear() {
    let mut g = Graph::new();
    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(10.0, 10.0);
    g.add_edge(a, b).unwrap();

    let j = g.to_json();
    #[derive(Deserialize)]
    struct NodeSer {
        id: u32,
        x: f32,
        y: f32,
    }
    #[derive(Deserialize)]
    struct EdgeSer {
        id: u32,
        a: u32,
        b: u32,
    }
    #[derive(Deserialize)]
    struct Doc {
        nodes: Vec<NodeSer>,
        edges: Vec<EdgeSer>,
    }
    let doc: Doc = serde_wasm_bindgen::from_value(j.clone()).unwrap();
    assert_eq!(doc.nodes.len(), 2);
    assert_eq!(doc.edges.len(), 1);

    // Load into a new graph
    let mut g2 = Graph::new();
    assert!(g2.from_json(j));
    assert_eq!(g2.node_count(), 2);
    assert_eq!(g2.edge_count(), 1);

    // Clear
    g2.clear();
    assert_eq!(g2.node_count(), 0);
    assert_eq!(g2.edge_count(), 0);
}

#[wasm_bindgen_test]
fn cubic_handles() {
    let mut g = Graph::new();
    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(100.0, 0.0);
    let e = g.add_edge(a, b).unwrap();
    // Make cubic with handles at 30% along the edge
    assert!(g.set_edge_cubic(e, 30.0, 0.0, 70.0, 0.0));
    let h = g.get_handles(e);
    let hv: Vec<f32> = serde_wasm_bindgen::from_value(h).unwrap();
    assert_eq!(hv, vec![30.0, 0.0, 70.0, 0.0]);

    // Move one handle
    assert!(g.set_handle_pos(e, 0, 25.0, 10.0));
    let hv2: Vec<f32> = serde_wasm_bindgen::from_value(g.get_handles(e)).unwrap();
    assert_eq!(hv2[0], 25.0);
    assert_eq!(hv2[1], 10.0);

    // Pick near handle
    let p = g.pick(26.0, 9.0, 10.0);
    #[derive(Deserialize)]
    struct HPick {
        kind: String,
        edge: f64,
        end: f64,
    }
    let ph: HPick = serde_wasm_bindgen::from_value(p).unwrap();
    assert_eq!(ph.kind, "handle");
    assert_eq!(ph.edge as u32, e);
}

#[wasm_bindgen_test]
fn regions_and_toggle() {
    let mut g = Graph::new();
    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(100.0, 0.0);
    let c = g.add_node(100.0, 100.0);
    let d = g.add_node(0.0, 100.0);
    g.add_edge(a, b).unwrap();
    g.add_edge(b, c).unwrap();
    g.add_edge(c, d).unwrap();
    g.add_edge(d, a).unwrap();

    // Should find one region approximately
    let regions = g.get_regions();
    let arr: Vec<serde_json::Value> = serde_wasm_bindgen::from_value(regions).unwrap();
    assert!(arr.len() >= 1);
    // Toggle first region
    let key = arr[0].get("key").unwrap().as_u64().unwrap() as u32;
    let after = g.toggle_region(key);
    assert!(!after);
}

#[wasm_bindgen_test]
fn svg_import_export_basic() {
    let mut g = Graph::new();
    let d = "M 0 0 L 100 0 L 100 100 L 0 100 Z";
    let added = g.add_svg_path(d);
    assert!(added >= 4);
    assert_eq!(g.edge_count(), 4);
    // Export fragments
    let paths = g.to_svg_paths();
    let arr: Vec<String> = serde_wasm_bindgen::from_value(paths).unwrap();
    assert_eq!(arr.len(), 4);
    assert!(arr.iter().all(|s| s.starts_with("M ")));
}
