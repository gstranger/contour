// no extra imports
use super::tolerance::{approx_eq, EPS_POS};

pub fn seg_distance_sq(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> (f32, f32) {
    let vx = x2 - x1; let vy = y2 - y1;
    let wx = px - x1; let wy = py - y1;
    let vv = vx*vx + vy*vy;
    let mut t = if vv > 0.0 { (wx*vx + wy*vy) / vv } else { 0.0 };
    if t < 0.0 { t = 0.0; } else if t > 1.0 { t = 1.0; }
    let projx = x1 + t * vx; let projy = y1 + t * vy;
    let dx = px - projx; let dy = py - projy;
    (dx*dx + dy*dy, t)
}

pub fn cubic_point(t: f32, x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) -> (f32, f32) {
    let u = 1.0 - t;
    let tt = t*t; let uu = u*u;
    let uuu = uu*u; let ttt = tt*t;
    let x = uuu*x0 + 3.0*uu*t*x1 + 3.0*u*tt*x2 + ttt*x3;
    let y = uuu*y0 + 3.0*uu*t*y1 + 3.0*u*tt*y2 + ttt*y3;
    (x, y)
}

pub fn dist_point_to_seg_sq(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let (d2, _) = seg_distance_sq(px, py, x1, y1, x2, y2);
    d2
}

pub fn cubic_distance_sq(px: f32, py: f32,
    x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) -> (f32, f32) {
    // Degenerate: all control points coincident
    if approx_eq(x0,x1,EPS_POS) && approx_eq(x1,x2,EPS_POS) && approx_eq(x2,x3,EPS_POS)
        && approx_eq(y0,y1,EPS_POS) && approx_eq(y1,y2,EPS_POS) && approx_eq(y2,y3,EPS_POS) {
        let dx = px - x0; let dy = py - y0; return (dx*dx+dy*dy, 0.0);
    }
    let mut best_d2 = f32::INFINITY;
    let mut best_t = 0.0;
    let n = 32;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let (x, y) = cubic_point(t, x0,y0,x1,y1,x2,y2,x3,y3);
        let dx = px - x; let dy = py - y; let d2 = dx*dx + dy*dy;
        if d2 < best_d2 { best_d2 = d2; best_t = t; }
    }
    (best_d2, best_t)
}
