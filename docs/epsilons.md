Epsilon Policy

Goals
- Consistent, centralized tolerances across geometry.
- Clamp parameters to valid domains and treat degenerates as no-ops.
- Avoid panics in all edit paths while keeping UI stable.

Constants
- EPS_POS (1e-4): point coincidence threshold in pixels.
- EPS_LEN (1e-6): zero-length vector threshold.
- EPS_DENOM (1e-8): denominator guard for least-squares/ratios.
- EPS_FACE_AREA (1e-2): tiny face filter area in px^2.
- EPS_ANG (1e-6): angular comparison slack in radians.
- QUANT_SCALE (10.0): 0.1 px grid for region quantization.
- MAX_FLATTEN_DEPTH (16): recursion cap for cubic flattening.

Helpers
- clamp01(x): clamps parameter x to [0,1].
- clamp(x, lo, hi): generic clamp.
- near_zero(x, eps): absolute near-zero check.
- approx_eq(a, b, eps): approximate equality.
- norm2(x, y): returns ((ux,uy), len) with EPS_LEN guard.
- safe_div(num, den, fallback): guarded division for small denominators.

Usage Map
- Picking: clamp cubic t; handle degenerate cubics (all points coincident).
- Bend: clamp t; guard denom; zero-length segments become no-ops; handle modes use EPS_LEN when normalizing.
- Regions: QUANT_SCALE replaces magic 10.0; EPS_FACE_AREA filters tiny faces; CCW traversal uses tolerant angle step.
- Flatten: cap recursion with MAX_FLATTEN_DEPTH.

Behavioral Guarantees
- No panics: degenerate inputs are treated as no-ops.
- Stable topology: tiny faces filtered; quantization consistent at 0.1 px.
- WASM API: parameters outside domains are clamped; edits succeed or return gracefully.

