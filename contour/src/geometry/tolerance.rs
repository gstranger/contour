// Centralized tolerances and helpers for robust geometry

pub const EPS_POS: f32 = 1e-4;            // point coincidence threshold (px)
pub const EPS_LEN: f32 = 1e-6;            // zero-length vector threshold
pub const EPS_DENOM: f32 = 1e-8;          // denominator guard for LS/ratios
pub const EPS_FACE_AREA: f32 = 1e-2;      // tiny face area threshold (px^2)
pub const EPS_ANG: f32 = 1e-6;            // angle compare slack (radians)
pub const EPS_CONSTRAINT: f32 = 1e-3;     // constraint tolerance for tests/invariants

// Quantization grid for region graph merging (0.1 px)
pub const QUANT_SCALE: f32 = 10.0;        // 1.0 / 0.1

// Adaptive flattening cap
pub const MAX_FLATTEN_DEPTH: u32 = 16;

#[inline] pub fn clamp01(x: f32) -> f32 { x.max(0.0).min(1.0) }
#[inline] pub fn clamp(x: f32, lo: f32, hi: f32) -> f32 { x.max(lo).min(hi) }
#[inline] pub fn near_zero(x: f32, eps: f32) -> bool { x.abs() <= eps }
#[inline] pub fn approx_eq(a: f32, b: f32, eps: f32) -> bool { (a - b).abs() <= eps }

#[inline]
pub fn norm2(mut x: f32, mut y: f32) -> ((f32,f32), f32) {
    let len = (x*x + y*y).sqrt();
    if len > EPS_LEN { x/=len; y/=len; ((x,y), len) } else { ((0.0,0.0), 0.0) }
}

#[inline]
pub fn safe_div(num: f32, den: f32, fallback: f32) -> f32 {
    if den.abs() <= EPS_DENOM { fallback } else { num/den }
}
