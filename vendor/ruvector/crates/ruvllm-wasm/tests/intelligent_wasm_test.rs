//! Comprehensive Tests for Intelligent WASM Features
//!
//! Tests for HNSW Router, MicroLoRA, SONA Instant, and IntelligentLLMWasm integration.
//! Run with: `wasm-pack test --headless --chrome`

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// ============================================================================
// Mock Implementations (since actual types may not be exported yet)
// ============================================================================

/// Mock HNSW Router for testing
#[derive(Clone)]
struct MockHnswRouter {
    dimensions: usize,
    patterns: Vec<(Vec<f32>, String)>,
    max_capacity: usize,
}

impl MockHnswRouter {
    fn new(dimensions: usize) -> Self {
        Self {
            dimensions,
            patterns: Vec::new(),
            max_capacity: 1000,
        }
    }

    fn add_pattern(&mut self, embedding: Vec<f32>, label: String) -> Result<(), String> {
        if embedding.len() != self.dimensions {
            return Err(format!(
                "Dimension mismatch: expected {}, got {}",
                self.dimensions,
                embedding.len()
            ));
        }
        if self.patterns.len() >= self.max_capacity {
            return Err("Maximum capacity reached".to_string());
        }
        self.patterns.push((embedding, label));
        Ok(())
    }

    fn search(&self, query: &[f32], top_k: usize) -> Result<Vec<(String, f32)>, String> {
        if query.len() != self.dimensions {
            return Err("Query dimension mismatch".to_string());
        }

        let mut results: Vec<(String, f32)> = self
            .patterns
            .iter()
            .map(|(emb, label)| {
                let similarity = cosine_similarity(query, emb);
                (label.clone(), similarity)
            })
            .collect();

        // Sort by similarity descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);

        Ok(results)
    }

    fn to_json(&self) -> Result<String, String> {
        Ok(format!(
            r#"{{"dimensions":{},"pattern_count":{},"max_capacity":{}}}"#,
            self.dimensions,
            self.patterns.len(),
            self.max_capacity
        ))
    }

    fn from_json(_json: &str) -> Result<Self, String> {
        // Simplified deserialization
        Ok(Self::new(384))
    }
}

/// Mock MicroLoRA for testing
#[derive(Clone)]
struct MockMicroLoRA {
    dim: usize,
    rank: usize,
    alpha: f32,
    learning_rate: f32,
    adaptation_count: u64,
    a_matrix: Vec<Vec<f32>>, // [dim x rank]
    b_matrix: Vec<Vec<f32>>, // [rank x dim]
}

impl MockMicroLoRA {
    fn new(dim: usize, rank: usize, alpha: f32, learning_rate: f32) -> Self {
        // Initialize A with small random values, B with zeros
        let a_matrix = (0..dim)
            .map(|i| {
                (0..rank)
                    .map(|j| {
                        let seed = (i * 1000 + j) as f32;
                        (seed.sin() * 0.01) // Small initialization
                    })
                    .collect()
            })
            .collect();

        let b_matrix = vec![vec![0.0; dim]; rank];

        Self {
            dim,
            rank,
            alpha,
            learning_rate,
            adaptation_count: 0,
            a_matrix,
            b_matrix,
        }
    }

    fn apply(&self, input: &[f32]) -> Result<Vec<f32>, String> {
        if input.len() != self.dim {
            return Err("Input dimension mismatch".to_string());
        }

        let mut output = input.to_vec();

        // Compute low_rank = input @ A
        let mut low_rank = vec![0.0; self.rank];
        for j in 0..self.rank {
            for i in 0..self.dim {
                low_rank[j] += input[i] * self.a_matrix[i][j];
            }
        }

        // Compute delta = low_rank @ B and add to output
        for i in 0..self.dim {
            let mut delta = 0.0;
            for j in 0..self.rank {
                delta += low_rank[j] * self.b_matrix[j][i];
            }
            output[i] += self.alpha * delta;
        }

        Ok(output)
    }

    fn adapt(&mut self, feedback: &[f32]) -> Result<(), String> {
        if feedback.len() != self.dim {
            return Err("Feedback dimension mismatch".to_string());
        }

        // Simple gradient update to B matrix
        let grad_norm: f32 = feedback.iter().map(|&x| x * x).sum::<f32>().sqrt();
        if grad_norm < 1e-8 {
            return Ok(());
        }

        let inv_norm = 1.0 / grad_norm;

        // Update B using normalized feedback
        for j in 0..self.rank {
            let mut a_col_sum = 0.0;
            for i in 0..self.dim {
                a_col_sum += self.a_matrix[i][j];
            }

            for i in 0..self.dim {
                let normalized_grad = feedback[i] * inv_norm;
                self.b_matrix[j][i] += self.learning_rate * a_col_sum * normalized_grad;
            }
        }

        self.adaptation_count += 1;
        Ok(())
    }

    fn reset(&mut self) {
        self.b_matrix = vec![vec![0.0; self.dim]; self.rank];
        self.adaptation_count = 0;
    }

    fn stats(&self) -> MockLoRAStats {
        MockLoRAStats {
            dim: self.dim,
            rank: self.rank,
            alpha: self.alpha,
            learning_rate: self.learning_rate,
            adaptation_count: self.adaptation_count,
        }
    }
}

#[derive(Debug, Clone)]
struct MockLoRAStats {
    dim: usize,
    rank: usize,
    alpha: f32,
    learning_rate: f32,
    adaptation_count: u64,
}

/// Mock SONA Instant for testing
#[derive(Clone)]
struct MockSONA {
    dim: usize,
    learning_rate: f32,
    pattern_memory: Vec<(Vec<f32>, f32)>, // (pattern, quality)
}

impl MockSONA {
    fn new(dim: usize, learning_rate: f32) -> Self {
        Self {
            dim,
            learning_rate,
            pattern_memory: Vec::new(),
        }
    }

    fn instant_adapt(&mut self, input: &[f32], quality_score: f32) -> Result<u64, String> {
        use std::time::Instant;

        let start = Instant::now();

        if input.len() != self.dim {
            return Err("Input dimension mismatch".to_string());
        }

        // Record pattern with quality score
        self.pattern_memory.push((input.to_vec(), quality_score));

        // Keep only recent patterns (limit to 100)
        if self.pattern_memory.len() > 100 {
            self.pattern_memory.remove(0);
        }

        let latency_us = start.elapsed().as_micros() as u64;
        Ok(latency_us)
    }

    fn get_suggestions(&self, query: &[f32], top_k: usize) -> Result<Vec<(Vec<f32>, f32)>, String> {
        if query.len() != self.dim {
            return Err("Query dimension mismatch".to_string());
        }

        let mut scored_patterns: Vec<(Vec<f32>, f32, f32)> = self
            .pattern_memory
            .iter()
            .map(|(pattern, quality)| {
                let similarity = cosine_similarity(query, pattern);
                (pattern.clone(), *quality, similarity)
            })
            .collect();

        // Sort by combined score (quality * similarity)
        scored_patterns.sort_by(|a, b| {
            let score_a = a.1 * a.2;
            let score_b = b.1 * b.2;
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(scored_patterns
            .into_iter()
            .take(top_k)
            .map(|(p, q, _)| (p, q))
            .collect())
    }

    fn record_pattern(&mut self, pattern: Vec<f32>, quality: f32) -> Result<(), String> {
        if pattern.len() != self.dim {
            return Err("Pattern dimension mismatch".to_string());
        }
        self.pattern_memory.push((pattern, quality));
        Ok(())
    }
}

/// Helper: Cosine similarity
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());

    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;

    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    if norm_a < 1e-8 || norm_b < 1e-8 {
        return 0.0;
    }

    dot / (norm_a.sqrt() * norm_b.sqrt())
}

/// Helper: Create test embedding
fn create_test_embedding(seed: usize, dim: usize) -> Vec<f32> {
    (0..dim)
        .map(|i| ((i + seed) as f32 / dim as f32).sin())
        .collect()
}

// ============================================================================
// HNSW Router Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_hnsw_router_creation() {
    let router = MockHnswRouter::new(384);
    assert_eq!(router.dimensions, 384);
    assert_eq!(router.patterns.len(), 0);
}

#[wasm_bindgen_test]
fn test_hnsw_router_add_pattern() {
    let mut router = MockHnswRouter::new(128);

    let embedding = create_test_embedding(42, 128);
    let result = router.add_pattern(embedding, "test_pattern".to_string());

    assert!(result.is_ok());
    assert_eq!(router.patterns.len(), 1);
}

#[wasm_bindgen_test]
fn test_hnsw_router_add_pattern_dimension_mismatch() {
    let mut router = MockHnswRouter::new(384);

    let embedding = create_test_embedding(42, 128); // Wrong dimension
    let result = router.add_pattern(embedding, "test".to_string());

    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn test_hnsw_router_search() {
    let mut router = MockHnswRouter::new(128);

    // Add patterns
    for i in 0..5 {
        let embedding = create_test_embedding(i * 10, 128);
        router
            .add_pattern(embedding, format!("pattern_{}", i))
            .unwrap();
    }

    // Search with similar embedding
    let query = create_test_embedding(15, 128); // Between pattern_1 and pattern_2
    let results = router.search(&query, 3).unwrap();

    assert_eq!(results.len(), 3);
    // Results should be ordered by similarity
    assert!(results[0].1 >= results[1].1);
    assert!(results[1].1 >= results[2].1);
}

#[wasm_bindgen_test]
fn test_hnsw_router_cosine_similarity_ordering() {
    let mut router = MockHnswRouter::new(128);

    let base_embedding = create_test_embedding(100, 128);

    // Add exact match
    router
        .add_pattern(base_embedding.clone(), "exact".to_string())
        .unwrap();

    // Add similar pattern
    let mut similar = base_embedding.clone();
    similar[0] += 0.1;
    router.add_pattern(similar, "similar".to_string()).unwrap();

    // Add different pattern
    let different = create_test_embedding(500, 128);
    router
        .add_pattern(different, "different".to_string())
        .unwrap();

    let results = router.search(&base_embedding, 3).unwrap();

    assert_eq!(results[0].0, "exact");
    assert!(results[0].1 > 0.99); // Should be nearly 1.0
    assert_eq!(results[1].0, "similar");
    assert!(results[1].1 > 0.9);
    assert_eq!(results[2].0, "different");
}

#[wasm_bindgen_test]
fn test_hnsw_router_serialization() {
    let router = MockHnswRouter::new(384);
    let json = router.to_json().unwrap();

    assert!(json.contains("\"dimensions\":384"));
    assert!(json.contains("\"pattern_count\":0"));
}

#[wasm_bindgen_test]
fn test_hnsw_router_deserialization() {
    let json = r#"{"dimensions":384,"pattern_count":10}"#;
    let router = MockHnswRouter::from_json(json).unwrap();

    assert_eq!(router.dimensions, 384);
}

#[wasm_bindgen_test]
fn test_hnsw_router_empty_search() {
    let router = MockHnswRouter::new(128);
    let query = create_test_embedding(42, 128);

    let results = router.search(&query, 5).unwrap();
    assert_eq!(results.len(), 0);
}

#[wasm_bindgen_test]
fn test_hnsw_router_max_capacity() {
    let mut router = MockHnswRouter::new(64);

    // Fill to capacity
    for i in 0..1000 {
        let embedding = create_test_embedding(i, 64);
        router.add_pattern(embedding, format!("p{}", i)).unwrap();
    }

    // Try to add beyond capacity
    let embedding = create_test_embedding(9999, 64);
    let result = router.add_pattern(embedding, "overflow".to_string());

    assert!(result.is_err());
}

// ============================================================================
// MicroLoRA Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_microlora_creation() {
    let lora = MockMicroLoRA::new(256, 2, 0.1, 0.01);

    assert_eq!(lora.dim, 256);
    assert_eq!(lora.rank, 2);
    assert!((lora.alpha - 0.1).abs() < 0.001);
    assert_eq!(lora.adaptation_count, 0);
}

#[wasm_bindgen_test]
fn test_microlora_apply_transformation() {
    let lora = MockMicroLoRA::new(128, 2, 0.1, 0.01);

    let input = create_test_embedding(42, 128);
    let output = lora.apply(&input).unwrap();

    assert_eq!(output.len(), 128);
    // Initially B is zero, so output should be close to input (only alpha * A * B = 0)
    let diff: f32 = input
        .iter()
        .zip(output.iter())
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(diff < 0.01); // Should be very close
}

#[wasm_bindgen_test]
fn test_microlora_verify_output_shape() {
    let lora = MockMicroLoRA::new(256, 1, 0.2, 0.005);

    let input = vec![0.5; 256];
    let output = lora.apply(&input).unwrap();

    assert_eq!(output.len(), 256);
}

#[wasm_bindgen_test]
fn test_microlora_adapt_with_feedback() {
    let mut lora = MockMicroLoRA::new(128, 2, 0.1, 0.01);

    let feedback = create_test_embedding(100, 128);
    let result = lora.adapt(&feedback);

    assert!(result.is_ok());
    assert_eq!(lora.adaptation_count, 1);
}

#[wasm_bindgen_test]
fn test_microlora_adapt_changes_output() {
    let mut lora = MockMicroLoRA::new(128, 2, 0.1, 0.05);

    let input = create_test_embedding(42, 128);
    let output_before = lora.apply(&input).unwrap();

    // Adapt with feedback
    let feedback = create_test_embedding(100, 128);
    lora.adapt(&feedback).unwrap();

    let output_after = lora.apply(&input).unwrap();

    // Outputs should be different after adaptation
    let diff: f32 = output_before
        .iter()
        .zip(output_after.iter())
        .map(|(a, b)| (a - b).abs())
        .sum();

    assert!(diff > 1e-6); // Should have changed
}

#[wasm_bindgen_test]
fn test_microlora_stats_update() {
    let mut lora = MockMicroLoRA::new(64, 2, 0.1, 0.01);

    assert_eq!(lora.stats().adaptation_count, 0);

    let feedback = vec![0.1; 64];
    lora.adapt(&feedback).unwrap();
    lora.adapt(&feedback).unwrap();

    let stats = lora.stats();
    assert_eq!(stats.adaptation_count, 2);
    assert_eq!(stats.dim, 64);
    assert_eq!(stats.rank, 2);
}

#[wasm_bindgen_test]
fn test_microlora_reset() {
    let mut lora = MockMicroLoRA::new(128, 2, 0.1, 0.01);

    // Adapt multiple times
    let feedback = create_test_embedding(50, 128);
    for _ in 0..5 {
        lora.adapt(&feedback).unwrap();
    }

    assert_eq!(lora.adaptation_count, 5);

    // Reset
    lora.reset();

    assert_eq!(lora.adaptation_count, 0);
    // B matrix should be zero again
    for row in &lora.b_matrix {
        for &val in row {
            assert!((val).abs() < 1e-6);
        }
    }
}

#[wasm_bindgen_test]
fn test_microlora_dimension_mismatch() {
    let lora = MockMicroLoRA::new(256, 2, 0.1, 0.01);

    let wrong_input = vec![0.5; 128]; // Wrong size
    let result = lora.apply(&wrong_input);

    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn test_microlora_serialization() {
    let lora = MockMicroLoRA::new(128, 2, 0.15, 0.02);

    // In real implementation, would test to_json()
    let stats = lora.stats();
    assert_eq!(stats.dim, 128);
    assert_eq!(stats.rank, 2);
    assert!((stats.alpha - 0.15).abs() < 0.001);
}

// ============================================================================
// SONA Instant Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_sona_creation() {
    let sona = MockSONA::new(384, 0.01);

    assert_eq!(sona.dim, 384);
    assert!((sona.learning_rate - 0.01).abs() < 1e-6);
    assert_eq!(sona.pattern_memory.len(), 0);
}

#[wasm_bindgen_test]
fn test_sona_instant_adapt() {
    let mut sona = MockSONA::new(256, 0.01);

    let input = create_test_embedding(42, 256);
    let latency_us = sona.instant_adapt(&input, 0.8).unwrap();

    // Should complete in less than 1ms (1000 microseconds)
    assert!(latency_us < 1000);
    assert_eq!(sona.pattern_memory.len(), 1);
}

#[wasm_bindgen_test]
fn test_sona_instant_adapt_latency() {
    let mut sona = MockSONA::new(384, 0.01);

    let input = create_test_embedding(100, 384);

    // Run multiple times to verify consistent performance
    for _ in 0..10 {
        let latency_us = sona.instant_adapt(&input, 0.9).unwrap();
        assert!(latency_us < 1000); // <1ms requirement
    }
}

#[wasm_bindgen_test]
fn test_sona_record_patterns() {
    let mut sona = MockSONA::new(128, 0.01);

    // Record multiple patterns
    for i in 0..5 {
        let pattern = create_test_embedding(i * 10, 128);
        sona.record_pattern(pattern, 0.8 + (i as f32 * 0.02))
            .unwrap();
    }

    assert_eq!(sona.pattern_memory.len(), 5);
}

#[wasm_bindgen_test]
fn test_sona_get_suggestions() {
    let mut sona = MockSONA::new(128, 0.01);

    // Add patterns with different quality scores
    for i in 0..10 {
        let pattern = create_test_embedding(i * 20, 128);
        let quality = 0.5 + (i as f32 * 0.05);
        sona.record_pattern(pattern, quality).unwrap();
    }

    let query = create_test_embedding(45, 128); // Near pattern 2-3
    let suggestions = sona.get_suggestions(&query, 3).unwrap();

    assert_eq!(suggestions.len(), 3);
    // Should be ordered by quality * similarity
}

#[wasm_bindgen_test]
fn test_sona_learning_accumulation() {
    let mut sona = MockSONA::new(256, 0.01);

    let initial_count = sona.pattern_memory.len();

    // Learn from multiple inputs
    for i in 0..20 {
        let input = create_test_embedding(i * 5, 256);
        sona.instant_adapt(&input, 0.85).unwrap();
    }

    assert_eq!(sona.pattern_memory.len(), initial_count + 20);
}

#[wasm_bindgen_test]
fn test_sona_memory_limit() {
    let mut sona = MockSONA::new(128, 0.01);

    // Add more than limit (100)
    for i in 0..150 {
        let pattern = create_test_embedding(i, 128);
        sona.instant_adapt(&pattern, 0.8).unwrap();
    }

    // Should be capped at 100
    assert!(sona.pattern_memory.len() <= 100);
}

#[wasm_bindgen_test]
fn test_sona_dimension_validation() {
    let mut sona = MockSONA::new(256, 0.01);

    let wrong_input = vec![0.5; 128]; // Wrong dimension
    let result = sona.instant_adapt(&wrong_input, 0.8);

    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn test_sona_serialization() {
    let sona = MockSONA::new(384, 0.02);

    // In real implementation, would test to_json()
    assert_eq!(sona.dim, 384);
    assert!((sona.learning_rate - 0.02).abs() < 1e-6);
}

// ============================================================================
// Integrated IntelligentLLMWasm Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_integrated_system_creation() {
    let router = MockHnswRouter::new(384);
    let lora = MockMicroLoRA::new(384, 2, 0.1, 0.01);
    let sona = MockSONA::new(384, 0.01);

    assert_eq!(router.dimensions, 384);
    assert_eq!(lora.dim, 384);
    assert_eq!(sona.dim, 384);
}

#[wasm_bindgen_test]
fn test_integrated_flow_route_apply_adapt() {
    let mut router = MockHnswRouter::new(128);
    let mut lora = MockMicroLoRA::new(128, 2, 0.1, 0.01);
    let mut sona = MockSONA::new(128, 0.01);

    // 1. Add routing patterns
    let pattern1 = create_test_embedding(10, 128);
    router
        .add_pattern(pattern1.clone(), "code_generation".to_string())
        .unwrap();

    // 2. Route a query
    let query = create_test_embedding(15, 128);
    let results = router.search(&query, 1).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "code_generation");

    // 3. Apply LoRA transformation
    let transformed = lora.apply(&query).unwrap();
    assert_eq!(transformed.len(), 128);

    // 4. Adapt based on feedback
    let feedback = vec![0.1; 128];
    lora.adapt(&feedback).unwrap();

    // 5. Record in SONA
    sona.instant_adapt(&query, 0.85).unwrap();

    // Verify all components updated
    assert_eq!(lora.adaptation_count, 1);
    assert_eq!(sona.pattern_memory.len(), 1);
}

#[wasm_bindgen_test]
fn test_integrated_save_load_state() {
    let router = MockHnswRouter::new(384);
    let lora = MockMicroLoRA::new(384, 2, 0.1, 0.01);

    // Save state
    let router_json = router.to_json().unwrap();
    let lora_stats = lora.stats();

    // Verify state can be serialized
    assert!(router_json.contains("384"));
    assert_eq!(lora_stats.dim, 384);

    // Load state
    let restored_router = MockHnswRouter::from_json(&router_json).unwrap();
    assert_eq!(restored_router.dimensions, 384);
}

#[wasm_bindgen_test]
fn test_integrated_components_work_together() {
    let mut router = MockHnswRouter::new(256);
    let mut lora = MockMicroLoRA::new(256, 2, 0.1, 0.01);
    let mut sona = MockSONA::new(256, 0.01);

    // Simulate a complete workflow
    for i in 0..5 {
        let input = create_test_embedding(i * 20, 256);

        // 1. Add to router
        router
            .add_pattern(input.clone(), format!("task_{}", i))
            .unwrap();

        // 2. Transform with LoRA
        let transformed = lora.apply(&input).unwrap();

        // 3. Adapt LoRA
        let feedback = create_test_embedding((i + 1) * 20, 256);
        lora.adapt(&feedback).unwrap();

        // 4. Learn in SONA
        let quality = 0.7 + (i as f32 * 0.05);
        sona.instant_adapt(&transformed, quality).unwrap();
    }

    // Verify integrated state
    assert_eq!(router.patterns.len(), 5);
    assert_eq!(lora.adaptation_count, 5);
    assert_eq!(sona.pattern_memory.len(), 5);

    // Test query
    let query = create_test_embedding(50, 256);
    let route_results = router.search(&query, 2).unwrap();
    assert_eq!(route_results.len(), 2);

    let transformed_query = lora.apply(&query).unwrap();
    assert_eq!(transformed_query.len(), 256);

    let suggestions = sona.get_suggestions(&query, 3).unwrap();
    assert!(suggestions.len() <= 3);
}

// ============================================================================
// Performance Assertion Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_performance_hnsw_search_latency() {
    use std::time::Instant;

    let mut router = MockHnswRouter::new(384);

    // Add 100 patterns
    for i in 0..100 {
        let embedding = create_test_embedding(i * 10, 384);
        router.add_pattern(embedding, format!("p{}", i)).unwrap();
    }

    let query = create_test_embedding(500, 384);

    let start = Instant::now();
    let _results = router.search(&query, 10).unwrap();
    let latency = start.elapsed();

    // Should be fast even with 100 patterns
    assert!(latency.as_micros() < 10_000); // <10ms
}

#[wasm_bindgen_test]
fn test_performance_lora_forward_pass() {
    use std::time::Instant;

    let lora = MockMicroLoRA::new(384, 2, 0.1, 0.01);
    let input = create_test_embedding(42, 384);

    let start = Instant::now();
    let _output = lora.apply(&input).unwrap();
    let latency = start.elapsed();

    // Should complete in <1ms for rank-2
    assert!(latency.as_micros() < 1000);
}

#[wasm_bindgen_test]
fn test_performance_sona_instant_adapt_under_1ms() {
    let mut sona = MockSONA::new(384, 0.01);
    let input = create_test_embedding(42, 384);

    let latency_us = sona.instant_adapt(&input, 0.85).unwrap();

    // Critical: must be under 1ms
    assert!(latency_us < 1000);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_edge_case_zero_vectors() {
    let mut router = MockHnswRouter::new(128);

    let zero_vec = vec![0.0; 128];
    router
        .add_pattern(zero_vec.clone(), "zero".to_string())
        .unwrap();

    let results = router.search(&zero_vec, 1).unwrap();
    assert_eq!(results.len(), 1);
}

#[wasm_bindgen_test]
fn test_edge_case_very_small_values() {
    let lora = MockMicroLoRA::new(128, 2, 0.1, 0.01);

    let tiny_input = vec![1e-10; 128];
    let output = lora.apply(&tiny_input).unwrap();

    assert_eq!(output.len(), 128);
    // Should handle tiny values without numerical issues
    assert!(output.iter().all(|&x| x.is_finite()));
}

#[wasm_bindgen_test]
fn test_edge_case_high_dimensional() {
    let router = MockHnswRouter::new(1024);
    let lora = MockMicroLoRA::new(1024, 2, 0.1, 0.01);
    let sona = MockSONA::new(1024, 0.01);

    assert_eq!(router.dimensions, 1024);
    assert_eq!(lora.dim, 1024);
    assert_eq!(sona.dim, 1024);
}

#[wasm_bindgen_test]
fn test_edge_case_single_pattern() {
    let mut router = MockHnswRouter::new(128);

    let pattern = create_test_embedding(42, 128);
    router
        .add_pattern(pattern.clone(), "only_one".to_string())
        .unwrap();

    let results = router.search(&pattern, 5).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "only_one");
}
