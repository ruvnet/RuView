//! Criterion benchmarks for all 24 WASM edge vendor modules (ADR-041).
//!
//! Since #![feature(test)] requires nightly, we use a lightweight custom
//! benchmarking harness that works on stable Rust.  Each module is
//! benchmarked with 1000 iterations, reporting throughput in frames/sec
//! and latency in microseconds.
//!
//! Run with:
//!   cargo test -p wifi-densepose-wasm-edge --features std --test vendor_modules_bench --release -- --nocapture
//!
//! (This is placed in benches/ but registered as a [[test]] so it works on stable.)

use std::time::Instant;

// --- Signal Intelligence ---
use wifi_densepose_wasm_edge::sig_coherence_gate::CoherenceGate;
use wifi_densepose_wasm_edge::sig_flash_attention::FlashAttention;
use wifi_densepose_wasm_edge::sig_sparse_recovery::SparseRecovery;
use wifi_densepose_wasm_edge::sig_temporal_compress::TemporalCompressor;
use wifi_densepose_wasm_edge::sig_optimal_transport::OptimalTransportDetector;
use wifi_densepose_wasm_edge::sig_mincut_person_match::PersonMatcher;

// --- Adaptive Learning ---
use wifi_densepose_wasm_edge::lrn_dtw_gesture_learn::GestureLearner;
use wifi_densepose_wasm_edge::lrn_anomaly_attractor::AttractorDetector;
use wifi_densepose_wasm_edge::lrn_meta_adapt::MetaAdapter;
use wifi_densepose_wasm_edge::lrn_ewc_lifelong::EwcLifelong;

// --- Spatial Reasoning ---
use wifi_densepose_wasm_edge::spt_micro_hnsw::MicroHnsw;
use wifi_densepose_wasm_edge::spt_pagerank_influence::PageRankInfluence;
use wifi_densepose_wasm_edge::spt_spiking_tracker::SpikingTracker;

// --- Temporal Analysis ---
use wifi_densepose_wasm_edge::tmp_pattern_sequence::PatternSequenceAnalyzer;
use wifi_densepose_wasm_edge::tmp_temporal_logic_guard::{TemporalLogicGuard, FrameInput};
use wifi_densepose_wasm_edge::tmp_goap_autonomy::GoapPlanner;

// --- AI Security ---
use wifi_densepose_wasm_edge::ais_prompt_shield::PromptShield;
use wifi_densepose_wasm_edge::ais_behavioral_profiler::BehavioralProfiler;

// --- Quantum-Inspired ---
use wifi_densepose_wasm_edge::qnt_quantum_coherence::QuantumCoherenceMonitor;
use wifi_densepose_wasm_edge::qnt_interference_search::InterferenceSearch;

// --- Autonomous Systems ---
use wifi_densepose_wasm_edge::aut_psycho_symbolic::PsychoSymbolicEngine;
use wifi_densepose_wasm_edge::aut_self_healing_mesh::SelfHealingMesh;

// --- Exotic / Research ---
use wifi_densepose_wasm_edge::exo_time_crystal::TimeCrystalDetector;
use wifi_densepose_wasm_edge::exo_hyperbolic_space::HyperbolicEmbedder;

// ==========================================================================
// Helpers
// ==========================================================================

const BENCH_ITERS: usize = 1000;

fn synthetic_phases(n: usize, seed: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity(n);
    let mut s = seed;
    for _ in 0..n {
        s = s.wrapping_mul(1103515245).wrapping_add(12345);
        v.push(((s >> 16) as f32 / 32768.0) * 6.2832 - 3.1416);
    }
    v
}

fn synthetic_amplitudes(n: usize, seed: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity(n);
    let mut s = seed;
    for _ in 0..n {
        s = s.wrapping_mul(1103515245).wrapping_add(12345);
        v.push(((s >> 16) as f32 / 32768.0) * 10.0 + 0.1);
    }
    v
}

#[allow(dead_code)]
struct BenchResult {
    name: &'static str,
    tier: &'static str,
    total_ns: u128,
    iters: usize,
    mean_us: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    fps_at_20hz_headroom: f64,
}

fn bench_module(name: &'static str, tier: &'static str, mut body: impl FnMut(usize)) -> BenchResult {
    // Warm up.
    for i in 0..50 { body(i); }

    let mut durations_ns: Vec<u128> = Vec::with_capacity(BENCH_ITERS);
    let start = Instant::now();
    for i in 0..BENCH_ITERS {
        let t0 = Instant::now();
        body(50 + i);
        durations_ns.push(t0.elapsed().as_nanos());
    }
    let total_ns = start.elapsed().as_nanos();

    durations_ns.sort();
    let to_us = |ns: u128| ns as f64 / 1000.0;
    let mean_us = durations_ns.iter().sum::<u128>() as f64 / durations_ns.len() as f64 / 1000.0;
    let p50_us = to_us(durations_ns[durations_ns.len() / 2]);
    let p95_us = to_us(durations_ns[(durations_ns.len() as f64 * 0.95) as usize]);
    let p99_us = to_us(durations_ns[(durations_ns.len() as f64 * 0.99) as usize]);

    // At 20 Hz (50ms per frame), how much headroom do we have?
    let budget_us = match tier {
        "L" => 2000.0,
        "S" => 5000.0,
        "H" => 10000.0,
        _ => 10000.0,
    };
    let fps_headroom = budget_us / p99_us;

    BenchResult { name, tier, total_ns, iters: BENCH_ITERS, mean_us, p50_us, p95_us, p99_us, fps_at_20hz_headroom: fps_headroom }
}

fn print_bench_table(results: &[BenchResult]) {
    eprintln!();
    eprintln!("  {:<36} {:>4} {:>10} {:>10} {:>10} {:>10} {:>8}",
        "Module", "Tier", "mean(us)", "p50(us)", "p95(us)", "p99(us)", "headroom");
    eprintln!("  {:-<36} {:-<4} {:-<10} {:-<10} {:-<10} {:-<10} {:-<8}",
        "", "", "", "", "", "", "");
    for r in results {
        eprintln!("  {:<36} {:>4} {:>10.1} {:>10.1} {:>10.1} {:>10.1} {:>7.0}x",
            r.name, r.tier, r.mean_us, r.p50_us, r.p95_us, r.p99_us, r.fps_at_20hz_headroom);
    }
    eprintln!();
}

// ==========================================================================
// Main Benchmark Test
// ==========================================================================

#[test]
fn bench_all_24_vendor_modules() {
    eprintln!("\n========== VENDOR MODULE BENCHMARKS ({} iterations) ==========", BENCH_ITERS);

    let mut results = Vec::new();

    // --- Signal Intelligence (6 modules) ---
    {
        let mut m = CoherenceGate::new();
        results.push(bench_module("sig_coherence_gate", "L", |i| {
            let p = synthetic_phases(32, 1000 + i as u32);
            m.process_frame(&p);
        }));
    }
    {
        let mut m = FlashAttention::new();
        results.push(bench_module("sig_flash_attention", "S", |i| {
            let p = synthetic_phases(32, 2000 + i as u32);
            let a = synthetic_amplitudes(32, 2500 + i as u32);
            m.process_frame(&p, &a);
        }));
    }
    {
        let mut m = SparseRecovery::new();
        results.push(bench_module("sig_sparse_recovery", "H", |i| {
            let mut a = synthetic_amplitudes(32, 3000 + i as u32);
            m.process_frame(&mut a);
        }));
    }
    {
        let mut m = TemporalCompressor::new();
        results.push(bench_module("sig_temporal_compress", "S", |i| {
            let p = synthetic_phases(16, 4000 + i as u32);
            let a = synthetic_amplitudes(16, 4500 + i as u32);
            m.push_frame(&p, &a, i as u32 * 50);
        }));
    }
    {
        let mut m = OptimalTransportDetector::new();
        results.push(bench_module("sig_optimal_transport", "S", |i| {
            let a = synthetic_amplitudes(32, 5000 + i as u32);
            m.process_frame(&a);
        }));
    }
    {
        let mut m = PersonMatcher::new();
        results.push(bench_module("sig_mincut_person_match", "H", |i| {
            let a = synthetic_amplitudes(32, 5500 + i as u32);
            let v = synthetic_amplitudes(32, 5600 + i as u32);
            m.process_frame(&a, &v, 3);
        }));
    }

    // --- Adaptive Learning (4 modules) ---
    {
        let mut m = GestureLearner::new();
        results.push(bench_module("lrn_dtw_gesture_learn", "H", |i| {
            let p = synthetic_phases(8, 6000 + i as u32);
            m.process_frame(&p, 0.3);
        }));
    }
    {
        let mut m = AttractorDetector::new();
        results.push(bench_module("lrn_anomaly_attractor", "S", |i| {
            let p = synthetic_phases(8, 7000 + i as u32);
            let a = synthetic_amplitudes(8, 7500 + i as u32);
            m.process_frame(&p, &a, 0.2);
        }));
    }
    {
        let mut m = MetaAdapter::new();
        results.push(bench_module("lrn_meta_adapt", "S", |_i| {
            m.report_true_positive();
            m.on_timer();
        }));
    }
    {
        let mut m = EwcLifelong::new();
        results.push(bench_module("lrn_ewc_lifelong", "L", |i| {
            let features = [0.5, 1.0, 0.3, 0.8, 0.2, 0.6, 0.4, 0.9];
            m.process_frame(&features, (i % 4) as i32);
        }));
    }

    // --- Spatial Reasoning (3 modules) ---
    {
        let mut m = MicroHnsw::new();
        for i in 0..10 {
            let v = synthetic_amplitudes(8, 100 + i);
            m.insert(&v[..8], i as u8);
        }
        results.push(bench_module("spt_micro_hnsw", "S", |i| {
            let q = synthetic_amplitudes(8, 8000 + i as u32);
            m.process_frame(&q[..8]);
        }));
    }
    {
        let mut m = PageRankInfluence::new();
        results.push(bench_module("spt_pagerank_influence", "S", |i| {
            let p = synthetic_phases(32, 9000 + i as u32);
            m.process_frame(&p, 4);
        }));
    }
    {
        let mut m = SpikingTracker::new();
        results.push(bench_module("spt_spiking_tracker", "S", |i| {
            let cur = synthetic_phases(32, 10000 + i as u32);
            let prev = synthetic_phases(32, 10500 + i as u32);
            m.process_frame(&cur, &prev);
        }));
    }

    // --- Temporal Analysis (3 modules) ---
    {
        let mut m = PatternSequenceAnalyzer::new();
        results.push(bench_module("tmp_pattern_sequence", "L", |i| {
            m.on_frame(1, 0.3, (i % 5) as i32);
        }));
    }
    {
        let mut m = TemporalLogicGuard::new();
        results.push(bench_module("tmp_temporal_logic_guard", "L", |_i| {
            let input = FrameInput {
                presence: 1, n_persons: 1, motion_energy: 0.3, coherence: 0.8,
                breathing_bpm: 16.0, heartrate_bpm: 72.0, fall_alert: false,
                intrusion_alert: false, person_id_active: true, vital_signs_active: true,
                seizure_detected: false, normal_gait: true,
            };
            m.on_frame(&input);
        }));
    }
    {
        let mut m = GoapPlanner::new();
        m.update_world(1, 0.5, 2, 0.8, 0.1, true, false);
        results.push(bench_module("tmp_goap_autonomy", "S", |_i| {
            m.on_timer();
        }));
    }

    // --- AI Security (2 modules) ---
    {
        let mut m = PromptShield::new();
        results.push(bench_module("ais_prompt_shield", "S", |i| {
            let p = synthetic_phases(16, 11000 + i as u32);
            let a = synthetic_amplitudes(16, 11500 + i as u32);
            m.process_frame(&p, &a);
        }));
    }
    {
        let mut m = BehavioralProfiler::new();
        results.push(bench_module("ais_behavioral_profiler", "S", |i| {
            m.process_frame(i % 3 == 0, 0.4, (i % 4) as u8);
        }));
    }

    // --- Quantum-Inspired (2 modules) ---
    {
        let mut m = QuantumCoherenceMonitor::new();
        results.push(bench_module("qnt_quantum_coherence", "H", |i| {
            let p = synthetic_phases(16, 12000 + i as u32);
            m.process_frame(&p);
        }));
    }
    {
        let mut m = InterferenceSearch::new();
        results.push(bench_module("qnt_interference_search", "H", |i| {
            m.process_frame((i % 2) as i32, 0.3, (i % 4) as i32);
        }));
    }

    // --- Autonomous Systems (2 modules) ---
    {
        let mut m = PsychoSymbolicEngine::new();
        results.push(bench_module("aut_psycho_symbolic", "H", |i| {
            m.process_frame(1.0, 0.3 + (i as f32 * 0.01), 15.0, 72.0, 1.0, (i % 4) as f32);
        }));
    }
    {
        let mut m = SelfHealingMesh::new();
        results.push(bench_module("aut_self_healing_mesh", "S", |i| {
            let qualities = [0.8 + (i as f32 * 0.001), 0.9, 0.85, 0.7];
            m.process_frame(&qualities);
        }));
    }

    // --- Exotic / Research (2 modules) ---
    {
        let mut m = TimeCrystalDetector::new();
        results.push(bench_module("exo_time_crystal", "H", |i| {
            let me = 0.5 + 0.3 * libm::sinf(i as f32 * 0.1);
            m.process_frame(me);
        }));
    }
    {
        let mut m = HyperbolicEmbedder::new();
        results.push(bench_module("exo_hyperbolic_space", "S", |i| {
            let a = synthetic_amplitudes(32, 14000 + i as u32);
            m.process_frame(&a);
        }));
    }

    // Print results table.
    print_bench_table(&results);

    // Summary stats.
    let total_us: f64 = results.iter().map(|r| r.mean_us).sum();
    let slowest = results.iter().max_by(|a, b| a.p99_us.partial_cmp(&b.p99_us).unwrap()).unwrap();
    let fastest = results.iter().min_by(|a, b| a.p99_us.partial_cmp(&b.p99_us).unwrap()).unwrap();
    let all_pass = results.iter().all(|r| {
        let budget = match r.tier { "L" => 2000.0, "S" => 5000.0, _ => 10000.0 };
        r.p99_us < budget
    });

    eprintln!("  Aggregate per-frame (all 24 modules): {:.1}us mean", total_us);
    eprintln!("  Fastest: {} at {:.1}us p99", fastest.name, fastest.p99_us);
    eprintln!("  Slowest: {} at {:.1}us p99", slowest.name, slowest.p99_us);
    eprintln!("  All within budget: {}", if all_pass { "YES" } else { "NO" });
    eprintln!();

    assert!(all_pass, "One or more modules exceeded their budget tier");
}
