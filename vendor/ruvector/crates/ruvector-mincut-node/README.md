# ruvector-mincut-node

Node.js native bindings for [ruvector-mincut](https://crates.io/crates/ruvector-mincut) - the world's first subpolynomial-time dynamic minimum cut implementation.

## Features

- **Native Performance**: Built with NAPI-RS for maximum speed
- **Full API**: Complete access to dynamic mincut operations
- **Type Definitions**: Full TypeScript support

## Installation

```bash
npm install ruvector-mincut-node
```

## Usage

```javascript
const { DynamicMinCut } = require('ruvector-mincut-node');

const graph = new DynamicMinCut(100);
graph.addEdge(0, 1, 1.0);
const mincut = graph.computeMinCut();
```

## Performance

- O(n^{1-ε}) query time for dynamic minimum cut
- Native Rust performance via NAPI-RS
- SIMD-optimized with AVX2/SSE support

## Supported Platforms

- Linux x64 (glibc/musl)
- macOS x64/ARM64
- Windows x64

## License

MIT

## See Also

- [ruvector-mincut](https://crates.io/crates/ruvector-mincut) - Core Rust implementation
- [ruvector-mincut-wasm](https://crates.io/crates/ruvector-mincut-wasm) - WebAssembly bindings
