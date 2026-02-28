# ADR-QE-008: Surface Code Error Correction Simulation

**Status**: Proposed
**Date**: 2026-02-06
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-06 | ruv.io | Initial surface code QEC simulation proposal |

---

## Context

### The Importance of QEC Simulation

Quantum Error Correction (QEC) is the bridge between noisy intermediate-scale quantum
(NISQ) devices and fault-tolerant quantum computing. Before deploying error correction
on real hardware, every aspect of the QEC stack must be validated through simulation:

1. **Decoder validation**: Verify that decoding algorithms (MWPM, Union-Find, neural
   decoders) produce correct corrections under various noise models
2. **Threshold estimation**: Determine the physical error rate below which logical error
   rate decreases with increasing code distance
3. **Architecture exploration**: Compare surface code layouts, flag qubit placements, and
   scheduling strategies
4. **Noise model development**: Test decoder robustness against realistic noise (correlated
   errors, leakage, crosstalk)

### Surface Codes as the Leading Architecture

The surface code is the most promising QEC architecture for superconducting qubit
platforms due to:

| Property | Value |
|----------|-------|
| Error threshold | ~1% (highest among practical codes) |
| Connectivity | Nearest-neighbor only (matches hardware) |
| Syndrome extraction | Local stabilizer measurements |
| Decoding | Efficient MWPM, Union-Find in O(n * alpha(n)) |

### Surface Code Layout (Distance-3)

```
Distance-3 Rotated Surface Code:

Data qubits: D0..D8 (9 total)
X-stabilizers: X0..X3 (4 ancilla qubits)
Z-stabilizers: Z0..Z3 (4 ancilla qubits)

    Z0          Z1
  /    \      /    \
D0 ──── D1 ──── D2
|  X0   |  X1   |
D3 ──── D4 ──── D5
|  X2   |  X3   |
D6 ──── D7 ──── D8
  \    /      \    /
    Z2          Z3

Qubit count: 9 data + 8 ancilla = 17 total qubits
State vector: 2^17 = 131,072 complex amplitudes
Memory: 2 MB per state vector
```

### What ruQu Provides Today

The existing ruQu crate already implements key components for error correction:

| Component | Module | Status |
|-----------|--------|--------|
| Syndrome processing | `syndrome.rs` | Production-ready (1M rounds/sec) |
| MWPM decoder | `decoder.rs` | Integrated via fusion-blossom |
| Min-cut coherence | `mincut.rs` | El-Hayek/Henzinger/Li algorithm |
| Three-filter pipeline | `filters.rs` | Structural + Shift + Evidence |
| Tile architecture | `tile.rs`, `fabric.rs` | 256-tile WASM fabric |
| Stim integration | `stim.rs` | Syndrome generation |

What is **missing** is the ability to simulate the full quantum state evolution of a
surface code cycle: ancilla initialization, stabilizer circuits, projective measurement,
state collapse, decoder feedback, and correction application. This ADR fills that gap.

### Requirements

| Requirement | Description | Priority |
|-------------|-------------|----------|
| Mid-circuit measurement | Projective measurement of individual qubits | P0 |
| Qubit reset | Reinitialize ancilla qubits to |0> each cycle | P0 |
| Conditional operations | Apply gates conditioned on measurement outcomes | P0 |
| Noise injection | Depolarizing, bit-flip, phase-flip channels | P0 |
| Syndrome extraction | Extract syndrome bits from ancilla measurements | P0 |
| Decoder integration | Feed syndromes to MWPM/min-cut decoder | P0 |
| Logical error tracking | Determine if logical error occurred | P1 |
| Multi-cycle simulation | Run thousands of QEC cycles efficiently | P1 |
| Leakage modeling | Simulate qubit leakage to non-computational states | P2 |

---

## Decision

### 1. Mid-Circuit Measurement

Mid-circuit measurement is the most critical new capability. Unlike final-state
measurement (which collapses the entire state), mid-circuit measurement collapses a
single qubit while preserving the rest of the system for continued evolution.

**Mathematical formulation**:

For measuring qubit q in the computational basis:

1. Split the state into two subspaces:
   - |psi_0>: amplitudes where qubit q = 0
   - |psi_1>: amplitudes where qubit q = 1
2. Compute probabilities:
   - P(0) = ||psi_0||^2 = sum_{k: bit_q(k)=0} |amp_k|^2
   - P(1) = ||psi_1||^2 = sum_{k: bit_q(k)=1} |amp_k|^2
3. Sample outcome m in {0, 1} according to P(0), P(1)
4. Collapse: zero out amplitudes in the non-selected subspace
5. Renormalize: divide remaining amplitudes by sqrt(P(m))

```rust
/// Result of a mid-circuit measurement.
pub struct MeasurementResult {
    /// The measured qubit index
    pub qubit: usize,
    /// The measurement outcome (0 or 1)
    pub outcome: u8,
    /// The probability of this outcome
    pub probability: f64,
}

impl QuantumState {
    /// Perform a projective measurement on a single qubit.
    ///
    /// This collapses the qubit to |0> or |1> based on Born probabilities,
    /// zeroes out amplitudes in the rejected subspace, and renormalizes.
    ///
    /// The remaining qubits are left in a valid quantum state for continued
    /// simulation (essential for mid-circuit measurement in QEC).
    ///
    /// Complexity: O(2^n) -- two passes over the state vector.
    ///   Pass 1: Compute probabilities P(0), P(1)
    ///   Pass 2: Collapse and renormalize
    pub fn measure_qubit(
        &mut self,
        qubit: usize,
        rng: &mut impl Rng,
    ) -> MeasurementResult {
        let mask = 1_usize << qubit;
        let n = self.amplitudes.len();

        // Pass 1: Compute P(0) and P(1)
        let mut prob_0 = 0.0_f64;
        let mut prob_1 = 0.0_f64;

        for k in 0..n {
            let p = self.amplitudes[k].norm_sqr();
            if (k & mask) == 0 {
                prob_0 += p;
            } else {
                prob_1 += p;
            }
        }

        // Sample outcome
        let outcome = if rng.gen::<f64>() < prob_0 { 0_u8 } else { 1_u8 };
        let prob_selected = if outcome == 0 { prob_0 } else { prob_1 };
        let norm_factor = 1.0 / prob_selected.sqrt();

        // Pass 2: Collapse and renormalize
        for k in 0..n {
            let bit = ((k & mask) >> qubit) as u8;
            if bit == outcome {
                self.amplitudes[k] *= norm_factor;
            } else {
                self.amplitudes[k] = Complex64::zero();
            }
        }

        MeasurementResult {
            qubit,
            outcome,
            probability: prob_selected,
        }
    }

    /// Measure multiple qubits (ancilla register).
    ///
    /// Measures each qubit sequentially. The order matters because each
    /// measurement collapses the state before the next measurement.
    /// For stabilizer measurements, this correctly handles correlated outcomes.
    pub fn measure_qubits(
        &mut self,
        qubits: &[usize],
        rng: &mut impl Rng,
    ) -> Vec<MeasurementResult> {
        qubits.iter()
            .map(|&q| self.measure_qubit(q, rng))
            .collect()
    }
}
```

### 2. Qubit Reset

Ancilla qubits must be reinitialized to |0> at the start of each syndrome extraction
cycle. The reset operation projects onto the |0> subspace and renormalizes:

```rust
impl QuantumState {
    /// Reset a qubit to |0>.
    ///
    /// Zeroes out all amplitudes where qubit q = 1, then renormalizes.
    /// This is equivalent to measuring the qubit and, if the outcome is |1>,
    /// applying an X gate to flip it back to |0>.
    ///
    /// Complexity: O(2^n) -- single pass over state vector.
    ///
    /// Used for ancilla reinitialization in each QEC cycle.
    pub fn reset_qubit(&mut self, qubit: usize) {
        let mask = 1_usize << qubit;
        let partner_mask = !mask;
        let n = self.amplitudes.len();

        // For each pair of states (k, k XOR mask), move amplitude from
        // the |1> component to the |0> component.
        // This implements: |0><0| + |0><1| (measure-then-flip).
        //
        // Simpler approach: zero out |1> subspace, renormalize.
        let mut norm_sq = 0.0_f64;

        for k in 0..n {
            if (k & mask) != 0 {
                // Qubit q is |1> in this basis state
                // Transfer amplitude to partner state with q = |0>
                let partner = k & partner_mask;
                // Coherent reset: add amplitudes
                // For incoherent reset (thermal): would zero out instead
                self.amplitudes[partner] += self.amplitudes[k];
                self.amplitudes[k] = Complex64::zero();
            }
        }

        // Renormalize
        for k in 0..n {
            norm_sq += self.amplitudes[k].norm_sqr();
        }
        let norm_factor = 1.0 / norm_sq.sqrt();
        for amp in self.amplitudes.iter_mut() {
            *amp *= norm_factor;
        }
    }
}
```

### 3. Noise Model

We implement three standard noise channels plus a combined depolarizing model.
Noise is applied by stochastically inserting Pauli gates after specified operations.

```
Noise Channels:

Bit-flip (X):     rho -> (1-p) * rho + p * X * rho * X
Phase-flip (Z):   rho -> (1-p) * rho + p * Z * rho * Z
Depolarizing:     rho -> (1-p) * rho + p/3 * (X*rho*X + Y*rho*Y + Z*rho*Z)
```

For state vector simulation, noise is applied via **stochastic Pauli insertion**:

```rust
/// Noise model configuration.
#[derive(Debug, Clone)]
pub struct NoiseModel {
    /// Single-qubit gate error rate
    pub single_qubit_error: f64,
    /// Two-qubit gate error rate
    pub two_qubit_error: f64,
    /// Measurement error rate (readout bit-flip)
    pub measurement_error: f64,
    /// Idle error rate (per qubit per cycle)
    pub idle_error: f64,
    /// Noise type
    pub noise_type: NoiseType,
}

#[derive(Debug, Clone, Copy)]
pub enum NoiseType {
    /// Random X errors with probability p
    BitFlip,
    /// Random Z errors with probability p
    PhaseFlip,
    /// Random X, Y, or Z errors each with probability p/3
    Depolarizing,
    /// Independent bit-flip (p_x) and phase-flip (p_z)
    Independent { p_x: f64, p_z: f64 },
}

impl QuantumState {
    /// Apply a noise channel to a single qubit.
    ///
    /// For depolarizing noise with probability p:
    ///   - With probability 1-p: do nothing
    ///   - With probability p/3: apply X
    ///   - With probability p/3: apply Y
    ///   - With probability p/3: apply Z
    ///
    /// This stochastic Pauli insertion is exact for Pauli channels
    /// and a good approximation for general noise (Pauli twirl).
    pub fn apply_noise(
        &mut self,
        qubit: usize,
        error_rate: f64,
        noise_type: NoiseType,
        rng: &mut impl Rng,
    ) {
        match noise_type {
            NoiseType::BitFlip => {
                if rng.gen::<f64>() < error_rate {
                    self.apply_x(qubit);
                }
            }
            NoiseType::PhaseFlip => {
                if rng.gen::<f64>() < error_rate {
                    self.apply_z(qubit);
                }
            }
            NoiseType::Depolarizing => {
                let r = rng.gen::<f64>();
                if r < error_rate / 3.0 {
                    self.apply_x(qubit);
                } else if r < 2.0 * error_rate / 3.0 {
                    self.apply_y(qubit);
                } else if r < error_rate {
                    self.apply_z(qubit);
                }
                // else: no error (identity)
            }
            NoiseType::Independent { p_x, p_z } => {
                if rng.gen::<f64>() < p_x {
                    self.apply_x(qubit);
                }
                if rng.gen::<f64>() < p_z {
                    self.apply_z(qubit);
                }
            }
        }
    }

    /// Apply idle noise to all data qubits.
    ///
    /// Called once per QEC cycle to model decoherence during idle periods.
    pub fn apply_idle_noise(
        &mut self,
        data_qubits: &[usize],
        noise: &NoiseModel,
        rng: &mut impl Rng,
    ) {
        for &q in data_qubits {
            self.apply_noise(q, noise.idle_error, noise.noise_type, rng);
        }
    }
}
```

### 4. Syndrome Extraction Circuit

A complete surface code syndrome extraction cycle consists of:

1. Reset ancilla qubits to |0>
2. Apply CNOT chains from data qubits to ancilla (stabilizer circuits)
3. Measure ancilla qubits to extract syndrome bits
4. (Optionally) apply noise after each gate

```
Syndrome Extraction for X-Stabilizer X0 = X_D0 * X_D1 * X_D3 * X_D4:

  D0: ────────●───────────────────────────
              │
  D1: ────────┼──────●────────────────────
              │      │
  D3: ────────┼──────┼──────●─────────────
              │      │      │
  D4: ────────┼──────┼──────┼──────●──────
              │      │      │      │
  X0: ──|0>──[H]──CNOT──CNOT──CNOT──CNOT──[H]──[M]── syndrome bit

  (For X-stabilizers: Hadamard on ancilla before and after CNOTs)
  (For Z-stabilizers: CNOTs in opposite direction, no Hadamards)
```

```rust
/// Surface code layout definition.
pub struct SurfaceCodeLayout {
    /// Code distance
    pub distance: usize,
    /// Data qubit indices
    pub data_qubits: Vec<usize>,
    /// X-stabilizer definitions: (ancilla_qubit, [data_qubits])
    pub x_stabilizers: Vec<(usize, Vec<usize>)>,
    /// Z-stabilizer definitions: (ancilla_qubit, [data_qubits])
    pub z_stabilizers: Vec<(usize, Vec<usize>)>,
    /// Total qubit count (data + ancilla)
    pub total_qubits: usize,
}

impl SurfaceCodeLayout {
    /// Generate a distance-d rotated surface code layout.
    pub fn rotated(distance: usize) -> Self {
        let n_data = distance * distance;
        let n_x_stab = (distance * distance - 1) / 2;
        let n_z_stab = (distance * distance - 1) / 2;
        let total = n_data + n_x_stab + n_z_stab;

        // Assign qubit indices:
        // 0..n_data: data qubits
        // n_data..n_data+n_x_stab: X-stabilizer ancillae
        // n_data+n_x_stab..total: Z-stabilizer ancillae

        let data_qubits: Vec<usize> = (0..n_data).collect();

        // Build stabilizer mappings based on rotated surface code geometry
        let (x_stabilizers, z_stabilizers) =
            build_rotated_stabilizers(distance, n_data);

        Self {
            distance,
            data_qubits,
            x_stabilizers,
            z_stabilizers,
            total_qubits: total,
        }
    }
}

/// One complete syndrome extraction cycle.
///
/// Returns the syndrome bitstring (one bit per stabilizer).
pub fn extract_syndrome(
    state: &mut QuantumState,
    layout: &SurfaceCodeLayout,
    noise: &Option<NoiseModel>,
    rng: &mut impl Rng,
) -> SyndromeBits {
    let mut syndrome = SyndromeBits::new(
        layout.x_stabilizers.len() + layout.z_stabilizers.len()
    );

    // Step 1: Reset all ancilla qubits
    for &(ancilla, _) in layout.x_stabilizers.iter()
        .chain(layout.z_stabilizers.iter())
    {
        state.reset_qubit(ancilla);
    }

    // Step 2: X-stabilizer circuits
    for (stab_idx, &(ancilla, ref data)) in layout.x_stabilizers.iter().enumerate() {
        // Hadamard on ancilla (transforms Z-basis CNOT to X-basis measurement)
        state.apply_hadamard(ancilla);
        if let Some(ref n) = noise {
            state.apply_noise(ancilla, n.single_qubit_error, n.noise_type, rng);
        }

        // CNOT from each data qubit to ancilla
        for &d in data {
            state.apply_cnot(d, ancilla);
            if let Some(ref n) = noise {
                state.apply_noise(d, n.two_qubit_error, n.noise_type, rng);
                state.apply_noise(ancilla, n.two_qubit_error, n.noise_type, rng);
            }
        }

        // Hadamard on ancilla
        state.apply_hadamard(ancilla);
        if let Some(ref n) = noise {
            state.apply_noise(ancilla, n.single_qubit_error, n.noise_type, rng);
        }

        // Measure ancilla
        let result = state.measure_qubit(ancilla, rng);

        // Apply measurement error
        let mut outcome = result.outcome;
        if let Some(ref n) = noise {
            if rng.gen::<f64>() < n.measurement_error {
                outcome ^= 1; // Flip the classical bit
            }
        }

        syndrome.set(stab_idx, outcome);
    }

    // Step 3: Z-stabilizer circuits
    let offset = layout.x_stabilizers.len();
    for (stab_idx, &(ancilla, ref data)) in layout.z_stabilizers.iter().enumerate() {
        // No Hadamard for Z-stabilizers

        // CNOT from ancilla to each data qubit
        for &d in data {
            state.apply_cnot(ancilla, d);
            if let Some(ref n) = noise {
                state.apply_noise(d, n.two_qubit_error, n.noise_type, rng);
                state.apply_noise(ancilla, n.two_qubit_error, n.noise_type, rng);
            }
        }

        // Measure ancilla
        let result = state.measure_qubit(ancilla, rng);

        let mut outcome = result.outcome;
        if let Some(ref n) = noise {
            if rng.gen::<f64>() < n.measurement_error {
                outcome ^= 1;
            }
        }

        syndrome.set(offset + stab_idx, outcome);
    }

    // Step 4: Apply idle noise to data qubits
    if let Some(ref n) = noise {
        state.apply_idle_noise(&layout.data_qubits, n, rng);
    }

    syndrome
}
```

### 5. Decoder Integration

The syndrome bits feed into ruQu's existing decoder infrastructure:

```
Decoder Pipeline:

  Syndrome Bits ──> SyndromeFilter ──> MWPM Decoder ──> Correction ──> Apply to State
        │                                    │
        │                              ┌─────▼─────┐
        │                              │ ruvector-  │
        │                              │ mincut     │
        └──────────────────────────────│ coherence  │
                                       │ validation │
                                       └────────────┘
```

```rust
/// Decode syndrome and apply corrections.
///
/// This function bridges the quantum simulation (state vector) with
/// ruQu's classical decoder infrastructure.
pub fn decode_and_correct(
    state: &mut QuantumState,
    syndrome: &SyndromeBits,
    layout: &SurfaceCodeLayout,
    decoder: &mut MWPMDecoder,
) -> DecoderResult {
    // Convert syndrome bits to DetectorBitmap (ruQu format)
    let mut bitmap = DetectorBitmap::new(syndrome.len());
    for i in 0..syndrome.len() {
        bitmap.set(i, syndrome.get(i) == 1);
    }

    // Decode using MWPM
    let correction = decoder.decode(&bitmap);

    // Apply X corrections to data qubits
    for &qubit in &correction.x_corrections {
        state.apply_x(qubit);
    }

    // Apply Z corrections to data qubits
    for &qubit in &correction.z_corrections {
        state.apply_z(qubit);
    }

    DecoderResult {
        correction,
        syndrome: bitmap,
        applied: true,
    }
}
```

Integration with `ruvector-mincut` for coherence validation:

```rust
/// Validate decoder correction using min-cut coherence analysis.
///
/// Uses ruQu's existing DynamicMinCutEngine to assess whether the
/// post-correction state maintains structural coherence.
pub fn validate_correction(
    syndrome: &SyndromeBits,
    correction: &Correction,
    mincut_engine: &mut DynamicMinCutEngine,
) -> CoherenceAssessment {
    // Update min-cut graph edges based on syndrome pattern
    // High syndrome density in a region lowers edge weights (less coherent)
    // Correction success restores edge weights

    let cut_value = mincut_engine.query_min_cut();

    CoherenceAssessment {
        min_cut_value: cut_value.value,
        is_coherent: cut_value.value > COHERENCE_THRESHOLD,
        witness: cut_value.witness_hash,
    }
}
```

### 6. Logical Error Tracking

To determine if a logical error has occurred, we compare the initial and final
logical qubit states:

```rust
/// Track logical errors across QEC cycles.
///
/// A logical error occurs when the cumulative effect of physical errors
/// and decoder corrections results in a non-trivial logical operator
/// being applied to the encoded qubit.
pub struct LogicalErrorTracker {
    /// Accumulated X corrections on data qubits
    x_correction_parity: Vec<bool>,
    /// Accumulated Z corrections on data qubits
    z_correction_parity: Vec<bool>,
    /// Known physical X errors (for debugging/validation)
    x_error_parity: Vec<bool>,
    /// Known physical Z errors
    z_error_parity: Vec<bool>,
    /// Logical X operator support (which data qubits)
    logical_x_support: Vec<usize>,
    /// Logical Z operator support
    logical_z_support: Vec<usize>,
}

impl LogicalErrorTracker {
    /// Check if a logical X error has occurred.
    ///
    /// A logical X error occurs when the net X-type operator
    /// (errors + corrections) has odd overlap with the logical Z operator.
    pub fn has_logical_x_error(&self) -> bool {
        let mut parity = false;
        for &q in &self.logical_z_support {
            parity ^= self.x_error_parity[q] ^ self.x_correction_parity[q];
        }
        parity
    }

    /// Check if a logical Z error has occurred.
    pub fn has_logical_z_error(&self) -> bool {
        let mut parity = false;
        for &q in &self.logical_x_support {
            parity ^= self.z_error_parity[q] ^ self.z_correction_parity[q];
        }
        parity
    }

    /// Check if any logical error has occurred.
    pub fn has_logical_error(&self) -> bool {
        self.has_logical_x_error() || self.has_logical_z_error()
    }
}
```

### 7. Full Surface Code Simulation Cycle

Putting it all together, the complete simulation loop:

```
Full Surface Code QEC Cycle
============================

Input:  Code distance d, noise model, number of cycles T, decoder

Output: Logical error rate estimate

    layout = SurfaceCodeLayout::rotated(d)
    state = QuantumState::new(layout.total_qubits)
    tracker = LogicalErrorTracker::new(layout)
    decoder = MWPMDecoder::new(d)
    mincut = DynamicMinCutEngine::new()

    // Prepare initial logical |0> state
    prepare_logical_zero(&mut state, &layout)

    for cycle in 0..T:
        ┌─────────────────────────────────────────────────────┐
        │  1. INJECT NOISE                                     │
        │     Apply depolarizing noise to all data qubits      │
        │     (models decoherence during idle + gate errors)   │
        │     tracker.record_errors(noise_locations)            │
        └─────────────────────────────────────────────────────┘
                               │
                               ▼
        ┌─────────────────────────────────────────────────────┐
        │  2. EXTRACT SYNDROME                                 │
        │     Reset ancillae -> stabilizer circuits -> measure │
        │     Returns syndrome bitstring for this cycle        │
        └─────────────────────────────────────────────────────┘
                               │
                               ▼
        ┌─────────────────────────────────────────────────────┐
        │  3. DECODE                                           │
        │     Feed syndrome to MWPM decoder                    │
        │     Decoder returns correction (X and Z Pauli ops)   │
        └─────────────────────────────────────────────────────┘
                               │
                               ▼
        ┌─────────────────────────────────────────────────────┐
        │  4. APPLY CORRECTION                                 │
        │     Apply Pauli corrections to data qubits           │
        │     tracker.record_corrections(corrections)          │
        └─────────────────────────────────────────────────────┘
                               │
                               ▼
        ┌─────────────────────────────────────────────────────┐
        │  5. VALIDATE COHERENCE (optional)                    │
        │     Run min-cut analysis on syndrome pattern         │
        │     Flag if coherence drops below threshold          │
        └─────────────────────────────────────────────────────┘

    // After T cycles, check for logical error
    logical_error = tracker.has_logical_error()
```

**Pseudocode for the full simulation**:

```rust
/// Run a complete surface code QEC simulation.
///
/// Returns the logical error rate estimated from `trials` independent runs,
/// each consisting of `cycles` QEC rounds.
pub fn simulate_surface_code(config: &SurfaceCodeConfig) -> SimulationResult {
    let layout = SurfaceCodeLayout::rotated(config.distance);
    let mut logical_errors = 0_u64;
    let mut total_cycles = 0_u64;

    for trial in 0..config.trials {
        let mut state = QuantumState::new(layout.total_qubits);
        let mut tracker = LogicalErrorTracker::new(&layout);
        let mut decoder = MWPMDecoder::new(DecoderConfig {
            distance: config.distance,
            physical_error_rate: config.noise.idle_error,
            ..Default::default()
        });
        let mut rng = StdRng::seed_from_u64(config.seed + trial);

        // Prepare logical |0>
        prepare_logical_zero(&mut state, &layout);

        for cycle in 0..config.cycles {
            // 1. Inject noise
            inject_data_noise(&mut state, &layout, &config.noise, &mut rng);

            // 2. Extract syndrome
            let syndrome = extract_syndrome(
                &mut state, &layout, &Some(config.noise.clone()), &mut rng
            );

            // 3. Decode
            let correction = decoder.decode_syndrome(&syndrome);

            // 4. Apply correction
            apply_correction(&mut state, &correction);
            tracker.record_correction(&correction);

            total_cycles += 1;
        }

        // Check for logical error
        if tracker.has_logical_error() {
            logical_errors += 1;
        }
    }

    let logical_error_rate = logical_errors as f64 / config.trials as f64;
    let error_per_cycle = 1.0 - (1.0 - logical_error_rate)
        .powf(1.0 / config.cycles as f64);

    SimulationResult {
        logical_error_rate,
        logical_error_per_cycle: error_per_cycle,
        total_trials: config.trials,
        total_cycles,
        logical_errors,
        distance: config.distance,
        physical_error_rate: config.noise.idle_error,
    }
}
```

### 8. Performance Estimates

#### Distance-3 Surface Code

| Parameter | Value |
|-----------|-------|
| Data qubits | 9 |
| Ancilla qubits | 8 |
| Total qubits | 17 |
| State vector entries | 2^17 = 131,072 |
| State vector memory | 2 MB |
| CNOTs per cycle | ~16 (4 per stabilizer, 4 stabilizers active) |
| Measurements per cycle | 8 |
| Resets per cycle | 8 |
| **Time per cycle** | **~0.5ms** |
| **1000 cycles** | **~0.5s** |

#### Distance-5 Surface Code

| Parameter | Value |
|-----------|-------|
| Data qubits | 25 |
| Ancilla qubits | 24 |
| Total qubits | 49 |
| State vector entries | 2^49 ~ 5.6 * 10^14 |
| State vector memory | **4 PB** (infeasible for full state vector) |

This highlights the fundamental scaling challenge: full state vector simulation of
distance-5 surface codes requires stabilizer simulation or tensor network methods,
not direct state vector evolution. However, for the critical distance-3 case, state
vector simulation is fast and provides ground truth.

**Practical simulation envelope**:

| Distance | Qubits | State Vector | Feasible? | Cycles/sec |
|----------|--------|-------------|-----------|------------|
| 2 (toy) | 7 | 128 entries | Yes | ~50,000 |
| 3 | 17 | 131K entries | Yes | ~2,000 |
| 3 (with noise) | 17 | 131K entries | Yes | ~1,000 |
| 4 | 31 | 2B entries | Marginal (16 GB) | ~0.1 |
| 5+ | 49+ | >10^14 | No (state vector) | -- |

For distance 5 and above, the implementation should fall back to **stabilizer
simulation** (Gottesman-Knill theorem: Clifford circuits on stabilizer states can be
simulated in polynomial time). Since surface code circuits consist entirely of Clifford
gates (H, CNOT, S) with Pauli noise, this is a natural fit.

### 9. Integration with Existing ruQu Pipeline

The surface code simulation integrates with the full ruQu stack:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    ruQu QEC Simulation Stack                         │
│                                                                       │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────────────┐   │
│  │  State       │  │  Syndrome     │  │  Decoder Pipeline          │   │
│  │  Vector      │  │  Processing   │  │                           │   │
│  │  Engine      │──│  (syndrome.rs)│──│  SyndromeFilter           │   │
│  │  (new)       │  │              │  │  ├── StructuralFilter      │   │
│  │              │  │  DetectorBitmap  │  │  ├── ShiftFilter         │   │
│  │  measure()   │  │  SyndromeBuffer │  │  ├── EvidenceFilter      │   │
│  │  reset()     │  │  SyndromeDelta │  │  └── MWPM Decoder        │   │
│  │  noise()     │  │              │  │      (decoder.rs)          │   │
│  └─────────────┘  └──────────────┘  └───────────────────────────┘   │
│         │                                        │                    │
│         │              ┌─────────────────────────┘                    │
│         │              │                                              │
│         ▼              ▼                                              │
│  ┌──────────────────────────┐  ┌────────────────────────────────┐   │
│  │  Correction Application  │  │  Coherence Validation           │   │
│  │                          │  │                                  │   │
│  │  apply_x(qubit)         │  │  DynamicMinCutEngine             │   │
│  │  apply_z(qubit)         │  │  (mincut.rs)                     │   │
│  │                          │  │                                  │   │
│  │  Logical Error Tracker   │  │  El-Hayek/Henzinger/Li          │   │
│  └──────────────────────────┘  │  O(n^{o(1)}) min-cut            │   │
│                                  └────────────────────────────────┘   │
│                                                                       │
│  ┌───────────────────────────────────────────────────────────────┐   │
│  │  Tile Architecture (fabric.rs, tile.rs)                        │   │
│  │                                                                 │   │
│  │  TileZero (coordinator) + 255 WorkerTiles                      │   │
│  │  Can parallelize across stabilizer groups for large codes      │   │
│  └───────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

Key integration points:

1. **Syndrome bits** from `measure_qubit()` are converted to `DetectorBitmap` format
   for compatibility with ruQu's existing syndrome processing pipeline
2. **MWPM decoder** from `decoder.rs` (backed by fusion-blossom) receives syndromes
   and returns corrections
3. **Min-cut coherence** from `mincut.rs` validates post-correction state quality
4. **Tile architecture** from `fabric.rs` can distribute stabilizer measurements across
   tiles for parallel processing of large codes
5. **Stim integration** from `stim.rs` provides reference syndrome distributions for
   decoder benchmarking

### 10. Error Rate Estimation

To estimate the error threshold, we run simulations at multiple physical error rates
and code distances:

```rust
/// Estimate the error threshold by scanning physical error rates.
///
/// The threshold is the physical error rate p* at which logical error rate
/// is independent of code distance. Below p*, larger codes are better.
/// Above p*, larger codes are worse.
pub fn estimate_threshold(
    distances: &[usize],
    error_rates: &[f64],
    cycles_per_trial: usize,
    trials: usize,
) -> ThresholdResult {
    let mut results = Vec::new();

    for &d in distances {
        for &p in error_rates {
            let config = SurfaceCodeConfig {
                distance: d,
                noise: NoiseModel {
                    idle_error: p,
                    single_qubit_error: p / 10.0,
                    two_qubit_error: p,
                    measurement_error: p,
                    noise_type: NoiseType::Depolarizing,
                },
                cycles: cycles_per_trial,
                trials: trials as u64,
                seed: 42,
            };

            let sim_result = simulate_surface_code(&config);
            results.push((d, p, sim_result.logical_error_per_cycle));
        }
    }

    // Find crossing point of d=3 and d=5 curves
    find_threshold_crossing(&results)
}
```

---

## Consequences

### Benefits

1. **Full quantum state simulation** provides ground truth for decoder validation that
   stabilizer simulation alone cannot (e.g., non-Clifford noise, leakage states)
2. **Seamless integration** with ruQu's existing syndrome processing, MWPM decoder,
   and min-cut coherence infrastructure minimizes new code and leverages battle-tested
   components
3. **Mid-circuit measurement** and qubit reset enable accurate simulation of the actual
   hardware QEC cycle, not just the error model
4. **Noise model flexibility** (bit-flip, phase-flip, depolarizing, independent) covers
   the standard noise models used in QEC research
5. **Logical error tracking** provides direct measurement of the quantity of interest
   (logical error rate) without post-hoc analysis
6. **Integration with min-cut coherence** validates that decoder corrections maintain
   structural coherence, bridging ruQu's unique coherence-gating approach with standard
   QEC metrics

### Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| State vector memory limits simulation to d <= 3 | High | High | Stabilizer simulation fallback for d >= 5 |
| Mid-circuit measurement breaks SIMD optimization | Medium | Medium | Separate hot/cold paths, measurement is infrequent |
| Noise model too simplistic for real hardware | Medium | Medium | Support custom noise channels, correlated errors |
| Decoder latency dominates simulation time | Low | Medium | Use streaming decoder, pre-built matching graphs |
| Logical error tracking complexity for higher distance | Low | Low | Automate logical operator computation from layout |

### Trade-offs

| Decision | Advantage | Disadvantage |
|----------|-----------|--------------|
| State vector over stabilizer simulation | Handles arbitrary noise and non-Clifford ops | Exponential memory, limited to d <= 3-4 |
| Stochastic Pauli insertion for noise | Simple, exact for Pauli channels | Approximate for non-Pauli noise |
| Sequential ancilla measurement | Correct correlated outcomes | Cannot parallelize measurement step |
| Integration with existing ruQu decoder | Reuses battle-tested code | Decoder API may not perfectly match simulation needs |
| Coherent reset (amplitude transfer) | Preserves entanglement structure | More complex than incoherent reset |

---

## References

- Fowler, A.G. et al. "Surface codes: Towards practical large-scale quantum computation." Physical Review A 86, 032324 (2012)
- Dennis, E. et al. "Topological quantum memory." Journal of Mathematical Physics 43, 4452-4505 (2002)
- Google Quantum AI. "Suppressing quantum errors by scaling a surface code logical qubit." Nature 614, 676-681 (2023)
- Higgott, O. "PyMatching: A Python package for decoding quantum codes with minimum-weight perfect matching." ACM Transactions on Quantum Computing 3, 1-16 (2022)
- Wu, Y. & Lin, H.H. "Hypergraph Decomposition and Secret Sharing." Discrete Applied Mathematics (2024)
- ADR-001: ruQu Architecture - Classical Nervous System for Quantum Machines
- ADR-QE-005: VQE Algorithm Support (quantum state manipulation, expectation values)
- ADR-QE-006: Grover's Search (state vector operations, measurement)
- ruQu syndrome module: `crates/ruQu/src/syndrome.rs` - DetectorBitmap, SyndromeBuffer
- ruQu decoder module: `crates/ruQu/src/decoder.rs` - MWPMDecoder, fusion-blossom
- ruQu mincut module: `crates/ruQu/src/mincut.rs` - DynamicMinCutEngine
- ruQu filters module: `crates/ruQu/src/filters.rs` - Three-filter coherence pipeline
- ruvector-mincut crate: `crates/ruvector-mincut/` - El-Hayek/Henzinger/Li algorithm
