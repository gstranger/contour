use contour::model::HandleMode;
use contour::Graph;
use std::f32;

#[test]
fn bend_zero_length_edge_is_noop() {
    let mut g = Graph::new();
    let n1 = g.add_node(10.0, 10.0);
    let n2 = g.add_node(10.0, 10.0);
    let eid = g.add_edge(n1, n2).expect("edge");
    let ok = g.bend_edge_to(eid, -0.5, 20.0, 20.0, 1.0);
    assert!(ok);
    let ea = g.get_edge_arrays();
    let mut kind = None;
    for (i, id) in ea.ids.iter().enumerate() {
        if *id == eid {
            kind = Some(ea.kinds[i]);
            break;
        }
    }
    assert_eq!(kind, Some(0), "edge should remain a Line kind");
}

#[test]
fn tiny_face_is_filtered() {
    let mut g = Graph::new();
    let n1 = g.add_node(0.0, 0.0);
    let n2 = g.add_node(0.1, 0.0);
    let n3 = g.add_node(0.05, 0.0866); // area ~0.00433
    let e1 = g.add_edge(n1, n2).unwrap();
    let _ = e1;
    let e2 = g.add_edge(n2, n3).unwrap();
    let _ = e2;
    let e3 = g.add_edge(n3, n1).unwrap();
    let _ = e3;
    let regions = g.get_regions();
    assert!(
        regions.is_empty(),
        "tiny triangle should be filtered as a face"
    );
}

#[test]
fn handles_degenerate_modes_do_not_panic() {
    let mut g = Graph::new();
    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(0.0, 0.0); // coincident endpoints (zero length)
    let e = g.add_edge(a, b).unwrap();
    // Convert to cubic with zero handles
    let _ = g.set_edge_cubic(e, 0.0, 0.0, 0.0, 0.0);
    // Set handle modes and positions near-origin; expect no panic
    let _ = g.set_handle_mode(e, 1); // Mirrored
    let _ = g.set_handle_pos(e, 0, 0.0, 0.0);
    let _ = g.set_handle_mode(e, 2); // Aligned
    let _ = g.set_handle_pos(e, 1, 0.0, 0.0);
    // Regions should also be robust
    let _ = g.get_regions();
}

#[test]
fn fuzz_10k_random_edits_no_panic() {
    let mut g = Graph::new();
    // Simple LCG to avoid external deps
    let mut seed: u64 = 0xDEADBEEFCAFEBABE;
    let mut rnd = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 16) as u32
    };

    // Seed with a few nodes
    let mut nodes: Vec<u32> = Vec::new();
    for _ in 0..10 {
        nodes.push(g.add_node((rnd() % 200) as f32 * 0.01, (rnd() % 200) as f32 * 0.01));
    }
    let mut edges: Vec<u32> = Vec::new();

    for step in 0..10_000u32 {
        // Periodically reset to keep graph small and runtime bounded
        if step % 750 == 0 && step != 0 {
            g.clear();
            nodes.clear();
            edges.clear();
            for _ in 0..5 {
                nodes.push(g.add_node((rnd() % 200) as f32 * 0.01, (rnd() % 200) as f32 * 0.01));
            }
        }

        // Bound graph growth
        if nodes.len() > 200 {
            let idx = (rnd() as usize) % nodes.len();
            let id = nodes.remove(idx);
            let _ = g.remove_node(id);
        }
        if edges.len() > 400 {
            let idx = (rnd() as usize) % edges.len();
            let id = edges.remove(idx);
            let _ = g.remove_edge(id);
        }

        let op = rnd() % 11;
        match op {
            0 => {
                // add node
                let id = g.add_node(
                    ((rnd() % 1000) as f32 - 500.0) * 0.01,
                    ((rnd() % 1000) as f32 - 500.0) * 0.01,
                );
                nodes.push(id);
            }
            1 => {
                // move node
                if !nodes.is_empty() {
                    let idx = (rnd() as usize) % nodes.len();
                    let id = nodes[idx];
                    let _ = g.move_node(
                        id,
                        ((rnd() % 1000) as f32 - 500.0) * 0.01,
                        ((rnd() % 1000) as f32 - 500.0) * 0.01,
                    );
                }
            }
            2 => {
                // add edge
                if nodes.len() >= 2 {
                    let i = (rnd() as usize) % nodes.len();
                    let j = (rnd() as usize) % nodes.len();
                    if let Some(e) = g.add_edge(nodes[i], nodes[j]) {
                        edges.push(e);
                    }
                }
            }
            3 => {
                // bend edge (occasional)
                if !edges.is_empty() {
                    let e = edges[(rnd() as usize) % edges.len()];
                    let t = f32::from_bits(rnd() as u32) % 2.0 - 0.5; // can be weird but bounded
                    let _ = g.bend_edge_to(
                        e,
                        t,
                        ((rnd() % 1000) as f32 - 500.0) * 0.01,
                        ((rnd() % 1000) as f32 - 500.0) * 0.01,
                        ((rnd() % 100) as f32) * 0.05 + 0.001,
                    );
                }
            }
            4 => {
                // set handle pos
                if !edges.is_empty() {
                    let e = edges[(rnd() as usize) % edges.len()];
                    let end = (rnd() % 2) as u8;
                    let _ = g.set_handle_pos(
                        e,
                        end,
                        ((rnd() % 1000) as f32 - 500.0) * 0.01,
                        ((rnd() % 1000) as f32 - 500.0) * 0.01,
                    );
                }
            }
            5 => {
                // set handle mode
                if !edges.is_empty() {
                    let e = edges[(rnd() as usize) % edges.len()];
                    let _ = g.set_handle_mode(e, (rnd() % 3) as u8);
                }
            }
            6 => {
                // regions (throttled)
                if step % 97 == 0 {
                    let _ = g.get_regions();
                }
            }
            7 => {
                // toggle region if any (rare)
                if step % 197 == 0 {
                    let regs = g.get_regions();
                    if let Some(r) = regs.get(0) {
                        if let Some(k) = r.get("key").and_then(|v| v.as_u64()) {
                            let _ = g.toggle_region(k as u32);
                        }
                    }
                }
            }
            8 => {
                // remove random node
                if !nodes.is_empty() {
                    let idx = (rnd() as usize) % nodes.len();
                    let id = nodes.remove(idx);
                    let _ = g.remove_node(id);
                }
            }
            9 => {
                // remove random edge
                if !edges.is_empty() {
                    let idx = (rnd() as usize) % edges.len();
                    let id = edges.remove(idx);
                    let _ = g.remove_edge(id);
                }
            }
            10 => {
                // invariants check on a random cubic edge (public API)
                if !edges.is_empty() {
                    let eid = edges[(rnd() as usize) % edges.len()];
                    if let Some(h) = g.get_handles(eid) {
                        if let Some(mode) = g.get_handle_mode(eid) {
                            // Find endpoints for this eid via ids index
                            let ea = g.get_edge_arrays();
                            let mut idx = None;
                            for (i, &id) in ea.ids.iter().enumerate() {
                                if id == eid {
                                    idx = Some(i);
                                    break;
                                }
                            }
                            if idx.is_none() {
                                continue;
                            }
                            let i = idx.unwrap();
                            let na = ea.endpoints[2 * i];
                            let nb = ea.endpoints[2 * i + 1];
                            let (ax, ay) = g.get_node(na).unwrap_or((0.0, 0.0));
                            let (bx, by) = g.get_node(nb).unwrap_or((0.0, 0.0));
                            let ha = (h[0] - ax, h[1] - ay);
                            let hb = (h[2] - bx, h[3] - by);
                            let la = (ha.0 * ha.0 + ha.1 * ha.1).sqrt();
                            let lb = (hb.0 * hb.0 + hb.1 * hb.1).sqrt();
                            match mode {
                                1 => {
                                    // Mirrored
                                    if la > 1e-6 || lb > 1e-6 {
                                        let dot = ha.0 * hb.0 + ha.1 * hb.1;
                                        assert!(
                                            (la - lb).abs() <= 1e-3 * (1.0 + la.max(lb)),
                                            "mirrored lengths diverged: la={}, lb={}",
                                            la,
                                            lb
                                        );
                                        assert!(
                                            dot <= -0.99 * la * lb + 1e-3,
                                            "mirrored not opposite enough: dot={}, la={}, lb={}",
                                            dot,
                                            la,
                                            lb
                                        );
                                    }
                                }
                                2 => {
                                    // Aligned
                                    if la > 1e-6 || lb > 1e-6 {
                                        let dot = ha.0 * hb.0 + ha.1 * hb.1;
                                        assert!(
                                            dot <= -0.99 * la * lb + 1e-3,
                                            "aligned not opposite enough: dot={}, la={}, lb={}",
                                            dot,
                                            la,
                                            lb
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            11 => {
                // force mode then assert invariants immediately
                if !edges.is_empty() {
                    let eid = edges[(rnd() as usize) % edges.len()];
                    let m = (rnd() % 2) + 1; // 1 Mirrored, 2 Aligned
                    let _ = g.set_handle_mode(eid, m as u8);
                    // Reuse op10 logic
                    let h = match g.get_handles(eid) {
                        Some(h) => h,
                        None => continue,
                    };
                    let ea = g.get_edge_arrays();
                    let mut idx = None;
                    for (i, &id) in ea.ids.iter().enumerate() {
                        if id == eid {
                            idx = Some(i);
                            break;
                        }
                    }
                    if idx.is_none() {
                        continue;
                    }
                    let i = idx.unwrap();
                    let na = ea.endpoints[2 * i];
                    let nb = ea.endpoints[2 * i + 1];
                    let (ax, ay) = g.get_node(na).unwrap_or((0.0, 0.0));
                    let (bx, by) = g.get_node(nb).unwrap_or((0.0, 0.0));
                    let ha = (h[0] - ax, h[1] - ay);
                    let hb = (h[2] - bx, h[3] - by);
                    let la = (ha.0 * ha.0 + ha.1 * ha.1).sqrt();
                    let lb = (hb.0 * hb.0 + hb.1 * hb.1).sqrt();
                    if m == 1 {
                        if la > 1e-6 || lb > 1e-6 {
                            let dot = ha.0 * hb.0 + ha.1 * hb.1;
                            assert!((la - lb).abs() <= 1e-3 * (1.0 + la.max(lb)));
                            assert!(dot <= -0.99 * la * lb + 1e-3);
                        }
                    }
                    if m == 2 {
                        if la > 1e-6 || lb > 1e-6 {
                            let dot = ha.0 * hb.0 + ha.1 * hb.1;
                            assert!(dot <= -0.99 * la * lb + 1e-3);
                        }
                    }
                }
            }
            12 => {
                // bend edge under Mirrored, then assert
                if !edges.is_empty() {
                    let eid = edges[(rnd() as usize) % edges.len()];
                    let _ = g.set_handle_mode(eid, 1);
                    let t = (rnd() % 100) as f32 / 100.0;
                    let _ = g.bend_edge_to(
                        eid,
                        t,
                        ((rnd() % 1000) as f32 - 500.0) * 0.01,
                        ((rnd() % 1000) as f32 - 500.0) * 0.01,
                        1.0,
                    );
                    // Assert mirrored invariants
                    if let Some(h) = g.get_handles(eid) {
                        let ea = g.get_edge_arrays();
                        let mut idx = None;
                        for (i, &id) in ea.ids.iter().enumerate() {
                            if id == eid {
                                idx = Some(i);
                                break;
                            }
                        }
                        if let Some(i) = idx {
                            let na = ea.endpoints[2 * i];
                            let nb = ea.endpoints[2 * i + 1];
                            let (ax, ay) = g.get_node(na).unwrap_or((0.0, 0.0));
                            let (bx, by) = g.get_node(nb).unwrap_or((0.0, 0.0));
                            let ha = (h[0] - ax, h[1] - ay);
                            let hb = (h[2] - bx, h[3] - by);
                            let la = (ha.0 * ha.0 + ha.1 * ha.1).sqrt();
                            let lb = (hb.0 * hb.0 + hb.1 * hb.1).sqrt();
                            if la > 1e-6 || lb > 1e-6 {
                                let dot = ha.0 * hb.0 + ha.1 * hb.1;
                                assert!((la - lb).abs() <= 1e-3 * (1.0 + la.max(lb)));
                                assert!(dot <= -0.99 * la * lb + 1e-3);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Final sanity: getters work
    let _ = g.get_node_arrays();
    let _ = g.get_edge_arrays();
}
