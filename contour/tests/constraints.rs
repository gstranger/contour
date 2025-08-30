use contour::Graph;

fn dot(ax:f32,ay:f32,bx:f32,by:f32)->f32{ ax*bx+ay*by }

#[test]
fn mirrored_move_node_preserves_invariants() {
    let mut g = Graph::new();
    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(100.0, 0.0);
    let e = g.add_edge(a,b).unwrap();
    // Set cubic with asymmetric handles
    assert!(g.set_edge_cubic(e, 20.0, 30.0, 80.0, -10.0));
    assert!(g.set_handle_mode(e, 1)); // Mirrored
    // Move node A
    assert!(g.move_node(a, 10.0, 5.0));
    let h = g.get_handles(e).unwrap();
    let (ax,ay) = g.get_node(a).unwrap(); let (bx,by) = g.get_node(b).unwrap();
    let ha = (h[0]-ax, h[1]-ay); let hb=(h[2]-bx, h[3]-by);
    let la=(ha.0*ha.0+ha.1*ha.1).sqrt(); let lb=(hb.0*hb.0+hb.1*hb.1).sqrt();
    if la>1e-6 || lb>1e-6 {
        assert!((la-lb).abs() <= 1e-3*(1.0+la.max(lb)));
        assert!(dot(ha.0,ha.1,hb.0,hb.1) <= -0.99*la*lb + 1e-3);
    }
}

#[test]
fn aligned_move_node_preserves_invariants() {
    let mut g = Graph::new();
    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(100.0, 0.0);
    let e = g.add_edge(a,b).unwrap();
    assert!(g.set_edge_cubic(e, 20.0, 0.0, 80.0, 0.0));
    assert!(g.set_handle_mode(e, 2)); // Aligned
    assert!(g.move_node(b, 110.0, 10.0));
    let h = g.get_handles(e).unwrap();
    let (ax,ay) = g.get_node(a).unwrap(); let (bx,by) = g.get_node(b).unwrap();
    let ha = (h[0]-ax, h[1]-ay); let hb=(h[2]-bx, h[3]-by);
    let la=(ha.0*ha.0+ha.1*ha.1).sqrt(); let lb=(hb.0*hb.0+hb.1*hb.1).sqrt();
    if la>1e-6 || lb>1e-6 { assert!(dot(ha.0,ha.1,hb.0,hb.1) <= -0.99*la*lb + 1e-3); }
}

#[test]
fn mirrored_bend_preserves_invariants() {
    let mut g = Graph::new();
    let a = g.add_node(0.0, 0.0);
    let b = g.add_node(100.0, 0.0);
    let e = g.add_edge(a,b).unwrap();
    assert!(g.set_edge_cubic(e, 30.0, 0.0, 70.0, 0.0));
    assert!(g.set_handle_mode(e, 1));
    // Bend near start
    assert!(g.bend_edge_to(e, 0.2, 30.0, 20.0, 1.0));
    let h1 = g.get_handles(e).unwrap();
    let (ax,ay) = g.get_node(a).unwrap(); let (bx,by) = g.get_node(b).unwrap();
    let ha1 = (h1[0]-ax, h1[1]-ay); let hb1=(h1[2]-bx, h1[3]-by);
    let la1=(ha1.0*ha1.0+ha1.1*ha1.1).sqrt(); let lb1=(hb1.0*hb1.0+hb1.1*hb1.1).sqrt();
    if la1>1e-6 || lb1>1e-6 {
        assert!((la1-lb1).abs() <= 1e-3*(1.0+la1.max(lb1)));
        assert!(dot(ha1.0,ha1.1,hb1.0,hb1.1) <= -0.99*la1*lb1 + 1e-3);
    }
    // Bend near end
    assert!(g.bend_edge_to(e, 0.8, 70.0, -20.0, 1.0));
    let h2 = g.get_handles(e).unwrap();
    let ha2 = (h2[0]-ax, h2[1]-ay); let hb2=(h2[2]-bx, h2[3]-by);
    let la2=(ha2.0*ha2.0+ha2.1*ha2.1).sqrt(); let lb2=(hb2.0*hb2.0+hb2.1*hb2.1).sqrt();
    if la2>1e-6 || lb2>1e-6 {
        assert!((la2-lb2).abs() <= 1e-3*(1.0+la2.max(lb2)));
        assert!(dot(ha2.0,ha2.1,hb2.0,hb2.1) <= -0.99*la2*lb2 + 1e-3);
    }
}

