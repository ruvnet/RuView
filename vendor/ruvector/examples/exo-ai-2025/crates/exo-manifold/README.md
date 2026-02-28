# exo-manifold

Continuous embedding space with SIREN networks for smooth manifold
deformation in cognitive AI. Provides the geometric foundation that
lets EXO-AI substrates represent and transform concepts as points on
learned manifolds.

## Features

- **SIREN coordinate network** -- uses sinusoidal representation
  networks (SIREN) to learn implicit neural representations of
  continuous coordinate spaces with high-frequency detail.
- **Manifold deformation** -- smoothly warps the embedding manifold to
  adapt cognitive geometry in response to new information, preserving
  local neighbourhood structure.
- **Transfer prior store with domain-pair indexing** -- caches learned
  deformation priors indexed by (source, target) domain pairs so that
  cross-domain transfers start from an informed initialisation.

## Quick Start

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
exo-manifold = "0.1"
```

Basic usage:

```rust
use exo_manifold::ManifoldEngine;
use exo_core::{ManifoldConfig, Pattern};
use burn::backend::NdArray;

// Create engine with default SIREN parameters
let config = ManifoldConfig::default();
let device = Default::default();
let mut engine = ManifoldEngine::<NdArray>::new(config, device);

// Deform manifold with a high-salience pattern
let pattern = Pattern { /* ... */ };
engine.deform(pattern, 0.9)?;

// Retrieve similar patterns via gradient descent
let query = vec![/* embedding */];
let results = engine.retrieve(&query, 10)?;

// Strategic forgetting of low-salience regions
engine.forget(0.5, 0.1)?;
```

## Crate Layout

| Module      | Purpose                                      |
|-------------|----------------------------------------------|
| `network`   | SIREN network definition and forward pass     |
| `retrieval` | Gradient descent retrieval algorithm           |
| `deform`    | Manifold deformation and curvature regulation |
| `forgetting`| Gaussian smoothing and weight pruning          |
| `transfer`  | Prior store with domain-pair indexing          |

## Requirements

- Rust 1.78+
- Depends on `exo-core`, `burn`, `burn-ndarray`

## Links

- [GitHub](https://github.com/ruvnet/ruvector)
- [EXO-AI Documentation](https://github.com/ruvnet/ruvector/tree/main/examples/exo-ai-2025)

## License

MIT OR Apache-2.0
