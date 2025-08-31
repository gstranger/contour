Ingestion Limits (JSON/SVG)

Purpose
- Harden loaders against untrusted input to avoid DoS or panics.

Caps
- Nodes: 200,000
- Edges: 300,000
- Polyline points per edge: 8,000
- Polyline points total: 2,000,000
- SVG `d` length: 8 MB
- SVG commands: 200,000
- SVG subpaths: 10,000
- SVG expanded segments: 500,000

Numeric Bounds
- Coordinates: [-1e7, 1e7]
- Width: (0, 1e4]
- Colors: 0–255

Behavior
- Strict APIs (`from_json_res`, `add_svg_path_res`) return typed errors (json_parse/svg_parse, caps_exceeded, out_of_bounds, invalid_structure).
- Legacy APIs return `false`/`0` on failure; never panic.

Notes
- Limits are conservative defaults intended for interactive workloads; tune for batch importers as needed.

