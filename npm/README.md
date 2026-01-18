# Contour

WebAssembly bindings for the Contour vector network engine.

```
npm install contour
```

```js
import init, { Graph } from "contour";

await init();
const graph = new Graph();
console.log(graph.get_regions());
```

## Feature Variants

- `import init from "contour"` – default build
- `import init from "contour/simd"` – compiles with `+simd128`
- `import init from "contour/threads"` – enables wasm threads (`SharedArrayBuffer` required)

Each build ships typed bindings and a versioned `.wasm` artifact.
