# ruvector-attention-wasm

WebAssembly bindings for the ruvector-attention package, providing high-performance attention mechanisms for browser and Node.js environments.

## Features

- **Multiple Attention Mechanisms**:
  - Scaled Dot-Product Attention
  - Multi-Head Attention
  - Hyperbolic Attention (for hierarchical data)
  - Linear Attention (Performer-style)
  - Flash Attention (memory-efficient)
  - Local-Global Attention
  - Mixture of Experts (MoE) Attention
  - **CGT Sheaf Attention** (coherence-gated via Prime-Radiant)

- **Training Utilities**:
  - InfoNCE contrastive loss
  - Adam optimizer
  - AdamW optimizer (with decoupled weight decay)
  - Learning rate scheduler (warmup + cosine decay)

- **TypeScript Support**: Full type definitions and modern API

## Installation

```bash
npm install ruvector-attention-wasm
```

## Usage

### TypeScript/JavaScript

```typescript
import { initialize, MultiHeadAttention, utils } from 'ruvector-attention-wasm';

// Initialize WASM module
await initialize();

// Create multi-head attention
const attention = new MultiHeadAttention({ dim: 64, numHeads: 8 });

// Prepare inputs
const query = new Float32Array(64);
const keys = [new Float32Array(64), new Float32Array(64)];
const values = [new Float32Array(64), new Float32Array(64)];

// Compute attention
const output = attention.compute(query, keys, values);

// Use utilities
const similarity = utils.cosineSimilarity(query, keys[0]);
```

### Advanced Examples

#### Hyperbolic Attention

```typescript
import { HyperbolicAttention } from 'ruvector-attention-wasm';

const hyperbolic = new HyperbolicAttention({
  dim: 128,
  curvature: 1.0
});

const output = hyperbolic.compute(query, keys, values);
```

#### MoE Attention with Expert Stats

```typescript
import { MoEAttention } from 'ruvector-attention-wasm';

const moe = new MoEAttention({
  dim: 64,
  numExperts: 4,
  topK: 2
});

const output = moe.compute(query, keys, values);

// Get expert utilization
const stats = moe.getExpertStats();
console.log('Load balance:', stats.loadBalance);
```

#### Training with InfoNCE Loss

```typescript
import { InfoNCELoss, Adam } from 'ruvector-attention-wasm';

const loss = new InfoNCELoss(0.07);
const optimizer = new Adam(paramCount, {
  learningRate: 0.001,
  beta1: 0.9,
  beta2: 0.999,
});

// Training loop
const lossValue = loss.compute(anchor, positive, negatives);
optimizer.step(params, gradients);
```

#### Learning Rate Scheduling

```typescript
import { LRScheduler, AdamW } from 'ruvector-attention-wasm';

const scheduler = new LRScheduler({
  initialLR: 0.001,
  warmupSteps: 1000,
  totalSteps: 10000,
});

const optimizer = new AdamW(paramCount, {
  learningRate: scheduler.getLR(),
  weightDecay: 0.01,
});

// Training loop
for (let step = 0; step < 10000; step++) {
  optimizer.learningRate = scheduler.getLR();
  optimizer.step(params, gradients);
  scheduler.step();
}
```

## Building from Source

### Prerequisites

- Rust 1.70+
- wasm-pack

### Build Commands

```bash
# Build for web (ES modules)
wasm-pack build --target web --out-dir pkg

# Build for Node.js
wasm-pack build --target nodejs --out-dir pkg-node

# Build for bundlers (webpack, vite, etc.)
wasm-pack build --target bundler --out-dir pkg-bundler

# Run tests
wasm-pack test --headless --firefox
```

## API Reference

### Attention Mechanisms

- `MultiHeadAttention` - Standard multi-head attention
- `HyperbolicAttention` - Attention in hyperbolic space
- `LinearAttention` - Linear complexity attention (Performer)
- `FlashAttention` - Memory-efficient attention
- `LocalGlobalAttention` - Combined local and global attention
- `MoEAttention` - Mixture of Experts attention
- `CGTSheafAttention` - Coherence-gated via Prime-Radiant energy
- `scaledDotAttention()` - Functional API for basic attention

### CGT Sheaf Attention (Prime-Radiant Integration)

The CGT (Coherence-Gated Transformer) Sheaf Attention mechanism uses Prime-Radiant's sheaf Laplacian energy to gate attention based on mathematical consistency:

```typescript
import { CGTSheafAttention } from 'ruvector-attention-wasm';

const cgtAttention = new CGTSheafAttention({
  dim: 128,
  numHeads: 8,
  coherenceThreshold: 0.3,  // Block if energy > threshold
});

// Attention is gated by coherence energy
const result = cgtAttention.compute(query, keys, values);
console.log('Coherence energy:', result.energy);
console.log('Is coherent:', result.isCoherent);
```

**Key features:**
- Energy-weighted attention: Lower coherence energy → higher attention
- Automatic hallucination detection via residual analysis
- GPU-accelerated with wgpu WGSL shaders (vec4 optimized)
- SIMD fallback (AVX-512/AVX2/NEON)

### Training

- `InfoNCELoss` - Contrastive loss function
- `Adam` - Adam optimizer
- `AdamW` - AdamW optimizer with weight decay
- `LRScheduler` - Learning rate scheduler

### Utilities

- `utils.cosineSimilarity()` - Cosine similarity between vectors
- `utils.l2Norm()` - L2 norm of a vector
- `utils.normalize()` - Normalize vector to unit length
- `utils.softmax()` - Apply softmax transformation
- `utils.attentionWeights()` - Compute attention weights from scores
- `utils.batchNormalize()` - Batch normalization
- `utils.randomOrthogonalMatrix()` - Generate random orthogonal matrix
- `utils.pairwiseDistances()` - Compute pairwise distances

## Performance

The WASM bindings provide near-native performance for attention computations:

- Optimized with `opt-level = "s"` and LTO
- SIMD acceleration where available
- Efficient memory management
- Zero-copy data transfer where possible

## License

MIT OR Apache-2.0
