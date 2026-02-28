# ruvector-mincut-wasm

WebAssembly bindings for [ruvector-mincut](https://crates.io/crates/ruvector-mincut) - the world's first subpolynomial-time dynamic minimum cut implementation.

## Features

- **Browser & Node.js**: Works in any JavaScript environment with WASM support
- **Full API**: Complete access to dynamic mincut operations
- **Zero Dependencies**: Pure WASM, no runtime requirements

## Installation

```bash
npm install ruvector-mincut-wasm
```

## Usage

```javascript
import init, { DynamicMinCut } from 'ruvector-mincut-wasm';

await init();
const graph = new DynamicMinCut(100);
graph.addEdge(0, 1, 1.0);
const mincut = graph.computeMinCut();
```

## Performance

- O(n^{1-ε}) query time for dynamic minimum cut
- Matches theoretical lower bounds
- SIMD-optimized when available

## License

MIT

## See Also

- [ruvector-mincut](https://crates.io/crates/ruvector-mincut) - Core Rust implementation
- [ruvector-mincut-node](https://crates.io/crates/ruvector-mincut-node) - Node.js native bindings
