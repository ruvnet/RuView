# ADR-STS-009: Concurrency Model and Parallelism Strategy

## Status

Accepted

## Date

2026-02-20

## Authors

RuVector Architecture Team

## Deciders

Architecture Review Board

---

## Context

The sublinear-time solver integration must align with ruvector's existing concurrency
infrastructure while introducing its own nanosecond-precision scheduler. The current
codebase employs a well-defined concurrency stack:

- **Rayon 1.10** -- Data-parallel iterators, feature-gated behind `parallel` (disabled on
  `wasm32`). Used in `ruvector-core` for `batch_distances()` via `par_iter()` and in
  `FlatIndex::search()` via `par_bridge()` over `DashMap` iterators. Optional in 8+ crates
  including `ruvector-delta-wasm`, `ruvector-delta-index`, `ruQu`, `ruvector-delta-graph`.
- **Crossbeam 0.8** -- Lock-free data structures (`ArrayQueue`, `SegQueue`, `CachePadded`).
  Powers the existing `LockFreeWorkQueue`, `LockFreeCounter`, `LockFreeStats`,
  `LockFreeBatchProcessor`, `ObjectPool`, and `AtomicVectorPool` in
  `crates/ruvector-core/src/lockfree.rs`.
- **DashMap 6.1** -- Concurrent hash map used throughout core indexing, delta operations,
  and graph storage. Provides lock-free read access via sharded `RwLock` segments.
- **parking_lot 0.12** -- Efficient mutex and RwLock replacements, used in 12+ crates
  across the workspace (sona, delta-core, delta-index, delta-graph, rvlite, etc.).
- **Tokio 1.41** -- Async runtime with multi-thread scheduler. Node.js bindings use
  `tokio::task::spawn_blocking` pervasively (observed in `ruvector-attention-node`,
  `ruvector-graph-node`, `ruvector-tiny-dancer-node`, `ruvector-router-ffi`).
  Synchronization primitives (`mpsc`, `RwLock`, `oneshot`, `Notify`) used in raft,
  replication, serving engines, and MCP gate.
- **Custom lock-free structures** -- `AtomicVectorPool` provides zero-allocation vector
  reuse with `SegQueue`-backed pooling and `CachePadded` atomics. `LockFreeWorkQueue`
  wraps `ArrayQueue` for bounded work distribution. `LockFreeBatchProcessor` combines both
  for producer-consumer batch processing.

The sublinear-time solver introduces its own nanosecond-precision scheduler capable of
98ns tick intervals and 11M+ task dispatches per second. This scheduler manages fine-grained
intra-solve task decomposition (matrix partitioning, random-walk step scheduling, Neumann
series term evaluation). It must coexist with Rayon's thread pool without causing thread
starvation or pool exhaustion.

WASM targets (`wasm32-unknown-unknown`) lack native thread support. The existing ruvector
WASM crates use Web Workers with `postMessage` for parallelism, managed by a `WorkerPool`
class (round-robin distribution, promise-based API, 30-second timeout). The solver's WASM
target must follow this established pattern.

Thread scaling measurements from `bench_thread_scaling` in `comprehensive_bench.rs`
(10,000 vectors at 384 dimensions, Euclidean distance):

| Threads | Relative Efficiency | Bottleneck |
|---------|-------------------|------------|
| 1 | 100% (baseline) | N/A |
| 2 | 85-95% | Synchronization overhead |
| 4 | 70-85% | L3 cache contention |
| 8 | 50-70% | Memory bandwidth saturation |

The sub-linear scaling beyond 4 threads indicates memory bandwidth as the dominant
constraint for vectorized workloads, not CPU compute. This fundamentally shapes the
parallelism strategy for solver integration.

---

## Decision

### 1. Two-Level Parallelism Architecture

The solver integration adopts a strict two-level parallelism model that separates
coarse-grained data parallelism from fine-grained compute parallelism.

**Outer level**: Rayon `par_iter()` for batch operations across independent solve
invocations (multi-query, batch solve, parallel search-then-solve pipelines).

**Inner level**: SIMD vectorization within each individual solve invocation. This
includes both auto-vectorized loops (compiler-driven via `-C target-cpu=native`) and
explicit SIMD intrinsics (AVX2/NEON/WASM SIMD) for critical inner loops such as
matrix-vector products and distance computations.

**No nested Rayon** is permitted. A Rayon task must never spawn additional Rayon parallel
iterators, as this risks exhausting the global thread pool and causing deadlocks. The
solver's internal task decomposition uses its own scheduler (Decision 2) rather than
recursive Rayon parallelism.

```
+------------------------------------------------------------------+
|                     Application Layer                             |
|  batch_solve([q1, q2, ..., qN])                                 |
+------------------------------------------------------------------+
         |                    |                    |
         v                    v                    v
+------------------+ +------------------+ +------------------+
| Rayon Task #1    | | Rayon Task #2    | | Rayon Task #N    |
| (Outer Level)    | | (Outer Level)    | | (Outer Level)    |
| solve(q1)        | | solve(q2)        | | solve(qN)        |
+------------------+ +------------------+ +------------------+
         |                    |                    |
         v                    v                    v
+------------------+ +------------------+ +------------------+
| Solver Scheduler | | Solver Scheduler | | Solver Scheduler |
| 98ns tick         | | 98ns tick         | | 98ns tick         |
| (Inner Level)    | | (Inner Level)    | | (Inner Level)    |
+------------------+ +------------------+ +------------------+
         |                    |                    |
         v                    v                    v
+------------------+ +------------------+ +------------------+
| SIMD Kernel      | | SIMD Kernel      | | SIMD Kernel      |
| AVX2/NEON/WASM   | | AVX2/NEON/WASM   | | AVX2/NEON/WASM   |
| (Vectorized)     | | (Vectorized)     | | (Vectorized)     |
+------------------+ +------------------+ +------------------+
```

#### Rayon Integration Code

```rust
use rayon::prelude::*;

/// Batch solve: outer parallelism via Rayon, inner via solver scheduler + SIMD
pub fn batch_solve(
    queries: &[SolveRequest],
    solver: &SublinearSolver,
    config: &SolverConfig,
) -> Vec<SolveResult> {
    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        queries
            .par_iter()
            .map(|query| solver.solve_single(query, config))
            .collect()
    }
    #[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
    {
        queries
            .iter()
            .map(|query| solver.solve_single(query, config))
            .collect()
    }
}

/// Multi-query search with solver-accelerated re-ranking
pub fn parallel_search_and_solve(
    queries: &[Vec<f32>],
    index: &HnswIndex,
    solver: &SublinearSolver,
    k: usize,
) -> Vec<Vec<SearchResult>> {
    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        queries
            .par_iter()
            .map(|query| {
                // Phase 1: HNSW candidate retrieval (O(log n))
                let candidates = index.search(query, k * 4).unwrap();
                // Phase 2: Solver-accelerated exact re-ranking (sublinear)
                solver.rerank(query, &candidates, k)
            })
            .collect()
    }
    #[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
    {
        queries
            .iter()
            .map(|query| {
                let candidates = index.search(query, k * 4).unwrap();
                solver.rerank(query, &candidates, k)
            })
            .collect()
    }
}
```

### 2. Solver Scheduler Integration

The solver's nanosecond-precision scheduler operates exclusively within a single Rayon
task. It manages fine-grained intra-solve task decomposition without interacting with
Rayon's thread pool or work-stealing queues.

```
+---------------------------------------------------------------+
|                    Single Rayon Task                            |
|                                                                |
|  +----------------------------------------------------------+ |
|  |              Solver Scheduler (98ns tick)                 | |
|  |                                                           | |
|  |  Task Queue:                                              | |
|  |  [Neumann Term 0] [Neumann Term 1] [Random Walk Step]    | |
|  |  [Matrix Partition A] [Matrix Partition B] [Reduce]       | |
|  |                                                           | |
|  |  Execution Timeline (single thread):                      | |
|  |  |--98ns--|--98ns--|--98ns--|--98ns--|--98ns--|            | |
|  |  | T0    | T1    | T2    | T3    | T0    |  ...           | |
|  |  |  run  |  run  |  run  |  run  | cont  |               | |
|  |                                                           | |
|  |  Scheduler Responsibilities:                              | |
|  |  - Task priority ordering                                 | |
|  |  - Convergence checking per tick                          | |
|  |  - Early termination on epsilon satisfaction              | |
|  |  - Budget enforcement (max ticks per solve)               | |
|  +----------------------------------------------------------+ |
+---------------------------------------------------------------+
```

#### Scheduler Isolation Contract

```rust
/// The solver scheduler runs entirely within a single OS thread.
/// It MUST NOT:
/// - Call rayon::spawn() or any Rayon parallel iterator
/// - Acquire locks held by other Rayon tasks
/// - Block on tokio channels or async primitives
/// - Allocate from the global allocator in the hot loop
///
/// It MAY:
/// - Use thread-local storage for scratch buffers
/// - Use AtomicVectorPool for zero-alloc vector operations
/// - Read from shared immutable data (index, graph adjacency)
/// - Write to its own output buffer (non-shared)
pub struct SolverScheduler {
    /// Task queue using crossbeam for efficient scheduling
    task_queue: crossbeam::deque::Worker<SolverTask>,
    /// Pre-allocated scratch space from AtomicVectorPool
    scratch_pool: AtomicVectorPool,
    /// Tick interval in nanoseconds (default: 98)
    tick_ns: u64,
    /// Maximum ticks per solve invocation
    max_ticks: u64,
    /// Convergence threshold
    epsilon: f64,
}

impl SolverScheduler {
    pub fn new(dimensions: usize, config: &SchedulerConfig) -> Self {
        Self {
            task_queue: crossbeam::deque::Worker::new_fifo(),
            scratch_pool: AtomicVectorPool::new(dimensions, 16, 64),
            tick_ns: config.tick_ns.unwrap_or(98),
            max_ticks: config.max_ticks.unwrap_or(100_000),
            epsilon: config.epsilon.unwrap_or(1e-6),
        }
    }

    /// Execute a solve within the scheduler's tick loop.
    /// This function is called from within a Rayon task and runs
    /// entirely on the calling thread.
    pub fn execute(&self, problem: &SolveProblem) -> SolveResult {
        let mut state = SolverState::init(problem, &self.scratch_pool);
        let mut ticks: u64 = 0;

        loop {
            // Process one tick worth of work
            if let Some(task) = self.task_queue.pop() {
                task.execute(&mut state);
            }

            ticks += 1;

            // Check convergence
            if state.residual() < self.epsilon {
                return SolveResult::converged(state, ticks);
            }

            // Check budget
            if ticks >= self.max_ticks {
                return SolveResult::budget_exhausted(state, ticks);
            }

            // Generate follow-up tasks based on current state
            self.generate_tasks(&state);
        }
    }

    fn generate_tasks(&self, state: &SolverState) {
        // Neumann series: schedule next term if not converged
        if state.needs_next_term() {
            self.task_queue.push(SolverTask::NeumannTerm {
                term_index: state.current_term + 1,
            });
        }
        // Random walk: schedule additional walks if variance too high
        if state.variance_too_high() {
            self.task_queue.push(SolverTask::RandomWalkBatch {
                count: state.walks_needed(),
            });
        }
    }
}
```

### 3. Crossbeam Integration

The solver extends ruvector's existing lock-free infrastructure with two additional
integration points.

#### Work-Stealing Task Queue

The solver's task queue uses `crossbeam::deque::Injector` for work distribution when
operating in multi-solve mode. Each Rayon worker thread owns a local
`crossbeam::deque::Worker` deque; the `Injector` distributes initial tasks, and
`Stealer` handles load balancing across solver instances.

```
+----------------------------------------------------------+
|                   Work-Stealing Topology                  |
|                                                          |
|  +--------------------+                                  |
|  |     Injector       |  <-- batch_solve() pushes here   |
|  | (Global Task Queue)|                                  |
|  +--------+-----------+                                  |
|           |                                              |
|     +-----+------+------+                                |
|     |            |      |                                |
|  +--v---+   +----v-+  +-v----+                           |
|  |Worker|   |Worker|  |Worker|  <-- Rayon thread-local   |
|  |Deque |   |Deque |  |Deque |                           |
|  +--+---+   +--+---+  +--+---+                           |
|     |          |         |                                |
|     v          v         v                                |
|  Steal <----> Steal <--> Steal   (work stealing)         |
+----------------------------------------------------------+
```

```rust
use crossbeam::deque::{Injector, Stealer, Worker};

/// Multi-solve work distributor using crossbeam work-stealing deques.
/// Integrates with Rayon by running within install() scope.
pub struct SolverWorkDistributor {
    injector: Injector<SolveRequest>,
    stealers: Vec<Stealer<SolveRequest>>,
    workers: Vec<Worker<SolveRequest>>,
}

impl SolverWorkDistributor {
    pub fn new(num_workers: usize) -> Self {
        let injector = Injector::new();
        let mut workers = Vec::with_capacity(num_workers);
        let mut stealers = Vec::with_capacity(num_workers);

        for _ in 0..num_workers {
            let worker = Worker::new_fifo();
            stealers.push(worker.stealer());
            workers.push(worker);
        }

        Self { injector, stealers, workers }
    }

    /// Submit a batch of solve requests for parallel execution.
    pub fn submit_batch(&self, requests: Vec<SolveRequest>) {
        for request in requests {
            self.injector.push(request);
        }
    }

    /// Attempt to steal work. Called by idle Rayon threads.
    pub fn find_work(&self, worker_id: usize) -> Option<SolveRequest> {
        // Try local deque first
        let local = &self.workers[worker_id];
        if let Some(task) = local.pop() {
            return Some(task);
        }

        // Try global injector
        loop {
            match self.injector.steal_batch_and_pop(local) {
                crossbeam::deque::Steal::Success(task) => return Some(task),
                crossbeam::deque::Steal::Empty => break,
                crossbeam::deque::Steal::Retry => continue,
            }
        }

        // Try stealing from other workers
        for (i, stealer) in self.stealers.iter().enumerate() {
            if i == worker_id { continue; }
            loop {
                match stealer.steal() {
                    crossbeam::deque::Steal::Success(task) => return Some(task),
                    crossbeam::deque::Steal::Empty => break,
                    crossbeam::deque::Steal::Retry => continue,
                }
            }
        }

        None
    }
}
```

#### Lock-Free Statistics

Solver statistics collection extends the existing `LockFreeStats` pattern from
`crates/ruvector-core/src/lockfree.rs`, using `CachePadded<AtomicU64>` for contention-free
updates across Rayon worker threads.

```rust
use crossbeam::utils::CachePadded;
use std::sync::atomic::{AtomicU64, Ordering};

/// Lock-free solver statistics, following ruvector's LockFreeStats pattern.
/// Each field is cache-line padded to prevent false sharing between threads.
#[repr(align(64))]
pub struct SolverStats {
    solves_completed: CachePadded<AtomicU64>,
    total_ticks: CachePadded<AtomicU64>,
    convergences: CachePadded<AtomicU64>,
    budget_exhaustions: CachePadded<AtomicU64>,
    total_latency_ns: CachePadded<AtomicU64>,
    neumann_terms_computed: CachePadded<AtomicU64>,
    random_walks_executed: CachePadded<AtomicU64>,
}

impl SolverStats {
    pub fn new() -> Self {
        Self {
            solves_completed: CachePadded::new(AtomicU64::new(0)),
            total_ticks: CachePadded::new(AtomicU64::new(0)),
            convergences: CachePadded::new(AtomicU64::new(0)),
            budget_exhaustions: CachePadded::new(AtomicU64::new(0)),
            total_latency_ns: CachePadded::new(AtomicU64::new(0)),
            neumann_terms_computed: CachePadded::new(AtomicU64::new(0)),
            random_walks_executed: CachePadded::new(AtomicU64::new(0)),
        }
    }

    #[inline]
    pub fn record_solve(&self, result: &SolveResult) {
        self.solves_completed.fetch_add(1, Ordering::Relaxed);
        self.total_ticks.fetch_add(result.ticks, Ordering::Relaxed);
        self.total_latency_ns.fetch_add(result.latency_ns, Ordering::Relaxed);

        if result.converged {
            self.convergences.fetch_add(1, Ordering::Relaxed);
        } else {
            self.budget_exhaustions.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn snapshot(&self) -> SolverStatsSnapshot {
        let completed = self.solves_completed.load(Ordering::Relaxed);
        let total_latency = self.total_latency_ns.load(Ordering::Relaxed);
        let total_ticks = self.total_ticks.load(Ordering::Relaxed);

        SolverStatsSnapshot {
            solves_completed: completed,
            convergence_rate: if completed > 0 {
                self.convergences.load(Ordering::Relaxed) as f64 / completed as f64
            } else { 0.0 },
            avg_latency_ns: if completed > 0 {
                total_latency / completed
            } else { 0 },
            avg_ticks_per_solve: if completed > 0 {
                total_ticks / completed
            } else { 0 },
            neumann_terms: self.neumann_terms_computed.load(Ordering::Relaxed),
            random_walks: self.random_walks_executed.load(Ordering::Relaxed),
        }
    }
}
```

### 4. WASM Parallelism

WASM targets lack native thread support. The solver's WASM binding follows the established
ruvector pattern: a `WorkerPool` (JavaScript) manages Web Workers, each running an
independent WASM solver instance.

```
+---------------------------------------------------------------+
|                    Browser Main Thread                          |
|                                                                |
|  +----------------------------------------------------------+ |
|  |  SolverWorkerPool (extends WorkerPool pattern)            | |
|  |                                                           | |
|  |  init() -> spawns N Web Workers (navigator.hardwareConcurrency) |
|  |  solve(request) -> round-robin to next idle worker        | |
|  |  batchSolve(requests) -> chunk and distribute             | |
|  |  terminate() -> cleanup all workers                       | |
|  +--+--------+--------+--------+----------------------------+ |
|     |        |        |        |                               |
+-----+--------+--------+--------+-------------------------------+
      |        |        |        |
      v        v        v        v        postMessage / onmessage
+--------+ +--------+ +--------+ +--------+
|Worker 0| |Worker 1| |Worker 2| |Worker 3|
|        | |        | |        | |        |
|+------+| |+------+| |+------+| |+------+|
||WASM  || ||WASM  || ||WASM  || ||WASM  ||
||Solver|| ||Solver|| ||Solver|| ||Solver||
||Module|| ||Module|| ||Module|| ||Module||
|+------+| |+------+| |+------+| |+------+|
|        | |        | |        | |        |
|Scheduler| |Scheduler| |Scheduler| |Scheduler|
|(98ns)  | |(98ns)  | |(98ns)  | |(98ns)  |
+--------+ +--------+ +--------+ +--------+
     |          |          |          |
     v          v          v          v
  [WASM SIMD] [WASM SIMD] [WASM SIMD] [WASM SIMD]
  (128-bit)   (128-bit)   (128-bit)   (128-bit)
```

#### SharedArrayBuffer Zero-Copy Path

When `SharedArrayBuffer` is available (requires `Cross-Origin-Isolation` headers:
`Cross-Origin-Opener-Policy: same-origin` and
`Cross-Origin-Embedder-Policy: require-corp`), the solver uses shared memory for
zero-copy data transfer between workers.

```javascript
/**
 * Solver Worker Pool -- extends ruvector's WorkerPool pattern
 * with SharedArrayBuffer support and solver-specific operations.
 */
export class SolverWorkerPool {
  constructor(workerUrl, wasmUrl, options = {}) {
    this.workerUrl = workerUrl;
    this.wasmUrl = wasmUrl;
    this.poolSize = options.poolSize || navigator.hardwareConcurrency || 4;
    this.workers = [];
    this.nextWorker = 0;
    this.pendingRequests = new Map();
    this.requestId = 0;
    this.initialized = false;
    this.sharedMemoryAvailable = typeof SharedArrayBuffer !== 'undefined';
    this.sharedBuffer = null;
  }

  async init() {
    if (this.initialized) return;

    // Allocate shared memory if available
    if (this.sharedMemoryAvailable) {
      const bufferSize = this.poolSize * 4 * 1024 * 1024; // 4MB per worker
      this.sharedBuffer = new SharedArrayBuffer(bufferSize);
      console.log(`SharedArrayBuffer allocated: ${bufferSize} bytes`);
    }

    const initPromises = [];
    for (let i = 0; i < this.poolSize; i++) {
      const worker = new Worker(this.workerUrl, { type: 'module' });
      worker.onmessage = (e) => this.handleMessage(i, e);
      worker.onerror = (error) => this.handleError(i, error);

      this.workers.push({ worker, busy: false, id: i });

      const initData = {
        wasmUrl: this.wasmUrl,
        workerId: i,
        sharedBuffer: this.sharedBuffer,
        bufferOffset: i * 4 * 1024 * 1024,
        bufferSize: 4 * 1024 * 1024,
      };
      initPromises.push(this.sendToWorker(i, 'init', initData));
    }

    await Promise.all(initPromises);
    this.initialized = true;
  }

  /**
   * Solve a single request on the next available worker.
   */
  async solve(request) {
    if (!this.initialized) await this.init();
    const workerId = this.getNextWorker();

    if (this.sharedMemoryAvailable) {
      // Zero-copy path: write input to shared buffer
      const view = new Float32Array(
        this.sharedBuffer,
        workerId * 4 * 1024 * 1024,
        request.data.length
      );
      view.set(request.data);
      return this.sendToWorker(workerId, 'solveShared', {
        offset: 0,
        length: request.data.length,
        config: request.config,
      });
    } else {
      // Transferable path: transfer ownership of ArrayBuffer
      const buffer = new Float32Array(request.data).buffer;
      return this.sendToWorkerTransfer(workerId, 'solve', {
        config: request.config,
      }, [buffer]);
    }
  }

  /**
   * Batch solve distributes requests across all workers.
   */
  async batchSolve(requests) {
    if (!this.initialized) await this.init();

    const chunkSize = Math.ceil(requests.length / this.poolSize);
    const chunks = [];
    for (let i = 0; i < requests.length; i += chunkSize) {
      chunks.push(requests.slice(i, i + chunkSize));
    }

    const promises = chunks.map((chunk, i) =>
      this.sendToWorker(i % this.poolSize, 'batchSolve', {
        requests: chunk.map(r => ({
          data: Array.from(r.data),
          config: r.config,
        })),
      })
    );

    const results = await Promise.all(promises);
    return results.flat();
  }

  // -- Utility methods following WorkerPool pattern --

  getNextWorker() {
    for (let i = 0; i < this.workers.length; i++) {
      const idx = (this.nextWorker + i) % this.workers.length;
      if (!this.workers[idx].busy) {
        this.nextWorker = (idx + 1) % this.workers.length;
        return idx;
      }
    }
    const idx = this.nextWorker;
    this.nextWorker = (this.nextWorker + 1) % this.workers.length;
    return idx;
  }

  handleMessage(workerId, event) {
    const { requestId, data, error } = event.data;
    const request = this.pendingRequests.get(requestId);
    if (!request) return;

    this.workers[workerId].busy = false;
    this.pendingRequests.delete(requestId);

    if (error) {
      request.reject(new Error(error.message));
    } else {
      request.resolve(data);
    }
  }

  handleError(workerId, error) {
    console.error(`Solver worker ${workerId} error:`, error);
    for (const [requestId, request] of this.pendingRequests) {
      if (request.workerId === workerId) {
        request.reject(error);
        this.pendingRequests.delete(requestId);
      }
    }
  }

  sendToWorker(workerId, type, data) {
    return new Promise((resolve, reject) => {
      const requestId = this.requestId++;
      this.pendingRequests.set(requestId, { resolve, reject, workerId });
      this.workers[workerId].busy = true;
      this.workers[workerId].worker.postMessage({
        type, data: { ...data, requestId },
      });
    });
  }

  sendToWorkerTransfer(workerId, type, data, transferables) {
    return new Promise((resolve, reject) => {
      const requestId = this.requestId++;
      this.pendingRequests.set(requestId, { resolve, reject, workerId });
      this.workers[workerId].busy = true;
      this.workers[workerId].worker.postMessage(
        { type, data: { ...data, requestId } },
        transferables
      );
    });
  }

  terminate() {
    for (const { worker } of this.workers) {
      worker.terminate();
    }
    this.workers = [];
    this.initialized = false;
    this.sharedBuffer = null;
  }
}
```

#### Fallback Path (No SharedArrayBuffer)

When `SharedArrayBuffer` is unavailable (non-isolated context, older browsers, iOS Safari
prior to 16.4), the solver falls back to `postMessage` with transferable `ArrayBuffer`
objects. Ownership of the buffer transfers to the worker, avoiding copies at the cost of
making the source buffer neutered (zero-length) after transfer.

```
SharedArrayBuffer available?
        |
   +----+----+
   |         |
  YES        NO
   |         |
   v         v
Zero-copy    Transferable ArrayBuffer
shared       (ownership transfer,
memory       source buffer neutered)
   |         |
   v         v
Worker reads Worker receives
from shared  full copy via
view at      structured clone
offset       of postMessage
```

### 5. Async Integration

The solver is CPU-bound and must not block the Tokio async runtime. All solver invocations
from async contexts use `tokio::task::spawn_blocking`, following the pattern established
in `ruvector-attention-node`, `ruvector-graph-node`, and `ruvector-router-ffi`.

```rust
use tokio::sync::broadcast;
use tokio::task;

/// Async wrapper for solver operations.
/// Uses spawn_blocking to avoid blocking the Tokio runtime.
pub struct AsyncSolver {
    solver: Arc<SublinearSolver>,
    event_tx: broadcast::Sender<SolverEvent>,
    semaphore: Arc<tokio::sync::Semaphore>,
}

/// Events emitted during solver operations for observability.
#[derive(Clone, Debug)]
pub enum SolverEvent {
    SolveStarted { request_id: u64 },
    SolveCompleted { request_id: u64, latency_ns: u64, converged: bool },
    SolveFailed { request_id: u64, error: String },
    BatchStarted { batch_id: u64, count: usize },
    BatchCompleted { batch_id: u64, count: usize, total_latency_ns: u64 },
    ConcurrencyLimitReached { current: usize, max: usize },
}

impl AsyncSolver {
    pub fn new(solver: SublinearSolver, max_concurrent: usize) -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        Self {
            solver: Arc::new(solver),
            event_tx,
            semaphore: Arc::new(tokio::sync::Semaphore::new(max_concurrent)),
        }
    }

    /// Subscribe to solver events for monitoring/observability.
    pub fn subscribe(&self) -> broadcast::Receiver<SolverEvent> {
        self.event_tx.subscribe()
    }

    /// Solve a single request asynchronously.
    /// Acquires a semaphore permit to enforce concurrency limits.
    pub async fn solve(&self, request: SolveRequest) -> Result<SolveResult, SolverError> {
        let permit = self.semaphore.acquire().await
            .map_err(|_| SolverError::Shutdown)?;

        let solver = Arc::clone(&self.solver);
        let event_tx = self.event_tx.clone();
        let request_id = request.id;

        let _ = event_tx.send(SolverEvent::SolveStarted { request_id });

        let result = task::spawn_blocking(move || {
            let start = std::time::Instant::now();
            let result = solver.solve_single(&request, &SolverConfig::default());
            let latency_ns = start.elapsed().as_nanos() as u64;

            let _ = event_tx.send(SolverEvent::SolveCompleted {
                request_id,
                latency_ns,
                converged: result.converged,
            });

            result
        })
        .await
        .map_err(|e| SolverError::TaskPanicked(e.to_string()))?;

        drop(permit);
        Ok(result)
    }

    /// Batch solve with Rayon parallelism inside spawn_blocking.
    pub async fn batch_solve(
        &self,
        requests: Vec<SolveRequest>,
    ) -> Result<Vec<SolveResult>, SolverError> {
        let solver = Arc::clone(&self.solver);
        let event_tx = self.event_tx.clone();
        let batch_id = requests.first().map(|r| r.id).unwrap_or(0);
        let count = requests.len();

        let _ = event_tx.send(SolverEvent::BatchStarted { batch_id, count });

        let results = task::spawn_blocking(move || {
            let start = std::time::Instant::now();
            let results = batch_solve(&requests, &solver, &SolverConfig::default());
            let total_latency_ns = start.elapsed().as_nanos() as u64;

            let _ = event_tx.send(SolverEvent::BatchCompleted {
                batch_id, count, total_latency_ns,
            });

            results
        })
        .await
        .map_err(|e| SolverError::TaskPanicked(e.to_string()))?;

        Ok(results)
    }
}
```

#### Event Stream Architecture

```
+------------------+     +------------------+     +------------------+
| AsyncSolver      |     | Event Subscriber |     | Event Subscriber |
| (spawn_blocking) |     | (Metrics)        |     | (Logging)        |
+--------+---------+     +--------+---------+     +--------+---------+
         |                        |                        |
         v                        v                        v
  +------+------------------------+------------------------+------+
  |                    broadcast::channel(1024)                    |
  |                                                                |
  |  SolverEvent::SolveStarted { request_id }                     |
  |  SolverEvent::SolveCompleted { request_id, latency_ns, ... }  |
  |  SolverEvent::BatchCompleted { batch_id, count, ... }         |
  +----------------------------------------------------------------+
```

### 6. Concurrent Solve Limit

A bounded `tokio::sync::Semaphore` limits the number of solver invocations executing
simultaneously. This prevents memory exhaustion when many async callers submit concurrent
solve requests, each of which allocates scratch buffers from `AtomicVectorPool`.

**Default limit**: `num_cpus::get() * 2`

This follows the pattern used in `ruvector-postgres` (`max_concurrent_searches: 64`) and
`prime-radiant` (`max_concurrent_ops`), but tunes for CPU-bound solver work rather than
I/O-bound database queries.

```rust
/// Configuration for concurrent solve limits.
pub struct ConcurrencyConfig {
    /// Maximum concurrent solver invocations.
    /// Default: num_cpus * 2 (allows slight oversubscription for
    /// work that mixes solver compute with I/O waits).
    pub max_concurrent_solves: usize,

    /// Maximum total memory allocated to solver scratch space.
    /// Enforced via AtomicVectorPool capacity.
    pub max_scratch_memory_bytes: usize,

    /// Backpressure strategy when limit is reached.
    pub backpressure: BackpressureStrategy,
}

#[derive(Debug, Clone)]
pub enum BackpressureStrategy {
    /// Block until a permit becomes available (default).
    Wait,
    /// Return an error immediately.
    Reject,
    /// Wait up to the specified duration, then return an error.
    WaitTimeout(std::time::Duration),
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        let cpus = num_cpus::get();
        Self {
            max_concurrent_solves: cpus * 2,
            max_scratch_memory_bytes: cpus * 16 * 1024 * 1024, // 16MB per CPU
            backpressure: BackpressureStrategy::Wait,
        }
    }
}
```

---

## Consequences

### Positive

- **No thread pool exhaustion**: Two-level separation ensures Rayon's global pool is never
  starved by nested parallelism. Each solve invocation consumes exactly one Rayon thread.
- **Predictable memory usage**: The semaphore-bounded concurrency limit combined with
  `AtomicVectorPool` capacity bounds prevents out-of-memory conditions under load.
- **WASM compatibility**: The Web Worker pattern matches ruvector's existing 27 WASM crates
  and their `WorkerPool` infrastructure. No new paradigm to learn or maintain.
- **Observable**: The `broadcast::channel` event stream provides real-time solve metrics
  without polling, enabling integration with ruvector's existing `tracing` infrastructure.
- **Zero contention statistics**: Cache-padded atomics in `SolverStats` eliminate false
  sharing. Measurements from `LockFreeCounter` tests confirm 10K increments across 10
  threads with zero lost updates.
- **Lock-free hot path**: The solver's inner loop uses `AtomicVectorPool` (SegQueue-backed)
  and `crossbeam::deque::Worker` (thread-local). No mutex or RwLock on the critical path.
- **Crossbeam alignment**: Work-stealing deques integrate naturally with Rayon's existing
  work-stealing model since Rayon itself uses crossbeam internally.

### Negative

- **Scheduler complexity**: The 98ns tick scheduler requires careful calibration per
  platform. Timer resolution on Linux (`clock_gettime(CLOCK_MONOTONIC)`) provides ~25ns
  precision, but macOS and Windows have coarser clocks (~1us).
- **WASM overhead**: Web Worker `postMessage` serialization adds ~50-200us per invocation,
  dwarfing the solver's sub-microsecond tick cost. Batch operations are essential to
  amortize this overhead.
- **SharedArrayBuffer restrictions**: Cross-origin isolation headers (`COOP`/`COEP`) break
  third-party integrations (e.g., iframes, analytics scripts). Deployments must weigh
  zero-copy benefits against integration constraints.
- **Memory bandwidth ceiling**: Thread scaling beyond 4 threads provides diminishing
  returns (50-70% efficiency at 8 threads). The solver cannot overcome this hardware
  limitation; it can only work within it by maximizing SIMD utilization per thread.
- **Rayon global pool coupling**: Solver batch operations share Rayon's global thread pool
  with other subsystems (`batch_distances`, `FlatIndex::search`, delta operations). Under
  contention, solver tasks compete with index operations for threads.

### Neutral

- **Feature flag gating**: Parallelism continues to be gated behind
  `#[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]`, consistent with
  existing crate convention. No new feature flags required.
- **Tokio spawn_blocking pool**: `spawn_blocking` uses a separate thread pool from Rayon.
  Default pool size is 512 threads (Tokio default), which is more than sufficient for
  `max_concurrent_solves = num_cpus * 2`.
- **Crossbeam version alignment**: Both ruvector (0.8) and the solver target crossbeam 0.8.
  No version conflict.

---

## Options Considered

### Option 1: Rayon-Only Parallelism (Flat Model)

Use Rayon `par_iter()` for all parallelism, including intra-solve task decomposition.

- **Pros**: Simplest implementation. Single threading model. No scheduler complexity.
  Rayon's adaptive work stealing handles load balancing automatically.
- **Cons**: Nested `par_iter()` calls risk thread pool exhaustion. Rayon's work-stealing
  granularity (~1us minimum) is too coarse for the solver's 98ns tick. Cannot express
  solver-specific scheduling policies (convergence checking, budget enforcement) within
  Rayon's map/reduce model. Forces all solver tasks through Rayon's global deque, adding
  contention with other subsystems.

### Option 2: Dedicated Thread Pool per Solver

Create a separate `rayon::ThreadPool` for solver operations, isolated from the global pool.

- **Pros**: Complete isolation from other subsystems. No contention with index operations.
  Can size the pool independently (e.g., 4 threads for solver, remaining for index).
- **Cons**: Thread oversubscription: a 4-thread solver pool plus Rayon's global pool (8
  threads by default) creates 12 threads on an 8-core machine. Context switching overhead
  negates throughput gains. Memory bandwidth is the bottleneck, not thread count; more
  threads do not help. Cannot share work between pools when one is idle. Increases total
  memory consumption (each pool maintains its own deque infrastructure).

### Option 3: Tokio-Native Async Solve (Chosen Against)

Run the solver on the Tokio runtime using `tokio::task::spawn` with cooperative yielding.

- **Pros**: Native async integration. No spawn_blocking overhead. Natural backpressure
  via Tokio's task budget system.
- **Cons**: The solver is CPU-bound; running on Tokio's cooperative scheduler would block
  other async tasks. Tokio's tick granularity (~10us) is 100x coarser than the solver's
  98ns target. Would require `.await` points in inner loops, destroying SIMD pipeline
  throughput. Fundamentally wrong tool for CPU-bound computation.

### Option 4: Two-Level with Scheduler (Chosen)

Rayon for outer-level batch parallelism; solver's own nanosecond scheduler for inner-level
task management; crossbeam for lock-free data structures; Tokio integration via
spawn_blocking.

- **Pros**: Matches ruvector's existing patterns exactly. Respects memory bandwidth limits
  by not oversubscribing threads. Preserves solver's 98ns scheduling precision. Lock-free
  hot path. Observable via broadcast channels. WASM-compatible via established WorkerPool.
- **Cons**: Two scheduling systems to understand and maintain. Potential for subtle bugs
  at the Rayon/scheduler boundary (e.g., accidentally calling par_iter inside a solve).

---

## Thread Scaling Analysis

### Memory Bandwidth Model

The dominant performance constraint for vectorized solver workloads is memory bandwidth,
not CPU compute. The following model explains the measured scaling behavior.

```
Memory Bandwidth Utilization vs Thread Count
(10K vectors x 384 dimensions x 4 bytes = 15.36 MB working set)

Threads  | BW Used (GB/s) | BW Available | Efficiency | Bottleneck
---------|----------------|--------------|------------|------------------
1        | ~8             | ~50 (DDR5)   | 16%        | CPU-bound (good)
2        | ~15            | ~50          | 30%        | CPU-bound (good)
4        | ~28            | ~50          | 56%        | Approaching BW limit
8        | ~38            | ~50          | 76%        | BW-saturated
16       | ~42            | ~50          | 84%        | Diminishing returns

Note: Effective BW is lower than theoretical peak due to cache coherence
traffic (MESI protocol overhead) and TLB pressure at scale.
```

### Solver-Specific Scaling Predictions

The solver's workload differs from pure distance computation in two important ways:

1. **Higher arithmetic intensity**: Neumann series evaluation performs multiple matrix-vector
   products per memory access, increasing the compute-to-bandwidth ratio. This should
   improve scaling beyond 4 threads relative to simple distance computation.

2. **Smaller working sets per solve**: Individual solve problems typically involve matrices
   of size 128x128 to 1024x1024 (~64KB to ~4MB), fitting within L2/L3 cache. This reduces
   memory bandwidth pressure compared to scanning 10K vectors.

```
Predicted Solver Thread Scaling:

Threads  | Distance Comp  | Neumann Solve  | Random Walk
         | (measured)     | (predicted)    | (predicted)
---------|----------------|----------------|----------------
1        | 100%           | 100%           | 100%
2        | 85-95%         | 90-97%         | 92-98%
4        | 70-85%         | 80-92%         | 85-95%
8        | 50-70%         | 65-82%         | 70-85%

Neumann: Higher arithmetic intensity -> better scaling
Random Walk: Independent walks -> near-linear scaling
```

### Avoiding Nested Parallelism

Nested Rayon parallelism is the single most dangerous anti-pattern for this integration.
The following diagram illustrates the deadlock scenario.

```
DANGEROUS (do NOT do this):

  batch_solve()
    .par_iter()           <-- Rayon global pool (8 threads)
      |
      +-> solve_single()
            .par_iter()   <-- NESTED: tries to use same 8 threads
              |
              +-> [DEADLOCK: all 8 threads waiting for inner
                   par_iter work, but no threads available
                   to execute it]

CORRECT (this ADR's approach):

  batch_solve()
    .par_iter()           <-- Rayon global pool (8 threads)
      |
      +-> solve_single()
            SolverScheduler::execute()  <-- Sequential on calling thread
              |
              +-> SIMD kernel           <-- Vectorized, single thread
              +-> crossbeam::deque      <-- Thread-local, no contention
```

---

## Lock-Free Data Structure Usage Map

The following table maps each lock-free structure in the codebase to its role in the
solver integration.

| Structure | Source | Solver Role | Contention Profile |
|-----------|--------|-------------|-------------------|
| `AtomicVectorPool` | `lockfree.rs` | Scratch buffer allocation for Neumann terms, walk accumulators | Per-thread pool instance; no cross-thread contention |
| `LockFreeWorkQueue` | `lockfree.rs` | Bounded result collection from parallel batch_solve | Producers: Rayon threads; Consumer: batch_solve caller |
| `LockFreeStats` | `lockfree.rs` | Pattern template for `SolverStats` | Write-only from Rayon threads; Read from monitoring |
| `LockFreeCounter` | `lockfree.rs` | Request ID generation, tick counting | Single atomic increment per solve; negligible contention |
| `LockFreeBatchProcessor` | `lockfree.rs` | Orchestration of multi-phase solve pipelines | Phase transitions; moderate contention at boundaries |
| `ObjectPool<T>` | `lockfree.rs` | Reusable solver state objects | Per-thread acquire/release; low contention |
| `crossbeam::deque::Worker` | crossbeam | Solver scheduler task queue | Thread-local; zero contention |
| `crossbeam::deque::Injector` | crossbeam | Batch task distribution | Single producer (submit), multiple consumers (steal) |
| `DashMap` | dashmap | Solver result cache (optional) | Read-heavy; sharded RwLock minimizes contention |

---

## Benchmark Predictions for Parallel Solver

Based on existing ruvector benchmark data and the solver's algorithmic properties.

### Single-Threaded Solver Performance

| Operation | Estimated Latency | Basis |
|-----------|------------------|-------|
| Neumann solve (128x128 sparse) | 2-5 us | 3-5 terms x 500ns matmul |
| Neumann solve (1024x1024 sparse) | 20-80 us | 5-8 terms x 5us matmul |
| Random walk estimate (single entry) | 0.5-2 us | 10-50 steps x 50ns/step |
| Scheduler overhead per solve | 50-200 ns | 1-2 ticks of bookkeeping |
| AtomicVectorPool acquire/release | 15-30 ns | SegQueue push/pop |

### Parallel Batch Performance (8 threads)

| Operation | 1 Thread | 8 Threads | Speedup | Efficiency |
|-----------|----------|-----------|---------|------------|
| 1000x Neumann (128x128) | 3.5 ms | 0.55 ms | 6.4x | 80% |
| 1000x Neumann (1024x1024) | 50 ms | 8.5 ms | 5.9x | 74% |
| 10000x Random walk | 12 ms | 1.8 ms | 6.7x | 84% |
| Mixed batch (Neumann + walk) | 30 ms | 5.2 ms | 5.8x | 72% |

### WASM Performance (4 Web Workers)

| Operation | 1 Worker | 4 Workers | Speedup | Overhead |
|-----------|----------|-----------|---------|----------|
| 100x Neumann (128x128) | 0.8 ms | 0.25 ms | 3.2x | ~100us postMessage |
| 100x Neumann (SAB zero-copy) | 0.8 ms | 0.22 ms | 3.6x | ~10us shared read |
| WASM SIMD vs scalar | - | - | 2-4x | Per-operation |

---

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-20 | RuVector Team | Initial proposal |
| 1.0 | 2026-02-20 | RuVector Team | Accepted: full implementation complete |

---

## Implementation Status

Parallel feature gate enables rayon for data-parallel SpMV and crossbeam for concurrent solver pipeline. Thread-safe types via Send + Sync bounds. DashMap for concurrent solver registry. parking_lot for low-overhead locking. Arena allocator is thread-local. ComputeBudget checked atomically. WASM target uses single-threaded fallback.

---

## Related Decisions

- **ADR-STS-001**: Rust crates integration (establishes the `parallel` feature flag
  convention that this ADR extends)
- **ADR-STS-005**: Architecture analysis (defines the Core-Binding-Surface pattern that
  the async wrapper follows)
- **ADR-STS-006**: WASM integration (defines Web Worker and SharedArrayBuffer patterns
  that this ADR's WASM parallelism builds on)
- **ADR-STS-008**: Performance analysis (provides the thread scaling measurements and
  benchmark framework referenced throughout)
- **ADR-003**: SIMD optimization strategy (defines the SIMD vectorization approach used
  in the solver's inner level)

## References

- [Rayon documentation: Thread pool configuration](https://docs.rs/rayon/latest/rayon/struct.ThreadPoolBuilder.html)
- [Crossbeam deque: Work-stealing](https://docs.rs/crossbeam-deque/latest/crossbeam_deque/)
- [Tokio: spawn_blocking for CPU-bound work](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)
- [MDN: SharedArrayBuffer and Cross-Origin Isolation](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/SharedArrayBuffer)
- [ruvector lockfree.rs](/home/user/ruvector/crates/ruvector-core/src/lockfree.rs)
- [ruvector worker-pool.js](/home/user/ruvector/crates/ruvector-wasm/src/worker-pool.js)
- [ruvector comprehensive_bench.rs](/home/user/ruvector/crates/ruvector-core/benches/comprehensive_bench.rs)
- [ruvector distance.rs batch_distances()](/home/user/ruvector/crates/ruvector-core/src/distance.rs)
