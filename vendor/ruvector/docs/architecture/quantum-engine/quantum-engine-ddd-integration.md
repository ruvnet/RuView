# Quantum Simulation Engine: Domain-Driven Design - Integration Patterns

**Version**: 0.1
**Date**: 2026-02-06
**Status**: Draft

---

## Overview

This document defines the cross-domain integration patterns, anti-corruption layers, shared kernel, and context mapping that connect the quantum simulation engine (`ruqu-core`, `ruqu-algorithms`, `ruqu-wasm`) to the existing ruVector subsystems. It specifies how the simulation domain communicates with the coherence engine, agent system, graph database, and WASM platform without contaminating bounded context boundaries.

---

## Context Map

```
+-------------------------------------------------------------------+
|                         CONTEXT MAP                                |
|                                                                    |
|  +--------------------+     Shared Kernel     +------------------+ |
|  |                    |<----(ruvector-math)--->|                  | |
|  |  Quantum Sim       |                       |  Coherence       | |
|  |  Engine             |                       |  Engine          | |
|  |  (ruqu-core,        |    Anti-Corruption    |  (ruvector-      | |
|  |   ruqu-algorithms)  |<----(CoherenceBridge) |   coherence)     | |
|  |                    |                       |                  | |
|  +--------+-----------+                       +------------------+ |
|           |                                          ^             |
|           | Customer-Supplier                        |             |
|           v                                          |             |
|  +--------------------+                    +---------+--------+   |
|  |                    |    Partnership     |                  |   |
|  |  Agent System      |<----------------->|  Graph Database  |   |
|  |  (claude-flow)     |                   |  (ruvector-graph)|   |
|  |                    |                   |                  |   |
|  +--------------------+                   +------------------+   |
|           |                                                       |
|           | Conformist                                            |
|           v                                                       |
|  +--------------------+     Published Language                    |
|  |                    |<----(OpenQASM 3.0)                       |
|  |  WASM Platform     |                                          |
|  |  (ruqu-wasm)       |                                          |
|  |                    |                                          |
|  +--------------------+                                          |
+-------------------------------------------------------------------+
```

### Relationship Summary

| Upstream | Downstream | Pattern | Shared Artifact |
|----------|------------|---------|-----------------|
| Quantum Engine | Coherence Engine | Anti-Corruption Layer | `CoherenceBridge` trait |
| ruvector-math | Quantum Engine, Coherence Engine | Shared Kernel | `Complex<f64>`, SIMD traits |
| Quantum Engine | Agent System | Customer-Supplier | `SimulationContract` |
| ruvector-graph | Quantum Engine | Partnership | Adjacency structures |
| External tools | Quantum Engine | Published Language | OpenQASM 3.0 |
| WASM platform | ruqu-wasm | Conformist | WASM constraints accepted |

---

## 1. Anti-Corruption Layer: Coherence Bridge

The Coherence Bridge translates between the quantum simulation domain language and the ruQu coherence domain. It prevents internal types from either domain from leaking into the other.

### Purpose

- Map syndrome bitstrings produced by surface code experiments into the `SyndromeFilter` input format expected by the coherence engine
- Map decoder correction outputs (Pauli operators) to gate operations the simulation can apply
- Translate coherence scores into the `CoherenceScore` value object used by simulation sessions
- Isolate the quantum simulation engine from changes in the coherence engine's internal API

### Interface

```rust
/// Anti-corruption layer between quantum simulation and coherence engine.
///
/// All translation between bounded contexts passes through this trait.
/// Neither domain's internal types appear on the wrong side of this boundary.
pub trait CoherenceBridge: Send + Sync {
    /// Translate a quantum syndrome into a coherence engine filter input.
    ///
    /// The simulation produces `SyndromeBits`; the coherence engine expects
    /// `DetectorBitmap` with specific tile routing. This method handles the
    /// mapping, including stabilizer-to-detector index translation.
    fn syndrome_to_filter_input(
        &self,
        syndrome: &SyndromeBits,
        code_distance: u32,
    ) -> Result<CoherenceFilterInput, BridgeError>;

    /// Translate a coherence decoder correction into Pauli gate operations.
    ///
    /// The coherence engine's decoder outputs correction vectors in its own
    /// format. This method maps them to `PauliOp` sequences that the
    /// simulation engine can apply as gate operations.
    fn correction_to_pauli_ops(
        &self,
        correction: &CoherenceCorrectionOutput,
    ) -> Result<Vec<(QubitIndex, PauliOp)>, BridgeError>;

    /// Query the current coherence score for a simulation region.
    ///
    /// Returns a domain-native `CoherenceScore` value object, hiding
    /// the coherence engine's internal energy representation.
    fn query_coherence_score(
        &self,
        region_id: &str,
    ) -> Result<CoherenceScore, BridgeError>;

    /// Submit simulation metrics to the coherence monitoring system.
    ///
    /// Translates `SimulationMetrics` into the coherence engine's
    /// signal ingestion format without exposing internal types.
    fn report_simulation_metrics(
        &self,
        session_id: &str,
        metrics: &SimulationMetrics,
    ) -> Result<(), BridgeError>;
}

/// Opaque input type for the coherence filter (ACL boundary type).
pub struct CoherenceFilterInput {
    pub detector_bitmap: Vec<u64>,
    pub tile_id: u8,
    pub round_id: u64,
}

/// Opaque output type from the coherence decoder (ACL boundary type).
pub struct CoherenceCorrectionOutput {
    pub corrections: Vec<(u32, u8)>,  // (qubit_index, pauli_code)
    pub confidence: f64,
}

/// Errors specific to the bridge translation layer.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("syndrome dimension mismatch: expected {expected}, got {actual}")]
    SyndromeDimensionMismatch { expected: usize, actual: usize },

    #[error("unknown correction code: {0}")]
    UnknownCorrectionCode(u8),

    #[error("coherence engine unavailable: {0}")]
    CoherenceUnavailable(String),

    #[error("tile routing failed for code distance {0}")]
    TileRoutingFailed(u32),
}
```

### Implementation Sketch

```rust
/// Production implementation backed by the ruQu coherence engine.
pub struct RuQuCoherenceBridge {
    /// Reference to the coherence engine's filter pipeline.
    filter_pipeline: Arc<dyn FilterPipelineAccess>,
    /// Stabilizer-to-detector mapping, precomputed per code distance.
    detector_maps: HashMap<u32, StabilizerDetectorMap>,
}

impl CoherenceBridge for RuQuCoherenceBridge {
    fn syndrome_to_filter_input(
        &self,
        syndrome: &SyndromeBits,
        code_distance: u32,
    ) -> Result<CoherenceFilterInput, BridgeError> {
        let map = self.detector_maps.get(&code_distance)
            .ok_or(BridgeError::TileRoutingFailed(code_distance))?;

        let mut bitmap = vec![0u64; (map.detector_count + 63) / 64];
        for (stab_idx, &fired) in syndrome.0.iter().enumerate() {
            if fired {
                let det_idx = map.stabilizer_to_detector(stab_idx);
                bitmap[det_idx / 64] |= 1u64 << (det_idx % 64);
            }
        }

        Ok(CoherenceFilterInput {
            detector_bitmap: bitmap,
            tile_id: map.tile_for_distance(code_distance),
            round_id: 0, // Filled by caller
        })
    }

    fn correction_to_pauli_ops(
        &self,
        correction: &CoherenceCorrectionOutput,
    ) -> Result<Vec<(QubitIndex, PauliOp)>, BridgeError> {
        correction.corrections.iter()
            .map(|(qubit, code)| {
                let op = match code {
                    0 => PauliOp::I,
                    1 => PauliOp::X,
                    2 => PauliOp::Y,
                    3 => PauliOp::Z,
                    other => return Err(BridgeError::UnknownCorrectionCode(*other)),
                };
                Ok((QubitIndex(*qubit), op))
            })
            .collect()
    }

    fn query_coherence_score(
        &self,
        region_id: &str,
    ) -> Result<CoherenceScore, BridgeError> {
        let energy = self.filter_pipeline.current_energy(region_id)
            .map_err(|e| BridgeError::CoherenceUnavailable(e.to_string()))?;
        // Invert: high energy = low coherence
        Ok(CoherenceScore(1.0 / (1.0 + energy as f64)))
    }

    fn report_simulation_metrics(
        &self,
        _session_id: &str,
        _metrics: &SimulationMetrics,
    ) -> Result<(), BridgeError> {
        // Translate to coherence signal format and submit
        Ok(())
    }
}
```

---

## 2. Shared Kernel: ruvector-math

Both the quantum simulation engine and the coherence engine depend on a shared mathematical foundation. Changes to `ruvector-math` must be validated against both domains before release.

### Shared Types

```rust
// ruvector-math provides these types used by both domains:

/// Complex number with f64 components (re, im).
/// Used by quantum state vectors AND coherence restriction maps.
pub struct Complex<T> {
    pub re: T,
    pub im: T,
}

/// Cache-line-aligned vector for SIMD operations.
/// Used by both state vector operations and residual computation.
#[repr(align(64))]
pub struct AlignedVec<T> {
    data: Vec<T>,
}

/// SIMD dispatch trait: implementations select AVX2, NEON, or scalar
/// at runtime depending on platform capabilities.
pub trait SimdOps {
    fn dot_product_f64(a: &[f64], b: &[f64]) -> f64;
    fn complex_multiply(a: &[Complex<f64>], b: &[Complex<f64>], out: &mut [Complex<f64>]);
    fn norm_squared(v: &[Complex<f64>]) -> f64;
    fn axpy(alpha: f64, x: &[f64], y: &mut [f64]);
}
```

### Change Coordination Protocol

1. Any proposed change to `ruvector-math` must include tests for both the quantum engine use case and the coherence engine use case.
2. The CI pipeline runs `cargo test -p ruqu-core` and `cargo test -p ruvector-coherence` after any change to `ruvector-math`.
3. Breaking changes require a version bump and simultaneous updates to both downstream crates.
4. Performance regressions in SIMD operations must be caught by benchmarks in both domains.

### Boundary

Only the types and functions listed above cross the shared kernel boundary. Internal implementation details of `ruvector-math` (e.g., specific SIMD intrinsics, platform detection) are not shared.

---

## 3. Customer-Supplier: Agent System Integration

The ruVector agent system (powered by claude-flow) acts as the customer, invoking the quantum simulation engine as a supplier. The contract defines what the agent can request and what it receives in return.

### Contract

```rust
/// Contract for agent system access to the quantum simulation engine.
///
/// The agent system (customer) invokes these operations.
/// The quantum engine (supplier) fulfills them.
pub trait SimulationContract: Send + Sync {
    /// Build a circuit from a high-level description.
    fn build_circuit(&self, spec: CircuitSpec) -> Result<CircuitHandle, ContractError>;

    /// Run a simulation and return results.
    fn run_simulation(&self, circuit: CircuitHandle, config: RunConfig)
        -> Result<SimulationOutput, ContractError>;

    /// Run a VQE optimization and return the ground state energy.
    fn run_vqe(&self, spec: VQESpec) -> Result<VQEOutput, ContractError>;

    /// Query resource requirements before committing to a run.
    fn estimate_resources(&self, circuit: CircuitHandle) -> Result<ResourceEstimate, ContractError>;
}

/// High-level circuit specification from the agent.
pub struct CircuitSpec {
    pub qubit_count: u32,
    pub gate_sequence: Vec<GateSpec>,
    pub parameters: HashMap<String, f64>,
}

/// Agent-facing gate specification (simplified from internal Gate).
pub struct GateSpec {
    pub gate_type: String,
    pub target: u32,
    pub control: Option<u32>,
    pub angle: Option<f64>,
}

/// Configuration limits the agent can set.
pub struct RunConfig {
    pub max_shots: u32,
    pub max_memory_mb: u32,
    pub timeout_seconds: u32,
    pub backend_preference: Option<String>,
}

/// Results returned to the agent.
pub struct SimulationOutput {
    pub measurement_counts: HashMap<String, u32>,
    pub expectation_values: Vec<(String, f64)>,
    pub metrics: SimulationMetrics,
}

/// VQE-specific results.
pub struct VQEOutput {
    pub ground_state_energy: f64,
    pub optimal_parameters: Vec<f64>,
    pub iterations: u32,
    pub converged: bool,
}

/// Resource estimate before execution.
pub struct ResourceEstimate {
    pub memory_bytes: usize,
    pub estimated_time_ms: f64,
    pub qubit_count: u32,
    pub gate_count: u32,
}
```

### Agent Integration Flow

```
Agent Context         Quantum Engine            Result
    |                      |                      |
    | 1. build_circuit()   |                      |
    |--------------------->|                      |
    |   CircuitHandle      |                      |
    |<---------------------|                      |
    |                      |                      |
    | 2. estimate_resources|                      |
    |--------------------->|                      |
    |   ResourceEstimate   |                      |
    |<---------------------|                      |
    |                      |                      |
    | 3. run_simulation()  |                      |
    |--------------------->|                      |
    |                      | [executes internally]|
    |                      |---+                  |
    |                      |   | circuit -> state |
    |                      |   | gates -> measure |
    |                      |<--+                  |
    |   SimulationOutput   |                      |
    |<---------------------|                      |
    |                      |                      |
    | 4. Agent acts on     |                      |
    |    results           |                      |
    v                      v                      v
```

### Resource Limits

The supplier enforces resource limits set by the customer:

- Memory: Capped at `max_memory_mb`; returns error if state vector exceeds budget
- Time: Monitored per-step; simulation aborted if `timeout_seconds` exceeded
- Qubits: Platform limit (30 for state vector, higher for tensor network) communicated via `estimate_resources`

---

## 4. Published Language: OpenQASM Compatibility

A future integration point for importing and exporting circuits in the OpenQASM 3.0 standard, enabling interoperability with IBM Qiskit, Google Cirq, and other quantum frameworks.

### Translation Layer

```rust
/// Trait for OpenQASM import/export.
pub trait OpenQASMTranslator {
    /// Parse an OpenQASM 3.0 string into the internal circuit representation.
    fn import(&self, qasm: &str) -> Result<QuantumCircuit, TranslationError>;

    /// Export an internal circuit to OpenQASM 3.0 format.
    fn export(&self, circuit: &QuantumCircuit) -> Result<String, TranslationError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error("unsupported gate in OpenQASM: {0}")]
    UnsupportedGate(String),

    #[error("parse error at line {line}: {message}")]
    ParseError { line: u32, message: String },

    #[error("circuit uses features not supported by OpenQASM 3.0: {0}")]
    UnsupportedFeature(String),
}
```

### Scope

- Phase 1: Import basic gate circuits (H, CNOT, Rz, measure)
- Phase 2: Export circuits with parameter bindings
- Phase 3: Support custom gate definitions and classical control flow

---

## 5. Conformist: WASM Platform

The `ruqu-wasm` crate conforms to WASM platform constraints without attempting to work around them. Limitations are accepted as-is, with graceful degradation where capabilities are reduced.

### Accepted Constraints

| Constraint | Impact | Mitigation |
|------------|--------|------------|
| No native threads | Single-threaded execution | Sequential gate application; no rayon |
| 4GB memory limit | Max ~25 qubits (state vector) | Tensor network backend for larger circuits |
| No filesystem | Cannot persist results | Return all data via JS callbacks |
| No system clock | Timing metrics unavailable | Use `performance.now()` via JS bridge |
| No SIMD (some runtimes) | Slower math | Feature-gated SIMD; scalar fallback |

### WASM API Surface

```rust
/// Public API exposed to JavaScript via wasm-bindgen.
///
/// This is the conformist boundary: we accept WASM constraints
/// and expose only what the platform allows.
#[cfg(target_arch = "wasm32")]
pub mod wasm_api {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub struct WasmSimulator {
        session: SimulationSession,
    }

    #[wasm_bindgen]
    impl WasmSimulator {
        /// Create a new simulator for the given qubit count.
        #[wasm_bindgen(constructor)]
        pub fn new(qubit_count: u32) -> Result<WasmSimulator, JsValue> {
            // Enforce WASM-specific qubit limit
            if qubit_count > 25 {
                return Err(JsValue::from_str(
                    "WASM platform supports at most 25 qubits in state vector mode"
                ));
            }
            // ... construction
            Ok(WasmSimulator { session: todo!() })
        }

        /// Add a gate to the circuit.
        pub fn add_gate(&mut self, gate_type: &str, target: u32, control: Option<u32>)
            -> Result<(), JsValue> { Ok(()) }

        /// Run the simulation and return measurement counts as JSON.
        pub fn run(&mut self, shots: u32) -> Result<String, JsValue> {
            Ok("{}".to_string())
        }

        /// Get memory usage estimate in bytes.
        pub fn memory_estimate(&self) -> usize { 0 }
    }
}
```

---

## 6. Partnership: Graph Database Integration

The `ruvector-graph` crate and the quantum simulation engine have a bidirectional partnership around graph-structured problems, particularly QAOA and MaxCut.

### Data Flow

```rust
/// Graph data provided by ruvector-graph for quantum optimization.
pub struct GraphProblem {
    pub vertex_count: u32,
    pub edges: Vec<(u32, u32, f64)>,  // (source, target, weight)
    pub problem_type: GraphProblemType,
}

#[derive(Debug, Clone, Copy)]
pub enum GraphProblemType { MaxCut, GraphColoring, TSP }

/// Results returned to ruvector-graph for annotation.
pub struct QuantumGraphResult {
    pub objective_value: CutValue,
    pub partition: Vec<bool>,
    pub confidence: f64,
    pub circuit_depth: CircuitDepth,
}

/// Partnership interface: both sides contribute and consume.
pub trait GraphQuantumPartnership {
    /// Graph -> Quantum: convert graph problem to QAOA circuit.
    fn graph_to_qaoa_circuit(
        &self,
        problem: &GraphProblem,
        layers: u32,
    ) -> Result<QuantumCircuit, DomainError>;

    /// Quantum -> Graph: feed optimization results back as graph annotations.
    fn annotate_graph_with_result(
        &self,
        problem: &GraphProblem,
        result: &QuantumGraphResult,
    ) -> Result<GraphAnnotation, DomainError>;

    /// Shared interest: partition graph using ruvector-mincut for subproblem decomposition.
    fn decompose_problem(
        &self,
        problem: &GraphProblem,
        max_subproblem_qubits: u32,
    ) -> Result<Vec<GraphProblem>, DomainError>;
}

/// Annotation written back to the graph database.
pub struct GraphAnnotation {
    pub vertex_labels: HashMap<u32, String>,
    pub edge_labels: HashMap<(u32, u32), String>,
    pub metadata: HashMap<String, String>,
}
```

---

## Cross-Cutting Concerns

### Error Handling Across Boundaries

Each bounded context defines its own error type. At integration boundaries, errors are translated through the ACL rather than propagated directly.

```rust
/// Integration boundary error: wraps domain errors from either side.
#[derive(Debug, thiserror::Error)]
pub enum IntegrationError {
    #[error("quantum engine error: {0}")]
    QuantumEngine(#[from] DomainError),

    #[error("coherence bridge error: {0}")]
    CoherenceBridge(#[from] BridgeError),

    #[error("contract violation: {0}")]
    ContractViolation(String),

    #[error("resource limit exceeded: {0}")]
    ResourceLimit(String),
}
```

### Observability

Distributed tracing spans cross crate boundaries with a shared trace context.

- Each integration call propagates a `TraceId` through the ACL
- The coherence bridge logs translation events at `DEBUG` level
- Agent contract calls log at `INFO` with duration and resource usage
- WASM calls use `console.log` via the JS bridge when tracing is enabled

### Resource Management

Memory and thread resources are coordinated with the ruVector runtime.

- State vector allocation checks the global memory budget before proceeding
- Tensor network contractions respect thread pool limits shared with rayon
- WASM mode has a fixed 4GB ceiling enforced at the conformist boundary
- All resource allocation events emit `MemoryAllocated` / `MemoryReleased` domain events

### Configuration Propagation

Configuration flows from the ruVector root config into the quantum engine.

```rust
/// Quantum engine configuration derived from ruVector global config.
pub struct QuantumEngineConfig {
    pub max_qubits: u32,
    pub default_backend: BackendType,
    pub memory_budget_bytes: usize,
    pub thread_count: usize,
    pub coherence_bridge_enabled: bool,
    pub wasm_mode: bool,
}

impl From<&RuVectorConfig> for QuantumEngineConfig {
    fn from(global: &RuVectorConfig) -> Self {
        Self {
            max_qubits: global.quantum.max_qubits.unwrap_or(30),
            default_backend: global.quantum.backend.parse().unwrap_or(BackendType::StateVector),
            memory_budget_bytes: global.memory.budget_bytes,
            thread_count: global.runtime.thread_count,
            coherence_bridge_enabled: global.coherence.enabled,
            wasm_mode: cfg!(target_arch = "wasm32"),
        }
    }
}
```

---

## Event Flow Diagrams

### 1. VQE Optimization Flow

```
Agent              CircuitBuilder     SimSession       QuantumState      Optimizer
  |                     |                |                 |                |
  | build_circuit(spec) |                |                 |                |
  |-------------------->|                |                 |                |
  |   CircuitHandle     |                |                 |                |
  |<--------------------|                |                 |                |
  |                     |                |                 |                |
  | run_vqe(spec)       |                |                 |                |
  |-------------------------------------------------------------->|        |
  |                     |                |                 |  init(params)  |
  |                     |                |                 |<---------------|
  |                     |                |                 |                |
  |                     |          +-----|---LOOP----------|--------+       |
  |                     |          |     |                 |        |       |
  |                     |          | start()              |        |       |
  |                     |          |     |----->|          |        |       |
  |                     |          |     | apply_gates()   |        |       |
  |                     |          |     |     |---------->|        |       |
  |                     |          |     |     |  expectation_value |       |
  |                     |          |     |     |---------->|        |       |
  |                     |          |     |     |  energy   |        |       |
  |                     |          |     |<----|-----------|        |       |
  |                     |          |     |                 | update(grad)   |
  |                     |          |     |                 |------->|       |
  |                     |          |     |                 | new_params     |
  |                     |          |     |                 |<-------|       |
  |                     |          +-----|---END LOOP------|--------+       |
  |                     |                |                 |                |
  |  VQEOutput(energy, params)           |                 |                |
  |<-------------------------------------------------------------|        |
  |                     |                |                 |                |
```

### 2. Surface Code QEC with Coherence Bridge

```
SurfaceCodeExp     NoiseService    CoherenceBridge    ruQu Filters     Decoder
  |                    |                |                  |               |
  | run_cycle()        |                |                  |               |
  |--+                 |                |                  |               |
  |  | inject_errors() |                |                  |               |
  |  |---------------->|                |                  |               |
  |  | error_list      |                |                  |               |
  |  |<----------------|                |                  |               |
  |  |                 |                |                  |               |
  |  | extract_syndrome()               |                  |               |
  |  |--+              |                |                  |               |
  |  |  | SyndromeBits |                |                  |               |
  |  |<-+              |                |                  |               |
  |  |                 |                |                  |               |
  |  | syndrome_to_filter_input()       |                  |               |
  |  |--------------------------------->|                  |               |
  |  |                 | FilterInput    |                  |               |
  |  |                 |                |  process()       |               |
  |  |                 |                |----------------->|               |
  |  |                 |                |  Verdict         |               |
  |  |                 |                |<-----------------|               |
  |  |                 |                |                  |               |
  |  |                 | correction_to_pauli_ops()         |               |
  |  |<---------------------------------|                  |               |
  |  |                 |                |                  |               |
  |  | decode(syndrome)|                |                  |               |
  |  |------------------------------------------------------------------>|
  |  | correction      |                |                  |               |
  |  |<------------------------------------------------------------------|
  |  |                 |                |                  |               |
  |  | check_logical_error()            |                  |               |
  |  |--+              |                |                  |               |
  |  |  | bool         |                |                  |               |
  |  |<-+              |                |                  |               |
  |  |                 |                |                  |               |
  | CycleReport       |                |                  |               |
  |<-+                 |                |                  |               |
```

### 3. WASM Deployment Flow

```
Browser JS          ruqu-wasm (WASM)       ruqu-core           Results
  |                      |                     |                   |
  | new WasmSimulator(n) |                     |                   |
  |--------------------->|                     |                   |
  |                      | QuantumState::new(n)|                   |
  |                      |-------------------->|                   |
  |                      | state               |                   |
  |                      |<--------------------|                   |
  |  WasmSimulator       |                     |                   |
  |<---------------------|                     |                   |
  |                      |                     |                   |
  | add_gate("h", 0)     |                     |                   |
  |--------------------->|                     |                   |
  |                      | circuit.add_gate()  |                   |
  |                      |-------------------->|                   |
  |  Ok                  |                     |                   |
  |<---------------------|                     |                   |
  |                      |                     |                   |
  | add_gate("cx", 1, 0) |                     |                   |
  |--------------------->|                     |                   |
  |                      | circuit.add_gate()  |                   |
  |                      |-------------------->|                   |
  |  Ok                  |                     |                   |
  |<---------------------|                     |                   |
  |                      |                     |                   |
  | run(1000)            |                     |                   |
  |--------------------->|                     |                   |
  |                      | session.start()     |                   |
  |                      |-------------------->|                   |
  |                      | run_to_completion() |                   |
  |                      |-------------------->|                   |
  |                      |                     | [gate loop]       |
  |                      |                     |---+               |
  |                      |                     |   | apply_gate()  |
  |                      |                     |<--+               |
  |                      |                     | measure()         |
  |                      |                     |---+               |
  |                      |                     |   | outcomes      |
  |                      |                     |<--+               |
  |                      | SimulationMetrics   |                   |
  |                      |<--------------------|                   |
  |                      |                     |                   |
  |                      | JSON.serialize(counts)                  |
  |                      |---------------------------------------->|
  |  "{\"00\": 503, \"11\": 497}"              |                   |
  |<---------------------|                     |                   |
  |                      |                     |                   |
  | [JS callback with results]                 |                   |
  |                      |                     |                   |
```

---

## Migration Strategy

### Phase 1: Standalone ruqu-core

**Goal**: A self-contained crate with no external dependencies except `ruvector-math`.

- Implement `QuantumCircuit`, `QuantumState`, `SimulationSession` aggregates
- Implement `CircuitBuilder`, `GateFusionService`, `NoiseInjectionService`
- All value objects and domain events defined
- Unit tests and property-based tests for normalization, gate unitarity
- No coherence bridge, no agent integration, no WASM

**Dependency**: `ruvector-math` (shared kernel only)

### Phase 2: ruqu-algorithms + Coherence Integration

**Goal**: Add VQE, surface code experiments, and the coherence bridge.

- Implement `VQEOptimization`, `SurfaceCodeExperiment` aggregates
- Implement `TensorNetworkState` for circuits exceeding state vector limits
- Build `CoherenceBridge` anti-corruption layer
- Integrate with ruQu `FilterPipeline` and `MWPMDecoder`
- Add `PauliExpectationService`, `ContractionPathOptimizer`
- Integration tests: VQE convergence, surface code logical error rate vs theory

**Dependencies**: `ruqu-core`, `ruvector-math`, `ruqu` (coherence bridge target)

### Phase 3: ruqu-wasm

**Goal**: Deploy to browser environments with graceful degradation.

- Implement `WasmSimulator` conformist wrapper
- Add `wasm-bindgen` API surface
- Enforce WASM constraints (25-qubit limit, no threads, no filesystem)
- JavaScript test harness running circuits in headless browser
- Performance benchmarks: gate throughput in WASM vs native

**Dependencies**: `ruqu-core`, `wasm-bindgen`, `wasm-pack`

### Phase 4: Full Agent System Integration

**Goal**: Complete customer-supplier integration with the claude-flow agent system.

- Implement `SimulationContract` trait and production adapter
- Add resource estimation and budget enforcement
- Implement `GraphQuantumPartnership` for QAOA/MaxCut
- Integration with `ruvector-graph` for graph problem decomposition
- End-to-end tests: agent builds circuit, runs simulation, acts on results
- OpenQASM import/export (published language)

**Dependencies**: All previous phases, `ruvector-graph`, `claude-flow` agent SDK

---

## References

1. Evans, E. (2003). "Domain-Driven Design: Tackling Complexity in the Heart of Software."
2. Vernon, V. (2013). "Implementing Domain-Driven Design." Chapter 13: Integrating Bounded Contexts.
3. Coherence Engine DDD: `docs/architecture/coherence-engine-ddd.md`
4. ruQu crate: `crates/ruQu/`
5. ruvector-math: shared kernel for SIMD and complex number operations
6. OpenQASM 3.0 specification: https://openqasm.com/
