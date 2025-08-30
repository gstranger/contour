# contour

Minimal Rust + WASM library for Figma‑style vector networks (undirected graphs) targeting the browser.

It exposes a small API to JavaScript for creating a graph, connecting nodes, listing neighbors, and computing shortest paths (BFS).

## Quickstart

```bash
# 1) Prereqs
rustup target add wasm32-unknown-unknown
cargo install wasm-pack

# 2) Build the WASM bundle (from contour-wasm/)
# outputs to ../pkg so the web demo can import it
cd contour-wasm && wasm-pack build --target web --out-dir ../pkg && cd -

# 3) Serve statically (from repo root)
python3 -m http.server

# 4) Open the demo in your browser
# visit http://localhost:8000/web/index.html
```

This produces a `pkg/` folder with JS/WASM bindings that the demo imports.

## Prerequisites

- Rust toolchain
- Target `wasm32-unknown-unknown`: `rustup target add wasm32-unknown-unknown`
- `wasm-pack` (recommended): `cargo install wasm-pack`

## Build

From the workspace `contour-wasm/` crate:

```bash
wasm-pack build --target web --out-dir ../pkg
```

This creates a `pkg/` folder with JS/WASM bindings.

## Test

WASM unit tests are written with `wasm-bindgen-test` under `tests/`.

Run in a headless browser (requires Chrome or Firefox installed):

```bash
# Chrome
wasm-pack test --headless --chrome

# or Firefox
wasm-pack test --headless --firefox
```

You can also run them in a non-headless browser by omitting `--headless`.

## Run the demo

Serve the folder via any static file server (so the browser can fetch the WASM file). For example, from `contour/`:

```bash
python3 -m http.server
# or use any other static server
```

Then open `http://localhost:8000/web/index.html`.

Demo controls:
- Import SVG: click "Import SVG" or drag-and-drop a `.svg` onto the canvas. The demo parses all `<path d>` commands (M/L/C/Z) and imports them into the graph.
- Save/Load: persists the current graph to `localStorage`.
- Clear: removes all nodes and edges.
- Bucket (F): toggles region fills.
- Bend (B): drag directly on a curve to bend.

## JS API (via wasm-bindgen)

- `new Graph()`
- `graph.add_node() -> number`
- `graph.add_edge(a: number, b: number) -> boolean`
- `graph.node_count() -> number`
- `graph.edge_count() -> number`
- `graph.neighbors(id: number) -> number[] | null`
- `graph.shortest_path(start: number, goal: number) -> number[] | null`
- `graph.add_svg_path(d: string) -> number` (append path data; supports M/L/C/Z)
- `graph.to_svg_paths() -> string[]` (export independent path fragments)
- `graph.get_regions() -> [{ key, area, filled, color?: [r,g,b,a], points[] }]`
- `graph.toggle_region(key: number) -> boolean`
- `graph.set_region_fill(key: number, filled: boolean)`
- `graph.set_region_color(key: number, r: number, g: number, b: number, a: number)`

### Freehand (new)

- `graph.add_freehand(points: Float32Array, close: boolean) -> Uint32Array`
  - Fits a smooth cubic Bezier chain (Catmull–Rom with corner detection) through sampled points.
  - Intended for the Pen → Free Draw mode in the demo.

## Notes

- The crate is built as a `cdylib` for WebAssembly and uses `wasm-bindgen` for bindings.
- The simple web demo imports from `./pkg/contour_wasm.js`, which is created by building the `contour-wasm` crate.
- JSON `to_json` now includes `version` and `fills` arrays; `from_json` reads them.
