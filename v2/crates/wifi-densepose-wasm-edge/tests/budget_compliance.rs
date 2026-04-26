//! Budget compliance tests for all 24 WASM edge vendor modules (ADR-041).
//!
//! Validates per-frame processing time against budget tiers:
//!   L (Lightweight) < 2ms, S (Standard) < 5ms, H (Heavy) < 10ms
//!
//! Run with:
//!   cargo test -p wifi-densepose-wasm-edge --features std --test budget_compliance -- --nocapture

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

const N_ITER: usize = 100;

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

struct BudgetResult {
    module: &'static str,
    tier: &'static str,
    budget_ms: f64,
    mean_us: f64,
    p99_us: f64,
    max_us: f64,
    pass: bool,
}

fn measure_and_check(
    module: &'static str,
    tier: &'static str,
    budget_ms: f64,
    mut body: impl FnMut(usize),
) -> BudgetResult {
    // Warm up.
    for i in 0..10 {
        body(i);
    }

    let mut durations = Vec::with_capacity(N_ITER);
    for i in 0..N_ITER {
        let t0 = Instant::now();
        body(10 + i);
        durations.push(t0.elapsed().as_nanos() as f64 / 1000.0); // microseconds
    }

    durations.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean_us = durations.iter().sum::<f64>() / durations.len() as f64;
    let p99_idx = (durations.len() as f64 * 0.99) as usize;
    let p99_us = durations[p99_idx.min(durations.len() - 1)];
    let max_us = durations[durations.len() - 1];
    let pass = p99_us / 1000.0 < budget_ms;

    BudgetResult { module, tier, budget_ms, mean_us, p99_us, max_us, pass }
}

fn print_result(r: &BudgetResult) {
    let status = if r.pass { "PASS" } else { "FAIL" };
    eprintln!(
        "  [{status}] {mod:36} tier={tier} budget={b:>5.1}ms  mean={mean:>8.1}us  p99={p99:>8.1}us  max={max:>8.1}us",
        status = status,
        mod = r.module,
        tier = r.tier,
        b = r.budget_ms,
        mean = r.mean_us,
        p99 = r.p99_us,
        max = r.max_us,
    );
}

// ==========================================================================
// Signal Intelligence Tests
// ==========================================================================

#[test]
fn budget_sig_coherence_gate() {
    let mut m = CoherenceGate::new();
    let r = measure_and_check("sig_coherence_gate", "L", 2.0, |i| {
        let p = synthetic_phases(32, 1000 + i as u32);
        m.process_frame(&p);
    });
    print_result(&r);
    assert!(r.pass, "sig_coherence_gate p99={:.1}us exceeds L budget 2ms", r.p99_us);
}

#[test]
fn budget_sig_flash_attention() {
    let mut m = FlashAttention::new();
    let r = measure_and_check("sig_flash_attention", "S", 5.0, |i| {
        let p = synthetic_phases(32, 2000 + i as u32);
        let a = synthetic_amplitudes(32, 2500 + i as u32);
        m.process_frame(&p, &a);
    });
    print_result(&r);
    assert!(r.pass, "sig_flash_attention p99={:.1}us exceeds S budget 5ms", r.p99_us);
}

#[test]
fn budget_sig_sparse_recovery() {
    let mut m = SparseRecovery::new();
    let r = measure_and_check("sig_sparse_recovery", "H", 10.0, |i| {
        let mut a = synthetic_amplitudes(32, 3000 + i as u32);
        m.process_frame(&mut a);
    });
    print_result(&r);
    assert!(r.pass, "sig_sparse_recovery p99={:.1}us exceeds H budget 10ms", r.p99_us);
}

#[test]
fn budget_sig_temporal_compress() {
    let mut m = TemporalCompressor::new();
    let r = measure_and_check("sig_temporal_compress", "S", 5.0, |i| {
        let p = synthetic_phases(16, 4000 + i as u32);
        let a = synthetic_amplitudes(16, 4500 + i as u32);
        m.push_frame(&p, &a, i as u32 * 50);
    });
    print_result(&r);
    assert!(r.pass, "sig_temporal_compress p99={:.1}us exceeds S budget 5ms", r.p99_us);
}

#[test]
fn budget_sig_optimal_transport() {
    let mut m = OptimalTransportDetector::new();
    let r = measure_and_check("sig_optimal_transport", "S", 5.0, |i| {
        let a = synthetic_amplitudes(32, 5000 + i as u32);
        m.process_frame(&a);
    });
    print_result(&r);
    assert!(r.pass, "sig_optimal_transport p99={:.1}us exceeds S budget 5ms", r.p99_us);
}

#[test]
fn budget_sig_mincut_person_match() {
    let mut m = PersonMatcher::new();
    let r = measure_and_check("sig_mincut_person_match", "H", 10.0, |i| {
        let a = synthetic_amplitudes(32, 5500 + i as u32);
        let v = synthetic_amplitudes(32, 5600 + i as u32);
        m.process_frame(&a, &v, 3);
    });
    print_result(&r);
    assert!(r.pass, "sig_mincut_person_match p99={:.1}us exceeds H budget 10ms", r.p99_us);
}

// ==========================================================================
// Adaptive Learning Tests
// ==========================================================================

#[test]
fn budget_lrn_dtw_gesture_learn() {
    let mut m = GestureLearner::new();
    let r = measure_and_check("lrn_dtw_gesture_learn", "H", 10.0, |i| {
        let p = synthetic_phases(8, 6000 + i as u32);
        m.process_frame(&p, 0.3 + (i as f32 * 0.01));
    });
    print_result(&r);
    assert!(r.pass, "lrn_dtw_gesture_learn p99={:.1}us exceeds H budget 10ms", r.p99_us);
}

#[test]
fn budget_lrn_anomaly_attractor() {
    let mut m = AttractorDetector::new();
    let r = measure_and_check("lrn_anomaly_attractor", "S", 5.0, |i| {
        let p = synthetic_phases(8, 7000 + i as u32);
        let a = synthetic_amplitudes(8, 7500 + i as u32);
        m.process_frame(&p, &a, 0.2);
    });
    print_result(&r);
    assert!(r.pass, "lrn_anomaly_attractor p99={:.1}us exceeds S budget 5ms", r.p99_us);
}

#[test]
fn budget_lrn_meta_adapt() {
    let mut m = MetaAdapter::new();
    let r = measure_and_check("lrn_meta_adapt", "S", 5.0, |_i| {
        m.report_true_positive();
        m.on_timer();
    });
    print_result(&r);
    assert!(r.pass, "lrn_meta_adapt p99={:.1}us exceeds S budget 5ms", r.p99_us);
}

#[test]
fn budget_lrn_ewc_lifelong() {
    let mut m = EwcLifelong::new();
    let r = measure_and_check("lrn_ewc_lifelong", "L", 2.0, |i| {
        let features = [0.5, 1.0, 0.3, 0.8, 0.2, 0.6, 0.4, 0.9];
        m.process_frame(&features, (i % 4) as i32);
    });
    print_result(&r);
    assert!(r.pass, "lrn_ewc_lifelong p99={:.1}us exceeds L budget 2ms", r.p99_us);
}

// ==========================================================================
// Spatial Reasoning Tests
// ==========================================================================

#[test]
fn budget_spt_micro_hnsw() {
    let mut m = MicroHnsw::new();
    // Pre-populate with some vectors.
    for i in 0..10 {
        let v = synthetic_amplitudes(8, 100 + i);
        m.insert(&v[..8], i as u8);
    }
    let r = measure_and_check("spt_micro_hnsw", "S", 5.0, |i| {
        let q = synthetic_amplitudes(8, 8000 + i as u32);
        m.process_frame(&q[..8]);
    });
    print_result(&r);
    assert!(r.pass, "spt_micro_hnsw p99={:.1}us exceeds S budget 5ms", r.p99_us);
}

#[test]
fn budget_spt_pagerank_influence() {
    let mut m = PageRankInfluence::new();
    let r = measure_and_check("spt_pagerank_influence", "S", 5.0, |i| {
        let p = synthetic_phases(32, 9000 + i as u32);
        m.process_frame(&p, 4);
    });
    print_result(&r);
    assert!(r.pass, "spt_pagerank_influence p99={:.1}us exceeds S budget 5ms", r.p99_us);
}

#[test]
fn budget_spt_spiking_tracker() {
    let mut m = SpikingTracker::new();
    let r = measure_and_check("spt_spiking_tracker", "S", 5.0, |i| {
        let cur = synthetic_phases(32, 10000 + i as u32);
        let prev = synthetic_phases(32, 10500 + i as u32);
        m.process_frame(&cur, &prev);
    });
    print_result(&r);
    assert!(r.pass, "spt_spiking_tracker p99={:.1}us exceeds S budget 5ms", r.p99_us);
}

// ==========================================================================
// Temporal Analysis Tests
// ==========================================================================

#[test]
fn budget_tmp_pattern_sequence() {
    let mut m = PatternSequenceAnalyzer::new();
    let r = measure_and_check("tmp_pattern_sequence", "L", 2.0, |i| {
        m.on_frame(1, 0.3 + (i as f32 * 0.01), (i % 5) as i32);
    });
    print_result(&r);
    assert!(r.pass, "tmp_pattern_sequence p99={:.1}us exceeds L budget 2ms", r.p99_us);
}

#[test]
fn budget_tmp_temporal_logic_guard() {
    let mut m = TemporalLogicGuard::new();
    let r = measure_and_check("tmp_temporal_logic_guard", "L", 2.0, |_i| {
        let input = FrameInput {
            presence: 1,
            n_persons: 1,
            motion_energy: 0.3,
            coherence: 0.8,
            breathing_bpm: 16.0,
            heartrate_bpm: 72.0,
            fall_alert: false,
            intrusion_alert: false,
            person_id_active: true,
            vital_signs_active: true,
            seizure_detected: false,
            normal_gait: true,
        };
        m.on_frame(&input);
    });
    print_result(&r);
    assert!(r.pass, "tmp_temporal_logic_guard p99={:.1}us exceeds L budget 2ms", r.p99_us);
}

#[test]
fn budget_tmp_goap_autonomy() {
    let mut m = GoapPlanner::new();
    m.update_world(1, 0.5, 2, 0.8, 0.1, true, false);
    let r = measure_and_check("tmp_goap_autonomy", "S", 5.0, |_i| {
        m.on_timer();
    });
    print_result(&r);
    assert!(r.pass, "tmp_goap_autonomy p99={:.1}us exceeds S budget 5ms", r.p99_us);
}

// ==========================================================================
// AI Security Tests
// ==========================================================================

#[test]
fn budget_ais_prompt_shield() {
    let mut m = PromptShield::new();
    let r = measure_and_check("ais_prompt_shield", "S", 5.0, |i| {
        let p = synthetic_phases(16, 11000 + i as u32);
        let a = synthetic_amplitudes(16, 11500 + i as u32);
        m.process_frame(&p, &a);
    });
    print_result(&r);
    assert!(r.pass, "ais_prompt_shield p99={:.1}us exceeds S budget 5ms", r.p99_us);
}

#[test]
fn budget_ais_behavioral_profiler() {
    let mut m = BehavioralProfiler::new();
    let r = measure_and_check("ais_behavioral_profiler", "S", 5.0, |i| {
        m.process_frame(i % 3 == 0, 0.4 + (i as f32 * 0.01), (i % 4) as u8);
    });
    print_result(&r);
    assert!(r.pass, "ais_behavioral_profiler p99={:.1}us exceeds S budget 5ms", r.p99_us);
}

// ==========================================================================
// Quantum-Inspired Tests
// ==========================================================================

#[test]
fn budget_qnt_quantum_coherence() {
    let mut m = QuantumCoherenceMonitor::new();
    let r = measure_and_check("qnt_quantum_coherence", "H", 10.0, |i| {
        let p = synthetic_phases(16, 12000 + i as u32);
        m.process_frame(&p);
    });
    print_result(&r);
    assert!(r.pass, "qnt_quantum_coherence p99={:.1}us exceeds H budget 10ms", r.p99_us);
}

#[test]
fn budget_qnt_interference_search() {
    let mut m = InterferenceSearch::new();
    let r = measure_and_check("qnt_interference_search", "H", 10.0, |i| {
        m.process_frame((i % 2) as i32, 0.3 + (i as f32 * 0.01), (i % 4) as i32);
    });
    print_result(&r);
    assert!(r.pass, "qnt_interference_search p99={:.1}us exceeds H budget 10ms", r.p99_us);
}

// ==========================================================================
// Autonomous Systems Tests
// ==========================================================================

#[test]
fn budget_aut_psycho_symbolic() {
    let mut m = PsychoSymbolicEngine::new();
    let r = measure_and_check("aut_psycho_symbolic", "H", 10.0, |i| {
        m.process_frame(
            1.0,                        // presence
            0.3 + (i as f32 * 0.01),   // motion
            15.0,                       // breathing
            72.0,                       // heartrate
            1.0,                        // n_persons
            (i % 4) as f32,            // time_bucket
        );
    });
    print_result(&r);
    assert!(r.pass, "aut_psycho_symbolic p99={:.1}us exceeds H budget 10ms", r.p99_us);
}

#[test]
fn budget_aut_self_healing_mesh() {
    let mut m = SelfHealingMesh::new();
    let r = measure_and_check("aut_self_healing_mesh", "S", 5.0, |i| {
        let q0 = 0.8 + (i as f32 * 0.001);
        let qualities = [q0, 0.9, 0.85, 0.7];
        m.process_frame(&qualities);
    });
    print_result(&r);
    assert!(r.pass, "aut_self_healing_mesh p99={:.1}us exceeds S budget 5ms", r.p99_us);
}

// ==========================================================================
// Exotic / Research Tests
// ==========================================================================

#[test]
fn budget_exo_time_crystal() {
    let mut m = TimeCrystalDetector::new();
    let r = measure_and_check("exo_time_crystal", "H", 10.0, |i| {
        let me = 0.5 + 0.3 * libm::sinf(i as f32 * 0.1);
        m.process_frame(me);
    });
    print_result(&r);
    assert!(r.pass, "exo_time_crystal p99={:.1}us exceeds H budget 10ms", r.p99_us);
}

#[test]
fn budget_exo_hyperbolic_space() {
    let mut m = HyperbolicEmbedder::new();
    let r = measure_and_check("exo_hyperbolic_space", "S", 5.0, |i| {
        let a = synthetic_amplitudes(32, 14000 + i as u32);
        m.process_frame(&a);
    });
    print_result(&r);
    assert!(r.pass, "exo_hyperbolic_space p99={:.1}us exceeds S budget 5ms", r.p99_us);
}

// ==========================================================================
// Summary Test
// ==========================================================================

#[test]
fn budget_summary_all_24_modules() {
    eprintln!("\n========== BUDGET COMPLIANCE SUMMARY (24 modules) ==========\n");

    let mut results = Vec::new();

    // 1. sig_coherence_gate (L)
    let mut m1 = CoherenceGate::new();
    results.push(measure_and_check("sig_coherence_gate", "L", 2.0, |i| {
        let p = synthetic_phases(32, 1000 + i as u32);
        m1.process_frame(&p);
    }));

    // 2. sig_flash_attention (S)
    let mut m2 = FlashAttention::new();
    results.push(measure_and_check("sig_flash_attention", "S", 5.0, |i| {
        let p = synthetic_phases(32, 2000 + i as u32);
        let a = synthetic_amplitudes(32, 2500 + i as u32);
        m2.process_frame(&p, &a);
    }));

    // 3. sig_sparse_recovery (H)
    let mut m3 = SparseRecovery::new();
    results.push(measure_and_check("sig_sparse_recovery", "H", 10.0, |i| {
        let mut a = synthetic_amplitudes(32, 3000 + i as u32);
        m3.process_frame(&mut a);
    }));

    // 4. sig_temporal_compress (S)
    let mut m4 = TemporalCompressor::new();
    results.push(measure_and_check("sig_temporal_compress", "S", 5.0, |i| {
        let p = synthetic_phases(16, 4000 + i as u32);
        let a = synthetic_amplitudes(16, 4500 + i as u32);
        m4.push_frame(&p, &a, i as u32 * 50);
    }));

    // 5. sig_optimal_transport (S)
    let mut m5 = OptimalTransportDetector::new();
    results.push(measure_and_check("sig_optimal_transport", "S", 5.0, |i| {
        let a = synthetic_amplitudes(32, 5000 + i as u32);
        m5.process_frame(&a);
    }));

    // 6. sig_mincut_person_match (H)
    let mut m6 = PersonMatcher::new();
    results.push(measure_and_check("sig_mincut_person_match", "H", 10.0, |i| {
        let a = synthetic_amplitudes(32, 5500 + i as u32);
        let v = synthetic_amplitudes(32, 5600 + i as u32);
        m6.process_frame(&a, &v, 3);
    }));

    // 7. lrn_dtw_gesture_learn (H)
    let mut m7 = GestureLearner::new();
    results.push(measure_and_check("lrn_dtw_gesture_learn", "H", 10.0, |i| {
        let p = synthetic_phases(8, 6000 + i as u32);
        m7.process_frame(&p, 0.3);
    }));

    // 8. lrn_anomaly_attractor (S)
    let mut m8 = AttractorDetector::new();
    results.push(measure_and_check("lrn_anomaly_attractor", "S", 5.0, |i| {
        let p = synthetic_phases(8, 7000 + i as u32);
        let a = synthetic_amplitudes(8, 7500 + i as u32);
        m8.process_frame(&p, &a, 0.2);
    }));

    // 9. lrn_meta_adapt (S)
    let mut m9 = MetaAdapter::new();
    results.push(measure_and_check("lrn_meta_adapt", "S", 5.0, |_i| {
        m9.report_true_positive();
        m9.on_timer();
    }));

    // 10. lrn_ewc_lifelong (L)
    let mut m10 = EwcLifelong::new();
    results.push(measure_and_check("lrn_ewc_lifelong", "L", 2.0, |i| {
        let features = [0.5, 1.0, 0.3, 0.8, 0.2, 0.6, 0.4, 0.9];
        m10.process_frame(&features, (i % 4) as i32);
    }));

    // 11. spt_micro_hnsw (S)
    let mut m11 = MicroHnsw::new();
    for i in 0..10 {
        let v = synthetic_amplitudes(8, 100 + i);
        m11.insert(&v[..8], i as u8);
    }
    results.push(measure_and_check("spt_micro_hnsw", "S", 5.0, |i| {
        let q = synthetic_amplitudes(8, 8000 + i as u32);
        m11.process_frame(&q[..8]);
    }));

    // 12. spt_pagerank_influence (S)
    let mut m12 = PageRankInfluence::new();
    results.push(measure_and_check("spt_pagerank_influence", "S", 5.0, |i| {
        let p = synthetic_phases(32, 9000 + i as u32);
        m12.process_frame(&p, 4);
    }));

    // 13. spt_spiking_tracker (S)
    let mut m13 = SpikingTracker::new();
    results.push(measure_and_check("spt_spiking_tracker", "S", 5.0, |i| {
        let cur = synthetic_phases(32, 10000 + i as u32);
        let prev = synthetic_phases(32, 10500 + i as u32);
        m13.process_frame(&cur, &prev);
    }));

    // 14. tmp_pattern_sequence (L)
    let mut m14 = PatternSequenceAnalyzer::new();
    results.push(measure_and_check("tmp_pattern_sequence", "L", 2.0, |i| {
        m14.on_frame(1, 0.3, (i % 5) as i32);
    }));

    // 15. tmp_temporal_logic_guard (L)
    let mut m15 = TemporalLogicGuard::new();
    results.push(measure_and_check("tmp_temporal_logic_guard", "L", 2.0, |_i| {
        let input = FrameInput {
            presence: 1, n_persons: 1, motion_energy: 0.3, coherence: 0.8,
            breathing_bpm: 16.0, heartrate_bpm: 72.0, fall_alert: false,
            intrusion_alert: false, person_id_active: true, vital_signs_active: true,
            seizure_detected: false, normal_gait: true,
        };
        m15.on_frame(&input);
    }));

    // 16. tmp_goap_autonomy (S)
    let mut m16 = GoapPlanner::new();
    m16.update_world(1, 0.5, 2, 0.8, 0.1, true, false);
    results.push(measure_and_check("tmp_goap_autonomy", "S", 5.0, |_i| {
        m16.on_timer();
    }));

    // 17. ais_prompt_shield (S)
    let mut m17 = PromptShield::new();
    results.push(measure_and_check("ais_prompt_shield", "S", 5.0, |i| {
        let p = synthetic_phases(16, 11000 + i as u32);
        let a = synthetic_amplitudes(16, 11500 + i as u32);
        m17.process_frame(&p, &a);
    }));

    // 18. ais_behavioral_profiler (S)
    let mut m18 = BehavioralProfiler::new();
    results.push(measure_and_check("ais_behavioral_profiler", "S", 5.0, |i| {
        m18.process_frame(i % 3 == 0, 0.4, (i % 4) as u8);
    }));

    // 19. qnt_quantum_coherence (H)
    let mut m19 = QuantumCoherenceMonitor::new();
    results.push(measure_and_check("qnt_quantum_coherence", "H", 10.0, |i| {
        let p = synthetic_phases(16, 12000 + i as u32);
        m19.process_frame(&p);
    }));

    // 20. qnt_interference_search (H)
    let mut m20 = InterferenceSearch::new();
    results.push(measure_and_check("qnt_interference_search", "H", 10.0, |i| {
        m20.process_frame((i % 2) as i32, 0.3, (i % 4) as i32);
    }));

    // 21. aut_psycho_symbolic (H)
    let mut m21 = PsychoSymbolicEngine::new();
    results.push(measure_and_check("aut_psycho_symbolic", "H", 10.0, |i| {
        m21.process_frame(1.0, 0.3 + (i as f32 * 0.01), 15.0, 72.0, 1.0, (i % 4) as f32);
    }));

    // 22. aut_self_healing_mesh (S)
    let mut m22 = SelfHealingMesh::new();
    results.push(measure_and_check("aut_self_healing_mesh", "S", 5.0, |i| {
        let qualities = [0.8 + (i as f32 * 0.001), 0.9, 0.85, 0.7];
        m22.process_frame(&qualities);
    }));

    // 23. exo_time_crystal (H)
    let mut m23 = TimeCrystalDetector::new();
    results.push(measure_and_check("exo_time_crystal", "H", 10.0, |i| {
        let me = 0.5 + 0.3 * libm::sinf(i as f32 * 0.1);
        m23.process_frame(me);
    }));

    // 24. exo_hyperbolic_space (S)
    let mut m24 = HyperbolicEmbedder::new();
    results.push(measure_and_check("exo_hyperbolic_space", "S", 5.0, |i| {
        let a = synthetic_amplitudes(32, 14000 + i as u32);
        m24.process_frame(&a);
    }));

    // Print all results.
    for r in &results {
        print_result(r);
    }

    let n_pass = results.iter().filter(|r| r.pass).count();
    let n_fail = results.iter().filter(|r| !r.pass).count();
    eprintln!("\n  Total: {}/{} PASS, {} FAIL\n", n_pass, results.len(), n_fail);
    eprintln!("=============================================================\n");

    assert_eq!(n_fail, 0, "{} module(s) exceeded their budget tier", n_fail);
}
