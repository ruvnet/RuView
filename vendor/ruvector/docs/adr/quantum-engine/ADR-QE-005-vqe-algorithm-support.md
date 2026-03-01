# ADR-QE-005: Variational Quantum Eigensolver (VQE) Support

**Status**: Proposed
**Date**: 2026-02-06
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-06 | ruv.io | Initial VQE architecture proposal |

---

## Context

### The Variational Quantum Eigensolver Problem

The Variational Quantum Eigensolver (VQE) is one of the most important near-term quantum
algorithms, with direct applications in computational chemistry, materials science, and
combinatorial optimization. VQE computes ground-state energies of molecular Hamiltonians
by variationally minimizing the expectation value of a Hamiltonian operator with respect
to a parameterized quantum state (ansatz).

### Why VQE Matters for ruQu

VQE sits at the intersection of quantum simulation and classical optimization, making it
a natural fit for ruQu's hybrid classical-quantum architecture:

1. **Chemistry applications**: Drug discovery, catalyst design, battery materials
2. **Optimization**: QUBO problems, portfolio optimization, logistics
3. **Benchmarking**: VQE circuits exercise the full gate set and serve as a representative
   workload for evaluating simulator performance
4. **Agent integration**: ruVector agents can autonomously explore chemical configuration
   spaces using VQE as the inner evaluation kernel

### Core Requirements

| Requirement | Description | Priority |
|-------------|-------------|----------|
| Parameterized circuits | Symbolic gate angles resolved at evaluation time | P0 |
| Hamiltonian decomposition | Represent H as sum of weighted Pauli strings | P0 |
| Exact expectation values | Direct state vector computation (no shot noise) | P0 |
| Gradient evaluation | Parameter-shift rule for classical optimizer | P0 |
| Shot-based sampling | Optional mode for hardware noise emulation | P1 |
| Classical optimizer interface | Trait-based abstraction for multiple optimizers | P1 |
| Hardware-efficient ansatz | Pre-built ansatz library for common topologies | P2 |

### Current Limitations

Without dedicated VQE support, users must manually:
- Construct parameterized circuits with explicit angle substitution per iteration
- Decompose Hamiltonians into individual Pauli measurements
- Implement gradient computation by duplicating circuit evaluations
- Wire up classical optimizers with no standard interface

This is error-prone and leaves significant performance on the table, since a state vector
simulator can compute exact expectation values in a single pass without sampling overhead.

---

## Decision

### 1. Parameterized Gate Architecture

Circuits accept symbolic parameters that are resolved to numeric values per evaluation.
This avoids circuit reconstruction on each VQE iteration.

```
                ┌──────────────────────────────────────────────────┐
                │            Parameterized Circuit                  │
                │                                                    │
                │  ┌─────┐  ┌──────────┐  ┌─────┐  ┌──────────┐  │
   |0> ─────────┤  │  H  ├──┤ Ry(θ[0]) ├──┤ CX  ├──┤ Rz(θ[2]) ├──┤───
                │  └─────┘  └──────────┘  └──┬──┘  └──────────┘  │
                │                             │                     │
   |0> ─────────┤──────────────────────────────●───── Ry(θ[1]) ────┤───
                │                                                    │
                └──────────────────────────────────────────────────┘
                                      │
                                      ▼
                          parameters: [θ[0], θ[1], θ[2]]
                          values:     [0.54, 1.23, -0.87]
```

**Data model**:

```rust
/// A symbolic parameter in a quantum circuit.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Parameter {
    pub name: String,
    pub index: usize,
}

/// A gate that may reference symbolic parameters.
pub enum ParameterizedGate {
    /// Fixed gate (no parameters)
    Fixed(Gate),
    /// Rotation gate with a symbolic angle
    Rx(ParameterExpr),
    Ry(ParameterExpr),
    Rz(ParameterExpr),
    /// Parameterized two-qubit gate
    Rzz(ParameterExpr, Qubit, Qubit),
}

/// Expression for a gate parameter (supports linear combinations).
pub enum ParameterExpr {
    /// Direct parameter reference: θ[i]
    Param(usize),
    /// Scaled parameter: c * θ[i]
    Scaled(f64, usize),
    /// Sum of expressions
    Sum(Box<ParameterExpr>, Box<ParameterExpr>),
    /// Constant value
    Constant(f64),
}
```

**Resolution**: When `evaluate(params: &[f64])` is called, each `ParameterExpr` is resolved
to a concrete `f64`, and the corresponding unitary matrix is computed. This happens once per
VQE iteration and is negligible compared to state vector manipulation.

### 2. Hamiltonian Representation

The Hamiltonian is represented as a sum of weighted Pauli strings:

```
H = c_0 * I + c_1 * Z_0 + c_2 * Z_1 + c_3 * Z_0 Z_1 + c_4 * X_0 X_1 + ...
```

where each term is a tensor product of single-qubit Pauli operators {I, X, Y, Z}.

```rust
/// A single Pauli operator on one qubit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pauli {
    I,
    X,
    Y,
    Z,
}

/// A Pauli string: tensor product of single-qubit Paulis.
/// Stored as a compact bitfield for n-qubit systems.
///
/// Encoding: 2 bits per qubit (00=I, 01=X, 10=Y, 11=Z)
/// For n <= 32 qubits, fits in a single u64.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PauliString {
    /// Packed Pauli operators (2 bits each)
    pub ops: Vec<u64>,
    /// Number of qubits
    pub n_qubits: usize,
}

/// A Hamiltonian as a sum of weighted Pauli strings.
///
/// H = sum_j c_j P_j
pub struct PauliSum {
    /// Terms: (coefficient, Pauli string)
    pub terms: Vec<(Complex64, PauliString)>,
    /// Number of qubits
    pub n_qubits: usize,
}
```

**Optimization**: Identity terms (all-I Pauli strings) contribute a constant energy offset
and require no state vector computation. The implementation detects and separates these
before the expectation loop.

### 3. Direct Expectation Value Computation

This is the critical performance advantage of state vector simulation over real hardware.
On physical quantum computers, expectation values must be estimated via repeated
measurement (shot-based sampling), requiring O(1/epsilon^2) shots for epsilon precision.

In a state vector simulator, we compute the **exact** expectation value:

```
<psi| H |psi> = sum_j c_j * <psi| P_j |psi>
```

For each Pauli string P_j, the expectation value is:

```
<psi| P_j |psi> = sum_k psi_k* (P_j |psi>)_k
```

Since P_j is a tensor product of single-qubit Paulis, its action on a basis state |k> is:
- I: |k> -> |k>
- X: flips qubit, no phase
- Y: flips qubit, phase factor +/- i
- Z: no flip, phase factor +/- 1

This means each Pauli string maps each basis state to exactly one other basis state with
a phase factor. The expectation value reduces to a sum over 2^n amplitudes.

```rust
impl QuantumState {
    /// Compute the exact expectation value of a PauliSum.
    ///
    /// Complexity: O(T * 2^n) where T = number of Pauli terms, n = qubits.
    /// For a 12-qubit system with 100 Pauli terms:
    ///   100 * 4096 = 409,600 operations ~ 0.5ms
    pub fn expectation(&self, hamiltonian: &PauliSum) -> f64 {
        let mut total = 0.0_f64;

        for (coeff, pauli) in &hamiltonian.terms {
            let mut term_val = Complex64::zero();

            for k in 0..self.amplitudes.len() {
                // Compute P_j |k>: determine target index and phase
                let (target_idx, phase) = pauli.apply_to_basis(k);
                // <k| P_j |psi> = phase * psi[target_idx]
                // Accumulate psi[k]* * phase * psi[target_idx]
                term_val += self.amplitudes[k].conj()
                    * phase
                    * self.amplitudes[target_idx];
            }

            total += (coeff * term_val).re;
        }

        total
    }
}
```

**Function signature**: `QuantumState::expectation(PauliSum) -> f64`

#### Accuracy Advantage Over Sampling

| Method | Precision | Evaluations | 12-qubit Cost |
|--------|-----------|-------------|---------------|
| Shot-based (1000 shots) | ~3% | 1000 circuit runs per term | ~500ms |
| Shot-based (10000 shots) | ~1% | 10000 circuit runs per term | ~5s |
| Shot-based (1M shots) | ~0.1% | 1M circuit runs per term | ~500s |
| **Exact (state vector)** | **Machine epsilon** | **1 pass over state** | **~0.5ms** |

For VQE convergence, exact expectation values eliminate the statistical noise floor that
plagues hardware-based VQE. Classical optimizers receive clean gradients, leading to:
- Faster convergence (fewer iterations)
- No barren plateau artifacts from shot noise
- Deterministic reproducibility

### 4. Gradient Support via Parameter-Shift Rule

The parameter-shift rule provides exact analytic gradients for parameterized quantum gates.
For a gate with parameter theta:

```
d/d(theta) <H> = [<H>(theta + pi/2) - <H>(theta - pi/2)] / 2
```

This requires two circuit evaluations per parameter per gradient component.

```rust
/// Compute the gradient of the expectation value with respect to all parameters.
///
/// Uses the parameter-shift rule:
///   grad_i = [E(theta_i + pi/2) - E(theta_i - pi/2)] / 2
///
/// Complexity: O(2 * n_params * circuit_eval_cost)
/// For 12 qubits, 20 parameters, 100 Pauli terms:
///   2 * 20 * (circuit_sim + expectation) ~ 40 * 1ms = 40ms
pub fn gradient(
    circuit: &ParameterizedCircuit,
    hamiltonian: &PauliSum,
    params: &[f64],
) -> Vec<f64> {
    let n_params = params.len();
    let mut grad = vec![0.0; n_params];
    let shift = std::f64::consts::FRAC_PI_2; // pi/2

    for i in 0..n_params {
        // Forward shift
        let mut params_plus = params.to_vec();
        params_plus[i] += shift;
        let e_plus = evaluate_energy(circuit, hamiltonian, &params_plus);

        // Backward shift
        let mut params_minus = params.to_vec();
        params_minus[i] -= shift;
        let e_minus = evaluate_energy(circuit, hamiltonian, &params_minus);

        grad[i] = (e_plus - e_minus) / 2.0;
    }

    grad
}
```

### 5. Classical Optimizer Interface

A trait-based abstraction supports plugging in different classical optimizers without
changing the VQE loop:

```rust
/// Trait for classical optimizers used in the VQE outer loop.
pub trait ClassicalOptimizer: Send {
    /// Initialize the optimizer with the parameter count.
    fn initialize(&mut self, n_params: usize);

    /// Propose next parameter values given current energy and optional gradient.
    fn step(
        &mut self,
        params: &[f64],
        energy: f64,
        gradient: Option<&[f64]>,
    ) -> OptimizerResult;

    /// Check if the optimizer has converged.
    fn has_converged(&self) -> bool;

    /// Get optimizer name for logging.
    fn name(&self) -> &str;
}

/// Result of an optimizer step.
pub struct OptimizerResult {
    pub new_params: Vec<f64>,
    pub converged: bool,
    pub iteration: usize,
}
```

**Provided implementations**:

| Optimizer | Type | Gradient Required | Best For |
|-----------|------|-------------------|----------|
| `GradientDescent` | Gradient-based | Yes | Simple landscapes |
| `Adam` | Adaptive gradient | Yes | Noisy gradients, deep circuits |
| `LBFGS` | Quasi-Newton | Yes | Smooth landscapes, fast convergence |
| `COBYLA` | Derivative-free | No | Non-differentiable cost functions |
| `NelderMead` | Simplex | No | Low-dimensional problems |
| `SPSA` | Stochastic | No | Shot-based mode, noisy evaluations |

### 6. VQE Iteration Loop

The complete VQE algorithm proceeds as follows:

```
VQE Iteration Loop
==================

Input:  Hamiltonian H (PauliSum), Ansatz A (ParameterizedCircuit),
        Optimizer O (ClassicalOptimizer), initial params theta_0

Output: Minimum energy E_min, optimal params theta_opt

    theta = theta_0
    O.initialize(len(theta))

    repeat:
        ┌─────────────────────────────────────────────┐
        │  1. PREPARE STATE                            │
        │     |psi(theta)> = A(theta) |0...0>          │
        │     [Simulate parameterized circuit]          │
        │     Cost: O(G * 2^n) where G = gate count    │
        └─────────────────────────────────────────────┘
                           │
                           ▼
        ┌─────────────────────────────────────────────┐
        │  2. EVALUATE ENERGY                          │
        │     E = <psi(theta)| H |psi(theta)>          │
        │     [Direct state vector expectation]         │
        │     Cost: O(T * 2^n) where T = Pauli terms   │
        └─────────────────────────────────────────────┘
                           │
                           ▼
        ┌─────────────────────────────────────────────┐
        │  3. COMPUTE GRADIENT (if optimizer needs it) │
        │     grad = parameter_shift(A, H, theta)      │
        │     [2 * n_params circuit evaluations]        │
        │     Cost: O(2P * (G + T) * 2^n)              │
        └─────────────────────────────────────────────┘
                           │
                           ▼
        ┌─────────────────────────────────────────────┐
        │  4. CLASSICAL UPDATE                         │
        │     theta_new = O.step(theta, E, grad)       │
        │     [Pure classical computation]              │
        │     Cost: O(P^2) for quasi-Newton             │
        └─────────────────────────────────────────────┘
                           │
                           ▼
        ┌─────────────────────────────────────────────┐
        │  5. CONVERGENCE CHECK                        │
        │     if |E_new - E_old| < tol: STOP           │
        │     else: theta = theta_new, continue         │
        └─────────────────────────────────────────────┘

    return (E_min, theta_opt)
```

**Pseudocode**:

```rust
pub fn vqe(
    ansatz: &ParameterizedCircuit,
    hamiltonian: &PauliSum,
    optimizer: &mut dyn ClassicalOptimizer,
    config: &VqeConfig,
) -> VqeResult {
    let n_params = ansatz.parameter_count();
    let mut params = config.initial_params.clone()
        .unwrap_or_else(|| vec![0.0; n_params]);

    optimizer.initialize(n_params);

    let mut best_energy = f64::INFINITY;
    let mut best_params = params.clone();
    let mut history = Vec::new();

    for iteration in 0..config.max_iterations {
        // Step 1+2: Simulate circuit and compute energy
        let state = ansatz.simulate(&params);
        let energy = state.expectation(hamiltonian);

        // Track best
        if energy < best_energy {
            best_energy = energy;
            best_params = params.clone();
        }

        // Step 3: Compute gradient if needed
        let grad = if optimizer.needs_gradient() {
            Some(gradient(ansatz, hamiltonian, &params))
        } else {
            None
        };

        history.push(VqeIteration { iteration, energy, params: params.clone() });

        // Step 4: Classical update
        let result = optimizer.step(&params, energy, grad.as_deref());
        params = result.new_params;

        // Step 5: Convergence check
        if result.converged || (iteration > 0 &&
            (history[iteration].energy - history[iteration - 1].energy).abs()
                < config.convergence_threshold) {
            break;
        }
    }

    VqeResult {
        energy: best_energy,
        optimal_params: best_params,
        iterations: history.len(),
        history,
        converged: optimizer.has_converged(),
    }
}
```

### 7. Optional Shot-Based Sampling Mode

For mimicking real hardware behavior and testing noise resilience:

```rust
/// Configuration for shot-based VQE mode.
pub struct ShotConfig {
    /// Number of measurement shots per expectation estimation
    pub shots: usize,
    /// Random seed for reproducibility
    pub seed: Option<u64>,
    /// Readout error rate (probability of bit flip on measurement)
    pub readout_error: f64,
}

impl QuantumState {
    /// Estimate expectation value via shot-based sampling.
    ///
    /// Samples the state `shots` times in the computational basis,
    /// then computes the empirical expectation of each Pauli term.
    pub fn expectation_sampled(
        &self,
        hamiltonian: &PauliSum,
        config: &ShotConfig,
    ) -> (f64, f64) {
        // Returns (mean, standard_error)
        // Standard error = std_dev / sqrt(shots)
        todo!()
    }
}
```

### 8. Hardware-Efficient Ansatz Patterns

Pre-built ansatz constructors for common use cases:

```
Hardware-Efficient Ansatz (depth d, n qubits):

Layer 1..d:
  ┌─────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐
  ┤ Ry  ├──┤  Rz      ├──┤  CNOT    ├──┤  Ry      ├──
  └─────┘  └──────────┘  │  ladder  │  └──────────┘
  ┌─────┐  ┌──────────┐  │          │  ┌──────────┐
  ┤ Ry  ├──┤  Rz      ├──┤          ├──┤  Ry      ├──
  └─────┘  └──────────┘  └──────────┘  └──────────┘

Parameters per layer: 3n (Ry + Rz + Ry per qubit)
Total parameters:     3nd
```

```rust
/// Pre-built ansatz constructors.
pub mod ansatz {
    /// Hardware-efficient ansatz with Ry-Rz layers and linear CNOT entanglement.
    pub fn hardware_efficient(n_qubits: usize, depth: usize) -> ParameterizedCircuit;

    /// UCCSD (Unitary Coupled Cluster Singles and Doubles) for chemistry.
    /// Generates excitation operators based on active space.
    pub fn uccsd(n_electrons: usize, n_orbitals: usize) -> ParameterizedCircuit;

    /// Hamiltonian variational ansatz: layers of exp(-i * theta_j * P_j)
    /// for each term P_j in the Hamiltonian.
    pub fn hamiltonian_variational(
        hamiltonian: &PauliSum,
        depth: usize,
    ) -> ParameterizedCircuit;

    /// Symmetry-preserving ansatz that respects particle number conservation.
    pub fn symmetry_preserving(
        n_qubits: usize,
        n_particles: usize,
        depth: usize,
    ) -> ParameterizedCircuit;
}
```

### 9. Performance Analysis

#### 12-Qubit VQE Performance Estimate

| Component | Operations | Time |
|-----------|-----------|------|
| State vector size | 2^12 = 4,096 complex amplitudes | 64 KB |
| Circuit simulation (50 gates) | 50 * 4096 = 204,800 ops | ~0.3ms |
| Expectation (100 Pauli terms) | 100 * 4096 = 409,600 ops | ~0.5ms |
| Gradient (20 params) | 40 * (0.3 + 0.5) ms | ~32ms |
| Classical optimizer step | O(20^2) | ~0.001ms |
| **Total per iteration (with gradient)** | | **~33ms** |
| **Total per iteration (no gradient)** | | **~0.8ms** |

For gradient-free optimizers (COBYLA, Nelder-Mead), a 12-qubit VQE iteration completes
in under 1ms. With parameter-shift gradients, the cost scales linearly with parameter
count but remains under 50ms for typical chemistry ansatze.

**Scaling with qubit count**:

| Qubits | State Size | Memory | Energy Eval (100 terms) | Gradient (20 params) |
|--------|-----------|--------|------------------------|---------------------|
| 8 | 256 | 4 KB | ~0.03ms | ~2ms |
| 12 | 4,096 | 64 KB | ~0.5ms | ~33ms |
| 16 | 65,536 | 1 MB | ~8ms | ~500ms |
| 20 | 1,048,576 | 16 MB | ~130ms | ~8s |
| 24 | 16,777,216 | 256 MB | ~2s | ~130s |
| 28 | 268,435,456 | 4 GB | ~33s | ~35min |

### 10. Integration with ruVector Agent System

ruVector agents can drive autonomous chemistry optimization using VQE as the evaluation
kernel:

```
┌─────────────────────────────────────────────────────────────────┐
│                  ruVector Agent Orchestration                     │
│                                                                   │
│  ┌──────────┐    ┌──────────────┐    ┌────────────────────┐     │
│  │ Research  │───>│ Architecture │───>│  Chemistry Agent   │     │
│  │  Agent    │    │    Agent     │    │                    │     │
│  │           │    │              │    │  - Molecule spec   │     │
│  │ Literature│    │ Hamiltonian  │    │  - Basis set sel.  │     │
│  │ search    │    │ generation   │    │  - Active space    │     │
│  └──────────┘    └──────────────┘    │  - VQE execution   │     │
│                                       │  - Result analysis │     │
│                                       └────────┬───────────┘     │
│                                                │                  │
│                                       ┌────────▼───────────┐     │
│                                       │   ruQu VQE Engine  │     │
│                                       │                    │     │
│                                       │  Parameterized     │     │
│                                       │  Circuit + PauliSum│     │
│                                       │  + Optimizer        │     │
│                                       └────────────────────┘     │
└─────────────────────────────────────────────────────────────────┘
```

The agent workflow:
1. **Research agent** retrieves molecular structure and prior computational results
2. **Architecture agent** generates the qubit Hamiltonian (Jordan-Wigner or Bravyi-Kitaev
   transformation from fermionic operators)
3. **Chemistry agent** selects ansatz, optimizer, and runs VQE iterations
4. **Results** are stored in ruVector memory for pattern learning across molecules

---

## Consequences

### Benefits

1. **Exact expectation values** eliminate sampling noise, enabling faster convergence and
   deterministic reproducibility -- a major advantage over hardware VQE
2. **Symbolic parameterization** avoids circuit reconstruction overhead, reducing per-iteration
   cost to pure state manipulation
3. **Trait-based optimizer interface** allows users to swap optimizers without touching VQE
   logic, and supports custom optimizer implementations
4. **Hardware-efficient ansatz library** provides tested, production-quality circuit templates
   for common use cases
5. **Gradient support** via parameter-shift rule enables modern gradient-based optimization
   (Adam, L-BFGS) that converges significantly faster than derivative-free methods
6. **Agent integration** enables autonomous, memory-enhanced chemistry exploration that
   learns from prior VQE runs across molecular configurations

### Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Exponential memory scaling limits qubit count | High | Medium | Tensor network backend for >30 qubits (future ADR) |
| Parameter-shift gradient cost scales with parameter count | Medium | Medium | Batched gradient evaluation, simultaneous perturbation (SPSA) fallback |
| Hamiltonian term count explosion for large molecules | Medium | High | Pauli grouping (qubit-wise commuting), measurement reduction techniques |
| Optimizer convergence to local minima | Medium | Medium | Multi-start strategies, QAOA-inspired initialization |

### Trade-offs

| Decision | Advantage | Disadvantage |
|----------|-----------|--------------|
| Exact expectation over sampling | Machine-precision accuracy | Not representative of real hardware noise |
| Parameter-shift over finite-difference | Exact gradients | 2x evaluations per parameter |
| Trait-based optimizer | Extensible | Slight abstraction overhead |
| Compact PauliString bitfield | Cache-friendly | Complex bit manipulation logic |

---

## References

- Peruzzo, A. et al. "A variational eigenvalue solver on a photonic quantum processor." Nature Communications 5, 4213 (2014)
- McClean, J.R. et al. "The theory of variational hybrid quantum-classical algorithms." New Journal of Physics 18, 023023 (2016)
- Kandala, A. et al. "Hardware-efficient variational quantum eigensolver for small molecules." Nature 549, 242-246 (2017)
- Schuld, M. et al. "Evaluating analytic gradients on quantum hardware." Physical Review A 99, 032331 (2019)
- ADR-001: ruQu Architecture - Classical Nervous System for Quantum Machines
- ADR-QE-001 through ADR-QE-004: Prior quantum engine architecture decisions
- ruQu crate: `crates/ruQu/src/` - existing syndrome processing and coherence gate infrastructure
- ruVector memory system: pattern storage for cross-molecule VQE learning
