# System Architecture Plan - Sublinear Time Solver

## 1. System Overview

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Sublinear Time Solver System                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐         │
│  │   CLI Module    │    │  HTTP Server    │    │  WASM Interface │         │
│  │   (Node.js)     │    │   (Node.js)     │    │   (Browser)     │         │
│  └─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘         │
│            │                      │                      │                 │
│            └──────────────────────┼──────────────────────┘                 │
│                                   │                                        │
│  ┌─────────────────────────────────┼─────────────────────────────────────┐  │
│  │              Rust Core Library  │                                     │  │
│  │                                 ▼                                     │  │
│  │  ┌─────────────────────────────────────────────────────────────────┐  │  │
│  │  │                    Unified Solver API                          │  │  │
│  │  │              SolverAlgorithm Trait + Solver Struct             │  │  │
│  │  └─────────────────────┬───────────────────────────────────────────┘  │  │
│  │                        │                                              │  │
│  │  ┌─────────────────────┼─────────────────────────────────────────────┐  │  │
│  │  │            Core Algorithm Modules                               │  │  │
│  │  │                     │                                           │  │  │
│  │  │  ┌─────────────┐   ┌┴────────────┐   ┌─────────────────┐       │  │  │
│  │  │  │ Neumann     │   │ Forward     │   │ Backward        │       │  │  │
│  │  │  │ Series      │   │ Push        │   │ Push            │       │  │  │
│  │  │  │ Solver      │   │ Solver      │   │ Solver          │       │  │  │
│  │  │  └─────────────┘   └─────────────┘   └─────────────────┘       │  │  │
│  │  │                                                                 │  │  │
│  │  │  ┌─────────────────────────────┐   ┌─────────────────────────┐  │  │  │
│  │  │  │      Hybrid Random-Walk     │   │    Verification         │  │  │  │
│  │  │  │      Solver                 │   │    Module               │  │  │  │
│  │  │  └─────────────────────────────┘   └─────────────────────────┘  │  │  │
│  │  └─────────────────────────────────────────────────────────────────┘  │  │
│  │                                                                        │  │
│  │  ┌─────────────────────────────────────────────────────────────────┐  │  │
│  │  │                   Matrix & Linear Algebra                       │  │  │
│  │  │                                                                 │  │  │
│  │  │  ┌─────────────────┐  ┌─────────────────┐  ┌───────────────┐   │  │  │
│  │  │  │ Sparse Matrix   │  │ Dense Vector    │  │ Graph         │   │  │  │
│  │  │  │ (CSR/CSC)       │  │ Operations      │  │ Adjacency     │   │  │  │
│  │  │  └─────────────────┘  └─────────────────┘  └───────────────┘   │  │  │
│  │  └─────────────────────────────────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────────┐  │
│  │                         External Integrations                          │  │
│  │                                                                         │  │
│  │  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐     │  │
│  │  │   Flow-Nexus    │    │      npm        │    │  WebAssembly    │     │  │
│  │  │   Streaming     │    │   Package       │    │   Runtime       │     │  │
│  │  │   HTTP API      │    │  Distribution   │    │   Environment   │     │  │
│  │  └─────────────────┘    └─────────────────┘    └─────────────────┘     │  │
│  └─────────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Component Relationships & Data Flow

```
Data Flow Diagram:

[Input Matrix A] ──┐
                   ├──► [Matrix Module] ──► [Solver Algorithm] ──► [Solution x]
[Input Vector b] ──┘                            │                      │
                                                ▼                      ▼
[Updates Δb]   ────► [Incremental Update] ──► [Verification] ──► [Streamed Results]
                                                │
                                                ▼
                                        [Error Bounds Check]
```

### Technology Stack Decisions

| Layer | Technology | Rationale |
|-------|------------|-----------|
| **Core Algorithms** | Rust | Memory safety, performance, zero-cost abstractions |
| **WASM Runtime** | wasm-bindgen + wasm-pack | Standard Rust-to-WASM toolchain, type safety |
| **HTTP Server** | Node.js + Express | Async I/O, streaming support, Flow-Nexus compatibility |
| **CLI Interface** | Node.js wrapper | Cross-platform, npm ecosystem integration |
| **Linear Algebra** | Custom sparse structures | Optimized for ADD systems, graph-aware |
| **Streaming Protocol** | HTTP Chunked + SSE | Real-time updates, Flow-Nexus integration |
| **Package Distribution** | npm | Universal JS/Node.js ecosystem access |

## 2. Module Architecture

### Core Rust Module Structure

```
src/
├── lib.rs                    # Main library entry point
├── matrix/
│   ├── mod.rs               # Matrix module public interface
│   ├── sparse.rs            # CSR/CSC sparse matrix implementations
│   ├── dense.rs             # Dense vector operations
│   ├── graph.rs             # Graph adjacency list structures
│   └── operations.rs        # Matrix-vector operations
├── solver/
│   ├── mod.rs               # Solver trait definitions
│   ├── neumann.rs           # Neumann series implementation
│   ├── forward_push.rs      # Forward push algorithm
│   ├── backward_push.rs     # Backward push algorithm
│   ├── hybrid.rs            # Hybrid random-walk solver
│   └── common.rs            # Shared solver utilities
├── verification/
│   ├── mod.rs               # Verification module interface
│   ├── residual.rs          # Residual computation and checking
│   ├── random_probe.rs      # Random solution verification
│   └── bounds.rs            # Error bounds analysis
├── wasm_iface/
│   ├── mod.rs               # WASM bindings module
│   ├── solver_api.rs        # JavaScript-exposed solver API
│   ├── streaming.rs         # Streaming solution interface
│   └── memory.rs            # WASM memory management
├── http_server/
│   ├── mod.rs               # HTTP server data models
│   ├── requests.rs          # Request/response schemas
│   └── streaming.rs         # Server-side streaming utilities
└── cli/
    ├── mod.rs               # CLI module (feature-gated)
    ├── args.rs              # Command-line argument parsing
    └── server.rs            # HTTP server launcher
```

### Detailed Module Interfaces

#### 1. Matrix Module Interface

```rust
// matrix/mod.rs
pub trait Matrix {
    fn rows(&self) -> usize;
    fn cols(&self) -> usize;
    fn get(&self, row: usize, col: usize) -> Option<f64>;
    fn row_iter(&self, row: usize) -> impl Iterator<Item = (usize, f64)>;
    fn col_iter(&self, col: usize) -> impl Iterator<Item = (usize, f64)>;
    fn multiply_vector(&self, x: &[f64], result: &mut [f64]);
    fn is_diagonally_dominant(&self) -> bool;
}

pub struct SparseMatrix {
    pub format: SparseFormat,  // CSR, CSC, or COO
    pub values: Vec<f64>,
    pub indices: Vec<usize>,
    pub row_ptr: Vec<usize>,   // For CSR
    pub col_ptr: Vec<usize>,   // For CSC
    pub rows: usize,
    pub cols: usize,
}

pub struct GraphMatrix {
    pub adjacency: Vec<Vec<(usize, f64)>>,  // Adjacency list representation
    pub degrees: Vec<f64>,                   // Node degrees for normalization
    pub nodes: usize,
}
```

#### 2. Solver Module Interface

```rust
// solver/mod.rs
pub trait SolverAlgorithm {
    fn solve(&self, matrix: &dyn Matrix, b: &[f64], opts: &SolverOptions) -> SolverResult;
    fn solve_streaming(&self, matrix: &dyn Matrix, b: &[f64], opts: &SolverOptions)
        -> impl Iterator<Item = PartialSolution>;
    fn update_solution(&self, state: &mut SolverState, delta_b: &[(usize, f64)]) -> SolverResult;
    fn algorithm_name(&self) -> &'static str;
}

pub struct SolverOptions {
    pub tolerance: f64,
    pub max_iterations: usize,
    pub precision: NumericPrecision,  // f32 vs f64
    pub streaming_interval: usize,    // Iterations between updates
    pub convergence_check: ConvergenceMode,
}

pub struct SolverResult {
    pub solution: Vec<f64>,
    pub residual_norm: f64,
    pub iterations: usize,
    pub converged: bool,
    pub error_bounds: Option<ErrorBounds>,
}

pub struct PartialSolution {
    pub iteration: usize,
    pub partial_x: Vec<(usize, f64)>,  // Sparse solution updates
    pub residual_norm: f64,
    pub timestamp: std::time::Instant,
}
```

#### 3. WASM Interface Design

```rust
// wasm_iface/solver_api.rs
#[wasm_bindgen]
pub struct Solver {
    inner: Box<dyn SolverAlgorithm>,
    matrix: Box<dyn Matrix>,
    current_state: Option<SolverState>,
}

#[wasm_bindgen]
impl Solver {
    #[wasm_bindgen(constructor)]
    pub fn new(matrix_data: JsValue, opts: JsValue) -> Result<Solver, JsValue>;

    #[wasm_bindgen]
    pub fn solve(&mut self, method: &str, b: &[f64]) -> Result<JsValue, JsValue>;

    #[wasm_bindgen]
    pub fn update_costs(&mut self, updates: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen]
    pub fn stream_solve(&mut self, method: &str, b: &[f64]) -> SolutionStream;
}

#[wasm_bindgen]
pub struct SolutionStream {
    inner: Box<dyn Iterator<Item = PartialSolution>>,
}

#[wasm_bindgen]
impl SolutionStream {
    #[wasm_bindgen]
    pub fn next(&mut self) -> Option<JsValue>;
}
```

#### 4. HTTP Server Module

```rust
// http_server/requests.rs
#[derive(Serialize, Deserialize)]
pub struct SolveRequest {
    pub matrix: MatrixData,
    pub b: Vec<f64>,
    pub method: String,
    pub options: SolverOptions,
    pub session_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct UpdateRequest {
    pub session_id: String,
    pub delta_costs: Vec<(usize, f64)>,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize)]
pub struct SolveResponse {
    pub session_id: String,
    pub iteration: usize,
    pub solution: Option<Vec<f64>>,
    pub partial_solution: Option<Vec<(usize, f64)>>,
    pub residual_norm: f64,
    pub converged: bool,
    pub error_bounds: Option<ErrorBounds>,
}
```

## 3. Data Structures

### Matrix Representation Strategy

```rust
// Optimized for different access patterns
pub enum MatrixStorage {
    // For general sparse operations
    CSR {
        values: Vec<f64>,
        col_indices: Vec<u32>,  // u32 for better cache efficiency
        row_ptr: Vec<u32>,
    },
    // For column-oriented operations
    CSC {
        values: Vec<f64>,
        row_indices: Vec<u32>,
        col_ptr: Vec<u32>,
    },
    // For graph algorithms (push methods)
    GraphAdjacency {
        out_edges: Vec<Vec<Edge>>,
        in_edges: Vec<Vec<Edge>>,  // For backward push
        degrees: Vec<f64>,
    },
}

#[derive(Clone, Copy)]
pub struct Edge {
    pub target: u32,    // 4 bytes
    pub weight: f32,    // 4 bytes for cache efficiency
}
```

### Vector Storage Optimization

```rust
pub struct DenseVector {
    data: Vec<f64>,
    capacity: usize,
}

pub struct SparseVector {
    indices: Vec<u32>,
    values: Vec<f64>,
    dimension: usize,
}

// For streaming updates
pub struct DeltaVector {
    updates: HashMap<u32, f64>,  // Index -> new value
    timestamp: u64,
}
```

### Memory Management Strategies

```rust
// Memory pools for frequent allocations
pub struct MemoryPool {
    vector_pool: Vec<Vec<f64>>,
    sparse_pool: Vec<SparseVector>,
    residual_buffers: Vec<Vec<f64>>,
}

impl MemoryPool {
    pub fn get_vector(&mut self, size: usize) -> Vec<f64> {
        self.vector_pool.pop()
            .map(|mut v| { v.clear(); v.resize(size, 0.0); v })
            .unwrap_or_else(|| vec![0.0; size])
    }

    pub fn return_vector(&mut self, mut vec: Vec<f64>) {
        if vec.capacity() <= MAX_POOLED_SIZE {
            vec.clear();
            self.vector_pool.push(vec);
        }
    }
}
```

### Sparse Matrix Handling

```rust
// Adaptive sparse matrix with automatic format selection
pub struct AdaptiveMatrix {
    storage: MatrixStorage,
    stats: AccessStats,
}

impl AdaptiveMatrix {
    // Automatically choose storage format based on access patterns
    pub fn optimize_storage(&mut self) {
        match &self.stats {
            stats if stats.row_access_ratio > 0.8 => {
                if !matches!(self.storage, MatrixStorage::CSR { .. }) {
                    self.convert_to_csr();
                }
            },
            stats if stats.col_access_ratio > 0.8 => {
                if !matches!(self.storage, MatrixStorage::CSC { .. }) {
                    self.convert_to_csc();
                }
            },
            stats if stats.graph_operations > 0.5 => {
                if !matches!(self.storage, MatrixStorage::GraphAdjacency { .. }) {
                    self.convert_to_graph();
                }
            },
            _ => {} // Keep current format
        }
    }
}
```

## 4. Algorithm Integration

### Unified Solver Trait Design

```rust
// Base trait for all solver algorithms
pub trait SolverAlgorithm: Send + Sync {
    type State: SolverState;

    fn initialize(&self, matrix: &dyn Matrix, b: &[f64], opts: &SolverOptions) -> Self::State;

    fn step(&self, state: &mut Self::State) -> StepResult;

    fn is_converged(&self, state: &Self::State) -> bool;

    fn extract_solution(&self, state: &Self::State) -> Vec<f64>;

    fn update_rhs(&self, state: &mut Self::State, delta_b: &[(usize, f64)]);

    // Default implementation for full solve
    fn solve(&self, matrix: &dyn Matrix, b: &[f64], opts: &SolverOptions) -> SolverResult {
        let mut state = self.initialize(matrix, b, opts);
        let mut iterations = 0;

        while !self.is_converged(&state) && iterations < opts.max_iterations {
            match self.step(&mut state) {
                StepResult::Continue => iterations += 1,
                StepResult::Converged => break,
                StepResult::Error(e) => return SolverResult::error(e),
            }
        }

        SolverResult {
            solution: self.extract_solution(&state),
            iterations,
            converged: self.is_converged(&state),
            residual_norm: state.residual_norm(),
            error_bounds: state.error_bounds(),
        }
    }
}
```

### Plugin Architecture for Algorithms

```rust
// Registry for solver algorithms
pub struct SolverRegistry {
    algorithms: HashMap<String, Box<dyn SolverAlgorithm>>,
}

impl SolverRegistry {
    pub fn register<T: SolverAlgorithm + 'static>(&mut self, name: &str, solver: T) {
        self.algorithms.insert(name.to_string(), Box::new(solver));
    }

    pub fn create_solver(&self, name: &str) -> Option<&dyn SolverAlgorithm> {
        self.algorithms.get(name).map(|b| b.as_ref())
    }

    pub fn default() -> Self {
        let mut registry = Self { algorithms: HashMap::new() };

        registry.register("neumann", NeumannSolver::default());
        registry.register("forward_push", ForwardPushSolver::default());
        registry.register("backward_push", BackwardPushSolver::default());
        registry.register("hybrid", HybridSolver::default());

        registry
    }
}

// Algorithm-specific implementations
impl SolverAlgorithm for NeumannSolver {
    type State = NeumannState;

    fn initialize(&self, matrix: &dyn Matrix, b: &[f64], opts: &SolverOptions) -> Self::State {
        // Precompute M = I - D^{-1}A for Neumann series
        let scaling_matrix = self.compute_scaling_matrix(matrix);
        NeumannState::new(matrix, b, scaling_matrix, opts.max_iterations)
    }

    fn step(&self, state: &mut Self::State) -> StepResult {
        // Compute next term: M^k * b
        state.compute_next_term();
        if state.series_converged() {
            StepResult::Converged
        } else {
            StepResult::Continue
        }
    }
}
```

### Error Handling and Recovery

```rust
#[derive(Debug, Clone)]
pub enum SolverError {
    MatrixNotDiagonallyDominant,
    NumericalInstability,
    ConvergenceFailure { iterations: usize, residual: f64 },
    InvalidInput(String),
    MemoryAllocationError,
    WasmBindingError(String),
}

impl SolverError {
    pub fn is_recoverable(&self) -> bool {
        matches!(self,
            SolverError::ConvergenceFailure { .. } |
            SolverError::NumericalInstability
        )
    }

    pub fn recovery_strategy(&self) -> Option<RecoveryAction> {
        match self {
            SolverError::ConvergenceFailure { .. } => {
                Some(RecoveryAction::SwitchAlgorithm("neumann".to_string()))
            },
            SolverError::NumericalInstability => {
                Some(RecoveryAction::IncreasePrecision)
            },
            _ => None,
        }
    }
}

pub enum RecoveryAction {
    SwitchAlgorithm(String),
    IncreasePrecision,
    RelaxTolerance(f64),
    RestartWithDifferentSeeed,
}
```

### Convergence Criteria Management

```rust
pub struct ConvergenceChecker {
    criteria: Vec<Box<dyn ConvergenceCriterion>>,
}

pub trait ConvergenceCriterion {
    fn check(&self, state: &dyn SolverState) -> bool;
    fn name(&self) -> &'static str;
}

pub struct ResidualNormCriterion {
    tolerance: f64,
    norm_type: NormType,
}

pub struct RelativeChangeCriterion {
    tolerance: f64,
    window_size: usize,
}

pub struct StagnationCriterion {
    max_stagnant_iterations: usize,
    improvement_threshold: f64,
}

impl ConvergenceChecker {
    pub fn with_defaults(tolerance: f64) -> Self {
        Self {
            criteria: vec![
                Box::new(ResidualNormCriterion::new(tolerance, NormType::L2)),
                Box::new(StagnationCriterion::new(50, tolerance * 0.1)),
            ],
        }
    }

    pub fn check_convergence(&self, state: &dyn SolverState) -> ConvergenceResult {
        for criterion in &self.criteria {
            if criterion.check(state) {
                return ConvergenceResult::Converged(criterion.name().to_string());
            }
        }
        ConvergenceResult::Continue
    }
}
```

## 5. Performance Considerations

### SIMD Optimization Points

```rust
// Key hot paths for SIMD optimization
impl SparseMatrix {
    // Vectorized sparse matrix-vector multiplication
    #[cfg(target_feature = "avx2")]
    fn multiply_vector_simd(&self, x: &[f64], result: &mut [f64]) {
        use std::arch::x86_64::*;

        for (row_idx, &row_start) in self.row_ptr.iter().enumerate() {
            let row_end = self.row_ptr[row_idx + 1];
            let mut sum = 0.0;

            // Process 4 elements at a time with AVX2
            let mut i = row_start;
            while i + 4 <= row_end {
                unsafe {
                    let values = _mm256_loadu_pd(&self.values[i]);
                    let indices = &self.indices[i..i+4];
                    let x_vals = _mm256_set_pd(
                        x[indices[3]], x[indices[2]],
                        x[indices[1]], x[indices[0]]
                    );
                    let products = _mm256_mul_pd(values, x_vals);
                    let sum_vec = _mm256_hadd_pd(products, products);
                    sum += _mm256_cvtsd_f64(sum_vec);
                }
                i += 4;
            }

            // Handle remaining elements
            for j in i..row_end {
                sum += self.values[j] * x[self.indices[j]];
            }

            result[row_idx] = sum;
        }
    }
}
```

### Memory Pooling Strategies

```rust
// Thread-local memory pools for performance
thread_local! {
    static MEMORY_POOL: RefCell<MemoryPool> = RefCell::new(MemoryPool::new());
}

pub struct WorkspaceManager {
    vector_workspace: Vec<f64>,
    sparse_workspace: SparseVector,
    temp_indices: Vec<usize>,
}

impl WorkspaceManager {
    pub fn get() -> Self {
        MEMORY_POOL.with(|pool| {
            pool.borrow_mut().get_workspace()
        })
    }

    pub fn return_workspace(self) {
        MEMORY_POOL.with(|pool| {
            pool.borrow_mut().return_workspace(self);
        });
    }
}

// RAII wrapper for automatic cleanup
pub struct WorkspaceGuard(Option<WorkspaceManager>);

impl WorkspaceGuard {
    pub fn new() -> Self {
        Self(Some(WorkspaceManager::get()))
    }

    pub fn workspace(&mut self) -> &mut WorkspaceManager {
        self.0.as_mut().unwrap()
    }
}

impl Drop for WorkspaceGuard {
    fn drop(&mut self) {
        if let Some(workspace) = self.0.take() {
            workspace.return_workspace();
        }
    }
}
```

### Cache-Friendly Data Layouts

```rust
// Structure of Arrays (SoA) for better cache utilization
pub struct GraphEdgesSoA {
    targets: Vec<u32>,      // All target nodes together
    weights: Vec<f32>,      // All weights together
    row_offsets: Vec<u32>,  // Where each row starts
}

// Memory-mapped matrix for very large problems
pub struct MappedMatrix {
    file: MemoryMap,
    header: MatrixHeader,
    values_offset: usize,
    indices_offset: usize,
}

impl MappedMatrix {
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        // Read header to understand layout
        let header: MatrixHeader = unsafe {
            std::ptr::read(mmap.as_ptr() as *const MatrixHeader)
        };

        Ok(Self {
            file: mmap,
            header,
            values_offset: std::mem::size_of::<MatrixHeader>(),
            indices_offset: std::mem::size_of::<MatrixHeader>() +
                           header.nnz * std::mem::size_of::<f64>(),
        })
    }
}
```

### Parallel Computation Opportunities

```rust
// Parallel sparse matrix operations using Rayon
impl Matrix for SparseMatrix {
    fn multiply_vector_parallel(&self, x: &[f64], result: &mut [f64]) {
        use rayon::prelude::*;

        result.par_iter_mut()
              .enumerate()
              .for_each(|(row, result_elem)| {
                  let row_start = self.row_ptr[row];
                  let row_end = self.row_ptr[row + 1];

                  *result_elem = self.values[row_start..row_end]
                      .iter()
                      .zip(&self.indices[row_start..row_end])
                      .map(|(&val, &idx)| val * x[idx])
                      .sum();
              });
    }
}

// Parallel push algorithms
impl ForwardPushSolver {
    fn parallel_push_step(&self, state: &mut ForwardPushState) {
        let chunks = state.active_nodes.chunks(CHUNK_SIZE);
        let updates: Vec<_> = chunks
            .into_par_iter()
            .map(|chunk| self.process_node_chunk(chunk, &state.graph))
            .collect();

        // Apply updates sequentially to avoid race conditions
        for update_batch in updates {
            state.apply_updates(update_batch);
        }
    }
}
```

## 6. Testing Architecture

### Unit Test Structure

```rust
// tests/unit/solver/
mod neumann_tests {
    use super::*;

    #[test]
    fn test_neumann_convergence_diagonally_dominant() {
        let matrix = create_test_matrix_diagonally_dominant(100);
        let b = vec![1.0; 100];
        let solver = NeumannSolver::new();

        let result = solver.solve(&matrix, &b, &SolverOptions::default());

        assert!(result.converged);
        assert!(result.residual_norm < 1e-6);

        // Verify solution by direct substitution
        let residual = compute_residual(&matrix, &result.solution, &b);
        assert!(residual.iter().all(|&r| r.abs() < 1e-6));
    }

    #[test]
    fn test_neumann_incremental_updates() {
        // Test that incremental updates produce consistent results
        let mut state = setup_neumann_state();
        let initial_solution = state.current_solution().clone();

        // Apply small update to RHS
        let delta_b = vec![(5, 0.1), (10, -0.05)];
        state.apply_rhs_update(&delta_b);

        let updated_solution = state.current_solution();

        // Verify solution changed appropriately
        assert_ne!(initial_solution[5], updated_solution[5]);
        assert_ne!(initial_solution[10], updated_solution[10]);
    }
}

// Property-based testing with arbitrary matrices
#[cfg(test)]
mod property_tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn solver_satisfies_equation(
            matrix in arbitrary_diagonally_dominant_matrix(10..50),
            b in prop::collection::vec(-10.0..10.0, 10..50)
        ) {
            let solver = HybridSolver::new();
            let result = solver.solve(&matrix, &b, &SolverOptions::default());

            if result.converged {
                let residual_norm = compute_residual_norm(&matrix, &result.solution, &b);
                prop_assert!(residual_norm < 1e-3);
            }
        }
    }
}
```

### Integration Test Framework

```rust
// tests/integration/
mod flow_nexus_integration {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_streaming_http_interface() {
        // Start HTTP server
        let server = HttpServer::new(SolverRegistry::default());
        let addr = server.start().await.unwrap();

        // Create test client
        let client = reqwest::Client::new();

        // Send initial solve request
        let request = SolveRequest {
            matrix: create_test_matrix_json(),
            b: vec![1.0; 100],
            method: "hybrid".to_string(),
            options: SolverOptions::default(),
            session_id: None,
        };

        let response = client
            .post(&format!("http://{}/solve-stream", addr))
            .json(&request)
            .send()
            .await
            .unwrap();

        assert!(response.status().is_success());

        // Read streaming responses
        let mut stream = response.bytes_stream();
        let mut solutions = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.unwrap();
            if let Ok(solution) = serde_json::from_slice::<SolveResponse>(&chunk) {
                solutions.push(solution);
                if solution.converged {
                    break;
                }
            }
        }

        assert!(!solutions.is_empty());
        assert!(solutions.last().unwrap().converged);
    }

    #[tokio::test]
    async fn test_cost_update_streaming() {
        // Test incremental cost updates via streaming
        let server_addr = setup_test_server().await;

        // Send updates in sequence
        let updates = vec![
            UpdateRequest { session_id: "test-1".to_string(), delta_costs: vec![(0, 1.5)], timestamp: 1 },
            UpdateRequest { session_id: "test-1".to_string(), delta_costs: vec![(5, -0.7)], timestamp: 2 },
            UpdateRequest { session_id: "test-1".to_string(), delta_costs: vec![(10, 2.1)], timestamp: 3 },
        ];

        for update in updates {
            let response = send_update(&server_addr, &update).await;
            assert!(response.is_ok());
        }
    }
}
```

### Benchmark Suite Design

```rust
// benches/solver_benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn benchmark_solvers(c: &mut Criterion) {
    let mut group = c.benchmark_group("solvers");

    let sizes = vec![100, 500, 1000, 5000];
    let algorithms = vec!["neumann", "forward_push", "backward_push", "hybrid"];

    for size in sizes {
        for algorithm in &algorithms {
            let matrix = create_benchmark_matrix(size);
            let b = vec![1.0; size];
            let solver = create_solver(algorithm);

            group.bench_with_input(
                BenchmarkId::new(*algorithm, size),
                &size,
                |bencher, _| {
                    bencher.iter(|| {
                        solver.solve(black_box(&matrix), black_box(&b), &SolverOptions::default())
                    });
                },
            );
        }
    }

    group.finish();
}

fn benchmark_memory_usage(c: &mut Criterion) {
    c.bench_function("memory_pool_allocation", |b| {
        let pool = MemoryPool::new();
        b.iter(|| {
            let vectors: Vec<_> = (0..100)
                .map(|_| pool.get_vector(1000))
                .collect();
            for vec in vectors {
                pool.return_vector(vec);
            }
        });
    });
}

fn benchmark_simd_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd");

    let matrix = create_large_sparse_matrix(10000);
    let x = vec![1.0; 10000];
    let mut result = vec![0.0; 10000];

    group.bench_function("scalar_multiply", |b| {
        b.iter(|| matrix.multiply_vector_scalar(black_box(&x), black_box(&mut result)));
    });

    #[cfg(target_feature = "avx2")]
    group.bench_function("simd_multiply", |b| {
        b.iter(|| matrix.multiply_vector_simd(black_box(&x), black_box(&mut result)));
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_solvers,
    benchmark_memory_usage,
    benchmark_simd_operations
);
criterion_main!(benches);
```

### Verification Test Patterns

```rust
// tests/verification/
mod verification_tests {
    use super::*;

    #[test]
    fn test_solution_accuracy_vs_direct_solver() {
        // Compare against high-precision direct solver
        let matrix = load_test_matrix("tests/data/well_conditioned_100x100.mtx");
        let b = load_test_vector("tests/data/rhs_100.vec");

        // Ground truth solution using LU decomposition
        let ground_truth = direct_solve_lu(&matrix, &b);

        // Test each sublinear algorithm
        for algorithm in ["neumann", "forward_push", "hybrid"] {
            let solver = create_solver(algorithm);
            let result = solver.solve(&matrix, &b, &SolverOptions::high_precision());

            let relative_error = compute_relative_error(&result.solution, &ground_truth);
            assert!(relative_error < 1e-4,
                   "Algorithm {} failed accuracy test: error = {}",
                   algorithm, relative_error);
        }
    }

    #[test]
    fn test_error_bounds_reliability() {
        // Verify that computed error bounds are conservative
        let matrix = create_challenging_test_matrix();
        let b = vec![1.0; matrix.rows()];

        let solver = HybridSolver::new();
        let result = solver.solve(&matrix, &b, &SolverOptions::default());

        if let Some(bounds) = result.error_bounds {
            let actual_error = compute_actual_error(&matrix, &result.solution, &b);
            assert!(actual_error <= bounds.upper_bound,
                   "Error bounds are not conservative: actual={}, bound={}",
                   actual_error, bounds.upper_bound);
        }
    }

    #[test]
    fn test_incremental_consistency() {
        // Verify incremental updates give same result as full recomputation
        let matrix = create_test_matrix(500);
        let b = vec![1.0; 500];

        // Solve initial system
        let solver = NeumannSolver::new();
        let mut state = solver.initialize(&matrix, &b, &SolverOptions::default());
        solver.solve_to_convergence(&mut state);
        let incremental_solution = state.current_solution().clone();

        // Apply update incrementally
        let delta_b = vec![(10, 0.5), (20, -0.3)];
        state.apply_rhs_update(&delta_b);
        solver.solve_to_convergence(&mut state);
        let updated_incremental = state.current_solution().clone();

        // Solve full system with updated RHS
        let mut b_updated = b.clone();
        for &(idx, delta) in &delta_b {
            b_updated[idx] += delta;
        }
        let full_solution = solver.solve(&matrix, &b_updated, &SolverOptions::default());

        // Compare solutions
        let difference = compute_solution_difference(&updated_incremental, &full_solution.solution);
        assert!(difference < 1e-10, "Incremental update inconsistent with full solve");
    }
}
```

---

## Architecture Decision Records (ADRs)

### ADR-001: Rust as Core Implementation Language

**Status:** Accepted
**Date:** 2025-09-19

**Context:** Need high-performance numerical computing with memory safety for a system that will be deployed across browsers, Node.js, and cloud environments.

**Decision:** Use Rust as the core implementation language.

**Rationale:**
- Zero-cost abstractions enable high performance without runtime overhead
- Memory safety prevents buffer overflows and use-after-free bugs critical in numerical code
- Excellent WASM compilation support via wasm-bindgen
- Strong type system catches numerical precision errors at compile time
- Active ecosystem for linear algebra and sparse matrix operations

**Consequences:**
- Steeper learning curve for developers unfamiliar with Rust
- Longer compile times compared to interpreted languages
- Excellent performance and safety characteristics
- Cross-platform deployment via WASM compilation

### ADR-002: Plugin Architecture for Solver Algorithms

**Status:** Accepted
**Date:** 2025-09-19

**Context:** Multiple sublinear algorithms (Neumann, push methods, hybrid) need to coexist and be selectable at runtime.

**Decision:** Implement a trait-based plugin architecture with a central registry.

**Rationale:**
- Enables easy addition of new algorithms without modifying core code
- Allows algorithm selection based on problem characteristics
- Supports A/B testing and performance comparison
- Clean separation of concerns between algorithms and infrastructure

**Consequences:**
- Slight runtime overhead from dynamic dispatch
- More complex architecture than monolithic design
- Excellent extensibility and maintainability
- Clear testing boundaries for each algorithm

### ADR-003: WASM + Node.js Hybrid Architecture

**Status:** Accepted
**Date:** 2025-09-19

**Context:** Need to support both browser and server environments while maintaining performance.

**Decision:** Compile Rust core to WASM with Node.js wrapper for I/O and HTTP serving.

**Rationale:**
- WASM provides near-native performance in browsers
- Node.js handles I/O, networking, and file system operations efficiently
- Single codebase for multiple deployment targets
- wasm-bindgen provides mature Rust-to-JavaScript interop

**Consequences:**
- Additional complexity in build pipeline
- Memory marshaling overhead between WASM and JavaScript
- Broad platform compatibility and excellent performance
- npm ecosystem integration

---

This architecture plan provides a comprehensive foundation for implementing the sublinear-time solver system with clear module boundaries, performance optimizations, and extensive testing strategies. The design balances theoretical sophistication with practical engineering concerns, ensuring the system can meet both academic research needs and production deployment requirements.