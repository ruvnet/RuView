# ruvector-sparse-inference

PowerInfer-style Activation Locality Inference Engine for RuVector.

A high-performance sparse inference engine that exploits neural network activation patterns to achieve 2×–10× speedups with <1% accuracy loss.

## Features

### Core Capabilities

- **Activation Locality**: Exploits power-law distribution where ~10% of neurons handle ~90% of activations
- **Low-Rank Prediction**: Fast P·Q matrix factorization predicts active neurons in O(r·d) time
- **Sparse FFN**: Computes only active neurons, skipping cold weights entirely
- **SIMD Optimization**: AVX2/FMA (GELU, SiLU, axpy), SSE4.1, NEON, and WASM SIMD backends
- **GGUF Support**: Full compatibility with quantized Llama models (Q4_0 through Q6_K)
- **Hot/Cold Caching**: LRU/LFU strategies for intelligent neuron weight management

### Precision Lanes (3/5/7-bit)

Layered quantization that turns activation selectivity into anatomical control:

| Lane | Bits | Range | Use Case |
|------|------|-------|----------|
| **Bit3** | 3 | -4..3 | Reflex signals, gating, anomaly triggers |
| **Bit5** | 5 | -16..15 | Streaming embeddings, drift detection |
| **Bit7** | 7 | -64..63 | Reasoning, synthesis, micro-LoRA |
| **Float** | 32 | Full | Training, offline calibration |

**Graduation Rules**: Signals move UP lanes on novelty/drift, DOWN on stability/stall.

### π Integration

π (pi) provides structural constants for low-precision systems:

```
π breaks symmetry.
```

| Module | Purpose |
|--------|---------|
| **Calibration** | π-derived constants avoid power-of-2 resonance |
| **Drift Detection** | Quantization honesty signals via π transforms |
| **Angular Embeddings** | Hyperspherical projections with π phase encoding |
| **Chaos Seeding** | Deterministic pseudo-randomness from π digits |

## Performance (v0.1.31)

**6× speedup** over previous version through W2 transpose optimization and SIMD-accelerated activations.

| Sparsity Level | Latency | vs Dense | Improvement |
|----------------|---------|----------|-------------|
| 10% active | 130µs | 52× faster | **83% reduction** |
| 30% active | 383µs | 18× faster | **83% reduction** |
| 50% active | 651µs | 10× faster | **83% reduction** |
| 70% active | 912µs | 7× faster | **83% reduction** |

### Key Optimizations (v0.1.31)

- **W2 Transpose Storage**: Column access becomes contiguous row access
- **SIMD GELU/SiLU**: AVX2 polynomial approximations for activations
- **Cached Feature Detection**: OnceLock eliminates runtime CPUID calls
- **SIMD axpy**: Vectorized accumulation in sparse second layer

### Target Performance

| Model | Target Latency | Speedup | Memory Reduction |
|-------|----------------|---------|------------------|
| LFM2 350M | ~5-10ms/sentence | 2.5× | 40% |
| Sentence-transformers | ~2-5ms/sentence | 2× | 30% |
| Llama 7B | 50-100ms/token | 5-10× | 50% |

## Quick Start

```rust
use ruvector_sparse_inference::{
    SparseInferenceEngine, SparsityConfig, PiContext, PrecisionLane
};

// Create sparse inference engine
let engine = SparseInferenceEngine::new_sparse(512, 2048, 0.1)?;

// Run inference
let input = vec![0.1f32; 512];
let output = engine.infer(&input)?;

// Use π context for calibration
let pi_ctx = PiContext::new(PrecisionLane::Bit5);
let calibrated = pi_ctx.calibrate(1.0);

// Check quantization honesty
let honesty = pi_ctx.check_honesty(&original, &quantized);
if !honesty.is_honest {
    // Escalate to higher precision lane
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Input Embedding                          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              Low-Rank Predictor (P·Q)                       │
│  ┌───────────┐    ┌───────────┐    ┌──────────────────┐    │
│  │ Input x   │───▶│  P matrix │───▶│  Q matrix        │    │
│  │ [d×1]     │    │  [d×r]    │    │  [r×hidden]      │    │
│  └───────────┘    └───────────┘    └──────────────────┘    │
│                                             │               │
│                                             ▼               │
│                              ┌──────────────────────────┐  │
│                              │ Threshold/Top-K Selection │  │
│                              │ Active Neuron Indices    │  │
│                              └──────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Sparse FFN Forward                       │
│  ┌─────────────────┐                                        │
│  │ Hot Weights     │◀── Always in memory                    │
│  │ (20% neurons)   │                                        │
│  └─────────────────┘                                        │
│           │                                                 │
│           ▼                                                 │
│  ┌─────────────────┐    ┌──────────────────────────────┐   │
│  │ W1[active] @ x  │───▶│ Activation (ReLU/GELU/SiLU)  │   │
│  └─────────────────┘    └──────────────────────────────┘   │
│           │                                                 │
│           ▼                                                 │
│  ┌─────────────────┐                                        │
│  │ W2 @ activated  │───▶ Output                             │
│  └─────────────────┘                                        │
└─────────────────────────────────────────────────────────────┘
```

## π-Based Systems

### Why π Matters

In 3/5/7-bit math, you deliberately throw away bits. π lets you check whether the system is still behaving honestly.

```rust
use ruvector_sparse_inference::pi::*;

// π as calibration constant
let calibration = PiCalibration::for_lane(PrecisionLane::Bit5);
let normalized = calibration.normalize(value);

// π as drift detector
let mut detector = DriftDetector::new(PrecisionLane::Bit5);
let honesty = detector.check(&original, &quantized);
if honesty.should_escalate {
    // Precision too low or hardware misbehaving
}

// π for angular embeddings
let angular = AngularEmbedding::new(PrecisionLane::Bit7);
let projected = angular.project(&vector);
let distance = angular.angular_distance(&a, &b);

// π for deterministic chaos
let chaos = PiChaos::new();
let jitter = chaos.jitter(index);  // Same input = same output, always
let schedule = chaos.schedule_order(n_agents, round);
```

### Key Constants

```rust
// π-based scale factors (avoid power-of-2 resonance)
pub const PI_SCALE_3BIT: f32 = π / 4.0;   // ~0.785
pub const PI_SCALE_5BIT: f32 = π / 16.0;  // ~0.196
pub const PI_SCALE_7BIT: f32 = π / 64.0;  // ~0.049
```

## Precision Lane Graduation

```rust
use ruvector_sparse_inference::precision::*;

// Configure graduation policy
let config = GraduationConfig {
    novelty_threshold: 0.3,
    drift_persistence_threshold: 5,
    confidence_threshold: 0.8,
    escalation_budget: 0.2,
};

let mut policy = GraduationPolicy::new(PrecisionLane::Bit5, config);

// Update metrics during inference
policy.update_metrics(GraduationMetrics {
    novelty: 0.4,      // High novelty detected
    drift_steps: 3,
    confidence: 0.9,
    cost_usage: 0.1,
    ..Default::default()
});

// Check graduation decision
match policy.decide() {
    GraduationDecision::Stay => { /* Continue at Bit5 */ }
    GraduationDecision::Escalate(PrecisionLane::Bit7) => { /* Upgrade */ }
    GraduationDecision::Demote(PrecisionLane::Bit3) => { /* Downgrade */ }
}
```

## Configuration Options

### Sparsity Selection

```rust
// Top-K selection
SparsityConfig::with_top_k(100);

// Threshold-based selection
SparsityConfig::with_threshold(0.01);

// Target sparsity ratio
SparsityConfig::with_target_sparsity(0.95); // 95% sparse
```

### Activation Functions

- `Relu`: max(0, x)
- `Gelu`: Gaussian Error Linear Unit
- `Silu`/`Swish`: x * sigmoid(x)
- `Identity`: No activation

### Quantization

```rust
use ruvector_sparse_inference::memory::QuantizedWeights;

// Int8 quantization
let weights = QuantizedWeights::quantize_int8(&original);
let dequantized = weights.dequantize_row(0);

// Int4 quantization (GGUF-style)
let weights = QuantizedWeights::quantize_int4(&original, 32);
```

## WASM Support

```rust
// In ruvector-sparse-inference-wasm
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn create_sparse_engine(
    input_dim: usize,
    hidden_dim: usize,
    sparsity: f32,
) -> Result<SparseEngineWasm, JsValue>;

#[wasm_bindgen]
pub fn infer(
    engine: &SparseEngineWasm,
    input: &[f32],
) -> Result<Vec<f32>, JsValue>;
```

## Integration

### With RuVector (EmbeddingProvider)

```rust
use ruvector_sparse_inference::integration::SparseEmbeddingProvider;

let provider = SparseEmbeddingProvider::new(config)?;
let embedding = provider.embed("Hello world")?;
```

### With RuvLLM (InferenceBackend)

```rust
use ruvector_sparse_inference::integration::SparseInferenceBackend;

let backend = SparseInferenceBackend::new(model_path)?;
let output = backend.generate(tokens, &config)?;
```

## Benchmarks

Run benchmarks:

```bash
cargo bench -p ruvector-sparse-inference
```

SIMD kernel benchmarks:
```bash
cargo bench -p ruvector-sparse-inference --bench simd_kernels
```

## Testing

```bash
# Unit tests
cargo test -p ruvector-sparse-inference

# Integration tests
cargo test -p ruvector-sparse-inference --test '*'
```

## Hardware Targets

| Platform | SIMD Backend | Precision Lanes |
|----------|--------------|-----------------|
| x86_64 (AVX2) | 256-bit vectors | All |
| x86_64 (SSE4.1) | 128-bit vectors | All |
| ARM (NEON) | 128-bit vectors | All |
| WASM | 128-bit SIMD | Bit5, Bit7 |
| ESP32 | Scalar | Bit3 only |

## The Deeper Insight

> π is not about geometry here. It is about injecting infinite structure into finite machines without breaking determinism.

Low-bit quantization simplifies the math. π reintroduces richness without cost.

- Quantization makes systems stable
- π makes them expressive
- Together: the math stays boring, the behavior stays interesting, the proofs stay simple

## Features

- `default = ["simd"]`
- `simd`: Enable SIMD optimizations
- `parallel`: Enable parallel computation with rayon
- `quantization`: Enable quantization support
- `npu`: Enable ARM NPU support (experimental)

## License

MIT OR Apache-2.0
