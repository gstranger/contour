// Robust segment-segment intersection using f64 with tolerances.
// Classifies proper crossings, endpoint touches, and collinear overlaps.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SegIntersection {
    None,
    // Proper interior intersection (not at endpoints within tolerance)
    Proper { t: f64, u: f64, x: f64, y: f64 },
    // Touch at endpoints (may be both endpoints). t/u may be 0 or 1 within tolerance
    Touch { t: f64, u: f64, x: f64, y: f64 },
    // Collinear overlapping span: parameter ranges on each segment (inclusive, ordered)
    CollinearOverlap { t0: f64, t1: f64, u0: f64, u1: f64 },
}

#[inline]
fn orient(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64) -> f64 {
    (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
}

#[inline]
fn within_eps(x: f64, eps: f64) -> bool { x.abs() <= eps }

#[inline]
fn clamp01f64(x: f64) -> f64 { if x < 0.0 { 0.0 } else if x > 1.0 { 1.0 } else { x } }

// Project segment AB and CD onto the dominant axis and compute parameter ranges for overlap.
fn collinear_overlap(ax: f64, ay: f64, bx: f64, by: f64,
                     cx: f64, cy: f64, dx: f64, dy: f64,
                     eps: f64) -> SegIntersection {
    // Choose projection axis with larger extent on AB
    let dxab = (bx - ax).abs();
    let dyab = (by - ay).abs();
    let (pa1, pa2, pc1, pc2) = if dxab >= dyab {
        (ax, bx, cx, dx)
    } else {
        (ay, by, cy, dy)
    };
    // Normalize parameters on AB
    let len_ab = (pa2 - pa1);
    if within_eps(len_ab, eps) {
        // AB is a point: treat as touch if it lies on CD
        let t = 0.0;
        let (x, y) = (ax, ay);
        // Param on CD using least-squares along dominant axis
        let len_cd = (pc2 - pc1);
        let u = if within_eps(len_cd, eps) { 0.0 } else { (if dxab >= dyab { x - cx } else { y - cy }) / len_cd };
        return SegIntersection::Touch { t, u, x, y };
    }
    let t_c1 = (pc1 - pa1) / len_ab; // where C/D project onto AB in t-space
    let t_c2 = (pc2 - pa1) / len_ab;
    let mut lo = t_c1.min(t_c2);
    let mut hi = t_c1.max(t_c2);
    // Intersect with [0,1]
    if hi < -eps || lo > 1.0 + eps { return SegIntersection::None; }
    lo = lo.max(0.0);
    hi = hi.min(1.0);
    if hi < lo { return SegIntersection::None; }
    // Map back to u-range linearly along CD
    let len_cd = (pc2 - pc1);
    let u0 = if within_eps(len_cd, eps) { 0.0 } else { (pa1 + lo * len_ab - pc1) / len_cd };
    let u1 = if within_eps(len_cd, eps) { 0.0 } else { (pa1 + hi * len_ab - pc1) / len_cd };
    // Tighten ordering and clamp
    let (t0, t1) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    let (u0, u1) = if u0 <= u1 { (u0, u1) } else { (u1, u0) };
    SegIntersection::CollinearOverlap { t0, t1, u0, u1 }
}

pub fn intersect_segments(ax: f32, ay: f32, bx: f32, by: f32,
                          cx: f32, cy: f32, dx: f32, dy: f32,
                          eps_pos: f32, eps_denom: f32) -> SegIntersection {
    let ax = ax as f64; let ay = ay as f64; let bx = bx as f64; let by = by as f64;
    let cx = cx as f64; let cy = cy as f64; let dx = dx as f64; let dy = dy as f64;
    let eps = eps_pos as f64;
    let denom_eps = eps_denom as f64;

    let o1 = orient(ax, ay, bx, by, cx, cy);
    let o2 = orient(ax, ay, bx, by, dx, dy);
    let o3 = orient(cx, cy, dx, dy, ax, ay);
    let o4 = orient(cx, cy, dx, dy, bx, by);

    // Collinear cases: all orientations ~ 0
    if within_eps(o1, eps) && within_eps(o2, eps) && within_eps(o3, eps) && within_eps(o4, eps) {
        return collinear_overlap(ax, ay, bx, by, cx, cy, dx, dy, eps);
    }

    // General intersection test with tolerance: o1 and o2 have opposite signs (or zero), and o3, o4 too
    let inter1 = (o1 > 0.0 && o2 < 0.0) || (o1 < 0.0 && o2 > 0.0) || within_eps(o1, eps) || within_eps(o2, eps);
    let inter2 = (o3 > 0.0 && o4 < 0.0) || (o3 < 0.0 && o4 > 0.0) || within_eps(o3, eps) || within_eps(o4, eps);
    if !(inter1 && inter2) {
        return SegIntersection::None;
    }

    // Compute exact intersection for lines AB and CD, then test if within [0,1]
    let r_x = bx - ax; let r_y = by - ay;
    let s_x = dx - cx; let s_y = dy - cy;
    let rxs = r_x * s_y - r_y * s_x;
    let q_p_x = cx - ax; let q_p_y = cy - ay;
    let qpxr = q_p_x * r_y - q_p_y * r_x;

    if within_eps(rxs, denom_eps) {
        // Parallel but not collinear (already handled)
        return SegIntersection::None;
    }

    let t = (q_p_x * s_y - q_p_y * s_x) / rxs;
    let u = qpxr / rxs;
    let x = ax + t * r_x;
    let y = ay + t * r_y;

    // Classify as touch vs proper using endpoint tolerance
    let is_touch = within_eps(t, eps) || within_eps(1.0 - t, eps) || within_eps(u, eps) || within_eps(1.0 - u, eps);

    if is_touch {
        SegIntersection::Touch { t: clamp01f64(t), u: clamp01f64(u), x, y }
    } else if t >= -eps && t <= 1.0 + eps && u >= -eps && u <= 1.0 + eps {
        SegIntersection::Proper { t, u, x, y }
    } else {
        SegIntersection::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EP: f32 = 1e-4;
    const ED: f32 = 1e-8;

    #[test]
    fn proper_cross() {
        let r = intersect_segments(0.0,0.0,  2.0,2.0,  0.0,2.0,  2.0,0.0, EP, ED);
        match r { SegIntersection::Proper{t,u,..} => { assert!(t>0.4 && t<0.6); assert!(u>0.4 && u<0.6); }, _ => panic!("expected proper") }
    }

    #[test]
    fn endpoint_touch() {
        let r = intersect_segments(0.0,0.0,  1.0,0.0,  1.0,0.0,  1.0,1.0, EP, ED);
        match r { SegIntersection::Touch{t,u,x,y} => { assert!((x-1.0).abs()<1e-9 && (y-0.0).abs()<1e-9); assert!((t-1.0).abs()<1e-6); assert!((u-0.0).abs()<1e-6); }, _ => panic!("expected touch") }
    }

    #[test]
    fn collinear_overlap() {
        let r = intersect_segments(0.0,0.0,  3.0,0.0,  1.0,0.0,  2.0,0.0, EP, ED);
        match r { SegIntersection::CollinearOverlap{t0,t1,..} => { assert!(t0>=0.33 && t1<=0.67); }, _ => panic!("expected overlap") }
    }
}
