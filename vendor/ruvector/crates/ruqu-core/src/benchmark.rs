//! Comprehensive benchmark and proof suite for ruqu-core's four flagship
//! capabilities: cost-model routing, entanglement budgeting, adaptive
//! decoding, and cross-backend certification.
//!
//! All benchmarks are deterministic (seeded RNG) and self-contained,
//! using only `rand` and `std` beyond crate-internal imports.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::Instant;

use crate::backend::{analyze_circuit, BackendType};
use crate::circuit::QuantumCircuit;
use crate::confidence::total_variation_distance;
use crate::decoder::{
    PartitionedDecoder, StabilizerMeasurement, SurfaceCodeDecoder, SyndromeData, UnionFindDecoder,
};
use crate::decomposition::{classify_segment, decompose, estimate_segment_cost};
use crate::planner::{plan_execution, PlannerConfig};
use crate::simulator::Simulator;
use crate::verification::{is_clifford_circuit, run_stabilizer_shots};

// ---------------------------------------------------------------------------
// Proof 1: Routing benchmark
// ---------------------------------------------------------------------------

/// Result for a single circuit's routing comparison.
pub struct RoutingResult {
    pub circuit_id: usize,
    pub num_qubits: u32,
    pub depth: u32,
    pub t_count: u32,
    pub naive_time_ns: u64,
    pub heuristic_time_ns: u64,
    pub planner_time_ns: u64,
    pub planner_backend: String,
    pub speedup_vs_naive: f64,
    pub speedup_vs_heuristic: f64,
}

/// Aggregated routing benchmark across many circuits.
pub struct RoutingBenchmark {
    pub num_circuits: usize,
    pub results: Vec<RoutingResult>,
}

impl RoutingBenchmark {
    /// Percentage of circuits where the cost-model planner matches or beats
    /// the naive selector on predicted runtime.
    pub fn planner_win_rate_vs_naive(&self) -> f64 {
        if self.results.is_empty() {
            return 0.0;
        }
        let wins = self
            .results
            .iter()
            .filter(|r| r.planner_time_ns <= r.naive_time_ns)
            .count();
        wins as f64 / self.results.len() as f64 * 100.0
    }

    /// Median speedup of planner vs naive.
    pub fn median_speedup_vs_naive(&self) -> f64 {
        if self.results.is_empty() {
            return 1.0;
        }
        let mut speedups: Vec<f64> = self.results.iter().map(|r| r.speedup_vs_naive).collect();
        speedups.sort_by(|a, b| a.partial_cmp(b).unwrap());
        speedups[speedups.len() / 2]
    }
}

/// Simulate the predicted runtime (nanoseconds) for a circuit on a specific
/// backend, using the planner's cost model.
fn predicted_runtime_ns(circuit: &QuantumCircuit, backend: BackendType) -> u64 {
    let analysis = analyze_circuit(circuit);
    let n = analysis.num_qubits;
    let gates = analysis.total_gates;
    match backend {
        BackendType::Stabilizer => {
            let ns = (n as f64) * (n as f64) * (gates as f64) * 0.1;
            ns as u64
        }
        BackendType::StateVector => {
            if n >= 64 {
                return u64::MAX;
            }
            let base = (1u64 << n) as f64 * gates as f64 * 4.0;
            let scaling = if n > 25 {
                2.0_f64.powi((n - 25) as i32)
            } else {
                1.0
            };
            (base * scaling) as u64
        }
        BackendType::TensorNetwork => {
            let chi = 64.0_f64;
            let ns = (n as f64) * chi * chi * chi * (gates as f64) * 2.0;
            ns as u64
        }
        BackendType::CliffordT => {
            // 2^t stabiliser terms, each O(n^2) per gate.
            let t = analysis.non_clifford_gates as u32;
            let terms = 1u64.checked_shl(t).unwrap_or(u64::MAX);
            let flops_per_gate = 4 * (n as u64) * (n as u64);
            let ns = terms as f64 * flops_per_gate as f64 * gates as f64 * 0.1;
            ns as u64
        }
        BackendType::Auto => {
            let plan = plan_execution(circuit, &PlannerConfig::default());
            predicted_runtime_ns(circuit, plan.backend)
        }
    }
}

/// Naive selector: always picks StateVector.
fn naive_select(_circuit: &QuantumCircuit) -> BackendType {
    BackendType::StateVector
}

/// Simple heuristic: Clifford fraction > 0.95 => Stabilizer, else StateVector.
fn heuristic_select(circuit: &QuantumCircuit) -> BackendType {
    let analysis = analyze_circuit(circuit);
    if analysis.clifford_fraction > 0.95 {
        BackendType::Stabilizer
    } else {
        BackendType::StateVector
    }
}

/// Run the routing benchmark: generate diverse circuits, route through
/// three selectors, and compare predicted runtimes.
pub fn run_routing_benchmark(seed: u64, num_circuits: usize) -> RoutingBenchmark {
    let mut rng = StdRng::seed_from_u64(seed);
    let config = PlannerConfig::default();
    let mut results = Vec::with_capacity(num_circuits);

    for id in 0..num_circuits {
        let kind = id % 5;
        let circuit = match kind {
            0 => gen_clifford_circuit(&mut rng),
            1 => gen_low_t_circuit(&mut rng),
            2 => gen_high_t_circuit(&mut rng),
            3 => gen_large_nn_circuit(&mut rng),
            _ => gen_mixed_circuit(&mut rng),
        };

        let analysis = analyze_circuit(&circuit);
        let t_count = analysis.non_clifford_gates as u32;
        let depth = circuit.depth();
        let num_qubits = circuit.num_qubits();

        let plan = plan_execution(&circuit, &config);
        let planner_backend = plan.backend;

        let naive_backend = naive_select(&circuit);
        let heuristic_backend = heuristic_select(&circuit);

        let planner_time = predicted_runtime_ns(&circuit, planner_backend);
        let naive_time = predicted_runtime_ns(&circuit, naive_backend);
        let heuristic_time = predicted_runtime_ns(&circuit, heuristic_backend);

        let speedup_naive = if planner_time > 0 {
            naive_time as f64 / planner_time as f64
        } else {
            1.0
        };
        let speedup_heuristic = if planner_time > 0 {
            heuristic_time as f64 / planner_time as f64
        } else {
            1.0
        };

        results.push(RoutingResult {
            circuit_id: id,
            num_qubits,
            depth,
            t_count,
            naive_time_ns: naive_time,
            heuristic_time_ns: heuristic_time,
            planner_time_ns: planner_time,
            planner_backend: format!("{:?}", planner_backend),
            speedup_vs_naive: speedup_naive,
            speedup_vs_heuristic: speedup_heuristic,
        });
    }

    RoutingBenchmark {
        num_circuits,
        results,
    }
}

// ---------------------------------------------------------------------------
// Circuit generators (kept minimal to stay under 500 lines)
// ---------------------------------------------------------------------------

fn gen_clifford_circuit(rng: &mut StdRng) -> QuantumCircuit {
    let n = rng.gen_range(2..=60);
    let mut circ = QuantumCircuit::new(n);
    for q in 0..n {
        circ.h(q);
    }
    let gates = rng.gen_range(n..n * 3);
    for _ in 0..gates {
        let q1 = rng.gen_range(0..n);
        let q2 = (q1 + 1) % n;
        circ.cnot(q1, q2);
    }
    circ
}

fn gen_low_t_circuit(rng: &mut StdRng) -> QuantumCircuit {
    let n = rng.gen_range(4..=20);
    let mut circ = QuantumCircuit::new(n);
    for q in 0..n {
        circ.h(q);
    }
    for q in 0..(n - 1) {
        circ.cnot(q, q + 1);
    }
    let t_count = rng.gen_range(1..=3);
    for _ in 0..t_count {
        circ.t(rng.gen_range(0..n));
    }
    circ
}

fn gen_high_t_circuit(rng: &mut StdRng) -> QuantumCircuit {
    let n = rng.gen_range(3..=15);
    let mut circ = QuantumCircuit::new(n);
    let depth = rng.gen_range(5..20);
    for _ in 0..depth {
        for q in 0..n {
            if rng.gen_bool(0.5) {
                circ.t(q);
            } else {
                circ.h(q);
            }
        }
        if n > 1 {
            let q1 = rng.gen_range(0..n - 1);
            circ.cnot(q1, q1 + 1);
        }
    }
    circ
}

fn gen_large_nn_circuit(rng: &mut StdRng) -> QuantumCircuit {
    let n = rng.gen_range(40..=100);
    let mut circ = QuantumCircuit::new(n);
    for q in 0..(n - 1) {
        circ.cnot(q, q + 1);
    }
    let t_count = rng.gen_range(15..30);
    for _ in 0..t_count {
        circ.t(rng.gen_range(0..n));
    }
    circ
}

fn gen_mixed_circuit(rng: &mut StdRng) -> QuantumCircuit {
    let n = rng.gen_range(5..=25);
    let mut circ = QuantumCircuit::new(n);
    let layers = rng.gen_range(3..10);
    for _ in 0..layers {
        for q in 0..n {
            match rng.gen_range(0..4) {
                0 => {
                    circ.h(q);
                }
                1 => {
                    circ.t(q);
                }
                2 => {
                    circ.s(q);
                }
                _ => {
                    circ.x(q);
                }
            }
        }
        if n > 1 {
            let q1 = rng.gen_range(0..n - 1);
            circ.cnot(q1, q1 + 1);
        }
    }
    circ
}

// ---------------------------------------------------------------------------
// Proof 2: Entanglement budget benchmark
// ---------------------------------------------------------------------------

/// Results from the entanglement budget verification.
pub struct EntanglementBudgetBenchmark {
    pub circuits_tested: usize,
    pub segments_total: usize,
    pub segments_within_budget: usize,
    pub max_violation: f64,
    pub decomposition_overhead_pct: f64,
}

/// Run the entanglement budget benchmark: decompose circuits into segments
/// and verify each segment's estimated entanglement stays within budget.
pub fn run_entanglement_benchmark(seed: u64, num_circuits: usize) -> EntanglementBudgetBenchmark {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut segments_total = 0usize;
    let mut segments_within = 0usize;
    let mut max_violation = 0.0_f64;
    let max_segment_qubits = 25;

    let mut baseline_cost = 0u64;
    let mut decomposed_cost = 0u64;

    for _ in 0..num_circuits {
        let circuit = gen_entanglement_circuit(&mut rng);

        // Baseline cost: whole circuit on a single backend.
        let base_backend = classify_segment(&circuit);
        let base_seg = estimate_segment_cost(&circuit, base_backend);
        baseline_cost += base_seg.estimated_flops;

        // Decomposed cost: sum of segment costs.
        let partition = decompose(&circuit, max_segment_qubits);
        for seg in &partition.segments {
            segments_total += 1;
            decomposed_cost += seg.estimated_cost.estimated_flops;

            // Check entanglement budget: the segment qubit count should
            // not exceed the max_segment_qubits threshold.
            let active = seg.circuit.num_qubits();
            if active <= max_segment_qubits {
                segments_within += 1;
            } else {
                let violation = (active - max_segment_qubits) as f64 / max_segment_qubits as f64;
                if violation > max_violation {
                    max_violation = violation;
                }
            }
        }
    }

    let overhead = if baseline_cost > 0 {
        ((decomposed_cost as f64 / baseline_cost as f64) - 1.0) * 100.0
    } else {
        0.0
    };

    EntanglementBudgetBenchmark {
        circuits_tested: num_circuits,
        segments_total,
        segments_within_budget: segments_within,
        max_violation,
        decomposition_overhead_pct: overhead.max(0.0),
    }
}

fn gen_entanglement_circuit(rng: &mut StdRng) -> QuantumCircuit {
    let n = rng.gen_range(6..=40);
    let mut circ = QuantumCircuit::new(n);
    // Create two disconnected blocks with a bridge.
    let half = n / 2;
    for q in 0..half.saturating_sub(1) {
        circ.h(q);
        circ.cnot(q, q + 1);
    }
    for q in half..(n - 1) {
        circ.h(q);
        circ.cnot(q, q + 1);
    }
    // Occasional bridge gate.
    if rng.gen_bool(0.3) && half > 0 && half < n {
        circ.cnot(half - 1, half);
    }
    // Sprinkle some T gates.
    let t_count = rng.gen_range(0..5);
    for _ in 0..t_count {
        circ.t(rng.gen_range(0..n));
    }
    circ
}

// ---------------------------------------------------------------------------
// Proof 3: Decoder benchmark
// ---------------------------------------------------------------------------

/// Result for a single code distance's decoder comparison.
pub struct DecoderBenchmarkResult {
    pub distance: u32,
    pub union_find_avg_ns: f64,
    pub partitioned_avg_ns: f64,
    pub speedup: f64,
    pub union_find_accuracy: f64,
    pub partitioned_accuracy: f64,
}

/// Run the decoder benchmark across multiple code distances.
pub fn run_decoder_benchmark(
    seed: u64,
    distances: &[u32],
    rounds_per_distance: u32,
) -> Vec<DecoderBenchmarkResult> {
    let mut rng = StdRng::seed_from_u64(seed);
    let error_rate = 0.05;
    let mut results = Vec::with_capacity(distances.len());

    for &d in distances {
        let uf_decoder = UnionFindDecoder::new(0);
        let tile_size = (d / 2).max(2);
        let part_decoder = PartitionedDecoder::new(tile_size, Box::new(UnionFindDecoder::new(0)));

        let mut uf_total_ns = 0u64;
        let mut part_total_ns = 0u64;
        let mut uf_correct = 0u64;
        let mut part_correct = 0u64;

        for _ in 0..rounds_per_distance {
            let syndrome = gen_syndrome(&mut rng, d, error_rate);

            let uf_corr = uf_decoder.decode(&syndrome);
            uf_total_ns += uf_corr.decode_time_ns;

            let part_corr = part_decoder.decode(&syndrome);
            part_total_ns += part_corr.decode_time_ns;

            // A simple accuracy check: count defects and compare logical
            // outcome expectation.
            let defect_count = syndrome.stabilizers.iter().filter(|s| s.value).count();
            let expected_logical = defect_count >= d as usize;
            if uf_corr.logical_outcome == expected_logical {
                uf_correct += 1;
            }
            if part_corr.logical_outcome == expected_logical {
                part_correct += 1;
            }
        }

        let r = rounds_per_distance as f64;
        let uf_avg = uf_total_ns as f64 / r;
        let part_avg = part_total_ns as f64 / r;
        let speedup = if part_avg > 0.0 {
            uf_avg / part_avg
        } else {
            1.0
        };

        results.push(DecoderBenchmarkResult {
            distance: d,
            union_find_avg_ns: uf_avg,
            partitioned_avg_ns: part_avg,
            speedup,
            union_find_accuracy: uf_correct as f64 / r,
            partitioned_accuracy: part_correct as f64 / r,
        });
    }

    results
}

fn gen_syndrome(rng: &mut StdRng, distance: u32, error_rate: f64) -> SyndromeData {
    let grid = if distance > 1 { distance - 1 } else { 1 };
    let mut stabilizers = Vec::with_capacity((grid * grid) as usize);
    for y in 0..grid {
        for x in 0..grid {
            stabilizers.push(StabilizerMeasurement {
                x,
                y,
                round: 0,
                value: rng.gen_bool(error_rate),
            });
        }
    }
    SyndromeData {
        stabilizers,
        code_distance: distance,
        num_rounds: 1,
    }
}

// ---------------------------------------------------------------------------
// Proof 4: Cross-backend certification
// ---------------------------------------------------------------------------

/// Results from the cross-backend certification benchmark.
pub struct CertificationBenchmark {
    pub circuits_tested: usize,
    pub certified: usize,
    pub certification_rate: f64,
    pub max_tvd: f64,
    pub avg_tvd: f64,
    pub tvd_bound: f64,
}

/// Run the certification benchmark: compare Clifford circuits across
/// state-vector and stabilizer backends, measuring TVD.
pub fn run_certification_benchmark(
    seed: u64,
    num_circuits: usize,
    shots: u32,
) -> CertificationBenchmark {
    let mut rng = StdRng::seed_from_u64(seed);
    let tvd_bound = 0.15;
    let mut certified = 0usize;
    let mut max_tvd = 0.0_f64;
    let mut tvd_sum = 0.0_f64;
    let mut tested = 0usize;

    for i in 0..num_circuits {
        let circuit = gen_certifiable_circuit(&mut rng);
        if !is_clifford_circuit(&circuit) || circuit.num_qubits() > 20 {
            continue;
        }

        tested += 1;
        let shot_seed = seed.wrapping_add(i as u64 * 9973);

        // Run on state-vector backend.
        let sv_result = Simulator::run_shots(&circuit, shots, Some(shot_seed));
        let sv_counts = match sv_result {
            Ok(r) => r.counts,
            Err(_) => continue,
        };

        // Run on stabilizer backend.
        let stab_counts = run_stabilizer_shots(&circuit, shots, shot_seed);

        // Compute TVD.
        let tvd = total_variation_distance(&sv_counts, &stab_counts);
        tvd_sum += tvd;
        if tvd > max_tvd {
            max_tvd = tvd;
        }
        if tvd <= tvd_bound {
            certified += 1;
        }
    }

    let avg_tvd = if tested > 0 {
        tvd_sum / tested as f64
    } else {
        0.0
    };
    let cert_rate = if tested > 0 {
        certified as f64 / tested as f64
    } else {
        0.0
    };

    CertificationBenchmark {
        circuits_tested: tested,
        certified,
        certification_rate: cert_rate,
        max_tvd,
        avg_tvd,
        tvd_bound,
    }
}

fn gen_certifiable_circuit(rng: &mut StdRng) -> QuantumCircuit {
    let n = rng.gen_range(2..=10);
    let mut circ = QuantumCircuit::new(n);
    circ.h(0);
    for q in 0..(n - 1) {
        circ.cnot(q, q + 1);
    }
    let extras = rng.gen_range(0..n * 2);
    for _ in 0..extras {
        let q = rng.gen_range(0..n);
        match rng.gen_range(0..4) {
            0 => {
                circ.h(q);
            }
            1 => {
                circ.s(q);
            }
            2 => {
                circ.x(q);
            }
            _ => {
                circ.z(q);
            }
        }
    }
    // Add measurements for all qubits.
    for q in 0..n {
        circ.measure(q);
    }
    circ
}

// ---------------------------------------------------------------------------
// Master benchmark runner
// ---------------------------------------------------------------------------

/// Aggregated report from all four proof-point benchmarks.
pub struct FullBenchmarkReport {
    pub routing: RoutingBenchmark,
    pub entanglement: EntanglementBudgetBenchmark,
    pub decoder: Vec<DecoderBenchmarkResult>,
    pub certification: CertificationBenchmark,
    pub total_time_ms: u64,
}

/// Run all four benchmarks with a single seed for reproducibility.
pub fn run_full_benchmark(seed: u64) -> FullBenchmarkReport {
    let start = Instant::now();

    let routing = run_routing_benchmark(seed, 1000);
    let entanglement = run_entanglement_benchmark(seed.wrapping_add(1), 200);
    let decoder = run_decoder_benchmark(
        seed.wrapping_add(2),
        &[3, 5, 7, 9, 11, 13, 15, 17, 21, 25],
        100,
    );
    let certification = run_certification_benchmark(seed.wrapping_add(3), 100, 500);

    let total_time_ms = start.elapsed().as_millis() as u64;

    FullBenchmarkReport {
        routing,
        entanglement,
        decoder,
        certification,
        total_time_ms,
    }
}

/// Format a full benchmark report as a human-readable text summary.
pub fn format_report(report: &FullBenchmarkReport) -> String {
    let mut out = String::with_capacity(2048);

    out.push_str("=== ruqu-core Full Benchmark Report ===\n\n");

    // -- Routing --
    out.push_str("--- Proof 1: Cost-Model Routing ---\n");
    out.push_str(&format!(
        "  Circuits tested: {}\n",
        report.routing.num_circuits
    ));
    out.push_str(&format!(
        "  Planner win rate vs naive: {:.1}%\n",
        report.routing.planner_win_rate_vs_naive()
    ));
    out.push_str(&format!(
        "  Median speedup vs naive:  {:.2}x\n",
        report.routing.median_speedup_vs_naive()
    ));
    let mut heuristic_speedups: Vec<f64> = report
        .routing
        .results
        .iter()
        .map(|r| r.speedup_vs_heuristic)
        .collect();
    heuristic_speedups.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_h = if heuristic_speedups.is_empty() {
        1.0
    } else {
        heuristic_speedups[heuristic_speedups.len() / 2]
    };
    out.push_str(&format!(
        "  Median speedup vs heuristic: {:.2}x\n\n",
        median_h
    ));

    // -- Entanglement --
    out.push_str("--- Proof 2: Entanglement Budgeting ---\n");
    let eb = &report.entanglement;
    out.push_str(&format!("  Circuits tested: {}\n", eb.circuits_tested));
    out.push_str(&format!("  Total segments:  {}\n", eb.segments_total));
    out.push_str(&format!(
        "  Within budget:   {} ({:.1}%)\n",
        eb.segments_within_budget,
        if eb.segments_total > 0 {
            eb.segments_within_budget as f64 / eb.segments_total as f64 * 100.0
        } else {
            0.0
        }
    ));
    out.push_str(&format!(
        "  Max violation:   {:.2}%\n",
        eb.max_violation * 100.0
    ));
    out.push_str(&format!(
        "  Decomposition overhead: {:.1}%\n\n",
        eb.decomposition_overhead_pct
    ));

    // -- Decoder --
    out.push_str("--- Proof 3: Adaptive Decoder Latency ---\n");
    out.push_str("  distance | UF avg (ns) | Part avg (ns) | speedup | UF acc  | Part acc\n");
    out.push_str("  ---------+-------------+---------------+---------+---------+---------\n");
    for d in &report.decoder {
        out.push_str(&format!(
            "  {:>7}  | {:>11.0} | {:>13.0} | {:>6.2}x | {:>6.1}% | {:>6.1}%\n",
            d.distance,
            d.union_find_avg_ns,
            d.partitioned_avg_ns,
            d.speedup,
            d.union_find_accuracy * 100.0,
            d.partitioned_accuracy * 100.0,
        ));
    }
    out.push('\n');

    // -- Certification --
    out.push_str("--- Proof 4: Cross-Backend Certification ---\n");
    let c = &report.certification;
    out.push_str(&format!("  Circuits tested:      {}\n", c.circuits_tested));
    out.push_str(&format!("  Certified:            {}\n", c.certified));
    out.push_str(&format!(
        "  Certification rate:   {:.1}%\n",
        c.certification_rate * 100.0
    ));
    out.push_str(&format!("  Max TVD observed:     {:.6}\n", c.max_tvd));
    out.push_str(&format!("  Avg TVD:              {:.6}\n", c.avg_tvd));
    out.push_str(&format!("  TVD bound:            {:.6}\n\n", c.tvd_bound));

    // -- Summary --
    out.push_str(&format!(
        "Total benchmark time: {} ms\n",
        report.total_time_ms
    ));

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_benchmark_runs() {
        let bench = run_routing_benchmark(42, 50);
        assert_eq!(bench.num_circuits, 50);
        assert_eq!(bench.results.len(), 50);
        assert!(bench.planner_win_rate_vs_naive() > 0.0);
    }

    #[test]
    fn test_entanglement_benchmark_runs() {
        let bench = run_entanglement_benchmark(42, 20);
        assert_eq!(bench.circuits_tested, 20);
        assert!(bench.segments_total > 0);
    }

    #[test]
    fn test_decoder_benchmark_runs() {
        let results = run_decoder_benchmark(42, &[3, 5, 7], 10);
        assert_eq!(results.len(), 3);
        for r in &results {
            assert!(r.union_find_avg_ns >= 0.0);
            assert!(r.partitioned_avg_ns >= 0.0);
        }
    }

    #[test]
    fn test_certification_benchmark_runs() {
        let bench = run_certification_benchmark(42, 10, 100);
        assert!(bench.circuits_tested > 0);
        assert!(bench.certification_rate >= 0.0);
        assert!(bench.certification_rate <= 1.0);
    }

    #[test]
    fn test_format_report_nonempty() {
        let report = FullBenchmarkReport {
            routing: run_routing_benchmark(0, 10),
            entanglement: run_entanglement_benchmark(0, 5),
            decoder: run_decoder_benchmark(0, &[3, 5], 5),
            certification: run_certification_benchmark(0, 5, 50),
            total_time_ms: 42,
        };
        let text = format_report(&report);
        assert!(text.contains("Proof 1"));
        assert!(text.contains("Proof 2"));
        assert!(text.contains("Proof 3"));
        assert!(text.contains("Proof 4"));
        assert!(text.contains("Total benchmark time"));
    }

    #[test]
    fn test_routing_speedup_for_clifford() {
        // Pure Clifford circuit: planner should choose Stabilizer,
        // which is faster than naive StateVector.
        let mut circ = QuantumCircuit::new(50);
        for q in 0..50 {
            circ.h(q);
        }
        for q in 0..49 {
            circ.cnot(q, q + 1);
        }
        let plan = plan_execution(&circ, &PlannerConfig::default());
        assert_eq!(plan.backend, BackendType::Stabilizer);
        let planner_ns = predicted_runtime_ns(&circ, plan.backend);
        let naive_ns = predicted_runtime_ns(&circ, BackendType::StateVector);
        assert!(
            planner_ns < naive_ns,
            "Stabilizer should be faster than SV for 50-qubit Clifford"
        );
    }
}
