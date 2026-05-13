// Verify that the built threads .wasm is actually a threaded build (i.e., it
// imports a *shared* memory). Fails loudly if release_npm.sh ever silently
// regresses to a non-threaded artifact under the ./threads export.
//
// Usage: node scripts/verify-threads.mjs [path-to.wasm]
// Default path: npm/pkg/threads/contour_bg_v<version>_threads.wasm
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";

const explicit = process.argv[2];
const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const threadsDir = path.join(repoRoot, "npm", "pkg", "threads");

let wasmPath = explicit;
if (!wasmPath) {
  const candidates = readdirSync(threadsDir).filter((n) => n.endsWith(".wasm"));
  if (candidates.length !== 1) {
    console.error(`verify-threads: expected exactly one .wasm in ${threadsDir}, found ${candidates.length}`);
    process.exit(2);
  }
  wasmPath = path.join(threadsDir, candidates[0]);
}

const buf = readFileSync(wasmPath);
const mod = new WebAssembly.Module(buf);
const memImports = WebAssembly.Module.imports(mod).filter((i) => i.kind === "memory");
if (memImports.length !== 1) {
  console.error(`verify-threads: expected 1 memory import, got ${memImports.length}`);
  process.exit(1);
}

// Probe the shared flag by attempting to instantiate against a non-shared memory.
// A threaded wasm declares `shared = 1` and the engine refuses the mismatch.
// We use a deliberately oversized initial so the failure is the shared-state
// mismatch and not a pages-too-small mismatch.
const { module: mod_name, name } = memImports[0];
const nonShared = new WebAssembly.Memory({ initial: 4096, maximum: 16384, shared: false });
try {
  new WebAssembly.Instance(mod, { [mod_name]: { [name]: nonShared } });
  console.error(`verify-threads: FAIL — ${wasmPath} accepted a non-shared memory; this is not a threaded build`);
  process.exit(1);
} catch (e) {
  if (!/shared state/.test(e.message)) {
    console.error(`verify-threads: FAIL — ${wasmPath} rejected non-shared memory but not for the expected reason`);
    console.error(`  engine said: ${e.message}`);
    process.exit(1);
  }
  console.log(`verify-threads: OK — ${path.basename(wasmPath)} requires shared memory`);
}
