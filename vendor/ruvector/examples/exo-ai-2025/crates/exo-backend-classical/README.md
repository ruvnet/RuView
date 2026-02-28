# exo-backend-classical

Classical compute backend for the EXO-AI cognitive substrate with SIMD
acceleration. Implements the `SubstrateBackend` trait from `exo-core` on
standard CPU hardware, optimised for throughput and energy efficiency.

## Features

- **SIMD-accelerated vector operations** -- uses platform SIMD intrinsics
  (SSE4.2, AVX2, NEON) for fast dot products, cosine similarity, and
  element-wise transforms.
- **Dither quantization integration** -- applies stochastic dithered
  quantization to compress activations while preserving gradient signal.
- **Thermodynamic layer (thermorust)** -- wraps every compute step with
  Landauer energy accounting so the substrate can track real
  thermodynamic cost.
- **Domain bridge with Thompson sampling** -- routes cross-domain
  queries to the most promising transfer path using Thompson sampling
  over historical success rates.
- **Transfer orchestrator** -- coordinates end-to-end knowledge
  transfers across domains.
- **5-phase cross-domain transfer pipeline** -- executes transfers
  through assess, align, project, adapt, and validate phases for
  reliable domain migration.

## Quick Start

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
exo-backend-classical = "0.1"
```

Basic usage:

```rust
use exo_backend_classical::ClassicalBackend;
use exo_core::SubstrateBackend;

let backend = ClassicalBackend::new()
    .with_simd(true)
    .with_dither_quantization(8); // 8-bit dithered

// Run a forward pass
let output = backend.forward(&input_tensor)?;

// Check thermodynamic cost
println!("Energy: {:.4} kT", backend.energy_cost());

// Cross-domain transfer (5-phase pipeline)
let result = backend.transfer("vision", "language", &payload)?;
println!("Transfer score: {:.4}", result.quality);
```

## Crate Layout

| Module      | Purpose                                      |
|-------------|----------------------------------------------|
| `simd`      | Platform-specific SIMD kernels                |
| `quantize`  | Dither quantization and de-quantization       |
| `thermo`    | Landauer energy tracking (thermorust)         |
| `bridge`    | Domain bridge with Thompson sampling          |
| `transfer`  | 5-phase cross-domain transfer orchestrator    |

## Requirements

- Rust 1.78+
- Depends on `exo-core`
- Optional: AVX2-capable CPU for best SIMD performance

## Links

- [GitHub](https://github.com/ruvnet/ruvector)
- [EXO-AI Documentation](https://github.com/ruvnet/ruvector/tree/main/examples/exo-ai-2025)

## License

MIT OR Apache-2.0
