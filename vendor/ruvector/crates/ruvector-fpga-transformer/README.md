# FPGA Transformer

**Run AI models on specialized hardware with predictable, ultra-low latency.**

FPGA Transformer is a Rust library that lets you run transformer neural networks (like those used in ChatGPT, code completion, and other AI applications) on FPGA hardware instead of GPUs. This gives you consistent, predictable response times - essential for real-time applications.

## Why Use This?

| Problem | Solution |
|---------|----------|
| GPU inference has unpredictable latency spikes | FPGAs provide deterministic, bounded timing |
| Cloud AI is too slow for edge devices | Run models locally on low-power FPGAs |
| Need to verify AI didn't hallucinate | Witness logging proves what computation ran |
| Want to skip unnecessary computation | Coherence gating exits early when confident |

## Quick Start

```bash
# Add to your Cargo.toml
cargo add ruvector-fpga-transformer
```

```rust
use ruvector_fpga_transformer::prelude::*;
use std::sync::Arc;

fn main() -> Result<()> {
    // Create an engine (uses CPU simulator by default)
    let mut engine = Engine::native_sim();

    // Load your model
    let model_bytes = std::fs::read("model.rvt")?;
    let model_id = engine.load_artifact(&model_bytes)?;

    // Prepare input tokens
    let tokens: Vec<u16> = vec![1, 2, 3, 4, 5];  // Your tokenized input
    let mask = vec![1u8; tokens.len()];          // Attention mask

    // Run inference
    let request = InferenceRequest::new(
        model_id,
        FixedShape::micro(),  // 32 seq, 64 dim, 4096 vocab
        &tokens,
        &mask,
        GateHint::allow_all(),
    );

    let result = engine.infer(request)?;

    // Get predictions
    println!("Top prediction: token {}", result.topk.unwrap()[0].0);
    println!("Latency: {}ns", result.witness.latency_ns);

    Ok(())
}
```

## Features

### Core Capabilities

| Feature | Description |
|---------|-------------|
| **Deterministic Latency** | Fixed execution time - no surprise slowdowns |
| **Quantization-First** | INT4/INT8 math for 4-8x memory savings |
| **Zero Allocation Hot Path** | No garbage collection pauses during inference |
| **Early Exit** | Stop computation when the model is already confident |
| **Witness Logging** | Cryptographic proof of what ran and when |

### Supported Backends

| Backend | Use Case | Feature Flag |
|---------|----------|--------------|
| **NativeSim** | Development & testing on any CPU | `native_sim` |
| **WasmSim** | Run in web browsers | `wasm` |
| **FpgaDaemon** | Connect to FPGA via network | `daemon` |
| **FpgaPcie** | Direct PCIe access (fastest) | `pcie` |

## Model Shapes

Pre-defined configurations for common use cases:

| Shape | Sequence | Dimensions | Vocab | Use Case |
|-------|----------|------------|-------|----------|
| `micro()` | 32 | 64 | 4,096 | Testing, tiny models |
| `small()` | 128 | 256 | 32,768 | Edge devices |
| `medium()` | 512 | 512 | 50,257 | Standard inference |
| `large()` | 2,048 | 1,024 | 50,257 | High-quality output |

```rust
// Use predefined shapes
let shape = FixedShape::small();

// Or create custom
let custom = FixedShape {
    seq_len: 256,
    d_model: 384,
    vocab: 16000,
};
```

## Coherence Gating

Skip unnecessary computation when the model is already confident:

```rust
use ruvector_fpga_transformer::gating::{GatingConfig, PolicyGate};

// Configure early exit behavior
let config = GatingConfig {
    min_coherence: 0.7,      // Require 70% confidence to exit early
    max_compute_class: 3,    // Allow up to 3 layers before forcing exit
    allow_writes: true,      // Allow writes if confidence is high
    ..Default::default()
};

let gate = PolicyGate::new(config);
```

**Gate Decisions:**
- `RanFull` - Model ran all layers
- `EarlyExit { layer }` - Exited early at specified layer
- `Skipped { reason }` - Computation was blocked

## Quantization

Convert floating-point models to efficient integer math:

| Format | Bits | Memory Savings | Use Case |
|--------|------|----------------|----------|
| INT8 | 8 | 4x | General purpose |
| INT4 | 4 | 8x | Memory-constrained |
| Binary | 1 | 32x | Ultra-compact |

```rust
// INT8 quantization (recommended)
let quant = QuantSpec::int8();

// INT4 for memory savings
let quant = QuantSpec::int4();

// Custom quantization
let quant = QuantSpec {
    bits: 8,
    scale: 127.0,
    zero_point: 0,
    symmetric: true,
};
```

## Witness Logging

Every inference produces a cryptographic witness proving:
- Which model ran (by hash)
- What quantization was used
- Which backend executed it
- Exact cycle count and latency
- Gate decision made

```rust
let result = engine.infer(request)?;
let witness = &result.witness;

println!("Model hash: {}", hex::encode(&witness.model_hash));
println!("Backend: {:?}", witness.backend);
println!("Cycles: {}", witness.cycles);
println!("Decision: {:?}", witness.gate_decision);

// Verify witness authenticity
assert!(witness.verify());
```

## Backend Selection

### Native Simulator (Default)
Best for development and testing:

```rust
let engine = Engine::native_sim();
```

### FPGA Daemon
Connect to a remote FPGA over network:

```rust
use ruvector_fpga_transformer::backend::fpga_daemon::{FpgaDaemonBackend, DaemonConnection};

let backend = FpgaDaemonBackend::with_connection(
    DaemonConnection::tcp("192.168.1.100:9000"),
    Default::default(),
);
```

### FPGA PCIe (Fastest)
Direct hardware access:

```rust
use ruvector_fpga_transformer::backend::fpga_pcie::{FpgaPcieBackend, PcieConfig};

let config = PcieConfig {
    device_path: "/dev/ruvector0".into(),
    ring_slots: 16,
    dma_timeout_ms: 100,
    ..Default::default()
};

let backend = FpgaPcieBackend::new(config)?;
```

## Feature Flags

Enable only what you need:

```toml
[dependencies]
ruvector-fpga-transformer = { version = "0.1", default-features = false, features = ["native_sim"] }
```

| Flag | Description |
|------|-------------|
| `native_sim` | CPU-based simulator |
| `daemon` | Network daemon client |
| `pcie` | Direct PCIe access |
| `wasm` | WebAssembly support |
| `witness` | Witness logging |
| `strict_verify` | Extra verification checks |
| `lut_softmax` | LUT-based softmax (faster) |
| `trace` | Debug tracing |

## Performance Tips

1. **Use appropriate shapes** - Don't use `large()` for simple tasks
2. **Enable early exit** - Set reasonable `min_coherence` threshold
3. **Batch requests** - Reuse loaded models across multiple inferences
4. **Use topk_only** - Return only top predictions, not full vocabulary

```rust
// Efficient configuration
let config = DaemonConfig {
    topk_only: true,    // Only return top-K predictions
    topk: 10,           // Return top 10
    retries: 3,         // Retry on transient failures
    ..Default::default()
};
```

## Architecture

```
                    ┌─────────────┐
                    │   Engine    │
                    │  (public)   │
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
        ┌─────▼─────┐ ┌────▼────┐ ┌─────▼─────┐
        │ Coherence │ │ Backend │ │  Witness  │
        │   Gate    │ │ Trait   │ │   Log     │
        └───────────┘ └────┬────┘ └───────────┘
                           │
         ┌────────┬────────┼────────┬────────┐
         │        │        │        │        │
     ┌───▼───┐┌───▼───┐┌───▼───┐┌───▼───┐
     │Native ││ WASM  ││Daemon ││ PCIe  │
     │  Sim  ││  Sim  ││       ││       │
     └───────┘└───────┘└───────┘└───────┘
```

## Examples

See the [examples](./examples/) directory:
- `basic_inference.rs` - Simple inference example
- `daemon_client.rs` - Connect to FPGA daemon

Run examples:
```bash
cargo run --example basic_inference
cargo run --example daemon_client --features daemon
```

## Testing

```bash
# Run all tests
cargo test --features native_sim

# Run with tracing
RUST_LOG=debug cargo test --features "native_sim trace"

# Run benchmarks
cargo bench --features native_sim
```

## License

MIT OR Apache-2.0

## Contributing

Contributions welcome! Please read the [contributing guidelines](../../CONTRIBUTING.md) first.
