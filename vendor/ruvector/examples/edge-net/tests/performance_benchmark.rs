//! Performance Benchmark Suite for edge-net WASM Library
//!
//! Comprehensive benchmarks measuring operations per second and latency statistics.
//! Run with: `cargo test --test performance_benchmark --release -- --nocapture`

use std::time::{Duration, Instant};

// ============================================================================
// Benchmark Statistics
// ============================================================================

#[derive(Debug, Clone)]
pub struct BenchmarkStats {
    pub name: String,
    pub iterations: usize,
    pub total_duration: Duration,
    pub mean_ns: f64,
    pub median_ns: f64,
    pub p95_ns: f64,
    pub p99_ns: f64,
    pub min_ns: f64,
    pub max_ns: f64,
    pub ops_per_sec: f64,
}

impl BenchmarkStats {
    pub fn from_durations(name: &str, durations: &mut [Duration]) -> Self {
        let iterations = durations.len();
        let total_duration: Duration = durations.iter().sum();

        // Convert to nanoseconds for statistics
        let mut ns_values: Vec<f64> = durations.iter()
            .map(|d| d.as_nanos() as f64)
            .collect();

        ns_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mean_ns = ns_values.iter().sum::<f64>() / iterations as f64;
        let median_ns = ns_values[iterations / 2];
        let p95_ns = ns_values[(iterations as f64 * 0.95) as usize];
        let p99_ns = ns_values[(iterations as f64 * 0.99) as usize];
        let min_ns = ns_values[0];
        let max_ns = ns_values[iterations - 1];
        let ops_per_sec = 1_000_000_000.0 / mean_ns;

        BenchmarkStats {
            name: name.to_string(),
            iterations,
            total_duration,
            mean_ns,
            median_ns,
            p95_ns,
            p99_ns,
            min_ns,
            max_ns,
            ops_per_sec,
        }
    }

    pub fn print_report(&self) {
        println!("\n=== {} ===", self.name);
        println!("  Iterations:    {:>12}", self.iterations);
        println!("  Total time:    {:>12.3} ms", self.total_duration.as_secs_f64() * 1000.0);
        println!("  Ops/sec:       {:>12.0}", self.ops_per_sec);
        println!("  Mean:          {:>12.1} ns ({:.3} us)", self.mean_ns, self.mean_ns / 1000.0);
        println!("  Median:        {:>12.1} ns ({:.3} us)", self.median_ns, self.median_ns / 1000.0);
        println!("  P95:           {:>12.1} ns ({:.3} us)", self.p95_ns, self.p95_ns / 1000.0);
        println!("  P99:           {:>12.1} ns ({:.3} us)", self.p99_ns, self.p99_ns / 1000.0);
        println!("  Min:           {:>12.1} ns", self.min_ns);
        println!("  Max:           {:>12.1} ns ({:.3} us)", self.max_ns, self.max_ns / 1000.0);
    }
}

/// Run a benchmark with warmup and return statistics
fn run_benchmark<F>(name: &str, iterations: usize, warmup: usize, mut f: F) -> BenchmarkStats
where
    F: FnMut() -> ()
{
    // Warmup phase
    for _ in 0..warmup {
        f();
    }

    // Measurement phase
    let mut durations = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        f();
        durations.push(start.elapsed());
    }

    BenchmarkStats::from_durations(name, &mut durations)
}

// ============================================================================
// Test Module
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use ruvector_edge_net::credits::{WasmCreditLedger, qdag::QDAGLedger};
    use ruvector_edge_net::rac::{
        CoherenceEngine, Event, EventKind, AssertEvent, Ruvector, EvidenceRef,
        QuarantineManager,
    };
    use ruvector_edge_net::learning::{
        TrajectoryTracker, SpikeDrivenAttention,
        LearnedPattern, MultiHeadAttention,
    };
    use ruvector_edge_net::swarm::consensus::EntropyConsensus;
    use sha2::{Sha256, Digest};

    const ITERATIONS: usize = 1000;
    const WARMUP: usize = 100;

    // ========================================================================
    // Credit Operations Benchmarks
    // ========================================================================

    #[test]
    fn benchmark_credit_operations() {
        println!("\n");
        println!("================================================================================");
        println!("                    CREDIT OPERATIONS BENCHMARKS");
        println!("================================================================================");

        // Credit operation
        let mut ledger = WasmCreditLedger::new("bench-node".to_string()).unwrap();
        let stats = run_benchmark("Credit Operation", ITERATIONS, WARMUP, || {
            let _ = ledger.credit(100, "task");
        });
        stats.print_report();

        // Debit operation (need balance first)
        let mut ledger = WasmCreditLedger::new("bench-node".to_string()).unwrap();
        let _ = ledger.credit(10_000_000, "initial");
        let stats = run_benchmark("Debit Operation", ITERATIONS, WARMUP, || {
            let _ = ledger.deduct(10);
        });
        stats.print_report();

        // Balance lookup (after many operations)
        let mut ledger = WasmCreditLedger::new("bench-node".to_string()).unwrap();
        for i in 0..1000 {
            let _ = ledger.credit(100, &format!("task-{}", i));
        }
        let stats = run_benchmark("Balance Lookup (1K history)", ITERATIONS, WARMUP, || {
            let _ = ledger.balance();
        });
        stats.print_report();

        // Large history balance lookup
        let mut ledger = WasmCreditLedger::new("bench-node".to_string()).unwrap();
        for i in 0..10000 {
            let _ = ledger.credit(100, &format!("task-{}", i));
        }
        let stats = run_benchmark("Balance Lookup (10K history)", ITERATIONS, WARMUP, || {
            let _ = ledger.balance();
        });
        stats.print_report();
    }

    // ========================================================================
    // QDAG Transaction Benchmarks
    // ========================================================================

    #[test]
    fn benchmark_qdag_operations() {
        println!("\n");
        println!("================================================================================");
        println!("                    QDAG TRANSACTION BENCHMARKS");
        println!("================================================================================");

        // QDAG ledger creation
        let stats = run_benchmark("QDAG Ledger Creation", ITERATIONS, WARMUP, || {
            let _ = QDAGLedger::new();
        });
        stats.print_report();

        // Balance query
        let ledger = QDAGLedger::new();
        let stats = run_benchmark("QDAG Balance Query", ITERATIONS, WARMUP, || {
            let _ = ledger.balance("test-node");
        });
        stats.print_report();

        // Tip count query
        let ledger = QDAGLedger::new();
        let stats = run_benchmark("QDAG Tip Count", ITERATIONS, WARMUP, || {
            let _ = ledger.tip_count();
        });
        stats.print_report();

        // Transaction count query
        let ledger = QDAGLedger::new();
        let stats = run_benchmark("QDAG Transaction Count", ITERATIONS, WARMUP, || {
            let _ = ledger.transaction_count();
        });
        stats.print_report();
    }

    // ========================================================================
    // RAC Coherence Engine Benchmarks
    // ========================================================================

    fn create_test_event(i: usize) -> Event {
        let proposition = format!("test-proposition-{}", i);
        let mut hasher = Sha256::new();
        hasher.update(proposition.as_bytes());
        hasher.update(&i.to_le_bytes());
        let id_bytes = hasher.finalize();
        let mut event_id = [0u8; 32];
        event_id.copy_from_slice(&id_bytes);

        Event {
            id: event_id,
            prev: None,
            ts_unix_ms: 1704067200000 + i as u64, // Fixed timestamp for determinism
            author: [0u8; 32],
            context: [0u8; 32],
            ruvector: Ruvector::new(vec![0.1, 0.2, 0.3]),
            kind: EventKind::Assert(AssertEvent {
                proposition: proposition.as_bytes().to_vec(),
                evidence: vec![EvidenceRef::hash(&[1, 2, 3])],
                confidence: 0.9,
                expires_at_unix_ms: None,
            }),
            sig: vec![0u8; 64],
        }
    }

    #[test]
    fn benchmark_rac_coherence_operations() {
        println!("\n");
        println!("================================================================================");
        println!("                    RAC COHERENCE ENGINE BENCHMARKS");
        println!("================================================================================");

        // Event ingestion
        let mut engine = CoherenceEngine::new();
        let mut counter = 0usize;
        let stats = run_benchmark("Event Ingestion", ITERATIONS, WARMUP, || {
            let event = create_test_event(counter);
            engine.ingest(event);
            counter += 1;
        });
        stats.print_report();

        // Merkle root computation
        let mut engine = CoherenceEngine::new();
        for i in 0..100 {
            engine.ingest(create_test_event(i));
        }
        let stats = run_benchmark("Merkle Root (100 events)", ITERATIONS, WARMUP, || {
            let _ = engine.get_merkle_root();
        });
        stats.print_report();

        // Stats retrieval
        let mut engine = CoherenceEngine::new();
        for i in 0..100 {
            engine.ingest(create_test_event(i));
        }
        let stats = run_benchmark("Get Stats", ITERATIONS, WARMUP, || {
            let _ = engine.get_stats();
        });
        stats.print_report();

        // Event count
        let mut engine = CoherenceEngine::new();
        for i in 0..1000 {
            engine.ingest(create_test_event(i));
        }
        let stats = run_benchmark("Event Count (1K events)", ITERATIONS, WARMUP, || {
            let _ = engine.event_count();
        });
        stats.print_report();

        // Quarantine check
        let quarantine = QuarantineManager::new();
        for i in 0..100 {
            quarantine.set_level(&format!("claim-{}", i), (i % 4) as u8);
        }
        let stats = run_benchmark("Quarantine Check", ITERATIONS, WARMUP, || {
            let _ = quarantine.can_use("claim-50");
        });
        stats.print_report();

        // Quarantine set level
        let quarantine = QuarantineManager::new();
        let mut counter = 0usize;
        let stats = run_benchmark("Quarantine Set Level", ITERATIONS, WARMUP, || {
            quarantine.set_level(&format!("claim-{}", counter), (counter % 4) as u8);
            counter += 1;
        });
        stats.print_report();

        // Conflict count
        let mut engine = CoherenceEngine::new();
        for i in 0..100 {
            engine.ingest(create_test_event(i));
        }
        let stats = run_benchmark("Conflict Count", ITERATIONS, WARMUP, || {
            let _ = engine.conflict_count();
        });
        stats.print_report();

        // Bulk event ingestion (1K events)
        let stats = run_benchmark("Bulk Ingest 1K Events", 10, 2, || {
            let mut engine = CoherenceEngine::new();
            for i in 0..1000 {
                engine.ingest(create_test_event(i));
            }
        });
        stats.print_report();
    }

    // ========================================================================
    // Learning Engine Benchmarks
    // ========================================================================

    /// Create a trajectory JSON without using js_sys::Date
    fn create_trajectory_json(counter: usize) -> String {
        format!(
            r#"{{"task_vector":[{},0.5,0.3],"latency_ms":100,"energy_spent":50,"energy_earned":100,"success":true,"executor_id":"node-{}","timestamp":1704067200000}}"#,
            counter as f32 * 0.01,
            counter % 10
        )
    }

    #[test]
    fn benchmark_learning_operations() {
        println!("\n");
        println!("================================================================================");
        println!("                    LEARNING ENGINE BENCHMARKS");
        println!("================================================================================");

        // NOTE: ReasoningBank.store() and lookup() use js_sys::Date::now() which
        // doesn't work on native targets. Testing pattern operations that work natively.

        // Trajectory recording (works on native)
        let tracker = TrajectoryTracker::new(1000);
        let mut counter = 0usize;
        let stats = run_benchmark("Trajectory Record", ITERATIONS, WARMUP, || {
            let json = create_trajectory_json(counter);
            tracker.record(&json);
            counter += 1;
        });
        stats.print_report();

        // Trajectory stats
        let tracker = TrajectoryTracker::new(1000);
        for i in 0..500 {
            let json = create_trajectory_json(i);
            tracker.record(&json);
        }
        let stats = run_benchmark("Trajectory Stats (500 entries)", ITERATIONS, WARMUP, || {
            let _ = tracker.get_stats();
        });
        stats.print_report();

        // Pattern similarity computation (pure computation, no WASM deps)
        let pattern = LearnedPattern::new(
            vec![1.0, 0.5, 0.3, 0.2, 0.1, 0.05, 0.02, 0.01],
            0.8,
            100,
            0.9,
            10,
            50.0,
            Some(0.95),
        );
        let query = vec![0.9, 0.6, 0.25, 0.15, 0.12, 0.04, 0.03, 0.015];
        let stats = run_benchmark("Pattern Similarity (8 dim)", ITERATIONS, WARMUP, || {
            let _ = pattern.similarity(&query);
        });
        stats.print_report();

        // Pattern similarity (higher dimension)
        let pattern = LearnedPattern::new(
            (0..64).map(|i| (i as f32 + 1.0) / 100.0).collect(),
            0.8,
            100,
            0.9,
            10,
            50.0,
            Some(0.95),
        );
        let query: Vec<f32> = (0..64).map(|i| (i as f32 + 2.0) / 100.0).collect();
        let stats = run_benchmark("Pattern Similarity (64 dim)", ITERATIONS, WARMUP, || {
            let _ = pattern.similarity(&query);
        });
        stats.print_report();

        // Pattern similarity (high dimension)
        let pattern = LearnedPattern::new(
            (0..256).map(|i| (i as f32 + 1.0) / 1000.0).collect(),
            0.8,
            100,
            0.9,
            10,
            50.0,
            Some(0.95),
        );
        let query: Vec<f32> = (0..256).map(|i| (i as f32 + 2.0) / 1000.0).collect();
        let stats = run_benchmark("Pattern Similarity (256 dim)", ITERATIONS, WARMUP, || {
            let _ = pattern.similarity(&query);
        });
        stats.print_report();

        // Trajectory count
        let tracker = TrajectoryTracker::new(1000);
        for i in 0..500 {
            let json = create_trajectory_json(i);
            tracker.record(&json);
        }
        let stats = run_benchmark("Trajectory Count", ITERATIONS, WARMUP, || {
            let _ = tracker.count();
        });
        stats.print_report();
    }

    // ========================================================================
    // Spike-Driven Attention Benchmarks
    // ========================================================================

    #[test]
    fn benchmark_spike_attention() {
        println!("\n");
        println!("================================================================================");
        println!("                    SPIKE-DRIVEN ATTENTION BENCHMARKS");
        println!("================================================================================");

        // Spike encoding (small)
        let attn = SpikeDrivenAttention::new();
        let values: Vec<i8> = (0..64).map(|i| (i % 128) as i8).collect();
        let stats = run_benchmark("Spike Encode 64 values", ITERATIONS, WARMUP, || {
            let _ = attn.encode_spikes(&values);
        });
        stats.print_report();

        // Spike encoding (medium)
        let values: Vec<i8> = (0..256).map(|i| (i % 128) as i8).collect();
        let stats = run_benchmark("Spike Encode 256 values", ITERATIONS, WARMUP, || {
            let _ = attn.encode_spikes(&values);
        });
        stats.print_report();

        // Spike encoding (large)
        let values: Vec<i8> = (0..1024).map(|i| (i % 128) as i8).collect();
        let stats = run_benchmark("Spike Encode 1024 values", ITERATIONS, WARMUP, || {
            let _ = attn.encode_spikes(&values);
        });
        stats.print_report();

        // Spike attention (seq=16, dim=64)
        let attn = SpikeDrivenAttention::new();
        let values: Vec<i8> = (0..64).map(|i| (i % 128 - 64) as i8).collect();
        let spikes = attn.encode_spikes(&values);
        let stats = run_benchmark("Spike Attention seq=16, dim=64", ITERATIONS, WARMUP, || {
            let _ = attn.attention(&spikes[0..16.min(spikes.len())], &spikes[0..16.min(spikes.len())], &spikes[0..64.min(spikes.len())]);
        });
        stats.print_report();

        // Energy ratio calculation
        let attn = SpikeDrivenAttention::new();
        let stats = run_benchmark("Energy Ratio Calculation", ITERATIONS, WARMUP, || {
            let _ = attn.energy_ratio(64, 256);
        });
        stats.print_report();
    }

    // ========================================================================
    // Multi-Head Attention Benchmarks
    // ========================================================================

    #[test]
    fn benchmark_multi_head_attention() {
        println!("\n");
        println!("================================================================================");
        println!("                    MULTI-HEAD ATTENTION BENCHMARKS");
        println!("================================================================================");

        // 2 heads, dim 8
        let attn = MultiHeadAttention::new(8, 2);
        let query = vec![1.0f32; 8];
        let key = vec![0.5f32; 8];
        let val = vec![1.0f32; 8];
        let keys: Vec<&[f32]> = vec![key.as_slice()];
        let values: Vec<&[f32]> = vec![val.as_slice()];
        let stats = run_benchmark("MHA 2 heads, dim=8, 1 KV", ITERATIONS, WARMUP, || {
            let _ = attn.compute(&query, &keys, &values);
        });
        stats.print_report();

        // 4 heads, dim 64
        let attn = MultiHeadAttention::new(64, 4);
        let query = vec![1.0f32; 64];
        let key = vec![0.5f32; 64];
        let val = vec![1.0f32; 64];
        let keys: Vec<&[f32]> = vec![key.as_slice()];
        let values: Vec<&[f32]> = vec![val.as_slice()];
        let stats = run_benchmark("MHA 4 heads, dim=64, 1 KV", ITERATIONS, WARMUP, || {
            let _ = attn.compute(&query, &keys, &values);
        });
        stats.print_report();

        // 8 heads, dim 256, 10 keys
        let attn = MultiHeadAttention::new(256, 8);
        let query = vec![1.0f32; 256];
        let keys_data: Vec<Vec<f32>> = (0..10).map(|_| vec![0.5f32; 256]).collect();
        let values_data: Vec<Vec<f32>> = (0..10).map(|_| vec![1.0f32; 256]).collect();
        let keys: Vec<&[f32]> = keys_data.iter().map(|k| k.as_slice()).collect();
        let values: Vec<&[f32]> = values_data.iter().map(|v| v.as_slice()).collect();
        let stats = run_benchmark("MHA 8 heads, dim=256, 10 KV", ITERATIONS, WARMUP, || {
            let _ = attn.compute(&query, &keys, &values);
        });
        stats.print_report();
    }

    // ========================================================================
    // Consensus Benchmarks
    // ========================================================================

    #[test]
    fn benchmark_consensus_operations() {
        println!("\n");
        println!("================================================================================");
        println!("                    ENTROPY CONSENSUS BENCHMARKS");
        println!("================================================================================");

        // Consensus creation
        let stats = run_benchmark("Consensus Creation", ITERATIONS, WARMUP, || {
            let _ = EntropyConsensus::new();
        });
        stats.print_report();

        // Set belief
        let consensus = EntropyConsensus::new();
        let mut counter = 0u64;
        let stats = run_benchmark("Set Belief", ITERATIONS, WARMUP, || {
            consensus.set_belief(counter, 0.5);
            counter += 1;
        });
        stats.print_report();

        // Get belief
        let consensus = EntropyConsensus::new();
        for i in 0..100 {
            consensus.set_belief(i, 0.5);
        }
        let stats = run_benchmark("Get Belief", ITERATIONS, WARMUP, || {
            let _ = consensus.get_belief(50);
        });
        stats.print_report();

        // Entropy calculation
        let consensus = EntropyConsensus::new();
        for i in 0..10 {
            consensus.set_belief(i, (i as f32 + 1.0) / 55.0);
        }
        let stats = run_benchmark("Entropy Calculation (10 options)", ITERATIONS, WARMUP, || {
            let _ = consensus.entropy();
        });
        stats.print_report();

        // Convergence check
        let consensus = EntropyConsensus::new();
        consensus.set_belief(1, 0.95);
        consensus.set_belief(2, 0.05);
        let stats = run_benchmark("Convergence Check", ITERATIONS, WARMUP, || {
            let _ = consensus.converged();
        });
        stats.print_report();

        // Get stats
        let consensus = EntropyConsensus::new();
        for i in 0..10 {
            consensus.set_belief(i, (i as f32 + 1.0) / 55.0);
        }
        let stats = run_benchmark("Get Consensus Stats", ITERATIONS, WARMUP, || {
            let _ = consensus.get_stats();
        });
        stats.print_report();
    }

    // ========================================================================
    // Vector Operations Benchmarks (HNSW-style search simulation)
    // ========================================================================

    #[test]
    fn benchmark_vector_operations() {
        println!("\n");
        println!("================================================================================");
        println!("                    VECTOR OPERATIONS BENCHMARKS");
        println!("================================================================================");

        // RuVector similarity
        let v1 = Ruvector::new(vec![1.0, 0.5, 0.3, 0.2, 0.1, 0.05, 0.02, 0.01]);
        let v2 = Ruvector::new(vec![0.9, 0.6, 0.25, 0.15, 0.12, 0.04, 0.03, 0.015]);
        let stats = run_benchmark("RuVector Similarity (8 dim)", ITERATIONS, WARMUP, || {
            let _ = v1.similarity(&v2);
        });
        stats.print_report();

        // RuVector similarity (higher dimension)
        let v1 = Ruvector::new((0..64).map(|i| (i as f32 + 1.0) / 100.0).collect());
        let v2 = Ruvector::new((0..64).map(|i| (i as f32 + 2.0) / 100.0).collect());
        let stats = run_benchmark("RuVector Similarity (64 dim)", ITERATIONS, WARMUP, || {
            let _ = v1.similarity(&v2);
        });
        stats.print_report();

        // RuVector similarity (high dimension)
        let v1 = Ruvector::new((0..256).map(|i| (i as f32 + 1.0) / 1000.0).collect());
        let v2 = Ruvector::new((0..256).map(|i| (i as f32 + 2.0) / 1000.0).collect());
        let stats = run_benchmark("RuVector Similarity (256 dim)", ITERATIONS, WARMUP, || {
            let _ = v1.similarity(&v2);
        });
        stats.print_report();

        // RuVector distance
        let v1 = Ruvector::new((0..64).map(|i| (i as f32 + 1.0) / 100.0).collect());
        let v2 = Ruvector::new((0..64).map(|i| (i as f32 + 2.0) / 100.0).collect());
        let stats = run_benchmark("RuVector L2 Distance (64 dim)", ITERATIONS, WARMUP, || {
            let _ = v1.distance(&v2);
        });
        stats.print_report();

        // RuVector drift
        let v1 = Ruvector::new((0..64).map(|i| (i as f32 + 1.0) / 100.0).collect());
        let v2 = Ruvector::new((0..64).map(|i| (i as f32 + 5.0) / 100.0).collect());
        let stats = run_benchmark("RuVector Drift (64 dim)", ITERATIONS, WARMUP, || {
            let _ = v1.drift_from(&v2);
        });
        stats.print_report();

        // Brute-force kNN search (1K vectors, 64 dim)
        let vectors: Vec<Ruvector> = (0..1000)
            .map(|i| Ruvector::new((0..64).map(|j| ((i * 64 + j) as f32 % 1000.0) / 1000.0).collect()))
            .collect();
        let query = Ruvector::new((0..64).map(|i| (i as f32) / 64.0).collect());
        let stats = run_benchmark("Brute kNN k=10 (1K vectors, 64 dim)", 100, 10, || {
            let mut results: Vec<(usize, f64)> = vectors.iter()
                .enumerate()
                .map(|(i, v)| (i, query.similarity(v)))
                .collect();
            results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let _ = results.into_iter().take(10).collect::<Vec<_>>();
        });
        stats.print_report();
    }

    // ========================================================================
    // Integration Benchmarks
    // ========================================================================

    #[test]
    fn benchmark_integration_scenarios() {
        println!("\n");
        println!("================================================================================");
        println!("                    INTEGRATION SCENARIO BENCHMARKS");
        println!("================================================================================");

        // Combined trajectory + coherence operations
        let stats = run_benchmark("Trajectory + Coherence Round", 100, 10, || {
            let tracker = TrajectoryTracker::new(100);
            let mut coherence = CoherenceEngine::new();

            // Learning operations (trajectories work natively)
            for i in 0..10 {
                let json = create_trajectory_json(i);
                tracker.record(&json);
            }

            // Coherence operations
            for i in 0..10 {
                coherence.ingest(create_test_event(i));
            }

            // Get stats
            let _ = tracker.get_stats();
            let _ = coherence.get_stats();
        });
        stats.print_report();

        // Credit + Trajectory transaction
        let stats = run_benchmark("Credit + Trajectory Transaction", 100, 10, || {
            let mut ledger = WasmCreditLedger::new("bench-node".to_string()).unwrap();
            let _ = ledger.credit(1000, "initial");

            let tracker = TrajectoryTracker::new(100);

            // Simulate 10 task completions
            for i in 0..10 {
                // Record trajectory
                let json = create_trajectory_json(i);
                tracker.record(&json);

                // Credit earned
                let _ = ledger.credit(10, &format!("task-{}", i));
            }

            let _ = ledger.balance();
        });
        stats.print_report();

        // Full coherence cycle
        let stats = run_benchmark("Full Coherence Cycle (100 events)", 10, 2, || {
            let mut coherence = CoherenceEngine::new();

            // Ingest 100 events
            for i in 0..100 {
                coherence.ingest(create_test_event(i));
            }

            // Check various states
            let _ = coherence.event_count();
            let _ = coherence.conflict_count();
            let _ = coherence.quarantined_count();
            let _ = coherence.get_merkle_root();
            let _ = coherence.get_stats();
        });
        stats.print_report();
    }

    // ========================================================================
    // Summary Report
    // ========================================================================

    #[test]
    fn benchmark_summary() {
        println!("\n");
        println!("================================================================================");
        println!("                         PERFORMANCE BENCHMARK SUMMARY");
        println!("================================================================================");
        println!("");
        println!("Running all benchmarks to generate summary report...");
        println!("");

        let mut results: Vec<BenchmarkStats> = Vec::new();

        // Credit operations
        let mut ledger = WasmCreditLedger::new("bench".to_string()).unwrap();
        results.push(run_benchmark("Credit", ITERATIONS, WARMUP, || {
            let _ = ledger.credit(100, "task");
        }));

        let mut ledger = WasmCreditLedger::new("bench".to_string()).unwrap();
        let _ = ledger.credit(10_000_000, "initial");
        results.push(run_benchmark("Debit", ITERATIONS, WARMUP, || {
            let _ = ledger.deduct(10);
        }));

        // RAC operations
        let mut engine = CoherenceEngine::new();
        let mut counter = 0usize;
        results.push(run_benchmark("Event Ingest", ITERATIONS, WARMUP, || {
            engine.ingest(create_test_event(counter));
            counter += 1;
        }));

        // Trajectory recording (native-compatible)
        let tracker = TrajectoryTracker::new(1000);
        let mut counter = 0usize;
        results.push(run_benchmark("Trajectory Record", ITERATIONS, WARMUP, || {
            let json = create_trajectory_json(counter);
            tracker.record(&json);
            counter += 1;
        }));

        // Pattern similarity (native-compatible)
        let pattern = LearnedPattern::new(
            (0..64).map(|i| (i as f32 + 1.0) / 100.0).collect(),
            0.8, 100, 0.9, 10, 50.0, Some(0.95),
        );
        let query: Vec<f32> = (0..64).map(|i| (i as f32 + 2.0) / 100.0).collect();
        results.push(run_benchmark("Pattern Similarity", ITERATIONS, WARMUP, || {
            let _ = pattern.similarity(&query);
        }));

        // Vector operations
        let v1 = Ruvector::new((0..64).map(|i| (i as f32 + 1.0) / 100.0).collect());
        let v2 = Ruvector::new((0..64).map(|i| (i as f32 + 2.0) / 100.0).collect());
        results.push(run_benchmark("Vector Similarity", ITERATIONS, WARMUP, || {
            let _ = v1.similarity(&v2);
        }));

        // Consensus operations
        let consensus = EntropyConsensus::new();
        for i in 0..10 {
            consensus.set_belief(i, (i as f32 + 1.0) / 55.0);
        }
        results.push(run_benchmark("Entropy Calc", ITERATIONS, WARMUP, || {
            let _ = consensus.entropy();
        }));

        // Multi-head attention
        let attn = MultiHeadAttention::new(64, 4);
        let query = vec![1.0f32; 64];
        let key = vec![0.5f32; 64];
        let val = vec![1.0f32; 64];
        let keys: Vec<&[f32]> = vec![key.as_slice()];
        let values: Vec<&[f32]> = vec![val.as_slice()];
        results.push(run_benchmark("MHA 4h dim64", ITERATIONS, WARMUP, || {
            let _ = attn.compute(&query, &keys, &values);
        }));

        // Quarantine check
        let quarantine = QuarantineManager::new();
        for i in 0..100 {
            quarantine.set_level(&format!("claim-{}", i), (i % 4) as u8);
        }
        results.push(run_benchmark("Quarantine Check", ITERATIONS, WARMUP, || {
            let _ = quarantine.can_use("claim-50");
        }));

        // Spike attention
        let attn = SpikeDrivenAttention::new();
        let values: Vec<i8> = (0..64).map(|i| (i % 128) as i8).collect();
        results.push(run_benchmark("Spike Encode 64", ITERATIONS, WARMUP, || {
            let _ = attn.encode_spikes(&values);
        }));

        // Print summary table
        println!("\n");
        println!("┌─────────────────────────┬──────────────┬──────────────┬──────────────┬──────────────┐");
        println!("│ Operation               │ Ops/sec      │ Mean (us)    │ P95 (us)     │ P99 (us)     │");
        println!("├─────────────────────────┼──────────────┼──────────────┼──────────────┼──────────────┤");

        for stat in &results {
            println!("│ {:23} │ {:>12.0} │ {:>12.3} │ {:>12.3} │ {:>12.3} │",
                     if stat.name.len() > 23 { &stat.name[..23] } else { &stat.name },
                     stat.ops_per_sec,
                     stat.mean_ns / 1000.0,
                     stat.p95_ns / 1000.0,
                     stat.p99_ns / 1000.0);
        }

        println!("└─────────────────────────┴──────────────┴──────────────┴──────────────┴──────────────┘");

        // Identify slowest operations
        let mut sorted = results.clone();
        sorted.sort_by(|a, b| b.mean_ns.partial_cmp(&a.mean_ns).unwrap());

        println!("\n");
        println!("SLOWEST OPERATIONS (candidates for optimization):");
        println!("─────────────────────────────────────────────────");
        for (i, stat) in sorted.iter().take(3).enumerate() {
            println!("  {}. {} - {:.1} us mean ({:.0} ops/sec)",
                     i + 1, stat.name, stat.mean_ns / 1000.0, stat.ops_per_sec);
        }

        println!("\n");
        println!("FASTEST OPERATIONS:");
        println!("───────────────────");
        for (i, stat) in sorted.iter().rev().take(3).enumerate() {
            println!("  {}. {} - {:.1} us mean ({:.0} ops/sec)",
                     i + 1, stat.name, stat.mean_ns / 1000.0, stat.ops_per_sec);
        }

        println!("\n");
        println!("================================================================================");
        println!("                              BENCHMARK COMPLETE");
        println!("================================================================================");
    }
}
