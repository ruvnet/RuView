//! Loss functions for contrastive learning.
//!
//! This module provides various loss functions for training GNN models
//! on graph-structured data with contrastive learning objectives.


/// Compute InfoNCE (Noise Contrastive Estimation) loss.
///
/// InfoNCE loss encourages the model to distinguish positive samples
/// from negative samples in the embedding space.
///
/// # Arguments
/// * `anchor` - The anchor embedding
/// * `positive` - The positive (similar) embedding
/// * `negatives` - Slice of negative (dissimilar) embeddings
/// * `temperature` - Temperature parameter for softmax scaling (typical: 0.07-0.5)
///
/// # Returns
/// The InfoNCE loss value (lower is better)
///
/// # Formula
/// L = -log(exp(sim(a,p)/τ) / Σ exp(sim(a,n_i)/τ))
///
/// # Example
/// ```
/// use sevensense_learning::info_nce_loss;
///
/// let anchor = vec![1.0, 0.0, 0.0];
/// let positive = vec![0.9, 0.1, 0.0];
/// let negative = vec![0.0, 1.0, 0.0];
///
/// let loss = info_nce_loss(&anchor, &positive, &[&negative], 0.07);
/// assert!(loss >= 0.0);
/// ```
#[must_use]
pub fn info_nce_loss(
    anchor: &[f32],
    positive: &[f32],
    negatives: &[&[f32]],
    temperature: f32,
) -> f32 {
    if anchor.is_empty() || positive.is_empty() || negatives.is_empty() {
        return 0.0;
    }

    let temp = temperature.max(1e-6); // Prevent division by zero

    // Compute similarity with positive
    let pos_sim = cosine_similarity(anchor, positive) / temp;

    // Compute similarities with all negatives
    let neg_sims: Vec<f32> = negatives
        .iter()
        .map(|neg| cosine_similarity(anchor, neg) / temp)
        .collect();

    // Log-sum-exp for numerical stability
    // L = -pos_sim + log(exp(pos_sim) + Σ exp(neg_sim_i))
    let max_sim = neg_sims
        .iter()
        .chain(std::iter::once(&pos_sim))
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);

    let sum_exp: f32 = std::iter::once(pos_sim)
        .chain(neg_sims)
        .map(|s| (s - max_sim).exp())
        .sum();

    let log_sum_exp = max_sim + sum_exp.ln();

    -pos_sim + log_sum_exp
}

/// Compute triplet loss with margin.
///
/// Triplet loss ensures that the anchor is closer to the positive
/// than to the negative by at least a margin.
///
/// # Arguments
/// * `anchor` - The anchor embedding
/// * `positive` - The positive (similar) embedding
/// * `negative` - The negative (dissimilar) embedding
/// * `margin` - The margin to enforce between positive and negative distances
///
/// # Returns
/// The triplet loss value (lower is better)
///
/// # Formula
/// L = max(0, d(a,p) - d(a,n) + margin)
///
/// # Example
/// ```
/// use sevensense_learning::triplet_loss;
///
/// let anchor = vec![1.0, 0.0, 0.0];
/// let positive = vec![0.9, 0.1, 0.0];
/// let negative = vec![0.0, 1.0, 0.0];
///
/// let loss = triplet_loss(&anchor, &positive, &negative, 1.0);
/// assert!(loss >= 0.0);
/// ```
#[must_use]
pub fn triplet_loss(anchor: &[f32], positive: &[f32], negative: &[f32], margin: f32) -> f32 {
    if anchor.is_empty() || positive.is_empty() || negative.is_empty() {
        return 0.0;
    }

    let d_pos = euclidean_distance(anchor, positive);
    let d_neg = euclidean_distance(anchor, negative);

    (d_pos - d_neg + margin).max(0.0)
}

/// Compute margin ranking loss.
///
/// Similar to triplet loss but uses a ranking formulation.
///
/// # Arguments
/// * `anchor` - The anchor embedding
/// * `positive` - The positive embedding
/// * `negative` - The negative embedding
/// * `margin` - The margin for ranking
///
/// # Returns
/// The margin ranking loss value
///
/// # Formula
/// L = max(0, margin - (sim(a,p) - sim(a,n)))
#[must_use]
pub fn margin_ranking_loss(
    anchor: &[f32],
    positive: &[f32],
    negative: &[f32],
    margin: f32,
) -> f32 {
    if anchor.is_empty() || positive.is_empty() || negative.is_empty() {
        return 0.0;
    }

    let sim_pos = cosine_similarity(anchor, positive);
    let sim_neg = cosine_similarity(anchor, negative);

    (margin - (sim_pos - sim_neg)).max(0.0)
}

/// Compute contrastive loss (SimCLR style).
///
/// # Arguments
/// * `z_i` - First view embedding
/// * `z_j` - Second view embedding (augmented view of same sample)
/// * `other_samples` - Embeddings of other samples in the batch
/// * `temperature` - Temperature parameter
///
/// # Returns
/// The contrastive loss value
#[must_use]
pub fn contrastive_loss(
    z_i: &[f32],
    z_j: &[f32],
    other_samples: &[&[f32]],
    temperature: f32,
) -> f32 {
    if z_i.is_empty() || z_j.is_empty() {
        return 0.0;
    }

    let temp = temperature.max(1e-6);

    // Similarity between positive pair
    let pos_sim = cosine_similarity(z_i, z_j) / temp;

    // Similarities with all other samples
    let mut all_sims: Vec<f32> = vec![pos_sim];
    for sample in other_samples {
        all_sims.push(cosine_similarity(z_i, sample) / temp);
    }

    // Log-sum-exp trick
    let max_sim = all_sims.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let sum_exp: f32 = all_sims.iter().map(|s| (s - max_sim).exp()).sum();
    let log_sum_exp = max_sim + sum_exp.ln();

    -pos_sim + log_sum_exp
}

/// Compute NT-Xent loss (Normalized Temperature-scaled Cross Entropy).
///
/// This is the loss function used in SimCLR.
///
/// # Arguments
/// * `embeddings` - All embeddings in the batch (2N for N samples with 2 views each)
/// * `temperature` - Temperature parameter
///
/// # Returns
/// The average NT-Xent loss across all positive pairs
#[must_use]
pub fn nt_xent_loss(embeddings: &[Vec<f32>], temperature: f32) -> f32 {
    let n = embeddings.len();
    if n < 2 {
        return 0.0;
    }

    let temp = temperature.max(1e-6);

    // Assume embeddings are organized as [z_1_a, z_1_b, z_2_a, z_2_b, ...]
    // where z_i_a and z_i_b are two views of sample i

    // Compute all pairwise similarities
    let mut sim_matrix = vec![vec![0.0f32; n]; n];
    for i in 0..n {
        for j in 0..n {
            sim_matrix[i][j] = cosine_similarity(&embeddings[i], &embeddings[j]) / temp;
        }
    }

    let mut total_loss = 0.0;
    let mut count = 0;

    // For each sample, compute loss against its positive pair
    for i in 0..n {
        // Find positive pair (assumes alternating views)
        let j = if i % 2 == 0 { i + 1 } else { i - 1 };
        if j >= n {
            continue;
        }

        let pos_sim = sim_matrix[i][j];

        // Sum of all negative similarities (excluding self and positive)
        let max_sim = sim_matrix[i]
            .iter()
            .enumerate()
            .filter(|(k, _)| *k != i)
            .map(|(_, &s)| s)
            .fold(f32::NEG_INFINITY, f32::max);

        let sum_exp: f32 = sim_matrix[i]
            .iter()
            .enumerate()
            .filter(|(k, _)| *k != i)
            .map(|(_, &s)| (s - max_sim).exp())
            .sum();

        let log_sum_exp = max_sim + sum_exp.ln();

        total_loss += -pos_sim + log_sum_exp;
        count += 1;
    }

    if count > 0 {
        total_loss / count as f32
    } else {
        0.0
    }
}

/// Compute supervised contrastive loss (SupCon).
///
/// Extends contrastive loss to use label information.
///
/// # Arguments
/// * `embeddings` - All embeddings
/// * `labels` - Label for each embedding
/// * `temperature` - Temperature parameter
///
/// # Returns
/// The supervised contrastive loss
#[must_use]
pub fn supervised_contrastive_loss(
    embeddings: &[Vec<f32>],
    labels: &[usize],
    temperature: f32,
) -> f32 {
    let n = embeddings.len();
    if n < 2 || n != labels.len() {
        return 0.0;
    }

    let temp = temperature.max(1e-6);

    // Compute all pairwise similarities
    let mut sim_matrix = vec![vec![0.0f32; n]; n];
    for i in 0..n {
        for j in 0..n {
            sim_matrix[i][j] = cosine_similarity(&embeddings[i], &embeddings[j]) / temp;
        }
    }

    let mut total_loss = 0.0;

    for i in 0..n {
        // Find all positive pairs (same label, excluding self)
        let positives: Vec<usize> = (0..n)
            .filter(|&j| j != i && labels[j] == labels[i])
            .collect();

        if positives.is_empty() {
            continue;
        }

        // Compute denominator (all except self)
        let max_sim = sim_matrix[i]
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, &s)| s)
            .fold(f32::NEG_INFINITY, f32::max);

        let denom_exp: f32 = sim_matrix[i]
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, &s)| (s - max_sim).exp())
            .sum();

        let log_denom = max_sim + denom_exp.ln();

        // Average over all positive pairs
        let pos_loss: f32 = positives
            .iter()
            .map(|&j| -sim_matrix[i][j] + log_denom)
            .sum();

        total_loss += pos_loss / positives.len() as f32;
    }

    total_loss / n as f32
}

/// Compute center loss.
///
/// Encourages embeddings to be close to their class centers.
///
/// # Arguments
/// * `embeddings` - Embeddings
/// * `labels` - Class labels
/// * `centers` - Class center embeddings
///
/// # Returns
/// The center loss value
#[must_use]
pub fn center_loss(
    embeddings: &[Vec<f32>],
    labels: &[usize],
    centers: &[Vec<f32>],
) -> f32 {
    if embeddings.is_empty() || embeddings.len() != labels.len() {
        return 0.0;
    }

    let mut total_loss = 0.0;

    for (emb, &label) in embeddings.iter().zip(labels.iter()) {
        if label < centers.len() {
            let dist = euclidean_distance(emb, &centers[label]);
            total_loss += dist * dist;
        }
    }

    total_loss / (2.0 * embeddings.len() as f32)
}

/// Compute focal loss (for imbalanced classification).
///
/// # Arguments
/// * `predictions` - Predicted probabilities
/// * `targets` - Target labels
/// * `gamma` - Focusing parameter (typical: 2.0)
/// * `alpha` - Class weighting parameter
///
/// # Returns
/// The focal loss value
#[must_use]
pub fn focal_loss(
    predictions: &[f32],
    targets: &[usize],
    gamma: f32,
    alpha: f32,
) -> f32 {
    if predictions.is_empty() || predictions.len() != targets.len() {
        return 0.0;
    }

    let eps = 1e-7;
    let mut total_loss = 0.0;

    for (&pred, &target) in predictions.iter().zip(targets.iter()) {
        let p = pred.clamp(eps, 1.0 - eps);
        let pt = if target == 1 { p } else { 1.0 - p };
        let at = if target == 1 { alpha } else { 1.0 - alpha };

        let loss = -at * (1.0 - pt).powf(gamma) * pt.ln();
        total_loss += loss;
    }

    total_loss / predictions.len() as f32
}

// =========== Helper Functions ===========

/// Compute cosine similarity between two vectors
#[inline]
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < 1e-10 || norm_b < 1e-10 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Compute Euclidean distance between two vectors
#[inline]
fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return f32::INFINITY;
    }

    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

/// Compute squared Euclidean distance (faster, no sqrt)
#[inline]
#[allow(dead_code)]
fn squared_euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return f32::INFINITY;
    }

    a.iter().zip(b).map(|(x, y)| (x - y).powi(2)).sum()
}

/// Compute dot product
#[inline]
#[allow(dead_code)]
fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// L2 normalize a vector
#[must_use]
pub fn l2_normalize(v: &[f32]) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm < 1e-10 {
        return v.to_vec();
    }
    v.iter().map(|x| x / norm).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_nce_loss() {
        let anchor = vec![1.0, 0.0, 0.0];
        let positive = vec![0.95, 0.05, 0.0];
        let negative1 = vec![0.0, 1.0, 0.0];
        let negative2 = vec![0.0, 0.0, 1.0];

        let loss = info_nce_loss(&anchor, &positive, &[&negative1, &negative2], 0.1);
        assert!(loss >= 0.0);

        // Similar positive should have lower loss
        let similar_positive = vec![0.99, 0.01, 0.0];
        let lower_loss = info_nce_loss(&anchor, &similar_positive, &[&negative1, &negative2], 0.1);
        assert!(lower_loss < loss);
    }

    #[test]
    fn test_triplet_loss() {
        let anchor = vec![1.0, 0.0, 0.0];
        let positive = vec![0.9, 0.1, 0.0];
        let negative = vec![0.0, 1.0, 0.0];

        let loss = triplet_loss(&anchor, &positive, &negative, 1.0);
        assert!(loss >= 0.0);

        // When positive is very similar, loss should be lower
        let close_positive = vec![0.99, 0.01, 0.0];
        let lower_loss = triplet_loss(&anchor, &close_positive, &negative, 1.0);
        assert!(lower_loss <= loss);
    }

    #[test]
    fn test_margin_ranking_loss() {
        let anchor = vec![1.0, 0.0, 0.0];
        let positive = vec![0.9, 0.1, 0.0];
        let negative = vec![0.0, 1.0, 0.0];

        let loss = margin_ranking_loss(&anchor, &positive, &negative, 0.5);
        assert!(loss >= 0.0);
    }

    #[test]
    fn test_contrastive_loss() {
        let z_i = vec![1.0, 0.0, 0.0];
        let z_j = vec![0.95, 0.05, 0.0];
        let other1 = vec![0.0, 1.0, 0.0];
        let other2 = vec![0.0, 0.0, 1.0];

        let loss = contrastive_loss(&z_i, &z_j, &[&other1, &other2], 0.1);
        assert!(loss >= 0.0);
    }

    #[test]
    fn test_nt_xent_loss() {
        let embeddings = vec![
            vec![1.0, 0.0],
            vec![0.95, 0.05], // positive pair with first
            vec![0.0, 1.0],
            vec![0.05, 0.95], // positive pair with third
        ];

        let loss = nt_xent_loss(&embeddings, 0.5);
        assert!(loss >= 0.0);
    }

    #[test]
    fn test_supervised_contrastive_loss() {
        let embeddings = vec![
            vec![1.0, 0.0],
            vec![0.9, 0.1],  // same class as first
            vec![0.0, 1.0],
            vec![0.1, 0.9],  // same class as third
        ];
        let labels = vec![0, 0, 1, 1];

        let loss = supervised_contrastive_loss(&embeddings, &labels, 0.1);
        assert!(loss >= 0.0);
    }

    #[test]
    fn test_center_loss() {
        let embeddings = vec![
            vec![1.0, 0.0],
            vec![0.9, 0.1],
            vec![0.0, 1.0],
        ];
        let labels = vec![0, 0, 1];
        let centers = vec![
            vec![0.95, 0.05], // center for class 0
            vec![0.05, 0.95], // center for class 1
        ];

        let loss = center_loss(&embeddings, &labels, &centers);
        assert!(loss >= 0.0);
    }

    #[test]
    fn test_focal_loss() {
        let predictions = vec![0.9, 0.1, 0.8, 0.2];
        let targets = vec![1, 0, 1, 0];

        let loss = focal_loss(&predictions, &targets, 2.0, 0.25);
        assert!(loss >= 0.0);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 1e-6);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        assert!((euclidean_distance(&a, &b) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize() {
        let v = vec![3.0, 4.0];
        let normalized = l2_normalize(&v);

        let norm: f32 = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);

        assert!((normalized[0] - 0.6).abs() < 1e-6);
        assert!((normalized[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_empty_inputs() {
        let empty: Vec<f32> = vec![];
        let valid = vec![1.0, 0.0, 0.0];

        assert_eq!(info_nce_loss(&empty, &valid, &[&valid], 0.1), 0.0);
        assert_eq!(triplet_loss(&empty, &valid, &valid, 1.0), 0.0);
        assert_eq!(contrastive_loss(&empty, &valid, &[], 0.1), 0.0);
    }
}
