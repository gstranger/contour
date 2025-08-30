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

