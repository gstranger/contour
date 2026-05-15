import init, { Graph } from "vecnet-wasm";
import { createRequire } from "module";
import path from "path";
import { readdirSync, readFileSync } from "fs";

const require = createRequire(import.meta.url);
const pkgRoot = path.dirname(require.resolve("vecnet-wasm/package.json"));
const wasmDir = path.join(pkgRoot, "pkg", "default");
const wasmFile = readdirSync(wasmDir).find((name) => name.endsWith(".wasm"));
if (!wasmFile) {
  throw new Error("vecnet-wasm artifact not found");
}
const wasmBytes = readFileSync(path.join(wasmDir, wasmFile));
await init({ module_or_path: wasmBytes });

const graph = new Graph();

// Exercise the round-trip the way a real consumer would: build a tiny shape,
// query it, and serialize it back out. Failures here mean broken bindings even
// when the package merely loads.
const a = graph.add_node(0, 0);
const b = graph.add_node(10, 0);
const c = graph.add_node(10, 10);
graph.add_edge(a, b);
graph.add_edge(b, c);
graph.add_edge(c, a);

if (graph.node_count() !== 3) throw new Error(`node_count expected 3, got ${graph.node_count()}`);
if (graph.edge_count() !== 3) throw new Error(`edge_count expected 3, got ${graph.edge_count()}`);

const regions = graph.get_regions();
if (!Array.isArray(regions)) throw new Error("get_regions did not return an array");

const json = graph.to_json();
if (json === null || typeof json !== "object") throw new Error("to_json did not return an object");

// Exercise get_dirty: checkpoint, mutate, observe the diff.
graph.dirty_reset();
const checkpoint = graph.geom_version();
graph.add_node(20, 5);
const diff = graph.get_dirty(checkpoint);
if (diff.full) throw new Error("get_dirty unexpectedly returned full=true");
if (!Array.isArray(diff.nodes_added) || diff.nodes_added.length !== 1) {
  throw new Error(`get_dirty nodes_added expected length 1, got ${JSON.stringify(diff.nodes_added)}`);
}
if (diff.current_ver <= checkpoint) {
  throw new Error("get_dirty current_ver did not advance");
}

console.log("vecnet-wasm npm smoke test passed");
