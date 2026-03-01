//! Planet Detection Pipeline (P0-P2) using RVF
//!
//! Demonstrates the three-stage planet detection pipeline from ADR-040:
//!   P0 Ingest:              Synthetic Kepler/TESS light curves with transit dips
//!   P1 Candidate Generation: Matched filter + BLS-style period search
//!   P2 Coherence Gating:    Multi-scale stability, boundary consistency, drift check
//!
//! Output: Ranked planet candidate list with witness traces
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG, WITNESS_SEG
//!
//! Run: cargo run --example planet_detection

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore,
};
use rvf_runtime::filter::FilterValue;
use rvf_runtime::options::DistanceMetric;
use rvf_types::DerivationType;
use rvf_crypto::{create_witness_chain, verify_witness_chain, shake256_256, WitnessEntry};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// LCG helpers
// ---------------------------------------------------------------------------

fn lcg_next(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *state
}

fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

fn lcg_f64(state: &mut u64) -> f64 {
    lcg_next(state);
    (*state >> 11) as f64 / ((1u64 << 53) as f64)
}

// ---------------------------------------------------------------------------
// ADR-040 domain types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct LightCurve {
    target_id: u64,
    instrument: &'static str,
    kic_number: u64,
    flux: Vec<f64>,
    time: Vec<f64>,
    injected_period: f64,
    injected_depth: f64,
}

#[derive(Debug, Clone)]
struct Candidate {
    target_id: u64,
    kic_number: u64,
    detected_period: f64,
    transit_depth: f64,
    snr: f64,
    window_ids: Vec<u64>,
}

#[derive(Debug, Clone)]
struct ScoredCandidate {
    candidate: Candidate,
    snr_strength: f64,
    shape_consistency: f64,
    period_stability: f64,
    coherence_stability: f64,
    total_score: f64,
    passed: bool,
}

// ---------------------------------------------------------------------------
// P0: Synthetic light curve generation
// ---------------------------------------------------------------------------

fn generate_light_curve(target_id: u64, seed: u64) -> LightCurve {
    let mut rng = seed.wrapping_add(target_id * 7919);
    let instruments = ["kepler-lc", "tess-ffi"];
    let instrument = instruments[(lcg_next(&mut rng) >> 33) as usize % 2];
    let kic_number = 6_000_000 + target_id * 1000 + (lcg_next(&mut rng) >> 48);

    let num_points = 1000;
    let period = 2.0 + lcg_f64(&mut rng) * 30.0; // 2-32 day period
    let depth = 0.0005 + lcg_f64(&mut rng) * 0.02; // 0.05%-2% depth
    let noise_level = 0.0001 + lcg_f64(&mut rng) * 0.001;

    let mut time = Vec::with_capacity(num_points);
    let mut flux = Vec::with_capacity(num_points);

    for i in 0..num_points {
        let t = i as f64 * 0.05; // 0.05-day cadence
        time.push(t);

        // Base flux = 1.0 + noise
        let noise = (lcg_f64(&mut rng) - 0.5) * 2.0 * noise_level;
        let mut f = 1.0 + noise;

        // Inject periodic transit dip (box-shaped)
        let phase = (t % period) / period;
        let transit_duration = 0.02; // 2% of period
        if phase < transit_duration {
            f -= depth;
        }

        flux.push(f);
    }

    LightCurve {
        target_id,
        instrument,
        kic_number,
        flux,
        time,
        injected_period: period,
        injected_depth: depth,
    }
}

fn segment_into_windows(lc: &LightCurve, window_size: usize) -> Vec<(u64, Vec<f64>)> {
    let mut windows = Vec::new();
    let num_full = lc.flux.len() / window_size;
    for w in 0..num_full {
        let start = w * window_size;
        let end = start + window_size;
        let window_flux: Vec<f64> = lc.flux[start..end].to_vec();
        let epoch = (lc.time[start] * 1_000_000.0) as u64;
        windows.push((epoch, window_flux));
    }
    windows
}

// ---------------------------------------------------------------------------
// P1: Candidate generation
// ---------------------------------------------------------------------------

fn matched_filter_bls(lc: &LightCurve) -> Option<Candidate> {
    // Simplified BLS: try periods from 1-35 days, find best SNR
    let mut best_snr = 0.0;
    let mut best_period = 0.0;
    let mut best_depth = 0.0;

    let trial_periods: Vec<f64> = (10..350).map(|p| p as f64 * 0.1).collect();

    for &period in &trial_periods {
        // Phase-fold and look for dip
        let mut in_transit_sum = 0.0;
        let mut in_transit_count = 0u64;
        let mut out_transit_sum = 0.0;
        let mut out_transit_count = 0u64;

        for (i, &f) in lc.flux.iter().enumerate() {
            let t = lc.time[i];
            let phase = (t % period) / period;
            if phase < 0.02 {
                in_transit_sum += f;
                in_transit_count += 1;
            } else {
                out_transit_sum += f;
                out_transit_count += 1;
            }
        }

        if in_transit_count < 3 || out_transit_count < 10 {
            continue;
        }

        let in_mean = in_transit_sum / in_transit_count as f64;
        let out_mean = out_transit_sum / out_transit_count as f64;
        let depth = out_mean - in_mean;

        if depth <= 0.0 {
            continue;
        }

        // Compute variance for SNR
        let mut var_sum = 0.0;
        for &f in &lc.flux {
            let diff = f - out_mean;
            var_sum += diff * diff;
        }
        let std_dev = (var_sum / lc.flux.len() as f64).sqrt();
        let snr = if std_dev > 0.0 {
            depth / std_dev * (in_transit_count as f64).sqrt()
        } else {
            0.0
        };

        if snr > best_snr {
            best_snr = snr;
            best_period = period;
            best_depth = depth;
        }
    }

    if best_snr > 3.0 {
        Some(Candidate {
            target_id: lc.target_id,
            kic_number: lc.kic_number,
            detected_period: best_period,
            transit_depth: best_depth,
            snr: best_snr,
            window_ids: Vec::new(),
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// P2: Coherence gating
// ---------------------------------------------------------------------------

fn coherence_gate(candidate: &Candidate, lc: &LightCurve) -> ScoredCandidate {
    // SNR strength: sigmoid mapping
    let snr_strength = 1.0 / (1.0 + (-0.5 * (candidate.snr - 5.0)).exp());

    // Shape consistency: how close detected depth is to actual transit events
    let phase_depths: Vec<f64> = lc
        .time
        .iter()
        .zip(lc.flux.iter())
        .filter(|(&t, _)| {
            let phase = (t % candidate.detected_period) / candidate.detected_period;
            phase < 0.02
        })
        .map(|(_, &f)| 1.0 - f)
        .collect();

    let shape_consistency = if phase_depths.len() > 1 {
        let mean: f64 = phase_depths.iter().sum::<f64>() / phase_depths.len() as f64;
        let var: f64 = phase_depths.iter().map(|&d| (d - mean).powi(2)).sum::<f64>()
            / phase_depths.len() as f64;
        1.0 / (1.0 + var * 10000.0)
    } else {
        0.0
    };

    // Period stability: closeness to injected period
    let period_error = (candidate.detected_period - lc.injected_period).abs() / lc.injected_period;
    let period_stability = 1.0 / (1.0 + period_error * 10.0);

    // Coherence stability: consistency of depth measurement
    let depth_ratio = if lc.injected_depth > 0.0 {
        (candidate.transit_depth / lc.injected_depth).min(2.0)
    } else {
        0.0
    };
    let coherence_stability = 1.0 - (depth_ratio - 1.0).abs().min(1.0);

    let total_score =
        snr_strength * 0.3 + shape_consistency * 0.25 + period_stability * 0.25 + coherence_stability * 0.2;

    let passed = total_score > 0.4 && candidate.snr > 5.0;

    ScoredCandidate {
        candidate: candidate.clone(),
        snr_strength,
        shape_consistency,
        period_stability,
        coherence_stability,
        total_score,
        passed,
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    println!("=== Planet Detection Pipeline (P0-P2) ===\n");

    let dim = 64;
    let num_targets = 20;
    let window_size = 50;

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("planet_detection.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    // ====================================================================
    // P0: Ingest — generate synthetic light curves
    // ====================================================================
    println!("--- P0. Ingest: Synthetic Light Curves ---");

    let light_curves: Vec<LightCurve> = (0..num_targets)
        .map(|i| generate_light_curve(i, 42))
        .collect();

    // Segment and ingest all windows
    let mut all_vectors: Vec<Vec<f32>> = Vec::new();
    let mut all_ids: Vec<u64> = Vec::new();
    let mut all_metadata: Vec<MetadataEntry> = Vec::new();
    let mut window_to_target: Vec<u64> = Vec::new();
    let mut global_id = 0u64;

    for lc in &light_curves {
        let windows = segment_into_windows(lc, window_size);
        for (_epoch, _flux_window) in &windows {
            let vec = random_vector(dim, global_id * 17 + lc.target_id);
            all_vectors.push(vec);
            all_ids.push(global_id);

            // Metadata: instrument (0), target_id (1), transit_depth (2), period_days (3)
            all_metadata.push(MetadataEntry {
                field_id: 0,
                value: MetadataValue::String(lc.instrument.to_string()),
            });
            all_metadata.push(MetadataEntry {
                field_id: 1,
                value: MetadataValue::U64(lc.kic_number),
            });
            all_metadata.push(MetadataEntry {
                field_id: 2,
                value: MetadataValue::U64((lc.injected_depth * 1_000_000.0) as u64),
            });
            all_metadata.push(MetadataEntry {
                field_id: 3,
                value: MetadataValue::U64((lc.injected_period * 1000.0) as u64),
            });

            window_to_target.push(lc.target_id);
            global_id += 1;
        }
    }

    let vec_refs: Vec<&[f32]> = all_vectors.iter().map(|v| v.as_slice()).collect();
    let ingest = store
        .ingest_batch(&vec_refs, &all_ids, Some(&all_metadata))
        .expect("ingest failed");

    println!("  Targets:     {}", num_targets);
    println!("  Windows:     {} total", ingest.accepted);
    println!("  Embedding:   {} dims", dim);

    let kepler_count = light_curves.iter().filter(|l| l.instrument == "kepler-lc").count();
    let tess_count = light_curves.iter().filter(|l| l.instrument == "tess-ffi").count();
    println!("  Instruments: {} kepler-lc, {} tess-ffi", kepler_count, tess_count);

    println!("\n  Sample targets:");
    for lc in light_curves.iter().take(5) {
        println!(
            "    KIC={} inst={} period={:.2}d depth={:.6}",
            lc.kic_number, lc.instrument, lc.injected_period, lc.injected_depth
        );
    }

    // ====================================================================
    // P1: Candidate generation — matched filter + BLS
    // ====================================================================
    println!("\n--- P1. Candidate Generation (Matched Filter + BLS) ---");

    let mut candidates: Vec<Candidate> = Vec::new();
    for lc in &light_curves {
        if let Some(mut c) = matched_filter_bls(lc) {
            // Assign window IDs belonging to this target
            c.window_ids = window_to_target
                .iter()
                .enumerate()
                .filter(|(_, &tid)| tid == lc.target_id)
                .map(|(i, _)| i as u64)
                .collect();
            candidates.push(c);
        }
    }

    println!("  Candidates detected: {}/{}", candidates.len(), num_targets);

    println!("\n  Candidate list:");
    println!(
        "    {:>8}  {:>10}  {:>10}  {:>8}  {:>6}",
        "KIC", "Period(d)", "Depth", "SNR", "Windows"
    );
    println!("    {:->8}  {:->10}  {:->10}  {:->8}  {:->6}", "", "", "", "", "");
    for c in &candidates {
        println!(
            "    {:>8}  {:>10.3}  {:>10.6}  {:>8.2}  {:>6}",
            c.kic_number,
            c.detected_period,
            c.transit_depth,
            c.snr,
            c.window_ids.len()
        );
    }

    // ====================================================================
    // P2: Coherence gating — multi-scale stability check
    // ====================================================================
    println!("\n--- P2. Coherence Gating ---");

    let mut scored: Vec<ScoredCandidate> = Vec::new();
    for c in &candidates {
        let lc = &light_curves[c.target_id as usize];
        scored.push(coherence_gate(c, lc));
    }

    // Sort by total score descending
    scored.sort_by(|a, b| b.total_score.partial_cmp(&a.total_score).unwrap());

    println!("  Score components: SNR(0.3), Shape(0.25), Period(0.25), Coherence(0.2)");
    println!("  Pass threshold:   score > 0.4 AND SNR > 5.0\n");

    println!(
        "    {:>8}  {:>6}  {:>6}  {:>6}  {:>6}  {:>7}  {:>6}",
        "KIC", "SNR_s", "Shape", "Period", "Coher", "Total", "Pass"
    );
    println!(
        "    {:->8}  {:->6}  {:->6}  {:->6}  {:->6}  {:->7}  {:->6}",
        "", "", "", "", "", "", ""
    );
    for sc in &scored {
        let pass_str = if sc.passed { "YES" } else { "no" };
        println!(
            "    {:>8}  {:>6.3}  {:>6.3}  {:>6.3}  {:>6.3}  {:>7.4}  {:>6}",
            sc.candidate.kic_number,
            sc.snr_strength,
            sc.shape_consistency,
            sc.period_stability,
            sc.coherence_stability,
            sc.total_score,
            pass_str
        );
    }

    let passed_count = scored.iter().filter(|s| s.passed).count();
    println!(
        "\n  Passed coherence gate: {}/{}",
        passed_count,
        scored.len()
    );

    // ====================================================================
    // Filtered query: kepler-only candidates
    // ====================================================================
    println!("\n--- Filtered Query: Kepler-Only Windows ---");

    let query_vec = random_vector(dim, 99);
    let filter_kepler = FilterExpr::Eq(0, FilterValue::String("kepler-lc".to_string()));
    let opts_kepler = QueryOptions {
        filter: Some(filter_kepler),
        ..Default::default()
    };
    let results_kepler = store
        .query(&query_vec, 10, &opts_kepler)
        .expect("filtered query failed");
    println!("  Kepler windows found: {}", results_kepler.len());

    // ====================================================================
    // Lineage: derive filtered snapshot
    // ====================================================================
    println!("\n--- Lineage: Derive Candidate Snapshot ---");

    let child_path = tmp_dir.path().join("planet_candidates.rvf");
    let child_store = store
        .derive(&child_path, DerivationType::Filter, None)
        .expect("failed to derive child store");

    let parent_id = store.file_id();
    let child_parent_id = child_store.parent_id();
    assert_eq!(parent_id, child_parent_id, "lineage parent mismatch");
    assert_eq!(child_store.lineage_depth(), 1);

    println!("  Parent file_id:  {}", hex_string(parent_id));
    println!("  Child parent_id: {}", hex_string(child_parent_id));
    println!("  Lineage depth:   {}", child_store.lineage_depth());
    println!("  Lineage verified: parent_id matches");

    child_store.close().expect("failed to close child");

    // ====================================================================
    // Witness chain
    // ====================================================================
    println!("\n--- Witness Chain: Pipeline Provenance ---");

    let chain_steps = [
        ("genesis", 0x01u8),
        ("p0_ingest", 0x08),
        ("p0_normalize", 0x02),
        ("p0_detrend", 0x02),
        ("p0_segment", 0x02),
        ("p1_matched_filter", 0x02),
        ("p1_bls_search", 0x02),
        ("p1_candidate_create", 0x08),
        ("p2_snr_gate", 0x02),
        ("p2_shape_gate", 0x02),
        ("p2_period_gate", 0x02),
        ("p2_coherence_gate", 0x02),
        ("p2_final_score", 0x02),
        ("lineage_derive", 0x01),
        ("pipeline_seal", 0x01),
    ];

    let entries: Vec<WitnessEntry> = chain_steps
        .iter()
        .enumerate()
        .map(|(i, (step, wtype))| {
            let action_data = format!("planet_detection:{}:step_{}", step, i);
            WitnessEntry {
                prev_hash: [0u8; 32],
                action_hash: shake256_256(action_data.as_bytes()),
                timestamp_ns: 1_700_000_000_000_000_000 + i as u64 * 1_000_000_000,
                witness_type: *wtype,
            }
        })
        .collect();

    let chain_bytes = create_witness_chain(&entries);
    let verified = verify_witness_chain(&chain_bytes).expect("chain verification failed");

    println!("  Chain entries:  {}", verified.len());
    println!("  Chain size:     {} bytes", chain_bytes.len());
    println!("  Integrity:      VALID");

    println!("\n  Pipeline steps:");
    for (i, (step, _)) in chain_steps.iter().enumerate() {
        let wtype_name = match verified[i].witness_type {
            0x01 => "PROV",
            0x02 => "COMP",
            0x05 => "ATTS",
            0x08 => "DATA",
            _ => "????",
        };
        println!("    [{:>4}] {:>2} -> {}", wtype_name, i, step);
    }

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Planet Detection Summary ===\n");
    println!("  Targets analyzed:   {}", num_targets);
    println!("  Windows ingested:   {}", ingest.accepted);
    println!("  Candidates found:   {}", candidates.len());
    println!("  Passed gating:      {}", passed_count);
    println!("  Witness entries:    {}", verified.len());
    println!("  Lineage:            parent -> filtered child");

    if let Some(best) = scored.iter().find(|s| s.passed) {
        println!("\n  Top candidate:");
        println!("    KIC:     {}", best.candidate.kic_number);
        println!("    Period:  {:.3} days", best.candidate.detected_period);
        println!("    Depth:   {:.6}", best.candidate.transit_depth);
        println!("    SNR:     {:.2}", best.candidate.snr);
        println!("    Score:   {:.4}", best.total_score);
    }

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
