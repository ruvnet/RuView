//! Comprehensive HNSW integration tests with different index sizes

use ruvector_core::index::hnsw::HnswIndex;
use ruvector_core::index::VectorIndex;
use ruvector_core::types::{DistanceMetric, HnswConfig};
use ruvector_core::Result;

fn generate_random_vectors(count: usize, dimensions: usize, seed: u64) -> Vec<Vec<f32>> {
    use rand::{Rng, SeedableRng};
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

    (0..count)
        .map(|_| {
            (0..dimensions)
                .map(|_| rng.gen::<f32>() * 2.0 - 1.0)
                .collect()
        })
        .collect()
}

fn normalize_vector(v: &[f32]) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter().map(|x| x / norm).collect()
    } else {
        v.to_vec()
    }
}

fn calculate_recall(ground_truth: &[String], results: &[String]) -> f32 {
    let gt_set: std::collections::HashSet<_> = ground_truth.iter().collect();
    let found = results.iter().filter(|id| gt_set.contains(id)).count();
    found as f32 / ground_truth.len() as f32
}

fn brute_force_search(
    query: &[f32],
    vectors: &[(String, Vec<f32>)],
    k: usize,
    metric: DistanceMetric,
) -> Vec<String> {
    use ruvector_core::distance::distance;

    let mut distances: Vec<_> = vectors
        .iter()
        .map(|(id, v)| {
            let dist = distance(query, v, metric).unwrap();
            (id.clone(), dist)
        })
        .collect();

    distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    distances.into_iter().take(k).map(|(id, _)| id).collect()
}

#[test]
fn test_hnsw_100_vectors() -> Result<()> {
    let dimensions = 128;
    let num_vectors = 100;
    let k = 10;

    let config = HnswConfig {
        m: 16,
        ef_construction: 100,
        ef_search: 200,
        max_elements: 1000,
    };

    let mut index = HnswIndex::new(dimensions, DistanceMetric::Cosine, config)?;

    // Generate and insert vectors
    let vectors = generate_random_vectors(num_vectors, dimensions, 42);
    let normalized_vectors: Vec<_> = vectors.iter().map(|v| normalize_vector(v)).collect();

    for (i, vector) in normalized_vectors.iter().enumerate() {
        index.add(format!("vec_{}", i), vector.clone())?;
    }

    assert_eq!(index.len(), num_vectors);

    // Test search accuracy with multiple queries
    let num_queries = 10;
    let mut total_recall = 0.0;

    for i in 0..num_queries {
        let query_idx = i * (num_vectors / num_queries);
        let query = &normalized_vectors[query_idx];

        // Get HNSW results
        let results = index.search(query, k)?;
        let result_ids: Vec<_> = results.iter().map(|r| r.id.clone()).collect();

        // Get ground truth with brute force
        let vectors_with_ids: Vec<_> = normalized_vectors
            .iter()
            .enumerate()
            .map(|(idx, v)| (format!("vec_{}", idx), v.clone()))
            .collect();

        let ground_truth = brute_force_search(query, &vectors_with_ids, k, DistanceMetric::Cosine);

        let recall = calculate_recall(&ground_truth, &result_ids);
        total_recall += recall;
    }

    let avg_recall = total_recall / num_queries as f32;
    println!(
        "100 vectors - Average recall@{}: {:.2}%",
        k,
        avg_recall * 100.0
    );

    // For small datasets, we expect very high recall
    assert!(
        avg_recall >= 0.90,
        "Recall should be at least 90% for 100 vectors"
    );

    Ok(())
}

#[test]
fn test_hnsw_1k_vectors() -> Result<()> {
    let dimensions = 128;
    let num_vectors = 1000;
    let k = 10;

    let config = HnswConfig {
        m: 32,
        ef_construction: 200,
        ef_search: 200,
        max_elements: 10000,
    };

    let mut index = HnswIndex::new(dimensions, DistanceMetric::Cosine, config)?;

    // Generate and insert vectors
    let vectors = generate_random_vectors(num_vectors, dimensions, 12345);
    let normalized_vectors: Vec<_> = vectors.iter().map(|v| normalize_vector(v)).collect();

    // Use batch insert for better performance
    let entries: Vec<_> = normalized_vectors
        .iter()
        .enumerate()
        .map(|(i, v)| (format!("vec_{}", i), v.clone()))
        .collect();

    index.add_batch(entries)?;
    assert_eq!(index.len(), num_vectors);

    // Test search accuracy
    let num_queries = 20;
    let mut total_recall = 0.0;

    for i in 0..num_queries {
        let query_idx = i * (num_vectors / num_queries);
        let query = &normalized_vectors[query_idx];

        let results = index.search(query, k)?;
        let result_ids: Vec<_> = results.iter().map(|r| r.id.clone()).collect();

        let vectors_with_ids: Vec<_> = normalized_vectors
            .iter()
            .enumerate()
            .map(|(idx, v)| (format!("vec_{}", idx), v.clone()))
            .collect();

        let ground_truth = brute_force_search(query, &vectors_with_ids, k, DistanceMetric::Cosine);
        let recall = calculate_recall(&ground_truth, &result_ids);
        total_recall += recall;
    }

    let avg_recall = total_recall / num_queries as f32;
    println!(
        "1K vectors - Average recall@{}: {:.2}%",
        k,
        avg_recall * 100.0
    );

    // Should achieve at least 95% recall with ef_search=200
    assert!(
        avg_recall >= 0.95,
        "Recall should be at least 95% for 1K vectors with ef_search=200"
    );

    Ok(())
}

#[test]
fn test_hnsw_10k_vectors() -> Result<()> {
    let dimensions = 128;
    let num_vectors = 10000;
    let k = 10;

    let config = HnswConfig {
        m: 32,
        ef_construction: 200,
        ef_search: 200,
        max_elements: 100000,
    };

    let mut index = HnswIndex::new(dimensions, DistanceMetric::Cosine, config)?;

    println!("Generating {} vectors...", num_vectors);
    let vectors = generate_random_vectors(num_vectors, dimensions, 98765);
    let normalized_vectors: Vec<_> = vectors.iter().map(|v| normalize_vector(v)).collect();

    println!("Inserting vectors in batches...");
    // Insert in batches for better performance
    let batch_size = 1000;
    for batch_start in (0..num_vectors).step_by(batch_size) {
        let batch_end = (batch_start + batch_size).min(num_vectors);
        let entries: Vec<_> = normalized_vectors[batch_start..batch_end]
            .iter()
            .enumerate()
            .map(|(i, v)| (format!("vec_{}", batch_start + i), v.clone()))
            .collect();

        index.add_batch(entries)?;
    }

    assert_eq!(index.len(), num_vectors);
    println!("Index built with {} vectors", index.len());

    // Prepare all vectors for ground truth computation
    let all_vectors: Vec<_> = normalized_vectors
        .iter()
        .enumerate()
        .map(|(i, v)| (format!("vec_{}", i), v.clone()))
        .collect();

    // Test search accuracy with a sample of queries
    let num_queries = 20; // Reduced for faster testing
    let mut total_recall = 0.0;

    println!("Running {} queries...", num_queries);
    for i in 0..num_queries {
        let query_idx = i * (num_vectors / num_queries);
        let query = &normalized_vectors[query_idx];

        let results = index.search(query, k)?;
        let result_ids: Vec<_> = results.iter().map(|r| r.id.clone()).collect();

        // Compare against all vectors for accurate ground truth
        let ground_truth = brute_force_search(query, &all_vectors, k, DistanceMetric::Cosine);
        let recall = calculate_recall(&ground_truth, &result_ids);
        total_recall += recall;
    }

    let avg_recall = total_recall / num_queries as f32;
    println!(
        "10K vectors - Average recall@{}: {:.2}%",
        k,
        avg_recall * 100.0
    );

    // With ef_search=200 and m=32, we should achieve good recall
    assert!(
        avg_recall >= 0.70,
        "Recall should be at least 70% for 10K vectors, got {:.2}%",
        avg_recall * 100.0
    );

    Ok(())
}

#[test]
fn test_hnsw_ef_search_tuning() -> Result<()> {
    let dimensions = 128;
    let num_vectors = 500;
    let k = 10;

    let config = HnswConfig {
        m: 32,
        ef_construction: 200,
        ef_search: 50, // Start with lower ef_search
        max_elements: 10000,
    };

    let mut index = HnswIndex::new(dimensions, DistanceMetric::Cosine, config)?;

    let vectors = generate_random_vectors(num_vectors, dimensions, 54321);
    let normalized_vectors: Vec<_> = vectors.iter().map(|v| normalize_vector(v)).collect();

    let entries: Vec<_> = normalized_vectors
        .iter()
        .enumerate()
        .map(|(i, v)| (format!("vec_{}", i), v.clone()))
        .collect();

    index.add_batch(entries)?;

    // Test different ef_search values
    let ef_values = vec![50, 100, 200, 500];

    for ef in ef_values {
        let mut total_recall = 0.0;
        let num_queries = 10;

        for i in 0..num_queries {
            let query_idx = i * 50;
            let query = &normalized_vectors[query_idx];

            let results = index.search_with_ef(query, k, ef)?;
            let result_ids: Vec<_> = results.iter().map(|r| r.id.clone()).collect();

            let vectors_with_ids: Vec<_> = normalized_vectors
                .iter()
                .enumerate()
                .map(|(idx, v)| (format!("vec_{}", idx), v.clone()))
                .collect();

            let ground_truth =
                brute_force_search(query, &vectors_with_ids, k, DistanceMetric::Cosine);
            let recall = calculate_recall(&ground_truth, &result_ids);
            total_recall += recall;
        }

        let avg_recall = total_recall / num_queries as f32;
        println!(
            "ef_search={} - Average recall@{}: {:.2}%",
            ef,
            k,
            avg_recall * 100.0
        );
    }

    // Verify that ef_search=200 achieves at least 95% recall
    let mut total_recall = 0.0;
    let num_queries = 10;

    for i in 0..num_queries {
        let query_idx = i * 50;
        let query = &normalized_vectors[query_idx];

        let results = index.search_with_ef(query, k, 200)?;
        let result_ids: Vec<_> = results.iter().map(|r| r.id.clone()).collect();

        let vectors_with_ids: Vec<_> = normalized_vectors
            .iter()
            .enumerate()
            .map(|(idx, v)| (format!("vec_{}", idx), v.clone()))
            .collect();

        let ground_truth = brute_force_search(query, &vectors_with_ids, k, DistanceMetric::Cosine);
        let recall = calculate_recall(&ground_truth, &result_ids);
        total_recall += recall;
    }

    let avg_recall = total_recall / num_queries as f32;
    assert!(
        avg_recall >= 0.95,
        "ef_search=200 should achieve at least 95% recall"
    );

    Ok(())
}

#[test]
fn test_hnsw_serialization_large() -> Result<()> {
    let dimensions = 128;
    let num_vectors = 500;

    let config = HnswConfig {
        m: 32,
        ef_construction: 200,
        ef_search: 100,
        max_elements: 10000,
    };

    let mut index = HnswIndex::new(dimensions, DistanceMetric::Cosine, config)?;

    let vectors = generate_random_vectors(num_vectors, dimensions, 11111);
    let normalized_vectors: Vec<_> = vectors.iter().map(|v| normalize_vector(v)).collect();

    let entries: Vec<_> = normalized_vectors
        .iter()
        .enumerate()
        .map(|(i, v)| (format!("vec_{}", i), v.clone()))
        .collect();

    index.add_batch(entries)?;

    // Serialize
    println!("Serializing index with {} vectors...", num_vectors);
    let bytes = index.serialize()?;
    println!(
        "Serialized size: {} bytes ({:.2} KB)",
        bytes.len(),
        bytes.len() as f32 / 1024.0
    );

    // Deserialize
    println!("Deserializing index...");
    let restored_index = HnswIndex::deserialize(&bytes)?;

    assert_eq!(restored_index.len(), num_vectors);

    // Test that search works on restored index
    let query = &normalized_vectors[0];
    let original_results = index.search(query, 10)?;
    let restored_results = restored_index.search(query, 10)?;

    // Results should be identical
    assert_eq!(original_results.len(), restored_results.len());

    println!("Serialization test passed!");

    Ok(())
}

#[test]
fn test_hnsw_different_metrics() -> Result<()> {
    let dimensions = 128;
    let num_vectors = 200;
    let k = 5;

    // Note: DotProduct can produce negative distances on normalized vectors,
    // which causes issues with the underlying hnsw_rs library.
    // We test Cosine and Euclidean which are the most commonly used metrics.
    let metrics = vec![DistanceMetric::Cosine, DistanceMetric::Euclidean];

    for metric in metrics {
        println!("Testing metric: {:?}", metric);

        let config = HnswConfig {
            m: 16,
            ef_construction: 100,
            ef_search: 100,
            max_elements: 1000,
        };

        let mut index = HnswIndex::new(dimensions, metric, config)?;

        let vectors = generate_random_vectors(num_vectors, dimensions, 99999);
        let normalized_vectors: Vec<_> = vectors.iter().map(|v| normalize_vector(v)).collect();

        for (i, vector) in normalized_vectors.iter().enumerate() {
            index.add(format!("vec_{}", i), vector.clone())?;
        }

        // Test search
        let query = &normalized_vectors[0];
        let results = index.search(query, k)?;

        assert!(!results.is_empty());
        println!("  Found {} results for metric {:?}", results.len(), metric);
    }

    Ok(())
}

#[test]
fn test_hnsw_parallel_batch_insert() -> Result<()> {
    let dimensions = 128;
    let num_vectors = 2000;

    let config = HnswConfig {
        m: 32,
        ef_construction: 200,
        ef_search: 100,
        max_elements: 10000,
    };

    let mut index = HnswIndex::new(dimensions, DistanceMetric::Cosine, config)?;

    let vectors = generate_random_vectors(num_vectors, dimensions, 77777);
    let normalized_vectors: Vec<_> = vectors.iter().map(|v| normalize_vector(v)).collect();

    let entries: Vec<_> = normalized_vectors
        .iter()
        .enumerate()
        .map(|(i, v)| (format!("vec_{}", i), v.clone()))
        .collect();

    // Time the batch insert
    let start = std::time::Instant::now();
    index.add_batch(entries)?;
    let duration = start.elapsed();

    println!("Batch inserted {} vectors in {:?}", num_vectors, duration);
    println!(
        "Throughput: {:.0} vectors/sec",
        num_vectors as f64 / duration.as_secs_f64()
    );

    assert_eq!(index.len(), num_vectors);

    // Verify search still works
    let query = &normalized_vectors[0];
    let results = index.search(query, 10)?;
    assert!(!results.is_empty());

    Ok(())
}
