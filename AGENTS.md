# vecnet-wasm — Architecture and Approach

This library provides a Rust + WebAssembly engine for Figma‑style vector networks that runs in the browser. It exposes a compact JS API for building and editing networks (nodes + edges), rendering via Canvas, interactive picking, fills, and direct “bend” manipulation of curves.

## Goals
- Vector networks (not just single paths): connect anything to anything; multi‑degree nodes are first class.
- Natural editing: drag nodes or curve points (bend) directly; handles and modes for precision.
- Fills that “just work”: enclosed regions auto‑fill; a paint bucket toggles regions without winding rules.
- Browser‑ready: small, predictable WASM API; real‑time interaction with Canvas renderer.

---

## Core Model
- Nodes: stable integer ids with positions `(x, y)`.
- Edges: stable integer ids; endpoints `(a, b)`; kind `Line` or `Cubic { ha, hb, mode }` where `ha/hb` are per‑end handle offsets and `mode` ∈ {Free, Mirrored, Aligned}.
- Geometry version: `geom_ver` increments on any geometry mutation; used to drive caching.
- Fills: map from stable region keys → boolean (filled). Keys are topology‑stable (see “Regions”).

Data is stored in arenas (`Vec<Option<…>>`) for stable ids and O(1) lookups.

---

## WASM API (selected)
- Nodes/Edges: `add_node`, `move_node`, `remove_node`, `add_edge`, `remove_edge`.
- Getters: `get_node_data() -> { ids: Uint32Array, positions: Float32Array }`, `get_edge_data() -> { ids, endpoints, kinds }`.
- Picking: `pick(x, y, tol) -> { kind: 'node'|'edge'|'handle', … }` with edge param `t` for curves.
- Curves: `set_edge_cubic`, `set_edge_line`, `get_handles`, `set_handle_pos`, `set_handle_mode`.
- Bend tool: `bend_edge_to(edge_id, t, tx, ty, stiffness)` — direct manipulation of a curve point.
- Fills/Regions: `get_regions() -> [{ key, area, filled, points[] }]`, `toggle_region(key)`.
- SVG: `add_svg_path(d)`, `to_svg_paths()`.
- JSON: `to_json()` (versioned, includes `fills`), `from_json()`.
- Tuning: `set_flatten_tolerance(px)`; `geom_version()`.

The API favors typed arrays and compact structs for efficient JS interop.

---

## Demo UX (Canvas)
- Drag nodes to move; Alt+drag from node to connect; hover highlights.
- Bend tool (B) or button: click an edge and drag directly on the curve.
- Convert to cubic (C); cycle handle modes (M). Save/Load/Clear via localStorage.
- Fill bucket (F): toggles enclosed regions; hover outlines the region.
- Real‑time region recompute is throttled to once per animation frame for stability.

---

## Algorithms & Key Techniques

### Picking
- Nodes by radius test.
- Edges:
  - Lines: analytic projection.
  - Cubics: sample distance (fast approximation) + return param `t`.
- Handles: nearest to absolute handle positions, prioritized over edges/nodes.

### Curves
- Cubic evaluation and a de Casteljau subdivision used for flattening and distance sampling.
- Handle modes:
  - Free: independent.
  - Mirrored: equal length, opposite directions.
  - Aligned: opposite directions; preserves the opposite handle’s length.

### Bend (direct curve drag)
- Given `t` and target `(tx, ty)`, compute the minimal‑change ΔP1/ΔP2 for control points so the cubic point at `t` moves toward the target:
  - Constraint: `C1*ΔP1 + C2*ΔP2 = d` where `C1 = 3(1−t)^2 t`, `C2 = 3(1−t) t^2`, `d = target − current`.
  - Minimize `λ1|ΔP1|^2 + λ2|ΔP2|^2 →` closed‑form LS solution; enforce mode constraints afterward.
  - Lines auto‑convert to cubics on first bend with default handles (~30% of segment).

### Regions (fills)
- Build a planar graph by flattening each edge:
  - Lines → 1 segment; Cubics → adaptively flattened with tolerance (default 0.25 px, configurable).
  - Quantize vertices (0.1 px) to merge joins; create half‑edges in both directions.
  - At each vertex, sort outgoing by angle; face‑traverse by always taking the next CCW edge.
- Fallback: if no faces found, detect simple degree‑2 cycles; construct the boundary following real edge geometry (not chords).
- Stable region keys: derived from the canonical sequence of edge ids around the face (minimal rotation in both directions), hashed (FNV‑1a). This key is topology‑stable under bends.
- Fill persistence under edits: when topology doesn’t change, keys stay the same. If topology changes, a nearest‑centroid remap is used as a best effort.

### Rendering
- Canvas 2D for the demo; edges drawn as lines or `bezierCurveTo`. Regions draw behind edges. Hover outlines use the same cached region polygons.
- Real‑time stability: regions recompute at most once per animation frame.

---

## Performance & Stability
- Versioning: `geom_ver` increments on geometry edits; consumers cache and refresh by version.
- Real‑time throttle: region recompute scheduled via `requestAnimationFrame` to avoid thrash.
- Flatten tolerance: configurable trade‑off between fidelity and cost; default 0.25 px.
- Stable keys: outlines/fills don’t “hop” during bends; avoids centroid jitter.
- JS interop: returns simple arrays/typed arrays; avoids per‑face object churn where possible.

---

## SVG Compatibility
- Import: `M/m`, `L/l`, `C/c`, `Z/z` supported; merges coincident endpoints to shared nodes; `C` becomes cubic edges.
- Export: per‑edge fragments (`M L` or `M C`). Intended for round‑trip and quick interop; path compaction and arcs are future work.

---

## Testing
- Headless wasm-bindgen tests (Chrome/Firefox):
  - Nodes/edges basics and typed arrays
  - Picking correctness
  - JSON round‑trip and clear
  - Cubic handles set/move; handle picking
  - SVG import/export basics

Run: `wasm-pack test --headless --chrome` (or `--firefox`).

---

## Roadmap (next)
- SVG: arcs (A) → cubic approximation; path compaction on export.
- Dirty diffs: `get_dirty(since)` for nodes/edges/regions.
- Worker offload: region detection + flattening to Web Worker for big scenes.
- Selection visuals: halo underlay, keep strokes at 2 px while editing to avoid fill overlap artifacts.
- Region preview overlay: draw current curve as an overlay during bend and swap in recomputed faces next frame (ultra‑smooth).

---

## Constraints & Known Trade‑offs
- Region extraction is a pragmatic face‑walk with quantization and flattening; extreme precision or self‑intersections may require more robust planarization.
- Cubic distance is sampled, not analytic; good for picking but not exact.
- Canvas demo is intentionally simple; production UIs likely want GPU tessellation and retained rendering.

---

## How to Use (quick)
1) Build: `wasm-pack build --target web`
2) Serve: `python3 -m http.server` and open `web/index.html`
3) Interact: bend (B), bucket (F), convert to cubic (C), cycle modes (M), Alt+drag to connect, Save/Load.
4) Tune: `g.set_flatten_tolerance(0.15)` for tighter region outlines.

---

## Philosophy
Lead with a topology‑first model (vector networks) and make editing feel direct. Keep the WASM boundary lean, use typed arrays for throughput, and build rendering as a replaceable layer. Favor stable identities (edge‑loop keys, versioning) to make UI state resilient under continuous edits.
