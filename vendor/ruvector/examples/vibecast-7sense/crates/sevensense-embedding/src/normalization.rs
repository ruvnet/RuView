//! Normalization utilities for embedding vectors.
//!
//! Provides L2 normalization and validation functions to ensure
//! embedding vectors are properly normalized for cosine similarity
//! operations in vector databases.

use crate::EMBEDDING_DIM;

/// L2 normalize an embedding vector in-place.
///
/// After normalization, the vector will have unit length (norm = 1.0),
/// enabling cosine similarity to be computed as a simple dot product.
///
/// # Arguments
///
/// * `embedding` - The embedding vector to normalize in-place
///
/// # Example
///
/// ```rust
/// use sevensense_embedding::normalization::l2_normalize;
///
/// let mut vector = vec![3.0, 4.0];
/// l2_normalize(&mut vector);
/// assert!((vector[0] - 0.6).abs() < 1e-6);
/// assert!((vector[1] - 0.8).abs() < 1e-6);
/// ```
pub fn l2_normalize(embedding: &mut [f32]) {
    let norm = compute_norm(embedding);

    if norm > 1e-12 {
        for x in embedding.iter_mut() {
            *x /= norm;
        }
    } else {
        // Handle near-zero embeddings (likely silent input)
        // Set to unit vector in first dimension
        embedding.iter_mut().for_each(|x| *x = 0.0);
        if !embedding.is_empty() {
            embedding[0] = 1.0;
        }
    }
}

/// Compute the L2 norm of a vector.
///
/// # Arguments
///
/// * `vector` - The vector to compute the norm for
///
/// # Returns
///
/// The L2 norm (Euclidean length) of the vector.
#[must_use]
pub fn compute_norm(vector: &[f32]) -> f32 {
    vector.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Compute the sparsity of a vector.
///
/// Sparsity is the fraction of near-zero values in the vector.
/// High sparsity may indicate issues with the embedding model.
///
/// # Arguments
///
/// * `vector` - The vector to analyze
///
/// # Returns
///
/// Sparsity as a value between 0.0 (no zeros) and 1.0 (all zeros).
#[must_use]
pub fn compute_sparsity(vector: &[f32]) -> f32 {
    if vector.is_empty() {
        return 0.0;
    }

    let near_zero_count = vector.iter().filter(|&&x| x.abs() < 1e-6).count();
    near_zero_count as f32 / vector.len() as f32
}

/// Validate an embedding vector for common issues.
///
/// # Arguments
///
/// * `embedding` - The embedding vector to validate
///
/// # Returns
///
/// A `ValidationResult` containing detailed information about the vector.
#[must_use]
pub fn validate_embedding(embedding: &[f32]) -> ValidationResult {
    let dimension_valid = embedding.len() == EMBEDDING_DIM;
    let has_nan = embedding.iter().any(|x| x.is_nan());
    let has_inf = embedding.iter().any(|x| x.is_infinite());
    let norm = compute_norm(embedding);
    let is_normalized = (0.99..=1.01).contains(&norm);
    let sparsity = compute_sparsity(embedding);

    let issues = collect_issues(
        dimension_valid,
        embedding.len(),
        has_nan,
        has_inf,
        is_normalized,
        norm,
        sparsity,
    );

    ValidationResult {
        dimension: embedding.len(),
        dimension_valid,
        norm,
        is_normalized,
        has_nan,
        has_inf,
        sparsity,
        is_valid: dimension_valid && !has_nan && !has_inf,
        issues,
    }
}

fn collect_issues(
    dimension_valid: bool,
    actual_dim: usize,
    has_nan: bool,
    has_inf: bool,
    is_normalized: bool,
    norm: f32,
    sparsity: f32,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if !dimension_valid {
        issues.push(ValidationIssue::InvalidDimension {
            expected: EMBEDDING_DIM,
            actual: actual_dim,
        });
    }

    if has_nan {
        issues.push(ValidationIssue::ContainsNaN);
    }

    if has_inf {
        issues.push(ValidationIssue::ContainsInfinite);
    }

    if !is_normalized && !has_nan && !has_inf {
        issues.push(ValidationIssue::NotNormalized { norm });
    }

    if sparsity > 0.9 {
        issues.push(ValidationIssue::HighSparsity { sparsity });
    }

    issues
}

/// Result of embedding validation
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Actual dimension of the embedding
    pub dimension: usize,

    /// Whether the dimension matches expected (1536)
    pub dimension_valid: bool,

    /// L2 norm of the embedding
    pub norm: f32,

    /// Whether the embedding is L2 normalized (norm close to 1.0)
    pub is_normalized: bool,

    /// Whether the embedding contains NaN values
    pub has_nan: bool,

    /// Whether the embedding contains infinite values
    pub has_inf: bool,

    /// Fraction of near-zero values
    pub sparsity: f32,

    /// Overall validity (no NaN, no Inf, correct dimension)
    pub is_valid: bool,

    /// List of specific issues found
    pub issues: Vec<ValidationIssue>,
}

impl ValidationResult {
    /// Check if the embedding passes all validation checks
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.issues.is_empty()
    }

    /// Get a human-readable summary of the validation result
    #[must_use]
    pub fn summary(&self) -> String {
        if self.issues.is_empty() {
            return "Embedding is valid".to_string();
        }

        let issue_strings: Vec<String> = self.issues.iter().map(|i| i.to_string()).collect();
        format!("Embedding has issues: {}", issue_strings.join(", "))
    }
}

/// Specific validation issues that can be detected
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationIssue {
    /// Embedding dimension doesn't match expected
    InvalidDimension {
        /// Expected dimension
        expected: usize,
        /// Actual dimension
        actual: usize,
    },

    /// Embedding contains NaN values
    ContainsNaN,

    /// Embedding contains infinite values
    ContainsInfinite,

    /// Embedding is not L2 normalized
    NotNormalized {
        /// Actual norm
        norm: f32,
    },

    /// Embedding has high sparsity (many near-zero values)
    HighSparsity {
        /// Sparsity value
        sparsity: f32,
    },
}

impl std::fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDimension { expected, actual } => {
                write!(f, "invalid dimension (expected {expected}, got {actual})")
            }
            Self::ContainsNaN => write!(f, "contains NaN values"),
            Self::ContainsInfinite => write!(f, "contains infinite values"),
            Self::NotNormalized { norm } => {
                write!(f, "not normalized (norm = {norm:.4})")
            }
            Self::HighSparsity { sparsity } => {
                write!(f, "high sparsity ({:.1}%)", sparsity * 100.0)
            }
        }
    }
}

/// L1 normalize a vector (sum of absolute values = 1)
pub fn l1_normalize(embedding: &mut [f32]) {
    let sum: f32 = embedding.iter().map(|x| x.abs()).sum();

    if sum > 1e-12 {
        for x in embedding.iter_mut() {
            *x /= sum;
        }
    }
}

/// Min-max normalize a vector to [0, 1] range
pub fn minmax_normalize(embedding: &mut [f32]) {
    if embedding.is_empty() {
        return;
    }

    let min = embedding.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max = embedding.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    let range = max - min;

    if range > 1e-12 {
        for x in embedding.iter_mut() {
            *x = (*x - min) / range;
        }
    } else {
        // All values are the same
        embedding.iter_mut().for_each(|x| *x = 0.5);
    }
}

/// Z-score normalize a vector (mean = 0, std = 1)
pub fn zscore_normalize(embedding: &mut [f32]) {
    if embedding.is_empty() {
        return;
    }

    let n = embedding.len() as f32;
    let mean: f32 = embedding.iter().sum::<f32>() / n;
    let variance: f32 = embedding.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n;
    let std = variance.sqrt();

    if std > 1e-12 {
        for x in embedding.iter_mut() {
            *x = (*x - mean) / std;
        }
    } else {
        // Zero variance - all values are the same
        embedding.iter_mut().for_each(|x| *x = 0.0);
    }
}

/// Clamp values to a specified range
pub fn clamp(embedding: &mut [f32], min: f32, max: f32) {
    for x in embedding.iter_mut() {
        *x = x.clamp(min, max);
    }
}

/// Soft clipping using tanh
pub fn soft_clip(embedding: &mut [f32], scale: f32) {
    for x in embedding.iter_mut() {
        *x = (*x / scale).tanh() * scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l2_normalize() {
        let mut vector = vec![3.0, 4.0];
        l2_normalize(&mut vector);
        assert!((vector[0] - 0.6).abs() < 1e-6);
        assert!((vector[1] - 0.8).abs() < 1e-6);

        let norm = compute_norm(&vector);
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize_zero_vector() {
        let mut vector = vec![0.0, 0.0, 0.0];
        l2_normalize(&mut vector);
        assert_eq!(vector[0], 1.0);
        assert_eq!(vector[1], 0.0);
        assert_eq!(vector[2], 0.0);
    }

    #[test]
    fn test_compute_norm() {
        let vector = vec![3.0, 4.0];
        let norm = compute_norm(&vector);
        assert!((norm - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_compute_sparsity() {
        let vector = vec![0.0, 1.0, 0.0, 2.0, 0.0];
        let sparsity = compute_sparsity(&vector);
        assert!((sparsity - 0.6).abs() < 1e-6);
    }

    #[test]
    fn test_compute_sparsity_empty() {
        let vector: Vec<f32> = vec![];
        let sparsity = compute_sparsity(&vector);
        assert_eq!(sparsity, 0.0);
    }

    #[test]
    fn test_validate_embedding_valid() {
        let mut vector = vec![0.0; EMBEDDING_DIM];
        vector[0] = 1.0;
        let result = validate_embedding(&vector);
        assert!(result.is_valid);
        assert!(result.is_normalized);
        assert!(!result.has_nan);
        assert!(!result.has_inf);
    }

    #[test]
    fn test_validate_embedding_wrong_dimension() {
        let vector = vec![1.0; 100];
        let result = validate_embedding(&vector);
        assert!(!result.dimension_valid);
        assert!(result.issues.iter().any(|i| matches!(i, ValidationIssue::InvalidDimension { .. })));
    }

    #[test]
    fn test_validate_embedding_nan() {
        let mut vector = vec![0.0; EMBEDDING_DIM];
        vector[0] = f32::NAN;
        let result = validate_embedding(&vector);
        assert!(result.has_nan);
        assert!(!result.is_valid);
    }

    #[test]
    fn test_validate_embedding_infinite() {
        let mut vector = vec![0.0; EMBEDDING_DIM];
        vector[0] = f32::INFINITY;
        let result = validate_embedding(&vector);
        assert!(result.has_inf);
        assert!(!result.is_valid);
    }

    #[test]
    fn test_l1_normalize() {
        let mut vector = vec![1.0, 2.0, 3.0];
        l1_normalize(&mut vector);
        let sum: f32 = vector.iter().map(|x| x.abs()).sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_minmax_normalize() {
        let mut vector = vec![0.0, 5.0, 10.0];
        minmax_normalize(&mut vector);
        assert!((vector[0] - 0.0).abs() < 1e-6);
        assert!((vector[1] - 0.5).abs() < 1e-6);
        assert!((vector[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_zscore_normalize() {
        let mut vector = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        zscore_normalize(&mut vector);
        let mean: f32 = vector.iter().sum::<f32>() / vector.len() as f32;
        assert!(mean.abs() < 1e-6);
    }

    #[test]
    fn test_clamp() {
        let mut vector = vec![-2.0, 0.5, 2.0];
        clamp(&mut vector, -1.0, 1.0);
        assert_eq!(vector, vec![-1.0, 0.5, 1.0]);
    }

    #[test]
    fn test_soft_clip() {
        let mut vector = vec![0.0, 1.0, 2.0];
        soft_clip(&mut vector, 1.0);
        assert!((vector[0] - 0.0).abs() < 1e-6);
        // tanh(1) ≈ 0.7616
        assert!(vector[1] > 0.5 && vector[1] < 0.8);
        // tanh(2) ≈ 0.964
        assert!(vector[2] > 0.9 && vector[2] < 1.0);
    }

    #[test]
    fn test_validation_result_summary() {
        // Create a reasonably distributed embedding (not too sparse)
        let mut vector = vec![0.0; EMBEDDING_DIM];
        // Fill first half with small values that sum to norm 1.0
        let val = 1.0 / (EMBEDDING_DIM as f32 / 2.0).sqrt();
        for i in 0..EMBEDDING_DIM / 2 {
            vector[i] = val;
        }
        let result = validate_embedding(&vector);
        assert!(result.summary().contains("valid"), "Summary: {}", result.summary());
    }
}
