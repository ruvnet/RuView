# ruvector-learning-wasm

Ultra-fast MicroLoRA adaptation for WASM - rank-2 LoRA with <100us latency for per-operator learning.

## Installation

```bash
npm install ruvector-learning-wasm
```

## Overview

This package provides Low-Rank Adaptation (LoRA) matrices optimized for WebAssembly execution. It enables real-time per-operator-type learning in query optimization systems with minimal latency overhead.

### Key Features

- **Rank-2 LoRA**: Minimal parameter count (2d parameters per adapter)
- **Per-Operator Scoping**: Separate adapters for 17 different operator types
- **<100us Adaptation**: Instant weight updates for real-time learning
- **WASM-Optimized**: Compiled to WebAssembly for near-native performance
- **Zero-Allocation Hot Paths**: Pre-allocated buffers for performance-critical operations

## JavaScript API

### WasmMicroLoRA

The main LoRA engine for single-adapter use cases.

```typescript
import init, { WasmMicroLoRA } from 'ruvector-learning-wasm';

// Initialize WASM module
await init();

// Create a new MicroLoRA engine
const lora = new WasmMicroLoRA(
  256,    // dim: Embedding dimension (max 256)
  0.1,    // alpha: Scaling factor
  0.01    // learning_rate: Learning rate for adaptation
);

// Forward pass with typed array
const input = new Float32Array(256).fill(1.0);
const output = lora.forward_array(input);

// Adapt with gradient
const gradient = new Float32Array(256).fill(0.1);
lora.adapt_array(gradient);

// Get statistics
console.log('Forward count:', lora.forward_count());
console.log('Adapt count:', lora.adapt_count());
console.log('Delta norm:', lora.delta_norm());
console.log('Parameter count:', lora.param_count());

// Reset engine
lora.reset();
```

#### Zero-Allocation API

For performance-critical loops, use the buffer-based API:

```typescript
const lora = new WasmMicroLoRA(256, 0.1, 0.01);

// Get buffer pointers
const inputPtr = lora.get_input_ptr();
const outputPtr = lora.get_output_ptr();
const dim = lora.dim();

// Create views into WASM memory
const memory = new Float32Array(lora.memory.buffer);
const inputView = new Float32Array(memory.buffer, inputPtr, dim);
const outputView = new Float32Array(memory.buffer, outputPtr, dim);

// Write input directly
inputView.set(myInputData);

// Forward pass (zero allocation)
lora.forward();

// Read output directly
const result = outputView.slice();

// Adapt using input buffer as gradient
lora.adapt();

// Adapt with reward (for RL)
lora.adapt_with_reward(0.5); // improvement ratio
```

### WasmScopedLoRA

Per-operator-type LoRA manager with 17 specialized adapters plus category fallback.

```typescript
import init, { WasmScopedLoRA } from 'ruvector-learning-wasm';

await init();

const scopedLora = new WasmScopedLoRA(
  256,    // dim
  0.1,    // alpha
  0.01    // learning_rate
);

// Operator types (0-16)
const HNSW_SCAN = 2;
const HASH_JOIN = 5;
const FILTER = 9;

// Forward for specific operator
const input = new Float32Array(256).fill(1.0);
const output = scopedLora.forward_array(HNSW_SCAN, input);

// Adapt for specific operator
const gradient = new Float32Array(256).fill(0.1);
scopedLora.adapt_array(FILTER, gradient);

// Per-operator statistics
console.log('HNSW forward count:', scopedLora.forward_count(HNSW_SCAN));
console.log('Filter adapt count:', scopedLora.adapt_count(FILTER));
console.log('Filter delta norm:', scopedLora.delta_norm(FILTER));

// Total statistics
console.log('Total forwards:', scopedLora.total_forward_count());
console.log('Total adapts:', scopedLora.total_adapt_count());

// Get operator name
console.log(WasmScopedLoRA.scope_name(HNSW_SCAN)); // "HnswScan"

// Enable/disable category fallback (default: enabled)
scopedLora.set_category_fallback(true);

// Reset specific operator or all
scopedLora.reset_scope(FILTER);
scopedLora.reset_all();
```

#### Operator Types

| Value | Name | Category |
|-------|------|----------|
| 0 | SeqScan | Scan |
| 1 | IndexScan | Scan |
| 2 | HnswScan | Scan |
| 3 | IvfFlatScan | Scan |
| 4 | NestedLoopJoin | Join |
| 5 | HashJoin | Join |
| 6 | MergeJoin | Join |
| 7 | Aggregate | Aggregation |
| 8 | GroupBy | Aggregation |
| 9 | Filter | Transform |
| 10 | Project | Transform |
| 11 | Sort | Order |
| 12 | Limit | Order |
| 13 | VectorDistance | Vector |
| 14 | Rerank | Vector |
| 15 | Materialize | Utility |
| 16 | Result | Utility |

### WasmTrajectoryBuffer

Trajectory recording for reinforcement learning and pattern analysis.

```typescript
import init, { WasmTrajectoryBuffer } from 'ruvector-learning-wasm';

await init();

const buffer = new WasmTrajectoryBuffer(
  1000,   // capacity: max trajectories to store
  256     // embedding_dim
);

// Record a trajectory
const embedding = new Float32Array(256).fill(1.0);
buffer.record(
  embedding,
  2,        // op_type: HnswScan
  0,        // attention_type
  100.0,    // execution_ms
  150.0     // baseline_ms (improvement = 150/100 - 1 = 0.5)
);

// Get statistics
console.log('Total count:', buffer.total_count());
console.log('Buffer length:', buffer.len());
console.log('Mean improvement:', buffer.mean_improvement());
console.log('Best improvement:', buffer.best_improvement());
console.log('Success rate:', buffer.success_rate());
console.log('Best attention type:', buffer.best_attention());
console.log('Variance:', buffer.variance());

// Filter by quality
console.log('High quality count:', buffer.high_quality_count(0.5));

// Filter by operator
console.log('HnswScan trajectories:', buffer.count_by_operator(2));

// Reset buffer
buffer.reset();
```

## Architecture

```
Input Embedding (d-dim)
       |
       v
  +---------+
  | A: d x 2 |  Down projection
  +---------+
       |
       v
  +---------+
  | B: 2 x d |  Up projection (initialized to zero)
  +---------+
       |
       v
Delta W = alpha * (A @ B)
       |
       v
Output = Input + Delta W
```

### Category Fallback

When an operator has fewer than 10 adaptations, the output is blended with the category adapter based on relative experience:

```
weight = min(operator_adapt_count / 10, 1.0)
output = operator_output * weight + category_output * (1 - weight)
```

This enables transfer learning between similar operators (e.g., all scan types share Scan category knowledge).

## Performance

- **Forward pass**: ~50us for 256-dim embeddings
- **Adaptation**: ~30us for gradient update
- **Memory**: ~2KB per LoRA pair (A + B matrices)
- **WASM size**: ~39KB (release build)

## TypeScript Types

Full TypeScript definitions are included in the package:

```typescript
export class WasmMicroLoRA {
  constructor(dim?: number, alpha?: number, learning_rate?: number);
  get_input_ptr(): number;
  get_output_ptr(): number;
  dim(): number;
  forward(): void;
  forward_array(input: Float32Array): Float32Array;
  adapt(): void;
  adapt_array(gradient: Float32Array): void;
  adapt_with_reward(improvement: number): void;
  reset(): void;
  forward_count(): bigint;
  adapt_count(): bigint;
  delta_norm(): number;
  param_count(): number;
}

export class WasmScopedLoRA {
  constructor(dim?: number, alpha?: number, learning_rate?: number);
  get_input_ptr(): number;
  get_output_ptr(): number;
  forward(op_type: number): void;
  forward_array(op_type: number, input: Float32Array): Float32Array;
  adapt(op_type: number): void;
  adapt_array(op_type: number, gradient: Float32Array): void;
  adapt_with_reward(op_type: number, improvement: number): void;
  reset_scope(op_type: number): void;
  reset_all(): void;
  forward_count(op_type: number): bigint;
  adapt_count(op_type: number): bigint;
  delta_norm(op_type: number): number;
  total_forward_count(): bigint;
  total_adapt_count(): bigint;
  set_category_fallback(enabled: boolean): void;
  static scope_name(op_type: number): string;
}

export class WasmTrajectoryBuffer {
  constructor(capacity?: number, embedding_dim?: number);
  record(
    embedding: Float32Array,
    op_type: number,
    attention_type: number,
    execution_ms: number,
    baseline_ms: number
  ): void;
  total_count(): bigint;
  len(): number;
  is_empty(): boolean;
  mean_improvement(): number;
  best_improvement(): number;
  success_rate(): number;
  best_attention(): number;
  variance(): number;
  reset(): void;
  high_quality_count(threshold: number): number;
  count_by_operator(op_type: number): number;
}
```

## License

MIT OR Apache-2.0
