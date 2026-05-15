// Verify that the built threads .wasm is actually a threaded build (i.e., it
// declares shared memory). Fails loudly if release_npm.sh ever silently
// regresses to a non-threaded artifact under the ./threads export.
//
// Some toolchains (wasm-bindgen + wasm-pack with --target web) emit shared
// memory as an *internal* memory definition rather than an import; both forms
// are valid threaded wasms, so we accept either by parsing the wasm binary.
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

// Walk the wasm binary looking for any memory (imported or internally defined)
// whose limits flags include the shared bit (0x02). Spec:
// https://webassembly.github.io/threads/core/binary/types.html#binary-limits
function readULEB128(buf, off) {
  let result = 0;
  let shift = 0;
  let bytes = 0;
  while (true) {
    const b = buf[off + bytes];
    bytes++;
    result |= (b & 0x7f) << shift;
    if ((b & 0x80) === 0) break;
    shift += 7;
    if (shift > 35) throw new Error("ULEB128 overflow");
  }
  return [result >>> 0, bytes];
}

function skipLimits(buf, off) {
  const flags = buf[off++];
  const [, mb] = readULEB128(buf, off);
  off += mb;
  if (flags & 0x01) {
    const [, xb] = readULEB128(buf, off);
    off += xb;
  }
  return { flags, off };
}

function findSharedMemory(buf) {
  if (buf.length < 8 || buf[0] !== 0x00 || buf[1] !== 0x61 || buf[2] !== 0x73 || buf[3] !== 0x6d) {
    throw new Error("not a wasm binary (bad magic)");
  }
  let off = 8;
  while (off < buf.length) {
    const id = buf[off++];
    const [size, sb] = readULEB128(buf, off);
    off += sb;
    const sectionEnd = off + size;
    if (id === 2) {
      // Imports section
      let p = off;
      const [count, cb] = readULEB128(buf, p);
      p += cb;
      for (let i = 0; i < count; i++) {
        const [modLen, ml] = readULEB128(buf, p);
        p += ml + modLen;
        const [nameLen, nl] = readULEB128(buf, p);
        p += nl + nameLen;
        const kind = buf[p++];
        if (kind === 0) {
          const [, x] = readULEB128(buf, p);
          p += x;
        } else if (kind === 1) {
          p += 1; // reftype byte
          const r = skipLimits(buf, p);
          p = r.off;
        } else if (kind === 2) {
          const r = skipLimits(buf, p);
          p = r.off;
          if (r.flags & 0x02) return { kind: "imported", flags: r.flags };
        } else if (kind === 3) {
          p += 2; // valtype + mut
        } else if (kind === 4) {
          // tag: attribute byte + typeidx
          p += 1;
          const [, x] = readULEB128(buf, p);
          p += x;
        }
      }
    } else if (id === 5) {
      // Memory section
      let p = off;
      const [count, cb] = readULEB128(buf, p);
      p += cb;
      for (let i = 0; i < count; i++) {
        const r = skipLimits(buf, p);
        p = r.off;
        if (r.flags & 0x02) return { kind: "internal", flags: r.flags };
      }
    }
    off = sectionEnd;
  }
  return null;
}

const found = findSharedMemory(buf);
if (!found) {
  console.error(`verify-threads: FAIL — ${wasmPath} declares no shared memory; not a threaded build`);
  process.exit(1);
}
console.log(`verify-threads: OK — ${path.basename(wasmPath)} declares ${found.kind} shared memory (flags=0x${found.flags.toString(16)})`);
