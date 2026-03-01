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
await init(wasmBytes);
const graph = new Graph();
graph.get_regions();
console.log("vecnet-wasm npm smoke test passed");
