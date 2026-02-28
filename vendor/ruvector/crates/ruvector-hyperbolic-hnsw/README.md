# ruvector-hyperbolic-hnsw

Hyperbolic (Poincaré ball) embeddings with HNSW integration for hierarchy-aware vector search.

## Why Hyperbolic Space?

Hierarchies compress naturally in hyperbolic space. Taxonomies, catalogs, ICD trees, product facets, org charts, and long-tail tags all fit better than in Euclidean space, which means higher recall on deep leaves without blowing up memory or latency.

## Key Features

- **Poincaré Ball Model**: Store vectors in the Poincaré ball with clamp `r < 1 − eps`
- **HNSW Speed Trick**: Prune with cheap tangent-space proxy, rank with true hyperbolic distance
- **Per-Shard Curvature**: Different parts of the hierarchy can have different optimal curvatures
- **Dual-Space Index**: Keep a synchronized Euclidean ANN for fallback and mutual-ranking fusion
- **Production Guardrails**: Numerical stability, canary testing, hot curvature reload

## Installation

### Rust

```toml
[dependencies]
ruvector-hyperbolic-hnsw = "0.1.0"
```

### WebAssembly

```bash
cd crates/ruvector-hyperbolic-hnsw-wasm
wasm-pack build --target web --release
```

### TypeScript/JavaScript

```typescript
import init, {
  HyperbolicIndex,
  poincareDistance,
  mobiusAdd,
  expMap,
  logMap
} from 'ruvector-hyperbolic-hnsw-wasm';

await init();

const index = new HyperbolicIndex(16, 1.0);
index.insert(new Float32Array([0.1, 0.2, 0.3]));
const results = index.search(new Float32Array([0.15, 0.1, 0.2]), 5);
```

## Quick Start

```rust
use ruvector_hyperbolic_hnsw::{HyperbolicHnsw, HyperbolicHnswConfig};

// Create index with default settings
let mut index = HyperbolicHnsw::default_config();

// Insert vectors (automatically projected to Poincaré ball)
index.insert(vec![0.1, 0.2, 0.3]).unwrap();
index.insert(vec![-0.1, 0.15, 0.25]).unwrap();
index.insert(vec![0.2, -0.1, 0.1]).unwrap();

// Search for nearest neighbors
let results = index.search(&[0.15, 0.1, 0.2], 2).unwrap();
for r in results {
    println!("ID: {}, Distance: {:.4}", r.id, r.distance);
}
```

## HNSW Speed Trick

The core optimization:

1. Precompute `u = log_c(x)` at a shard centroid `c`
2. During neighbor selection, use Euclidean `||u_q - u_p||` to prune
3. Run exact Poincaré distance only on top N candidates before final ranking

```rust
use ruvector_hyperbolic_hnsw::{HyperbolicHnsw, HyperbolicHnswConfig};

let mut config = HyperbolicHnswConfig::default();
config.use_tangent_pruning = true;
config.prune_factor = 10; // Consider 10x candidates in tangent space

let mut index = HyperbolicHnsw::new(config);

// ... insert vectors ...

// Build tangent cache for pruning optimization
index.build_tangent_cache().unwrap();

// Search with pruning (faster!)
let results = index.search_with_pruning(&[0.1, 0.15], 5).unwrap();
```

## Core Mathematical Operations

```rust
use ruvector_hyperbolic_hnsw::poincare::{
    mobius_add, exp_map, log_map, poincare_distance, project_to_ball
};

let x = vec![0.3, 0.2];
let y = vec![-0.1, 0.4];
let c = 1.0; // Curvature

// Möbius addition (hyperbolic vector addition)
let z = mobius_add(&x, &y, c);

// Geodesic distance in hyperbolic space
let d = poincare_distance(&x, &y, c);

// Map to tangent space at x
let v = log_map(&y, &x, c);

// Map back to manifold
let y_recovered = exp_map(&v, &x, c);
```

## Sharded Index with Per-Shard Curvature

```rust
use ruvector_hyperbolic_hnsw::{ShardedHyperbolicHnsw, ShardStrategy};

let mut manager = ShardedHyperbolicHnsw::new(1.0);

// Insert with hierarchy depth information
manager.insert(vec![0.1, 0.2], Some(0)).unwrap(); // Root level
manager.insert(vec![0.3, 0.1], Some(3)).unwrap(); // Deeper level

// Update curvature for specific shard
manager.update_curvature("radius_1", 0.5).unwrap();

// Canary testing for new curvature
manager.registry.set_canary("radius_1", 0.3, 10); // 10% traffic

// Search across all shards
let results = manager.search(&[0.2, 0.15], 5).unwrap();
```

## Numerical Stability

All operations include numerical safeguards:

- **Norm clamping**: Points projected with `eps = 1e-5`
- **Projection after updates**: All operations keep points inside the ball
- **Stable acosh**: Uses `log1p` expansions for safety
- **Clamp arguments**: `arctanh` and `atanh` arguments bounded away from ±1

## Evaluation Protocol

### Datasets

- WordNet
- DBpedia slices
- Synthetic scale-free tree
- Domain taxonomy

### Primary Metrics

- **recall@k** (1, 5, 10)
- **Mean rank**
- **NDCG**

### Hierarchy Metrics

- **Radius vs depth Spearman correlation**
- **Distance distortion**
- **Ancestor AUPRC**

### Baselines

- Euclidean HNSW
- OPQ/PQ compressed
- Simple mutual-ranking fusion

### Ablations

- Tangent proxy vs full hyperbolic
- Fixed vs learnable curvature c
- Global vs shard centroids

## Production Integration

### Reflex Loop (on writes)

Small Möbius deltas and tangent-space micro updates that never push points outside the ball.

```rust
use ruvector_hyperbolic_hnsw::tangent_micro_update;

let updated = tangent_micro_update(
    &point,
    &delta,
    &centroid,
    curvature,
    0.1  // max step size
);
```

### Habit (nightly)

Riemannian SGD passes to clean neighborhoods and optionally relearn per-shard curvature. Run canary first.

### Structural (periodic)

Rebuild of HNSW with true hyperbolic metric, curvature retune, and shard reshuffle if hierarchy preservation drops below SLO.

## Dependencies (Exact Versions)

```toml
nalgebra = "0.34.1"
ndarray = "0.17.1"
wasm-bindgen = "0.2.106"
```

## Benchmarks

```bash
cd crates/ruvector-hyperbolic-hnsw
cargo bench
```

Benchmark suite includes:

- Poincaré distance computation
- Möbius addition
- exp/log map operations
- HNSW insert and search
- Tangent cache building
- Search with vs without pruning

## License

MIT

## Related

- [ruvector-attention](../ruvector-attention) - Hyperbolic attention mechanisms
- [micro-hnsw-wasm](../micro-hnsw-wasm) - Minimal HNSW for WASM
- [ruvector-math](../ruvector-math) - General math primitives
