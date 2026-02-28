//! Causal Atlas with Coherence Field and Boundary Tracking
//!
//! Demonstrates ADR-040 constructs 1-4:
//!   - Windowing:          Multi-scale light-curve windows (2h, 12h, 3d, 27d)
//!   - Feature extraction: Flux derivative stats, autocorrelation peaks
//!   - Embedding:          Window embeddings stored via ingest_batch()
//!   - Causal edges:       Interaction graph with causal/periodicity/shape_similarity types
//!   - Coherence field:    Cut pressure and partition entropy over graph subsets
//!   - Boundary tracking:  Boundary evolution with alert emission
//!   - Multi-scale memory: S/M/L tier retention with metadata tags
//!   - Witness chain:      Every construction step recorded
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG, WITNESS_SEG
//!
//! Run: cargo run --example causal_atlas

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore, SearchResult,
};
use rvf_runtime::filter::FilterValue;
use rvf_runtime::options::DistanceMetric;
use rvf_crypto::{create_witness_chain, verify_witness_chain, shake256_256, WitnessEntry};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// LCG pseudo-random helpers
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
#[allow(dead_code)]
struct LightCurveWindow {
    target_id: u64,
    scale: &'static str,
    window_start_epoch: u64,
    flux_mean: f64,
    flux_std: f64,
    derivative_mean: f64,
    autocorr_peak: f64,
}

#[derive(Debug, Clone)]
struct InteractionEdge {
    source: u64,
    target: u64,
    edge_type: &'static str,
    weight: f64,
}

#[derive(Debug)]
struct CoherenceField {
    cut_pressure: f64,
    partition_entropy: f64,
    num_partitions: usize,
}

#[derive(Debug)]
struct Boundary {
    epoch: u64,
    pressure: f64,
    alert: bool,
}

// ---------------------------------------------------------------------------
// Synthetic data generators
// ---------------------------------------------------------------------------

fn generate_light_curve_windows(target_id: u64, seed: u64) -> Vec<LightCurveWindow> {
    let scales: &[(&str, u64)] = &[
        ("2h", 7200),
        ("12h", 43200),
        ("3d", 259200),
        ("27d", 2332800),
    ];
    let mut windows = Vec::new();
    let mut rng = seed.wrapping_add(target_id * 997);

    for &(scale_name, duration) in scales {
        let num_windows = match scale_name {
            "2h" => 8,
            "12h" => 4,
            "3d" => 2,
            _ => 1,
        };
        for w in 0..num_windows {
            let epoch = 1_600_000_000 + w * duration;
            let flux_mean = 1.0 + lcg_f64(&mut rng) * 0.01;
            let flux_std = 0.001 + lcg_f64(&mut rng) * 0.005;
            let derivative_mean = (lcg_f64(&mut rng) - 0.5) * 0.002;
            let autocorr_peak = lcg_f64(&mut rng) * 0.95;

            windows.push(LightCurveWindow {
                target_id,
                scale: scale_name,
                window_start_epoch: epoch,
                flux_mean,
                flux_std,
                derivative_mean,
                autocorr_peak,
            });
        }
    }
    windows
}

fn build_causal_edges(windows: &[LightCurveWindow]) -> Vec<InteractionEdge> {
    let mut edges = Vec::new();
    let mut rng: u64 = 0xCAFE_BABE;
    let edge_types = ["causal", "periodicity", "shape_similarity"];

    for i in 0..windows.len() {
        for j in (i + 1)..windows.len() {
            if windows[i].target_id != windows[j].target_id {
                continue;
            }
            // Connect windows that are temporally adjacent at different scales
            let same_scale = windows[i].scale == windows[j].scale;
            let autocorr_close =
                (windows[i].autocorr_peak - windows[j].autocorr_peak).abs() < 0.3;

            if same_scale || autocorr_close {
                let etype_idx = (lcg_next(&mut rng) >> 33) as usize % edge_types.len();
                let weight = 0.5 + lcg_f64(&mut rng) * 0.5;
                edges.push(InteractionEdge {
                    source: i as u64,
                    target: j as u64,
                    edge_type: edge_types[etype_idx],
                    weight,
                });
            }
        }
    }
    edges
}

fn compute_coherence(edges: &[InteractionEdge], num_nodes: usize) -> CoherenceField {
    // Simulate cut pressure: average edge weight across boundaries
    let total_weight: f64 = edges.iter().map(|e| e.weight).sum();
    let cut_pressure = if edges.is_empty() {
        0.0
    } else {
        total_weight / edges.len() as f64
    };

    // Partition entropy: simple degree-based partitioning
    let mut degrees = vec![0u32; num_nodes];
    for e in edges {
        if (e.source as usize) < num_nodes {
            degrees[e.source as usize] += 1;
        }
        if (e.target as usize) < num_nodes {
            degrees[e.target as usize] += 1;
        }
    }

    let num_partitions = 3; // S/M/L tiers
    let total_degree: f64 = degrees.iter().map(|&d| d as f64).sum();
    let mut entropy = 0.0;
    if total_degree > 0.0 {
        // Compute entropy of degree distribution across partitions
        let partition_size = (num_nodes + num_partitions - 1) / num_partitions;
        for p in 0..num_partitions {
            let start = p * partition_size;
            let end = (start + partition_size).min(num_nodes);
            let part_deg: f64 = degrees[start..end].iter().map(|&d| d as f64).sum();
            let prob = part_deg / total_degree;
            if prob > 0.0 {
                entropy -= prob * prob.ln();
            }
        }
    }

    CoherenceField {
        cut_pressure,
        partition_entropy: entropy,
        num_partitions,
    }
}

fn track_boundaries(edges: &[InteractionEdge], num_epochs: usize) -> Vec<Boundary> {
    let alert_threshold = 0.8;
    let mut boundaries = Vec::new();
    let mut rng: u64 = 0xDEAD;

    for epoch in 0..num_epochs {
        let pressure = 0.5 + lcg_f64(&mut rng) * 0.4
            + if epoch > num_epochs / 2 { 0.15 } else { 0.0 };
        let _ = edges; // edges inform the pressure in a real implementation
        boundaries.push(Boundary {
            epoch: epoch as u64,
            pressure,
            alert: pressure > alert_threshold,
        });
    }
    boundaries
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    println!("=== Causal Atlas with Coherence Field ===\n");

    let dim = 64;
    let num_targets = 5;
    let domains = ["transit", "flare", "rotation", "eclipse", "variability"];
    let scales_list = ["2h", "12h", "3d", "27d"];

    // ====================================================================
    // 1. Generate multi-scale light-curve windows
    // ====================================================================
    println!("--- 1. Multi-Scale Windowing ---");

    let mut all_windows: Vec<LightCurveWindow> = Vec::new();
    for target_id in 0..num_targets as u64 {
        let windows = generate_light_curve_windows(target_id, 42);
        all_windows.extend(windows);
    }

    println!("  Targets:    {}", num_targets);
    println!("  Scales:     {:?}", scales_list);
    println!("  Windows:    {} total", all_windows.len());

    for scale in &scales_list {
        let count = all_windows.iter().filter(|w| w.scale == *scale).count();
        println!("    {}: {} windows", scale, count);
    }

    // ====================================================================
    // 2. Feature extraction and embedding
    // ====================================================================
    println!("\n--- 2. Feature Extraction & Embedding ---");

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("causal_atlas.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    let num_windows = all_windows.len();
    let vectors: Vec<Vec<f32>> = (0..num_windows)
        .map(|i| {
            let w = &all_windows[i];
            // Embed using window features as seed perturbation
            let seed = i as u64 * 31 + w.window_start_epoch;
            random_vector(dim, seed)
        })
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..num_windows as u64).collect();

    // Metadata: domain (0), scale (1), target_id (2), window_start_epoch (3)
    let mut metadata = Vec::with_capacity(num_windows * 4);
    for (i, w) in all_windows.iter().enumerate() {
        let domain_idx = w.target_id as usize % domains.len();
        metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(domains[domain_idx].to_string()),
        });
        metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::String(w.scale.to_string()),
        });
        metadata.push(MetadataEntry {
            field_id: 2,
            value: MetadataValue::U64(w.target_id),
        });
        metadata.push(MetadataEntry {
            field_id: 3,
            value: MetadataValue::U64(w.window_start_epoch),
        });
        let _ = i;
    }

    let ingest = store
        .ingest_batch(&vec_refs, &ids, Some(&metadata))
        .expect("ingest failed");
    println!("  Embeddings: {} ingested ({} dims)", ingest.accepted, dim);
    println!("  Features per window: flux_mean, flux_std, derivative_mean, autocorr_peak");

    println!("\n  Sample windows:");
    for w in all_windows.iter().take(5) {
        println!(
            "    target={} scale={:>3} epoch={} flux={:.6} autocorr={:.4}",
            w.target_id, w.scale, w.window_start_epoch, w.flux_mean, w.autocorr_peak
        );
    }

    // ====================================================================
    // 3. Build causal edge graph
    // ====================================================================
    println!("\n--- 3. Causal Edge Graph ---");

    let edges = build_causal_edges(&all_windows);

    let causal_count = edges.iter().filter(|e| e.edge_type == "causal").count();
    let period_count = edges.iter().filter(|e| e.edge_type == "periodicity").count();
    let shape_count = edges.iter().filter(|e| e.edge_type == "shape_similarity").count();

    println!("  Total edges:        {}", edges.len());
    println!("    causal:           {}", causal_count);
    println!("    periodicity:      {}", period_count);
    println!("    shape_similarity: {}", shape_count);

    println!("\n  Sample edges:");
    for e in edges.iter().take(5) {
        println!(
            "    {} -> {} [{}] weight={:.4}",
            e.source, e.target, e.edge_type, e.weight
        );
    }

    // ====================================================================
    // 4. Coherence field computation
    // ====================================================================
    println!("\n--- 4. Coherence Field ---");

    let coherence = compute_coherence(&edges, num_windows);
    println!("  Cut pressure:       {:.6}", coherence.cut_pressure);
    println!("  Partition entropy:  {:.6}", coherence.partition_entropy);
    println!("  Partitions (S/M/L): {}", coherence.num_partitions);

    // ====================================================================
    // 5. Boundary tracking
    // ====================================================================
    println!("\n--- 5. Boundary Tracking ---");

    let boundaries = track_boundaries(&edges, 10);
    let alert_count = boundaries.iter().filter(|b| b.alert).count();

    println!("  Tracked epochs: {}", boundaries.len());
    println!("  Alerts fired:   {}", alert_count);

    println!("\n  Boundary evolution:");
    for b in &boundaries {
        let marker = if b.alert { " ** ALERT **" } else { "" };
        println!("    epoch={} pressure={:.4}{}", b.epoch, b.pressure, marker);
    }

    // ====================================================================
    // 6. Multi-scale memory tiers
    // ====================================================================
    println!("\n--- 6. Multi-Scale Memory (S/M/L Tiers) ---");

    // Query by scale to demonstrate tier-based retention
    let tiers = [
        ("S (short)", "2h"),
        ("M (medium)", "12h"),
        ("L (long)", "27d"),
    ];

    let query_vec = random_vector(dim, 77);
    for (tier_name, scale) in &tiers {
        let filter = FilterExpr::Eq(1, FilterValue::String(scale.to_string()));
        let opts = QueryOptions {
            filter: Some(filter),
            ..Default::default()
        };
        let results = store
            .query(&query_vec, 5, &opts)
            .expect("filtered query failed");
        println!("  Tier {} [scale={}]: {} results", tier_name, scale, results.len());
        print_atlas_results(&results, &all_windows);
    }

    // ====================================================================
    // 7. Witness chain
    // ====================================================================
    println!("\n--- 7. Witness Chain ---");

    let chain_steps = [
        ("genesis", 0x01u8),
        ("windowing", 0x02),
        ("feature_extraction", 0x02),
        ("embedding_ingest", 0x08),
        ("edge_construction", 0x02),
        ("coherence_compute", 0x02),
        ("boundary_tracking", 0x02),
        ("tier_assignment", 0x08),
        ("validation", 0x02),
        ("atlas_seal", 0x01),
    ];

    let entries: Vec<WitnessEntry> = chain_steps
        .iter()
        .enumerate()
        .map(|(i, (step, wtype))| {
            let action_data = format!("causal_atlas:{}:step_{}", step, i);
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

    println!("\n  Construction steps:");
    for (i, (step, _)) in chain_steps.iter().enumerate() {
        let wtype_name = match verified[i].witness_type {
            0x01 => "PROV",
            0x02 => "COMP",
            0x05 => "ATTS",
            0x08 => "DATA",
            _ => "????",
        };
        println!("    [{:>4}] {} -> {}", wtype_name, i, step);
    }

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Causal Atlas Summary ===\n");
    println!("  Targets:            {}", num_targets);
    println!("  Windows:            {}", num_windows);
    println!("  Embedding dims:     {}", dim);
    println!("  Causal edges:       {}", edges.len());
    println!("  Cut pressure:       {:.6}", coherence.cut_pressure);
    println!("  Partition entropy:  {:.6}", coherence.partition_entropy);
    println!("  Boundary alerts:    {}/{}", alert_count, boundaries.len());
    println!("  Witness entries:    {}", verified.len());

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn print_atlas_results(results: &[SearchResult], windows: &[LightCurveWindow]) {
    if results.is_empty() {
        println!("    (no results)");
        return;
    }
    for r in results.iter().take(3) {
        let idx = r.id as usize;
        if idx < windows.len() {
            let w = &windows[idx];
            println!(
                "    id={:>3} dist={:.6} target={} scale={} epoch={}",
                r.id, r.distance, w.target_id, w.scale, w.window_start_epoch
            );
        }
    }
}
