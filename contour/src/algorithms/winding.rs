//! Winding number calculation for point-in-polygon testing.
//!
//! Uses horizontal ray casting with signed crossing count to determine
//! the winding number of a point relative to a polygon.

use crate::model::Vec2;

/// Compute the winding number of a point relative to a polygon.
///
/// Returns the number of times the polygon winds around the point.
/// - Positive = counter-clockwise winding
/// - Negative = clockwise winding
/// - Zero = point is outside
///
/// Uses the crossing number algorithm with signed crossings.
pub fn winding_number(px: f32, py: f32, polygon: &[Vec2]) -> i32 {
    if polygon.len() < 3 {
        return 0;
    }

    let mut winding = 0i32;
    let n = polygon.len();

    for i in 0..n {
        let p1 = polygon[i];
        let p2 = polygon[(i + 1) % n];

        // Check if the edge crosses the horizontal ray from (px, py) going right
        if p1.y <= py {
            if p2.y > py {
                // Upward crossing
                let cross = cross_product(p1.x - px, p1.y - py, p2.x - px, p2.y - py);
                if cross > 0.0 {
                    winding += 1;
                }
            }
        } else if p2.y <= py {
            // Downward crossing
            let cross = cross_product(p1.x - px, p1.y - py, p2.x - px, p2.y - py);
            if cross < 0.0 {
                winding -= 1;
            }
        }
    }

    winding
}

/// Check if a point is inside a polygon using the non-zero winding rule.
#[inline]
pub fn point_in_polygon_nonzero(px: f32, py: f32, polygon: &[Vec2]) -> bool {
    winding_number(px, py, polygon) != 0
}

/// Check if a point is inside a polygon using the even-odd rule.
#[inline]
pub fn point_in_polygon_evenodd(px: f32, py: f32, polygon: &[Vec2]) -> bool {
    crossing_number(px, py, polygon) % 2 == 1
}

/// Compute the crossing number (number of edge crossings) for even-odd rule.
pub fn crossing_number(px: f32, py: f32, polygon: &[Vec2]) -> i32 {
    if polygon.len() < 3 {
        return 0;
    }

    let mut crossings = 0i32;
    let n = polygon.len();

    for i in 0..n {
        let p1 = polygon[i];
        let p2 = polygon[(i + 1) % n];

        // Check if edge crosses the horizontal ray from (px, py) going right
        let y_crosses = (p1.y <= py && p2.y > py) || (p2.y <= py && p1.y > py);

        if y_crosses {
            // Compute x-coordinate of intersection
            let t = (py - p1.y) / (p2.y - p1.y);
            let x_intersect = p1.x + t * (p2.x - p1.x);

            if px < x_intersect {
                crossings += 1;
            }
        }
    }

    crossings
}

/// Cross product of 2D vectors (ax, ay) and (bx, by).
#[inline]
fn cross_product(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    ax * by - ay * bx
}

/// Compute winding number for a point relative to multiple polygons.
/// Returns (winding_for_shape_a, winding_for_shape_b).
pub fn winding_numbers_dual(
    px: f32,
    py: f32,
    polygon_a: &[Vec2],
    polygon_b: &[Vec2],
) -> (i32, i32) {
    (
        winding_number(px, py, polygon_a),
        winding_number(px, py, polygon_b),
    )
}

/// Check if a point lies on a polygon edge within tolerance.
pub fn point_on_polygon_edge(px: f32, py: f32, polygon: &[Vec2], tol: f32) -> bool {
    if polygon.is_empty() {
        return false;
    }

    let tol_sq = tol * tol;
    let n = polygon.len();

    for i in 0..n {
        let p1 = polygon[i];
        let p2 = polygon[(i + 1) % n];

        if point_on_segment_sq(px, py, p1.x, p1.y, p2.x, p2.y, tol_sq) {
            return true;
        }
    }

    false
}

/// Check if point (px, py) is within tol_sq of segment (x1,y1)-(x2,y2).
fn point_on_segment_sq(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32, tol_sq: f32) -> bool {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 1e-12 {
        // Degenerate segment - just check distance to point
        let dpx = px - x1;
        let dpy = py - y1;
        return dpx * dpx + dpy * dpy <= tol_sq;
    }

    // Project point onto segment
    let t = ((px - x1) * dx + (py - y1) * dy) / len_sq;
    let t_clamped = t.clamp(0.0, 1.0);

    let closest_x = x1 + t_clamped * dx;
    let closest_y = y1 + t_clamped * dy;

    let dist_sq = (px - closest_x).powi(2) + (py - closest_y).powi(2);
    dist_sq <= tol_sq
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vec2(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }

    #[test]
    fn test_winding_number_square() {
        // Counter-clockwise square
        let square = vec![
            vec2(0.0, 0.0),
            vec2(10.0, 0.0),
            vec2(10.0, 10.0),
            vec2(0.0, 10.0),
        ];

        // Point inside
        assert_eq!(winding_number(5.0, 5.0, &square), 1);

        // Point outside
        assert_eq!(winding_number(-5.0, 5.0, &square), 0);
        assert_eq!(winding_number(15.0, 5.0, &square), 0);
        assert_eq!(winding_number(5.0, -5.0, &square), 0);
        assert_eq!(winding_number(5.0, 15.0, &square), 0);
    }

    #[test]
    fn test_winding_number_clockwise() {
        // Clockwise square (negative winding)
        let square = vec![
            vec2(0.0, 0.0),
            vec2(0.0, 10.0),
            vec2(10.0, 10.0),
            vec2(10.0, 0.0),
        ];

        // Point inside has negative winding
        assert_eq!(winding_number(5.0, 5.0, &square), -1);
    }

    #[test]
    fn test_crossing_number_square() {
        let square = vec![
            vec2(0.0, 0.0),
            vec2(10.0, 0.0),
            vec2(10.0, 10.0),
            vec2(0.0, 10.0),
        ];

        // Point inside - ray crosses 1 edge (right side)
        assert_eq!(crossing_number(5.0, 5.0, &square), 1);

        // Point outside to the left - ray crosses 2 edges (left and right sides)
        assert_eq!(crossing_number(-5.0, 5.0, &square), 2);

        // Point outside to the right - ray crosses 0 edges
        assert_eq!(crossing_number(15.0, 5.0, &square), 0);

        // Even-odd rule: inside = odd crossings, outside = even crossings
        assert!(point_in_polygon_evenodd(5.0, 5.0, &square));  // 1 = odd = inside
        assert!(!point_in_polygon_evenodd(-5.0, 5.0, &square)); // 2 = even = outside
        assert!(!point_in_polygon_evenodd(15.0, 5.0, &square)); // 0 = even = outside
    }

    #[test]
    fn test_point_in_polygon() {
        let square = vec![
            vec2(0.0, 0.0),
            vec2(10.0, 0.0),
            vec2(10.0, 10.0),
            vec2(0.0, 10.0),
        ];

        assert!(point_in_polygon_nonzero(5.0, 5.0, &square));
        assert!(!point_in_polygon_nonzero(-5.0, 5.0, &square));

        assert!(point_in_polygon_evenodd(5.0, 5.0, &square));
        assert!(!point_in_polygon_evenodd(-5.0, 5.0, &square));
    }

    #[test]
    fn test_self_intersecting_polygon() {
        // Figure-8 / bowtie shape (self-intersecting at center)
        // For self-intersecting polygons, winding behavior depends on edge order
        // and can result in counter-intuitive values. The key is robustness.
        let figure8 = vec![
            vec2(0.0, 0.0),
            vec2(10.0, 10.0),
            vec2(10.0, 0.0),
            vec2(0.0, 10.0),
        ];

        // Just verify the algorithm doesn't crash and returns consistent values
        let center_winding = winding_number(5.0, 5.0, &figure8);
        let center_crossing = crossing_number(5.0, 5.0, &figure8);

        // Call multiple times to ensure deterministic behavior
        assert_eq!(winding_number(5.0, 5.0, &figure8), center_winding);
        assert_eq!(crossing_number(5.0, 5.0, &figure8), center_crossing);

        // Test various points - algorithm should not crash
        let _ = winding_number(7.0, 3.0, &figure8);
        let _ = winding_number(3.0, 7.0, &figure8);
        let _ = winding_number(1.0, 1.0, &figure8);
        let _ = winding_number(9.0, 9.0, &figure8);

        // Point clearly outside should have winding 0
        assert_eq!(winding_number(15.0, 5.0, &figure8), 0);
        assert_eq!(winding_number(-5.0, 5.0, &figure8), 0);
    }

    #[test]
    fn test_concave_polygon() {
        // L-shaped polygon
        let l_shape = vec![
            vec2(0.0, 0.0),
            vec2(10.0, 0.0),
            vec2(10.0, 5.0),
            vec2(5.0, 5.0),
            vec2(5.0, 10.0),
            vec2(0.0, 10.0),
        ];

        // Inside the L
        assert_eq!(winding_number(2.0, 2.0, &l_shape), 1);
        assert_eq!(winding_number(2.0, 7.0, &l_shape), 1);

        // Outside the L (in the concave part)
        assert_eq!(winding_number(7.0, 7.0, &l_shape), 0);
    }

    #[test]
    fn test_point_on_edge() {
        let square = vec![
            vec2(0.0, 0.0),
            vec2(10.0, 0.0),
            vec2(10.0, 10.0),
            vec2(0.0, 10.0),
        ];

        // Points on edges
        assert!(point_on_polygon_edge(5.0, 0.0, &square, 0.001));
        assert!(point_on_polygon_edge(10.0, 5.0, &square, 0.001));
        assert!(point_on_polygon_edge(0.0, 5.0, &square, 0.001));

        // Corner points
        assert!(point_on_polygon_edge(0.0, 0.0, &square, 0.001));
        assert!(point_on_polygon_edge(10.0, 10.0, &square, 0.001));

        // Point not on edge
        assert!(!point_on_polygon_edge(5.0, 5.0, &square, 0.001));
    }

    #[test]
    fn test_empty_and_degenerate() {
        assert_eq!(winding_number(0.0, 0.0, &[]), 0);
        assert_eq!(winding_number(0.0, 0.0, &[vec2(0.0, 0.0)]), 0);
        assert_eq!(
            winding_number(0.0, 0.0, &[vec2(0.0, 0.0), vec2(1.0, 1.0)]),
            0
        );
    }
}
