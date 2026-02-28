# Agent 12: Integration Tests

## Overview

Comprehensive integration test suite for GNN latent space attention mechanisms, covering integration with ruvector-gnn, ruvector-core, cross-platform consistency, and end-to-end workflows.

## 1. Integration with ruvector-gnn

### 1.1 Attention as GNN Layer

```rust
// tests/integration/gnn_attention_layer.rs

use ruvector_gnn::{GNNModel, GraphBatch, LayerType};
use ruvector_latent::attention::{AttentionConfig, AttentionLayer, AttentionType};
use ndarray::{Array2, Array3};

#[test]
fn test_attention_as_gnn_message_passing() {
    // Setup: Create graph with 10 nodes, 5 neighbors each
    let num_nodes = 10;
    let num_neighbors = 5;
    let embed_dim = 64;

    let graph = create_test_graph(num_nodes, num_neighbors);

    // Create attention layer as GNN message passing
    let config = AttentionConfig {
        num_heads: 4,
        embed_dim,
        dropout: 0.1,
        attention_type: AttentionType::GraphAttention,
    };

    let mut attention_layer = AttentionLayer::new(config);

    // Node features
    let node_features = Array2::<f32>::random((num_nodes, embed_dim), rand::distributions::Uniform::new(-1.0, 1.0));

    // Edge indices for graph connectivity
    let edge_index = graph.edge_index();

    // Forward pass: Attention over neighbors
    let output = attention_layer.forward_graph(&node_features, &edge_index);

    // Assertions
    assert_eq!(output.shape(), &[num_nodes, embed_dim]);
    assert!(!output.iter().any(|&x| x.is_nan()));

    // Verify message aggregation: output should differ from input
    let difference = (&output - &node_features).mapv(|x| x.abs()).sum();
    assert!(difference > 0.01, "Attention should modify node features");

    // Verify attention weights sum to 1
    let attention_weights = attention_layer.get_attention_weights();
    for head in 0..config.num_heads {
        for node in 0..num_nodes {
            let weights_sum: f32 = attention_weights
                .slice(s![head, node, ..num_neighbors])
                .sum();
            assert!((weights_sum - 1.0).abs() < 1e-5,
                "Attention weights should sum to 1, got {}", weights_sum);
        }
    }
}

#[test]
fn test_attention_gnn_multi_layer_stack() {
    // Test stacking attention layers in GNN architecture
    let num_nodes = 20;
    let num_neighbors = 8;
    let embed_dims = vec![64, 128, 256, 128, 64];

    let graph = create_test_graph(num_nodes, num_neighbors);
    let edge_index = graph.edge_index();

    let mut layers = Vec::new();
    for (i, &dim) in embed_dims.iter().enumerate() {
        let config = AttentionConfig {
            num_heads: if i < 2 { 4 } else { 8 },
            embed_dim: dim,
            dropout: 0.1,
            attention_type: AttentionType::GraphAttention,
        };
        layers.push(AttentionLayer::new(config));
    }

    // Initial features
    let mut features = Array2::<f32>::random(
        (num_nodes, embed_dims[0]),
        rand::distributions::Uniform::new(-1.0, 1.0)
    );

    // Forward pass through all layers
    for (i, layer) in layers.iter_mut().enumerate() {
        features = layer.forward_graph(&features, &edge_index);

        // Verify shape after each layer
        assert_eq!(features.shape(), &[num_nodes, embed_dims[i]]);
        assert!(!features.iter().any(|&x| x.is_nan()));

        println!("Layer {} output range: [{:.4}, {:.4}]",
            i,
            features.iter().cloned().fold(f32::INFINITY, f32::min),
            features.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
        );
    }
}

#[test]
fn test_attention_heterogeneous_graph() {
    // Test attention on heterogeneous graphs (multiple node/edge types)
    let num_items = 50;
    let num_users = 30;
    let num_categories = 10;

    // Create bipartite user-item graph
    let graph = create_heterogeneous_graph(num_users, num_items, num_categories);

    let config = AttentionConfig {
        num_heads: 4,
        embed_dim: 64,
        dropout: 0.1,
        attention_type: AttentionType::GraphAttention,
    };

    let mut attention = AttentionLayer::new(config);

    // Separate embeddings for each node type
    let user_features = Array2::<f32>::random((num_users, 64), rand::distributions::Uniform::new(-1.0, 1.0));
    let item_features = Array2::<f32>::random((num_items, 64), rand::distributions::Uniform::new(-1.0, 1.0));

    // Process user-item interactions
    let user_embeddings = attention.forward_heterogeneous(
        &user_features,
        &item_features,
        &graph.user_item_edges()
    );

    assert_eq!(user_embeddings.shape(), &[num_users, 64]);
    assert!(!user_embeddings.iter().any(|&x| x.is_nan()));
}

### 1.2 Training Loop Integration

```rust
// tests/integration/gnn_training.rs

use ruvector_gnn::{GNNTrainer, TrainingConfig, Loss};
use ruvector_latent::attention::{AttentionConfig, AttentionLayer};

#[test]
fn test_attention_backward_pass_integration() {
    let config = AttentionConfig {
        num_heads: 4,
        embed_dim: 64,
        dropout: 0.1,
        attention_type: AttentionType::MultiHead,
    };

    let mut attention = AttentionLayer::new(config);
    let optimizer = Adam::new(0.001);

    let batch_size = 32;
    let seq_len = 50;
    let embed_dim = 64;

    // Create synthetic training data
    let input = Array3::<f32>::random(
        (batch_size, seq_len, embed_dim),
        rand::distributions::Uniform::new(-1.0, 1.0)
    );
    let target = Array3::<f32>::random(
        (batch_size, seq_len, embed_dim),
        rand::distributions::Uniform::new(-1.0, 1.0)
    );

    // Training loop
    let mut losses = Vec::new();
    for epoch in 0..100 {
        // Forward pass
        let output = attention.forward(&input);

        // Compute loss
        let loss = mse_loss(&output, &target);
        losses.push(loss);

        // Backward pass
        let grad_output = compute_gradient(&output, &target);
        let grad_input = attention.backward(&grad_output);

        // Verify gradient shapes
        assert_eq!(grad_input.shape(), input.shape());
        assert!(!grad_input.iter().any(|&x| x.is_nan()));

        // Update weights
        optimizer.step(&mut attention.parameters(), &attention.gradients());

        if epoch % 20 == 0 {
            println!("Epoch {}: Loss = {:.6}", epoch, loss);
        }
    }

    // Verify training reduced loss
    let initial_loss = losses[0];
    let final_loss = losses[losses.len() - 1];
    assert!(final_loss < initial_loss * 0.5,
        "Training should reduce loss by at least 50%: {:.6} -> {:.6}",
        initial_loss, final_loss);
}

#[test]
fn test_attention_gradient_flow() {
    // Test gradient flow through attention mechanism
    let config = AttentionConfig {
        num_heads: 8,
        embed_dim: 128,
        dropout: 0.0, // No dropout for gradient testing
        attention_type: AttentionType::MultiHead,
    };

    let mut attention = AttentionLayer::new(config);

    let batch_size = 16;
    let seq_len = 32;
    let embed_dim = 128;

    let input = Array3::<f32>::random(
        (batch_size, seq_len, embed_dim),
        rand::distributions::Uniform::new(-0.5, 0.5)
    );

    // Forward pass
    let output = attention.forward(&input);

    // Create synthetic gradient
    let grad_output = Array3::<f32>::ones((batch_size, seq_len, embed_dim));

    // Backward pass
    let grad_input = attention.backward(&grad_output);

    // Verify gradients exist for all parameters
    let param_grads = attention.gradients();
    assert!(param_grads.query_weights.iter().any(|&x| x.abs() > 1e-8));
    assert!(param_grads.key_weights.iter().any(|&x| x.abs() > 1e-8));
    assert!(param_grads.value_weights.iter().any(|&x| x.abs() > 1e-8));
    assert!(param_grads.output_weights.iter().any(|&x| x.abs() > 1e-8));

    // Check for exploding/vanishing gradients
    let grad_norm = grad_input.mapv(|x| x * x).sum().sqrt();
    let input_norm = input.mapv(|x| x * x).sum().sqrt();
    let grad_ratio = grad_norm / input_norm;

    assert!(grad_ratio > 0.01 && grad_ratio < 100.0,
        "Gradient ratio should be reasonable: {:.4}", grad_ratio);
}

#[test]
fn test_attention_with_graph_convolution() {
    // Integration test: Attention + GCN layers
    let num_nodes = 100;
    let num_edges = 500;
    let embed_dim = 64;

    let graph = create_random_graph(num_nodes, num_edges);

    // Build hybrid model: GCN -> Attention -> GCN
    let mut model = GNNModel::new(vec![
        LayerType::GraphConv { in_dim: embed_dim, out_dim: 64 },
        LayerType::Attention(AttentionConfig {
            num_heads: 4,
            embed_dim: 64,
            dropout: 0.1,
            attention_type: AttentionType::GraphAttention,
        }),
        LayerType::GraphConv { in_dim: 64, out_dim: 32 },
    ]);

    let optimizer = Adam::new(0.001);
    let trainer = GNNTrainer::new(model, optimizer);

    // Training data
    let node_features = Array2::<f32>::random(
        (num_nodes, embed_dim),
        rand::distributions::Uniform::new(-1.0, 1.0)
    );
    let labels = Array1::<usize>::from_vec(
        (0..num_nodes).map(|_| rand::random::<usize>() % 5).collect()
    );

    // Train for 50 epochs
    let history = trainer.fit(
        &graph,
        &node_features,
        &labels,
        TrainingConfig {
            epochs: 50,
            batch_size: 32,
            validation_split: 0.2,
        }
    );

    // Verify training progress
    assert!(history.train_loss[0] > history.train_loss[49]);
    assert!(history.train_accuracy[49] > 0.6);

    println!("Final accuracy: {:.2}%", history.train_accuracy[49] * 100.0);
}

### 1.3 Backward Pass Verification

```rust
// tests/integration/gradient_verification.rs

use approx::assert_relative_eq;

#[test]
fn test_attention_numerical_gradients() {
    // Verify analytical gradients against numerical gradients
    let config = AttentionConfig {
        num_heads: 2,
        embed_dim: 32,
        dropout: 0.0,
        attention_type: AttentionType::MultiHead,
    };

    let mut attention = AttentionLayer::new(config);

    let batch_size = 4;
    let seq_len = 8;
    let embed_dim = 32;

    let input = Array3::<f32>::random(
        (batch_size, seq_len, embed_dim),
        rand::distributions::Uniform::new(-0.5, 0.5)
    );

    // Analytical gradients
    let output = attention.forward(&input);
    let grad_output = Array3::<f32>::ones(output.raw_dim());
    let analytical_grad = attention.backward(&grad_output);

    // Numerical gradients (finite differences)
    let epsilon = 1e-4;
    let mut numerical_grad = Array3::<f32>::zeros(input.raw_dim());

    for i in 0..batch_size {
        for j in 0..seq_len {
            for k in 0..embed_dim {
                // f(x + epsilon)
                let mut input_plus = input.clone();
                input_plus[[i, j, k]] += epsilon;
                let output_plus = attention.forward(&input_plus);
                let loss_plus = output_plus.sum();

                // f(x - epsilon)
                let mut input_minus = input.clone();
                input_minus[[i, j, k]] -= epsilon;
                let output_minus = attention.forward(&input_minus);
                let loss_minus = output_minus.sum();

                // Numerical gradient
                numerical_grad[[i, j, k]] = (loss_plus - loss_minus) / (2.0 * epsilon);
            }
        }
    }

    // Compare analytical and numerical gradients
    for i in 0..batch_size {
        for j in 0..seq_len {
            for k in 0..embed_dim {
                assert_relative_eq!(
                    analytical_grad[[i, j, k]],
                    numerical_grad[[i, j, k]],
                    epsilon = 1e-3,
                    max_relative = 0.01
                );
            }
        }
    }
}

#[test]
fn test_attention_gradient_accumulation() {
    // Test gradient accumulation across mini-batches
    let config = AttentionConfig {
        num_heads: 4,
        embed_dim: 64,
        dropout: 0.1,
        attention_type: AttentionType::MultiHead,
    };

    let mut attention = AttentionLayer::new(config);
    let accumulation_steps = 4;

    for step in 0..accumulation_steps {
        let input = Array3::<f32>::random(
            (8, 16, 64),
            rand::distributions::Uniform::new(-1.0, 1.0)
        );

        let output = attention.forward(&input);
        let grad_output = Array3::<f32>::ones(output.raw_dim());
        let _ = attention.backward(&grad_output);

        // Don't zero gradients until accumulation is done
        if step < accumulation_steps - 1 {
            attention.accumulate_gradients();
        }
    }

    // Verify accumulated gradients
    let grads = attention.gradients();
    let total_grad_norm = grads.query_weights.mapv(|x| x * x).sum().sqrt();

    assert!(total_grad_norm > 0.0);
    println!("Accumulated gradient norm: {:.6}", total_grad_norm);
}

## 2. Integration with ruvector-core

### 2.1 HNSW with Attention-Guided Search

```rust
// tests/integration/hnsw_attention.rs

use ruvector_core::{HNSWIndex, IndexConfig, SearchConfig};
use ruvector_latent::attention::{AttentionConfig, AttentionGuidedSearch};

#[test]
fn test_hnsw_attention_search_integration() {
    let dim = 128;
    let num_vectors = 10000;

    // Build HNSW index
    let index_config = IndexConfig {
        m: 16,
        ef_construction: 200,
        max_elements: num_vectors,
        distance_type: DistanceType::Cosine,
    };

    let mut index = HNSWIndex::new(dim, index_config);

    // Add vectors
    let vectors = generate_clustered_vectors(num_vectors, dim, 20);
    for (i, vec) in vectors.iter().enumerate() {
        index.add_vector(i, vec);
    }

    // Create attention-guided search
    let attention_config = AttentionConfig {
        num_heads: 8,
        embed_dim: dim,
        dropout: 0.0,
        attention_type: AttentionType::CrossAttention,
    };

    let attention_search = AttentionGuidedSearch::new(attention_config);

    // Query vector
    let query = Array1::<f32>::random(dim, rand::distributions::Uniform::new(-1.0, 1.0));

    // Standard HNSW search
    let search_config = SearchConfig { ef: 50, k: 10 };
    let standard_results = index.search(&query, search_config);

    // Attention-guided search
    let attention_results = attention_search.search(
        &index,
        &query,
        search_config,
        &vectors
    );

    // Compare results
    println!("Standard HNSW results:");
    for (i, (id, dist)) in standard_results.iter().enumerate() {
        println!("  {}: ID={}, dist={:.6}", i, id, dist);
    }

    println!("\nAttention-guided results:");
    for (i, (id, dist)) in attention_results.iter().enumerate() {
        println!("  {}: ID={}, dist={:.6}", i, id, dist);
    }

    // Verify attention improves relevance
    let attention_avg_dist: f32 = attention_results.iter()
        .map(|(_, d)| d)
        .sum::<f32>() / attention_results.len() as f32;

    let standard_avg_dist: f32 = standard_results.iter()
        .map(|(_, d)| d)
        .sum::<f32>() / standard_results.len() as f32;

    assert!(attention_avg_dist <= standard_avg_dist * 1.1,
        "Attention should not significantly degrade distance metrics");
}

#[test]
fn test_attention_reranking() {
    // Test using attention for result reranking
    let dim = 64;
    let num_vectors = 5000;
    let k = 100; // Retrieve more candidates
    let top_k = 10; // Final top results

    let mut index = HNSWIndex::new(dim, IndexConfig::default());
    let vectors = generate_semantic_vectors(num_vectors, dim);

    for (i, vec) in vectors.iter().enumerate() {
        index.add_vector(i, vec);
    }

    let query = Array1::<f32>::random(dim, rand::distributions::Uniform::new(-1.0, 1.0));

    // Stage 1: HNSW retrieval (fast, approximate)
    let candidates = index.search(&query, SearchConfig { ef: 100, k });

    // Stage 2: Attention reranking (precise, contextual)
    let attention_config = AttentionConfig {
        num_heads: 4,
        embed_dim: dim,
        dropout: 0.0,
        attention_type: AttentionType::CrossAttention,
    };

    let reranker = AttentionReranker::new(attention_config);

    let candidate_vectors: Vec<_> = candidates.iter()
        .map(|(id, _)| vectors[*id].clone())
        .collect();

    let reranked = reranker.rerank(&query, &candidate_vectors, top_k);

    // Verify reranking changes order
    let order_changed = reranked.iter()
        .zip(candidates.iter().take(top_k))
        .any(|(r, c)| r.0 != c.0);

    assert!(order_changed, "Reranking should change result order");

    println!("Top-{} after reranking:", top_k);
    for (i, (id, score)) in reranked.iter().enumerate() {
        println!("  {}: ID={}, score={:.6}", i, id, score);
    }
}

#[test]
fn test_attention_guided_graph_traversal() {
    // Test attention directing graph traversal in HNSW
    let dim = 128;
    let num_vectors = 20000;

    let mut index = HNSWIndex::new(dim, IndexConfig {
        m: 32,
        ef_construction: 200,
        max_elements: num_vectors,
        distance_type: DistanceType::Euclidean,
    });

    let vectors = generate_hierarchical_vectors(num_vectors, dim);
    for (i, vec) in vectors.iter().enumerate() {
        index.add_vector(i, vec);
    }

    let attention_config = AttentionConfig {
        num_heads: 8,
        embed_dim: dim,
        dropout: 0.0,
        attention_type: AttentionType::GraphAttention,
    };

    let mut guided_search = AttentionGuidedHNSW::new(attention_config);

    let query = vectors[0].clone();

    // Track traversal path
    let (results, traversal_stats) = guided_search.search_with_stats(
        &index,
        &query,
        SearchConfig { ef: 50, k: 10 },
        &vectors
    );

    println!("Traversal statistics:");
    println!("  Nodes visited: {}", traversal_stats.nodes_visited);
    println!("  Distance computations: {}", traversal_stats.distance_computations);
    println!("  Attention computations: {}", traversal_stats.attention_computations);

    // Attention should reduce nodes visited
    let standard_stats = index.search_with_stats(&query, SearchConfig { ef: 50, k: 10 });

    assert!(traversal_stats.nodes_visited <= standard_stats.nodes_visited,
        "Attention-guided search should visit fewer nodes");
}

### 2.2 Learned Distance Metrics

```rust
// tests/integration/learned_metrics.rs

use ruvector_core::{DistanceMetric, HNSWIndex};
use ruvector_latent::metrics::{LearnedMetric, MetricTrainer};

#[test]
fn test_learned_metric_integration() {
    let dim = 64;
    let num_vectors = 5000;

    // Generate training data with semantic relationships
    let (vectors, similarity_labels) = generate_labeled_vectors(num_vectors, dim);

    // Train learned metric
    let metric_config = LearnedMetricConfig {
        input_dim: dim,
        hidden_dims: vec![128, 64, 32],
        output_dim: 1,
        learning_rate: 0.001,
    };

    let mut trainer = MetricTrainer::new(metric_config);

    // Training loop
    for epoch in 0..100 {
        let loss = trainer.train_epoch(&vectors, &similarity_labels);

        if epoch % 20 == 0 {
            println!("Epoch {}: Loss = {:.6}", epoch, loss);
        }
    }

    let learned_metric = trainer.get_metric();

    // Create HNSW index with learned metric
    let mut index = HNSWIndex::new_with_metric(
        dim,
        IndexConfig::default(),
        Box::new(learned_metric)
    );

    for (i, vec) in vectors.iter().enumerate() {
        index.add_vector(i, vec);
    }

    // Test search with learned metric
    let query = vectors[0].clone();
    let results = index.search(&query, SearchConfig { ef: 50, k: 10 });

    // Verify results respect learned similarity
    for (rank, (id, dist)) in results.iter().enumerate() {
        let true_similarity = similarity_labels.get(&(0, *id)).unwrap_or(&0.0);
        println!("Rank {}: ID={}, dist={:.6}, true_sim={:.4}",
            rank, id, dist, true_similarity);
    }
}

#[test]
fn test_metric_learning_with_triplet_loss() {
    // Train metric using triplet loss (anchor, positive, negative)
    let dim = 128;
    let num_triplets = 10000;

    let triplets = generate_triplets(num_triplets, dim);

    let config = LearnedMetricConfig {
        input_dim: dim,
        hidden_dims: vec![256, 128, 64],
        output_dim: dim,
        learning_rate: 0.0001,
    };

    let mut trainer = MetricTrainer::new(config);

    for epoch in 0..200 {
        let loss = trainer.train_triplet_epoch(&triplets, margin = 1.0);

        if epoch % 20 == 0 {
            println!("Epoch {}: Triplet Loss = {:.6}", epoch, loss);
        }
    }

    let metric = trainer.get_metric();

    // Verify triplet constraints
    let mut violations = 0;
    for (anchor, positive, negative) in triplets.iter().take(100) {
        let dist_pos = metric.distance(anchor, positive);
        let dist_neg = metric.distance(anchor, negative);

        if dist_pos >= dist_neg {
            violations += 1;
        }
    }

    let violation_rate = violations as f32 / 100.0;
    assert!(violation_rate < 0.1,
        "Triplet violation rate should be < 10%, got {:.2}%",
        violation_rate * 100.0);
}

#[test]
fn test_adaptive_metric_during_search() {
    // Test metric that adapts during search based on query
    let dim = 64;
    let num_vectors = 8000;

    let vectors = generate_multi_modal_vectors(num_vectors, dim);

    let adaptive_config = AdaptiveMetricConfig {
        base_dim: dim,
        num_modes: 5,
        attention_heads: 4,
    };

    let adaptive_metric = AdaptiveLearnedMetric::new(adaptive_config);

    let mut index = HNSWIndex::new_with_metric(
        dim,
        IndexConfig::default(),
        Box::new(adaptive_metric.clone())
    );

    for (i, vec) in vectors.iter().enumerate() {
        index.add_vector(i, vec);
    }

    // Different queries should use different metric adaptations
    let query1 = vectors[0].clone(); // Mode 1
    let query2 = vectors[num_vectors / 2].clone(); // Mode 2

    // Search adapts metric based on query
    adaptive_metric.adapt_to_query(&query1);
    let results1 = index.search(&query1, SearchConfig { ef: 50, k: 10 });

    adaptive_metric.adapt_to_query(&query2);
    let results2 = index.search(&query2, SearchConfig { ef: 50, k: 10 });

    // Verify different adaptations produce different results
    let overlap = results1.iter()
        .filter(|(id1, _)| results2.iter().any(|(id2, _)| id1 == id2))
        .count();

    assert!(overlap < 8, "Different queries should produce different results");
}

## 3. Cross-Platform Consistency

### 3.1 Rust vs WASM Results

```rust
// tests/integration/cross_platform.rs

#[cfg(test)]
mod cross_platform_tests {
    use super::*;

    #[test]
    fn test_rust_wasm_attention_consistency() {
        let config = AttentionConfig {
            num_heads: 4,
            embed_dim: 64,
            dropout: 0.0, // Disable dropout for deterministic testing
            attention_type: AttentionType::MultiHead,
        };

        // Create identical models in Rust and WASM
        let mut rust_attention = AttentionLayer::new(config.clone());

        #[cfg(target_arch = "wasm32")]
        let mut wasm_attention = wasm::AttentionLayer::new(config);

        // Test input
        let input = Array3::<f32>::from_shape_fn((4, 16, 64), |(i, j, k)| {
            ((i * 16 * 64 + j * 64 + k) as f32 * 0.01).sin()
        });

        // Rust forward pass
        let rust_output = rust_attention.forward(&input);

        #[cfg(target_arch = "wasm32")]
        let wasm_output = {
            // Convert to WASM-compatible format
            let wasm_input = wasm::Array3::from_rust(&input);
            let output = wasm_attention.forward(&wasm_input);
            output.to_rust()
        };

        #[cfg(target_arch = "wasm32")]
        {
            // Compare outputs
            let max_diff = rust_output.iter()
                .zip(wasm_output.iter())
                .map(|(r, w)| (r - w).abs())
                .fold(0.0f32, f32::max);

            assert!(max_diff < 1e-5,
                "Rust and WASM outputs should match within tolerance, max diff: {}",
                max_diff);

            println!("Rust vs WASM max difference: {:.8}", max_diff);
        }
    }

    #[test]
    fn test_wasm_numerical_stability() {
        // Test WASM implementation for numerical stability
        #[cfg(target_arch = "wasm32")]
        {
            let config = AttentionConfig {
                num_heads: 8,
                embed_dim: 128,
                dropout: 0.0,
                attention_type: AttentionType::MultiHead,
            };

            let mut attention = wasm::AttentionLayer::new(config);

            // Test with extreme values
            let large_input = wasm::Array3::from_fn((2, 8, 128), |_| 100.0);
            let small_input = wasm::Array3::from_fn((2, 8, 128), |_| 0.001);
            let mixed_input = wasm::Array3::from_fn((2, 8, 128), |(i, j, k)| {
                if k % 2 == 0 { 100.0 } else { 0.001 }
            });

            let outputs = vec![
                attention.forward(&large_input),
                attention.forward(&small_input),
                attention.forward(&mixed_input),
            ];

            for (i, output) in outputs.iter().enumerate() {
                assert!(!output.iter().any(|&x| x.is_nan()),
                    "Output {} contains NaN", i);
                assert!(!output.iter().any(|&x| x.is_infinite()),
                    "Output {} contains infinity", i);
            }
        }
    }
}

### 3.2 Rust vs NAPI Results

```typescript
// tests/integration/napi_consistency.test.ts

import { describe, it, expect } from 'vitest';
import { AttentionLayer, AttentionConfig, AttentionType } from '../../napi';

describe('NAPI Consistency Tests', () => {
  it('should match Rust implementation outputs', async () => {
    const config: AttentionConfig = {
      numHeads: 4,
      embedDim: 64,
      dropout: 0.0,
      attentionType: AttentionType.MultiHead,
    };

    const attention = new AttentionLayer(config);

    // Generate deterministic input
    const batchSize = 4;
    const seqLen = 16;
    const embedDim = 64;

    const input = new Float32Array(batchSize * seqLen * embedDim);
    for (let i = 0; i < input.length; i++) {
      input[i] = Math.sin(i * 0.01);
    }

    // Forward pass through NAPI
    const napiOutput = attention.forward(input, [batchSize, seqLen, embedDim]);

    // Load reference Rust output (generated offline)
    const rustOutput = await loadRustReference('attention_reference.bin');

    // Compare
    let maxDiff = 0;
    let totalDiff = 0;

    for (let i = 0; i < napiOutput.length; i++) {
      const diff = Math.abs(napiOutput[i] - rustOutput[i]);
      maxDiff = Math.max(maxDiff, diff);
      totalDiff += diff;
    }

    const avgDiff = totalDiff / napiOutput.length;

    expect(maxDiff).toBeLessThan(1e-5);
    expect(avgDiff).toBeLessThan(1e-6);

    console.log(`NAPI vs Rust - Max diff: ${maxDiff}, Avg diff: ${avgDiff}`);
  });

  it('should handle concurrent requests consistently', async () => {
    const config: AttentionConfig = {
      numHeads: 8,
      embedDim: 128,
      dropout: 0.0,
      attentionType: AttentionType.MultiHead,
    };

    const attention = new AttentionLayer(config);

    // Create 100 random inputs
    const inputs = Array.from({ length: 100 }, (_, i) => {
      const data = new Float32Array(4 * 16 * 128);
      for (let j = 0; j < data.length; j++) {
        data[j] = Math.sin((i * 1000 + j) * 0.01);
      }
      return data;
    });

    // Process sequentially first
    const sequentialOutputs = inputs.map(input =>
      attention.forward(input, [4, 16, 128])
    );

    // Process concurrently
    const concurrentOutputs = await Promise.all(
      inputs.map(input =>
        Promise.resolve(attention.forward(input, [4, 16, 128]))
      )
    );

    // Verify consistency
    for (let i = 0; i < 100; i++) {
      const sequential = sequentialOutputs[i];
      const concurrent = concurrentOutputs[i];

      let maxDiff = 0;
      for (let j = 0; j < sequential.length; j++) {
        maxDiff = Math.max(maxDiff, Math.abs(sequential[j] - concurrent[j]));
      }

      expect(maxDiff).toBeLessThan(1e-7);
    }
  });

  it('should maintain precision across type conversions', () => {
    const config: AttentionConfig = {
      numHeads: 4,
      embedDim: 64,
      dropout: 0.0,
      attentionType: AttentionType.MultiHead,
    };

    const attention = new AttentionLayer(config);

    // Test with various numeric ranges
    const testCases = [
      { range: [0, 1], name: 'normalized' },
      { range: [-1, 1], name: 'centered' },
      { range: [-100, 100], name: 'large' },
      { range: [-0.001, 0.001], name: 'small' },
    ];

    for (const testCase of testCases) {
      const input = new Float32Array(4 * 16 * 64);
      const [min, max] = testCase.range;

      for (let i = 0; i < input.length; i++) {
        input[i] = min + (max - min) * Math.random();
      }

      const output = attention.forward(input, [4, 16, 64]);

      // Verify no precision loss symptoms
      const hasNaN = output.some(x => isNaN(x));
      const hasInf = output.some(x => !isFinite(x));
      const allZeros = output.every(x => x === 0);

      expect(hasNaN).toBe(false);
      expect(hasInf).toBe(false);
      expect(allZeros).toBe(false);

      console.log(`${testCase.name}: output range [${Math.min(...output)}, ${Math.max(...output)}]`);
    }
  });
});

### 3.3 Numerical Tolerance Checks

```rust
// tests/integration/numerical_tolerance.rs

use approx::assert_relative_eq;

#[test]
fn test_cross_platform_numerical_tolerance() {
    // Define acceptable tolerances for each platform
    struct PlatformTolerance {
        absolute: f32,
        relative: f32,
    }

    let tolerances = HashMap::from([
        ("rust_x86_64", PlatformTolerance { absolute: 1e-7, relative: 1e-5 }),
        ("rust_aarch64", PlatformTolerance { absolute: 1e-6, relative: 1e-5 }),
        ("wasm32", PlatformTolerance { absolute: 1e-5, relative: 1e-4 }),
        ("napi", PlatformTolerance { absolute: 1e-5, relative: 1e-4 }),
    ]);

    let config = AttentionConfig {
        num_heads: 4,
        embed_dim: 64,
        dropout: 0.0,
        attention_type: AttentionType::MultiHead,
    };

    // Generate reference output on current platform
    let mut attention = AttentionLayer::new(config.clone());
    let input = Array3::<f32>::from_shape_fn((4, 16, 64), |(i, j, k)| {
        ((i * 16 * 64 + j * 64 + k) as f32 * 0.01).sin()
    });

    let reference_output = attention.forward(&input);

    // Save reference for cross-platform comparison
    save_reference("attention_output_reference.bin", &reference_output);

    // Load outputs from other platforms (if available)
    let platform_outputs = load_platform_outputs("attention_output_*.bin");

    for (platform, output) in platform_outputs {
        let tolerance = tolerances.get(platform.as_str()).unwrap();

        let mut max_abs_diff = 0.0f32;
        let mut max_rel_diff = 0.0f32;

        for (ref_val, plat_val) in reference_output.iter().zip(output.iter()) {
            let abs_diff = (ref_val - plat_val).abs();
            let rel_diff = if ref_val.abs() > 1e-8 {
                abs_diff / ref_val.abs()
            } else {
                abs_diff
            };

            max_abs_diff = max_abs_diff.max(abs_diff);
            max_rel_diff = max_rel_diff.max(rel_diff);
        }

        assert!(max_abs_diff < tolerance.absolute,
            "{}: Absolute difference {} exceeds tolerance {}",
            platform, max_abs_diff, tolerance.absolute);

        assert!(max_rel_diff < tolerance.relative,
            "{}: Relative difference {} exceeds tolerance {}",
            platform, max_rel_diff, tolerance.relative);

        println!("{}: abs_diff={:.8}, rel_diff={:.8}",
            platform, max_abs_diff, max_rel_diff);
    }
}

#[test]
fn test_deterministic_execution() {
    // Verify same input produces same output across runs
    let config = AttentionConfig {
        num_heads: 4,
        embed_dim: 64,
        dropout: 0.0, // Critical: no dropout for determinism
        attention_type: AttentionType::MultiHead,
    };

    let input = Array3::<f32>::from_shape_fn((4, 16, 64), |(i, j, k)| {
        ((i * 16 * 64 + j * 64 + k) as f32 * 0.01).sin()
    });

    let mut outputs = Vec::new();

    // Run 10 times
    for run in 0..10 {
        let mut attention = AttentionLayer::new(config.clone());
        let output = attention.forward(&input);
        outputs.push(output);
    }

    // All outputs should be identical
    let reference = &outputs[0];

    for (run, output) in outputs.iter().enumerate().skip(1) {
        for (i, (ref_val, out_val)) in reference.iter().zip(output.iter()).enumerate() {
            assert_eq!(ref_val, out_val,
                "Run {} differs at index {}: {} != {}",
                run, i, ref_val, out_val);
        }
    }
}

#[test]
fn test_floating_point_edge_cases() {
    // Test handling of special float values
    let config = AttentionConfig {
        num_heads: 2,
        embed_dim: 32,
        dropout: 0.0,
        attention_type: AttentionType::MultiHead,
    };

    let mut attention = AttentionLayer::new(config);

    // Test cases
    let test_inputs = vec![
        ("zeros", Array3::<f32>::zeros((2, 8, 32))),
        ("ones", Array3::<f32>::ones((2, 8, 32))),
        ("large", Array3::<f32>::from_elem((2, 8, 32), 1e6)),
        ("small", Array3::<f32>::from_elem((2, 8, 32), 1e-6)),
        ("negative", Array3::<f32>::from_elem((2, 8, 32), -1.0)),
    ];

    for (name, input) in test_inputs {
        let output = attention.forward(&input);

        // Verify no NaN or Inf
        assert!(!output.iter().any(|&x| x.is_nan()),
            "{}: Output contains NaN", name);
        assert!(!output.iter().any(|&x| x.is_infinite()),
            "{}: Output contains Inf", name);

        println!("{}: output range [{:.6}, {:.6}]",
            name,
            output.iter().cloned().fold(f32::INFINITY, f32::min),
            output.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
        );
    }
}

## 4. End-to-End Workflows

### 4.1 Index Building with Attention

```rust
// tests/integration/e2e_index_building.rs

#[test]
fn test_e2e_build_attention_enhanced_index() {
    // Complete workflow: data loading -> training -> index building

    println!("Step 1: Load dataset");
    let dataset = load_dataset("datasets/wikipedia_embeddings.bin");
    let num_vectors = dataset.len();
    let dim = dataset[0].len();
    println!("  Loaded {} vectors of dimension {}", num_vectors, dim);

    println!("\nStep 2: Train attention model");
    let attention_config = AttentionConfig {
        num_heads: 8,
        embed_dim: dim,
        dropout: 0.1,
        attention_type: AttentionType::GraphAttention,
    };

    let mut attention_model = AttentionLayer::new(attention_config);

    // Train on random batches
    let num_epochs = 50;
    let batch_size = 128;

    for epoch in 0..num_epochs {
        let mut epoch_loss = 0.0;
        let num_batches = num_vectors / batch_size;

        for batch_idx in 0..num_batches {
            let start = batch_idx * batch_size;
            let end = (start + batch_size).min(num_vectors);

            let batch = stack_vectors(&dataset[start..end]);
            let output = attention_model.forward(&batch);

            // Self-supervised loss
            let loss = contrastive_loss(&output, &batch);
            epoch_loss += loss;

            let grad = compute_gradient(&output, &batch);
            attention_model.backward(&grad);
            attention_model.update_weights(0.001);
        }

        if epoch % 10 == 0 {
            println!("  Epoch {}: Loss = {:.6}", epoch, epoch_loss / num_batches as f32);
        }
    }

    println!("\nStep 3: Build HNSW index with learned attention");

    // Transform vectors using trained attention
    let transformed_vectors: Vec<_> = dataset.iter()
        .map(|vec| {
            let input = Array2::from_shape_vec((1, dim), vec.clone()).unwrap();
            let output = attention_model.forward_single(&input);
            output.to_vec()
        })
        .collect();

    let mut index = HNSWIndex::new(dim, IndexConfig {
        m: 32,
        ef_construction: 200,
        max_elements: num_vectors,
        distance_type: DistanceType::Cosine,
    });

    for (i, vec) in transformed_vectors.iter().enumerate() {
        index.add_vector(i, vec);

        if i % 10000 == 0 {
            println!("  Indexed {} / {} vectors", i, num_vectors);
        }
    }

    println!("\nStep 4: Evaluate search quality");

    // Test queries
    let num_queries = 100;
    let k = 10;

    let mut recall_scores = Vec::new();

    for query_idx in 0..num_queries {
        let query = &transformed_vectors[query_idx];

        // Search in attention-enhanced index
        let results = index.search(query, SearchConfig { ef: 100, k });

        // Compare to ground truth (brute force on original vectors)
        let ground_truth = brute_force_search(&dataset[query_idx], &dataset, k);

        // Calculate recall
        let recall = calculate_recall(&results, &ground_truth);
        recall_scores.push(recall);
    }

    let avg_recall = recall_scores.iter().sum::<f32>() / recall_scores.len() as f32;

    println!("  Average Recall@{}: {:.4}", k, avg_recall);
    assert!(avg_recall > 0.9, "Recall should be > 0.9");

    println!("\nStep 5: Save index and model");
    index.save("output/attention_enhanced_index.hnsw");
    attention_model.save("output/attention_model.bin");

    println!("\n✓ End-to-end index building complete");
}

### 4.2 Search with Various Attention Types

```rust
// tests/integration/e2e_search.rs

#[test]
fn test_e2e_multi_attention_search() {
    println!("=== Multi-Attention Search Comparison ===\n");

    // Load pre-built index
    let index = HNSWIndex::load("output/wikipedia_index.hnsw");
    let vectors = load_vectors("datasets/wikipedia_embeddings.bin");

    let query = vectors[0].clone();
    let k = 20;

    // Define attention variants to test
    let attention_types = vec![
        ("No Attention", None),
        ("Multi-Head", Some(AttentionType::MultiHead)),
        ("Graph", Some(AttentionType::GraphAttention)),
        ("Cross", Some(AttentionType::CrossAttention)),
        ("Sparse", Some(AttentionType::SparseAttention)),
        ("Hyperbolic", Some(AttentionType::Hyperbolic)),
        ("MoE", Some(AttentionType::MixtureOfExperts)),
    ];

    let mut results_comparison = Vec::new();

    for (name, attention_type) in attention_types {
        println!("Testing: {}", name);

        let start_time = Instant::now();

        let results = if let Some(attn_type) = attention_type {
            let config = AttentionConfig {
                num_heads: 8,
                embed_dim: vectors[0].len(),
                dropout: 0.0,
                attention_type: attn_type,
            };

            let attention_search = AttentionGuidedSearch::new(config);
            attention_search.search(&index, &query, SearchConfig { ef: 100, k }, &vectors)
        } else {
            index.search(&query, SearchConfig { ef: 100, k })
        };

        let search_time = start_time.elapsed();

        // Calculate metrics
        let ground_truth = brute_force_search(&query, &vectors, k);
        let recall = calculate_recall(&results, &ground_truth);
        let ndcg = calculate_ndcg(&results, &ground_truth);

        println!("  Recall@{}: {:.4}", k, recall);
        println!("  NDCG@{}: {:.4}", k, ndcg);
        println!("  Search time: {:.2}ms", search_time.as_millis());
        println!();

        results_comparison.push((name, recall, ndcg, search_time));
    }

    // Summary comparison
    println!("=== Summary ===");
    println!("{:<20} {:>10} {:>10} {:>12}", "Type", "Recall", "NDCG", "Time (ms)");
    println!("{:-<54}", "");

    for (name, recall, ndcg, time) in results_comparison {
        println!("{:<20} {:>10.4} {:>10.4} {:>12.2}",
            name, recall, ndcg, time.as_millis());
    }
}

#[test]
fn test_e2e_hybrid_search_pipeline() {
    // Multi-stage search pipeline
    println!("=== Hybrid Search Pipeline ===\n");

    let index = HNSWIndex::load("output/large_index.hnsw");
    let vectors = load_vectors("datasets/embeddings.bin");
    let query = vectors[0].clone();

    println!("Stage 1: Fast HNSW retrieval");
    let start = Instant::now();
    let candidates = index.search(&query, SearchConfig { ef: 500, k: 500 });
    println!("  Retrieved {} candidates in {:.2}ms",
        candidates.len(), start.elapsed().as_millis());

    println!("\nStage 2: Attention-based reranking");
    let rerank_config = AttentionConfig {
        num_heads: 16,
        embed_dim: vectors[0].len(),
        dropout: 0.0,
        attention_type: AttentionType::CrossAttention,
    };

    let start = Instant::now();
    let reranker = AttentionReranker::new(rerank_config);

    let candidate_vectors: Vec<_> = candidates.iter()
        .map(|(id, _)| vectors[*id].clone())
        .collect();

    let reranked = reranker.rerank(&query, &candidate_vectors, 100);
    println!("  Reranked to {} results in {:.2}ms",
        reranked.len(), start.elapsed().as_millis());

    println!("\nStage 3: Diversity filtering");
    let start = Instant::now();
    let diverse_results = diversity_filter(&reranked, &candidate_vectors, 20, 0.7);
    println!("  Filtered to {} diverse results in {:.2}ms",
        diverse_results.len(), start.elapsed().as_millis());

    println!("\nFinal Results:");
    for (rank, (id, score)) in diverse_results.iter().take(10).enumerate() {
        println!("  {}: ID={}, score={:.6}", rank + 1, id, score);
    }
}

### 4.3 Training and Inference

```rust
// tests/integration/e2e_training_inference.rs

#[test]
fn test_e2e_attention_training_deployment() {
    println!("=== Attention Training & Deployment Pipeline ===\n");

    // Phase 1: Training
    println!("Phase 1: Training");

    let train_data = load_dataset("datasets/train.bin");
    let val_data = load_dataset("datasets/val.bin");

    let config = AttentionConfig {
        num_heads: 8,
        embed_dim: 256,
        dropout: 0.1,
        attention_type: AttentionType::MultiHead,
    };

    let mut model = AttentionLayer::new(config);
    let optimizer = Adam::new(0.0001);

    let num_epochs = 100;
    let batch_size = 64;

    let mut best_val_loss = f32::INFINITY;
    let mut patience = 0;
    let max_patience = 10;

    for epoch in 0..num_epochs {
        // Training
        let train_loss = train_epoch(&mut model, &train_data, batch_size, &optimizer);

        // Validation
        let val_loss = validate(&model, &val_data, batch_size);

        println!("Epoch {}: train_loss={:.6}, val_loss={:.6}",
            epoch, train_loss, val_loss);

        // Early stopping
        if val_loss < best_val_loss {
            best_val_loss = val_loss;
            patience = 0;
            model.save("checkpoints/best_model.bin");
        } else {
            patience += 1;
            if patience >= max_patience {
                println!("Early stopping at epoch {}", epoch);
                break;
            }
        }
    }

    // Phase 2: Export for deployment
    println!("\nPhase 2: Export");

    // Load best model
    model = AttentionLayer::load("checkpoints/best_model.bin");

    // Export to different formats
    println!("  Exporting to ONNX...");
    model.export_onnx("deploy/model.onnx");

    println!("  Exporting to TorchScript...");
    model.export_torchscript("deploy/model.pt");

    println!("  Exporting quantized version...");
    let quantized = model.quantize_int8();
    quantized.save("deploy/model_int8.bin");

    // Phase 3: Inference benchmarking
    println!("\nPhase 3: Inference Benchmarking");

    let test_data = load_dataset("datasets/test.bin");
    let test_queries = &test_data[..1000];

    // Benchmark full precision
    let start = Instant::now();
    for query in test_queries {
        let _ = model.forward_single(query);
    }
    let fp32_time = start.elapsed();
    let fp32_latency = fp32_time.as_micros() as f32 / 1000.0;

    println!("  FP32 latency: {:.2}ms per query", fp32_latency);

    // Benchmark quantized
    let start = Instant::now();
    for query in test_queries {
        let _ = quantized.forward_single(query);
    }
    let int8_time = start.elapsed();
    let int8_latency = int8_time.as_micros() as f32 / 1000.0;

    println!("  INT8 latency: {:.2}ms per query", int8_latency);
    println!("  Speedup: {:.2}x", fp32_latency / int8_latency);

    // Accuracy comparison
    let mut accuracy_diffs = Vec::new();

    for query in test_queries.iter().take(100) {
        let fp32_output = model.forward_single(query);
        let int8_output = quantized.forward_single(query);

        let diff = compute_cosine_similarity(&fp32_output, &int8_output);
        accuracy_diffs.push(diff);
    }

    let avg_similarity = accuracy_diffs.iter().sum::<f32>() / accuracy_diffs.len() as f32;
    println!("  FP32 vs INT8 similarity: {:.6}", avg_similarity);

    // Phase 4: Production deployment
    println!("\nPhase 4: Production Deployment");

    println!("  Creating production config...");
    let prod_config = ProductionConfig {
        model_path: "deploy/model_int8.bin",
        batch_size: 128,
        num_threads: 8,
        use_gpu: false,
        max_latency_ms: 10.0,
    };
    prod_config.save("deploy/config.json");

    println!("  Setting up serving endpoint...");
    let server = AttentionServer::new(prod_config);

    // Simulate production load
    let num_requests = 10000;
    let concurrency = 100;

    println!("  Simulating {} requests with concurrency {}",
        num_requests, concurrency);

    let start = Instant::now();
    let latencies = simulate_production_load(&server, num_requests, concurrency);
    let total_time = start.elapsed();

    let throughput = num_requests as f32 / total_time.as_secs_f32();
    let p50 = percentile(&latencies, 0.5);
    let p95 = percentile(&latencies, 0.95);
    let p99 = percentile(&latencies, 0.99);

    println!("  Throughput: {:.2} req/s", throughput);
    println!("  Latency P50: {:.2}ms", p50);
    println!("  Latency P95: {:.2}ms", p95);
    println!("  Latency P99: {:.2}ms", p99);

    assert!(p99 < prod_config.max_latency_ms,
        "P99 latency exceeds SLA: {:.2}ms", p99);

    println!("\n✓ Training and deployment pipeline complete");
}

## Test Utilities

```rust
// tests/integration/utils.rs

// Helper functions for integration tests

pub fn create_test_graph(num_nodes: usize, num_neighbors: usize) -> Graph {
    let mut edges = Vec::new();

    for i in 0..num_nodes {
        for j in 0..num_neighbors {
            let neighbor = (i + j + 1) % num_nodes;
            edges.push((i, neighbor));
        }
    }

    Graph::from_edges(num_nodes, &edges)
}

pub fn generate_clustered_vectors(
    num_vectors: usize,
    dim: usize,
    num_clusters: usize
) -> Vec<Array1<f32>> {
    let mut rng = rand::thread_rng();
    let mut vectors = Vec::new();

    // Create cluster centers
    let centers: Vec<Array1<f32>> = (0..num_clusters)
        .map(|_| Array1::random(dim, Uniform::new(-1.0, 1.0)))
        .collect();

    // Generate vectors around centers
    for _ in 0..num_vectors {
        let cluster = rng.gen_range(0..num_clusters);
        let center = &centers[cluster];

        let noise: Array1<f32> = Array1::random(dim, Uniform::new(-0.1, 0.1));
        let vector = center + &noise;

        vectors.push(vector);
    }

    vectors
}

pub fn calculate_recall(results: &[(usize, f32)], ground_truth: &[(usize, f32)]) -> f32 {
    let result_ids: HashSet<usize> = results.iter().map(|(id, _)| *id).collect();
    let truth_ids: HashSet<usize> = ground_truth.iter().map(|(id, _)| *id).collect();

    let intersection = result_ids.intersection(&truth_ids).count();
    intersection as f32 / ground_truth.len() as f32
}

pub fn calculate_ndcg(results: &[(usize, f32)], ground_truth: &[(usize, f32)]) -> f32 {
    let truth_map: HashMap<usize, f32> = ground_truth.iter()
        .enumerate()
        .map(|(rank, (id, _))| (*id, 1.0 / (rank + 1) as f32))
        .collect();

    let dcg: f32 = results.iter()
        .enumerate()
        .map(|(rank, (id, _))| {
            let relevance = truth_map.get(id).unwrap_or(&0.0);
            relevance / (rank + 2) as f32.log2()
        })
        .sum();

    let idcg: f32 = (0..results.len())
        .map(|rank| 1.0 / (rank + 2) as f32.log2())
        .sum();

    dcg / idcg
}

pub fn mse_loss(output: &Array3<f32>, target: &Array3<f32>) -> f32 {
    let diff = output - target;
    diff.mapv(|x| x * x).mean().unwrap()
}

pub fn contrastive_loss(output: &Array3<f32>, target: &Array3<f32>) -> f32 {
    // Simplified contrastive loss
    let batch_size = output.shape()[0];
    let mut loss = 0.0;

    for i in 0..batch_size {
        let out_i = output.slice(s![i, .., ..]);
        let target_i = target.slice(s![i, .., ..]);

        let pos_sim = cosine_similarity(&out_i, &target_i);

        for j in 0..batch_size {
            if i != j {
                let target_j = target.slice(s![j, .., ..]);
                let neg_sim = cosine_similarity(&out_i, &target_j);
                loss += (1.0 - pos_sim + neg_sim).max(0.0);
            }
        }
    }

    loss / (batch_size * (batch_size - 1)) as f32
}

## Summary

This integration test suite provides comprehensive coverage of:

1. **GNN Integration**: Attention as graph layers, training loops, gradient verification
2. **Core Integration**: HNSW search enhancement, learned metrics
3. **Platform Consistency**: Cross-platform numerical verification with defined tolerances
4. **E2E Workflows**: Complete pipelines from training to production deployment

**Testing Strategy**:
- Unit-level integration: Component interactions
- System-level integration: Full workflow testing
- Platform verification: Cross-platform consistency
- Performance validation: Benchmarking and SLA verification

**Key Metrics**:
- Recall@K > 0.9
- NDCG@K > 0.85
- Cross-platform tolerance < 1e-5
- P99 latency < 10ms (production)
- Training convergence within 100 epochs

**Execution**:
```bash
# Run all integration tests
cargo test --test integration -- --test-threads=1

# Run specific integration suites
cargo test --test integration::gnn_attention
cargo test --test integration::hnsw_attention
cargo test --test integration::cross_platform
cargo test --test integration::e2e_workflows

# Run with detailed output
cargo test --test integration -- --nocapture
```
