use crate::layer::RuvectorLayer;

/// Compute cosine similarity between two vectors with improved precision
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vectors must have the same length");

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();

    // Use f64 accumulator for better precision in norm computation
    let norm_a: f32 = (a
        .iter()
        .map(|&x| (x as f64) * (x as f64))
        .sum::<f64>()
        .sqrt()) as f32;
    let norm_b: f32 = (b
        .iter()
        .map(|&x| (x as f64) * (x as f64))
        .sum::<f64>()
        .sqrt()) as f32;

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

/// Apply softmax with temperature scaling
fn softmax(values: &[f32], temperature: f32) -> Vec<f32> {
    if values.is_empty() {
        return Vec::new();
    }

    // Scale by temperature and subtract max for numerical stability
    let max_val = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp_values: Vec<f32> = values
        .iter()
        .map(|&x| ((x - max_val) / temperature).exp())
        .collect();

    let sum: f32 = exp_values.iter().sum::<f32>().max(1e-10);

    exp_values.iter().map(|&x| x / sum).collect()
}

/// Differentiable search using soft attention mechanism
///
/// # Arguments
/// * `query` - The query vector
/// * `candidate_embeddings` - List of candidate embedding vectors
/// * `k` - Number of top results to return
/// * `temperature` - Temperature for softmax (lower = sharper, higher = smoother)
///
/// # Returns
/// * Tuple of (indices, soft_weights) for top-k candidates
pub fn differentiable_search(
    query: &[f32],
    candidate_embeddings: &[Vec<f32>],
    k: usize,
    temperature: f32,
) -> (Vec<usize>, Vec<f32>) {
    if candidate_embeddings.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let k = k.min(candidate_embeddings.len());

    // 1. Compute similarities using cosine similarity
    let similarities: Vec<f32> = candidate_embeddings
        .iter()
        .map(|embedding| cosine_similarity(query, embedding))
        .collect();

    // 2. Apply softmax with temperature to get soft weights
    let soft_weights = softmax(&similarities, temperature);

    // 3. Get top-k indices by sorting similarities
    let mut indexed_weights: Vec<(usize, f32)> = soft_weights
        .iter()
        .enumerate()
        .map(|(i, &w)| (i, w))
        .collect();

    // Sort by weight descending
    indexed_weights.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Take top-k
    let top_k: Vec<(usize, f32)> = indexed_weights.into_iter().take(k).collect();

    let indices: Vec<usize> = top_k.iter().map(|&(i, _)| i).collect();
    let weights: Vec<f32> = top_k.iter().map(|&(_, w)| w).collect();

    (indices, weights)
}

/// Hierarchical forward pass through GNN layers
///
/// # Arguments
/// * `query` - The query vector
/// * `layer_embeddings` - Embeddings organized by layer (outer vec = layers, inner vec = nodes per layer)
/// * `gnn_layers` - The GNN layers to process through
///
/// # Returns
/// * Final embedding after hierarchical processing
pub fn hierarchical_forward(
    query: &[f32],
    layer_embeddings: &[Vec<Vec<f32>>],
    gnn_layers: &[RuvectorLayer],
) -> Vec<f32> {
    if layer_embeddings.is_empty() || gnn_layers.is_empty() {
        return query.to_vec();
    }

    let mut current_embedding = query.to_vec();

    // Process through each layer from top to bottom
    for (layer_idx, (embeddings, gnn_layer)) in
        layer_embeddings.iter().zip(gnn_layers.iter()).enumerate()
    {
        if embeddings.is_empty() {
            continue;
        }

        // Find most relevant nodes at this layer using differentiable search
        let (top_indices, weights) = differentiable_search(
            &current_embedding,
            embeddings,
            5.min(embeddings.len()), // Top-5 or all if less
            1.0,                     // Default temperature
        );

        // Aggregate embeddings from top nodes using soft weights
        let mut aggregated = vec![0.0; current_embedding.len()];
        for (&idx, &weight) in top_indices.iter().zip(weights.iter()) {
            for (i, &val) in embeddings[idx].iter().enumerate() {
                if i < aggregated.len() {
                    aggregated[i] += weight * val;
                }
            }
        }

        // Combine with current embedding
        let combined: Vec<f32> = current_embedding
            .iter()
            .zip(&aggregated)
            .map(|(curr, agg)| (curr + agg) / 2.0)
            .collect();

        // Apply GNN layer transformation
        // Extract neighbor embeddings and compute edge weights
        let neighbor_embs: Vec<Vec<f32>> = top_indices
            .iter()
            .map(|&idx| embeddings[idx].clone())
            .collect();

        let edge_weights_vec: Vec<f32> = weights.clone();

        current_embedding = gnn_layer.forward(&combined, &neighbor_embs, &edge_weights_vec);
    }

    current_embedding
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![1.0, 0.0, 0.0];
        let d = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&c, &d) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_softmax() {
        let values = vec![1.0, 2.0, 3.0];
        let result = softmax(&values, 1.0);

        // Sum should be 1.0
        let sum: f32 = result.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);

        // Higher values should have higher probabilities
        assert!(result[2] > result[1]);
        assert!(result[1] > result[0]);
    }

    #[test]
    fn test_softmax_with_temperature() {
        let values = vec![1.0, 2.0, 3.0];

        // Lower temperature = sharper distribution
        let sharp = softmax(&values, 0.1);
        let smooth = softmax(&values, 10.0);

        // Sharp should have more weight on max
        assert!(sharp[2] > smooth[2]);
    }

    #[test]
    fn test_differentiable_search() {
        let query = vec![1.0, 0.0, 0.0];
        let candidates = vec![
            vec![1.0, 0.0, 0.0], // Perfect match
            vec![0.9, 0.1, 0.0], // Close match
            vec![0.0, 1.0, 0.0], // Orthogonal
        ];

        let (indices, weights) = differentiable_search(&query, &candidates, 2, 1.0);

        assert_eq!(indices.len(), 2);
        assert_eq!(weights.len(), 2);

        // First result should be the perfect match
        assert_eq!(indices[0], 0);

        // Weights should sum to less than or equal to 1.0 (since we took top-k)
        let sum: f32 = weights.iter().sum();
        assert!(sum <= 1.0 + 1e-6);
    }

    #[test]
    fn test_hierarchical_forward() {
        // Use consistent dimensions throughout
        let query = vec![1.0, 0.0];

        // Layer embeddings should match the output dimensions of each layer
        let layer_embeddings = vec![
            // First layer: embeddings are 2-dimensional (match query)
            vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        ];

        // Single GNN layer that maintains dimension
        let gnn_layers = vec![
            RuvectorLayer::new(2, 2, 1, 0.0).unwrap(), // input_dim, hidden_dim, heads, dropout
        ];

        let result = hierarchical_forward(&query, &layer_embeddings, &gnn_layers);

        assert_eq!(result.len(), 2); // Should match hidden_dim of last layer
    }
}
