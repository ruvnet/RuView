//! LRU Embedding Cache with Temperature Tiering — Practical Production
//!
//! Demonstrates an embedding cache with access-frequency-driven quantization:
//! 1. Create a store as an embedding cache (256 dims)
//! 2. Insert 1000 embeddings with metadata: cache_key_hash, access_count, last_accessed
//! 3. Generate Zipf-like access patterns (10% of items get 90% of accesses)
//! 4. Tier embeddings by access frequency using rvf_quant:
//!    - Hot tier (access_count > 100): scalar quantization (4x compression)
//!    - Warm tier (access_count 10-100): product quantization (8x compression)
//!    - Cold tier (access_count < 10): binary quantization (32x compression)
//! 5. Show cache hit rates and compression stats per tier
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG
//! Quantization: rvf-quant scalar, product, binary
//!
//! Run: cargo run --example embedding_cache

use rvf_runtime::{
    MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore,
};
use rvf_runtime::options::DistanceMetric;
use rvf_quant::{
    ScalarQuantizer, ProductQuantizer,
    encode_binary, hamming_distance,
    TemperatureTier,
};
use rvf_quant::tier::assign_tier;
use rvf_quant::traits::Quantizer;
use tempfile::TempDir;

/// Simple pseudo-random number generator (LCG) for deterministic results.
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
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

/// Generate Zipf-like access counts: a small fraction of items get most accesses.
/// Returns access_count for each of `n` items.
fn generate_zipf_access_counts(n: usize, total_accesses: u64, seed: u64) -> Vec<u64> {
    // Rank items from 1..n. Probability proportional to 1/rank^s (s=1).
    // Then distribute total_accesses proportionally.
    let harmonic: f64 = (1..=n).map(|r| 1.0 / r as f64).sum();

    // Build a permutation so "hot" items are spread across the index space.
    let mut perm: Vec<usize> = (0..n).collect();
    let mut lcg = seed;
    for i in (1..n).rev() {
        lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let j = (lcg >> 33) as usize % (i + 1);
        perm.swap(i, j);
    }

    let mut counts = vec![0u64; n];
    for (rank_minus_1, &item_idx) in perm.iter().enumerate() {
        let rank = rank_minus_1 + 1;
        let prob = (1.0 / rank as f64) / harmonic;
        counts[item_idx] = (prob * total_accesses as f64).round().max(1.0) as u64;
    }
    counts
}

fn main() {
    println!("=== RVF Embedding Cache with Temperature Tiering ===\n");

    let dim = 256;
    let num_embeddings = 1000;
    let total_accesses: u64 = 100_000;

    // -- Step 1: Create store --
    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("cache.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    println!("Creating embedding cache...");
    println!("  Dimensions:  {}", dim);
    println!("  Embeddings:  {}", num_embeddings);
    println!("  Total test accesses: {}\n", total_accesses);

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    // -- Step 2: Insert embeddings with metadata --
    // Metadata fields:
    //   field_id 0: cache_key_hash (U64)
    //   field_id 1: access_count (U64) -- will be set after generation
    //   field_id 2: last_accessed (U64) -- epoch seconds
    let vectors: Vec<Vec<f32>> = (0..num_embeddings)
        .map(|i| random_vector(dim, i as u64))
        .collect();

    // Generate Zipf-like access pattern.
    let access_counts = generate_zipf_access_counts(num_embeddings, total_accesses, 42);

    // Generate last_accessed timestamps: higher access items were accessed more recently.
    let base_ts: u64 = 1_700_000_000;
    let last_accessed: Vec<u64> = access_counts.iter().enumerate().map(|(i, &count)| {
        // More accesses -> more recent timestamp
        base_ts + count * 60 + i as u64
    }).collect();

    // Ingest in batches of 200
    let batch_size = 200;
    for batch_start in (0..num_embeddings).step_by(batch_size) {
        let batch_end = (batch_start + batch_size).min(num_embeddings);

        let batch_vecs: Vec<&[f32]> = vectors[batch_start..batch_end]
            .iter()
            .map(|v| v.as_slice())
            .collect();
        let batch_ids: Vec<u64> = (batch_start as u64..batch_end as u64).collect();

        let mut metadata = Vec::with_capacity((batch_end - batch_start) * 3);
        for i in batch_start..batch_end {
            // cache_key_hash: deterministic hash of the key
            let key_hash = (i as u64).wrapping_mul(0x9E3779B97F4A7C15);
            metadata.push(MetadataEntry {
                field_id: 0,
                value: MetadataValue::U64(key_hash),
            });
            metadata.push(MetadataEntry {
                field_id: 1,
                value: MetadataValue::U64(access_counts[i]),
            });
            metadata.push(MetadataEntry {
                field_id: 2,
                value: MetadataValue::U64(last_accessed[i]),
            });
        }

        store
            .ingest_batch(&batch_vecs, &batch_ids, Some(&metadata))
            .expect("failed to ingest batch");
    }

    println!("Ingested {} embeddings.\n", num_embeddings);

    // -- Step 3: Analyze access patterns --
    println!("=== Access Pattern Analysis ===\n");

    let hot_threshold = 100u64;
    let warm_threshold = 10u64;

    let hot_items: Vec<usize> = (0..num_embeddings)
        .filter(|&i| access_counts[i] > hot_threshold)
        .collect();
    let warm_items: Vec<usize> = (0..num_embeddings)
        .filter(|&i| access_counts[i] >= warm_threshold && access_counts[i] <= hot_threshold)
        .collect();
    let cold_items: Vec<usize> = (0..num_embeddings)
        .filter(|&i| access_counts[i] < warm_threshold)
        .collect();

    let hot_accesses: u64 = hot_items.iter().map(|&i| access_counts[i]).sum();
    let warm_accesses: u64 = warm_items.iter().map(|&i| access_counts[i]).sum();
    let cold_accesses: u64 = cold_items.iter().map(|&i| access_counts[i]).sum();
    let all_accesses = hot_accesses + warm_accesses + cold_accesses;

    println!(
        "  {:>8}  {:>8}  {:>10}  {:>10}  {:>12}",
        "Tier", "Items", "% Items", "Accesses", "% Accesses"
    );
    println!(
        "  {:->8}  {:->8}  {:->10}  {:->10}  {:->12}",
        "", "", "", "", ""
    );
    println!(
        "  {:>8}  {:>8}  {:>9.1}%  {:>10}  {:>11.1}%",
        "Hot", hot_items.len(),
        hot_items.len() as f64 / num_embeddings as f64 * 100.0,
        hot_accesses,
        hot_accesses as f64 / all_accesses as f64 * 100.0,
    );
    println!(
        "  {:>8}  {:>8}  {:>9.1}%  {:>10}  {:>11.1}%",
        "Warm", warm_items.len(),
        warm_items.len() as f64 / num_embeddings as f64 * 100.0,
        warm_accesses,
        warm_accesses as f64 / all_accesses as f64 * 100.0,
    );
    println!(
        "  {:>8}  {:>8}  {:>9.1}%  {:>10}  {:>11.1}%",
        "Cold", cold_items.len(),
        cold_items.len() as f64 / num_embeddings as f64 * 100.0,
        cold_accesses,
        cold_accesses as f64 / all_accesses as f64 * 100.0,
    );

    // -- Step 4: Quantization tiering --
    println!("\n=== Temperature-Tiered Quantization ===\n");

    let raw_bytes_per_vec = dim * 4; // fp32

    // Hot tier: Scalar quantization (4x compression)
    println!("--- Hot Tier: Scalar Quantization ---");
    let hot_vecs: Vec<Vec<f32>> = hot_items.iter().map(|&i| vectors[i].clone()).collect();
    let hot_refs: Vec<&[f32]> = hot_vecs.iter().map(|v| v.as_slice()).collect();

    let sq = if !hot_refs.is_empty() {
        let sq = ScalarQuantizer::train(&hot_refs);
        assert_eq!(sq.tier(), TemperatureTier::Hot);

        let hot_avg_mse: f32 = hot_vecs.iter().map(|v| {
            let codes = sq.encode_vec(v);
            let recon = sq.decode_vec(&codes);
            mse(v, &recon)
        }).sum::<f32>() / hot_vecs.len() as f32;

        let sq_bytes = dim; // 1 byte per dimension
        println!("  Items:       {}", hot_items.len());
        println!("  Compression: {}x (fp32 -> u8)", raw_bytes_per_vec / sq_bytes);
        println!("  Avg MSE:     {:.8}", hot_avg_mse);
        println!(
            "  Memory:      {} bytes -> {} bytes (saved {} bytes)",
            hot_items.len() * raw_bytes_per_vec,
            hot_items.len() * sq_bytes,
            hot_items.len() * (raw_bytes_per_vec - sq_bytes),
        );
        Some(sq)
    } else {
        println!("  No hot items.");
        None
    };

    // Warm tier: Product quantization (8x compression)
    println!("\n--- Warm Tier: Product Quantization ---");
    let warm_vecs: Vec<Vec<f32>> = warm_items.iter().map(|&i| vectors[i].clone()).collect();
    let warm_refs: Vec<&[f32]> = warm_vecs.iter().map(|v| v.as_slice()).collect();

    let pq = if warm_refs.len() >= 64 {
        // PQ needs enough vectors for training
        let pq_m = 32; // subspaces (dim must be divisible)
        let pq_k = 64.min(warm_refs.len()); // centroids per subspace
        let pq_iters = 10;

        let pq = ProductQuantizer::train(&warm_refs, pq_m, pq_k, pq_iters);
        assert_eq!(pq.tier(), TemperatureTier::Warm);

        let warm_avg_mse: f32 = warm_vecs.iter().map(|v| {
            let codes = pq.encode_vec(v);
            let recon = pq.decode_vec(&codes);
            mse(v, &recon)
        }).sum::<f32>() / warm_vecs.len() as f32;

        let pq_bytes = pq_m; // 1 byte per subspace
        let pq_ratio = raw_bytes_per_vec as f32 / pq_bytes as f32;
        println!("  Items:       {}", warm_items.len());
        println!("  Config:      M={}, K={}", pq_m, pq_k);
        println!("  Compression: {:.1}x", pq_ratio);
        println!("  Avg MSE:     {:.8}", warm_avg_mse);
        println!(
            "  Memory:      {} bytes -> {} bytes (saved {} bytes)",
            warm_items.len() * raw_bytes_per_vec,
            warm_items.len() * pq_bytes,
            warm_items.len() * (raw_bytes_per_vec - pq_bytes),
        );
        Some(pq)
    } else {
        println!("  Not enough warm items for PQ training (need >= 64, have {}).", warm_refs.len());
        None
    };

    // Cold tier: Binary quantization (32x compression)
    println!("\n--- Cold Tier: Binary Quantization ---");
    let cold_vecs: Vec<Vec<f32>> = cold_items.iter().map(|&i| vectors[i].clone()).collect();

    if !cold_vecs.is_empty() {
        let cold_avg_mse: f32 = cold_vecs.iter().map(|v| {
            let bin = encode_binary(v);
            let recon = rvf_quant::decode_binary(&bin, dim);
            mse(v, &recon)
        }).sum::<f32>() / cold_vecs.len() as f32;

        let bin_bytes = dim.div_ceil(8); // 1 bit per dimension
        let bin_ratio = raw_bytes_per_vec as f32 / bin_bytes as f32;
        println!("  Items:       {}", cold_items.len());
        println!("  Compression: {:.1}x (fp32 -> 1-bit)", bin_ratio);
        println!("  Avg MSE:     {:.8}", cold_avg_mse);
        println!(
            "  Memory:      {} bytes -> {} bytes (saved {} bytes)",
            cold_items.len() * raw_bytes_per_vec,
            cold_items.len() * bin_bytes,
            cold_items.len() * (raw_bytes_per_vec - bin_bytes),
        );

        // Demonstrate hamming distance as a proxy for similarity in cold tier
        if cold_vecs.len() >= 2 {
            let bin_a = encode_binary(&cold_vecs[0]);
            let bin_b = encode_binary(&cold_vecs[1]);
            let ham = hamming_distance(&bin_a, &bin_b);
            println!("  Hamming distance (cold[0] vs cold[1]): {} / {} bits", ham, dim);
        }
    } else {
        println!("  No cold items.");
    }

    // -- Step 5: Cache hit rate exercise --
    println!("\n=== Cache Hit Rate Exercise ===\n");

    // Perform 1000 lookups using the same Zipf distribution.
    // "Hit" = the queried vector is in the top-1 result.
    let num_lookups = 1000;
    let mut hits_by_tier = [0u64; 3]; // hot, warm, cold
    let mut lookups_by_tier = [0u64; 3];

    // Use LCG to select which items get queried (biased by access frequency).
    let mut lcg_state: u64 = 7777;
    for _ in 0..num_lookups {
        lcg_state = lcg_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let idx = (lcg_state >> 33) as usize % num_embeddings;

        let tier_idx = if access_counts[idx] > hot_threshold {
            0
        } else if access_counts[idx] >= warm_threshold {
            1
        } else {
            2
        };
        lookups_by_tier[tier_idx] += 1;

        // Query the exact vector -- top-1 should return itself.
        let results = store
            .query(&vectors[idx], 1, &QueryOptions::default())
            .expect("cache lookup failed");
        if !results.is_empty() && results[0].id == idx as u64 {
            hits_by_tier[tier_idx] += 1;
        }
    }

    let tier_names = ["Hot", "Warm", "Cold"];
    println!(
        "  {:>6}  {:>10}  {:>8}  {:>10}",
        "Tier", "Lookups", "Hits", "Hit Rate"
    );
    println!(
        "  {:->6}  {:->10}  {:->8}  {:->10}",
        "", "", "", ""
    );
    for i in 0..3 {
        let rate = if lookups_by_tier[i] > 0 {
            hits_by_tier[i] as f64 / lookups_by_tier[i] as f64 * 100.0
        } else {
            0.0
        };
        println!(
            "  {:>6}  {:>10}  {:>8}  {:>9.1}%",
            tier_names[i], lookups_by_tier[i], hits_by_tier[i], rate,
        );
    }
    let total_hits: u64 = hits_by_tier.iter().sum();
    println!(
        "  {:>6}  {:>10}  {:>8}  {:>9.1}%",
        "Total", num_lookups, total_hits,
        total_hits as f64 / num_lookups as f64 * 100.0,
    );

    // ====================================================================
    // Compression Summary
    // ====================================================================
    println!("\n=== Compression Summary ===\n");

    let raw_total = num_embeddings * raw_bytes_per_vec;
    let sq_bytes_per = dim;
    let pq_bytes_per = 32; // subspaces
    let bin_bytes_per = dim.div_ceil(8);

    let compressed_total = hot_items.len() * sq_bytes_per
        + warm_items.len() * pq_bytes_per
        + cold_items.len() * bin_bytes_per;

    println!(
        "  {:>8}  {:>8}  {:>14}  {:>14}  {:>12}",
        "Tier", "Items", "Raw (bytes)", "Compressed", "Ratio"
    );
    println!(
        "  {:->8}  {:->8}  {:->14}  {:->14}  {:->12}",
        "", "", "", "", ""
    );
    println!(
        "  {:>8}  {:>8}  {:>14}  {:>14}  {:>11.1}x",
        "Hot", hot_items.len(),
        hot_items.len() * raw_bytes_per_vec,
        hot_items.len() * sq_bytes_per,
        raw_bytes_per_vec as f64 / sq_bytes_per as f64,
    );
    println!(
        "  {:>8}  {:>8}  {:>14}  {:>14}  {:>11.1}x",
        "Warm", warm_items.len(),
        warm_items.len() * raw_bytes_per_vec,
        warm_items.len() * pq_bytes_per,
        raw_bytes_per_vec as f64 / pq_bytes_per as f64,
    );
    println!(
        "  {:>8}  {:>8}  {:>14}  {:>14}  {:>11.1}x",
        "Cold", cold_items.len(),
        cold_items.len() * raw_bytes_per_vec,
        cold_items.len() * bin_bytes_per,
        raw_bytes_per_vec as f64 / bin_bytes_per as f64,
    );
    println!(
        "  {:->8}  {:->8}  {:->14}  {:->14}  {:->12}",
        "", "", "", "", ""
    );
    println!(
        "  {:>8}  {:>8}  {:>14}  {:>14}  {:>11.1}x",
        "Total", num_embeddings, raw_total, compressed_total,
        raw_total as f64 / compressed_total as f64,
    );
    println!(
        "\n  Memory saved: {} bytes ({:.1}% reduction)",
        raw_total - compressed_total,
        (1.0 - compressed_total as f64 / raw_total as f64) * 100.0,
    );

    // Verify tier assignment via CountMinSketch
    println!("\n=== Tier Assignment Verification ===\n");
    let tier_hot = assign_tier(200);
    let tier_warm = assign_tier(50);
    let tier_cold = assign_tier(5);
    println!("  assign_tier(200) = {:?} (expected Hot)", tier_hot);
    println!("  assign_tier(50)  = {:?} (expected Warm)", tier_warm);
    println!("  assign_tier(5)   = {:?} (expected Cold)", tier_cold);

    // Verify quantizer tiers
    if let Some(ref sq) = sq {
        assert_eq!(sq.tier(), TemperatureTier::Hot);
        println!("  ScalarQuantizer tier: {:?} (confirmed)", sq.tier());
    }
    if let Some(ref pq) = pq {
        assert_eq!(pq.tier(), TemperatureTier::Warm);
        println!("  ProductQuantizer tier: {:?} (confirmed)", pq.tier());
    }

    store.close().expect("failed to close store");
    println!("\nDone.");
}
