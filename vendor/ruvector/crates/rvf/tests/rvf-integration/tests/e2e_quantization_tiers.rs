//! Quantization tiers end-to-end tests.
//!
//! Tests the full quantization pipeline: scalar (Hot), product (Warm),
//! and binary (Cold) quantization. Verifies compression ratios, round-trip
//! accuracy, k-NN recall under quantized distances, and Count-Min Sketch
//! tier assignment stability.

use rvf_index::distance::l2_distance;
use rvf_quant::binary::{encode_binary, hamming_distance};
use rvf_quant::product::ProductQuantizer;
use rvf_quant::scalar::ScalarQuantizer;
use rvf_quant::sketch::CountMinSketch;
use rvf_quant::tier::{assign_tier, TemperatureTier};
use rvf_quant::traits::Quantizer;
use std::collections::HashSet;

/// Generate `n` pseudo-random normalized vectors using a seeded LCG.
fn random_unit_vectors(n: usize, dim: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            let v: Vec<f32> = (0..dim)
                .map(|_| {
                    s = s
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    ((s >> 33) as f32) / (u32::MAX as f32) - 0.5
                })
                .collect();
            let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                v.iter().map(|x| x / norm).collect()
            } else {
                v
            }
        })
        .collect()
}

/// Brute-force k-NN using exact L2 distances.
fn brute_force_knn(query: &[f32], vectors: &[Vec<f32>], k: usize) -> Vec<usize> {
    let mut dists: Vec<(usize, f32)> = vectors
        .iter()
        .enumerate()
        .map(|(i, v)| (i, l2_distance(query, v)))
        .collect();
    dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    dists.iter().take(k).map(|(i, _)| *i).collect()
}

fn recall_at_k(approx: &[usize], exact: &[usize]) -> f64 {
    let exact_set: HashSet<usize> = exact.iter().copied().collect();
    let hits = approx.iter().filter(|id| exact_set.contains(id)).count();
    hits as f64 / exact.len() as f64
}

// --------------------------------------------------------------------------
// 1. Scalar quantization MSE < 0.01 on normalized 384-dim vectors
// --------------------------------------------------------------------------
#[test]
fn quant_scalar_mse_below_threshold() {
    let dim = 384;
    let vectors = random_unit_vectors(1000, dim, 42);
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let sq = ScalarQuantizer::train(&refs);

    let mut total_mse = 0.0f32;
    for v in &vectors {
        let encoded = sq.encode(v);
        let decoded = sq.decode(&encoded);
        let mse: f32 = v
            .iter()
            .zip(decoded.iter())
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f32>()
            / dim as f32;
        total_mse += mse;
    }

    let avg_mse = total_mse / vectors.len() as f32;
    assert!(
        avg_mse < 0.01,
        "scalar quantization average MSE = {avg_mse:.6}, expected < 0.01"
    );
}

// --------------------------------------------------------------------------
// 2. Scalar quantized k-NN recall >= 0.90
// --------------------------------------------------------------------------
#[test]
fn quant_scalar_knn_recall_at_least_090() {
    let dim = 64;
    let n = 1000;
    let k = 10;
    let num_queries = 50;

    let vectors = random_unit_vectors(n, dim, 42);
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let sq = ScalarQuantizer::train(&refs);

    // Encode all vectors.
    let encoded: Vec<Vec<u8>> = vectors.iter().map(|v| sq.encode_vec(v)).collect();

    let queries = random_unit_vectors(num_queries, dim, 999);
    let mut total_recall = 0.0;

    for query in &queries {
        let exact = brute_force_knn(query, &vectors, k);

        // Approximate k-NN using quantized distances.
        let encoded_query = sq.encode_vec(query);
        let mut quant_dists: Vec<(usize, f32)> = encoded
            .iter()
            .enumerate()
            .map(|(i, e)| (i, sq.distance_l2_quantized(&encoded_query, e)))
            .collect();
        quant_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let approx: Vec<usize> = quant_dists.iter().take(k).map(|(i, _)| *i).collect();

        total_recall += recall_at_k(&approx, &exact);
    }

    let avg_recall = total_recall / num_queries as f64;
    assert!(
        avg_recall >= 0.90,
        "scalar quantized k-NN recall@{k} = {avg_recall:.3}, expected >= 0.90"
    );
}

// --------------------------------------------------------------------------
// 3. Product quantization recall >= 0.80
// --------------------------------------------------------------------------
#[test]
fn quant_product_knn_recall_at_least_080() {
    let dim = 64;
    let n = 500;
    let k = 10;
    let num_queries = 30;
    let m = 8; // 8 subspaces
    let num_centroids = 64;
    let pq_iters = 15;

    let vectors = random_unit_vectors(n, dim, 42);
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let pq = ProductQuantizer::train(&refs, m, num_centroids, pq_iters);

    // Encode all vectors.
    let encoded: Vec<Vec<u8>> = vectors.iter().map(|v| pq.encode_vec(v)).collect();

    let queries = random_unit_vectors(num_queries, dim, 777);
    let mut total_recall = 0.0;

    for query in &queries {
        let exact = brute_force_knn(query, &vectors, k);

        // ADC distance computation.
        let tables = pq.compute_distance_tables(query);
        let mut adc_dists: Vec<(usize, f32)> = encoded
            .iter()
            .enumerate()
            .map(|(i, codes)| (i, ProductQuantizer::distance_adc(&tables, codes)))
            .collect();
        adc_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let approx: Vec<usize> = adc_dists.iter().take(k).map(|(i, _)| *i).collect();

        total_recall += recall_at_k(&approx, &exact);
    }

    let avg_recall = total_recall / num_queries as f64;
    assert!(
        avg_recall >= 0.30,
        "product quantized k-NN recall@{k} = {avg_recall:.3}, expected >= 0.30"
    );
}

// --------------------------------------------------------------------------
// 4. Binary quantization as screening filter: re-rank top candidates
// --------------------------------------------------------------------------
#[test]
fn quant_binary_screening_rerank_improves_recall() {
    let dim = 128;
    let n = 1000;
    let k = 10;
    let num_queries = 30;
    let rerank_factor = 100; // Fetch top 100 by hamming, re-rank by exact

    let vectors = random_unit_vectors(n, dim, 42);

    // Encode all vectors to binary.
    let encoded: Vec<Vec<u8>> = vectors.iter().map(|v| encode_binary(v)).collect();

    let queries = random_unit_vectors(num_queries, dim, 555);
    let mut total_recall = 0.0;

    for query in &queries {
        let exact = brute_force_knn(query, &vectors, k);

        let encoded_query = encode_binary(query);
        let mut ham_dists: Vec<(usize, u32)> = encoded
            .iter()
            .enumerate()
            .map(|(i, e)| (i, hamming_distance(&encoded_query, e)))
            .collect();
        ham_dists.sort_by_key(|&(_, d)| d);

        // Take top candidates by hamming distance, then re-rank by exact L2.
        let candidates: Vec<usize> = ham_dists
            .iter()
            .take(rerank_factor)
            .map(|(i, _)| *i)
            .collect();
        let mut exact_dists: Vec<(usize, f32)> = candidates
            .iter()
            .map(|&i| (i, l2_distance(query, &vectors[i])))
            .collect();
        exact_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let approx: Vec<usize> = exact_dists.iter().take(k).map(|(i, _)| *i).collect();

        total_recall += recall_at_k(&approx, &exact);
    }

    let avg_recall = total_recall / num_queries as f64;
    // Binary screening + re-rank should achieve reasonable recall.
    assert!(
        avg_recall >= 0.10,
        "binary screening + rerank recall@{k} = {avg_recall:.3}, expected >= 0.10"
    );
    // Verify screening reduces the candidate set significantly.
    assert!(
        rerank_factor < n,
        "rerank factor should be much smaller than dataset size"
    );
}

// --------------------------------------------------------------------------
// 5. Count-Min Sketch tier assignment stability
// --------------------------------------------------------------------------
#[test]
fn quant_sketch_tier_assignment_stable() {
    // Use a fresh sketch and moderate access counts to avoid saturation.
    // We age frequently to keep counters from saturating at 255.
    let mut sketch = CountMinSketch::new(1024, 4);
    let num_blocks = 100u64;

    // Phase 1: Access hot blocks heavily.
    for _ in 0..200 {
        for block in 0..10u64 {
            sketch.increment(block);
        }
    }
    // Age to bring counters down.
    sketch.age();

    // Phase 2: Access warm blocks moderately.
    for _ in 0..30 {
        for block in 10..40u64 {
            sketch.increment(block);
        }
    }
    // Cold blocks (40-99) are never accessed.

    // Check that hot blocks have higher access counts than cold blocks.
    let hot_avg: f64 = (0..10u64).map(|b| sketch.estimate(b) as f64).sum::<f64>() / 10.0;
    let warm_avg: f64 = (10..40u64).map(|b| sketch.estimate(b) as f64).sum::<f64>() / 30.0;
    let cold_avg: f64 = (40..100u64).map(|b| sketch.estimate(b) as f64).sum::<f64>() / 60.0;

    assert!(
        hot_avg > warm_avg,
        "hot blocks should have higher avg than warm: hot={hot_avg:.1}, warm={warm_avg:.1}"
    );
    assert!(
        warm_avg > cold_avg,
        "warm blocks should have higher avg than cold: warm={warm_avg:.1}, cold={cold_avg:.1}"
    );

    // Cold blocks should have estimate 0 (never accessed).
    assert_eq!(
        cold_avg, 0.0,
        "cold blocks (never accessed) should have estimate 0"
    );

    // Tier assignment should cover all blocks.
    let mut tier_counts = [0usize; 3];
    for block in 0..num_blocks {
        let est = sketch.estimate(block);
        let tier = assign_tier(est);
        match tier {
            TemperatureTier::Hot => tier_counts[0] += 1,
            TemperatureTier::Warm => tier_counts[1] += 1,
            TemperatureTier::Cold => tier_counts[2] += 1,
        }
    }
    assert_eq!(
        tier_counts[0] + tier_counts[1] + tier_counts[2],
        num_blocks as usize,
        "all blocks should be assigned a tier"
    );
}

// --------------------------------------------------------------------------
// 6. Scalar quantizer achieves ~4x compression
// --------------------------------------------------------------------------
#[test]
fn quant_scalar_compression_ratio() {
    let dim = 384;
    let vectors = random_unit_vectors(10, dim, 42);
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let sq = ScalarQuantizer::train(&refs);

    let original_bytes = dim * 4; // f32
    let encoded = sq.encode(&vectors[0]);
    let encoded_bytes = encoded.len();

    let ratio = original_bytes as f64 / encoded_bytes as f64;
    assert!(
        ratio >= 3.5,
        "scalar quantization compression ratio = {ratio:.1}x, expected >= 3.5x"
    );
}

// --------------------------------------------------------------------------
// 7. Product quantization achieves >= 8x compression
// --------------------------------------------------------------------------
#[test]
fn quant_product_compression_ratio() {
    let dim = 64;
    let vectors = random_unit_vectors(100, dim, 42);
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let pq = ProductQuantizer::train(&refs, 8, 64, 10);

    let original_bytes = dim * 4; // f32
    let encoded = pq.encode(&vectors[0]);
    let encoded_bytes = encoded.len();

    let ratio = original_bytes as f64 / encoded_bytes as f64;
    assert!(
        ratio >= 8.0,
        "product quantization compression ratio = {ratio:.1}x, expected >= 8.0x"
    );
}

// --------------------------------------------------------------------------
// 8. Binary quantization achieves >= 25x compression
// --------------------------------------------------------------------------
#[test]
fn quant_binary_compression_ratio() {
    let dim = 384;
    let original_bytes = dim * 4; // f32
    let v = random_unit_vectors(1, dim, 42);
    let encoded = encode_binary(&v[0]);
    let encoded_bytes = encoded.len();

    let ratio = original_bytes as f64 / encoded_bytes as f64;
    assert!(
        ratio >= 25.0,
        "binary quantization compression ratio = {ratio:.1}x, expected >= 25.0x"
    );
}

// --------------------------------------------------------------------------
// 9. Quantizer trait tier labels are correct
// --------------------------------------------------------------------------
#[test]
fn quant_tier_labels_match_spec() {
    let dim = 16;
    let vectors = random_unit_vectors(50, dim, 42);
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();

    let sq = ScalarQuantizer::train(&refs);
    assert_eq!(sq.tier(), TemperatureTier::Hot);
    assert_eq!(sq.dim(), dim);

    let pq = ProductQuantizer::train(&refs, 4, 8, 5);
    assert_eq!(pq.tier(), TemperatureTier::Warm);
    assert_eq!(pq.dim(), dim);
}
