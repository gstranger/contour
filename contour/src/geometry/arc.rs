//! SVG elliptical-arc to cubic Bézier conversion.
//!
//! SVG's `A` command parameterizes an arc by its two endpoints plus radii,
//! x-axis rotation, and two flags. The rest of this codebase works in
//! cubic Béziers, so we convert at parse time using the W3C standard
//! endpoint-to-center conversion (SVG 1.1, Appendix F.6) followed by
//! subdivision into ≤90° segments.
//!
//! The 90°-split-then-cubic approximation introduces error on the order
//! of 0.03% of the radius, well below the parser's 0.01-unit coordinate
//! quantization.

/// A cubic segment as `(control1, control2, end)`. The start point is
/// implicit: it is the end point of the previous segment, or the original
/// start point passed to [`arc_to_cubics`] for the first segment.
pub type ArcCubic = ((f32, f32), (f32, f32), (f32, f32));

/// Convert an SVG elliptical-arc command into a chain of cubic Béziers.
///
/// `phi_deg` is the x-axis rotation in degrees, matching SVG's parameter.
/// Returns an empty vector for degenerate inputs (zero radius or
/// coincident endpoints), per the SVG spec — the caller should emit a
/// straight line in that case if the endpoints differ.
pub fn arc_to_cubics(
    start: (f32, f32),
    rx: f32,
    ry: f32,
    phi_deg: f32,
    large: bool,
    sweep: bool,
    end: (f32, f32),
) -> Vec<ArcCubic> {
    // SVG F.6.2: if endpoints are identical, arc is a no-op.
    if (start.0 - end.0).abs() < 1e-7 && (start.1 - end.1).abs() < 1e-7 {
        return Vec::new();
    }
    // SVG F.6.2: if either radius is zero, arc collapses to a straight
    // line. Empty return signals the caller to emit an L instead.
    if rx == 0.0 || ry == 0.0 {
        return Vec::new();
    }

    // F.6.6.1: take absolute values of radii.
    let mut rx = rx.abs();
    let mut ry = ry.abs();

    let phi = phi_deg.to_radians();
    let (sin_phi, cos_phi) = phi.sin_cos();

    // F.6.5.1: compute (x1', y1'), the start point in a frame rotated by
    // -phi with origin at the chord midpoint.
    let dx = (start.0 - end.0) * 0.5;
    let dy = (start.1 - end.1) * 0.5;
    let x1p = cos_phi * dx + sin_phi * dy;
    let y1p = -sin_phi * dx + cos_phi * dy;

    // F.6.6.2-3: scale radii up if they're too small for the chord.
    let mut rxsq = rx * rx;
    let mut rysq = ry * ry;
    let x1psq = x1p * x1p;
    let y1psq = y1p * y1p;
    let radii_check = x1psq / rxsq + y1psq / rysq;
    if radii_check > 1.0 {
        let scale = radii_check.sqrt();
        rx *= scale;
        ry *= scale;
        rxsq = rx * rx;
        rysq = ry * ry;
    }

    // F.6.5.2: center (cx', cy') in the rotated frame.
    let sign = if large == sweep { -1.0 } else { 1.0 };
    let num = rxsq * rysq - rxsq * y1psq - rysq * x1psq;
    let den = rxsq * y1psq + rysq * x1psq;
    let factor = if num <= 0.0 || den <= 0.0 {
        0.0
    } else {
        (num / den).sqrt()
    };
    let cxp = sign * factor * (rx * y1p / ry);
    let cyp = -sign * factor * (ry * x1p / rx);

    // F.6.5.3: rotate (cx', cy') back and translate to original frame.
    let mx = (start.0 + end.0) * 0.5;
    let my = (start.1 + end.1) * 0.5;
    let cx = cos_phi * cxp - sin_phi * cyp + mx;
    let cy = sin_phi * cxp + cos_phi * cyp + my;

    // F.6.5.4-6: compute start angle theta1 and angle delta dtheta.
    let ux = (x1p - cxp) / rx;
    let uy = (y1p - cyp) / ry;
    let vx = (-x1p - cxp) / rx;
    let vy = (-y1p - cyp) / ry;
    let theta1 = signed_angle(1.0, 0.0, ux, uy);
    let mut dtheta = signed_angle(ux, uy, vx, vy);
    if !sweep && dtheta > 0.0 {
        dtheta -= std::f32::consts::TAU;
    } else if sweep && dtheta < 0.0 {
        dtheta += std::f32::consts::TAU;
    }

    // Subdivide into segments of at most 90° and emit one cubic per
    // segment. The standard ratio k = (4/3) tan(theta/4) puts the
    // control points on the tangent at distance k * r from each
    // endpoint, matching the unit-circle approximation.
    let n_segs = (dtheta.abs() / std::f32::consts::FRAC_PI_2).ceil() as usize;
    let n_segs = n_segs.max(1);
    let seg_angle = dtheta / n_segs as f32;
    let k = (4.0_f32 / 3.0) * (seg_angle * 0.25).tan();

    let map = |cos_a: f32, sin_a: f32| -> (f32, f32) {
        // Point on the unit ellipse, then rotated by phi and translated.
        let ex = rx * cos_a;
        let ey = ry * sin_a;
        (
            cos_phi * ex - sin_phi * ey + cx,
            sin_phi * ex + cos_phi * ey + cy,
        )
    };
    let tangent = |cos_a: f32, sin_a: f32| -> (f32, f32) {
        // d/da of the unit ellipse point, then rotated by phi.
        let tx = -rx * sin_a;
        let ty = ry * cos_a;
        (cos_phi * tx - sin_phi * ty, sin_phi * tx + cos_phi * ty)
    };

    let mut out = Vec::with_capacity(n_segs);
    for i in 0..n_segs {
        let a1 = theta1 + i as f32 * seg_angle;
        let a2 = a1 + seg_angle;
        let (sa1, ca1) = a1.sin_cos();
        let (sa2, ca2) = a2.sin_cos();
        let p0 = map(ca1, sa1);
        let p3 = map(ca2, sa2);
        let (t1x, t1y) = tangent(ca1, sa1);
        let (t2x, t2y) = tangent(ca2, sa2);
        let p1 = (p0.0 + k * t1x, p0.1 + k * t1y);
        let p2 = (p3.0 - k * t2x, p3.1 - k * t2y);
        out.push((p1, p2, p3));
    }

    // Snap the final endpoint to the caller-supplied `end`. The
    // trigonometric round-trip can leave a tiny gap that produces
    // distinct quantized nodes in the parser's node cache.
    if let Some(last) = out.last_mut() {
        last.2 = end;
    }
    out
}

/// Signed angle from (ux, uy) to (vx, vy), in radians, in (-pi, pi].
fn signed_angle(ux: f32, uy: f32, vx: f32, vy: f32) -> f32 {
    let dot = ux * vx + uy * vy;
    let lu = (ux * ux + uy * uy).sqrt();
    let lv = (vx * vx + vy * vy).sqrt();
    if lu == 0.0 || lv == 0.0 {
        return 0.0;
    }
    let cos_t = (dot / (lu * lv)).clamp(-1.0, 1.0);
    let s = if ux * vy - uy * vx >= 0.0 { 1.0 } else { -1.0 };
    s * cos_t.acos()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    fn point_eq(p: (f32, f32), q: (f32, f32), eps: f32) -> bool {
        approx_eq(p.0, q.0, eps) && approx_eq(p.1, q.1, eps)
    }

    /// Evaluate a cubic Bézier at parameter t given p0, p1, p2, p3.
    fn cubic_eval(
        p0: (f32, f32),
        p1: (f32, f32),
        p2: (f32, f32),
        p3: (f32, f32),
        t: f32,
    ) -> (f32, f32) {
        let mt = 1.0 - t;
        let b0 = mt * mt * mt;
        let b1 = 3.0 * mt * mt * t;
        let b2 = 3.0 * mt * t * t;
        let b3 = t * t * t;
        (
            b0 * p0.0 + b1 * p1.0 + b2 * p2.0 + b3 * p3.0,
            b0 * p0.1 + b1 * p1.1 + b2 * p2.1 + b3 * p3.1,
        )
    }

    #[test]
    fn endpoints_match() {
        let start = (0.0, 0.0);
        let end = (100.0, 0.0);
        let cubics = arc_to_cubics(start, 50.0, 50.0, 0.0, false, true, end);
        assert!(!cubics.is_empty());
        // Last cubic ends at `end` (snapped).
        assert_eq!(cubics.last().unwrap().2, end);
    }

    #[test]
    fn degenerate_endpoints_returns_empty() {
        let cubics = arc_to_cubics((5.0, 5.0), 10.0, 10.0, 0.0, true, true, (5.0, 5.0));
        assert!(cubics.is_empty());
    }

    #[test]
    fn zero_radius_returns_empty() {
        let cubics = arc_to_cubics((0.0, 0.0), 0.0, 50.0, 0.0, false, true, (10.0, 0.0));
        assert!(cubics.is_empty());
        let cubics = arc_to_cubics((0.0, 0.0), 50.0, 0.0, 0.0, false, true, (10.0, 0.0));
        assert!(cubics.is_empty());
    }

    #[test]
    fn negative_radii_use_absolute_value() {
        let a = arc_to_cubics((0.0, 0.0), 50.0, 50.0, 0.0, false, true, (100.0, 0.0));
        let b = arc_to_cubics((0.0, 0.0), -50.0, -50.0, 0.0, false, true, (100.0, 0.0));
        assert_eq!(a.len(), b.len());
        for (ca, cb) in a.iter().zip(b.iter()) {
            assert!(point_eq(ca.0, cb.0, 1e-4));
            assert!(point_eq(ca.1, cb.1, 1e-4));
            assert!(point_eq(ca.2, cb.2, 1e-4));
        }
    }

    /// Sample the assembled cubic chain at uniform parameter intervals.
    fn samples(start: (f32, f32), cubics: &[ArcCubic], per_seg: usize) -> Vec<(f32, f32)> {
        let mut out = Vec::with_capacity(cubics.len() * per_seg + 1);
        out.push(start);
        let mut prev = start;
        for c in cubics {
            for step in 1..=per_seg {
                let t = step as f32 / per_seg as f32;
                out.push(cubic_eval(prev, c.0, c.1, c.2, t));
            }
            prev = c.2;
        }
        out
    }

    #[test]
    fn semicircle_passes_through_expected_midpoint() {
        // M 0 0 A 50 50 0 0 1 100 0 — semicircle (2r == chord). SVG's
        // angle parameterization runs from theta=180° to theta=360° in
        // the positive-angle direction for sweep=1, passing through
        // theta=270°, which puts the geometric midpoint at (50, -50).
        let start = (0.0, 0.0);
        let end = (100.0, 0.0);
        let cubics = arc_to_cubics(start, 50.0, 50.0, 0.0, false, true, end);
        assert_eq!(cubics.len(), 2);
        let mid_of_first = cubic_eval(start, cubics[0].0, cubics[0].1, cubics[0].2, 1.0);
        assert!(
            point_eq(mid_of_first, (50.0, -50.0), 0.5),
            "expected midpoint near (50, -50), got {:?}",
            mid_of_first
        );
    }

    #[test]
    fn large_flag_chooses_long_arc() {
        // Use rx=ry=60 with chord length 100 so 2r > chord and the small
        // and large arcs differ. Large sweep covers more arc length, so
        // it should travel farther from the chord.
        let start = (0.0, 0.0);
        let end = (100.0, 0.0);
        let small = arc_to_cubics(start, 60.0, 60.0, 0.0, false, true, end);
        let large = arc_to_cubics(start, 60.0, 60.0, 0.0, true, true, end);
        let max_dev = |pts: &[(f32, f32)]| pts.iter().map(|p| p.1.abs()).fold(0.0_f32, f32::max);
        let dev_small = max_dev(&samples(start, &small, 20));
        let dev_large = max_dev(&samples(start, &large, 20));
        assert!(
            dev_large > dev_small,
            "large={} should exceed small={}",
            dev_large,
            dev_small
        );
    }

    #[test]
    fn sweep_flag_chooses_side() {
        // Sweep=0 and sweep=1 put the arc on opposite sides of the chord.
        let start = (0.0, 0.0);
        let end = (100.0, 0.0);
        let s0 = arc_to_cubics(start, 60.0, 60.0, 0.0, false, false, end);
        let s1 = arc_to_cubics(start, 60.0, 60.0, 0.0, false, true, end);
        let signed_dev = |pts: &[(f32, f32)]| {
            let mut max_neg = 0.0_f32;
            let mut max_pos = 0.0_f32;
            for p in pts {
                if p.1 < max_neg {
                    max_neg = p.1;
                }
                if p.1 > max_pos {
                    max_pos = p.1;
                }
            }
            // Whichever side has the larger magnitude is "the" side.
            if max_pos > -max_neg {
                max_pos
            } else {
                max_neg
            }
        };
        let d0 = signed_dev(&samples(start, &s0, 20));
        let d1 = signed_dev(&samples(start, &s1, 20));
        assert!(
            d0 * d1 < 0.0,
            "expected opposite sides, got sweep0={} sweep1={}",
            d0,
            d1
        );
    }

    #[test]
    fn radii_too_small_are_scaled_up() {
        // Endpoints 100 apart, both radii only 10: the spec says scale
        // them up so the arc is just barely reachable. Should not produce
        // NaN.
        let start = (0.0, 0.0);
        let end = (100.0, 0.0);
        let cubics = arc_to_cubics(start, 10.0, 10.0, 0.0, false, true, end);
        assert!(!cubics.is_empty());
        let mut prev = start;
        for c in cubics {
            assert!(c.0 .0.is_finite() && c.0 .1.is_finite());
            assert!(c.1 .0.is_finite() && c.1 .1.is_finite());
            assert!(c.2 .0.is_finite() && c.2 .1.is_finite());
            prev = c.2;
        }
        assert!(point_eq(prev, end, 1e-3));
    }

    #[test]
    fn rotated_arc_endpoints_match() {
        // Arc with non-zero x-axis-rotation. Endpoints should still match.
        let start = (10.0, 20.0);
        let end = (60.0, 70.0);
        let cubics = arc_to_cubics(start, 30.0, 50.0, 30.0, false, true, end);
        assert!(!cubics.is_empty());
        assert!(point_eq(cubics.last().unwrap().2, end, 1e-3));
    }

    #[test]
    fn approximation_error_within_tolerance() {
        // For a unit circle, sample the cubic chain densely and check
        // that each sampled point is within ~0.03% of radius 1.
        let start = (1.0, 0.0);
        let end = (-1.0, 0.0);
        let cubics = arc_to_cubics(start, 1.0, 1.0, 0.0, false, true, end);
        let mut prev = start;
        let mut max_err: f32 = 0.0;
        for c in cubics {
            for step in 0..=20 {
                let t = step as f32 / 20.0;
                let p = cubic_eval(prev, c.0, c.1, c.2, t);
                let r = (p.0 * p.0 + p.1 * p.1).sqrt();
                max_err = max_err.max((r - 1.0).abs());
            }
            prev = c.2;
        }
        assert!(max_err < 5e-4, "max error {} too large", max_err);
    }
}
