# The Build Succeeded. The Artifact Did Not.

A couple of days ago, while landing a v1-hardening PR on a wasm vector-graphics library, my CI's `npm-smoke` job failed on a check I'd added that morning:

```
verify-threads: FAIL — contour_bg_v0.1.0_threads.wasm
declares no shared memory; not a threaded build
```

The script parses the wasm binary directly and asserts that the memory section declares the `shared` flag. The threads build recipe it was checking had been in the repo for months, building successfully every time. wasm-pack reported "Done"; wasm-bindgen processed it without complaint; wasm-opt optimized it without error.

The bytes told a different story. The wasm had `(memory $0 17)`, plain non-shared memory. The threads-variant npm artifact, the one we were about to ship, wasn't actually threaded.

If I hadn't added the verifier that morning, the next npm publish would have shipped a threads-variant package that wasn't threaded. Consumers would have spun up worker pools that silently fell back to single-threaded execution. No error to surface in telemetry, just unexplained perf cliffs in production.

## Was this user error?

Technically yes. The interesting part is what "technically" hides.

I didn't pass enough linker flags. The recipe in `release_npm.sh` was the standard one, copy-pasted from the rustwasm book and countless threads-wasm blog posts:

```bash
RUSTUP_TOOLCHAIN=nightly
RUSTFLAGS="-C target-feature=+atomics,+bulk-memory,+mutable-globals"
wasm-pack build --target web -- \
  --features threads -Z build-std=panic_abort,std
```

That recipe worked on `nightly-2025-06-12`. It doesn't work on `nightly-2026-05-14`. Sometime in those eleven months, the relationship between Rust's `+atomics` target feature and the linker's memory layout decisions changed. The atomics feature still emits atomic instructions. The linker no longer infers `--shared-memory --import-memory --max-memory=...` from its presence. You pass them yourself now, plus TLS symbol exports (`__wasm_init_tls`, `__tls_size`, `__tls_align`, `__tls_base`) that wasm-bindgen's threads-xform pass needs to inject per-thread initialization code.

The full recipe for current nightly:

```bash
RUSTFLAGS="-C target-feature=+atomics,+bulk-memory,+mutable-globals \
  -C link-arg=--shared-memory \
  -C link-arg=--import-memory \
  -C link-arg=--max-memory=4294967296 \
  -C link-arg=--export=__heap_base \
  -C link-arg=--export=__data_end \
  -C link-arg=--export=__wasm_init_tls \
  -C link-arg=--export=__tls_size \
  -C link-arg=--export=__tls_align \
  -C link-arg=--export=__tls_base"
```

Did I do something wrong by not passing those? Technically yes. But the recipe was the canonical one, copy-pasted from documentation that was correct when written and is now silently incorrect. Nothing emitted a deprecation warning. The toolchain saw a partial specification, made a defensible default choice (non-shared memory), and proceeded.

This isn't language drift or a regression in Rust's safety story. It's a cliff in the contract between what target features mean and what the produced artifact actually contains. The cliffs are everywhere.

## Why this isn't a memory-safety story

Wasm, shared memory, and atomics together can sound like a Rust safety story. It isn't.

Rust's safety guarantees are language-level. Within safe Rust, the compiler rules out data races, use-after-free, null derefs, and so on. Those guarantees are about program behavior, and they held here. The non-threaded wasm would have run safely. It would have run slowly, with one worker doing all the work and the others idle, but no safety violations.

What broke is at a different layer: the shape of the compiled artifact. Things like whether the wasm declares shared memory, whether memory is imported or internal, which TLS symbols get exported, which target features end up in the producers section.

Rust has no language contract about any of that. It can't, really. Those are properties of compilation output, controlled by codegen options, linker flags, and post-processing tools. The language doesn't even know they exist.

The real story: artifact integrity is a separate, weaker contract from program correctness, and almost nobody puts it in CI.

## What each layer knows

To understand the silent failure, you need to know what each layer in a wasm build pipeline knows and doesn't know about threads.

`rustc` with `+atomics` emits atomic instructions wherever the std rebuild calls for them. On current nightly, it does not mark the produced memory as shared. That's the linker's job.

`wasm-ld` defaults to producing a non-shared internal memory. To get a shared imported memory, it needs `--shared-memory --import-memory --max-memory=N` passed explicitly. Without those flags, atomic instructions still compile and run. They just operate on a non-shared memory, which means every "atomic" operation is process-local and you have no actual threading.

`wasm-bindgen` with its threads-xform pass requires the memory to be imported, not internal. It asserts `mem.import.is_some()`. If your wasm has an internal shared memory, threads-xform panics. If your wasm has a non-shared memory, threads-xform processes it without complaint and emits a non-threaded glue layer.

`wasm-opt` (binaryen) strips features it doesn't recognize. Without `--enable-threads`, it rewrites shared memory back to non-shared during optimization. The only sign anything happened is that the file got smaller.

`wasm-pack` orchestrates the previous three and exposes a Cargo.toml metadata config for wasm-opt args. In v0.13, the config is honored locally but behaves differently in different runner environments. I never got a satisfying explanation. We sidestepped by disabling wasm-pack's wasm-opt step and invoking wasm-opt manually.

Each layer treats partial specification as a defensible default, doing something sensible with what it was given and moving on. The composition is a pipeline where any layer can silently drop or transform a property you specified at the start.

## The verifier

The detection mechanism is fifty lines of Node:

```javascript
import { readFileSync } from "node:fs";

const buf = readFileSync(wasmPath);

function findSharedMemory(buf) {
  // Walk wasm sections, find imports section (id 2) and memory
  // section (id 5), inspect each memory's limits flag byte for
  // the shared bit (0x02).
  //   https://webassembly.github.io/threads/core/binary/types.html
  // ... about 50 lines of binary parsing ...
}

const found = findSharedMemory(buf);
if (!found) {
  console.error("FAIL — no shared memory; not a threaded build");
  process.exit(1);
}
```

Three constants from the wasm spec, a ULEB128 reader, a section walker. It bypasses the toolchain and reads what's actually in the bytes. When the standard build produced `(memory $0 17)` instead of `(memory $0 17 shared)`, the script caught it. Nothing else did.

The lesson generalizes. A small parser, written once, answers the exact question we actually care about: does the artifact have the property we're shipping it for? That question rarely has the same answer as "did the build exit zero."

## The general principle

When you ship a build variant whose purpose is to provide a property (threads, SIMD, fully-linked debug info, PIE, stripped symbols, a specific compression dictionary, a signing chain), write a binary-level assertion that the artifact has that property. Don't trust the toolchain to enforce it.

The wasm-threads story is one instance. Others:

- Native binaries. You enabled stack canaries with a compiler flag. Did the linker put them in? `checksec` knows. Your build script doesn't.
- Container images. Your "slim" image extends `python:3.12-slim` and copies in only your wheel. Did your CI end up with a slim image, or a bloated one because someone added a transient `apt-get install -y curl` four PRs ago? Image size is the verifier.
- Bundlers. You marked a chunk as lazy-loaded. Did webpack split it, or did it inline the chunk because it was under the inline-limit heuristic? The chunk graph is the verifier.
- CI itself. You have `cargo clippy -- -D warnings` in CI. Did clippy run with those flags, or did a workflow include silently relax them? Print the resolved invocation in the log.
- Code signing. Did you sign the artifact, or did the signing step succeed against a stub because the secret wasn't in this branch's context? Verify against the public cert at install.

The failure mode is the same each time: a property you thought you had is silently absent, the build reports success, the consequences appear in production. The fix is the same too: a parser that reads the artifact and asserts the property.

## What people get wrong

Two anti-patterns.

First, "run the integration test." The assumption that if the feature works in a smoke test, the artifact has the right shape. Often it doesn't. A non-threaded wasm runs fine in a single-worker test, a debug-symbols binary runs fine in a smoke test, a non-stripped binary runs fine in any test. Smoke tests check that the artifact works. They rarely check that it is what you said it was.

Second, "the toolchain would tell us." Modern toolchains compose programs that each take partial input and produce partial output, each with its own classification of what's an error, what's a warning, what's a default. The intersection of those decisions is "an artifact got produced." Whether it's the artifact you wanted is a separate question that only direct inspection can answer.

## What to do

If you ship compiled artifacts (native binaries, wasm, JVM bytecode, container images, signed packages), adopt this discipline:

1. Write a verifier for every shipped property. Threads, SIMD, debug info, signing, size budgets, feature flags baked in. If you advertise it on the package description, you owe yourself a CI check that asserts the artifact has it.

2. Make the verifier read the artifact, not the build log. Build logs describe what the build tried to do. The artifact is ground truth.

3. Run the verifier as a gate, not an advisory check. Mine runs in 30ms. The difference is catching the regression on PR versus hearing about it from a customer.

4. Write the verifier in a different language from the build. A regression in the Rust toolchain can't propagate into a Node script that just reads the file.

5. Don't trust ecosystem recipes more than a year old. Especially for fast-moving targets: wasm threads, container builds, signing chains, anything LLVM-adjacent. Canonical recipes rot. The verifier catches it.

## Closing

I spent the better part of a day on this. Filename glob mismatch first, then apt's binaryen too old to parse Rust 1.95 wasm, then wasm-opt feature preservation, then wasm-bindgen ignoring `WASM_BINDGEN_FLAGS`, then wasm-pack metadata behaving differently between local and CI. Finally, at the bottom, the linker flags that used to be inferred from `+atomics` and now aren't. Each fix revealed the next problem. The verifier was the only thing telling me whether each attempt had actually worked.

Languages do an excellent job at the layer they cover. Rust's safety story is genuinely best-in-class. That excellence makes it easy to forget that the layers below the language are essentially shell scripts gluing together C++ programs that exchange data through environment variables. The properties of your shipped artifact live in those layers. They're not protected by the guarantees you rely on day-to-day.

If you ship a property, verify the property. Fifty lines of parser is the only piece of CI between you and a silent regression.
