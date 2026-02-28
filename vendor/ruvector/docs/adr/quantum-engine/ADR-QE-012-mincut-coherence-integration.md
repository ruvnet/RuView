# ADR-QE-012: Min-Cut Coherence Integration

**Status**: Proposed
**Date**: 2026-02-06
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board

---

## Context

The ruVector ecosystem contains several components that must work together for
quantum error correction (QEC) simulation:

1. **ruQu (existing)**: A real-time coherence gating system that performs
   boundary-to-boundary min-cut analysis on surface code error patterns. It includes
   a three-filter syndrome pipeline (Structural | Shift | Evidence), a Minimum Weight
   Perfect Matching (MWPM) decoder, and an early warning system that predicts
   correlated failures 100+ cycles ahead.

2. **ruvector-mincut (existing)**: A graph partitioning crate that computes minimum
   cuts and balanced partitions. Currently used for vector index sharding but
   directly applicable to syndrome graph decomposition.

3. **Coherence Engine (ADR-014)**: Computes coherence energy via sheaf Laplacian
   analysis. The "mincut-gated-transformer" concept uses coherence energy to skip
   computation on "healthy" regions, achieving up to 50% FLOPs reduction.

4. **Quantum Simulation Engine (new, ADR-QE-001 through ADR-QE-011)**: The
   state-vector and tensor-network simulator being designed in this ADR series.

The challenge is integrating these components into a coherent (pun intended)
pipeline where simulated quantum circuits produce syndromes, those syndromes are
decoded in real-time, and coherence analysis feeds back into simulation parameters.

### Surface Code Background

A distance-d surface code encodes 1 logical qubit in d^2 data qubits + (d^2 - 1)
ancilla qubits:

| Distance | Data qubits | Ancilla qubits | Total qubits | Error threshold |
|----------|------------|----------------|--------------|----------------|
| 3 | 9 | 8 | 17 | ~1% |
| 5 | 25 | 24 | 49 | ~1% |
| 7 | 49 | 48 | 97 | ~1% |
| 9 | 81 | 80 | 161 | ~1% |
| 11 | 121 | 120 | 241 | ~1% |

Syndrome extraction involves measuring ancilla qubits each cycle. The measurement
outcomes (syndromes) indicate where errors may have occurred. The decoder's job is
to determine the most likely error pattern from the syndrome and apply corrections.

### Performance Requirements

ruQu's existing decoder targets P99 latency of <4 microseconds for syndrome
decoding. The integrated simulation + decode pipeline must meet:

| Operation | Target latency | Notes |
|-----------|---------------|-------|
| Single syndrome decode | <4 us | Existing ruQu target (MWPM) |
| Syndrome extraction sim | <5 ms | One round of ancilla measurement |
| Full cycle (sim + decode) | <10 ms | Distance-3, single error cycle |
| Full cycle (sim + decode) | <50 ms | Distance-5 |
| Full cycle (sim + decode) | <200 ms | Distance-7 (tensor network) |
| Early warning evaluation | <1 ms | Check predicted vs actual syndromes |

## Decision

### 1. Architecture Overview

The integration follows a pipeline architecture where data flows from quantum
simulation through syndrome extraction, filtering, decoding, and coherence analysis:

```
+------------------------------------------------------------------+
|                  Quantum Error Correction Pipeline                 |
+------------------------------------------------------------------+
|                                                                    |
|  +------------------+     +---------------------+                  |
|  | Quantum Circuit  |     | Error Model         |                  |
|  | (surface code    |---->| (depolarizing,      |                  |
|  |  syndrome        |     |  biased noise,      |                  |
|  |  extraction)     |     |  correlated)        |                  |
|  +------------------+     +---------------------+                  |
|           |                        |                               |
|           v                        v                               |
|  +--------------------------------------------+                   |
|  | Quantum Simulation Engine                   |                   |
|  | (state vector or tensor network)            |                   |
|  | - Simulates noisy syndrome extraction       |                   |
|  | - Outputs ancilla measurement outcomes      |                   |
|  +--------------------------------------------+                   |
|           |                                                        |
|           | syndrome bitstring                                     |
|           v                                                        |
|  +--------------------------------------------+                   |
|  | SyndromeFilter (ruQu)                       |                   |
|  | Filter 1: Structural (lattice geometry)     |                   |
|  | Filter 2: Shift (temporal correlations)     |                   |
|  | Filter 3: Evidence (statistical weight)     |                   |
|  +--------------------------------------------+                   |
|           |                                                        |
|           | filtered syndrome                                      |
|           v                                                        |
|  +--------------------------------------------+                   |
|  | MWPM Decoder (ruQu)                         |                   |
|  | - Minimum Weight Perfect Matching           |                   |
|  | - Returns Pauli correction operators        |                   |
|  | - Target: <4 us P99 latency                 |                   |
|  +--------------------------------------------+                   |
|           |                                                        |
|           | correction operators (X, Z Paulis)                     |
|           v                                                        |
|  +--------------------------------------------+                   |
|  | Correction Application                      |                   |
|  | - Apply Pauli gates to simulated state      |                   |
|  | - Verify logical qubit integrity            |                   |
|  +--------------------------------------------+                   |
|           |                                                        |
|           | corrected state                                        |
|           v                                                        |
|  +-----------------------+    +-------------------------+          |
|  | Coherence Engine      |    | Early Warning System    |          |
|  | (sheaf Laplacian)     |    | (100+ cycle prediction) |          |
|  | - Compute coherence   |<-->| - Correlate historical  |          |
|  |   energy              |    |   syndromes             |          |
|  | - Gate simulation     |    | - Predict failures      |          |
|  |   FLOPs if healthy    |    | - Feed back to sim      |          |
|  +-----------------------+    +-------------------------+          |
|           |                            |                           |
|           v                            v                           |
|  +--------------------------------------------+                   |
|  | Cryptographic Audit Trail                   |                   |
|  | - Ed25519 signed decisions                  |                   |
|  | - Blake3 hash chains                        |                   |
|  | - Every syndrome, decode, correction logged |                   |
|  +--------------------------------------------+                   |
|                                                                    |
+------------------------------------------------------------------+
```

### 2. Syndrome-to-Decoder Bridge

The quantum simulation engine outputs raw measurement bitstrings. These are
converted to the syndrome format expected by ruQu's decoder:

```rust
/// Bridge between quantum simulation output and ruQu decoder input.
pub struct SyndromeBridge;

impl SyndromeBridge {
    /// Convert simulation measurement outcomes to ruQu syndrome format.
    ///
    /// The simulation measures ancilla qubits. A detection event occurs
    /// when an ancilla measurement differs from the previous round
    /// (or from the expected value in the first round).
    pub fn extract_syndrome(
        measurements: &MeasurementOutcome,
        code: &SurfaceCodeLayout,
        previous_round: Option<&SyndromeRound>,
    ) -> SyndromeRound {
        let mut detections = Vec::new();

        for ancilla in code.ancilla_qubits() {
            let current = measurements.get(ancilla.index());
            let previous = previous_round
                .map(|r| r.get(ancilla.id()))
                .unwrap_or(0);  // Expected value in first round

            if current != previous {
                detections.push(Detection {
                    ancilla_id: ancilla.id(),
                    ancilla_type: ancilla.stabilizer_type(),  // X or Z
                    position: ancilla.lattice_position(),
                    round: measurements.round_number(),
                });
            }
        }

        SyndromeRound {
            round: measurements.round_number(),
            detections,
            raw_measurements: measurements.ancilla_bits().to_vec(),
        }
    }

    /// Apply decoder corrections back to the simulation state.
    pub fn apply_corrections(
        state: &mut StateVector,
        corrections: &DecoderCorrection,
        code: &SurfaceCodeLayout,
    ) {
        for (qubit_id, pauli) in &corrections.operations {
            let qubit_index = code.data_qubit_index(*qubit_id);
            match pauli {
                Pauli::X => state.apply_x(qubit_index),
                Pauli::Z => state.apply_z(qubit_index),
                Pauli::Y => {
                    state.apply_x(qubit_index);
                    state.apply_z(qubit_index);
                }
                Pauli::I => {}  // No correction needed
            }
        }
    }
}
```

### 3. SyndromeFilter Pipeline (ruQu Integration)

The three-filter pipeline processes raw syndromes before decoding:

```rust
/// ruQu's three-stage syndrome filtering pipeline.
pub struct SyndromeFilterPipeline {
    structural: StructuralFilter,
    shift: ShiftFilter,
    evidence: EvidenceFilter,
}

impl SyndromeFilterPipeline {
    /// Process a syndrome round through all three filters.
    pub fn filter(&mut self, syndrome: SyndromeRound) -> FilteredSyndrome {
        // Filter 1: Structural
        // Removes detections inconsistent with lattice geometry.
        // E.g., isolated detections with no nearby partner.
        let after_structural = self.structural.apply(&syndrome);

        // Filter 2: Shift
        // Accounts for temporal correlations between rounds.
        // Detections that appear and disappear in consecutive rounds
        // may be measurement errors (not data errors).
        let after_shift = self.shift.apply(&after_structural);

        // Filter 3: Evidence
        // Weights remaining detections by statistical evidence.
        // Uses error model probabilities to assign confidence scores.
        let after_evidence = self.evidence.apply(&after_shift);

        after_evidence
    }
}
```

### 4. MWPM Decoder Integration

The filtered syndrome feeds into ruQu's MWPM decoder:

```rust
/// Interface to ruQu's Minimum Weight Perfect Matching decoder.
pub trait SyndromeDecoder {
    /// Decode a filtered syndrome into correction operations.
    /// Target: <4 microseconds P99 latency.
    fn decode(
        &self,
        syndrome: &FilteredSyndrome,
        code: &SurfaceCodeLayout,
    ) -> DecoderCorrection;

    /// Decode with timing information for performance monitoring.
    fn decode_timed(
        &self,
        syndrome: &FilteredSyndrome,
        code: &SurfaceCodeLayout,
    ) -> (DecoderCorrection, DecoderTiming);
}

pub struct DecoderCorrection {
    /// Pauli corrections to apply to data qubits.
    pub operations: Vec<(QubitId, Pauli)>,

    /// Confidence score (0.0 = no confidence, 1.0 = certain).
    pub confidence: f64,

    /// Whether a logical error was detected (correction may be wrong).
    pub logical_error_detected: bool,

    /// Matching weight (lower is more likely).
    pub matching_weight: f64,
}

pub struct DecoderTiming {
    /// Total decode time.
    pub total_ns: u64,

    /// Time spent building the matching graph.
    pub graph_construction_ns: u64,

    /// Time spent in the MWPM algorithm.
    pub matching_ns: u64,

    /// Number of detection events in the input.
    pub num_detections: usize,
}
```

### 5. Min-Cut Graph Partitioning for Parallel Decoding

For large surface codes (distance >= 7), the syndrome graph can be partitioned
using `ruvector-mincut` for parallel decoding:

```rust
use ruvector_mincut::{partition, PartitionConfig, WeightedGraph};

/// Partition the syndrome graph for parallel decoding.
/// This exploits spatial locality in the surface code: errors in
/// distant regions can be decoded independently.
pub fn parallel_decode(
    syndrome: &FilteredSyndrome,
    code: &SurfaceCodeLayout,
    decoder: &dyn SyndromeDecoder,
) -> DecoderCorrection {
    // Build the detection graph (nodes = detections, edges = possible errors)
    let detection_graph = build_detection_graph(syndrome, code);

    // If small enough, decode directly
    if detection_graph.num_nodes() <= 20 {
        return decoder.decode(syndrome, code);
    }

    // Partition the detection graph using ruvector-mincut
    let config = PartitionConfig {
        num_partitions: estimate_partition_count(&detection_graph),
        balance_factor: 1.2,
        minimize: Objective::EdgeCut,
    };
    let partitions = partition(&detection_graph, &config);

    // Decode each partition independently (in parallel via Rayon)
    let partial_corrections: Vec<DecoderCorrection> = partitions
        .par_iter()
        .map(|partition| {
            let sub_syndrome = syndrome.restrict_to(partition);
            decoder.decode(&sub_syndrome, code)
        })
        .collect();

    // Handle boundary edges (detections that span partitions)
    let boundary_correction = decode_boundary_edges(
        syndrome, code, &partitions, decoder,
    );

    // Merge all corrections
    merge_corrections(partial_corrections, boundary_correction)
}

/// Estimate optimal partition count based on detection density.
fn estimate_partition_count(graph: &WeightedGraph) -> usize {
    let n = graph.num_nodes();
    if n <= 20 { 1 }
    else if n <= 50 { 2 }
    else if n <= 100 { 4 }
    else { (n / 25).min(rayon::current_num_threads()) }
}
```

This matches ruQu's existing boundary-to-boundary min-cut analysis: the partition
boundaries correspond to the cuts in the syndrome graph where independent decoding
regions meet.

### 6. Coherence Gating for Simulation FLOPs Reduction

The sheaf Laplacian coherence energy (from ADR-014) provides a measure of how
"healthy" a quantum state region is. High coherence energy means the region is
behaving as expected (low error rate). This enables a novel optimization:

```
  Coherence Gating Decision Tree
  ================================

  For each region R of the surface code:

    1. Compute coherence energy E(R) via sheaf Laplacian

    2. Compare to thresholds:

       E(R) > E_high (0.95)
         |
         +-- Region is HEALTHY
         |   Action: SKIP detailed simulation for this region
         |   Use: simplified noise model (Pauli channel approximation)
         |   Savings: ~50% FLOPs for this region
         |
       E_low (0.70) < E(R) <= E_high (0.95)
         |
         +-- Region is NOMINAL
         |   Action: STANDARD simulation
         |   Use: full gate-by-gate simulation with noise
         |   Savings: none
         |
       E(R) <= E_low (0.70)
         |
         +-- Region is DEGRADED
         |   Action: ENHANCED simulation
         |   Use: full simulation + additional diagnostics
         |   Extra: log detailed error patterns, trigger early warning
         |   Savings: negative (more work, but necessary)
```

Implementation:

```rust
/// Coherence-gated simulation mode.
/// Uses coherence energy to decide simulation fidelity per region.
pub struct CoherenceGatedSimulator {
    /// Full-fidelity simulator for nominal/degraded regions.
    full_simulator: Box<dyn SimulationBackend>,

    /// Simplified simulator for healthy regions.
    simplified_simulator: SimplifiedNoiseModel,

    /// Coherence engine for computing region health.
    coherence_engine: CoherenceEngine,

    /// Thresholds for gating decisions.
    high_threshold: f64,
    low_threshold: f64,
}

impl CoherenceGatedSimulator {
    /// Simulate one QEC cycle with coherence gating.
    pub fn simulate_cycle(
        &mut self,
        state: &mut StateVector,
        code: &SurfaceCodeLayout,
        error_model: &ErrorModel,
        history: &SyndromeHistory,
    ) -> CycleResult {
        // Step 1: Compute coherence energy per region
        let regions = code.spatial_regions();
        let coherence = self.coherence_engine.compute_regional(
            history, &regions,
        );

        // Step 2: Classify regions and simulate accordingly
        let mut cycle_syndromes = Vec::new();
        let mut flops_saved = 0_u64;
        let mut flops_total = 0_u64;

        for (region, energy) in regions.iter().zip(coherence.energies()) {
            let region_qubits = code.qubits_in_region(region);

            if *energy > self.high_threshold {
                // HEALTHY: Use simplified Pauli noise model
                let syndrome = self.simplified_simulator.simulate_region(
                    state, &region_qubits, error_model,
                );
                let full_cost = estimate_full_sim_cost(&region_qubits);
                let simplified_cost = estimate_simplified_cost(&region_qubits);
                flops_saved += full_cost - simplified_cost;
                flops_total += simplified_cost;
                cycle_syndromes.push(syndrome);

            } else if *energy > self.low_threshold {
                // NOMINAL: Full simulation
                let syndrome = self.full_simulator.simulate_region(
                    state, &region_qubits, error_model,
                );
                let cost = estimate_full_sim_cost(&region_qubits);
                flops_total += cost;
                cycle_syndromes.push(syndrome);

            } else {
                // DEGRADED: Full simulation + diagnostics
                let syndrome = self.full_simulator.simulate_region_with_diagnostics(
                    state, &region_qubits, error_model,
                );
                let cost = estimate_full_sim_cost(&region_qubits) * 12 / 10;
                flops_total += cost;
                cycle_syndromes.push(syndrome);

                // Trigger early warning system
                tracing::warn!(
                    region = %region.id(),
                    coherence_energy = energy,
                    "Degraded coherence detected; enhanced monitoring active"
                );
            }
        }

        CycleResult {
            syndromes: merge_region_syndromes(cycle_syndromes),
            flops_saved,
            flops_total,
            coherence_energies: coherence,
        }
    }
}
```

### 7. Cryptographic Audit Trail

All syndrome decisions are signed and chained for tamper-evident logging, following
the existing ruQu pattern:

```rust
use ed25519_dalek::{SigningKey, Signature, Signer};
use blake3::Hasher;

/// Cryptographically auditable decision record.
#[derive(Debug, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Sequence number in the audit chain.
    pub sequence: u64,

    /// Blake3 hash of the previous record (chain linkage).
    pub previous_hash: [u8; 32],

    /// Timestamp (nanosecond precision).
    pub timestamp_ns: u128,

    /// The decision being recorded.
    pub decision: AuditableDecision,

    /// Ed25519 signature over (sequence || previous_hash || timestamp || decision).
    pub signature: Signature,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AuditableDecision {
    /// Raw syndrome from simulation.
    SyndromeExtracted {
        round: u64,
        detections: Vec<Detection>,
        simulation_id: Uuid,
    },

    /// Filtered syndrome after pipeline.
    SyndromeFiltered {
        round: u64,
        detections_before: usize,
        detections_after: usize,
        filters_applied: Vec<String>,
    },

    /// Decoder correction decision.
    CorrectionApplied {
        round: u64,
        corrections: Vec<(QubitId, Pauli)>,
        confidence: f64,
        decode_time_ns: u64,
    },

    /// Coherence gating decision.
    CoherenceGating {
        round: u64,
        region_id: String,
        coherence_energy: f64,
        decision: GatingDecision,
        flops_saved: u64,
    },

    /// Early warning alert.
    EarlyWarning {
        round: u64,
        predicted_failure_round: u64,
        confidence: f64,
        affected_region: String,
    },

    /// Logical error detected.
    LogicalError {
        round: u64,
        error_type: String,
        decoder_confidence: f64,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GatingDecision {
    SkipDetailedSimulation,
    StandardSimulation,
    EnhancedSimulation,
}

/// Audit trail manager.
pub struct AuditTrail {
    signing_key: SigningKey,
    chain_head: [u8; 32],
    sequence: u64,
}

impl AuditTrail {
    /// Record a decision in the audit trail.
    pub fn record(&mut self, decision: AuditableDecision) -> AuditRecord {
        let timestamp_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        // Compute hash of the decision content
        let mut hasher = Hasher::new();
        hasher.update(&self.sequence.to_le_bytes());
        hasher.update(&self.chain_head);
        hasher.update(&timestamp_ns.to_le_bytes());
        hasher.update(&bincode::serialize(&decision).unwrap());
        let content_hash = hasher.finalize();

        // Sign the hash
        let signature = self.signing_key.sign(content_hash.as_bytes());

        let record = AuditRecord {
            sequence: self.sequence,
            previous_hash: self.chain_head,
            timestamp_ns,
            decision,
            signature,
        };

        // Update chain
        self.chain_head = *content_hash.as_bytes();
        self.sequence += 1;

        record
    }
}
```

### 8. Early Warning Feedback Loop

ruQu's early warning system predicts correlated failures 100+ cycles ahead. This
prediction feeds back into the simulation engine to validate decoder robustness:

```rust
/// Early warning integration with quantum simulation.
pub struct EarlyWarningIntegration {
    warning_system: EarlyWarningSystem,
    error_injector: ErrorInjector,
}

impl EarlyWarningIntegration {
    /// Check early warning predictions and optionally inject
    /// targeted errors to validate decoder response.
    pub fn process_cycle(
        &mut self,
        history: &SyndromeHistory,
        state: &mut StateVector,
        code: &SurfaceCodeLayout,
    ) -> Vec<EarlyWarningAction> {
        let predictions = self.warning_system.predict(history);
        let mut actions = Vec::new();

        for prediction in &predictions {
            if prediction.confidence > 0.8 {
                // High-confidence prediction: inject targeted errors
                // to validate that the decoder handles this failure mode
                let targeted_errors = self.error_injector.generate_targeted(
                    &prediction.affected_region,
                    &prediction.predicted_error_pattern,
                    code,
                );

                actions.push(EarlyWarningAction::InjectTargetedErrors {
                    region: prediction.affected_region.clone(),
                    errors: targeted_errors,
                    prediction_confidence: prediction.confidence,
                    predicted_failure_round: prediction.failure_round,
                });

                tracing::info!(
                    confidence = prediction.confidence,
                    failure_round = prediction.failure_round,
                    region = %prediction.affected_region,
                    "Early warning: injecting targeted errors for decoder validation"
                );
            } else if prediction.confidence > 0.5 {
                // Moderate confidence: increase monitoring, do not inject
                actions.push(EarlyWarningAction::IncreasedMonitoring {
                    region: prediction.affected_region.clone(),
                    enhanced_diagnostics: true,
                });
            }
        }

        actions
    }
}

pub enum EarlyWarningAction {
    /// Inject targeted errors to test decoder response.
    InjectTargetedErrors {
        region: String,
        errors: Vec<InjectedError>,
        prediction_confidence: f64,
        predicted_failure_round: u64,
    },
    /// Increase monitoring without error injection.
    IncreasedMonitoring {
        region: String,
        enhanced_diagnostics: bool,
    },
}
```

### 9. Performance Targets

| Pipeline stage | Target latency | Distance-3 | Distance-5 | Distance-7 |
|---|---|---|---|---|
| Syndrome extraction (sim) | Varies | 2 ms | 15 ms | 80 ms |
| Syndrome filtering | <0.5 ms | 0.1 ms | 0.2 ms | 0.4 ms |
| MWPM decoding | <4 us | 1 us | 2 us | 3.5 us |
| Correction application | <0.1 ms | 0.01 ms | 0.05 ms | 0.08 ms |
| Coherence computation | <1 ms | 0.3 ms | 0.5 ms | 0.8 ms |
| Audit record creation | <0.05 ms | 0.02 ms | 0.03 ms | 0.04 ms |
| **Total cycle** | | **~3 ms** | **~16 ms** | **~82 ms** |

For distance-7 and above, the tensor network backend (ADR-QE-009) is used for
the syndrome extraction simulation, as 97 qubits exceeds state-vector capacity.

### 10. Integration Data Flow Summary

```
  +-------------------+
  | QuantumCircuit    |   Surface code syndrome extraction circuit
  | (parameterized by |   with noise model applied
  |  error model)     |
  +--------+----------+
           |
           v
  +--------+----------+
  | SimulationEngine  |   State vector (d<=5) or tensor network (d>=7)
  | execute()         |
  +--------+----------+
           |
           | MeasurementOutcome (ancilla bitstring)
           v
  +--------+----------+
  | SyndromeBridge    |   Convert measurements to detection events
  | extract_syndrome()|
  +--------+----------+
           |
           | SyndromeRound
           v
  +--------+----------+
  | SyndromeFilter    |   Three-stage filtering (Structural|Shift|Evidence)
  | Pipeline          |
  +--------+----------+
           |
           | FilteredSyndrome
           v
  +--------+----------+     +------------------+
  | MWPM Decoder      |<--->| ruvector-mincut  |  Parallel decoding
  | (ruQu)            |     | graph partition  |  for large codes
  +--------+----------+     +------------------+
           |
           | DecoderCorrection (Pauli operators)
           v
  +--------+----------+
  | Correction Apply  |   Apply X/Z/Y Paulis to simulated state
  +--------+----------+
           |
           | Corrected state
           v
  +--------+--+------+-----+---+
  |           |              |  |
  v           v              v  v
  Coherence   Early Warning  Audit Trail
  Engine      System         (Ed25519 +
  (sheaf      (100+ cycle    Blake3)
  Laplacian)  prediction)
  |           |
  |           +---> Feeds back to simulation
  |                 (targeted error injection)
  |
  +---> Coherence gating
        (skip/standard/enhanced sim)
        ~50% FLOPs reduction when healthy
```

### 11. API Surface

The complete integration is exposed through a high-level API:

```rust
/// High-level QEC simulation with full pipeline integration.
pub struct QecSimulator {
    engine: QuantumEngine,
    bridge: SyndromeBridge,
    filter: SyndromeFilterPipeline,
    decoder: Box<dyn SyndromeDecoder>,
    coherence: Option<CoherenceGatedSimulator>,
    early_warning: Option<EarlyWarningIntegration>,
    audit: AuditTrail,
    history: SyndromeHistory,
}

impl QecSimulator {
    /// Run N cycles of QEC simulation.
    pub fn run_cycles(
        &mut self,
        code: &SurfaceCodeLayout,
        error_model: &ErrorModel,
        num_cycles: usize,
    ) -> QecSimulationResult {
        let mut results = Vec::with_capacity(num_cycles);

        for cycle in 0..num_cycles {
            let cycle_result = self.run_single_cycle(code, error_model, cycle);
            results.push(cycle_result);
        }

        QecSimulationResult {
            cycles: results,
            logical_error_rate: self.compute_logical_error_rate(&results),
            total_flops_saved: results.iter().map(|r| r.flops_saved).sum(),
            decoder_latency_p99: self.compute_decoder_p99(&results),
        }
    }

    fn run_single_cycle(
        &mut self,
        code: &SurfaceCodeLayout,
        error_model: &ErrorModel,
        cycle: usize,
    ) -> CycleResult {
        // ... full pipeline as described above
    }
}
```

## Consequences

### Positive

1. **Unified pipeline**: Simulation, decoding, coherence analysis, and auditing
   work together seamlessly rather than as disconnected tools.
2. **Real performance gains**: Coherence gating can reduce simulation FLOPs by
   ~50% for healthy regions, directly applicable to long QEC simulations.
3. **Decoder validation**: The simulation engine provides a controlled environment
   to test decoder correctness under various error models.
4. **Early warning validation**: Predicted failures can be injected and the decoder's
   response verified, increasing confidence in the early warning system.
5. **Auditable**: Every decision in the pipeline is cryptographically signed and
   hash-chained, meeting compliance requirements for safety-critical applications.
6. **Leverages existing infrastructure**: `ruvector-mincut`, ruQu's decoder, and
   the coherence engine are reused rather than reimplemented.

### Negative

1. **Coupling**: The integration creates dependencies between previously independent
   crates. Changes to ruQu's syndrome format require updates to the bridge.
   Mitigation: trait abstractions at integration boundaries.
2. **Complexity**: The full pipeline has many stages, each with its own configuration
   and failure modes. Mitigation: sensible defaults and the high-level `QecSimulator`
   API that hides complexity.
3. **Performance overhead**: Coherence computation and audit trail signing add
   latency to each cycle. Mitigation: both are optional and can be disabled.
4. **Tensor network dependency**: Distance >= 7 codes require the tensor network
   backend, which is behind a feature flag and may not always be compiled in.

### Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Coherence gating skips a region that has real errors | Low | Missed errors | Conservative thresholds; periodic full-fidelity verification cycles |
| MWPM decoder exceeds 4us on partitioned syndrome | Medium | Latency violation | Adaptive partition count; fallback to non-partitioned decode |
| Early warning false positives cause unnecessary error injection | Medium | Wasted cycles | Confidence threshold (>0.8) gates injection; injection is rate-limited |
| Audit trail storage grows unboundedly | Medium | Disk exhaustion | Configurable retention; periodic pruning of old records |
| Syndrome format version mismatch between sim and decoder | Low | Decode failure | Version field in SyndromeRound; compatibility checks at pipeline init |

## References

- ruQu crate: boundary-to-boundary min-cut coherence gating
- ruQu SyndromeFilter: three-filter pipeline (Structural | Shift | Evidence)
- `ruvector-mincut` crate: graph partitioning for parallel decoding
- ADR-014: Coherence Engine (sheaf Laplacian coherence computation)
- ADR-CE-001: Sheaf Laplacian (mathematical foundation)
- ADR-QE-001: Core Engine Architecture (simulation backends)
- ADR-QE-009: Tensor Network Evaluation Mode (large code simulation)
- ADR-QE-010: Observability & Monitoring (metrics for pipeline stages)
- ADR-QE-011: Memory Gating & Power Management (resource constraints)
- Fowler et al., "Surface codes: Towards practical large-scale quantum computation" (2012)
- Higgott, "PyMatching: A Python package for decoding quantum codes with MWPM" (2022)
- Dennis et al., "Topological quantum memory" (2002) -- MWPM decoding
- Ed25519: https://ed25519.cr.yp.to/
- Blake3: https://github.com/BLAKE3-team/BLAKE3
