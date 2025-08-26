use crate::geometry::math::dist_point_to_seg_sq;
use crate::model::Vec2;

pub fn flatten_cubic(points: &mut Vec<Vec2>,
    x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32,
    tol: f32, depth: u32)
{
    let d1 = dist_point_to_seg_sq(x1, y1, x0, y0, x3, y3);
    let d2 = dist_point_to_seg_sq(x2, y2, x0, y0, x3, y3);
    let tol2 = tol * tol;
    if d1.max(d2) <= tol2 || depth > 16 {
        points.push(Vec2 { x: x3, y: y3 });
        return;
    }
    let x01 = 0.5*(x0 + x1); let y01 = 0.5*(y0 + y1);
    let x12 = 0.5*(x1 + x2); let y12 = 0.5*(y1 + y2);
    let x23 = 0.5*(x2 + x3); let y23 = 0.5*(y2 + y3);
    let x012 = 0.5*(x01 + x12); let y012 = 0.5*(y01 + y12);
    let x123 = 0.5*(x12 + x23); let y123 = 0.5*(y12 + y23);
    let x0123 = 0.5*(x012 + x123); let y0123 = 0.5*(y012 + y123);
    flatten_cubic(points, x0, y0, x01, y01, x012, y012, x0123, y0123, tol, depth+1);
    flatten_cubic(points, x0123, y0123, x123, y123, x23, y23, x3, y3, tol, depth+1);
}

