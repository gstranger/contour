Regions: Planarization and Face Walk

Overview
- Flatten all edges into line segments (lines direct; cubics via adaptive flattening; polylines expanded).
- Robustly planarize: compute segment intersections with f64 predicates; split segments at intersection parameters; quantize vertices to a 0.1 px grid.
- Accelerate intersections with a uniform grid keyed by segment AABBs to prune pair checks; deduplicate pairs across cells.
- Build half-edges in both directions; sort outgoing by angle at each vertex; face-walk CCW to extract bounded faces.
- Compute stable region keys from canonical edge-id sequences; filter tiny/degenerate faces.

Robust Intersections
- Predicates: orientation tests in f64; classify disjoint, proper cross, endpoint touch, and collinear overlap.
- Tolerances: EPS_POS (endpoint/touch threshold), EPS_DENOM (parallel guard). We only use EPS_ANG for traversal angle comparisons.
- Endpoint touches: snap to the same vertex (no extra split beyond 0/1 parameters).
- Collinear overlaps: split at the overlap boundaries; drop zero-length segments.

Quantization
- Vertex identity uses QUANT_SCALE = 10.0 (0.1 px grid). All split points are quantized after geometric computation (f64) to avoid over-merging.
- Averaging: vertices sharing a quantized key store the average of their raw positions to reduce drift.
- Guards: subsegments shorter than EPS_POS are dropped to avoid zero-length half-edges.

Face Walk
- At half-edge u→v, at vertex v choose the previous edge in the angle-sorted outgoing list relative to the reverse v→u direction (left-hand, CCW rule).
- Traversal caps steps to guard against infinite loops; only accept cycles that are closed, have ≥3 vertices, and area |A| ≥ EPS_FACE_AREA.
- Region keys: derive from edge id sequences around the face using minimal rotation (both directions) and hash with FNV-1a. Consecutive duplicates of the same edge id are compressed.

Unsupported/Trade-offs
- Prolonged exact collinear overlaps across many edges produce ambiguous interiors; we split and filter zero-area faces, but “inside” is undefined there.
- Micro self-intersections below EPS_POS may collapse to degenerate faces which are filtered; topology is stable but tiny faces may be missed.
- Precision is bounded by flattening tolerance for curves; we do not insert vertices on the original curve beyond flattened segments.

Behavior Under Self-Touch
- Within tolerance, self-touches do not crash; endpoint joints are snapped; near-crossings outside tolerance are ignored.
- When no bounded faces exist, a fallback detects degree-2 simple cycles and reconstructs boundaries using real edge geometry.

Performance Notes
- Intersections use a uniform grid bucketing to reduce candidate pairs, deduplicating pairs across cells. Bbox checks remain as a quick reject.
