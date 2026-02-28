# @ruvector/math-wasm

[![npm version](https://img.shields.io/npm/v/@ruvector/math-wasm.svg)](https://www.npmjs.com/package/@ruvector/math-wasm)
[![crates.io](https://img.shields.io/crates/v/ruvector-math-wasm.svg)](https://crates.io/crates/ruvector-math-wasm)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![WASM](https://img.shields.io/badge/target-wasm32-orange.svg)](https://webassembly.org/)

**High-performance WebAssembly bindings for advanced mathematical algorithms in vector search and AI.**

Brings Optimal Transport, Information Geometry, and Product Manifolds to the browser with near-native performance.

## Features

- 🚀 **Optimal Transport** - Sliced Wasserstein, Sinkhorn, Gromov-Wasserstein distances
- 📐 **Information Geometry** - Fisher Information Matrix, Natural Gradient, K-FAC
- 🌐 **Product Manifolds** - E^n × H^n × S^n (Euclidean, Hyperbolic, Spherical)
- ⚡ **SIMD Optimized** - Vectorized operations where available
- 🔒 **Type-Safe** - Full TypeScript definitions included
- 📦 **Zero Dependencies** - Pure Rust compiled to WASM

## Installation

```bash
npm install @ruvector/math-wasm
# or
yarn add ruvector-math-wasm
# or
pnpm add ruvector-math-wasm
```

## Quick Start

### Browser (ES Modules)

```javascript
import init, {
  WasmSlicedWasserstein,
  WasmSinkhorn,
  WasmProductManifold
} from '@ruvector/math-wasm';

// Initialize WASM module
await init();

// Compute Sliced Wasserstein distance
const sw = new WasmSlicedWasserstein(100); // 100 projections
const source = new Float64Array([0, 0, 1, 1, 2, 2]); // 3 points in 2D
const target = new Float64Array([0.5, 0.5, 1.5, 1.5, 2.5, 2.5]);
const distance = sw.distance(source, target, 2);
console.log(`Wasserstein distance: ${distance}`);
```

### Node.js

```javascript
const { WasmSlicedWasserstein } = require('@ruvector/math-wasm');

const sw = new WasmSlicedWasserstein(100);
const dist = sw.distance(source, target, 2);
```

## Use Cases

### 1. Distribution Comparison in ML

Compare probability distributions for generative models, anomaly detection, or data drift monitoring.

```javascript
// Compare embedding distributions
const sw = new WasmSlicedWasserstein(200).withPower(2); // W2 distance

const trainEmbeddings = new Float64Array(/* ... */);
const testEmbeddings = new Float64Array(/* ... */);

const drift = sw.distance(trainEmbeddings, testEmbeddings, 768);
if (drift > threshold) {
  console.warn('Data drift detected!');
}
```

### 2. Semantic Vector Search

Use product manifolds for hierarchical and semantic search.

```javascript
const manifold = new WasmProductManifold({
  euclidean_dim: 256,
  hyperbolic_dim: 128,
  spherical_dim: 128,
  curvature_h: -1.0,
  curvature_s: 1.0
});

// Compute distance in mixed-curvature space
const dist = manifold.distance(queryVector, documentVector);
```

### 3. Optimal Transport for Image Comparison

```javascript
const sinkhorn = new WasmSinkhorn(0.01, 100); // regularization, max_iters

// Compare image histograms
const result = sinkhorn.solveTransport(
  costMatrix,
  sourceWeights,
  targetWeights,
  n, m
);

console.log(`Transport cost: ${result.cost}`);
console.log(`Converged: ${result.converged}`);
```

### 4. Natural Gradient Optimization

```javascript
const fisher = new WasmFisherInformation(512);

// Compute Fisher Information Matrix
const fim = fisher.compute(activations);

// Apply natural gradient
const naturalGrad = fisher.naturalGradientStep(gradient, 0.01);
```

## API Reference

### Optimal Transport

| Class | Description |
|-------|-------------|
| `WasmSlicedWasserstein` | Fast approximation via random projections |
| `WasmSinkhorn` | Entropy-regularized optimal transport |
| `WasmGromovWasserstein` | Cross-space structural comparison |

### Information Geometry

| Class | Description |
|-------|-------------|
| `WasmFisherInformation` | Fisher Information Matrix computation |
| `WasmNaturalGradient` | Natural gradient descent optimizer |

### Product Manifolds

| Class | Description |
|-------|-------------|
| `WasmProductManifold` | E^n × H^n × S^n mixed-curvature space |
| `WasmSphericalSpace` | Spherical geometry operations |

## Performance

Benchmarked on M1 MacBook Pro (WASM in Chrome):

| Operation | Dimension | Time |
|-----------|-----------|------|
| Sliced Wasserstein (100 proj) | 1000 points × 128D | 2.3ms |
| Sinkhorn (100 iter) | 500 × 500 | 8.7ms |
| Product Manifold distance | 512D | 0.04ms |

## TypeScript Support

Full TypeScript definitions are included:

```typescript
import { WasmSlicedWasserstein, WasmSinkhornConfig } from '@ruvector/math-wasm';

const sw: WasmSlicedWasserstein = new WasmSlicedWasserstein(100);
const distance: number = sw.distance(source, target, dim);
```

## Building from Source

```bash
# Install wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Build
cd crates/ruvector-math-wasm
wasm-pack build --target web --release

# Test
wasm-pack test --headless --chrome
```

## Related Packages

- [`ruvector-math`](https://crates.io/crates/ruvector-math) - Rust crate (native)
- [`@ruvector/attention`](https://www.npmjs.com/package/@ruvector/attention) - Attention mechanisms (native Node.js)
- [`@ruvector/attention-wasm`](https://www.npmjs.com/package/@ruvector/attention-wasm) - Attention mechanisms (WASM)

## License

MIT OR Apache-2.0

## Links

- [GitHub](https://github.com/ruvnet/ruvector)
- [Documentation](https://docs.rs/ruvector-math-wasm)
- [crates.io](https://crates.io/crates/ruvector-math-wasm)
- [npm](https://www.npmjs.com/package/ruvector-math-wasm)
