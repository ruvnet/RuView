//! Batch operations example
//!
//! Demonstrates efficient batch processing for high throughput

use ruvector_core::{VectorDB, VectorEntry, SearchQuery, DbOptions, Result};
use rand::Rng;
use std::time::Instant;

fn main() -> Result<()> {
    println!("🚀 Ruvector Batch Operations Example\n");

    // Setup
    let mut options = DbOptions::default();
    options.dimensions = 128;
    options.storage_path = "./examples_batch.db".to_string();

    let db = VectorDB::new(options)?;

    // Generate test data
    println!("1. Generating 10,000 random vectors...");
    let mut rng = rand::thread_rng();
    let entries: Vec<VectorEntry> = (0..10_000)
        .map(|i| {
            let vector: Vec<f32> = (0..128)
                .map(|_| rng.gen::<f32>())
                .collect();

            VectorEntry {
                id: Some(format!("vec_{:05}", i)),
                vector,
                metadata: None,
            }
        })
        .collect();
    println!("   ✓ Generated 10,000 vectors\n");

    // Batch insert
    println!("2. Batch inserting 10,000 vectors...");
    let start = Instant::now();
    let ids = db.insert_batch(entries)?;
    let duration = start.elapsed();

    println!("   ✓ Inserted {} vectors", ids.len());
    println!("   ✓ Time: {:?}", duration);
    println!("   ✓ Throughput: {:.0} vectors/sec\n",
        ids.len() as f64 / duration.as_secs_f64()
    );

    // Benchmark search
    println!("3. Benchmarking search operations...");
    let num_queries = 1000;
    let query_vector: Vec<f32> = (0..128).map(|_| rng.gen::<f32>()).collect();

    let start = Instant::now();
    for _ in 0..num_queries {
        let query = SearchQuery {
            vector: query_vector.clone(),
            k: 10,
            filter: None,
            include_vectors: false,
        };
        db.search(&query)?;
    }
    let duration = start.elapsed();

    println!("   ✓ Executed {} queries", num_queries);
    println!("   ✓ Total time: {:?}", duration);
    println!("   ✓ Average latency: {:.2}ms",
        duration.as_secs_f64() * 1000.0 / num_queries as f64
    );
    println!("   ✓ Throughput: {:.0} queries/sec\n",
        num_queries as f64 / duration.as_secs_f64()
    );

    println!("✅ Batch operations completed!");

    Ok(())
}
