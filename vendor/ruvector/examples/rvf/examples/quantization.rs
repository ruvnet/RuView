//! Temperature-Tiered Quantization
//!
//! Demonstrates all three quantization tiers in the RVF format:
//! 1. Scalar quantization (Hot tier): fp32 -> u8, 4x compression
//! 2. Product quantization (Warm tier): fp32 -> PQ codes, 8-16x compression
//! 3. Binary quantization (Cold tier): fp32 -> 1-bit, 32x compression
//! 4. Count-Min Sketch: track access patterns, assign temperature tiers

use rvf_quant::{
    ScalarQuantizer, ProductQuantizer, CountMinSketch,
    encode_binary, decode_binary, hamming_distance,
    TemperatureTier,
};
use rvf_quant::tier::assign_tier;
use rvf_quant::traits::Quantizer;

/// LCG-based pseudo-random vector generator.
fn random_vectors(n: usize, dim: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            (0..dim)
                .map(|_| {
                    s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    ((s >> 33) as f32) / (u32::MAX as f32) - 0.5
                })
                .collect()
        })
        .collect()
}

/// Compute mean squared error between original and reconstructed vectors.
fn mse(original: &[f32], reconstructed: &[f32]) -> f32 {
    assert_eq!(original.len(), reconstructed.len());
    let sum: f32 = original
        .iter()
        .zip(reconstructed.iter())
        .map(|(a, b)| {
            let d = a - b;
            d * d
        })
        .sum();
    sum / original.len() as f32
}

fn main() {
    println!("=== RVF Quantization Example ===\n");

    let dim = 384;
    let n = 1000;

    println!("Generating {} random vectors ({} dims)...\n", n, dim);
    let vectors = random_vectors(n, dim, 42);
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();

    // ====================================================================
    // 1. Scalar Quantization (Hot tier)
    // ====================================================================
    println!("--- 1. Scalar Quantization (Hot Tier) ---");
    println!("  Compression: fp32 -> u8 (4x)");
    println!("  Training scalar quantizer...");

    let sq = ScalarQuantizer::train(&vec_refs);
    assert_eq!(sq.tier(), TemperatureTier::Hot);
    assert_eq!(sq.dim(), dim);

    // Encode and decode a sample vector.
    let sample = &vectors[0];
    let sq_encoded = sq.encode_vec(sample);
    let sq_decoded = sq.decode_vec(&sq_encoded);

    let sq_error = mse(sample, &sq_decoded);
    let sq_orig_bytes = dim * 4; // fp32
    let sq_comp_bytes = sq_encoded.len(); // u8 per dim
    let sq_ratio = sq_orig_bytes as f32 / sq_comp_bytes as f32;

    println!("  Encoded size: {} bytes (from {} bytes)", sq_comp_bytes, sq_orig_bytes);
    println!("  Compression ratio: {:.1}x", sq_ratio);
    println!("  Reconstruction MSE: {:.8}", sq_error);

    // Compute average MSE over all vectors.
    let sq_avg_mse: f32 = vectors
        .iter()
        .map(|v| {
            let codes = sq.encode_vec(v);
            let recon = sq.decode_vec(&codes);
            mse(v, &recon)
        })
        .sum::<f32>()
        / n as f32;
    println!("  Average MSE (all {} vectors): {:.8}", n, sq_avg_mse);

    // Quantized distance comparison.
    let a_codes = sq.encode_vec(&vectors[0]);
    let b_codes = sq.encode_vec(&vectors[1]);
    let quant_dist = sq.distance_l2_quantized(&a_codes, &b_codes);
    let exact_dist: f32 = vectors[0]
        .iter()
        .zip(vectors[1].iter())
        .map(|(x, y)| { let d = x - y; d * d })
        .sum();
    println!(
        "  Distance (quantized vs exact): {:.4} vs {:.4} (error: {:.4})",
        quant_dist,
        exact_dist,
        (quant_dist - exact_dist).abs()
    );

    // ====================================================================
    // 2. Product Quantization (Warm tier)
    // ====================================================================
    println!("\n--- 2. Product Quantization (Warm Tier) ---");

    let pq_m = 48;  // Number of subspaces (dim must be divisible by M)
    let pq_k = 64;  // Centroids per subspace
    let pq_iters = 20;

    println!(
        "  Config: M={}, K={}, iterations={}, sub_dim={}",
        pq_m,
        pq_k,
        pq_iters,
        dim / pq_m,
    );
    println!("  Training product quantizer...");

    let pq = ProductQuantizer::train(&vec_refs, pq_m, pq_k, pq_iters);
    assert_eq!(pq.tier(), TemperatureTier::Warm);
    assert_eq!(pq.dim(), dim);

    let pq_encoded = pq.encode_vec(sample);
    let pq_decoded = pq.decode_vec(&pq_encoded);

    let pq_error = mse(sample, &pq_decoded);
    let pq_comp_bytes = pq_encoded.len(); // 1 byte per subspace
    let pq_ratio = sq_orig_bytes as f32 / pq_comp_bytes as f32;

    println!("  Encoded size: {} bytes (from {} bytes)", pq_comp_bytes, sq_orig_bytes);
    println!("  Compression ratio: {:.1}x", pq_ratio);
    println!("  Reconstruction MSE: {:.8}", pq_error);

    // ADC (Asymmetric Distance Computation) demo.
    let query = &vectors[42];
    let tables = pq.compute_distance_tables(query);
    let target_codes = pq.encode_vec(&vectors[99]);
    let adc_dist = ProductQuantizer::distance_adc(&tables, &target_codes);

    let exact_dist_pq: f32 = query
        .iter()
        .zip(vectors[99].iter())
        .map(|(x, y)| { let d = x - y; d * d })
        .sum();
    println!(
        "  ADC distance (query[42] -> vec[99]): {:.4} (exact: {:.4})",
        adc_dist, exact_dist_pq,
    );

    // Average MSE.
    let pq_avg_mse: f32 = vectors
        .iter()
        .map(|v| {
            let codes = pq.encode_vec(v);
            let recon = pq.decode_vec(&codes);
            mse(v, &recon)
        })
        .sum::<f32>()
        / n as f32;
    println!("  Average MSE (all {} vectors): {:.8}", n, pq_avg_mse);

    // ====================================================================
    // 3. Binary Quantization (Cold tier)
    // ====================================================================
    println!("\n--- 3. Binary Quantization (Cold Tier) ---");
    println!("  Compression: fp32 -> 1-bit (32x)");

    let bin_encoded = encode_binary(sample);
    let bin_decoded = decode_binary(&bin_encoded, dim);

    let bin_error = mse(sample, &bin_decoded);
    let bin_comp_bytes = bin_encoded.len();
    let bin_ratio = sq_orig_bytes as f32 / bin_comp_bytes as f32;

    println!("  Encoded size: {} bytes (from {} bytes)", bin_comp_bytes, sq_orig_bytes);
    println!("  Compression ratio: {:.1}x", bin_ratio);
    println!("  Reconstruction MSE: {:.8}", bin_error);

    // Hamming distance demo.
    let bin_a = encode_binary(&vectors[0]);
    let bin_b = encode_binary(&vectors[1]);
    let ham_dist = hamming_distance(&bin_a, &bin_b);
    println!(
        "  Hamming distance (vec[0] vs vec[1]): {} / {} bits",
        ham_dist, dim,
    );

    // ====================================================================
    // 4. Count-Min Sketch: Temperature Assignment
    // ====================================================================
    println!("\n--- 4. Count-Min Sketch: Temperature Tracking ---");

    let mut sketch = CountMinSketch::default_sketch();
    println!(
        "  Sketch size: {} bytes (width={}, depth={})",
        sketch.memory_bytes(),
        sketch.width,
        sketch.depth,
    );

    // Demonstrate access patterns:
    // - Block 0: very hot (200 accesses)
    // - Block 1: warm (50 accesses)
    // - Block 2: cold (5 accesses)
    // - Block 3: never accessed
    let access_patterns = [(0u64, 200u32), (1, 50), (2, 5)];

    for &(block_id, count) in &access_patterns {
        for _ in 0..count {
            sketch.increment(block_id);
        }
    }

    println!("\n  Access patterns:");
    println!("  {:>8}  {:>10}  {:>10}  {:>8}", "Block", "Accesses", "Estimate", "Tier");
    println!("  {:->8}  {:->10}  {:->10}  {:->8}", "", "", "", "");

    for &(block_id, true_count) in &access_patterns {
        let estimate = sketch.estimate(block_id);
        let tier = assign_tier(estimate);
        println!(
            "  {:>8}  {:>10}  {:>10}  {:>8?}",
            block_id, true_count, estimate, tier,
        );
    }

    // Unseen block.
    let unseen_est = sketch.estimate(3);
    let unseen_tier = assign_tier(unseen_est);
    println!(
        "  {:>8}  {:>10}  {:>10}  {:>8?}",
        3, 0, unseen_est, unseen_tier,
    );

    // Show aging effect.
    println!("\n  After aging (halving all counters):");
    sketch.age();
    for &(block_id, _) in &access_patterns {
        let estimate = sketch.estimate(block_id);
        let tier = assign_tier(estimate);
        println!(
            "    Block {}: estimate={}, tier={:?}",
            block_id, estimate, tier,
        );
    }

    // ====================================================================
    // Summary Table
    // ====================================================================
    println!("\n=== Quantization Comparison Summary ===\n");
    println!(
        "  {:>12}  {:>12}  {:>18}  {:>14}",
        "Tier", "Compression", "Avg MSE", "Bytes/Vector"
    );
    println!(
        "  {:->12}  {:->12}  {:->18}  {:->14}",
        "", "", "", ""
    );
    println!(
        "  {:>12}  {:>11.1}x  {:>18.8}  {:>14}",
        "Hot (SQ)", sq_ratio, sq_avg_mse, sq_comp_bytes
    );
    println!(
        "  {:>12}  {:>11.1}x  {:>18.8}  {:>14}",
        "Warm (PQ)", pq_ratio, pq_avg_mse, pq_comp_bytes
    );
    println!(
        "  {:>12}  {:>11.1}x  {:>18.8}  {:>14}",
        "Cold (Bin)", bin_ratio, bin_error, bin_comp_bytes
    );
    println!(
        "  {:>12}  {:>11.1}x  {:>18}  {:>14}",
        "Raw fp32", 1.0, "0.00000000", sq_orig_bytes
    );

    println!("\nDone.");
}
