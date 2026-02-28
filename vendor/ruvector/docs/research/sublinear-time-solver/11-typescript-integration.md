# TypeScript & Type System Integration Analysis

**Agent**: 11 -- TypeScript & Type System Integration
**Scope**: Type mapping between ruvector and sublinear-time-solver
**Date**: 2026-02-20

---

## 1. Existing TypeScript Types in ruvector

### 1.1 Core Vector Database Types (`npm/core/src/index.ts`)

The ruvector core package defines the foundational TypeScript types for the vector database:

```typescript
// Distance metric enum
enum DistanceMetric { Euclidean, Cosine, DotProduct, Manhattan }

// HNSW index configuration
interface HnswConfig {
  m?: number;
  efConstruction?: number;
  efSearch?: number;
  maxElements?: number;
}

// Quantization configuration
interface QuantizationConfig {
  type: 'none' | 'scalar' | 'product' | 'binary';
  subspaces?: number;
  k?: number;
}

// Database options
interface DbOptions {
  dimensions: number;
  distanceMetric?: DistanceMetric;
  storagePath?: string;
  hnswConfig?: HnswConfig;
  quantization?: QuantizationConfig;
}

// Vector entry (uses Float32Array)
interface VectorEntry {
  id?: string;
  vector: Float32Array | number[];
}

// Search types
interface SearchQuery {
  vector: Float32Array | number[];
  k: number;
  efSearch?: number;
}

interface SearchResult {
  id: string;
  score: number;
}

// VectorDB interface
interface VectorDB {
  insert(entry: VectorEntry): Promise<string>;
  insertBatch(entries: VectorEntry[]): Promise<string[]>;
  search(query: SearchQuery): Promise<SearchResult[]>;
  delete(id: string): Promise<boolean>;
  get(id: string): Promise<VectorEntry | null>;
  len(): Promise<number>;
  isEmpty(): Promise<boolean>;
}

// Collection management
interface CollectionConfig { dimensions, distanceMetric?, hnswConfig?, quantization? }
interface CollectionStats { vectorsCount, diskSizeBytes, ramSizeBytes }
interface CollectionManager { createCollection, listCollections, deleteCollection, ... }
interface HealthResponse { status: 'healthy' | 'degraded' | 'unhealthy', version, uptimeSeconds }
```

Key observation: ruvector core uses **Float32Array** for all vector operations, while sublinear-time-solver uses **Float64Array** for matrix data. This is a critical type divergence.

### 1.2 Attention WASM Types (`crates/ruvector-attention-wasm/js/types.ts`)

```typescript
interface AttentionConfig { dim, numHeads?, dropout?, scale?, causal? }
interface MultiHeadConfig extends AttentionConfig { numHeads: number }
interface HyperbolicConfig extends AttentionConfig { curvature: number }
interface LinearAttentionConfig extends AttentionConfig { numFeatures: number }
interface FlashAttentionConfig extends AttentionConfig { blockSize: number }
interface LocalGlobalConfig extends AttentionConfig { localWindow, globalTokens }
interface MoEConfig extends AttentionConfig { numExperts, topK, expertCapacity?, balanceCoeff? }
interface TrainingConfig { learningRate, temperature?, beta1?, beta2?, weightDecay?, epsilon? }
type AttentionType = 'scaled_dot_product' | 'multi_head' | 'hyperbolic' | 'linear' | 'flash' | 'local_global' | 'moe'
```

### 1.3 Unified WASM Types (`npm/packages/ruvector-wasm-unified/src/types.ts`)

This file defines the shared types for the unified WASM module system:

```typescript
interface Tensor { data: Float32Array; shape: number[]; dtype: 'float32' | 'float16' | 'int32' | 'uint8' }
interface Result<T, E = Error> { ok: boolean; value?: T; error?: E }
interface AsyncResult<T> extends Result<T> { pending: boolean; progress?: number }
interface AttentionScores { scores: Float32Array; weights: Float32Array; metadata: AttentionMetadata }
interface AttentionMetadata { mechanism, computeTimeMs, memoryUsageBytes, sparsityRatio? }
interface QueryNode { id, embedding: Float32Array, nodeType, metadata? }
interface QueryEdge { source, target, weight, edgeType }
interface QueryDag { nodes: QueryNode[]; edges: QueryEdge[]; rootIds, leafIds }
interface UnifiedConfig { wasmPath?, enableSimd?, enableThreads?, memoryLimit?, logLevel? }
```

### 1.4 Delta-Behavior Types (`examples/delta-behavior/wasm/src/types.ts`)

Extensive discriminated union types for coherence-preserving state transitions:

```typescript
type Coherence = number; // 0.0 to 1.0
interface DeltaConfig { bounds, energy, scheduling, gating, guidanceStrength }
type TransitionResult = { type: 'allowed' } | { type: 'throttled'; duration } | { type: 'blocked'; reason } | { type: 'energyExhausted' }
interface WasmInitOptions { wasmPath?, wasmBytes?: Uint8Array, memory?: WasmMemoryConfig, enableSimd?, enableThreads? }
interface WasmMemoryConfig { initial: number; maximum?: number; shared?: boolean }
interface DeltaHeader { sequence: bigint; operation; vectorId?; timestamp: bigint; payloadSize; checksum: bigint }
```

### 1.5 Edge-Net Dashboard Types (`examples/edge-net/dashboard/src/types/index.ts`)

```typescript
interface WASMModule { id, name, version, loaded, size, features: string[], status, error?, loadTime? }
interface WASMBenchmark { moduleId, operation, iterations, avgTime, minTime, maxTime, throughput }
interface NetworkStats { totalNodes, activeNodes, totalCompute, creditsEarned, ... }
```

### 1.6 Error Type Hierarchy

ruvector has three distinct error systems:

**RvfError** (`npm/packages/rvf/src/errors.ts`): Numeric error codes modeled after Rust enums with category bytes:
```typescript
enum RvfErrorCode { Ok=0x0000, DimensionMismatch=0x0200, TileTrap=0x0400, TileOom=0x0401, ... }
class RvfError extends Error { readonly code: RvfErrorCode; get category(); get isFormatError() }
```

**RuvBotError** (`npm/packages/ruvbot/src/core/errors.ts`): String-code error hierarchy:
```typescript
class RuvBotError extends Error { readonly code: string; context?: Record<string, unknown> }
class MemoryError extends RuvBotError { /* code: 'MEMORY_ERROR' */ }
class ValidationError extends RuvBotError { validationErrors: ValidationErrorDetail[] }
class WasmError extends RuvBotError { /* code: 'WASM_ERROR' */ }
```

**Result wrapper** (`npm/packages/ruvector-wasm-unified/src/types.ts`):
```typescript
interface Result<T, E = Error> { ok: boolean; value?: T; error?: E }
interface AsyncResult<T> extends Result<T> { pending: boolean; progress?: number }
```

### 1.7 Streaming Types

ruvector uses iterator-based streaming for query results:

```typescript
// npm/packages/graph-node/index.d.ts
class QueryResultStream { next(): JsQueryResult | null }
class HyperedgeStream { next(): JsHyperedgeResult | null; collect(): Array<JsHyperedgeResult> }
class NodeStream { next(): JsNode | null; collect(): Array<JsNode> }

// examples/edge-full/pkg/graph/ruvector_graph_wasm.d.ts
class ResultStream { /* WASM-backed stream */ }
```

### 1.8 WASM Module Initialization Pattern

All ruvector WASM modules follow a consistent init pattern:

```typescript
type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;
interface InitOutput { readonly memory: WebAssembly.Memory; /* exported functions */ }
function __wbg_init(module_or_path?: InitInput): Promise<InitOutput>;
function initSync(module: SyncInitInput): InitOutput;
```

---

## 2. Type Mapping Between ruvector and sublinear-time-solver

### 2.1 Configuration Type Mapping

| sublinear-time-solver | ruvector Equivalent | Notes |
|---|---|---|
| `SolverConfig.maxIterations` | No direct equivalent | New concept; map to `HnswConfig.efSearch` semantically |
| `SolverConfig.tolerance` | No direct equivalent | New concept for convergence checking |
| `SolverConfig.simdEnabled` | `WasmInitOptions.enableSimd` | Direct boolean mapping |
| `SolverConfig.streamChunkSize` | No direct equivalent | Streaming chunk control; new concept |
| `Features.simd_enabled` | `UnifiedConfig.enableSimd` | Equivalent intent |
| `Features.parallel_enabled` | `WasmInitOptions.enableThreads` | Equivalent intent |
| `Features.memory_64` | `WasmMemoryConfig.maximum` | Related but different semantics |
| `Features.wee_alloc` | No direct equivalent | Allocator detail not exposed in ruvector TS |

### 2.2 Data Type Mapping

| sublinear-time-solver | ruvector Equivalent | Compatibility |
|---|---|---|
| `MatrixData.data: Float64Array` | `Tensor.data: Float32Array` | **INCOMPATIBLE** -- precision mismatch |
| `MatrixData.rows, cols` | `Tensor.shape: number[]` | Bridgeable: `[rows, cols]` |
| `SolutionStep.iteration` | `Improvement.iteration` | Direct mapping possible |
| `SolutionStep.residual` | `Coherence` (type alias) | Semantically related (0-1 range) |
| `SolutionStep.timestamp` | `SystemState.timestamp` | Direct mapping |
| `SolutionStep.convergence` | No direct equivalent | New concept |

### 2.3 Batch Processing Mapping

| sublinear-time-solver | ruvector Equivalent | Notes |
|---|---|---|
| `BatchSolveRequest.id` | `VectorEntry.id` | Both optional string IDs |
| `BatchSolveRequest.matrix` | No direct equivalent | ruvector batches are `VectorEntry[]` |
| `BatchSolveRequest.vector` | `SearchQuery.vector` | Different precision (Float64 vs Float32) |
| `BatchSolveRequest.priority` | `SchedulingConfig.priorityThresholds` | Related priority concept |
| `BatchSolveResult.id` | `SearchResult.id` | Direct mapping |
| `BatchSolveResult.solution` | `SearchResult.score` + vector | Solution is richer |
| `BatchSolveResult.iterations` | No direct equivalent | Iteration count not tracked in search |
| `BatchSolveResult.error` | `Result.error` | Error wrapping pattern |

### 2.4 WASM Module Mapping

| sublinear-time-solver | ruvector Equivalent | Notes |
|---|---|---|
| `WasmModule.memory` | `InitOutput.memory` | Both `WebAssembly.Memory` |
| `WasmModule.get_features()` | `availableMechanisms()` | Different return shape |
| `WasmModule.enable_simd()` | `WasmInitOptions.enableSimd` | Config-time vs runtime toggle |
| `WasmModule.benchmark_matrix_multiply()` | `WASMBenchmark` interface | Both measure performance |

### 2.5 Class Mapping

| sublinear-time-solver | ruvector Equivalent | Compatibility |
|---|---|---|
| `Matrix` | `Tensor` (interface) | Bridgeable with shape constraint |
| `MatrixView` | No direct equivalent | New concept (zero-copy view) |
| `SolutionStream` | `QueryResultStream` | Both iterator-based streaming |
| `MemoryManager` | `WasmMemoryConfig` + wasm memory | Related but different abstraction levels |
| `SublinearSolver` | `VectorDB` / `FlashAttention` | Different domains but similar async patterns |

---

## 3. Generic Type Bridges

### 3.1 Numeric Array Bridge

The most critical bridge needed is between Float32Array (ruvector) and Float64Array (sublinear-time-solver):

```typescript
/**
 * Bridge between ruvector's Float32Array vectors and sublinear-time-solver's Float64Array matrices.
 * Precision loss occurs on the Float64 -> Float32 path.
 */
interface NumericArrayBridge {
  /** Convert Float32Array to Float64Array (lossless upcast) */
  toFloat64(data: Float32Array): Float64Array;

  /** Convert Float64Array to Float32Array (lossy downcast) */
  toFloat32(data: Float64Array): Float32Array;

  /** Check if precision loss exceeds acceptable tolerance */
  checkPrecisionLoss(original: Float64Array, converted: Float32Array, tolerance: number): boolean;
}

// Recommended implementation
const numericBridge: NumericArrayBridge = {
  toFloat64(data: Float32Array): Float64Array {
    const result = new Float64Array(data.length);
    for (let i = 0; i < data.length; i++) result[i] = data[i];
    return result;
  },
  toFloat32(data: Float64Array): Float32Array {
    return new Float32Array(data); // Native conversion
  },
  checkPrecisionLoss(original: Float64Array, converted: Float32Array, tolerance: number): boolean {
    for (let i = 0; i < original.length; i++) {
      if (Math.abs(original[i] - converted[i]) > tolerance) return false;
    }
    return true;
  }
};
```

### 3.2 Matrix-Tensor Bridge

```typescript
/**
 * Bridge between sublinear-time-solver's MatrixData and ruvector's Tensor.
 */
interface MatrixTensorBridge<P extends Float32Array | Float64Array> {
  /** Convert MatrixData (Float64Array) to ruvector Tensor (Float32Array) */
  matrixToTensor(matrix: MatrixData): Tensor;

  /** Convert ruvector Tensor back to MatrixData */
  tensorToMatrix(tensor: Tensor): MatrixData;

  /** Create a MatrixData view from Tensor without copying */
  createView(tensor: Tensor, rowStart: number, rowEnd: number): MatrixData;
}

type MatrixData = { data: Float64Array; rows: number; cols: number };
type Tensor = { data: Float32Array; shape: number[]; dtype: string };
```

### 3.3 Configuration Bridge

```typescript
/**
 * Unified configuration that can initialize both systems.
 */
interface UnifiedSolverConfig {
  // sublinear-time-solver fields
  maxIterations: number;
  tolerance: number;
  streamChunkSize: number;

  // ruvector fields
  dimensions: number;
  distanceMetric?: DistanceMetric;
  hnswConfig?: HnswConfig;

  // Shared fields
  simdEnabled: boolean;
  parallelEnabled: boolean;
  memoryConfig?: WasmMemoryConfig;
}

function toSolverConfig(unified: UnifiedSolverConfig): SolverConfig {
  return {
    maxIterations: unified.maxIterations,
    tolerance: unified.tolerance,
    simdEnabled: unified.simdEnabled,
    streamChunkSize: unified.streamChunkSize,
  };
}

function toDbOptions(unified: UnifiedSolverConfig): DbOptions {
  return {
    dimensions: unified.dimensions,
    distanceMetric: unified.distanceMetric,
    hnswConfig: unified.hnswConfig,
  };
}
```

### 3.4 Result Bridge

```typescript
/**
 * Generic Result bridge between ruvector's Result<T,E> and sublinear-time-solver's error-or-value patterns.
 */
type SolverResult<T> = { success: true; value: T } | { success: false; error: SolverError };
type RuvectorResult<T, E = Error> = { ok: boolean; value?: T; error?: E };

function solverResultToRuvector<T>(result: SolverResult<T>): RuvectorResult<T> {
  if (result.success) return { ok: true, value: result.value };
  return { ok: false, error: result.error };
}

function ruvectorResultToSolver<T>(result: RuvectorResult<T>): SolverResult<T> {
  if (result.ok && result.value !== undefined) return { success: true, value: result.value };
  return { success: false, error: new SolverError(result.error?.message ?? 'Unknown error') };
}
```

---

## 4. Error Type Unification

### 4.1 Current Error Landscape

ruvector has three separate error hierarchies that do not share a common base:

1. **RvfError** -- Numeric error codes (0x0000-0xFFFF), categorized by high byte, modeled after Rust.
2. **RuvBotError** -- String error codes with class hierarchy (MemoryError, ValidationError, WasmError, etc.).
3. **Result<T, E>** -- Simple ok/error wrapper used in WASM unified types.

sublinear-time-solver defines three error types:

1. **SolverError** -- Numerical solving failures (divergence, max iterations, invalid input).
2. **MemoryError** -- WASM memory allocation failures.
3. **ValidationError** -- Input validation failures.

### 4.2 Proposed Unified Error Hierarchy

```typescript
/**
 * Base error for all ruvector + solver operations.
 * Carries both a numeric code (for Rust interop) and a string code (for JS ergonomics).
 */
abstract class RuvectorBaseError extends Error {
  abstract readonly numericCode: number;
  abstract readonly stringCode: string;
  abstract readonly category: ErrorCategory;
  readonly context?: Record<string, unknown>;
  readonly timestamp: number = Date.now();
}

type ErrorCategory =
  | 'solver'      // Numerical solving errors
  | 'memory'      // WASM memory errors
  | 'validation'  // Input validation errors
  | 'format'      // File/data format errors
  | 'wasm'        // WASM runtime errors
  | 'query'       // Query/search errors
  | 'write'       // Storage write errors
  | 'crypto'      // Cryptographic errors
  | 'network';    // Network/connection errors

/**
 * Solver-specific errors (sublinear-time-solver integration).
 */
class SolverError extends RuvectorBaseError {
  readonly category = 'solver' as const;
  readonly stringCode: string;
  readonly numericCode: number;

  static readonly CODES = {
    DIVERGENCE:       { numeric: 0x0600, string: 'SOLVER_DIVERGENCE' },
    MAX_ITERATIONS:   { numeric: 0x0601, string: 'SOLVER_MAX_ITERATIONS' },
    SINGULAR_MATRIX:  { numeric: 0x0602, string: 'SOLVER_SINGULAR_MATRIX' },
    DIMENSION_MISMATCH: { numeric: 0x0603, string: 'SOLVER_DIMENSION_MISMATCH' },
    INVALID_TOLERANCE: { numeric: 0x0604, string: 'SOLVER_INVALID_TOLERANCE' },
  } as const;
}

/**
 * Unified MemoryError covering both ruvector WASM memory and solver memory failures.
 */
class UnifiedMemoryError extends RuvectorBaseError {
  readonly category = 'memory' as const;
  readonly stringCode: string;
  readonly numericCode: number;
  readonly requestedBytes?: number;
  readonly availableBytes?: number;

  static readonly CODES = {
    ALLOCATION_FAILED: { numeric: 0x0401, string: 'MEMORY_ALLOCATION_FAILED' },
    OUT_OF_BOUNDS:     { numeric: 0x0402, string: 'MEMORY_OUT_OF_BOUNDS' },
    WASM_OOM:          { numeric: 0x0401, string: 'WASM_OOM' },   // Maps to RvfErrorCode.TileOom
    GROW_FAILED:       { numeric: 0x0403, string: 'MEMORY_GROW_FAILED' },
  } as const;
}

/**
 * Unified ValidationError covering both systems.
 */
class UnifiedValidationError extends RuvectorBaseError {
  readonly category = 'validation' as const;
  readonly stringCode = 'VALIDATION_ERROR';
  readonly numericCode = 0x0700;
  readonly fieldErrors: Array<{ field: string; message: string; value?: unknown }>;
}
```

### 4.3 Error Code Registry Mapping

| Range | Category | ruvector Source | sublinear-time-solver |
|---|---|---|---|
| 0x0000-0x00FF | Success | RvfErrorCode.Ok | (implicit) |
| 0x0100-0x01FF | Format | RvfErrorCode.InvalidMagic, etc. | -- |
| 0x0200-0x02FF | Query | RvfErrorCode.DimensionMismatch, etc. | -- |
| 0x0300-0x03FF | Write | RvfErrorCode.LockHeld, etc. | -- |
| 0x0400-0x04FF | WASM/Tile | RvfErrorCode.TileTrap, TileOom, etc. | MemoryError |
| 0x0500-0x05FF | Crypto | RvfErrorCode.KeyNotFound, etc. | -- |
| **0x0600-0x06FF** | **Solver** | **(New)** | SolverError |
| **0x0700-0x07FF** | **Validation** | **(New)** | ValidationError |

### 4.4 Error Conversion Functions

```typescript
function fromSolverError(err: SolverError): RuvectorBaseError {
  if (err.type === 'divergence') return new SolverError(SolverError.CODES.DIVERGENCE, err.message);
  if (err.type === 'max_iterations') return new SolverError(SolverError.CODES.MAX_ITERATIONS, err.message);
  // ... etc
}

function fromRvfError(err: RvfError): RuvectorBaseError {
  switch (err.category) {
    case 0x04: return new UnifiedMemoryError(/* ... */);
    case 0x02: return new UnifiedValidationError(/* ... */);
    // ... etc
  }
}
```

---

## 5. Stream Type Compatibility

### 5.1 Current Streaming Patterns

**ruvector** uses synchronous pull-based iterators for WASM results:

```typescript
class QueryResultStream {
  next(): JsQueryResult | null;  // Returns null when exhausted
}

class HyperedgeStream {
  next(): JsHyperedgeResult | null;
  collect(): Array<JsHyperedgeResult>;  // Drain into array
}
```

**sublinear-time-solver** defines `SolutionStream` as an asynchronous, chunked stream of `SolutionStep` values:

```typescript
class SolutionStream {
  // Emits SolutionStep objects as the solver progresses
  // Supports backpressure via chunk size
  // Each step: { iteration, residual, timestamp, convergence }
}
```

### 5.2 Compatibility Issues

1. **Synchronous vs Asynchronous**: ruvector streams are synchronous `next()` calls; SolutionStream is inherently async (solver steps take variable time).
2. **Termination Semantics**: ruvector uses `null` return for exhaustion; SolutionStream terminates on convergence or error.
3. **Chunk Size**: ruvector streams are single-item; SolutionStream supports configurable chunk sizes.
4. **Progress Reporting**: SolutionStream naturally reports convergence progress; ruvector streams do not carry progress metadata.

### 5.3 Unified Stream Interface

```typescript
/**
 * Unified stream that works for both ruvector query results and solver solution steps.
 * Implements the async iterator protocol for for-await-of compatibility.
 */
interface UnifiedStream<T> {
  /** Pull the next value (async for solver, sync-wrapped for ruvector) */
  next(): Promise<StreamItem<T>>;

  /** Collect all remaining items into an array */
  collect(): Promise<T[]>;

  /** Cancel the stream early */
  cancel(): void;

  /** Whether the stream is still active */
  readonly active: boolean;

  /** Progress information (0-1 for solver, item count for queries) */
  readonly progress: StreamProgress;

  /** Async iterator protocol */
  [Symbol.asyncIterator](): AsyncIterableIterator<T>;
}

interface StreamItem<T> {
  done: boolean;
  value?: T;
  metadata?: StreamMetadata;
}

interface StreamProgress {
  current: number;
  total?: number;    // Known for batch operations, unknown for iterative solving
  percentage?: number;
}

interface StreamMetadata {
  timestamp: number;
  latencyMs: number;
  chunkIndex: number;
}
```

### 5.4 Stream Adapter Implementations

```typescript
/**
 * Wraps a ruvector synchronous stream as a UnifiedStream.
 */
class RuvectorStreamAdapter<T> implements UnifiedStream<T> {
  private inner: { next(): T | null; collect?(): T[] };
  private itemCount = 0;
  private _active = true;

  async next(): Promise<StreamItem<T>> {
    const value = this.inner.next();
    if (value === null) {
      this._active = false;
      return { done: true };
    }
    this.itemCount++;
    return { done: false, value, metadata: { timestamp: Date.now(), latencyMs: 0, chunkIndex: this.itemCount } };
  }

  async collect(): Promise<T[]> {
    if (this.inner.collect) return this.inner.collect();
    const results: T[] = [];
    while (true) {
      const item = await this.next();
      if (item.done) break;
      results.push(item.value!);
    }
    return results;
  }

  cancel(): void { this._active = false; }
  get active(): boolean { return this._active; }
  get progress(): StreamProgress { return { current: this.itemCount }; }

  async *[Symbol.asyncIterator](): AsyncIterableIterator<T> {
    while (this._active) {
      const item = await this.next();
      if (item.done) break;
      yield item.value!;
    }
  }
}

/**
 * Wraps a SolutionStream as a UnifiedStream<SolutionStep>.
 */
class SolverStreamAdapter implements UnifiedStream<SolutionStep> {
  private solver: SublinearSolver;
  private config: SolverConfig;
  private currentIteration = 0;
  private _active = true;

  get progress(): StreamProgress {
    return {
      current: this.currentIteration,
      total: this.config.maxIterations,
      percentage: this.currentIteration / this.config.maxIterations,
    };
  }

  // ... implementation follows same pattern
}
```

---

## 6. Recommended Shared Type Definitions

Based on the analysis above, the following shared types should be placed in a new file `src/types/shared-solver-types.ts` to serve as the integration contract between ruvector and sublinear-time-solver.

### 6.1 Shared Numeric Types

```typescript
/**
 * @file src/types/shared-solver-types.ts
 * Shared type definitions for ruvector <-> sublinear-time-solver integration.
 */

// --- Numeric Precision ---

/** Supported numeric array types */
type NumericArray = Float32Array | Float64Array;

/** Precision level for numeric operations */
type NumericPrecision = 'float32' | 'float64';

/** Matrix representation that supports both precisions */
interface Matrix<P extends NumericArray = Float64Array> {
  data: P;
  rows: number;
  cols: number;
}

/** Vector type that supports both precisions */
type Vector<P extends NumericArray = Float64Array> = P;

/** Convert between matrix representations */
interface PrecisionConverter {
  upcast(matrix: Matrix<Float32Array>): Matrix<Float64Array>;
  downcast(matrix: Matrix<Float64Array>): Matrix<Float32Array>;
  downcastChecked(matrix: Matrix<Float64Array>, tolerance: number): Matrix<Float32Array> | null;
}
```

### 6.2 Shared Configuration Types

```typescript
// --- Configuration ---

/** WASM feature flags shared between both systems */
interface WasmFeatures {
  simdEnabled: boolean;
  parallelEnabled: boolean;
  memory64: boolean;
  weeAlloc: boolean;
}

/** WASM memory configuration */
interface SharedMemoryConfig {
  initialPages: number;
  maximumPages?: number;
  shared: boolean;
}

/** Solver-specific configuration */
interface SolverOptions {
  maxIterations: number;
  tolerance: number;
  streamChunkSize: number;
}

/** Combined initialization config */
interface IntegrationConfig {
  solver: SolverOptions;
  wasm: WasmFeatures;
  memory: SharedMemoryConfig;
  precision: NumericPrecision;
}
```

### 6.3 Shared Result and Error Types

```typescript
// --- Results ---

/** Discriminated result type (preferred over boolean ok/error pattern) */
type Result<T, E = RuvectorBaseError> =
  | { readonly _tag: 'ok'; readonly value: T }
  | { readonly _tag: 'err'; readonly error: E };

/** Constructor helpers */
function ok<T>(value: T): Result<T, never> { return { _tag: 'ok', value }; }
function err<E>(error: E): Result<never, E> { return { _tag: 'err', error }; }

/** Type guards */
function isOk<T, E>(result: Result<T, E>): result is { _tag: 'ok'; value: T } { return result._tag === 'ok'; }
function isErr<T, E>(result: Result<T, E>): result is { _tag: 'err'; error: E } { return result._tag === 'err'; }

/** Async variant with progress tracking */
interface TrackedResult<T, E = RuvectorBaseError> {
  result: Promise<Result<T, E>>;
  progress: ReadonlyStream<number>;
  cancel: () => void;
}
```

### 6.4 Shared Benchmark Types

```typescript
// --- Benchmarking ---

/** Benchmark result (shared between WASMBenchmark and solver benchmarks) */
interface BenchmarkResult {
  operation: string;
  iterations: number;
  avgTimeMs: number;
  minTimeMs: number;
  maxTimeMs: number;
  throughput: number;
  memoryPeakBytes?: number;
  metadata?: Record<string, unknown>;
}
```

### 6.5 Shared Event/Stream Types

```typescript
// --- Streaming ---

/** Solution progress step (used by solver, adaptable for query streaming) */
interface ProgressStep {
  iteration: number;
  metric: number;        // residual for solver, score for search
  timestamp: number;
  convergence?: number;  // 0-1, solver-specific
}

/** Stream configuration */
interface StreamConfig {
  chunkSize: number;
  highWaterMark?: number;
  timeoutMs?: number;
}
```

---

## 7. Type-Safe Integration Patterns

### 7.1 Factory Pattern with Generic Precision

The solver factory should be generic over numeric precision to interoperate with ruvector's Float32Array world:

```typescript
/**
 * Create a solver with the appropriate precision for the context.
 * When integrating with ruvector VectorDB, use Float32; for standalone, use Float64.
 */
interface SolverFactory {
  /** Full-precision solver for standalone use */
  createSolver(config: SolverOptions): Promise<SublinearSolver<Float64Array>>;

  /** Reduced-precision solver for ruvector pipeline integration */
  createRuvectorSolver(config: SolverOptions & { dbOptions: DbOptions }): Promise<SublinearSolver<Float32Array>>;
}

interface SublinearSolver<P extends NumericArray = Float64Array> {
  solve(matrix: Matrix<P>, vector: Vector<P>): Promise<Result<Vector<P>, SolverError>>;
  solveStream(matrix: Matrix<P>, vector: Vector<P>): UnifiedStream<ProgressStep>;
  batchSolve(requests: BatchSolveRequest<P>[]): Promise<BatchSolveResult<P>[]>;
  getFeatures(): WasmFeatures;
  dispose(): void;
}

/** Parameterized batch types */
interface BatchSolveRequest<P extends NumericArray = Float64Array> {
  id: string;
  matrix: Matrix<P>;
  vector: Vector<P>;
  priority?: number;
}

interface BatchSolveResult<P extends NumericArray = Float64Array> {
  id: string;
  solution: Vector<P>;
  iterations: number;
  error?: SolverError;
}
```

### 7.2 Adapter Pattern for VectorDB Integration

```typescript
/**
 * Wraps VectorDB to use sublinear-time-solver for similarity computations.
 * The solver handles the heavy numerical work; VectorDB handles storage and indexing.
 */
class SolverBackedVectorDB implements VectorDB {
  private db: VectorDB;
  private solver: SublinearSolver<Float32Array>;

  constructor(db: VectorDB, solver: SublinearSolver<Float32Array>) {
    this.db = db;
    this.solver = solver;
  }

  async search(query: SearchQuery): Promise<SearchResult[]> {
    // Use solver for the distance computation, VectorDB for candidate retrieval
    const candidates = await this.db.search({ ...query, k: query.k * 4 }); // Over-retrieve
    // Re-rank using solver-based exact distance computation
    // ...
    return rerankedResults.slice(0, query.k);
  }

  // Delegate storage operations to underlying VectorDB
  insert(entry: VectorEntry): Promise<string> { return this.db.insert(entry); }
  insertBatch(entries: VectorEntry[]): Promise<string[]> { return this.db.insertBatch(entries); }
  delete(id: string): Promise<boolean> { return this.db.delete(id); }
  get(id: string): Promise<VectorEntry | null> { return this.db.get(id); }
  len(): Promise<number> { return this.db.len(); }
  isEmpty(): Promise<boolean> { return this.db.isEmpty(); }
}
```

### 7.3 Type Guard Pattern for WASM Interop

```typescript
/**
 * Type guards for safely handling WASM boundary values.
 * WASM functions may return unexpected types at the JS boundary.
 */
function isFloat64Array(value: unknown): value is Float64Array {
  return value instanceof Float64Array;
}

function isFloat32Array(value: unknown): value is Float32Array {
  return value instanceof Float32Array;
}

function assertMatrix<P extends NumericArray>(value: unknown, precision: 'float32' | 'float64'): Matrix<P> {
  if (typeof value !== 'object' || value === null) throw new UnifiedValidationError('Expected matrix object');
  const obj = value as Record<string, unknown>;
  if (typeof obj.rows !== 'number') throw new UnifiedValidationError('Matrix missing rows');
  if (typeof obj.cols !== 'number') throw new UnifiedValidationError('Matrix missing cols');
  if (precision === 'float64' && !isFloat64Array(obj.data)) throw new UnifiedValidationError('Expected Float64Array');
  if (precision === 'float32' && !isFloat32Array(obj.data)) throw new UnifiedValidationError('Expected Float32Array');
  return value as Matrix<P>;
}
```

### 7.4 Dispose Pattern for WASM Resources

Both ruvector and sublinear-time-solver use WASM classes that require explicit cleanup. The ruvector unified WASM `.d.ts` files already define `free()` and `[Symbol.dispose]()` methods:

```typescript
/**
 * Ensure both ruvector WASM objects and solver objects are properly disposed.
 * Uses the TC39 explicit resource management proposal (Symbol.dispose).
 */
interface Disposable {
  free(): void;
  [Symbol.dispose](): void;
}

class ManagedSolverSession implements Disposable {
  private solver: SublinearSolver;
  private attention?: WasmFlashAttention;
  private memory?: MemoryManager;

  async runPipeline(matrix: MatrixData): Promise<Result<Float64Array, SolverError>> {
    using solver = await createSolver({ maxIterations: 1000, tolerance: 1e-8 });
    // solver.free() called automatically when scope exits
    return solver.solve(matrix, vector);
  }

  free(): void {
    this.solver?.dispose();
    this.attention?.free();
    this.memory?.free();
  }

  [Symbol.dispose](): void {
    this.free();
  }
}
```

### 7.5 Discriminated Union Pattern for Operation Results

ruvector's delta-behavior module demonstrates extensive use of discriminated unions. The sublinear-time-solver should follow the same pattern for consistency:

```typescript
/**
 * Solver operation result following ruvector's discriminated union convention.
 * Each variant carries only the data relevant to that outcome.
 */
type SolveResult =
  | { type: 'converged'; solution: Float64Array; iterations: number; finalResidual: number }
  | { type: 'diverged'; lastResidual: number; iterationsCompleted: number }
  | { type: 'maxIterationsReached'; bestSolution: Float64Array; residual: number; iterations: number }
  | { type: 'singularMatrix'; row: number; col: number }
  | { type: 'memoryExhausted'; requestedBytes: number; availableBytes: number };

// Pattern matching via switch:
function handleSolveResult(result: SolveResult): void {
  switch (result.type) {
    case 'converged':
      console.log(`Solved in ${result.iterations} iterations`);
      break;
    case 'diverged':
      console.error(`Diverged at residual ${result.lastResidual}`);
      break;
    // exhaustive check enforced by TypeScript
  }
}
```

### 7.6 Branded Types for Domain Safety

```typescript
/**
 * Branded types prevent accidental mixing of semantically different numbers.
 * Prevents passing a vector dimension where a matrix row count is expected.
 */
type Brand<T, B extends string> = T & { readonly __brand: B };

type Dimension = Brand<number, 'Dimension'>;
type RowCount = Brand<number, 'RowCount'>;
type ColCount = Brand<number, 'ColCount'>;
type Tolerance = Brand<number, 'Tolerance'>;  // Must be > 0
type Coherence = Brand<number, 'Coherence'>;  // Must be 0-1

function asDimension(n: number): Dimension {
  if (n <= 0 || !Number.isInteger(n)) throw new UnifiedValidationError(`Invalid dimension: ${n}`);
  return n as Dimension;
}

function asTolerance(n: number): Tolerance {
  if (n <= 0) throw new UnifiedValidationError(`Tolerance must be positive: ${n}`);
  return n as Tolerance;
}
```

### 7.7 Module Initialization Coordination

Both systems require async WASM initialization. The integration should coordinate init order:

```typescript
/**
 * Initialize both ruvector WASM modules and solver WASM in the correct order.
 * Memory must be shared or coordinated to avoid double-allocation.
 */
async function initIntegration(config: IntegrationConfig): Promise<{
  solver: SublinearSolver;
  vectorDb: VectorDB;
  features: WasmFeatures;
}> {
  // Step 1: Initialize shared WASM memory
  const memory = new WebAssembly.Memory({
    initial: config.memory.initialPages,
    maximum: config.memory.maximumPages,
    shared: config.memory.shared,
  });

  // Step 2: Initialize ruvector WASM modules (attention, etc.)
  await ruvectorInit({ memory, enableSimd: config.wasm.simdEnabled });

  // Step 3: Initialize solver WASM (can share the same memory if compatible)
  const solver = await createSolver({
    ...config.solver,
    simdEnabled: config.wasm.simdEnabled,
  });

  // Step 4: Create VectorDB
  const vectorDb = new VectorDB({
    dimensions: config.solver.streamChunkSize, // or separate config
    hnswConfig: { efSearch: config.solver.maxIterations },
  });

  return { solver, vectorDb, features: solver.getFeatures() };
}
```

---

## Summary of Key Findings

### Critical Type Incompatibilities

1. **Float32Array vs Float64Array**: ruvector uses Float32Array everywhere; sublinear-time-solver uses Float64Array for matrix operations. A precision bridge is required.
2. **Sync vs Async Streams**: ruvector uses synchronous `next() -> T | null`; solver uses async chunked streams. The UnifiedStream adapter resolves this.
3. **Error Hierarchies**: Three separate error systems in ruvector with no shared base. Proposed unified hierarchy with numeric code ranges and the new 0x0600 solver category.

### Type Compatibility Strengths

1. **WASM Module Pattern**: Both systems follow the same `init() -> Promise<Module>` pattern with `WebAssembly.Memory`.
2. **Discriminated Unions**: ruvector's delta-behavior module already uses the same `{ type: '...' }` discriminated union pattern that fits solver results naturally.
3. **Dispose Protocol**: ruvector WASM types already implement `free()` and `[Symbol.dispose]()`, which the solver should adopt.
4. **Config Object Pattern**: Both use flat/nested config interfaces with optional fields and sensible defaults.

### Recommended Actions

1. **Create `src/types/shared-solver-types.ts`** with the shared types from Section 6.
2. **Parameterize solver types with `<P extends NumericArray>`** to support both Float32 and Float64 pipelines.
3. **Register solver error codes in range 0x0600-0x06FF** in the RvfErrorCode enum.
4. **Implement `UnifiedStream<T>`** as the standard streaming interface across both systems.
5. **Add branded types** for Dimension, Tolerance, and Coherence to prevent cross-domain value confusion.
6. **Coordinate WASM initialization** through a shared `initIntegration()` function that manages memory allocation order.

### Files Analyzed

- `/home/user/ruvector/npm/core/src/index.ts` -- Core VectorDB types
- `/home/user/ruvector/crates/ruvector-attention-wasm/js/types.ts` -- Attention config types
- `/home/user/ruvector/crates/ruvector-attention-wasm/js/index.ts` -- Attention class wrappers
- `/home/user/ruvector/crates/ruvector-attention-unified-wasm/pkg/ruvector_attention_unified_wasm.d.ts` -- Unified WASM declarations
- `/home/user/ruvector/examples/delta-behavior/wasm/src/types.ts` -- Delta-behavior type system
- `/home/user/ruvector/examples/delta-behavior/wasm/src/index.ts` -- Delta-behavior implementations
- `/home/user/ruvector/examples/delta-behavior/wasm/dist/types.d.ts` -- Compiled type declarations
- `/home/user/ruvector/examples/edge-net/dashboard/src/types/index.ts` -- Dashboard/WASM module types
- `/home/user/ruvector/examples/edge-net/dashboard/src/stores/wasmStore.ts` -- WASM store state management
- `/home/user/ruvector/examples/edge-full/pkg/index.d.ts` -- Edge-full module declarations
- `/home/user/ruvector/examples/scipix/web/types.ts` -- SciPix OCR types
- `/home/user/ruvector/npm/packages/ruvector-wasm-unified/src/types.ts` -- Unified WASM types (Tensor, Result, etc.)
- `/home/user/ruvector/npm/packages/rvf/src/errors.ts` -- RVF error codes and RvfError class
- `/home/user/ruvector/npm/packages/ruvbot/src/core/errors.ts` -- RuvBot error hierarchy
- `/home/user/ruvector/npm/packages/graph-node/index.d.ts` -- Graph DB streaming types
- `/home/user/ruvector/package.json` -- Root package configuration
