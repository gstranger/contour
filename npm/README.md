# vecnet-wasm

WebAssembly bindings for the vecnet-wasm vector network engine.

```
npm install vecnet-wasm
```

```js
import init, { Graph } from "vecnet-wasm";

await init();
const graph = new Graph();
console.log(graph.get_regions());
```

## Feature Variants

- `import init from "vecnet-wasm"` – default build
- `import init from "vecnet-wasm/simd"` – compiles with `+simd128`
- `import init from "vecnet-wasm/threads"` – enables wasm threads (`SharedArrayBuffer` required)

Each build ships typed bindings and a versioned `.wasm` artifact.
