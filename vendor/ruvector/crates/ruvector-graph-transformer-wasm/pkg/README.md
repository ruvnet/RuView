# ruvector-graph-transformer-wasm

[![Crates.io](https://img.shields.io/crates/v/ruvector-graph-transformer-wasm.svg)](https://crates.io/crates/ruvector-graph-transformer-wasm)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**WebAssembly bindings for RuVector Graph Transformer — proof-gated graph attention, verified training, and 8 specialized graph layers running client-side in the browser.**

Run the full graph transformer in any browser tab — no server, no API calls, no data leaving the device. Every graph mutation is formally verified client-side, so your users get the same mathematical safety guarantees as the Rust version. The WASM binary is size-optimized and loads in milliseconds.

## Install

```bash
# With wasm-pack (recommended)
wasm-pack build crates/ruvector-graph-transformer-wasm --target web

# Or from npm (when published)
npm install ruvector-graph-transformer-wasm
```

## Quick Start

```javascript
import init, { JsGraphTransformer } from "ruvector-graph-transformer-wasm";

await init();
const gt = new JsGraphTransformer();
console.log(gt.version()); // "2.0.4"

// Proof-gated mutation
const gate = gt.create_proof_gate(128);
const proof = gt.prove_dimension(128, 128);
console.log(proof.verified); // true

// 82-byte attestation for RVF witness chains
const attestation = gt.create_attestation(proof.proof_id);
console.log(attestation.length); // 82

// Sublinear attention — O(n log n)
const result = gt.sublinear_attention(
  new Float32Array([0.1, 0.2, 0.3, 0.4]),
  [{ src: 0, tgt: 1 }, { src: 0, tgt: 2 }],
  4, 2
);

// Verified training step with certificate
const step = gt.verified_training_step(
  [1.0, 2.0], [0.1, 0.2], 0.01
);
console.log(step.weights, step.certificate);

// Physics: symplectic integration
const state = gt.hamiltonian_step([1.0, 0.0], [0.0, 1.0], 0.01);
console.log(state.energy);

// Biological: spiking attention
const spikes = gt.spiking_attention(
  [0.5, 1.5, 0.3], [[1], [0, 2], [1]], 1.0
);

// Manifold: mixed-curvature distance
const d = gt.product_manifold_distance(
  [1, 0, 0, 1], [0, 1, 1, 0], [0.0, -1.0]
);

// Temporal: causal masking
const scores = gt.causal_attention(
  [1.0, 0.0],
  [[1.0, 0.0], [0.0, 1.0]],
  [1.0, 2.0]
);

// Economic: Nash equilibrium
const nash = gt.game_theoretic_attention(
  [1.0, 0.5, 0.8],
  [{ src: 0, tgt: 1 }, { src: 1, tgt: 2 }]
);
console.log(nash.converged);

// Stats
console.log(gt.stats());
```

## API

### Proof-Gated Operations

| Method | Returns | Description |
|--------|---------|-------------|
| `new JsGraphTransformer(config?)` | `JsGraphTransformer` | Create transformer instance |
| `version()` | `string` | Crate version |
| `create_proof_gate(dim)` | `object` | Create proof gate for dimension |
| `prove_dimension(expected, actual)` | `object` | Prove dimension equality |
| `create_attestation(proof_id)` | `Uint8Array` | 82-byte proof attestation |
| `verify_attestation(bytes)` | `boolean` | Verify attestation from bytes |
| `compose_proofs(stages)` | `object` | Type-checked pipeline composition |

### Sublinear Attention

| Method | Returns | Description |
|--------|---------|-------------|
| `sublinear_attention(q, edges, dim, k)` | `object` | Graph-sparse top-k attention |
| `ppr_scores(source, adj, alpha)` | `Float64Array` | Personalized PageRank scores |

### Physics-Informed

| Method | Returns | Description |
|--------|---------|-------------|
| `hamiltonian_step(positions, momenta, dt)` | `object` | Symplectic leapfrog step |
| `verify_energy_conservation(before, after, tol)` | `object` | Energy conservation proof |

### Biological

| Method | Returns | Description |
|--------|---------|-------------|
| `spiking_attention(spikes, edges, threshold)` | `Float64Array` | Event-driven spiking attention |
| `hebbian_update(pre, post, weights, lr)` | `Float64Array` | Hebbian weight update |
| `spiking_step(features, adjacency)` | `object` | Full spiking step over feature matrix |

### Verified Training

| Method | Returns | Description |
|--------|---------|-------------|
| `verified_step(weights, gradients, lr)` | `object` | SGD step + proof receipt |
| `verified_training_step(features, targets, weights)` | `object` | Training step + certificate |

### Manifold

| Method | Returns | Description |
|--------|---------|-------------|
| `product_manifold_distance(a, b, curvatures)` | `number` | Mixed-curvature distance |
| `product_manifold_attention(features, edges)` | `object` | Product manifold attention |

### Temporal-Causal

| Method | Returns | Description |
|--------|---------|-------------|
| `causal_attention(query, keys, timestamps)` | `Float64Array` | Temporally masked attention |
| `causal_attention_graph(features, timestamps, edges)` | `Float64Array` | Causal graph attention |
| `granger_extract(history, num_nodes, num_steps)` | `object` | Granger causality DAG |

### Economic

| Method | Returns | Description |
|--------|---------|-------------|
| `game_theoretic_attention(features, edges)` | `object` | Nash equilibrium attention |

### Meta

| Method | Returns | Description |
|--------|---------|-------------|
| `stats()` | `object` | Aggregate proof/attestation statistics |
| `reset()` | `void` | Reset all internal state |

## Building

```bash
# Web target (recommended for browsers)
wasm-pack build crates/ruvector-graph-transformer-wasm --target web

# Node.js target
wasm-pack build crates/ruvector-graph-transformer-wasm --target nodejs

# Cargo check
cargo check -p ruvector-graph-transformer-wasm
```

## Bundle Size

The WASM binary is optimized for size with `opt-level = "s"`, LTO, and single codegen unit.

## Related Packages

| Package | Description |
|---------|-------------|
| [`ruvector-graph-transformer`](../ruvector-graph-transformer) | Core Rust crate (186 tests) |
| [`@ruvector/graph-transformer`](../ruvector-graph-transformer-node) | Node.js NAPI-RS bindings |
| [`ruvector-verified-wasm`](../ruvector-verified-wasm) | Formal verification WASM bindings |

## License

MIT
