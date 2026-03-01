//! Performance Benchmarks for iOS WASM
//!
//! Run with: cargo bench

use std::time::Instant;

// Import the library
use ruvector_ios_wasm::*;

fn main() {
    println!("=== iOS WASM Vector Database Benchmarks ===\n");

    bench_simd_operations();
    bench_hnsw_operations();
    bench_quantization();
    bench_distance_metrics();
    bench_recommendation_engine();

    println!("\n=== All benchmarks completed ===");
}

fn bench_simd_operations() {
    println!("--- SIMD Operations ---");

    let dim = 128;
    let iterations = 10000;
    let a: Vec<f32> = (0..dim).map(|i| i as f32 / dim as f32).collect();
    let b: Vec<f32> = (0..dim).map(|i| (dim - i) as f32 / dim as f32).collect();

    // Dot product benchmark
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = dot_product(&a, &b);
    }
    let elapsed = start.elapsed();
    println!(
        "  dot_product({} dims, {} iter): {:?} ({:.0} ops/sec)",
        dim,
        iterations,
        elapsed,
        iterations as f64 / elapsed.as_secs_f64()
    );

    // L2 distance benchmark
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = l2_distance(&a, &b);
    }
    let elapsed = start.elapsed();
    println!(
        "  l2_distance({} dims, {} iter): {:?} ({:.0} ops/sec)",
        dim,
        iterations,
        elapsed,
        iterations as f64 / elapsed.as_secs_f64()
    );

    // Cosine similarity benchmark
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = cosine_similarity(&a, &b);
    }
    let elapsed = start.elapsed();
    println!(
        "  cosine_similarity({} dims, {} iter): {:?} ({:.0} ops/sec)",
        dim,
        iterations,
        elapsed,
        iterations as f64 / elapsed.as_secs_f64()
    );
}

fn bench_hnsw_operations() {
    println!("\n--- HNSW Index ---");

    let dim = 64;
    let num_vectors = 1000;

    // Generate random vectors
    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|i| {
            (0..dim)
                .map(|j| ((i * 17 + j * 31) % 100) as f32 / 100.0)
                .collect()
        })
        .collect();

    // Insert benchmark
    let mut index = HnswIndex::with_defaults(dim, DistanceMetric::Cosine);
    let start = Instant::now();
    for (i, v) in vectors.iter().enumerate() {
        index.insert(i as u64, v.clone());
    }
    let insert_elapsed = start.elapsed();
    println!(
        "  insert {} vectors: {:?} ({:.0} vec/sec)",
        num_vectors,
        insert_elapsed,
        num_vectors as f64 / insert_elapsed.as_secs_f64()
    );

    // Search benchmark
    let query = &vectors[500];
    let k = 10;
    let iterations = 1000;

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = index.search(query, k);
    }
    let search_elapsed = start.elapsed();
    println!(
        "  search top-{} ({} iter): {:?} ({:.0} qps)",
        k,
        iterations,
        search_elapsed,
        iterations as f64 / search_elapsed.as_secs_f64()
    );

    // Verify search quality
    let results = index.search(query, k);
    println!(
        "  search quality: found {} results, best dist={:.4}",
        results.len(),
        results.first().map(|(_, d)| *d).unwrap_or(f32::MAX)
    );
}

fn bench_quantization() {
    println!("\n--- Quantization ---");

    let dim = 128;
    let iterations = 10000;
    let vector: Vec<f32> = (0..dim).map(|i| i as f32 / dim as f32).collect();

    // Scalar quantization
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = ScalarQuantized::quantize(&vector);
    }
    let elapsed = start.elapsed();
    println!(
        "  scalar_quantize({} dims, {} iter): {:?} ({:.0} ops/sec)",
        dim,
        iterations,
        elapsed,
        iterations as f64 / elapsed.as_secs_f64()
    );

    // Binary quantization
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = BinaryQuantized::quantize(&vector);
    }
    let elapsed = start.elapsed();
    println!(
        "  binary_quantize({} dims, {} iter): {:?} ({:.0} ops/sec)",
        dim,
        iterations,
        elapsed,
        iterations as f64 / elapsed.as_secs_f64()
    );

    // Memory savings
    let sq = ScalarQuantized::quantize(&vector);
    let bq = BinaryQuantized::quantize(&vector);
    let original_size = dim * 4; // f32 = 4 bytes
    println!(
        "  memory: original={}B, scalar={}B ({}x), binary={}B ({}x)",
        original_size,
        sq.memory_size(),
        original_size / sq.memory_size(),
        bq.memory_size(),
        original_size / bq.memory_size()
    );
}

fn bench_distance_metrics() {
    println!("\n--- Distance Metrics ---");

    let dim = 128;
    let iterations = 10000;
    let a: Vec<f32> = (0..dim).map(|i| i as f32 / dim as f32).collect();
    let b: Vec<f32> = (0..dim).map(|i| (dim - i) as f32 / dim as f32).collect();

    let metrics = [
        ("Euclidean", DistanceMetric::Euclidean),
        ("Cosine", DistanceMetric::Cosine),
        ("Manhattan", DistanceMetric::Manhattan),
        ("DotProduct", DistanceMetric::DotProduct),
    ];

    for (name, metric) in metrics {
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = distance::distance(&a, &b, metric);
        }
        let elapsed = start.elapsed();
        println!(
            "  {}: {:?} ({:.0} ops/sec)",
            name,
            elapsed,
            iterations as f64 / elapsed.as_secs_f64()
        );
    }
}

fn bench_recommendation_engine() {
    println!("\n--- Recommendation Engine ---");

    // Create VectorDatabase
    let dim = 64;
    let num_vectors = 500;

    let mut db = VectorDatabase::new(dim, DistanceMetric::Cosine, QuantizationMode::None);

    // Insert vectors
    let start = Instant::now();
    for i in 0..num_vectors {
        let v: Vec<f32> = (0..dim)
            .map(|j| ((i * 17 + j * 31) % 100) as f32 / 100.0)
            .collect();
        db.insert(i as u64, v);
    }
    let insert_elapsed = start.elapsed();
    println!(
        "  VectorDB insert {} vectors: {:?}",
        num_vectors, insert_elapsed
    );

    // Search
    let query: Vec<f32> = (0..dim).map(|i| i as f32 / dim as f32).collect();
    let iterations = 1000;

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = db.search(&query, 10);
    }
    let search_elapsed = start.elapsed();
    println!(
        "  VectorDB search ({} iter): {:?} ({:.0} qps)",
        iterations,
        search_elapsed,
        iterations as f64 / search_elapsed.as_secs_f64()
    );

    // Memory usage
    println!("  VectorDB memory: {} bytes", db.memory_usage());
}
