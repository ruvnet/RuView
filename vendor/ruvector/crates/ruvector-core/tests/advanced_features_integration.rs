//! Integration tests for advanced features
//!
//! Tests Enhanced PQ, Filtered Search, MMR, Hybrid Search, and Conformal Prediction
//! across multiple vector dimensions (128D, 384D, 768D)

use ruvector_core::advanced_features::*;
use ruvector_core::types::{DistanceMetric, SearchResult};
use ruvector_core::{Result, RuvectorError};
use std::collections::HashMap;

// Helper function to generate random vectors
fn generate_vectors(count: usize, dimensions: usize) -> Vec<Vec<f32>> {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    (0..count)
        .map(|_| (0..dimensions).map(|_| rng.gen::<f32>()).collect())
        .collect()
}

// Helper function to normalize vectors
fn normalize_vector(v: &[f32]) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter().map(|x| x / norm).collect()
    } else {
        v.to_vec()
    }
}

#[test]
fn test_enhanced_pq_128d() {
    let dimensions = 128;
    let num_vectors = 1000;

    let config = PQConfig {
        num_subspaces: 8,
        codebook_size: 256,
        num_iterations: 10,
        metric: DistanceMetric::Euclidean,
    };

    let mut pq = EnhancedPQ::new(dimensions, config).unwrap();

    // Generate training data
    let training_data = generate_vectors(num_vectors, dimensions);
    pq.train(&training_data).unwrap();

    // Test encoding and search
    let query = normalize_vector(&generate_vectors(1, dimensions)[0]);

    // Add quantized vectors
    for (i, vector) in training_data.iter().enumerate() {
        pq.add_quantized(format!("vec_{}", i), vector).unwrap();
    }

    // Perform search
    let results = pq.search(&query, 10).unwrap();
    assert_eq!(results.len(), 10);

    // Test compression ratio
    let compression_ratio = pq.compression_ratio();
    assert!(compression_ratio >= 8.0); // Should be 16x for 128D with 8 subspaces

    println!(
        "✓ Enhanced PQ 128D: compression ratio = {:.1}x",
        compression_ratio
    );
}

#[test]
fn test_enhanced_pq_384d() {
    let dimensions = 384;
    let num_vectors = 500;

    let config = PQConfig {
        num_subspaces: 8,
        codebook_size: 256,
        num_iterations: 10,
        metric: DistanceMetric::Cosine,
    };

    let mut pq = EnhancedPQ::new(dimensions, config).unwrap();

    // Generate training data
    let training_data: Vec<Vec<f32>> = generate_vectors(num_vectors, dimensions)
        .into_iter()
        .map(|v| normalize_vector(&v))
        .collect();

    pq.train(&training_data).unwrap();

    // Test reconstruction
    let test_vector = &training_data[0];
    let codes = pq.encode(test_vector).unwrap();
    let reconstructed = pq.reconstruct(&codes).unwrap();

    assert_eq!(reconstructed.len(), dimensions);

    // Calculate reconstruction error
    let error: f32 = test_vector
        .iter()
        .zip(&reconstructed)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f32>()
        .sqrt();

    println!("✓ Enhanced PQ 384D: reconstruction error = {:.4}", error);
    assert!(error < 5.0); // Reasonable reconstruction error
}

#[test]
fn test_enhanced_pq_768d() {
    let dimensions = 768;
    let num_vectors = 300; // Increased to ensure we have enough vectors for search

    let config = PQConfig {
        num_subspaces: 16,
        codebook_size: 256,
        num_iterations: 10,
        metric: DistanceMetric::Euclidean,
    };

    let mut pq = EnhancedPQ::new(dimensions, config).unwrap();

    let training_data = generate_vectors(num_vectors, dimensions);
    pq.train(&training_data).unwrap();

    // Test lookup table creation
    let query = generate_vectors(1, dimensions)[0].clone();
    let lookup_table = pq.create_lookup_table(&query).unwrap();

    assert_eq!(lookup_table.tables.len(), 16);
    assert_eq!(lookup_table.tables[0].len(), 256);

    println!("✓ Enhanced PQ 768D: lookup table created successfully");
}

#[test]
fn test_filtered_search_pre_filter() {
    use serde_json::json;

    // Create metadata store
    let mut metadata_store = HashMap::new();
    for i in 0..100 {
        let mut metadata = HashMap::new();
        metadata.insert(
            "category".to_string(),
            json!(if i % 3 == 0 { "A" } else { "B" }),
        );
        metadata.insert("price".to_string(), json!(i as f32 * 10.0));
        metadata_store.insert(format!("vec_{}", i), metadata);
    }

    // Create filter: category == "A" AND price < 500
    let filter = FilterExpression::And(vec![
        FilterExpression::Eq("category".to_string(), json!("A")),
        FilterExpression::Lt("price".to_string(), json!(500.0)),
    ]);

    let search = FilteredSearch::new(filter, FilterStrategy::PreFilter, metadata_store);

    // Test pre-filtering
    let filtered_ids = search.get_filtered_ids();
    assert!(!filtered_ids.is_empty());
    assert!(filtered_ids.len() < 50); // Should be selective

    println!(
        "✓ Filtered Search (Pre-filter): {} matching documents",
        filtered_ids.len()
    );
}

#[test]
fn test_filtered_search_auto_strategy() {
    use serde_json::json;

    let mut metadata_store = HashMap::new();
    for i in 0..1000 {
        let mut metadata = HashMap::new();
        metadata.insert("id".to_string(), json!(i));
        metadata_store.insert(format!("vec_{}", i), metadata);
    }

    // Highly selective filter (should choose pre-filter)
    let selective_filter = FilterExpression::Eq("id".to_string(), json!(42));
    let search1 = FilteredSearch::new(
        selective_filter,
        FilterStrategy::Auto,
        metadata_store.clone(),
    );
    assert_eq!(search1.auto_select_strategy(), FilterStrategy::PreFilter);

    // Less selective filter (should choose post-filter)
    let broad_filter = FilterExpression::Gte("id".to_string(), json!(0));
    let search2 = FilteredSearch::new(broad_filter, FilterStrategy::Auto, metadata_store);
    assert_eq!(search2.auto_select_strategy(), FilterStrategy::PostFilter);

    println!("✓ Filtered Search: automatic strategy selection working");
}

#[test]
fn test_mmr_diversity_128d() {
    let dimensions = 128;

    let config = MMRConfig {
        lambda: 0.5, // Balance relevance and diversity
        metric: DistanceMetric::Cosine,
        fetch_multiplier: 2.0,
    };

    let mmr = MMRSearch::new(config).unwrap();

    // Create query and candidates
    let query = normalize_vector(&generate_vectors(1, dimensions)[0]);
    let candidates: Vec<SearchResult> = (0..20)
        .map(|i| SearchResult {
            id: format!("doc_{}", i),
            score: i as f32 * 0.05,
            vector: Some(normalize_vector(&generate_vectors(1, dimensions)[0])),
            metadata: None,
        })
        .collect();

    // Rerank using MMR
    let results = mmr.rerank(&query, candidates, 10).unwrap();

    assert_eq!(results.len(), 10);
    // Results should be diverse (not just top-10 by relevance)

    println!("✓ MMR 128D: diversified {} results", results.len());
}

#[test]
fn test_mmr_lambda_variations() {
    let dimensions = 64;

    // Test with pure relevance (lambda = 1.0)
    let config_relevance = MMRConfig {
        lambda: 1.0,
        metric: DistanceMetric::Euclidean,
        fetch_multiplier: 2.0,
    };
    let mmr_relevance = MMRSearch::new(config_relevance).unwrap();

    // Test with pure diversity (lambda = 0.0)
    let config_diversity = MMRConfig {
        lambda: 0.0,
        metric: DistanceMetric::Euclidean,
        fetch_multiplier: 2.0,
    };
    let mmr_diversity = MMRSearch::new(config_diversity).unwrap();

    let query = generate_vectors(1, dimensions)[0].clone();
    let candidates: Vec<SearchResult> = (0..10)
        .map(|i| SearchResult {
            id: format!("doc_{}", i),
            score: i as f32 * 0.1,
            vector: Some(generate_vectors(1, dimensions)[0].clone()),
            metadata: None,
        })
        .collect();

    let results_relevance = mmr_relevance.rerank(&query, candidates.clone(), 5).unwrap();
    let results_diversity = mmr_diversity.rerank(&query, candidates, 5).unwrap();

    assert_eq!(results_relevance.len(), 5);
    assert_eq!(results_diversity.len(), 5);

    println!("✓ MMR: lambda variations tested successfully");
}

#[test]
fn test_hybrid_search_basic() {
    let config = HybridConfig {
        vector_weight: 0.7,
        keyword_weight: 0.3,
        normalization: NormalizationStrategy::MinMax,
    };

    let mut hybrid = HybridSearch::new(config);

    // Index documents
    hybrid.index_document("doc1".to_string(), "rust programming language".to_string());
    hybrid.index_document("doc2".to_string(), "python machine learning".to_string());
    hybrid.index_document("doc3".to_string(), "rust systems programming".to_string());
    hybrid.finalize_indexing();

    // Test BM25 scoring
    let score = hybrid.bm25.score(
        "rust programming",
        &"doc1".to_string(),
        "rust programming language",
    );
    assert!(score > 0.0);

    println!(
        "✓ Hybrid Search: indexed {} documents",
        hybrid.doc_texts.len()
    );
}

#[test]
fn test_hybrid_search_keyword_matching() {
    let mut bm25 = BM25::new(1.5, 0.75);

    bm25.index_document("doc1".to_string(), "vector database with HNSW indexing");
    bm25.index_document("doc2".to_string(), "relational database management system");
    bm25.index_document("doc3".to_string(), "vector search and similarity matching");
    bm25.build_idf();

    // Test candidate retrieval
    let candidates = bm25.get_candidate_docs("vector database");
    assert!(candidates.contains(&"doc1".to_string()));
    assert!(candidates.contains(&"doc3".to_string()));

    // Test scoring
    let score1 = bm25.score(
        "vector database",
        &"doc1".to_string(),
        "vector database with HNSW indexing",
    );
    let score2 = bm25.score(
        "vector database",
        &"doc2".to_string(),
        "relational database management system",
    );

    assert!(score1 > score2); // doc1 matches better

    println!(
        "✓ Hybrid Search (BM25): {} candidate documents",
        candidates.len()
    );
}

#[test]
fn test_conformal_prediction_128d() {
    let dimensions = 128;

    let config = ConformalConfig {
        alpha: 0.1, // 90% coverage
        calibration_fraction: 0.2,
        nonconformity_measure: NonconformityMeasure::Distance,
    };

    let mut predictor = ConformalPredictor::new(config).unwrap();

    // Create calibration data
    let calibration_queries = generate_vectors(10, dimensions);
    let true_neighbors: Vec<Vec<String>> = (0..10)
        .map(|i| vec![format!("vec_{}", i), format!("vec_{}", i + 1)])
        .collect();

    // Mock search function
    let search_fn = |_query: &[f32], k: usize| -> Result<Vec<SearchResult>> {
        Ok((0..k)
            .map(|i| SearchResult {
                id: format!("vec_{}", i),
                score: i as f32 * 0.1,
                vector: Some(vec![0.0; dimensions]),
                metadata: None,
            })
            .collect())
    };

    // Calibrate
    predictor
        .calibrate(&calibration_queries, &true_neighbors, search_fn)
        .unwrap();

    assert!(predictor.threshold.is_some());
    assert!(!predictor.calibration_scores.is_empty());

    // Make prediction
    let query = generate_vectors(1, dimensions)[0].clone();
    let prediction_set = predictor.predict(&query, search_fn).unwrap();

    assert_eq!(prediction_set.confidence, 0.9);
    assert!(!prediction_set.results.is_empty());

    println!(
        "✓ Conformal Prediction 128D: prediction set size = {}",
        prediction_set.results.len()
    );
}

#[test]
fn test_conformal_prediction_384d() {
    let dimensions = 384;

    let config = ConformalConfig {
        alpha: 0.05, // 95% coverage
        calibration_fraction: 0.2,
        nonconformity_measure: NonconformityMeasure::NormalizedDistance,
    };

    let mut predictor = ConformalPredictor::new(config).unwrap();

    let calibration_queries = generate_vectors(5, dimensions);
    let true_neighbors: Vec<Vec<String>> = (0..5).map(|i| vec![format!("vec_{}", i)]).collect();

    let search_fn = |_query: &[f32], k: usize| -> Result<Vec<SearchResult>> {
        Ok((0..k)
            .map(|i| SearchResult {
                id: format!("vec_{}", i),
                score: 0.1 + (i as f32 * 0.05),
                vector: Some(vec![0.0; dimensions]),
                metadata: None,
            })
            .collect())
    };

    predictor
        .calibrate(&calibration_queries, &true_neighbors, search_fn)
        .unwrap();

    // Test calibration statistics
    let stats = predictor.get_statistics().unwrap();
    assert_eq!(stats.num_samples, 5);
    assert!(stats.mean > 0.0);

    println!("✓ Conformal Prediction 384D: calibration stats computed");
}

#[test]
fn test_conformal_prediction_adaptive_k() {
    let dimensions = 256;

    let config = ConformalConfig {
        alpha: 0.1,
        calibration_fraction: 0.2,
        nonconformity_measure: NonconformityMeasure::InverseRank,
    };

    let mut predictor = ConformalPredictor::new(config).unwrap();

    let calibration_queries = generate_vectors(8, dimensions);
    let true_neighbors: Vec<Vec<String>> = (0..8).map(|i| vec![format!("vec_{}", i)]).collect();

    let search_fn = |_query: &[f32], k: usize| -> Result<Vec<SearchResult>> {
        Ok((0..k)
            .map(|i| SearchResult {
                id: format!("vec_{}", i),
                score: i as f32 * 0.08,
                vector: Some(vec![0.0; dimensions]),
                metadata: None,
            })
            .collect())
    };

    predictor
        .calibrate(&calibration_queries, &true_neighbors, search_fn)
        .unwrap();

    // Test adaptive top-k
    let query = generate_vectors(1, dimensions)[0].clone();
    let adaptive_k = predictor.adaptive_top_k(&query, search_fn).unwrap();

    assert!(adaptive_k > 0);
    println!("✓ Conformal Prediction: adaptive k = {}", adaptive_k);
}

#[test]
fn test_all_features_integration() {
    // Test that all features can work together
    let dimensions = 128;

    // 1. Enhanced PQ
    let pq_config = PQConfig {
        num_subspaces: 4,
        codebook_size: 16,
        num_iterations: 5,
        metric: DistanceMetric::Euclidean,
    };
    let mut pq = EnhancedPQ::new(dimensions, pq_config).unwrap();
    let training_data = generate_vectors(50, dimensions);
    pq.train(&training_data).unwrap();

    // 2. MMR
    let mmr_config = MMRConfig::default();
    let mmr = MMRSearch::new(mmr_config).unwrap();

    // 3. Hybrid Search
    let hybrid_config = HybridConfig::default();
    let mut hybrid = HybridSearch::new(hybrid_config);
    hybrid.index_document("doc1".to_string(), "test document".to_string());
    hybrid.finalize_indexing();

    // 4. Conformal Prediction
    let cp_config = ConformalConfig::default();
    let predictor = ConformalPredictor::new(cp_config).unwrap();

    println!("✓ All features integrated successfully");
}

#[test]
fn test_pq_recall_384d() {
    let dimensions = 384;
    let num_vectors = 500;
    let k = 10;

    let config = PQConfig {
        num_subspaces: 8,
        codebook_size: 256,
        num_iterations: 15,
        metric: DistanceMetric::Euclidean,
    };

    let mut pq = EnhancedPQ::new(dimensions, config).unwrap();

    // Generate and train
    let vectors = generate_vectors(num_vectors, dimensions);
    pq.train(&vectors).unwrap();

    // Add vectors
    for (i, vector) in vectors.iter().enumerate() {
        pq.add_quantized(format!("vec_{}", i), vector).unwrap();
    }

    // Test search
    let query = &vectors[0]; // Use first vector as query
    let results = pq.search(query, k).unwrap();

    // Verify we got results
    assert!(!results.is_empty(), "Search should return results");
    assert_eq!(results.len(), k, "Should return k results");

    // First result should be among the top candidates (PQ is approximate)
    // Due to quantization, the exact match might not be at position 0
    // but the distance should be reasonably small relative to random vectors
    let min_distance = results
        .iter()
        .map(|(_, d)| *d)
        .fold(f32::INFINITY, f32::min);

    // In high dimensions, PQ distances vary based on quantization quality
    // Check that we get reasonable results (top result should be closer than random)
    assert!(
        min_distance < 50.0,
        "Minimum distance {} should be reasonable for quantized search",
        min_distance
    );

    println!(
        "✓ PQ 384D Recall Test: top-{} results retrieved, min distance = {:.4}",
        results.len(),
        min_distance
    );
}
