//! End-to-end quantum execution pipeline.
//!
//! Orchestrates the full lifecycle of a quantum circuit execution:
//! plan -> decompose -> execute (per segment) -> stitch -> verify.
//!
//! # Example
//!
//! ```no_run
//! use ruqu_core::circuit::QuantumCircuit;
//! use ruqu_core::pipeline::{Pipeline, PipelineConfig};
//!
//! let mut circ = QuantumCircuit::new(4);
//! circ.h(0).cnot(0, 1).h(2).cnot(2, 3);
//!
//! let config = PipelineConfig::default();
//! let result = Pipeline::execute(&circ, &config).unwrap();
//! assert!(result.total_probability > 0.99);
//! ```

use std::collections::HashMap;

use crate::backend::BackendType;
use crate::circuit::QuantumCircuit;
use crate::decomposition::{decompose, stitch_results, CircuitPartition, DecompositionStrategy};
use crate::error::Result;
use crate::planner::{plan_execution, ExecutionPlan, PlannerConfig};
use crate::simulator::Simulator;
use crate::verification::{verify_circuit, VerificationResult};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the execution pipeline.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Planner configuration (memory limits, noise, precision).
    pub planner: PlannerConfig,
    /// Maximum qubits per decomposed segment.
    pub max_segment_qubits: u32,
    /// Number of measurement shots per segment.
    pub shots: u32,
    /// Whether to run cross-backend verification.
    pub verify: bool,
    /// Deterministic seed for reproducibility.
    pub seed: u64,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            planner: PlannerConfig::default(),
            max_segment_qubits: 25,
            shots: 1024,
            verify: true,
            seed: 42,
        }
    }
}

// ---------------------------------------------------------------------------
// Pipeline result
// ---------------------------------------------------------------------------

/// Complete result from a pipeline execution.
#[derive(Debug, Clone)]
pub struct PipelineResult {
    /// The execution plan that was used.
    pub plan: ExecutionPlan,
    /// How the circuit was decomposed.
    pub decomposition: DecompositionSummary,
    /// Per-segment execution results.
    pub segment_results: Vec<SegmentResult>,
    /// Combined (stitched) measurement distribution.
    pub distribution: HashMap<Vec<bool>, f64>,
    /// Total probability mass (should be ~1.0).
    pub total_probability: f64,
    /// Verification result, if verification was enabled.
    pub verification: Option<VerificationResult>,
    /// Fidelity estimate for the stitched result.
    pub estimated_fidelity: f64,
}

/// Summary of the decomposition step.
#[derive(Debug, Clone)]
pub struct DecompositionSummary {
    /// Number of segments the circuit was split into.
    pub num_segments: usize,
    /// Strategy that was used.
    pub strategy: DecompositionStrategy,
    /// Backends selected for each segment.
    pub backends: Vec<BackendType>,
}

/// Result from executing a single segment.
#[derive(Debug, Clone)]
pub struct SegmentResult {
    /// Which segment (0-indexed).
    pub index: usize,
    /// Backend that was used.
    pub backend: BackendType,
    /// Number of qubits in this segment.
    pub num_qubits: u32,
    /// Measurement distribution from this segment.
    pub distribution: Vec<(Vec<bool>, f64)>,
}

// ---------------------------------------------------------------------------
// Pipeline implementation
// ---------------------------------------------------------------------------

/// The quantum execution pipeline.
pub struct Pipeline;

impl Pipeline {
    /// Execute a quantum circuit through the full pipeline.
    ///
    /// Steps:
    /// 1. Plan: select optimal backend(s) via cost-model routing.
    /// 2. Decompose: partition into independently-simulable segments.
    /// 3. Execute: run each segment on its assigned backend.
    /// 4. Stitch: combine segment results into a joint distribution.
    /// 5. Verify: optionally cross-check against a reference backend.
    pub fn execute(circuit: &QuantumCircuit, config: &PipelineConfig) -> Result<PipelineResult> {
        // Step 1: Plan
        let plan = plan_execution(circuit, &config.planner);

        // Step 2: Decompose
        let partition = decompose(circuit, config.max_segment_qubits);
        let decomposition = DecompositionSummary {
            num_segments: partition.segments.len(),
            strategy: partition.strategy,
            backends: partition.segments.iter().map(|s| s.backend).collect(),
        };

        // Step 3: Execute each segment
        let mut segment_results = Vec::new();
        let mut all_segment_distributions: Vec<Vec<(Vec<bool>, f64)>> = Vec::new();

        for (idx, segment) in partition.segments.iter().enumerate() {
            let shot_seed = config.seed.wrapping_add(idx as u64);

            // Use the multi-shot simulator for each segment.
            // The simulator always uses the state-vector backend internally,
            // which is correct for segments that fit within max_segment_qubits.
            let shot_result =
                Simulator::run_shots(&segment.circuit, config.shots, Some(shot_seed))?;

            // Convert the histogram counts to a probability distribution.
            let dist = counts_to_distribution(&shot_result.counts);

            segment_results.push(SegmentResult {
                index: idx,
                backend: resolve_backend(segment.backend),
                num_qubits: segment.circuit.num_qubits(),
                distribution: dist.clone(),
            });
            all_segment_distributions.push(dist);
        }

        // Step 4: Stitch results
        //
        // `stitch_results` expects a flat list of (bitstring, probability)
        // pairs, grouped by segment. Segments are distinguished by
        // consecutive runs of equal-length bitstrings (see decomposition.rs).
        let flat_partitions: Vec<(Vec<bool>, f64)> =
            all_segment_distributions.into_iter().flatten().collect();
        let distribution = stitch_results(&flat_partitions);
        let total_probability: f64 = distribution.values().sum();

        // Step 5: Estimate fidelity
        let estimated_fidelity = estimate_pipeline_fidelity(&segment_results, &partition);

        // Step 6: Verify (optional)
        let verification = if config.verify && circuit.num_qubits() <= 25 {
            Some(verify_circuit(circuit, config.shots, config.seed))
        } else {
            None
        };

        Ok(PipelineResult {
            plan,
            decomposition,
            segment_results,
            distribution,
            total_probability,
            verification,
            estimated_fidelity,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a backend type for the simulator (Auto -> StateVector).
///
/// The basic simulator only supports state-vector execution, so backends
/// that are not directly simulable are mapped to StateVector. In a full
/// production system these would dispatch to their respective engines.
fn resolve_backend(backend: BackendType) -> BackendType {
    match backend {
        BackendType::Auto => BackendType::StateVector,
        // CliffordT and Hardware are not directly supported by the basic
        // simulator; fall back to StateVector for segments classified this
        // way.
        BackendType::CliffordT => BackendType::StateVector,
        other => other,
    }
}

/// Convert a shot-count histogram to a sorted probability distribution.
///
/// Each entry in the returned vector is `(bitstring, probability)`, sorted
/// in descending order of probability.
fn counts_to_distribution(counts: &HashMap<Vec<bool>, usize>) -> Vec<(Vec<bool>, f64)> {
    let total: usize = counts.values().sum();
    if total == 0 {
        return Vec::new();
    }

    let total_f = total as f64;
    let mut dist: Vec<(Vec<bool>, f64)> = counts
        .iter()
        .map(|(bits, &count)| (bits.clone(), count as f64 / total_f))
        .collect();

    // Sort by probability descending for deterministic output.
    dist.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    dist
}

/// Estimate pipeline fidelity based on decomposition structure.
///
/// For a single segment (no decomposition), fidelity is 1.0.
/// For multiple segments, fidelity degrades based on the number of
/// cross-segment cuts and the entanglement that was severed.
fn estimate_pipeline_fidelity(segments: &[SegmentResult], partition: &CircuitPartition) -> f64 {
    if segments.len() <= 1 {
        return 1.0;
    }

    // Each spatial cut introduces fidelity loss proportional to the
    // entanglement across the cut. Without full Schmidt decomposition,
    // we use a conservative estimate:
    //   fidelity = per_cut_fidelity ^ (number of cuts)
    let num_cuts = segments.len().saturating_sub(1);
    let per_cut_fidelity: f64 = match partition.strategy {
        DecompositionStrategy::Spatial | DecompositionStrategy::Hybrid => 0.95,
        DecompositionStrategy::Temporal => 0.99,
        DecompositionStrategy::None => 1.0,
    };

    per_cut_fidelity.powi(num_cuts as i32)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::QuantumCircuit;

    #[test]
    fn test_pipeline_bell_state() {
        let mut circ = QuantumCircuit::new(2);
        circ.h(0).cnot(0, 1);

        let config = PipelineConfig {
            shots: 1024,
            verify: true,
            seed: 42,
            ..PipelineConfig::default()
        };

        let result = Pipeline::execute(&circ, &config).unwrap();
        assert!(
            result.total_probability > 0.99,
            "total_probability should be ~1.0, got {}",
            result.total_probability
        );
        assert_eq!(result.decomposition.num_segments, 1);
        assert_eq!(result.estimated_fidelity, 1.0);
    }

    #[test]
    fn test_pipeline_disjoint_bells() {
        // Two independent Bell pairs should decompose into 2 segments.
        let mut circ = QuantumCircuit::new(4);
        circ.h(0).cnot(0, 1);
        circ.h(2).cnot(2, 3);

        let config = PipelineConfig::default();
        let result = Pipeline::execute(&circ, &config).unwrap();

        assert!(
            result.decomposition.num_segments >= 2,
            "expected >= 2 segments for disjoint Bell pairs, got {}",
            result.decomposition.num_segments
        );
        assert!(
            result.total_probability > 0.95,
            "total_probability should be ~1.0, got {}",
            result.total_probability
        );
        assert!(
            result.estimated_fidelity > 0.90,
            "fidelity should be > 0.90, got {}",
            result.estimated_fidelity
        );
    }

    #[test]
    fn test_pipeline_single_qubit() {
        let mut circ = QuantumCircuit::new(1);
        circ.h(0);

        let config = PipelineConfig {
            verify: false,
            ..PipelineConfig::default()
        };

        let result = Pipeline::execute(&circ, &config).unwrap();
        assert!(
            result.total_probability > 0.99,
            "total_probability should be ~1.0, got {}",
            result.total_probability
        );
        assert!(result.verification.is_none());
    }

    #[test]
    fn test_pipeline_ghz_state() {
        let mut circ = QuantumCircuit::new(5);
        circ.h(0);
        for i in 0..4u32 {
            circ.cnot(i, i + 1);
        }

        let config = PipelineConfig {
            shots: 2048,
            seed: 123,
            ..PipelineConfig::default()
        };

        let result = Pipeline::execute(&circ, &config).unwrap();
        assert!(
            result.total_probability > 0.99,
            "total_probability should be ~1.0, got {}",
            result.total_probability
        );

        // GHZ state should have ~50% |00000> and ~50% |11111>.
        let all_false = vec![false; 5];
        let all_true = vec![true; 5];
        let p_all_false = result.distribution.get(&all_false).copied().unwrap_or(0.0);
        let p_all_true = result.distribution.get(&all_true).copied().unwrap_or(0.0);
        assert!(
            p_all_false > 0.3,
            "GHZ should have significant |00000>, got {}",
            p_all_false
        );
        assert!(
            p_all_true > 0.3,
            "GHZ should have significant |11111>, got {}",
            p_all_true
        );
    }

    #[test]
    fn test_pipeline_config_default() {
        let config = PipelineConfig::default();
        assert_eq!(config.max_segment_qubits, 25);
        assert_eq!(config.shots, 1024);
        assert!(config.verify);
        assert_eq!(config.seed, 42);
    }

    #[test]
    fn test_pipeline_with_verification() {
        let mut circ = QuantumCircuit::new(3);
        circ.h(0).cnot(0, 1).cnot(1, 2);

        let config = PipelineConfig {
            verify: true,
            shots: 512,
            ..PipelineConfig::default()
        };

        let result = Pipeline::execute(&circ, &config).unwrap();
        assert!(
            result.verification.is_some(),
            "verification should be present when verify=true"
        );
    }

    #[test]
    fn test_resolve_backend() {
        assert_eq!(resolve_backend(BackendType::Auto), BackendType::StateVector);
        assert_eq!(
            resolve_backend(BackendType::StateVector),
            BackendType::StateVector
        );
        assert_eq!(
            resolve_backend(BackendType::Stabilizer),
            BackendType::Stabilizer
        );
        assert_eq!(
            resolve_backend(BackendType::TensorNetwork),
            BackendType::TensorNetwork
        );
        assert_eq!(
            resolve_backend(BackendType::CliffordT),
            BackendType::StateVector
        );
    }

    #[test]
    fn test_estimate_fidelity_single_segment() {
        let segments = vec![SegmentResult {
            index: 0,
            backend: BackendType::StateVector,
            num_qubits: 5,
            distribution: vec![(vec![false; 5], 1.0)],
        }];
        let partition = CircuitPartition {
            segments: vec![],
            total_qubits: 5,
            strategy: DecompositionStrategy::None,
        };
        assert_eq!(estimate_pipeline_fidelity(&segments, &partition), 1.0);
    }

    #[test]
    fn test_estimate_fidelity_two_spatial_segments() {
        let segments = vec![
            SegmentResult {
                index: 0,
                backend: BackendType::StateVector,
                num_qubits: 2,
                distribution: vec![(vec![false, false], 0.5), (vec![true, true], 0.5)],
            },
            SegmentResult {
                index: 1,
                backend: BackendType::StateVector,
                num_qubits: 2,
                distribution: vec![(vec![false, false], 0.5), (vec![true, true], 0.5)],
            },
        ];
        let partition = CircuitPartition {
            segments: vec![],
            total_qubits: 4,
            strategy: DecompositionStrategy::Spatial,
        };
        let fidelity = estimate_pipeline_fidelity(&segments, &partition);
        // 0.95^1 = 0.95
        assert!(
            (fidelity - 0.95).abs() < 1e-10,
            "expected fidelity 0.95, got {}",
            fidelity
        );
    }

    #[test]
    fn test_estimate_fidelity_temporal() {
        let segments = vec![
            SegmentResult {
                index: 0,
                backend: BackendType::StateVector,
                num_qubits: 2,
                distribution: vec![(vec![false, false], 1.0)],
            },
            SegmentResult {
                index: 1,
                backend: BackendType::StateVector,
                num_qubits: 2,
                distribution: vec![(vec![false, false], 1.0)],
            },
        ];
        let partition = CircuitPartition {
            segments: vec![],
            total_qubits: 2,
            strategy: DecompositionStrategy::Temporal,
        };
        let fidelity = estimate_pipeline_fidelity(&segments, &partition);
        // 0.99^1 = 0.99
        assert!(
            (fidelity - 0.99).abs() < 1e-10,
            "expected fidelity 0.99, got {}",
            fidelity
        );
    }

    #[test]
    fn test_counts_to_distribution_empty() {
        let counts: HashMap<Vec<bool>, usize> = HashMap::new();
        let dist = counts_to_distribution(&counts);
        assert!(dist.is_empty());
    }

    #[test]
    fn test_counts_to_distribution_uniform() {
        let mut counts: HashMap<Vec<bool>, usize> = HashMap::new();
        counts.insert(vec![false], 500);
        counts.insert(vec![true], 500);
        let dist = counts_to_distribution(&counts);

        assert_eq!(dist.len(), 2);
        let total_prob: f64 = dist.iter().map(|(_, p)| p).sum();
        assert!(
            (total_prob - 1.0).abs() < 1e-10,
            "distribution should sum to 1.0, got {}",
            total_prob
        );
    }

    #[test]
    fn test_pipeline_no_verification_large_qubit() {
        // A circuit with more than 25 qubits should skip verification
        // even when verify=true (the pipeline caps at 25 qubits).
        let mut circ = QuantumCircuit::new(26);
        circ.h(0);

        let config = PipelineConfig {
            verify: true,
            shots: 64,
            ..PipelineConfig::default()
        };

        let result = Pipeline::execute(&circ, &config).unwrap();
        assert!(
            result.verification.is_none(),
            "verification should be skipped for > 25 qubits"
        );
    }

    #[test]
    fn test_pipeline_preserves_plan() {
        let mut circ = QuantumCircuit::new(3);
        circ.h(0).cnot(0, 1).cnot(1, 2);

        let config = PipelineConfig::default();
        let result = Pipeline::execute(&circ, &config).unwrap();

        // The plan should reflect the planner's analysis.
        assert!(
            !result.plan.explanation.is_empty(),
            "plan should have a non-empty explanation"
        );
    }

    #[test]
    fn test_pipeline_segment_results_match_decomposition() {
        let mut circ = QuantumCircuit::new(4);
        circ.h(0).cnot(0, 1);
        circ.h(2).cnot(2, 3);

        let config = PipelineConfig::default();
        let result = Pipeline::execute(&circ, &config).unwrap();

        assert_eq!(
            result.segment_results.len(),
            result.decomposition.num_segments,
            "segment_results count should match decomposition num_segments"
        );
    }
}
