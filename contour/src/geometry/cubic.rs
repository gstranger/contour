//! Cubic Bézier curve utilities for subdivision and manipulation.
//!
//! These helpers are used by boolean operations to split curves at
//! intersection points and reconstruct curve segments.

use crate::model::Vec2;

/// Control points of a cubic Bézier curve.
#[derive(Clone, Copy, Debug)]
pub struct CubicBezier {
    pub p0: Vec2, // Start point
    pub p1: Vec2, // First control point
    pub p2: Vec2, // Second control point
    pub p3: Vec2, // End point
}

impl CubicBezier {
    pub fn new(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2) -> Self {
        Self { p0, p1, p2, p3 }
    }

    /// Evaluate the curve at parameter t ∈ [0, 1].
    pub fn eval(&self, t: f32) -> Vec2 {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        Vec2 {
            x: mt3 * self.p0.x + 3.0 * mt2 * t * self.p1.x + 3.0 * mt * t2 * self.p2.x + t3 * self.p3.x,
            y: mt3 * self.p0.y + 3.0 * mt2 * t * self.p1.y + 3.0 * mt * t2 * self.p2.y + t3 * self.p3.y,
        }
    }

    /// Evaluate the tangent (derivative) at parameter t.
    pub fn tangent(&self, t: f32) -> Vec2 {
        let t2 = t * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;

        Vec2 {
            x: 3.0 * mt2 * (self.p1.x - self.p0.x)
                + 6.0 * mt * t * (self.p2.x - self.p1.x)
                + 3.0 * t2 * (self.p3.x - self.p2.x),
            y: 3.0 * mt2 * (self.p1.y - self.p0.y)
                + 6.0 * mt * t * (self.p2.y - self.p1.y)
                + 3.0 * t2 * (self.p3.y - self.p2.y),
        }
    }

    /// Split the curve at parameter t using de Casteljau subdivision.
    ///
    /// Returns two cubic curves: the first from 0..t, the second from t..1.
    pub fn split_at(&self, t: f32) -> (CubicBezier, CubicBezier) {
        // de Casteljau algorithm
        let p01 = lerp_vec2(self.p0, self.p1, t);
        let p12 = lerp_vec2(self.p1, self.p2, t);
        let p23 = lerp_vec2(self.p2, self.p3, t);

        let p012 = lerp_vec2(p01, p12, t);
        let p123 = lerp_vec2(p12, p23, t);

        let p0123 = lerp_vec2(p012, p123, t); // The split point

        let first = CubicBezier::new(self.p0, p01, p012, p0123);
        let second = CubicBezier::new(p0123, p123, p23, self.p3);

        (first, second)
    }

    /// Extract a portion of the curve from t0 to t1.
    pub fn subcurve(&self, t0: f32, t1: f32) -> CubicBezier {
        if t0 >= t1 {
            return CubicBezier::new(self.eval(t0), self.eval(t0), self.eval(t0), self.eval(t0));
        }

        // First split at t1, take first part
        let (curve_to_t1, _) = self.split_at(t1);

        // Then split at t0 relative to [0, t1]
        let t0_relative = t0 / t1;
        let (_, result) = curve_to_t1.split_at(t0_relative);

        result
    }

    /// Compute approximate arc length using adaptive subdivision.
    pub fn arc_length(&self, tolerance: f32) -> f32 {
        arc_length_recursive(self.p0, self.p1, self.p2, self.p3, tolerance, 0)
    }

    /// Find parameter t for a given arc length from start.
    /// Returns None if arc_length is greater than total length.
    pub fn parameter_at_arc_length(&self, target_length: f32, tolerance: f32) -> Option<f32> {
        let total = self.arc_length(tolerance);
        if target_length >= total {
            return if target_length <= total * 1.01 {
                Some(1.0)
            } else {
                None
            };
        }
        if target_length <= 0.0 {
            return Some(0.0);
        }

        // Binary search for t
        let mut lo = 0.0f32;
        let mut hi = 1.0f32;

        for _ in 0..32 {
            let mid = (lo + hi) * 0.5;
            let (left, _) = self.split_at(mid);
            let len = left.arc_length(tolerance);

            if (len - target_length).abs() < tolerance * 0.1 {
                return Some(mid);
            }

            if len < target_length {
                lo = mid;
            } else {
                hi = mid;
            }
        }

        Some((lo + hi) * 0.5)
    }
}

/// Split a cubic bezier at parameter t.
///
/// Input: endpoints (p0, p3) and control points (p1, p2)
/// Output: Two sets of (p0, p1, p2, p3) for the split curves
pub fn split_cubic_at(
    p0: Vec2,
    p1: Vec2,
    p2: Vec2,
    p3: Vec2,
    t: f32,
) -> ((Vec2, Vec2, Vec2, Vec2), (Vec2, Vec2, Vec2, Vec2)) {
    let curve = CubicBezier::new(p0, p1, p2, p3);
    let (first, second) = curve.split_at(t);

    (
        (first.p0, first.p1, first.p2, first.p3),
        (second.p0, second.p1, second.p2, second.p3),
    )
}

/// Map a position along flattened segments back to approximate cubic parameter.
///
/// Given a list of flattened line segments and a target segment index + local t,
/// estimate the corresponding t value on the original cubic.
pub fn flat_position_to_cubic_t(
    curve: &CubicBezier,
    flat_segments: &[(Vec2, Vec2)],
    segment_idx: usize,
    local_t: f32,
    tolerance: f32,
) -> f32 {
    if flat_segments.is_empty() {
        return 0.5;
    }

    // Compute cumulative arc lengths up to each segment
    let mut cumulative = Vec::with_capacity(flat_segments.len() + 1);
    cumulative.push(0.0f32);

    for seg in flat_segments {
        let prev = *cumulative.last().unwrap();
        let dx = seg.1.x - seg.0.x;
        let dy = seg.1.y - seg.0.y;
        let len = (dx * dx + dy * dy).sqrt();
        cumulative.push(prev + len);
    }

    let total_flat_length = *cumulative.last().unwrap();
    if total_flat_length < 1e-10 {
        return 0.5;
    }

    // Target length along the flattened path
    let idx = segment_idx.min(flat_segments.len() - 1);
    let seg_start = cumulative[idx];
    let seg_end = cumulative[idx + 1];
    let target_length = seg_start + local_t * (seg_end - seg_start);

    // Convert to parameter on cubic
    curve.parameter_at_arc_length(target_length, tolerance).unwrap_or(0.5)
}

/// Linear interpolation between two Vec2s.
#[inline]
fn lerp_vec2(a: Vec2, b: Vec2, t: f32) -> Vec2 {
    Vec2 {
        x: a.x + t * (b.x - a.x),
        y: a.y + t * (b.y - a.y),
    }
}

/// Recursive arc length computation with adaptive subdivision.
fn arc_length_recursive(
    p0: Vec2,
    p1: Vec2,
    p2: Vec2,
    p3: Vec2,
    tolerance: f32,
    depth: u32,
) -> f32 {
    const MAX_DEPTH: u32 = 16;

    // Chord length
    let dx = p3.x - p0.x;
    let dy = p3.y - p0.y;
    let chord = (dx * dx + dy * dy).sqrt();

    // Control polygon length
    let d01x = p1.x - p0.x;
    let d01y = p1.y - p0.y;
    let d12x = p2.x - p1.x;
    let d12y = p2.y - p1.y;
    let d23x = p3.x - p2.x;
    let d23y = p3.y - p2.y;

    let poly_len = (d01x * d01x + d01y * d01y).sqrt()
        + (d12x * d12x + d12y * d12y).sqrt()
        + (d23x * d23x + d23y * d23y).sqrt();

    // If flat enough or max depth reached, use average of chord and polygon
    if depth >= MAX_DEPTH || (poly_len - chord).abs() < tolerance {
        return (chord + poly_len) * 0.5;
    }

    // Subdivide at t=0.5 using de Casteljau
    let p01 = lerp_vec2(p0, p1, 0.5);
    let p12 = lerp_vec2(p1, p2, 0.5);
    let p23 = lerp_vec2(p2, p3, 0.5);
    let p012 = lerp_vec2(p01, p12, 0.5);
    let p123 = lerp_vec2(p12, p23, 0.5);
    let mid = lerp_vec2(p012, p123, 0.5);

    arc_length_recursive(p0, p01, p012, mid, tolerance, depth + 1)
        + arc_length_recursive(mid, p123, p23, p3, tolerance, depth + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vec2(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }

    #[test]
    fn test_eval_endpoints() {
        let curve = CubicBezier::new(
            vec2(0.0, 0.0),
            vec2(1.0, 2.0),
            vec2(3.0, 2.0),
            vec2(4.0, 0.0),
        );

        let start = curve.eval(0.0);
        let end = curve.eval(1.0);

        assert!((start.x - 0.0).abs() < 1e-6);
        assert!((start.y - 0.0).abs() < 1e-6);
        assert!((end.x - 4.0).abs() < 1e-6);
        assert!((end.y - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_split_at_midpoint() {
        let curve = CubicBezier::new(
            vec2(0.0, 0.0),
            vec2(1.0, 2.0),
            vec2(3.0, 2.0),
            vec2(4.0, 0.0),
        );

        let (first, second) = curve.split_at(0.5);

        // First curve should start at original start
        assert!((first.p0.x - 0.0).abs() < 1e-6);

        // Second curve should end at original end
        assert!((second.p3.x - 4.0).abs() < 1e-6);

        // Split point should match
        let mid = curve.eval(0.5);
        assert!((first.p3.x - mid.x).abs() < 1e-6);
        assert!((first.p3.y - mid.y).abs() < 1e-6);
        assert!((second.p0.x - mid.x).abs() < 1e-6);
        assert!((second.p0.y - mid.y).abs() < 1e-6);
    }

    #[test]
    fn test_split_continuity() {
        let curve = CubicBezier::new(
            vec2(0.0, 0.0),
            vec2(0.0, 10.0),
            vec2(10.0, 10.0),
            vec2(10.0, 0.0),
        );

        let (first, second) = curve.split_at(0.3);

        // Sample points on split curves should match original
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let orig_t = t * 0.3;
            let orig_point = curve.eval(orig_t);
            let split_point = first.eval(t);

            assert!(
                (orig_point.x - split_point.x).abs() < 1e-4,
                "x mismatch at t={}: {} vs {}",
                t,
                orig_point.x,
                split_point.x
            );
            assert!(
                (orig_point.y - split_point.y).abs() < 1e-4,
                "y mismatch at t={}: {} vs {}",
                t,
                orig_point.y,
                split_point.y
            );
        }
    }

    #[test]
    fn test_arc_length_straight_line() {
        // A "cubic" that's actually a straight line
        let curve = CubicBezier::new(
            vec2(0.0, 0.0),
            vec2(1.0, 0.0),
            vec2(2.0, 0.0),
            vec2(3.0, 0.0),
        );

        let length = curve.arc_length(0.01);
        assert!((length - 3.0).abs() < 0.1, "Expected ~3.0, got {}", length);
    }

    #[test]
    fn test_subcurve() {
        let curve = CubicBezier::new(
            vec2(0.0, 0.0),
            vec2(1.0, 2.0),
            vec2(3.0, 2.0),
            vec2(4.0, 0.0),
        );

        let sub = curve.subcurve(0.25, 0.75);

        // Start and end should match original curve at those parameters
        let start = curve.eval(0.25);
        let end = curve.eval(0.75);

        assert!((sub.p0.x - start.x).abs() < 1e-4);
        assert!((sub.p0.y - start.y).abs() < 1e-4);
        assert!((sub.p3.x - end.x).abs() < 1e-4);
        assert!((sub.p3.y - end.y).abs() < 1e-4);
    }

    #[test]
    fn test_parameter_at_arc_length() {
        let curve = CubicBezier::new(
            vec2(0.0, 0.0),
            vec2(0.0, 1.0),
            vec2(1.0, 1.0),
            vec2(1.0, 0.0),
        );

        let total = curve.arc_length(0.01);
        let half = curve.parameter_at_arc_length(total / 2.0, 0.01).unwrap();

        // Should be roughly in the middle
        assert!(half > 0.4 && half < 0.6, "Expected ~0.5, got {}", half);
    }
}
