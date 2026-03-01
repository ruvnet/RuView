# Quantum Simulation Engine: Domain-Driven Design - Tactical Design

**Version**: 0.1
**Date**: 2026-02-06
**Status**: Draft

---

## Overview

This document defines the tactical DDD patterns for the ruVector Quantum Simulation Engine (`ruqu-core`, `ruqu-algorithms`, `ruqu-wasm`). It specifies the Aggregates, Entities, Value Objects, Domain Events, Repositories, and Domain Services that compose the simulation domain. All type signatures target Rust and align with the conventions established in the existing `ruqu` crate and the coherence engine DDD.

---

## Value Objects

Value Objects are immutable, identity-less types compared by structural equality. They form the mathematical vocabulary of the quantum simulation domain.

### Qubit and Gate Primitives

```rust
/// Immutable qubit identifier. Valid range: 0..qubit_count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QubitIndex(pub u32);

/// Single complex amplitude for one basis state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Amplitude(pub Complex<f64>);

/// 2x2 unitary matrix for a single-qubit gate.
#[derive(Debug, Clone, PartialEq)]
pub struct GateMatrix(pub [[Complex<f64>; 2]; 2]);

/// 4x4 unitary matrix for a two-qubit gate.
#[derive(Debug, Clone, PartialEq)]
pub struct TwoQubitMatrix(pub [[Complex<f64>; 4]; 4]);

/// Individual Pauli operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PauliOp { I, X, Y, Z }

/// Tensor product of Pauli operators acting on consecutive qubits.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PauliString(pub Vec<PauliOp>);

/// Weighted sum of Pauli strings representing an observable.
#[derive(Debug, Clone, PartialEq)]
pub struct Hamiltonian(pub Vec<(f64, PauliString)>);
```

### Measurement and Outcome Types

```rust
/// Outcome of measuring a single qubit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasurementOutcome {
    pub qubit: QubitIndex,
    pub result: bool,
    pub probability: f64,
}

/// Classical syndrome extracted from a QEC cycle.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyndromeBits(pub Vec<bool>);

/// Named parameter binding for parametric circuits.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterBinding {
    pub name: String,
    pub value: f64,
}
```

### Metrics and Resource Types

```rust
/// Longest path through the circuit DAG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CircuitDepth(pub u32);

/// Gate fidelity score in the range [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct GateFidelity(pub f64);

/// Error probability per gate or per QEC cycle.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct NoiseRate(pub f64);

/// QAOA objective function value for a MaxCut instance.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct CutValue(pub f64);

/// Logical error rate measured as errors per QEC round.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct LogicalErrorRate(pub f64);

/// Memory required in bytes for a given simulation configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemoryEstimate(pub usize);

/// Maximum qubit count supported by the current platform/backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct QubitLimit(pub u32);

/// Estimated floating-point operations for a tensor contraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContractionCost(pub u64);

/// MPS bond dimension controlling truncation fidelity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BondDimension(pub u32);

/// Aggregate simulation performance metrics (immutable snapshot).
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationMetrics {
    pub qubits: u32,
    pub gates: u32,
    pub time_ms: f64,
    pub peak_memory: usize,
    pub gates_per_sec: f64,
}

/// Coherence score bridged from the ruQu coherence engine.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct CoherenceScore(pub f64);
```

### Invariant Enforcement

All value objects enforce their invariants at construction time:

```rust
impl GateFidelity {
    pub fn new(value: f64) -> Result<Self, DomainError> {
        if !(0.0..=1.0).contains(&value) {
            return Err(DomainError::InvalidFidelity(value));
        }
        Ok(Self(value))
    }
}

impl NoiseRate {
    pub fn new(value: f64) -> Result<Self, DomainError> {
        if !(0.0..=1.0).contains(&value) {
            return Err(DomainError::InvalidNoiseRate(value));
        }
        Ok(Self(value))
    }
}

impl QubitIndex {
    pub fn validate(self, qubit_count: u32) -> Result<(), DomainError> {
        if self.0 >= qubit_count {
            return Err(DomainError::QubitIndexOutOfRange {
                index: self,
                qubit_count,
            });
        }
        Ok(())
    }
}
```

---

## Aggregates

### 1. QuantumCircuit Aggregate

**Root Entity**: `QuantumCircuit`
**Contains**: `Vec<Gate>`, `qubit_count: u32`, `parameter_bindings: HashMap<String, f64>`

**Invariants**:
- All gate qubit indices reference valid qubits (0..qubit_count)
- Parameter names are unique across the circuit
- Circuit scheduling is acyclic (gates on the same qubits are totally ordered)
- Gate count does not exceed platform-specific limits

**Factory**: `CircuitBuilder` with fluent API.

```rust
/// A quantum circuit: the central description of a quantum computation.
pub struct QuantumCircuit {
    id: CircuitId,
    qubit_count: u32,
    gates: Vec<Gate>,
    parameter_bindings: HashMap<String, f64>,
    metadata: CircuitMetadata,
}

/// A single gate operation within a circuit.
#[derive(Debug, Clone)]
pub struct Gate {
    pub gate_type: GateType,
    pub target: QubitIndex,
    pub control: Option<QubitIndex>,
    pub matrix: GateMatrix,
    pub parameter: Option<String>,
}

/// Supported gate types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GateType {
    H, X, Y, Z, S, T, Rx, Ry, Rz, CNOT, CZ, SWAP, Toffoli, Custom,
}

impl QuantumCircuit {
    /// Add a gate, enforcing qubit index validity.
    pub fn add_gate(&mut self, gate: Gate) -> Result<(), DomainError> {
        gate.target.validate(self.qubit_count)?;
        if let Some(ctrl) = gate.control {
            ctrl.validate(self.qubit_count)?;
            if ctrl == gate.target {
                return Err(DomainError::SelfControlGate);
            }
        }
        self.gates.push(gate);
        Ok(())
    }

    /// Bind a parameter value by name.
    pub fn set_parameter(&mut self, name: &str, value: f64) -> Result<(), DomainError> {
        if !self.has_parameter(name) {
            return Err(DomainError::UnknownParameter(name.to_string()));
        }
        self.parameter_bindings.insert(name.to_string(), value);
        Ok(())
    }

    /// Return circuit depth as the longest gate chain.
    pub fn depth(&self) -> CircuitDepth {
        CircuitDepth(self.compute_dag_depth())
    }

    /// Apply gate fusion optimization, combining adjacent single-qubit gates.
    pub fn fuse_gates(&mut self) -> usize {
        let original_count = self.gates.len();
        self.gates = GateFusionService::fuse(&self.gates);
        original_count - self.gates.len()
    }

    /// Full optimization pass: fusion, cancellation, commutation.
    pub fn optimize(&mut self) -> OptimizationReport {
        let before = self.gates.len();
        self.fuse_gates();
        // Additional passes: gate cancellation, commutation reordering
        OptimizationReport {
            gates_before: before as u32,
            gates_after: self.gates.len() as u32,
            depth_before: CircuitDepth(0), // placeholder
            depth_after: self.depth(),
        }
    }

    fn has_parameter(&self, name: &str) -> bool {
        self.gates.iter().any(|g| g.parameter.as_deref() == Some(name))
    }

    fn compute_dag_depth(&self) -> u32 {
        // DAG-based depth computation tracking per-qubit depth
        let mut qubit_depth = vec![0u32; self.qubit_count as usize];
        for gate in &self.gates {
            let t = gate.target.0 as usize;
            let d = if let Some(c) = gate.control {
                qubit_depth[t].max(qubit_depth[c.0 as usize]) + 1
            } else {
                qubit_depth[t] + 1
            };
            qubit_depth[t] = d;
            if let Some(c) = gate.control {
                qubit_depth[c.0 as usize] = d;
            }
        }
        qubit_depth.into_iter().max().unwrap_or(0)
    }
}

/// Fluent builder for constructing circuits.
pub struct CircuitBuilder {
    qubit_count: u32,
    gates: Vec<Gate>,
}

impl CircuitBuilder {
    pub fn new(qubit_count: u32) -> Self {
        Self { qubit_count, gates: Vec::new() }
    }

    pub fn h(mut self, target: u32) -> Self {
        self.gates.push(Gate::hadamard(QubitIndex(target)));
        self
    }

    pub fn cx(mut self, control: u32, target: u32) -> Self {
        self.gates.push(Gate::cnot(QubitIndex(control), QubitIndex(target)));
        self
    }

    pub fn rz(mut self, target: u32, param_name: &str) -> Self {
        self.gates.push(Gate::rz(QubitIndex(target), param_name.to_string()));
        self
    }

    pub fn build(self) -> Result<QuantumCircuit, DomainError> {
        let mut circuit = QuantumCircuit {
            id: CircuitId::new(),
            qubit_count: self.qubit_count,
            gates: Vec::new(),
            parameter_bindings: HashMap::new(),
            metadata: CircuitMetadata::default(),
        };
        for gate in self.gates {
            circuit.add_gate(gate)?;
        }
        Ok(circuit)
    }
}
```

---

### 2. QuantumState Aggregate

**Root Entity**: `QuantumState`
**Contains**: `state_vector: Vec<Complex<f64>>`, `qubit_count: u32`, `entanglement_map: EntanglementMap`

**Invariants**:
- State vector is normalized: sum of |amplitude|^2 = 1.0 (within epsilon)
- `qubit_count` matches `log2(state_vector.len())`
- State vector length is always a power of two

**Factory**: `QuantumState::new(n)` initializes the |00...0> computational basis state.

```rust
/// Full state vector representation of a quantum register.
pub struct QuantumState {
    state_vector: Vec<Complex<f64>>,
    qubit_count: u32,
    entanglement_map: EntanglementMap,
}

/// Tracks pairwise entanglement between qubits.
#[derive(Debug, Clone, Default)]
pub struct EntanglementMap {
    /// Adjacency set: (qubit_a, qubit_b) pairs known to be entangled.
    pairs: HashSet<(QubitIndex, QubitIndex)>,
}

impl QuantumState {
    /// Initialize |00...0> state for n qubits.
    pub fn new(qubit_count: u32) -> Result<Self, DomainError> {
        if qubit_count > 30 {
            return Err(DomainError::QubitLimitExceeded {
                requested: qubit_count,
                limit: QubitLimit(30),
            });
        }
        let dim = 1usize << qubit_count;
        let mut sv = vec![Complex::new(0.0, 0.0); dim];
        sv[0] = Complex::new(1.0, 0.0);
        Ok(Self {
            state_vector: sv,
            qubit_count,
            entanglement_map: EntanglementMap::default(),
        })
    }

    /// Apply a single-qubit gate to the state.
    pub fn apply_gate(&mut self, gate: &Gate) -> Result<(), DomainError> {
        gate.target.validate(self.qubit_count)?;
        match gate.control {
            None => self.apply_single_qubit(gate.target, &gate.matrix),
            Some(ctrl) => {
                ctrl.validate(self.qubit_count)?;
                self.apply_controlled(ctrl, gate.target, &gate.matrix);
                self.entanglement_map.mark_entangled(ctrl, gate.target);
            }
        }
        Ok(())
    }

    /// Measure a single qubit, collapsing the state.
    pub fn measure(&mut self, qubit: QubitIndex) -> Result<MeasurementOutcome, DomainError> {
        qubit.validate(self.qubit_count)?;
        let prob_one = self.probability_of_one(qubit);
        let result = rand::random::<f64>() < prob_one;
        self.collapse(qubit, result);
        self.renormalize();
        Ok(MeasurementOutcome {
            qubit,
            result,
            probability: if result { prob_one } else { 1.0 - prob_one },
        })
    }

    /// Reset a qubit to |0>, disentangling it from the register.
    pub fn reset_qubit(&mut self, qubit: QubitIndex) -> Result<(), DomainError> {
        qubit.validate(self.qubit_count)?;
        self.collapse(qubit, false);
        self.renormalize();
        self.entanglement_map.remove_qubit(qubit);
        Ok(())
    }

    /// Compute expectation value <psi|H|psi> for a Hamiltonian.
    pub fn expectation_value(&self, hamiltonian: &Hamiltonian) -> f64 {
        hamiltonian.0.iter()
            .map(|(coeff, pauli)| coeff * self.pauli_expectation(pauli))
            .sum()
    }

    /// Verify normalization invariant.
    pub fn is_normalized(&self, epsilon: f64) -> bool {
        let norm_sq: f64 = self.state_vector.iter()
            .map(|a| a.norm_sqr())
            .sum();
        (norm_sq - 1.0).abs() < epsilon
    }

    /// Memory estimate for this state.
    pub fn memory_estimate(&self) -> MemoryEstimate {
        MemoryEstimate(self.state_vector.len() * std::mem::size_of::<Complex<f64>>())
    }

    fn probability_of_one(&self, qubit: QubitIndex) -> f64 {
        let mask = 1usize << qubit.0;
        self.state_vector.iter().enumerate()
            .filter(|(i, _)| i & mask != 0)
            .map(|(_, a)| a.norm_sqr())
            .sum()
    }

    fn collapse(&mut self, qubit: QubitIndex, to_one: bool) {
        let mask = 1usize << qubit.0;
        for (i, amp) in self.state_vector.iter_mut().enumerate() {
            let is_one = (i & mask) != 0;
            if is_one != to_one {
                *amp = Complex::new(0.0, 0.0);
            }
        }
    }

    fn renormalize(&mut self) {
        let norm: f64 = self.state_vector.iter().map(|a| a.norm_sqr()).sum::<f64>().sqrt();
        if norm > 1e-15 {
            for amp in &mut self.state_vector {
                *amp /= norm;
            }
        }
    }

    fn apply_single_qubit(&mut self, target: QubitIndex, matrix: &GateMatrix) { /* ... */ }
    fn apply_controlled(&mut self, ctrl: QubitIndex, target: QubitIndex, matrix: &GateMatrix) { /* ... */ }
    fn pauli_expectation(&self, pauli: &PauliString) -> f64 { /* ... */ 0.0 }
}
```

---

### 3. SimulationSession Aggregate

**Root Entity**: `SimulationSession`
**Contains**: `circuit: QuantumCircuit`, `state: QuantumState`, `backend_config: BackendConfig`, `metrics: Vec<SimulationMetrics>`, `measurement_record: Vec<MeasurementOutcome>`

**Invariants**:
- Session lifecycle is linear: Created -> Running -> Completed | Failed
- Resources (state vector memory) are allocated only during the Running state
- Circuit qubit count matches state qubit count
- Backend configuration is immutable after the session enters Running

```rust
/// Lifecycle states for a simulation session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Created,
    Running,
    Completed,
    Failed,
}

/// Configuration for the simulation backend.
#[derive(Debug, Clone)]
pub struct BackendConfig {
    pub backend_type: BackendType,
    pub max_memory_bytes: usize,
    pub thread_count: usize,
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    StateVector,
    TensorNetwork,
    Stabilizer,
}

/// A single simulation execution session.
pub struct SimulationSession {
    id: SessionId,
    circuit: QuantumCircuit,
    state: Option<QuantumState>,
    backend_config: BackendConfig,
    status: SessionStatus,
    metrics: Vec<SimulationMetrics>,
    measurement_record: Vec<MeasurementOutcome>,
    step_index: usize,
    events: Vec<DomainEvent>,
}

impl SimulationSession {
    pub fn new(circuit: QuantumCircuit, config: BackendConfig) -> Self {
        Self {
            id: SessionId::new(),
            circuit,
            state: None,
            backend_config: config,
            status: SessionStatus::Created,
            metrics: Vec::new(),
            measurement_record: Vec::new(),
            step_index: 0,
            events: Vec::new(),
        }
    }

    /// Transition Created -> Running. Allocates state vector.
    pub fn start(&mut self) -> Result<(), DomainError> {
        if self.status != SessionStatus::Created {
            return Err(DomainError::InvalidSessionTransition {
                from: self.status,
                to: SessionStatus::Running,
            });
        }
        let state = QuantumState::new(self.circuit.qubit_count)?;
        self.state = Some(state);
        self.status = SessionStatus::Running;
        self.events.push(DomainEvent::SimulationStarted {
            session_id: self.id,
            qubits: self.circuit.qubit_count,
        });
        Ok(())
    }

    /// Apply the next gate in the circuit, returning any measurement.
    pub fn step(&mut self) -> Result<Option<MeasurementOutcome>, DomainError> {
        self.assert_running()?;
        let state = self.state.as_mut().unwrap();
        if self.step_index >= self.circuit.gates.len() {
            self.status = SessionStatus::Completed;
            self.events.push(DomainEvent::SimulationCompleted {
                session_id: self.id,
                total_gates: self.circuit.gates.len() as u32,
            });
            return Ok(None);
        }
        let gate = &self.circuit.gates[self.step_index];
        state.apply_gate(gate)?;
        self.step_index += 1;
        self.events.push(DomainEvent::GateApplied {
            session_id: self.id,
            gate_index: self.step_index as u32 - 1,
        });
        Ok(None)
    }

    /// Run all remaining gates to completion.
    pub fn run_to_completion(&mut self) -> Result<SimulationMetrics, DomainError> {
        self.assert_running()?;
        let start = std::time::Instant::now();
        while self.status == SessionStatus::Running {
            self.step()?;
        }
        let elapsed = start.elapsed();
        let total_gates = self.circuit.gates.len() as u32;
        let metrics = SimulationMetrics {
            qubits: self.circuit.qubit_count,
            gates: total_gates,
            time_ms: elapsed.as_secs_f64() * 1000.0,
            peak_memory: self.state.as_ref().map(|s| s.memory_estimate().0).unwrap_or(0),
            gates_per_sec: total_gates as f64 / elapsed.as_secs_f64(),
        };
        self.metrics.push(metrics.clone());
        Ok(metrics)
    }

    /// Abort a running session, transitioning to Failed.
    pub fn abort(&mut self, reason: &str) -> Result<(), DomainError> {
        self.assert_running()?;
        self.status = SessionStatus::Failed;
        self.state = None; // Release memory
        self.events.push(DomainEvent::SimulationFailed {
            session_id: self.id,
            reason: reason.to_string(),
        });
        Ok(())
    }

    /// Drain pending domain events.
    pub fn take_events(&mut self) -> Vec<DomainEvent> {
        std::mem::take(&mut self.events)
    }

    fn assert_running(&self) -> Result<(), DomainError> {
        if self.status != SessionStatus::Running {
            return Err(DomainError::SessionNotRunning(self.status));
        }
        Ok(())
    }
}
```

---

### 4. VQEOptimization Aggregate

**Root Entity**: `VQEOptimization`
**Contains**: `ansatz_circuit: QuantumCircuit`, `hamiltonian: Hamiltonian`, `optimizer_state: OptimizerState`, `iteration_history: Vec<VQEIteration>`

**Invariants**:
- Parameter count of the ansatz circuit matches the optimizer dimension
- Energy values are real (imaginary part zero within tolerance)
- Convergence criteria are checked after each iteration

```rust
/// A single VQE iteration record.
#[derive(Debug, Clone)]
pub struct VQEIteration {
    pub iteration: u32,
    pub parameters: Vec<f64>,
    pub energy: f64,
    pub gradient_norm: f64,
}

/// Classical optimizer state.
pub struct OptimizerState {
    pub method: OptimizerMethod,
    pub learning_rate: f64,
    pub momentum: Vec<f64>,
    pub velocity: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizerMethod { GradientDescent, Adam, COBYLA, SPSA }

/// VQE variational optimization session.
pub struct VQEOptimization {
    id: VQEId,
    ansatz: QuantumCircuit,
    hamiltonian: Hamiltonian,
    optimizer: OptimizerState,
    history: Vec<VQEIteration>,
    convergence_threshold: f64,
    max_iterations: u32,
    events: Vec<DomainEvent>,
}

impl VQEOptimization {
    pub fn new(
        ansatz: QuantumCircuit,
        hamiltonian: Hamiltonian,
        method: OptimizerMethod,
        convergence_threshold: f64,
        max_iterations: u32,
    ) -> Result<Self, DomainError> {
        let param_count = ansatz.parameter_count();
        Ok(Self {
            id: VQEId::new(),
            ansatz,
            hamiltonian,
            optimizer: OptimizerState::new(method, param_count),
            history: Vec::new(),
            convergence_threshold,
            max_iterations,
            events: Vec::new(),
        })
    }

    /// Evaluate the energy at the current parameter values.
    pub fn evaluate_energy(&self, params: &[f64]) -> Result<f64, DomainError> {
        let mut circuit = self.ansatz.clone();
        circuit.bind_all_parameters(params)?;
        let mut state = QuantumState::new(circuit.qubit_count)?;
        for gate in &circuit.gates {
            state.apply_gate(gate)?;
        }
        Ok(state.expectation_value(&self.hamiltonian))
    }

    /// Compute the parameter gradient using the parameter-shift rule.
    pub fn compute_gradient(&self, params: &[f64]) -> Result<Vec<f64>, DomainError> {
        let shift = std::f64::consts::FRAC_PI_2;
        let mut gradient = vec![0.0; params.len()];
        for i in 0..params.len() {
            let mut params_plus = params.to_vec();
            let mut params_minus = params.to_vec();
            params_plus[i] += shift;
            params_minus[i] -= shift;
            let e_plus = self.evaluate_energy(&params_plus)?;
            let e_minus = self.evaluate_energy(&params_minus)?;
            gradient[i] = (e_plus - e_minus) / 2.0;
        }
        Ok(gradient)
    }

    /// Run one optimization iteration.
    pub fn iterate(&mut self) -> Result<VQEIteration, DomainError> {
        let params = self.optimizer.current_parameters();
        let energy = self.evaluate_energy(&params)?;
        let gradient = self.compute_gradient(&params)?;
        let grad_norm = gradient.iter().map(|g| g * g).sum::<f64>().sqrt();
        self.optimizer.update(&gradient);
        let iteration = VQEIteration {
            iteration: self.history.len() as u32,
            parameters: params,
            energy,
            gradient_norm: grad_norm,
        };
        self.history.push(iteration.clone());
        self.events.push(DomainEvent::VQEIterationCompleted {
            vqe_id: self.id,
            iteration: iteration.iteration,
            energy,
        });
        if let Some(prev) = self.history.iter().rev().nth(1) {
            if prev.energy > energy {
                self.events.push(DomainEvent::VQEEnergyImproved {
                    vqe_id: self.id,
                    previous: prev.energy,
                    current: energy,
                });
            }
        }
        Ok(iteration)
    }

    /// Run iterations until convergence or max_iterations.
    pub fn converge(&mut self) -> Result<f64, DomainError> {
        for _ in 0..self.max_iterations {
            let iter = self.iterate()?;
            if iter.gradient_norm < self.convergence_threshold {
                self.events.push(DomainEvent::VQEConverged {
                    vqe_id: self.id,
                    final_energy: iter.energy,
                    iterations: iter.iteration,
                });
                return Ok(iter.energy);
            }
        }
        Err(DomainError::VQEDidNotConverge {
            iterations: self.max_iterations,
        })
    }
}
```

---

### 5. SurfaceCodeExperiment Aggregate

**Root Entity**: `SurfaceCodeExperiment`
**Contains**: `code_distance: u32`, `noise_model: NoiseModel`, `decoder: Box<dyn Decoder>`, `cycle_count: u32`, `error_log: Vec<ErrorEvent>`

**Invariants**:
- Decoder is compatible with the configured code distance
- Noise parameters are within valid probability ranges [0.0, 1.0]
- Cycle count advances monotonically

```rust
/// Noise model for stochastic error injection.
#[derive(Debug, Clone)]
pub struct NoiseModel {
    pub depolarizing_rate: NoiseRate,
    pub measurement_error_rate: NoiseRate,
    pub idle_error_rate: NoiseRate,
}

/// Trait for QEC decoders.
pub trait Decoder: Send {
    fn decode(&self, syndrome: &SyndromeBits) -> Vec<PauliOp>;
    fn code_distance(&self) -> u32;
}

/// An error event recorded during a QEC experiment.
#[derive(Debug, Clone)]
pub struct ErrorEvent {
    pub cycle: u32,
    pub error_type: ErrorType,
    pub qubits: Vec<QubitIndex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorType { DataError, MeasurementError, LogicalError }

/// Surface code QEC experiment.
pub struct SurfaceCodeExperiment {
    id: ExperimentId,
    code_distance: u32,
    noise_model: NoiseModel,
    decoder: Box<dyn Decoder>,
    cycle_count: u32,
    error_log: Vec<ErrorEvent>,
    logical_error_count: u32,
    events: Vec<DomainEvent>,
}

impl SurfaceCodeExperiment {
    pub fn new(
        code_distance: u32,
        noise_model: NoiseModel,
        decoder: Box<dyn Decoder>,
    ) -> Result<Self, DomainError> {
        if decoder.code_distance() != code_distance {
            return Err(DomainError::DecoderDistanceMismatch {
                decoder: decoder.code_distance(),
                experiment: code_distance,
            });
        }
        Ok(Self {
            id: ExperimentId::new(),
            code_distance,
            noise_model,
            decoder,
            cycle_count: 0,
            error_log: Vec::new(),
            logical_error_count: 0,
            events: Vec::new(),
        })
    }

    /// Run one QEC cycle: inject errors, extract syndrome, decode, check logical.
    pub fn run_cycle(&mut self) -> Result<CycleReport, DomainError> {
        self.cycle_count += 1;
        let errors = self.inject_errors();
        let syndrome = self.extract_syndrome(&errors);
        let correction = self.decode(&syndrome);
        let logical_error = self.check_logical_error(&errors, &correction);
        if logical_error {
            self.logical_error_count += 1;
            self.events.push(DomainEvent::LogicalErrorDetected {
                experiment_id: self.id,
                cycle: self.cycle_count,
            });
        }
        self.events.push(DomainEvent::SurfaceCodeCycleCompleted {
            experiment_id: self.id,
            cycle: self.cycle_count,
            syndrome_weight: syndrome.0.iter().filter(|&&b| b).count() as u32,
        });
        Ok(CycleReport {
            cycle: self.cycle_count,
            syndrome,
            correction,
            logical_error,
        })
    }

    /// Inject stochastic errors based on the noise model.
    pub fn inject_errors(&mut self) -> Vec<ErrorEvent> {
        let mut errors = Vec::new();
        let data_qubit_count = self.code_distance * self.code_distance;
        for q in 0..data_qubit_count {
            if rand::random::<f64>() < self.noise_model.depolarizing_rate.0 {
                let event = ErrorEvent {
                    cycle: self.cycle_count,
                    error_type: ErrorType::DataError,
                    qubits: vec![QubitIndex(q)],
                };
                self.error_log.push(event.clone());
                errors.push(event);
            }
        }
        errors
    }

    /// Extract syndrome bits from the current error configuration.
    pub fn extract_syndrome(&self, errors: &[ErrorEvent]) -> SyndromeBits {
        let stabilizer_count = 2 * (self.code_distance - 1) * self.code_distance;
        let mut bits = vec![false; stabilizer_count as usize];
        for error in errors {
            for qubit in &error.qubits {
                let affected = self.stabilizers_for_qubit(*qubit);
                for s in affected {
                    bits[s] ^= true;
                }
            }
        }
        self.events.last_mut().map(|_| {
            // SyndromeExtracted event appended in run_cycle
        });
        SyndromeBits(bits)
    }

    /// Apply the decoder to a syndrome.
    pub fn decode(&self, syndrome: &SyndromeBits) -> Vec<PauliOp> {
        self.decoder.decode(syndrome)
    }

    /// Compute the logical error rate over all cycles.
    pub fn logical_error_rate(&self) -> LogicalErrorRate {
        if self.cycle_count == 0 {
            return LogicalErrorRate(0.0);
        }
        LogicalErrorRate(self.logical_error_count as f64 / self.cycle_count as f64)
    }

    /// Track whether a logical error occurred after correction.
    pub fn track_logical_error(&mut self, errors: &[ErrorEvent], correction: &[PauliOp]) -> bool {
        self.check_logical_error(errors, correction)
    }

    fn check_logical_error(&self, _errors: &[ErrorEvent], _correction: &[PauliOp]) -> bool {
        // Computes residual Pauli frame and checks logical operator commutation
        false // Placeholder: actual implementation checks logical X/Z operators
    }

    fn stabilizers_for_qubit(&self, _qubit: QubitIndex) -> Vec<usize> {
        Vec::new() // Placeholder: returns stabilizer indices adjacent to qubit
    }
}
```

---

### 6. TensorNetworkState Aggregate

**Root Entity**: `TensorNetworkState`
**Contains**: `tensor_list: Vec<Tensor>`, `contraction_path: ContractionPath`, `bond_dimensions: Vec<BondDimension>`, `qubit_mapping: HashMap<QubitIndex, TensorIndex>`

**Invariants**:
- Tensor indices are consistent across all contractions (no dangling indices)
- Bond dimensions remain within the configured budget
- Qubit mapping is bijective (each qubit maps to exactly one tensor site)

```rust
/// A single tensor in the network.
#[derive(Debug, Clone)]
pub struct Tensor {
    pub data: Vec<Complex<f64>>,
    pub shape: Vec<usize>,
    pub indices: Vec<TensorIndex>,
}

/// Index identifying a tensor leg (bond or physical).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TensorIndex(pub u32);

/// An ordered sequence of pairwise contractions.
#[derive(Debug, Clone)]
pub struct ContractionPath(pub Vec<(TensorIndex, TensorIndex)>);

/// Tensor network representation of a quantum state.
pub struct TensorNetworkState {
    tensors: Vec<Tensor>,
    contraction_path: ContractionPath,
    bond_dimensions: Vec<BondDimension>,
    qubit_mapping: HashMap<QubitIndex, TensorIndex>,
    max_bond_dim: BondDimension,
}

impl TensorNetworkState {
    /// Create from an MPS (Matrix Product State) initialization.
    pub fn new_mps(qubit_count: u32, max_bond_dim: BondDimension) -> Self {
        let mut tensors = Vec::new();
        let mut qubit_mapping = HashMap::new();
        for q in 0..qubit_count {
            let idx = TensorIndex(q);
            tensors.push(Tensor::identity_site(idx));
            qubit_mapping.insert(QubitIndex(q), idx);
        }
        Self {
            tensors,
            contraction_path: ContractionPath(Vec::new()),
            bond_dimensions: vec![BondDimension(1); qubit_count as usize],
            qubit_mapping,
            max_bond_dim,
        }
    }

    /// Absorb a gate as a tensor into the network.
    pub fn add_gate_tensor(&mut self, gate: &Gate) -> Result<(), DomainError> {
        let target_idx = self.qubit_mapping.get(&gate.target)
            .ok_or(DomainError::UnmappedQubit(gate.target))?;
        let gate_tensor = Tensor::from_gate(gate, *target_idx);
        self.tensors.push(gate_tensor);
        Ok(())
    }

    /// Contract the network along the stored contraction path.
    pub fn contract(&mut self) -> Result<Vec<Complex<f64>>, DomainError> {
        for (i, j) in &self.contraction_path.0 {
            self.contract_pair(*i, *j)?;
        }
        Ok(self.final_tensor_data())
    }

    /// Truncate bond dimensions via SVD approximation.
    pub fn approximate(&mut self, target_bond_dim: BondDimension) -> f64 {
        let mut total_truncation_error = 0.0;
        for bond in &mut self.bond_dimensions {
            if bond.0 > target_bond_dim.0 {
                total_truncation_error += self.truncate_bond(bond, target_bond_dim);
                *bond = target_bond_dim;
            }
        }
        total_truncation_error
    }

    /// Convert the tensor network back to a full state vector.
    pub fn to_state_vector(&mut self) -> Result<Vec<Complex<f64>>, DomainError> {
        self.contract()
    }

    /// Estimated contraction cost in FLOPs.
    pub fn contraction_cost(&self) -> ContractionCost {
        ContractionCost(ContractionPathOptimizer::estimate_cost(
            &self.tensors,
            &self.contraction_path,
        ))
    }

    fn contract_pair(&mut self, _i: TensorIndex, _j: TensorIndex) -> Result<(), DomainError> {
        Ok(()) // Implementation contracts two tensors along shared indices
    }

    fn truncate_bond(&self, _bond: &BondDimension, _target: BondDimension) -> f64 {
        0.0 // Implementation performs SVD truncation, returns discarded weight
    }

    fn final_tensor_data(&self) -> Vec<Complex<f64>> {
        Vec::new() // Placeholder
    }
}
```

---

## Domain Events

All domain events are immutable records of state transitions. They drive cross-aggregate communication and integration with the ruQu coherence engine.

| Event | Payload | Produced By | Consumed By |
|-------|---------|-------------|-------------|
| `CircuitCreated` | `{ circuit_id, qubit_count }` | QuantumCircuit | SimulationSession |
| `GateAdded` | `{ circuit_id, gate_index, gate_type }` | QuantumCircuit | Metrics |
| `CircuitOptimized` | `{ circuit_id, gates_removed }` | QuantumCircuit | Metrics |
| `SimulationStarted` | `{ session_id, qubits }` | SimulationSession | Monitoring, MemoryTracker |
| `GateApplied` | `{ session_id, gate_index }` | SimulationSession | Metrics |
| `MeasurementPerformed` | `{ session_id, qubit, result, probability }` | SimulationSession | VQE, SurfaceCode |
| `SimulationCompleted` | `{ session_id, total_gates }` | SimulationSession | ResultRepository |
| `SimulationFailed` | `{ session_id, reason }` | SimulationSession | Monitoring |
| `VQEIterationCompleted` | `{ vqe_id, iteration, energy }` | VQEOptimization | Monitoring |
| `VQEEnergyImproved` | `{ vqe_id, previous, current }` | VQEOptimization | Logging |
| `VQEConverged` | `{ vqe_id, final_energy, iterations }` | VQEOptimization | Agent System |
| `SurfaceCodeCycleCompleted` | `{ experiment_id, cycle, syndrome_weight }` | SurfaceCodeExperiment | Monitoring |
| `SyndromeExtracted` | `{ experiment_id, cycle, bits }` | SurfaceCodeExperiment | CoherenceBridge |
| `LogicalErrorDetected` | `{ experiment_id, cycle }` | SurfaceCodeExperiment | Alerting |
| `MemoryAllocated` | `{ session_id, bytes }` | QuantumState | ResourceManager |
| `MemoryReleased` | `{ session_id, bytes }` | SimulationSession | ResourceManager |
| `BackendSwitched` | `{ session_id, from, to }` | SimulationSession | Monitoring |

```rust
/// All domain events in the quantum simulation engine.
#[derive(Debug, Clone)]
pub enum DomainEvent {
    CircuitCreated { circuit_id: CircuitId, qubit_count: u32 },
    GateAdded { circuit_id: CircuitId, gate_index: u32, gate_type: GateType },
    CircuitOptimized { circuit_id: CircuitId, gates_removed: u32 },
    SimulationStarted { session_id: SessionId, qubits: u32 },
    GateApplied { session_id: SessionId, gate_index: u32 },
    MeasurementPerformed { session_id: SessionId, outcome: MeasurementOutcome },
    SimulationCompleted { session_id: SessionId, total_gates: u32 },
    SimulationFailed { session_id: SessionId, reason: String },
    VQEIterationCompleted { vqe_id: VQEId, iteration: u32, energy: f64 },
    VQEEnergyImproved { vqe_id: VQEId, previous: f64, current: f64 },
    VQEConverged { vqe_id: VQEId, final_energy: f64, iterations: u32 },
    SurfaceCodeCycleCompleted { experiment_id: ExperimentId, cycle: u32, syndrome_weight: u32 },
    SyndromeExtracted { experiment_id: ExperimentId, cycle: u32, bits: SyndromeBits },
    LogicalErrorDetected { experiment_id: ExperimentId, cycle: u32 },
    MemoryAllocated { session_id: SessionId, bytes: usize },
    MemoryReleased { session_id: SessionId, bytes: usize },
    BackendSwitched { session_id: SessionId, from: BackendType, to: BackendType },
}
```

---

## Domain Services

Domain services encapsulate logic that does not naturally belong to a single aggregate.

### GateFusionService

Combines consecutive single-qubit gates on the same qubit into a single fused unitary matrix, reducing circuit depth and simulation time.

```rust
pub struct GateFusionService;

impl GateFusionService {
    /// Fuse consecutive single-qubit gates targeting the same qubit.
    pub fn fuse(gates: &[Gate]) -> Vec<Gate> {
        let mut result = Vec::new();
        let mut i = 0;
        while i < gates.len() {
            if gates[i].control.is_none() {
                let mut fused_matrix = gates[i].matrix.clone();
                let target = gates[i].target;
                let mut j = i + 1;
                while j < gates.len()
                    && gates[j].control.is_none()
                    && gates[j].target == target
                {
                    fused_matrix = GateMatrix::multiply(&gates[j].matrix, &fused_matrix);
                    j += 1;
                }
                result.push(Gate {
                    gate_type: GateType::Custom,
                    target,
                    control: None,
                    matrix: fused_matrix,
                    parameter: None,
                });
                i = j;
            } else {
                result.push(gates[i].clone());
                i += 1;
            }
        }
        result
    }
}
```

### EntanglementAnalysisService

Tracks qubit connectivity and suggests state-splitting boundaries for tensor network backends.

```rust
pub struct EntanglementAnalysisService;

impl EntanglementAnalysisService {
    /// Compute the entanglement graph from the circuit.
    pub fn connectivity_graph(circuit: &QuantumCircuit) -> HashMap<QubitIndex, HashSet<QubitIndex>> {
        let mut graph: HashMap<QubitIndex, HashSet<QubitIndex>> = HashMap::new();
        for gate in &circuit.gates {
            if let Some(ctrl) = gate.control {
                graph.entry(gate.target).or_default().insert(ctrl);
                graph.entry(ctrl).or_default().insert(gate.target);
            }
        }
        graph
    }

    /// Suggest partition points for splitting state into tensor subnetworks.
    pub fn suggest_partitions(
        circuit: &QuantumCircuit,
        max_partition_size: u32,
    ) -> Vec<Vec<QubitIndex>> {
        let graph = Self::connectivity_graph(circuit);
        // Greedy partitioning based on connectivity
        Self::greedy_partition(&graph, max_partition_size)
    }

    fn greedy_partition(
        _graph: &HashMap<QubitIndex, HashSet<QubitIndex>>,
        _max_size: u32,
    ) -> Vec<Vec<QubitIndex>> {
        Vec::new() // Implementation uses graph partitioning heuristics
    }
}
```

### ContractionPathOptimizer

Finds optimal or near-optimal tensor contraction orderings to minimize total FLOP count.

```rust
pub struct ContractionPathOptimizer;

impl ContractionPathOptimizer {
    /// Find a contraction path minimizing estimated FLOPs.
    pub fn optimize(tensors: &[Tensor]) -> ContractionPath {
        if tensors.len() <= 10 {
            Self::exhaustive_search(tensors)
        } else {
            Self::greedy_search(tensors)
        }
    }

    /// Estimate total cost of a given contraction path.
    pub fn estimate_cost(tensors: &[Tensor], path: &ContractionPath) -> u64 {
        let mut cost = 0u64;
        for (i, j) in &path.0 {
            cost += Self::pairwise_cost(tensors, *i, *j);
        }
        cost
    }

    fn exhaustive_search(_tensors: &[Tensor]) -> ContractionPath { ContractionPath(Vec::new()) }
    fn greedy_search(_tensors: &[Tensor]) -> ContractionPath { ContractionPath(Vec::new()) }
    fn pairwise_cost(_tensors: &[Tensor], _i: TensorIndex, _j: TensorIndex) -> u64 { 0 }
}
```

### PauliExpectationService

Efficiently computes expectation values of Pauli string observables, with grouping for commuting terms.

```rust
pub struct PauliExpectationService;

impl PauliExpectationService {
    /// Group commuting Pauli terms to minimize measurement overhead.
    pub fn group_commuting(hamiltonian: &Hamiltonian) -> Vec<Vec<(f64, PauliString)>> {
        // Greedy coloring of the non-commutativity graph
        Vec::new() // Implementation groups qubit-wise commuting terms
    }

    /// Compute expectation of a single Pauli string on a state vector.
    pub fn expectation(state: &[Complex<f64>], pauli: &PauliString) -> f64 {
        // Apply Pauli string as a diagonal/permutation operator
        let n = (state.len() as f64).log2() as u32;
        let mut result = 0.0;
        for (i, amp) in state.iter().enumerate() {
            let phase = Self::pauli_phase(i, n, pauli);
            let j = Self::pauli_permute(i, n, pauli);
            result += (amp.conj() * phase * state[j]).re;
        }
        result
    }

    fn pauli_phase(_basis: usize, _n: u32, _pauli: &PauliString) -> Complex<f64> {
        Complex::new(1.0, 0.0) // Placeholder
    }

    fn pauli_permute(basis: usize, _n: u32, _pauli: &PauliString) -> usize {
        basis // Placeholder: applies X/Y bit flips
    }
}
```

### NoiseInjectionService

Applies stochastic noise channels (depolarizing, amplitude damping, measurement error) to quantum states.

```rust
pub struct NoiseInjectionService;

impl NoiseInjectionService {
    /// Apply depolarizing noise to a single qubit.
    pub fn depolarize(state: &mut QuantumState, qubit: QubitIndex, rate: NoiseRate) {
        if rand::random::<f64>() < rate.0 {
            let pauli = match rand::random::<u8>() % 3 {
                0 => GateType::X,
                1 => GateType::Y,
                _ => GateType::Z,
            };
            let gate = Gate::pauli(qubit, pauli);
            let _ = state.apply_gate(&gate);
        }
    }

    /// Apply measurement error: flip outcome with given probability.
    pub fn measurement_error(outcome: &mut MeasurementOutcome, rate: NoiseRate) {
        if rand::random::<f64>() < rate.0 {
            outcome.result = !outcome.result;
        }
    }

    /// Apply noise model to all data qubits.
    pub fn apply_noise_model(state: &mut QuantumState, model: &NoiseModel) {
        for q in 0..state.qubit_count {
            Self::depolarize(state, QubitIndex(q), model.depolarizing_rate);
        }
    }
}
```

---

## Repositories

Repository interfaces define persistence boundaries. Implementations live in the infrastructure layer.

```rust
/// Store and retrieve circuit templates.
#[async_trait]
pub trait CircuitRepository: Send + Sync {
    async fn save(&self, circuit: &QuantumCircuit) -> Result<CircuitId, PersistenceError>;
    async fn find_by_id(&self, id: CircuitId) -> Result<Option<QuantumCircuit>, PersistenceError>;
    async fn find_by_qubit_count(&self, qubits: u32) -> Result<Vec<QuantumCircuit>, PersistenceError>;
    async fn list_templates(&self) -> Result<Vec<CircuitSummary>, PersistenceError>;
    async fn delete(&self, id: CircuitId) -> Result<bool, PersistenceError>;
}

/// Persist simulation experiment results.
#[async_trait]
pub trait SimulationResultRepository: Send + Sync {
    async fn save_metrics(&self, session_id: SessionId, metrics: &SimulationMetrics)
        -> Result<(), PersistenceError>;
    async fn save_measurement_record(
        &self,
        session_id: SessionId,
        record: &[MeasurementOutcome],
    ) -> Result<(), PersistenceError>;
    async fn find_by_session(&self, session_id: SessionId)
        -> Result<Option<SimulationResult>, PersistenceError>;
    async fn find_by_circuit(&self, circuit_id: CircuitId)
        -> Result<Vec<SimulationResult>, PersistenceError>;
}

/// Pre-built Hamiltonians for common molecular and lattice systems.
#[async_trait]
pub trait HamiltonianLibrary: Send + Sync {
    async fn get(&self, name: &str) -> Result<Option<Hamiltonian>, PersistenceError>;
    async fn list(&self) -> Result<Vec<HamiltonianEntry>, PersistenceError>;
    async fn save(&self, name: &str, hamiltonian: &Hamiltonian) -> Result<(), PersistenceError>;
}

/// Summary entry for library listings.
#[derive(Debug, Clone)]
pub struct HamiltonianEntry {
    pub name: String,
    pub qubit_count: u32,
    pub term_count: usize,
    pub description: String,
}

/// Summary entry for circuit listings.
#[derive(Debug, Clone)]
pub struct CircuitSummary {
    pub id: CircuitId,
    pub qubit_count: u32,
    pub gate_count: u32,
    pub depth: CircuitDepth,
    pub name: Option<String>,
}
```

---

## Error Types

```rust
/// Domain errors for the quantum simulation engine.
#[derive(Debug, Clone, thiserror::Error)]
pub enum DomainError {
    #[error("qubit index {index:?} out of range for {qubit_count}-qubit register")]
    QubitIndexOutOfRange { index: QubitIndex, qubit_count: u32 },

    #[error("qubit limit exceeded: requested {requested}, limit {limit:?}")]
    QubitLimitExceeded { requested: u32, limit: QubitLimit },

    #[error("control qubit cannot equal target qubit")]
    SelfControlGate,

    #[error("unknown parameter: {0}")]
    UnknownParameter(String),

    #[error("invalid session transition from {from:?} to {to:?}")]
    InvalidSessionTransition { from: SessionStatus, to: SessionStatus },

    #[error("session is not running (current status: {0:?})")]
    SessionNotRunning(SessionStatus),

    #[error("decoder distance {decoder} does not match experiment distance {experiment}")]
    DecoderDistanceMismatch { decoder: u32, experiment: u32 },

    #[error("VQE did not converge after {iterations} iterations")]
    VQEDidNotConverge { iterations: u32 },

    #[error("invalid fidelity value: {0}")]
    InvalidFidelity(f64),

    #[error("invalid noise rate: {0}")]
    InvalidNoiseRate(f64),

    #[error("unmapped qubit: {0:?}")]
    UnmappedQubit(QubitIndex),

    #[error("state vector not normalized")]
    StateNotNormalized,

    #[error("persistence error: {0}")]
    Persistence(String),
}
```

---

## Identifier Types

All aggregate roots use opaque, UUID-based identifiers.

```rust
macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(uuid::Uuid);

        impl $name {
            pub fn new() -> Self { Self(uuid::Uuid::new_v4()) }
            pub fn as_bytes(&self) -> &[u8] { self.0.as_bytes() }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

define_id!(CircuitId);
define_id!(SessionId);
define_id!(VQEId);
define_id!(ExperimentId);
```

---

## References

1. Evans, E. (2003). "Domain-Driven Design: Tackling Complexity in the Heart of Software."
2. Vernon, V. (2013). "Implementing Domain-Driven Design."
3. Nielsen, M. & Chuang, I. (2000). "Quantum Computation and Quantum Information."
4. Coherence Engine DDD: `docs/architecture/coherence-engine-ddd.md`
5. ruQu crate: `crates/ruQu/`
