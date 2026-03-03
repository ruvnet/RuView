# WASM Integration Plan - Sublinear Time Solver

## Overview

This plan outlines the comprehensive WebAssembly (WASM) integration strategy for the sublinear-time-solver project, focusing on optimal Rust-to-JavaScript bindings, streaming solutions, and performance optimization.

## 1. WASM Build Pipeline

### 1.1 wasm-pack Configuration

Create `wasm-pack` configuration in `Cargo.toml`:

```toml
[package.metadata.wasm-pack.profile.release]
wee_alloc = false
debug_assertions = false
overflow_checks = false
lto = true
panic = "abort"
codegen_units = 1
opt_level = "s"

[package.metadata.wasm-pack.profile.dev]
debug_assertions = true
overflow_checks = true
opt_level = 0
```

### 1.2 Build Targets

Support multiple targets with conditional builds:

```bash
# Browser ES modules (bundler)
wasm-pack build --target bundler --out-dir pkg/bundler

# Node.js CommonJS
wasm-pack build --target nodejs --out-dir pkg/nodejs

# Web (no bundler)
wasm-pack build --target web --out-dir pkg/web

# Universal (UMD)
wasm-pack build --target no-modules --out-dir pkg/umd
```

### 1.3 Optimization Flags

**Cargo.toml profile optimizations:**

```toml
[profile.release]
opt-level = "s"          # Optimize for size
lto = true               # Link-time optimization
codegen-units = 1        # Single codegen unit for better optimization
panic = "abort"          # Smaller binary size
strip = true             # Strip debug symbols

[features]
default = ["simd"]
simd = ["wide"]          # Enable SIMD optimizations
parallel = ["rayon"]     # Parallel processing support
```

### 1.4 Binary Size Optimization

```toml
# Cargo.toml
[dependencies]
wee_alloc = { version = "0.4", optional = true }

[features]
wee_alloc = ["wee_alloc/static_array_backend"]
```

**Build script optimization:**

```bash
#!/bin/bash
# scripts/build-wasm.sh

# Enable SIMD and optimize
export RUSTFLAGS="-C target-feature=+simd128 -C opt-level=s"

# Build with size optimization
wasm-pack build \
  --target bundler \
  --out-dir pkg \
  --features "simd" \
  -- --no-default-features

# Optimize WASM binary
wasm-opt -Oz -o pkg/solver_bg.wasm pkg/solver_bg.wasm

# Generate multiple targets
for target in nodejs web no-modules; do
  wasm-pack build --target $target --out-dir "pkg/$target"
done
```

### 1.5 SIMD Enablement

```rust
// src/simd.rs
use std::simd::f64x8;

#[cfg(target_feature = "simd128")]
pub fn simd_matrix_multiply(a: &[f64], b: &[f64], result: &mut [f64]) {
    // SIMD-optimized matrix operations
    let chunks_a = a.chunks_exact(8);
    let chunks_b = b.chunks_exact(8);

    for ((chunk_a, chunk_b), result_chunk) in
        chunks_a.zip(chunks_b).zip(result.chunks_exact_mut(8)) {

        let va = f64x8::from_slice(chunk_a);
        let vb = f64x8::from_slice(chunk_b);
        let vr = va * vb;

        result_chunk.copy_from_slice(vr.as_array());
    }
}
```

## 2. JavaScript Interface Design

### 2.1 TypeScript Definitions Structure

```typescript
// types/index.d.ts
export interface SolverConfig {
  maxIterations: number;
  tolerance: number;
  simdEnabled: boolean;
  streamChunkSize: number;
}

export interface Matrix {
  data: Float64Array;
  rows: number;
  cols: number;
}

export interface SolutionStep {
  iteration: number;
  residual: number;
  timestamp: number;
  convergence: boolean;
}

export class SublinearSolver {
  constructor(config?: Partial<SolverConfig>);

  // Synchronous solve
  solve(matrix: Matrix, vector: Float64Array): Float64Array;

  // Streaming solve
  solveStream(matrix: Matrix, vector: Float64Array): AsyncIterableIterator<SolutionStep>;

  // Batch operations
  solveBatch(problems: Array<{matrix: Matrix, vector: Float64Array}>): Promise<Float64Array[]>;

  // Memory management
  dispose(): void;
  getMemoryUsage(): {used: number, capacity: number};
}

export interface WasmModule {
  memory: WebAssembly.Memory;
  solver_new: (config_ptr: number) => number;
  solver_solve: (solver_ptr: number, matrix_ptr: number, vector_ptr: number) => number;
  solver_solve_stream: (solver_ptr: number, matrix_ptr: number, vector_ptr: number, callback_ptr: number) => void;
  solver_free: (solver_ptr: number) => void;
}
```

### 2.2 Async Initialization Patterns

```typescript
// src/init.ts
export async function initSolver(config?: Partial<SolverConfig>): Promise<SublinearSolver> {
  // Dynamic import for better code splitting
  const wasmModule = await import('../pkg/solver');
  await wasmModule.default(); // Initialize WASM module

  return new SublinearSolver(wasmModule, config);
}

// Lazy initialization pattern
let solverPromise: Promise<SublinearSolver> | null = null;

export function getSolver(config?: Partial<SolverConfig>): Promise<SublinearSolver> {
  if (!solverPromise) {
    solverPromise = initSolver(config);
  }
  return solverPromise;
}
```

### 2.3 Memory Management (SharedArrayBuffer)

```typescript
// src/memory.ts
export class WasmMemoryManager {
  private memory: WebAssembly.Memory;
  private allocations = new Map<number, number>();

  constructor(memory: WebAssembly.Memory) {
    this.memory = memory;
  }

  allocateFloat64Array(length: number): {ptr: number, view: Float64Array} {
    const bytes = length * 8;
    const ptr = this.allocate(bytes);
    const view = new Float64Array(this.memory.buffer, ptr, length);
    return {ptr, view};
  }

  allocateSharedBuffer(length: number): {ptr: number, buffer: SharedArrayBuffer} {
    if (typeof SharedArrayBuffer === 'undefined') {
      throw new Error('SharedArrayBuffer not available');
    }

    const bytes = length * 8;
    const buffer = new SharedArrayBuffer(bytes);
    const ptr = this.allocate(bytes);

    // Copy to WASM memory
    const wasmView = new Float64Array(this.memory.buffer, ptr, length);
    const sharedView = new Float64Array(buffer);
    wasmView.set(sharedView);

    return {ptr, buffer};
  }

  private allocate(bytes: number): number {
    // Implementation depends on WASM allocator
    // This is a simplified version
    const ptr = (this.memory.buffer.byteLength);
    this.memory.grow(Math.ceil(bytes / 65536));
    this.allocations.set(ptr, bytes);
    return ptr;
  }

  deallocate(ptr: number): void {
    this.allocations.delete(ptr);
    // Call WASM deallocator
  }
}
```

### 2.4 Error Handling Across Boundaries

```typescript
// src/error-handling.ts
export enum SolverErrorType {
  InvalidMatrix = 'INVALID_MATRIX',
  ConvergenceFailure = 'CONVERGENCE_FAILURE',
  MemoryError = 'MEMORY_ERROR',
  WasmError = 'WASM_ERROR'
}

export class SolverError extends Error {
  constructor(
    public type: SolverErrorType,
    message: string,
    public details?: any
  ) {
    super(message);
    this.name = 'SolverError';
  }
}

// WASM error handler
export function handleWasmError(errorCode: number, context: string): never {
  const errorMap = {
    1: {type: SolverErrorType.InvalidMatrix, message: 'Matrix dimensions invalid'},
    2: {type: SolverErrorType.ConvergenceFailure, message: 'Failed to converge'},
    3: {type: SolverErrorType.MemoryError, message: 'Memory allocation failed'},
    99: {type: SolverErrorType.WasmError, message: 'Unknown WASM error'}
  };

  const error = errorMap[errorCode] || errorMap[99];
  throw new SolverError(error.type, `${error.message} in ${context}`, {errorCode});
}
```

### 2.5 Stream Implementation (AsyncIterator)

```typescript
// src/streaming.ts
export class SolutionStream implements AsyncIterableIterator<SolutionStep> {
  private wasmSolver: number;
  private wasmModule: WasmModule;
  private buffer: SolutionStep[] = [];
  private isComplete = false;
  private error: Error | null = null;

  constructor(wasmSolver: number, wasmModule: WasmModule) {
    this.wasmSolver = wasmSolver;
    this.wasmModule = wasmModule;
  }

  async *[Symbol.asyncIterator](): AsyncIterableIterator<SolutionStep> {
    while (!this.isComplete && !this.error) {
      if (this.buffer.length > 0) {
        yield this.buffer.shift()!;
      } else {
        await this.waitForData();
      }
    }

    if (this.error) {
      throw this.error;
    }

    // Yield remaining buffered items
    while (this.buffer.length > 0) {
      yield this.buffer.shift()!;
    }
  }

  async next(): Promise<IteratorResult<SolutionStep>> {
    if (this.buffer.length > 0) {
      return {value: this.buffer.shift()!, done: false};
    }

    if (this.isComplete) {
      return {value: undefined, done: true};
    }

    if (this.error) {
      throw this.error;
    }

    await this.waitForData();
    return this.next();
  }

  private async waitForData(): Promise<void> {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Stream timeout'));
      }, 10000);

      const checkData = () => {
        if (this.buffer.length > 0 || this.isComplete || this.error) {
          clearTimeout(timeout);
          resolve();
        } else {
          setTimeout(checkData, 10);
        }
      };

      checkData();
    });
  }

  // Called from WASM via callback
  onData(step: SolutionStep): void {
    this.buffer.push(step);
  }

  onComplete(): void {
    this.isComplete = true;
  }

  onError(error: Error): void {
    this.error = error;
  }
}
```

## 3. Data Transfer Optimization

### 3.1 Zero-Copy Strategies

```rust
// src/lib.rs
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct MatrixView {
    data: Vec<f64>,
    rows: usize,
    cols: usize,
}

#[wasm_bindgen]
impl MatrixView {
    #[wasm_bindgen(constructor)]
    pub fn new(rows: usize, cols: usize) -> MatrixView {
        MatrixView {
            data: vec![0.0; rows * cols],
            rows,
            cols,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn data(&self) -> *const f64 {
        self.data.as_ptr()
    }

    #[wasm_bindgen(getter)]
    pub fn length(&self) -> usize {
        self.data.len()
    }

    // Zero-copy access to data
    #[wasm_bindgen]
    pub fn data_view(&self) -> js_sys::Float64Array {
        unsafe {
            js_sys::Float64Array::view(&self.data)
        }
    }

    // Set data without copying
    #[wasm_bindgen]
    pub fn set_data(&mut self, data: &[f64]) {
        self.data.copy_from_slice(data);
    }
}
```

### 3.2 Float64Array/Uint32Array Usage

```typescript
// src/data-transfer.ts
export class EfficientDataTransfer {
  private wasmModule: WasmModule;
  private memoryManager: WasmMemoryManager;

  transferMatrix(matrix: Matrix): number {
    // Use existing memory view if possible
    if (matrix.data.buffer === this.wasmModule.memory.buffer) {
      return matrix.data.byteOffset;
    }

    // Allocate and copy
    const {ptr, view} = this.memoryManager.allocateFloat64Array(
      matrix.rows * matrix.cols
    );
    view.set(matrix.data);
    return ptr;
  }

  transferMatrixZeroCopy(matrix: Matrix): Float64Array {
    // Create view directly into WASM memory
    const wasmView = this.createWasmView(matrix.rows * matrix.cols);
    wasmView.set(matrix.data);
    return wasmView;
  }

  private createWasmView(length: number): Float64Array {
    const bytes = length * 8;
    const offset = this.wasmModule.memory.buffer.byteLength;

    // Grow memory if needed
    const pagesNeeded = Math.ceil(bytes / 65536);
    if (pagesNeeded > 0) {
      this.wasmModule.memory.grow(pagesNeeded);
    }

    return new Float64Array(this.wasmModule.memory.buffer, offset, length);
  }
}
```

### 3.3 Memory View Patterns

```typescript
// src/memory-views.ts
export class MemoryViewPool {
  private freeViews = new Map<number, Float64Array[]>();
  private inUseViews = new Set<Float64Array>();

  getView(length: number): Float64Array {
    const powerOf2 = Math.pow(2, Math.ceil(Math.log2(length)));
    const pool = this.freeViews.get(powerOf2) || [];

    let view = pool.pop();
    if (!view) {
      view = new Float64Array(powerOf2);
    }

    this.inUseViews.add(view);
    return view.subarray(0, length);
  }

  releaseView(view: Float64Array): void {
    if (!this.inUseViews.has(view)) return;

    this.inUseViews.delete(view);
    const length = view.buffer.byteLength / 8;
    const pool = this.freeViews.get(length) || [];
    pool.push(view);
    this.freeViews.set(length, pool);
  }

  clear(): void {
    this.freeViews.clear();
    this.inUseViews.clear();
  }
}
```

### 3.4 Batch Operation Interfaces

```typescript
// src/batch-operations.ts
export interface BatchSolveRequest {
  id: string;
  matrix: Matrix;
  vector: Float64Array;
  priority?: number;
}

export interface BatchSolveResult {
  id: string;
  solution: Float64Array;
  iterations: number;
  error?: string;
}

export class BatchSolver {
  private requestQueue: BatchSolveRequest[] = [];
  private processing = false;

  async solveBatch(requests: BatchSolveRequest[]): Promise<BatchSolveResult[]> {
    // Sort by priority
    requests.sort((a, b) => (b.priority || 0) - (a.priority || 0));

    // Batch allocate memory
    const totalMatrixElements = requests.reduce(
      (sum, req) => sum + req.matrix.rows * req.matrix.cols, 0
    );
    const totalVectorElements = requests.reduce(
      (sum, req) => sum + req.vector.length, 0
    );

    const matrixBuffer = new Float64Array(totalMatrixElements);
    const vectorBuffer = new Float64Array(totalVectorElements);

    // Copy data in batches
    let matrixOffset = 0;
    let vectorOffset = 0;

    const batchInfo = requests.map(req => {
      const matrixStart = matrixOffset;
      const vectorStart = vectorOffset;

      matrixBuffer.set(req.matrix.data, matrixOffset);
      vectorBuffer.set(req.vector, vectorOffset);

      matrixOffset += req.matrix.data.length;
      vectorOffset += req.vector.length;

      return {
        id: req.id,
        matrixStart,
        matrixLength: req.matrix.data.length,
        vectorStart,
        vectorLength: req.vector.length,
        rows: req.matrix.rows,
        cols: req.matrix.cols
      };
    });

    // Call WASM batch solver
    return this.callWasmBatchSolver(matrixBuffer, vectorBuffer, batchInfo);
  }

  private async callWasmBatchSolver(
    matrices: Float64Array,
    vectors: Float64Array,
    batchInfo: any[]
  ): Promise<BatchSolveResult[]> {
    // Implementation calls WASM batch processing function
    // Returns processed results
    return [];
  }
}
```

### 3.5 Callback vs Polling Approaches

```typescript
// src/communication-patterns.ts
export class CallbackBasedSolver {
  private callbacks = new Map<string, (data: any) => void>();

  solveWithCallback(
    matrix: Matrix,
    vector: Float64Array,
    onProgress: (step: SolutionStep) => void,
    onComplete: (solution: Float64Array) => void,
    onError: (error: Error) => void
  ): void {
    const callbackId = Math.random().toString(36);

    this.callbacks.set(`progress_${callbackId}`, onProgress);
    this.callbacks.set(`complete_${callbackId}`, onComplete);
    this.callbacks.set(`error_${callbackId}`, onError);

    // Call WASM with callback ID
    this.wasmModule.solver_solve_with_callback(
      this.solver,
      this.transferMatrix(matrix),
      this.transferVector(vector),
      callbackId
    );
  }

  // Called from WASM
  handleCallback(callbackId: string, type: string, data: any): void {
    const callback = this.callbacks.get(`${type}_${callbackId}`);
    if (callback) {
      callback(data);

      if (type === 'complete' || type === 'error') {
        // Cleanup callbacks
        this.callbacks.delete(`progress_${callbackId}`);
        this.callbacks.delete(`complete_${callbackId}`);
        this.callbacks.delete(`error_${callbackId}`);
      }
    }
  }
}

export class PollingBasedSolver {
  private activeJobs = new Map<string, SolveJob>();

  async solveWithPolling(matrix: Matrix, vector: Float64Array): Promise<Float64Array> {
    const jobId = Math.random().toString(36);

    // Start solving
    this.wasmModule.solver_solve_async(
      this.solver,
      this.transferMatrix(matrix),
      this.transferVector(vector),
      jobId
    );

    // Poll for completion
    return new Promise((resolve, reject) => {
      const poll = () => {
        const status = this.wasmModule.solver_get_status(jobId);

        switch (status.state) {
          case 'completed':
            resolve(status.result);
            break;
          case 'error':
            reject(new Error(status.error));
            break;
          case 'running':
            setTimeout(poll, 10);
            break;
        }
      };

      poll();
    });
  }
}
```

## 4. NPM Package Structure

```
package/
├── package.json
├── index.js                 # Entry point
├── index.d.ts              # TypeScript definitions
├── dist/
│   ├── browser/            # Browser-optimized build
│   │   ├── index.js
│   │   ├── index.d.ts
│   │   └── solver.wasm
│   ├── node/               # Node.js optimized build
│   │   ├── index.js
│   │   ├── index.d.ts
│   │   └── solver.wasm
│   ├── web/                # Direct web usage
│   │   ├── index.js
│   │   ├── index.d.ts
│   │   └── solver.wasm
│   └── types/              # TypeScript definitions
│       ├── index.d.ts
│       ├── solver.d.ts
│       └── streaming.d.ts
├── wasm/                   # WASM binaries
│   ├── solver_bg.wasm
│   ├── solver.js
│   └── solver.d.ts
├── src/                    # Source TypeScript
│   ├── index.ts
│   ├── solver.ts
│   ├── streaming.ts
│   └── memory.ts
├── bin/                    # CLI tools
│   └── cli.js
├── examples/               # Usage examples
│   ├── browser.html
│   ├── node.js
│   └── streaming.js
└── bench/                  # Benchmarks
    ├── performance.js
    └── memory.js
```

### package.json Configuration

```json
{
  "name": "sublinear-time-solver",
  "version": "1.0.0",
  "description": "High-performance WebAssembly linear algebra solver",
  "main": "dist/node/index.js",
  "module": "dist/browser/index.js",
  "browser": "dist/browser/index.js",
  "types": "dist/types/index.d.ts",
  "exports": {
    ".": {
      "browser": "./dist/browser/index.js",
      "node": "./dist/node/index.js",
      "import": "./dist/browser/index.js",
      "require": "./dist/node/index.js",
      "types": "./dist/types/index.d.ts"
    },
    "./wasm": {
      "browser": "./wasm/solver.js",
      "node": "./wasm/solver.js",
      "types": "./wasm/solver.d.ts"
    }
  },
  "files": [
    "dist/",
    "wasm/",
    "bin/",
    "index.js",
    "index.d.ts"
  ],
  "bin": {
    "sublinear-solver": "./bin/cli.js"
  },
  "scripts": {
    "build": "npm run build:wasm && npm run build:js",
    "build:wasm": "./scripts/build-wasm.sh",
    "build:js": "tsc && webpack",
    "test": "jest",
    "bench": "node bench/performance.js",
    "prepare": "npm run build"
  },
  "dependencies": {
    "@types/node": "^18.0.0"
  },
  "devDependencies": {
    "wasm-pack": "^0.12.0",
    "typescript": "^5.0.0",
    "webpack": "^5.0.0",
    "jest": "^29.0.0"
  },
  "engines": {
    "node": ">=16.0.0"
  },
  "repository": {
    "type": "git",
    "url": "https://github.com/your-org/sublinear-time-solver"
  }
}
```

## 5. Streaming Solution Design

### 5.1 AsyncIterator Implementation

```typescript
// src/streaming/async-iterator.ts
export class StreamingSolver implements AsyncIterable<SolutionStep> {
  private controller: ReadableStreamDefaultController<SolutionStep> | null = null;
  private stream: ReadableStream<SolutionStep> | null = null;

  async *solve(matrix: Matrix, vector: Float64Array): AsyncIterableIterator<SolutionStep> {
    const stream = this.createSolutionStream(matrix, vector);
    const reader = stream.getReader();

    try {
      while (true) {
        const {done, value} = await reader.read();
        if (done) break;
        yield value;
      }
    } finally {
      reader.releaseLock();
    }
  }

  private createSolutionStream(matrix: Matrix, vector: Float64Array): ReadableStream<SolutionStep> {
    return new ReadableStream<SolutionStep>({
      start: (controller) => {
        this.controller = controller;
        this.startWasmSolver(matrix, vector);
      },

      cancel: () => {
        this.stopWasmSolver();
      }
    });
  }

  private startWasmSolver(matrix: Matrix, vector: Float64Array): void {
    // Transfer data to WASM
    const matrixPtr = this.transferMatrix(matrix);
    const vectorPtr = this.transferVector(vector);

    // Start streaming solve
    this.wasmModule.solver_solve_stream(
      this.solver,
      matrixPtr,
      vectorPtr,
      this.createStreamCallback()
    );
  }

  private createStreamCallback(): number {
    // Create callback that WASM can call
    const callback = (step: SolutionStep) => {
      if (this.controller) {
        this.controller.enqueue(step);

        if (step.convergence) {
          this.controller.close();
        }
      }
    };

    return this.registerCallback(callback);
  }
}
```

### 5.2 Chunked Computation Strategy

```rust
// src/streaming.rs
use wasm_bindgen::prelude::*;
use std::sync::Mutex;

#[wasm_bindgen]
pub struct StreamingSolver {
    config: SolverConfig,
    state: Mutex<SolverState>,
}

#[wasm_bindgen]
impl StreamingSolver {
    #[wasm_bindgen(constructor)]
    pub fn new(chunk_size: usize) -> StreamingSolver {
        StreamingSolver {
            config: SolverConfig { chunk_size },
            state: Mutex::new(SolverState::new()),
        }
    }

    #[wasm_bindgen]
    pub fn solve_chunked(
        &self,
        matrix_ptr: *const f64,
        matrix_rows: usize,
        matrix_cols: usize,
        vector_ptr: *const f64,
        vector_len: usize,
        callback: &js_sys::Function,
    ) {
        let matrix = unsafe {
            std::slice::from_raw_parts(matrix_ptr, matrix_rows * matrix_cols)
        };
        let vector = unsafe {
            std::slice::from_raw_parts(vector_ptr, vector_len)
        };

        let mut state = self.state.lock().unwrap();
        state.initialize(matrix, vector);

        // Process in chunks
        while !state.is_converged() && state.iteration < self.config.max_iterations {
            let chunk_result = self.process_chunk(&mut state);

            // Create solution step
            let step = SolutionStep {
                iteration: state.iteration,
                residual: chunk_result.residual,
                timestamp: js_sys::Date::now(),
                convergence: chunk_result.converged,
            };

            // Call JavaScript callback
            let step_obj = serde_wasm_bindgen::to_value(&step).unwrap();
            callback.call1(&JsValue::NULL, &step_obj).unwrap();

            if chunk_result.converged {
                break;
            }

            // Yield control back to JavaScript event loop
            self.yield_control();
        }
    }

    fn process_chunk(&self, state: &mut SolverState) -> ChunkResult {
        // Process one chunk of iterations
        for _ in 0..self.config.chunk_size {
            state.iteration += 1;

            // Perform one iteration step
            let residual = self.conjugate_gradient_step(state);

            if residual < self.config.tolerance {
                return ChunkResult {
                    residual,
                    converged: true,
                };
            }
        }

        ChunkResult {
            residual: state.current_residual,
            converged: false,
        }
    }

    fn yield_control(&self) {
        // Use setTimeout to yield control
        let closure = Closure::once(|| {});
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                0,
            )
            .unwrap();
        closure.forget();
    }
}
```

### 5.3 Event Emitter Patterns

```typescript
// src/streaming/event-emitter.ts
export class SolverEventEmitter extends EventTarget {
  solve(matrix: Matrix, vector: Float64Array): Promise<Float64Array> {
    return new Promise((resolve, reject) => {
      this.addEventListener('progress', (event: CustomEvent<SolutionStep>) => {
        // Handle progress updates
        console.log(`Iteration ${event.detail.iteration}: residual=${event.detail.residual}`);
      });

      this.addEventListener('complete', (event: CustomEvent<Float64Array>) => {
        resolve(event.detail);
      });

      this.addEventListener('error', (event: CustomEvent<Error>) => {
        reject(event.detail);
      });

      // Start WASM solver with event callbacks
      this.startSolverWithEvents(matrix, vector);
    });
  }

  private startSolverWithEvents(matrix: Matrix, vector: Float64Array): void {
    const progressCallback = (step: SolutionStep) => {
      this.dispatchEvent(new CustomEvent('progress', {detail: step}));
    };

    const completeCallback = (solution: Float64Array) => {
      this.dispatchEvent(new CustomEvent('complete', {detail: solution}));
    };

    const errorCallback = (error: Error) => {
      this.dispatchEvent(new CustomEvent('error', {detail: error}));
    };

    this.wasmModule.solver_solve_with_events(
      this.solver,
      this.transferMatrix(matrix),
      this.transferVector(vector),
      this.registerCallback(progressCallback),
      this.registerCallback(completeCallback),
      this.registerCallback(errorCallback)
    );
  }
}
```

### 5.4 Backpressure Handling

```typescript
// src/streaming/backpressure.ts
export class BackpressureController {
  private bufferSize: number;
  private highWaterMark: number;
  private lowWaterMark: number;
  private currentBufferSize = 0;
  private isPaused = false;

  constructor(bufferSize = 1000, highWaterMark = 0.8, lowWaterMark = 0.2) {
    this.bufferSize = bufferSize;
    this.highWaterMark = Math.floor(bufferSize * highWaterMark);
    this.lowWaterMark = Math.floor(bufferSize * lowWaterMark);
  }

  shouldPause(): boolean {
    return this.currentBufferSize >= this.highWaterMark;
  }

  shouldResume(): boolean {
    return this.currentBufferSize <= this.lowWaterMark;
  }

  addToBuffer(size: number): void {
    this.currentBufferSize += size;

    if (!this.isPaused && this.shouldPause()) {
      this.isPaused = true;
      this.pauseProducer();
    }
  }

  removeFromBuffer(size: number): void {
    this.currentBufferSize = Math.max(0, this.currentBufferSize - size);

    if (this.isPaused && this.shouldResume()) {
      this.isPaused = false;
      this.resumeProducer();
    }
  }

  private pauseProducer(): void {
    // Signal WASM to pause production
    this.wasmModule.solver_pause_streaming();
  }

  private resumeProducer(): void {
    // Signal WASM to resume production
    this.wasmModule.solver_resume_streaming();
  }
}
```

### 5.5 Progress Reporting

```typescript
// src/streaming/progress.ts
export interface ProgressInfo {
  percentage: number;
  estimatedTimeRemaining: number;
  currentIteration: number;
  totalIterations: number;
  residual: number;
  convergenceRate: number;
}

export class ProgressTracker {
  private startTime: number = 0;
  private iterations: number[] = [];
  private residuals: number[] = [];
  private maxHistory = 100;

  startTracking(): void {
    this.startTime = Date.now();
    this.iterations = [];
    this.residuals = [];
  }

  updateProgress(step: SolutionStep): ProgressInfo {
    this.iterations.push(step.iteration);
    this.residuals.push(step.residual);

    // Keep only recent history
    if (this.iterations.length > this.maxHistory) {
      this.iterations.shift();
      this.residuals.shift();
    }

    const currentTime = Date.now();
    const elapsedTime = currentTime - this.startTime;

    // Estimate convergence rate
    const convergenceRate = this.estimateConvergenceRate();

    // Estimate total iterations needed
    const estimatedTotalIterations = this.estimateTotalIterations(
      step.residual,
      convergenceRate
    );

    // Calculate progress percentage
    const percentage = Math.min(
      100,
      (step.iteration / estimatedTotalIterations) * 100
    );

    // Estimate time remaining
    const iterationsPerMs = step.iteration / elapsedTime;
    const remainingIterations = Math.max(0, estimatedTotalIterations - step.iteration);
    const estimatedTimeRemaining = remainingIterations / iterationsPerMs;

    return {
      percentage,
      estimatedTimeRemaining,
      currentIteration: step.iteration,
      totalIterations: estimatedTotalIterations,
      residual: step.residual,
      convergenceRate,
    };
  }

  private estimateConvergenceRate(): number {
    if (this.residuals.length < 2) return 0;

    const recent = this.residuals.slice(-10);
    if (recent.length < 2) return 0;

    // Calculate average reduction rate
    let totalRate = 0;
    for (let i = 1; i < recent.length; i++) {
      if (recent[i-1] > 0) {
        totalRate += recent[i] / recent[i-1];
      }
    }

    return totalRate / (recent.length - 1);
  }

  private estimateTotalIterations(currentResidual: number, convergenceRate: number): number {
    if (convergenceRate >= 1 || convergenceRate <= 0) {
      return this.iterations[this.iterations.length - 1] * 2; // Conservative estimate
    }

    const targetResidual = 1e-10; // Target convergence
    const iterationsNeeded = Math.log(targetResidual / currentResidual) / Math.log(convergenceRate);

    return Math.max(
      this.iterations[this.iterations.length - 1],
      this.iterations[this.iterations.length - 1] + iterationsNeeded
    );
  }
}
```

## 6. Performance Benchmarks

### 6.1 WASM vs Native Rust Comparison

```typescript
// bench/wasm-vs-native.ts
export class WasmNativeBenchmark {
  async runComparison(sizes: number[]): Promise<BenchmarkResults> {
    const results = {
      wasm: [],
      native: [],
      overhead: []
    };

    for (const size of sizes) {
      const matrix = this.generateMatrix(size, size);
      const vector = this.generateVector(size);

      // WASM benchmark
      const wasmTime = await this.benchmarkWasm(matrix, vector);
      results.wasm.push({size, time: wasmTime});

      // Native Rust benchmark (via Node.js addon)
      const nativeTime = await this.benchmarkNative(matrix, vector);
      results.native.push({size, time: nativeTime});

      // Calculate overhead
      const overhead = ((wasmTime - nativeTime) / nativeTime) * 100;
      results.overhead.push({size, overhead});

      console.log(`Size ${size}: WASM=${wasmTime}ms, Native=${nativeTime}ms, Overhead=${overhead.toFixed(2)}%`);
    }

    return results;
  }

  private async benchmarkWasm(matrix: Matrix, vector: Float64Array): Promise<number> {
    const solver = await this.createWasmSolver();

    const start = performance.now();
    const result = solver.solve(matrix, vector);
    const end = performance.now();

    return end - start;
  }

  private async benchmarkNative(matrix: Matrix, vector: Float64Array): Promise<number> {
    // This would call a native Node.js addon for comparison
    // Implementation depends on your native benchmark setup
    return 0;
  }
}
```

### 6.2 JS Overhead Measurement

```typescript
// bench/js-overhead.ts
export class JavaScriptOverheadBenchmark {
  async measureDataTransferOverhead(): Promise<OverheadResults> {
    const sizes = [100, 1000, 10000, 100000];
    const results = [];

    for (const size of sizes) {
      const data = new Float64Array(size);
      for (let i = 0; i < size; i++) {
        data[i] = Math.random();
      }

      // Measure direct WASM call
      const directTime = await this.measureDirectCall(data);

      // Measure with JS wrapper
      const wrapperTime = await this.measureWrapperCall(data);

      // Measure data copying
      const copyTime = await this.measureDataCopy(data);

      const overhead = {
        size,
        directTime,
        wrapperTime,
        copyTime,
        wrapperOverhead: ((wrapperTime - directTime) / directTime) * 100,
        copyOverhead: (copyTime / directTime) * 100
      };

      results.push(overhead);
      console.log(`Size ${size}: Direct=${directTime}ms, Wrapper=${wrapperTime}ms, Copy=${copyTime}ms`);
    }

    return {results, summary: this.calculateSummary(results)};
  }

  private async measureDirectCall(data: Float64Array): Promise<number> {
    const ptr = this.allocateWasmMemory(data.length);
    this.copyToWasmMemory(ptr, data);

    const start = performance.now();
    this.wasmModule.direct_computation(ptr, data.length);
    const end = performance.now();

    this.freeWasmMemory(ptr);
    return end - start;
  }

  private async measureWrapperCall(data: Float64Array): Promise<number> {
    const start = performance.now();
    this.solver.computeWithWrapper(data);
    const end = performance.now();

    return end - start;
  }

  private async measureDataCopy(data: Float64Array): Promise<number> {
    const start = performance.now();
    const copy = new Float64Array(data);
    const end = performance.now();

    return end - start;
  }
}
```

### 6.3 Memory Usage Profiling

```typescript
// bench/memory-profiler.ts
export class MemoryProfiler {
  private memorySnapshots: MemorySnapshot[] = [];

  async profileSolverMemory(matrix: Matrix, vector: Float64Array): Promise<MemoryProfile> {
    this.startProfiling();

    // Initialize solver
    this.takeSnapshot('initialization');
    const solver = await this.createSolver();
    this.takeSnapshot('solver_created');

    // Transfer data
    const matrixPtr = solver.transferMatrix(matrix);
    this.takeSnapshot('matrix_transferred');

    const vectorPtr = solver.transferVector(vector);
    this.takeSnapshot('vector_transferred');

    // Solve
    const solution = await solver.solve(matrixPtr, vectorPtr);
    this.takeSnapshot('solution_complete');

    // Cleanup
    solver.dispose();
    this.takeSnapshot('cleanup_complete');

    return this.generateProfile();
  }

  private takeSnapshot(label: string): void {
    const snapshot = {
      label,
      timestamp: Date.now(),
      wasmMemory: this.getWasmMemoryUsage(),
      jsHeap: this.getJSHeapUsage(),
      totalMemory: performance.memory?.usedJSHeapSize || 0
    };

    this.memorySnapshots.push(snapshot);
  }

  private getWasmMemoryUsage(): number {
    return this.wasmModule.memory.buffer.byteLength;
  }

  private getJSHeapUsage(): number {
    return performance.memory?.usedJSHeapSize || 0;
  }

  private generateProfile(): MemoryProfile {
    const peak = Math.max(...this.memorySnapshots.map(s => s.totalMemory));
    const growth = this.calculateMemoryGrowth();
    const leaks = this.detectMemoryLeaks();

    return {
      snapshots: this.memorySnapshots,
      peakUsage: peak,
      memoryGrowth: growth,
      potentialLeaks: leaks,
      recommendations: this.generateRecommendations()
    };
  }
}
```

### 6.4 Streaming Latency Metrics

```typescript
// bench/streaming-benchmark.ts
export class StreamingLatencyBenchmark {
  async measureStreamingPerformance(): Promise<StreamingMetrics> {
    const matrix = this.generateLargeMatrix(10000, 10000);
    const vector = this.generateVector(10000);

    const metrics = {
      firstYield: 0,
      averageLatency: 0,
      throughput: 0,
      bufferUtilization: [],
      backpressureEvents: 0
    };

    const solver = await this.createStreamingSolver();
    const startTime = performance.now();
    let firstYield = true;
    let totalYields = 0;
    let totalLatency = 0;

    for await (const step of solver.solve(matrix, vector)) {
      const currentTime = performance.now();

      if (firstYield) {
        metrics.firstYield = currentTime - startTime;
        firstYield = false;
      }

      // Measure latency between yields
      if (totalYields > 0) {
        const latency = currentTime - this.lastYieldTime;
        totalLatency += latency;
      }

      totalYields++;
      this.lastYieldTime = currentTime;

      // Track buffer utilization
      metrics.bufferUtilization.push(solver.getBufferUtilization());

      // Count backpressure events
      if (solver.isBackpressureActive()) {
        metrics.backpressureEvents++;
      }
    }

    const totalTime = performance.now() - startTime;
    metrics.averageLatency = totalLatency / (totalYields - 1);
    metrics.throughput = totalYields / (totalTime / 1000); // yields per second

    return metrics;
  }
}
```

## Example TypeScript Usage

```typescript
// examples/basic-usage.ts
import { SublinearSolver, Matrix, initSolver } from 'sublinear-time-solver';

async function basicExample() {
  // Initialize solver
  const solver = await initSolver({
    maxIterations: 1000,
    tolerance: 1e-10,
    simdEnabled: true,
    streamChunkSize: 100
  });

  // Create test matrix and vector
  const matrix: Matrix = {
    data: new Float64Array([4, 1, 1, 3]),
    rows: 2,
    cols: 2
  };
  const vector = new Float64Array([1, 2]);

  // Synchronous solve
  const solution = solver.solve(matrix, vector);
  console.log('Solution:', solution);

  // Streaming solve with progress tracking
  console.log('Streaming solve:');
  for await (const step of solver.solveStream(matrix, vector)) {
    console.log(`Iteration ${step.iteration}: residual=${step.residual}`);
    if (step.convergence) {
      console.log('Converged!');
      break;
    }
  }

  // Batch solving
  const problems = [
    {matrix, vector},
    {matrix: matrix, vector: new Float64Array([2, 3])},
    {matrix: matrix, vector: new Float64Array([3, 4])}
  ];

  const batchSolutions = await solver.solveBatch(problems);
  console.log('Batch solutions:', batchSolutions);

  // Memory usage
  console.log('Memory usage:', solver.getMemoryUsage());

  // Cleanup
  solver.dispose();
}

// Event-driven example
async function eventDrivenExample() {
  const solver = await initSolver();

  solver.addEventListener('progress', (event) => {
    console.log(`Progress: ${event.detail.percentage}%`);
  });

  solver.addEventListener('complete', (event) => {
    console.log('Solution:', event.detail);
  });

  const matrix: Matrix = {
    data: new Float64Array([...]),
    rows: 1000,
    cols: 1000
  };
  const vector = new Float64Array(1000);

  solver.solveAsync(matrix, vector);
}

basicExample().catch(console.error);
```

## WASM-Bindgen Annotations

```rust
// src/lib.rs
use wasm_bindgen::prelude::*;

// Enable console.log from Rust
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub struct SublinearSolver {
    config: SolverConfig,
    state: Option<SolverState>,
}

#[wasm_bindgen]
impl SublinearSolver {
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<SublinearSolver, JsValue> {
        let config: SolverConfig = serde_wasm_bindgen::from_value(config)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(SublinearSolver {
            config,
            state: None,
        })
    }

    #[wasm_bindgen]
    pub fn solve(
        &mut self,
        matrix_data: &[f64],
        matrix_rows: usize,
        matrix_cols: usize,
        vector_data: &[f64],
    ) -> Result<Vec<f64>, JsValue> {
        // Validate input
        if matrix_data.len() != matrix_rows * matrix_cols {
            return Err(JsValue::from_str("Matrix dimensions mismatch"));
        }

        if vector_data.len() != matrix_rows {
            return Err(JsValue::from_str("Vector size mismatch"));
        }

        // Create matrix and vector views
        let matrix = Matrix::from_slice(matrix_data, matrix_rows, matrix_cols);
        let vector = Vector::from_slice(vector_data);

        // Solve system
        match self.conjugate_gradient_solve(&matrix, &vector) {
            Ok(solution) => Ok(solution.to_vec()),
            Err(e) => Err(JsValue::from_str(&e.to_string())),
        }
    }

    #[wasm_bindgen]
    pub fn solve_stream(
        &mut self,
        matrix_data: &[f64],
        matrix_rows: usize,
        matrix_cols: usize,
        vector_data: &[f64],
        progress_callback: &js_sys::Function,
    ) -> Result<(), JsValue> {
        let matrix = Matrix::from_slice(matrix_data, matrix_rows, matrix_cols);
        let vector = Vector::from_slice(vector_data);

        self.conjugate_gradient_stream(&matrix, &vector, |step| {
            let step_js = serde_wasm_bindgen::to_value(&step).unwrap();
            progress_callback.call1(&JsValue::NULL, &step_js).unwrap();
        })?;

        Ok(())
    }

    #[wasm_bindgen(getter)]
    pub fn memory_usage(&self) -> JsValue {
        let usage = MemoryUsage {
            used: self.get_used_memory(),
            capacity: self.get_total_capacity(),
        };

        serde_wasm_bindgen::to_value(&usage).unwrap()
    }

    #[wasm_bindgen]
    pub fn dispose(&mut self) {
        self.state = None;
        // Additional cleanup
    }
}

// Enable SIMD features
#[wasm_bindgen]
pub fn enable_simd() -> bool {
    cfg!(target_feature = "simd128")
}

// Memory management utilities
#[wasm_bindgen]
pub fn allocate_matrix(rows: usize, cols: usize) -> *mut f64 {
    let size = rows * cols;
    let layout = std::alloc::Layout::array::<f64>(size).unwrap();
    unsafe { std::alloc::alloc(layout) as *mut f64 }
}

#[wasm_bindgen]
pub fn deallocate_matrix(ptr: *mut f64, rows: usize, cols: usize) {
    let size = rows * cols;
    let layout = std::alloc::Layout::array::<f64>(size).unwrap();
    unsafe { std::alloc::dealloc(ptr as *mut u8, layout) }
}

// Feature detection
#[wasm_bindgen]
pub fn get_features() -> JsValue {
    let features = Features {
        simd_enabled: cfg!(target_feature = "simd128"),
        parallel_enabled: cfg!(feature = "parallel"),
        memory_64: cfg!(target_pointer_width = "64"),
    };

    serde_wasm_bindgen::to_value(&features).unwrap()
}
```

## Implementation Roadmap

### Phase 1: Core WASM Integration (Weeks 1-2)
- [ ] Set up wasm-pack build pipeline
- [ ] Implement basic Rust-to-JS bindings
- [ ] Create TypeScript interface definitions
- [ ] Basic memory management

### Phase 2: Performance Optimization (Weeks 3-4)
- [ ] SIMD optimization implementation
- [ ] Zero-copy data transfer patterns
- [ ] Memory pooling and management
- [ ] Build target optimization

### Phase 3: Streaming Implementation (Weeks 5-6)
- [ ] AsyncIterator streaming interface
- [ ] Chunked computation strategy
- [ ] Backpressure handling
- [ ] Progress reporting system

### Phase 4: NPM Package & Testing (Weeks 7-8)
- [ ] Complete NPM package structure
- [ ] Comprehensive test suite
- [ ] Performance benchmarking
- [ ] Documentation and examples

### Phase 5: Advanced Features (Weeks 9-10)
- [ ] Batch operation interfaces
- [ ] Event-driven patterns
- [ ] CLI tool implementation
- [ ] Browser compatibility testing

This comprehensive WASM integration plan provides a solid foundation for building high-performance WebAssembly bindings for the sublinear-time-solver project, with focus on optimal performance, streaming capabilities, and excellent developer experience.