WASM Strict Errors

Overview
- All strict methods (suffix `_res`) validate inputs and never panic. They return a Result-like object:
  - `{ ok: true, value: T }` on success
  - `{ ok: false, error: { code, message, data? } }` on failure
- Legacy methods remain tolerant for backward compatibility (may clamp values or return null/false).

Error Codes
- invalid_id: data { kind: 'node'|'edge'|'region', id }
- invalid_mode: data { got }
- invalid_end: data omitted (end must be 0 or 1)
- out_of_range: data { param, min, max, got }
- non_finite: data { param }
- not_cubic: data { edge }
- not_polyline: data { edge }
- invalid_array: data { param, expected }
- json_parse: data omitted (message contains details)
- svg_parse: data omitted (message contains details)

Invariants
- On error: state is not mutated (no geometry changes, `geom_version` unchanged).
- Strict methods do not clamp; they reject invalid inputs with a typed error.
- Picking returns `{ ok: true, value: null }` when nothing is hit; this is not an error.

Examples
- `bend_edge_to_res(id, t, tx, ty, k)`
  - Errors: invalid_id(edge), non_finite(t|tx|ty|stiffness), out_of_range(t ∉ [0,1]), out_of_range(stiffness ≤ 0)
- `set_handle_pos_res(id, end, x, y)`
  - Errors: invalid_id(edge), invalid_end(end∉{0,1}), non_finite(x|y), not_cubic(edge)
- `add_edge_res(a, b)`
  - Errors: invalid_id(node), invalid_edge (a==b)

Migration
- Prefer `_res` methods for production usage. Legacy methods remain for compatibility and demo ergonomics.

## Behavior Matrix (Clamp vs Error)

| Operation | Legacy method | Legacy behavior (degenerates) | Strict method | Strict validation/errors |
|---|---|---|---|---|
| Add node | `add_node(x,y)` | Accepts any finite; creates node | `add_node_res` | `non_finite(x|y)` |
| Move node | `move_node(id,x,y)` | Returns false if id invalid | `move_node_res` | `invalid_id(node)`, `non_finite(x|y)` |
| Add edge | `add_edge(a,b)` | Returns `None` if ids invalid or `a==b` | `add_edge_res` | `invalid_id(node)`, `invalid_edge (a==b)` |
| Remove node | `remove_node(id)` | Returns false if id invalid; removes incident edges | `remove_node_res` | `invalid_id(node)` |
| Remove edge | `remove_edge(id)` | Returns false if id invalid | `remove_edge_res` | `invalid_id(edge)` |
| Set cubic | `set_edge_cubic(id,p1,p2)` | If both handles ~0 → keep Line | `set_edge_cubic_res` | `invalid_id(edge)`, `non_finite(p1|p2)` |
| Set line | `set_edge_line(id)` | Always sets if edge exists | `set_edge_line_res` | `invalid_id(edge)` |
| Get handles | `get_handles(id)` | `None` if not cubic | `get_handles_res` | `invalid_id(edge)`, `not_cubic` |
| Handle pos | `set_handle_pos(id,end,x,y)` | Returns false if `end∉{0,1}` or not cubic; constraints enforced; degenerates no‑op | `set_handle_pos_res` | `invalid_id(edge)`, `invalid_end`, `non_finite(x|y)`, `not_cubic` |
| Handle mode | `set_handle_mode(id,mode)` | Non-cubic → false; constraints enforced | `set_handle_mode_res` | `invalid_id(edge)`, `invalid_mode`, `not_cubic` |
| Bend | `bend_edge_to(id,t,tx,ty,k)` | Clamps `t∈[0,1]`; zero-length edges no‑op; guards small denom; Line→Cubic unless degenerate | `bend_edge_to_res` | `invalid_id(edge)`, `non_finite(t|tx|ty|stiffness)`, `out_of_range(t, [0,1])`, `out_of_range(stiffness>0)` |
| Pick | `pick(x,y,tol)` | Returns `null` if no hit | `pick_res` | `non_finite(x|y|tol)`, `out_of_range(tol≥0)`; returns `{ ok:true, value:null }` if no hit |
| Regions | `get_regions()` | Filters tiny faces (`EPS_FACE_AREA`); robust to degenerates | `get_regions_res` | Same as legacy (wrapped in `{ ok }`) |
| Toggle fill | `toggle_region(key)` | No-op if key unknown | `toggle_region_res` | `invalid_id(region)` if key unknown |
| Set flatten tol | `set_flatten_tolerance(tol)` | Clamps to `[0.01, 10.0]` | `set_flatten_tolerance_res` | `non_finite(tol)`, `out_of_range(0.01≤tol≤10.0)` |
| Add SVG | `add_svg_path(d)` | Best-effort parse; merges coincident endpoints; returns count | `add_svg_path_res` | `svg_parse` when no edges parsed |
| To SVG | `to_svg_paths()` | Skips malformed edges | `to_svg_paths_res` | Always `{ ok:true, value:string[] }` |
| JSON import | `from_json(v)` | Ignores edges with missing endpoints; never panics | `from_json_res` | `{ ok:true, value:bool }` or `json_parse` |

Notes
- Legacy methods favor smooth UX: clamping or no-op where possible; never panic.
- Strict methods validate upfront and preserve invariants: on error, no geometry mutation and version unchanged.
