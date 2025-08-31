// Centralized ingestion limits to harden against untrusted input (JSON/SVG)

// Scene size caps
pub const MAX_NODES: usize = 200_000;
pub const MAX_EDGES: usize = 300_000;

// Polylines
pub const MAX_POLYLINE_POINTS_PER_EDGE: usize = 8_000;
pub const MAX_POLYLINE_POINTS_TOTAL: usize = 2_000_000;

// SVG caps
pub const MAX_SVG_TOKENS: usize = 8 * 1024 * 1024; // 8 MB worth of characters in the 'd' string
pub const MAX_SVG_COMMANDS: usize = 200_000;
pub const MAX_SVG_SUBPATHS: usize = 10_000;
pub const MAX_SVG_SEGMENTS: usize = 500_000; // expanded segments across L/C/Z

// Numeric bounds
pub const COORD_MIN: f32 = -10_000_000.0;
pub const COORD_MAX: f32 =  10_000_000.0;
pub const WIDTH_MAX: f32 = 10_000.0;

#[inline]
pub fn in_coord_bounds(x: f32) -> bool { x.is_finite() && x >= COORD_MIN && x <= COORD_MAX }

#[inline]
pub fn in_width_bounds(w: f32) -> bool { w.is_finite() && w > 0.0 && w <= WIDTH_MAX }

