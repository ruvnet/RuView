# Sublinear-Time Solver: DDD Tactical Design

**Version**: 1.0
**Date**: 2026-02-20
**Status**: Proposed

---

## 1. Aggregate Design

### 1.1 SolverSession Aggregate (Root)

The SolverSession is the primary aggregate root, encapsulating the lifecycle of a solve operation.

```rust
/// Aggregate root for solver operations
pub struct SolverSession {
    // Identity
    id: SessionId,

    // Configuration (set at creation, immutable during solve)
    config: SolverConfig,
    budget: ComputeBudget,

    // State (mutated during solve lifecycle)
    state: SessionState,
    current_algorithm: Algorithm,

    // Event sourcing
    history: Vec<SolverEvent>,
    version: u64,

    // Timing
    created_at: Timestamp,
    started_at: Option<Timestamp>,
    completed_at: Option<Timestamp>,
}

/// Session state machine
#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    /// Created but not yet started
    Idle,
    /// Preprocessing (TRUE: JL, sparsification)
    Preprocessing { phase: PreprocessPhase, progress: f64 },
    /// Active solving
    Solving { iteration: usize, residual: f64 },
    /// Successfully converged
    Converged { result: SolverResult },
    /// Failed with error
    Failed { error: SolverError, best_effort: Option<Vec<f32>> },
    /// Cancelled by user or budget enforcement
    Cancelled { reason: String },
}

impl SolverSession {
    // === Invariants ===

    /// Budget is never exceeded
    fn check_budget(&self) -> Result<(), SolverError> {
        if let Some(started) = self.started_at {
            let elapsed = Timestamp::now() - started;
            if elapsed > self.budget.max_wall_time {
                return Err(SolverError::BudgetExhausted {
                    budget: self.budget.clone(),
                    progress: self.progress(),
                });
            }
        }
        if let SessionState::Solving { iteration, .. } = &self.state {
            if *iteration > self.budget.max_iterations as usize {
                return Err(SolverError::BudgetExhausted {
                    budget: self.budget.clone(),
                    progress: self.progress(),
                });
            }
        }
        Ok(())
    }

    /// State transitions are valid
    fn transition(&mut self, new_state: SessionState) -> Result<(), SolverError> {
        let valid = match (&self.state, &new_state) {
            (SessionState::Idle, SessionState::Preprocessing { .. }) => true,
            (SessionState::Idle, SessionState::Solving { .. }) => true,
            (SessionState::Preprocessing { .. }, SessionState::Solving { .. }) => true,
            (SessionState::Solving { .. }, SessionState::Solving { .. }) => true,
            (SessionState::Solving { .. }, SessionState::Converged { .. }) => true,
            (SessionState::Solving { .. }, SessionState::Failed { .. }) => true,
            (_, SessionState::Cancelled { .. }) => true, // Always cancellable
            _ => false,
        };

        if !valid {
            return Err(SolverError::InvalidStateTransition {
                from: format!("{:?}", self.state),
                to: format!("{:?}", new_state),
            });
        }

        self.state = new_state;
        self.version += 1;
        Ok(())
    }

    // === Commands ===

    pub fn start_solve(&mut self, system: &SparseSystem) -> Result<(), SolverError> {
        self.check_budget()?;
        self.started_at = Some(Timestamp::now());

        self.history.push(SolverEvent::SolveRequested {
            request_id: self.id,
            algorithm: self.current_algorithm,
            input_dimensions: (system.matrix.rows, system.matrix.cols, system.matrix.nnz()),
            timestamp: Timestamp::now(),
        });

        self.transition(SessionState::Solving { iteration: 0, residual: f64::INFINITY })
    }

    pub fn record_iteration(&mut self, iteration: usize, residual: f64) -> Result<(), SolverError> {
        self.check_budget()?;

        self.history.push(SolverEvent::IterationCompleted {
            request_id: self.id,
            iteration,
            residual_norm: residual,
            wall_time_us: self.elapsed_us(),
            timestamp: Timestamp::now(),
        });

        if residual < self.config.tolerance {
            self.transition(SessionState::Converged {
                result: SolverResult {
                    iterations: iteration,
                    final_residual: residual,
                    ..Default::default()
                },
            })
        } else {
            self.transition(SessionState::Solving { iteration, residual })
        }
    }

    pub fn fail_and_fallback(&mut self, error: SolverError) -> Option<Algorithm> {
        let fallback = self.next_fallback();

        self.history.push(SolverEvent::AlgorithmFallback {
            request_id: self.id,
            from_algorithm: self.current_algorithm,
            to_algorithm: fallback,
            reason: error.to_string(),
            timestamp: Timestamp::now(),
        });

        if let Some(next) = fallback {
            self.current_algorithm = next;
            self.state = SessionState::Idle; // Reset for retry
            Some(next)
        } else {
            let _ = self.transition(SessionState::Failed {
                error,
                best_effort: None,
            });
            None
        }
    }

    fn next_fallback(&self) -> Option<Algorithm> {
        match self.current_algorithm {
            Algorithm::Neumann | Algorithm::ForwardPush | Algorithm::BackwardPush |
            Algorithm::HybridRandomWalk | Algorithm::TRUE | Algorithm::BMSSP
                => Some(Algorithm::CG),
            Algorithm::CG => Some(Algorithm::DenseDirect),
            Algorithm::DenseDirect => None, // No further fallback
        }
    }
}
```

### 1.2 SparseSystem Aggregate

```rust
/// Immutable representation of a sparse linear system Ax = b
pub struct SparseSystem {
    id: SystemId,
    matrix: CsrMatrix<f32>,
    rhs: Vec<f32>,
    metadata: SystemMetadata,
}

pub struct SystemMetadata {
    pub sparsity: SparsityProfile,
    pub is_spd: bool,
    pub is_laplacian: bool,
    pub condition_estimate: Option<f64>,
    pub source_context: SourceContext,
}

pub enum SourceContext {
    CoherenceLaplacian { graph_id: String },
    GnnAdjacency { layer: usize, node_count: usize },
    GraphAnalytics { query_type: String },
    SpectralFilter { filter_degree: usize },
    UserProvided,
}

impl SparseSystem {
    // === Invariants ===

    pub fn validate(&self) -> Result<(), ValidationError> {
        // Matrix dimensions match RHS
        if self.matrix.rows != self.rhs.len() {
            return Err(ValidationError::DimensionMismatch {
                expected: self.matrix.rows,
                actual: self.rhs.len(),
            });
        }

        // All values finite
        for v in self.matrix.values.iter() {
            if !v.is_finite() {
                return Err(ValidationError::InvalidNumber {
                    field: "matrix_values", index: 0, reason: "non-finite",
                });
            }
        }
        for v in self.rhs.iter() {
            if !v.is_finite() {
                return Err(ValidationError::InvalidNumber {
                    field: "rhs", index: 0, reason: "non-finite",
                });
            }
        }

        // Sparsity > 0
        if self.matrix.nnz() == 0 {
            return Err(ValidationError::EmptyMatrix);
        }

        Ok(())
    }
}
```

### 1.3 GraphProblem Aggregate

```rust
/// Graph-based problem for Push algorithms and random walks
pub struct GraphProblem {
    id: ProblemId,
    graph: SparseAdjacency,
    query: GraphQuery,
    parameters: PushParameters,
}

pub struct SparseAdjacency {
    pub adj: CsrMatrix<f32>,
    pub directed: bool,
    pub weighted: bool,
}

pub enum GraphQuery {
    SingleSource { source: usize },
    SingleTarget { target: usize },
    Pairwise { source: usize, target: usize },
    BatchSources { sources: Vec<usize> },
    AllNodes,
}

pub struct PushParameters {
    pub alpha: f64,          // Damping factor (default: 0.85)
    pub epsilon: f64,        // Push threshold
    pub max_iterations: u64, // Safety bound
}
```

---

## 2. Entity Design

### 2.1 SolverResult Entity

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverResult {
    pub id: ResultId,
    pub session_id: SessionId,
    pub algorithm_used: Algorithm,
    pub solution: Vec<f32>,
    pub iterations: usize,
    pub residual_norm: f64,
    pub wall_time_us: u64,
    pub convergence: ConvergenceInfo,
    pub error_bounds: ErrorBounds,
    pub audit_entry: SolverAuditEntry,
}
```

### 2.2 ComputeBudget Entity

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeBudget {
    pub max_wall_time: Duration,
    pub max_iterations: u64,
    pub max_memory_bytes: usize,
    pub lane: ComputeLane,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ComputeLane {
    Reflex,     // < 1ms — cached results, trivial problems
    Retrieval,  // ~ 10ms — simple solves (small n, well-conditioned)
    Heavy,      // ~ 100ms — full solver pipeline
    Deliberate, // unbounded — streaming progress, complex problems
}

impl ComputeBudget {
    pub fn for_lane(lane: ComputeLane) -> Self {
        match lane {
            ComputeLane::Reflex => Self {
                max_wall_time: Duration::from_millis(1),
                max_iterations: 10,
                max_memory_bytes: 1 << 20, // 1MB
                lane,
            },
            ComputeLane::Retrieval => Self {
                max_wall_time: Duration::from_millis(10),
                max_iterations: 100,
                max_memory_bytes: 16 << 20, // 16MB
                lane,
            },
            ComputeLane::Heavy => Self {
                max_wall_time: Duration::from_millis(100),
                max_iterations: 10_000,
                max_memory_bytes: 256 << 20, // 256MB
                lane,
            },
            ComputeLane::Deliberate => Self {
                max_wall_time: Duration::from_secs(300),
                max_iterations: 1_000_000,
                max_memory_bytes: 2 << 30, // 2GB
                lane,
            },
        }
    }
}
```

### 2.3 AlgorithmProfile Entity

```rust
#[derive(Debug, Clone)]
pub struct AlgorithmProfile {
    pub algorithm: Algorithm,
    pub complexity_class: ComplexityClass,
    pub sparsity_range: (f64, f64),    // (min_density, max_density)
    pub size_range: (usize, usize),     // (min_n, max_n)
    pub deterministic: bool,
    pub parallelizable: bool,
    pub wasm_compatible: bool,
    pub numerical_stability: Stability,
    pub convergence_guarantee: ConvergenceGuarantee,
}

pub enum ComplexityClass {
    Logarithmic,     // O(log n)
    SquareRoot,      // O(√n)
    NearLinear,      // O(n · polylog(n))
    Linear,          // O(n)
    Quadratic,       // O(n²)
}

pub enum ConvergenceGuarantee {
    Guaranteed { max_iterations: usize },
    Probabilistic { confidence: f64 },
    Conditional { requirement: &'static str },
}
```

---

## 3. Value Objects

### 3.1 CsrMatrix<T>

```rust
/// Immutable value object — equality by content
#[derive(Clone)]
pub struct CsrMatrix<T: Copy> {
    pub values: AlignedVec<T>,
    pub col_indices: AlignedVec<u32>,
    pub row_ptrs: Vec<u32>,
    pub rows: usize,
    pub cols: usize,
}

impl<T: Copy> CsrMatrix<T> {
    pub fn nnz(&self) -> usize { self.values.len() }
    pub fn density(&self) -> f64 { self.nnz() as f64 / (self.rows * self.cols) as f64 }
    pub fn memory_bytes(&self) -> usize {
        self.values.len() * size_of::<T>()
        + self.col_indices.len() * size_of::<u32>()
        + self.row_ptrs.len() * size_of::<u32>()
    }
}
```

### 3.2 ConvergenceInfo

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergenceInfo {
    pub converged: bool,
    pub iterations: usize,
    pub residual_history: Vec<f64>,
    pub final_residual: f64,
    pub convergence_rate: f64, // ratio of consecutive residuals
}
```

### 3.3 SparsityProfile

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparsityProfile {
    pub nonzero_count: usize,
    pub total_elements: usize,
    pub density: f64,
    pub diagonal_dominance: f64,  // fraction of rows that are diag. dominant
    pub bandwidth: usize,          // max |i - j| for nonzero a_ij
    pub symmetry: f64,             // fraction of entries with a_ij == a_ji
    pub avg_row_nnz: f64,
    pub max_row_nnz: usize,
}
```

### 3.4 ComplexityEstimate

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityEstimate {
    pub estimated_flops: u64,
    pub estimated_memory_bytes: u64,
    pub estimated_wall_time_us: u64,
    pub recommended_algorithm: Algorithm,
    pub recommended_lane: ComputeLane,
    pub confidence: f64,
}
```

### 3.5 ErrorBounds

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBounds {
    pub absolute_error: f64,    // ||x_approx - x_exact||
    pub relative_error: f64,    // ||x_approx - x_exact|| / ||x_exact||
    pub residual_norm: f64,     // ||A*x_approx - b||
    pub confidence: f64,        // Statistical confidence (for randomized algorithms)
}
```

---

## 4. Domain Events

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SolverEvent {
    SolveRequested {
        request_id: SessionId,
        algorithm: Algorithm,
        input_dimensions: (usize, usize, usize), // rows, cols, nnz
        timestamp: Timestamp,
    },
    IterationCompleted {
        request_id: SessionId,
        iteration: usize,
        residual_norm: f64,
        wall_time_us: u64,
        timestamp: Timestamp,
    },
    SolveConverged {
        request_id: SessionId,
        total_iterations: usize,
        final_residual: f64,
        total_wall_time_us: u64,
        accuracy: ErrorBounds,
        timestamp: Timestamp,
    },
    SolveFailed {
        request_id: SessionId,
        error: String,
        best_residual: f64,
        iterations_completed: usize,
        timestamp: Timestamp,
    },
    AlgorithmFallback {
        request_id: SessionId,
        from_algorithm: Algorithm,
        to_algorithm: Option<Algorithm>,
        reason: String,
        timestamp: Timestamp,
    },
    BudgetExhausted {
        request_id: SessionId,
        budget: ComputeBudget,
        best_residual: f64,
        timestamp: Timestamp,
    },
    ComplexityEstimated {
        request_id: SessionId,
        estimate: ComplexityEstimate,
        timestamp: Timestamp,
    },
    SparsityDetected {
        system_id: SystemId,
        profile: SparsityProfile,
        recommended_path: Algorithm,
        timestamp: Timestamp,
    },
    NumericalWarning {
        request_id: SessionId,
        warning_type: NumericalWarningType,
        detail: String,
        timestamp: Timestamp,
    },
}

pub enum NumericalWarningType {
    NearSingular,
    SlowConvergence,
    OrthogonalityLoss,
    MassInvariantViolation,
    PrecisionLoss,
}
```

---

## 5. Domain Services

### 5.1 SolverOrchestrator

```rust
/// Orchestrates: routing → validation → execution → fallback → result
pub struct SolverOrchestrator {
    router: AlgorithmRouter,
    solvers: HashMap<Algorithm, Box<dyn SolverEngine>>,
    budget_enforcer: BudgetEnforcer,
    event_bus: broadcast::Sender<SolverEvent>,
}

impl SolverOrchestrator {
    pub async fn solve(&self, system: SparseSystem) -> Result<SolverResult, SolverError> {
        // 1. Analyze sparsity
        let profile = system.metadata.sparsity.clone();
        self.event_bus.send(SolverEvent::SparsityDetected { .. });

        // 2. Route to optimal algorithm
        let algorithm = self.router.select(&ProblemProfile::from(&system));
        let estimate = self.estimate_complexity(&system);
        self.event_bus.send(SolverEvent::ComplexityEstimated { .. });

        // 3. Create session
        let mut session = SolverSession::new(algorithm, estimate.recommended_lane);

        // 4. Execute with fallback chain
        loop {
            match self.execute_algorithm(&mut session, &system).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    match session.fail_and_fallback(e) {
                        Some(_next) => continue, // Retry with fallback
                        None => return Err(SolverError::AllAlgorithmsFailed),
                    }
                }
            }
        }
    }
}
```

### 5.2 SparsityAnalyzer

```rust
/// Analyzes matrix properties for routing decisions
pub struct SparsityAnalyzer;

impl SparsityAnalyzer {
    pub fn analyze(matrix: &CsrMatrix<f32>) -> SparsityProfile {
        SparsityProfile {
            nonzero_count: matrix.nnz(),
            total_elements: matrix.rows * matrix.cols,
            density: matrix.density(),
            diagonal_dominance: Self::measure_diagonal_dominance(matrix),
            bandwidth: Self::estimate_bandwidth(matrix),
            symmetry: Self::measure_symmetry(matrix),
            avg_row_nnz: matrix.nnz() as f64 / matrix.rows as f64,
            max_row_nnz: Self::max_row_nnz(matrix),
        }
    }
}
```

### 5.3 ConvergenceMonitor

```rust
/// Monitors convergence and triggers fallback
pub struct ConvergenceMonitor {
    stagnation_window: usize,  // Look back N iterations
    stagnation_threshold: f64, // Improvement < threshold → stagnant
    divergence_factor: f64,    // Residual growth > factor → diverging
}

impl ConvergenceMonitor {
    pub fn check(&self, history: &[f64]) -> ConvergenceStatus {
        if history.len() < 2 {
            return ConvergenceStatus::Progressing;
        }

        let latest = *history.last().unwrap();
        let previous = history[history.len() - 2];

        // Divergence check
        if latest > previous * self.divergence_factor {
            return ConvergenceStatus::Diverging;
        }

        // Stagnation check
        if history.len() >= self.stagnation_window {
            let window_start = history[history.len() - self.stagnation_window];
            let improvement = (window_start - latest) / window_start;
            if improvement < self.stagnation_threshold {
                return ConvergenceStatus::Stagnant;
            }
        }

        ConvergenceStatus::Progressing
    }
}
```

---

## 6. Repositories

### 6.1 SolverSessionRepository

```rust
pub trait SolverSessionRepository: Send + Sync {
    fn save(&self, session: &SolverSession) -> Result<(), RepositoryError>;
    fn find_by_id(&self, id: &SessionId) -> Result<Option<SolverSession>, RepositoryError>;
    fn find_active(&self) -> Result<Vec<SolverSession>, RepositoryError>;
    fn delete(&self, id: &SessionId) -> Result<(), RepositoryError>;
}

/// In-memory implementation (server, WASM)
pub struct InMemorySessionRepo {
    sessions: DashMap<SessionId, SolverSession>,
}
```

---

## 7. Factories

### 7.1 SolverFactory

```rust
pub struct SolverFactory;

impl SolverFactory {
    pub fn create(algorithm: Algorithm, config: &SolverConfig) -> Box<dyn SolverEngine> {
        match algorithm {
            Algorithm::Neumann => Box::new(NeumannSolver::from_config(config)),
            Algorithm::ForwardPush => Box::new(ForwardPushSolver::from_config(config)),
            Algorithm::BackwardPush => Box::new(BackwardPushSolver::from_config(config)),
            Algorithm::HybridRandomWalk => Box::new(HybridRandomWalkSolver::from_config(config)),
            Algorithm::TRUE => Box::new(TrueSolver::from_config(config)),
            Algorithm::CG => Box::new(ConjugateGradientSolver::from_config(config)),
            Algorithm::BMSSP => Box::new(BmsspSolver::from_config(config)),
            Algorithm::DenseDirect => Box::new(DenseDirectSolver::from_config(config)),
        }
    }
}
```

### 7.2 SparseSystemFactory

```rust
pub struct SparseSystemFactory;

impl SparseSystemFactory {
    pub fn from_hnsw(hnsw: &HnswIndex, level: usize) -> SparseSystem { ... }
    pub fn from_adjacency_list(edges: &[(usize, usize, f32)], n: usize) -> SparseSystem { ... }
    pub fn from_dense(matrix: &[Vec<f32>], threshold: f32) -> SparseSystem { ... }
    pub fn laplacian_from_graph(graph: &SparseAdjacency) -> SparseSystem { ... }
}
```

---

## 8. Module Structure

```
crates/ruvector-solver/src/
├── lib.rs                    # Public API surface
├── domain/
│   ├── mod.rs
│   ├── aggregates/
│   │   ├── session.rs        # SolverSession aggregate
│   │   ├── sparse_system.rs  # SparseSystem aggregate
│   │   └── graph_problem.rs  # GraphProblem aggregate
│   ├── entities/
│   │   ├── result.rs         # SolverResult entity
│   │   ├── budget.rs         # ComputeBudget entity
│   │   └── profile.rs        # AlgorithmProfile entity
│   ├── values/
│   │   ├── csr_matrix.rs     # CsrMatrix<T> value object
│   │   ├── convergence.rs    # ConvergenceInfo value object
│   │   ├── sparsity.rs       # SparsityProfile value object
│   │   └── estimate.rs       # ComplexityEstimate value object
│   └── events.rs             # SolverEvent enum
├── services/
│   ├── orchestrator.rs       # SolverOrchestrator
│   ├── sparsity_analyzer.rs  # SparsityAnalyzer
│   ├── convergence_monitor.rs # ConvergenceMonitor
│   └── budget_enforcer.rs    # BudgetEnforcer
├── algorithms/
│   ├── neumann.rs
│   ├── forward_push.rs
│   ├── backward_push.rs
│   ├── hybrid_random_walk.rs
│   ├── true_solver.rs
│   ├── conjugate_gradient.rs
│   ├── bmssp.rs
│   └── dense_direct.rs
├── routing/
│   ├── router.rs             # AlgorithmRouter
│   ├── heuristic.rs          # Tier 2 rules
│   └── adaptive.rs           # Tier 3 SONA
├── infrastructure/
│   ├── arena.rs              # Arena allocator integration
│   ├── simd.rs               # SIMD dispatch
│   ├── repository.rs         # Session repository
│   └── factory.rs            # SolverFactory, SparseSystemFactory
└── traits.rs                 # SolverEngine, NumericBackend, etc.
```

---

## 9. State Machine

```
                    ┌─────────┐
                    │  IDLE    │
                    └────┬────┘
                         │ start_solve()
                    ┌────▼────┐
              ┌─────│PREPROC. │──────┐
              │     └────┬────┘      │
              │          │ done      │ cancel
              │     ┌────▼────┐      │
              │     │ SOLVING  │◀────┤ (back to SOLVING on retry)
              │     └──┬──┬───┘      │
              │        │  │          │
              │  converge fail       │
              │        │  │          │
              │   ┌────▼┐ ┌▼────┐   │
              │   │CONV.│ │FAIL │   │
              │   └─────┘ └──┬──┘   │
              │              │      │
              │         fallback?   │
              │          Y    N     │
              │          │    │     │
              │     ┌────▼┐  │  ┌──▼──────┐
              └────▶│IDLE │  └─▶│CANCELLED│
                    └─────┘     └─────────┘
```
