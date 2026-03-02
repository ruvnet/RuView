//! Cross-viewpoint scaled dot-product attention with geometric bias (ADR-031).
//!
//! Implements the core RuView attention mechanism:
//!
//! ```text
//! Q = W_q * X,  K = W_k * X,  V = W_v * X
//! A = softmax((Q * K^T + G_bias) / sqrt(d))
//! fused = A * V
//! ```
//!
//! The geometric bias `G_bias` encodes angular separation and baseline distance
//! between each viewpoint pair, allowing the attention mechanism to learn that
//! widely-separated, orthogonal viewpoints are more complementary than clustered
//! ones.
//!
//! Wraps `ruvector_attention::ScaledDotProductAttention` for the underlying
//! attention computation.

// The cross-viewpoint attention is implemented directly rather than wrapping
// ruvector_attention::ScaledDotProductAttention, because we need to inject
// the geometric bias matrix G_bias into the QK^T scores before softmax --
// an operation not exposed by the ruvector API. The ruvector-attention crate
// is still a workspace dependency for the signal/bvp integration point.

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors produced by the cross-viewpoint attention module.
#[derive(Debug, Clone)]
pub enum AttentionError {
    /// The number of viewpoints is zero.
    EmptyViewpoints,
    /// Embedding dimension mismatch between viewpoints.
    DimensionMismatch {
        /// Expected embedding dimension.
        expected: usize,
        /// Actual embedding dimension found.
        actual: usize,
    },
    /// The geometric bias matrix dimensions do not match the viewpoint count.
    BiasDimensionMismatch {
        /// Number of viewpoints.
        n_viewpoints: usize,
        /// Rows in bias matrix.
        bias_rows: usize,
        /// Columns in bias matrix.
        bias_cols: usize,
    },
    /// The projection weight matrix has incorrect dimensions.
    WeightDimensionMismatch {
        /// Expected dimension.
        expected: usize,
        /// Actual dimension.
        actual: usize,
    },
}

impl std::fmt::Display for AttentionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttentionError::EmptyViewpoints => write!(f, "no viewpoint embeddings provided"),
            AttentionError::DimensionMismatch { expected, actual } => {
                write!(f, "embedding dimension mismatch: expected {expected}, got {actual}")
            }
            AttentionError::BiasDimensionMismatch { n_viewpoints, bias_rows, bias_cols } => {
                write!(
                    f,
                    "geometric bias matrix is {bias_rows}x{bias_cols} but {n_viewpoints} viewpoints require {n_viewpoints}x{n_viewpoints}"
                )
            }
            AttentionError::WeightDimensionMismatch { expected, actual } => {
                write!(f, "weight matrix dimension mismatch: expected {expected}, got {actual}")
            }
        }
    }
}

impl std::error::Error for AttentionError {}

// ---------------------------------------------------------------------------
// GeometricBias
// ---------------------------------------------------------------------------

/// Geometric bias matrix encoding spatial relationships between viewpoint pairs.
///
/// The bias for viewpoint pair `(i, j)` is computed as:
///
/// ```text
/// G_bias[i,j] = w_angle * cos(theta_ij) + w_dist * exp(-d_ij / d_ref)
/// ```
///
/// where `theta_ij` is the angular separation between viewpoints `i` and `j`
/// from the array centroid, `d_ij` is the baseline distance, `w_angle` and
/// `w_dist` are learnable scalar weights, and `d_ref` is a reference distance
/// (typically room diagonal / 2).
#[derive(Debug, Clone)]
pub struct GeometricBias {
    /// Learnable weight for the angular component.
    pub w_angle: f32,
    /// Learnable weight for the distance component.
    pub w_dist: f32,
    /// Reference distance for the exponential decay (metres).
    pub d_ref: f32,
}

impl Default for GeometricBias {
    fn default() -> Self {
        GeometricBias {
            w_angle: 1.0,
            w_dist: 1.0,
            d_ref: 5.0,
        }
    }
}

/// A single viewpoint geometry descriptor.
#[derive(Debug, Clone)]
pub struct ViewpointGeometry {
    /// Azimuth angle from array centroid (radians).
    pub azimuth: f32,
    /// 2-D position (x, y) in metres.
    pub position: (f32, f32),
}

impl GeometricBias {
    /// Create a new geometric bias with the given parameters.
    pub fn new(w_angle: f32, w_dist: f32, d_ref: f32) -> Self {
        GeometricBias { w_angle, w_dist, d_ref }
    }

    /// Compute the bias value for a single viewpoint pair.
    ///
    /// # Arguments
    ///
    /// - `theta_ij`: angular separation in radians between viewpoints `i` and `j`.
    /// - `d_ij`: baseline distance in metres between viewpoints `i` and `j`.
    ///
    /// # Returns
    ///
    /// The scalar bias value `w_angle * cos(theta_ij) + w_dist * exp(-d_ij / d_ref)`.
    pub fn compute_pair(&self, theta_ij: f32, d_ij: f32) -> f32 {
        let safe_d_ref = self.d_ref.max(1e-6);
        self.w_angle * theta_ij.cos() + self.w_dist * (-d_ij / safe_d_ref).exp()
    }

    /// Build the full N x N geometric bias matrix from viewpoint geometries.
    ///
    /// # Arguments
    ///
    /// - `viewpoints`: slice of viewpoint geometry descriptors.
    ///
    /// # Returns
    ///
    /// Flat row-major `N x N` bias matrix.
    pub fn build_matrix(&self, viewpoints: &[ViewpointGeometry]) -> Vec<f32> {
        let n = viewpoints.len();
        let mut matrix = vec![0.0_f32; n * n];
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    // Self-bias: maximum (cos(0) = 1, exp(0) = 1)
                    matrix[i * n + j] = self.w_angle + self.w_dist;
                } else {
                    let theta_ij = (viewpoints[i].azimuth - viewpoints[j].azimuth).abs();
                    let dx = viewpoints[i].position.0 - viewpoints[j].position.0;
                    let dy = viewpoints[i].position.1 - viewpoints[j].position.1;
                    let d_ij = (dx * dx + dy * dy).sqrt();
                    matrix[i * n + j] = self.compute_pair(theta_ij, d_ij);
                }
            }
        }
        matrix
    }
}

// ---------------------------------------------------------------------------
// Projection weights
// ---------------------------------------------------------------------------

/// Linear projection weights for Q, K, V transformations.
///
/// Each weight matrix is `d_out x d_in`, stored row-major. In the default
/// (identity) configuration `d_out == d_in` and the matrices are identity.
#[derive(Debug, Clone)]
pub struct ProjectionWeights {
    /// W_q projection matrix, row-major `[d_out, d_in]`.
    pub w_q: Vec<f32>,
    /// W_k projection matrix, row-major `[d_out, d_in]`.
    pub w_k: Vec<f32>,
    /// W_v projection matrix, row-major `[d_out, d_in]`.
    pub w_v: Vec<f32>,
    /// Input dimension.
    pub d_in: usize,
    /// Output (projected) dimension.
    pub d_out: usize,
}

impl ProjectionWeights {
    /// Create identity projections (d_out == d_in, W = I).
    pub fn identity(dim: usize) -> Self {
        let mut eye = vec![0.0_f32; dim * dim];
        for i in 0..dim {
            eye[i * dim + i] = 1.0;
        }
        ProjectionWeights {
            w_q: eye.clone(),
            w_k: eye.clone(),
            w_v: eye,
            d_in: dim,
            d_out: dim,
        }
    }

    /// Create projections with given weight matrices.
    ///
    /// Each matrix must be `d_out * d_in` elements, stored row-major.
    pub fn new(
        w_q: Vec<f32>,
        w_k: Vec<f32>,
        w_v: Vec<f32>,
        d_in: usize,
        d_out: usize,
    ) -> Result<Self, AttentionError> {
        let expected_len = d_out * d_in;
        if w_q.len() != expected_len {
            return Err(AttentionError::WeightDimensionMismatch {
                expected: expected_len,
                actual: w_q.len(),
            });
        }
        if w_k.len() != expected_len {
            return Err(AttentionError::WeightDimensionMismatch {
                expected: expected_len,
                actual: w_k.len(),
            });
        }
        if w_v.len() != expected_len {
            return Err(AttentionError::WeightDimensionMismatch {
                expected: expected_len,
                actual: w_v.len(),
            });
        }
        Ok(ProjectionWeights { w_q, w_k, w_v, d_in, d_out })
    }

    /// Project a single embedding vector through a weight matrix.
    ///
    /// `weight` is `[d_out, d_in]` row-major, `input` is `[d_in]`.
    /// Returns `[d_out]`.
    fn project(&self, weight: &[f32], input: &[f32]) -> Vec<f32> {
        let mut output = vec![0.0_f32; self.d_out];
        for row in 0..self.d_out {
            let mut sum = 0.0_f32;
            for col in 0..self.d_in {
                sum += weight[row * self.d_in + col] * input[col];
            }
            output[row] = sum;
        }
        output
    }

    /// Project all viewpoint embeddings through W_q.
    pub fn project_queries(&self, embeddings: &[Vec<f32>]) -> Vec<Vec<f32>> {
        embeddings.iter().map(|e| self.project(&self.w_q, e)).collect()
    }

    /// Project all viewpoint embeddings through W_k.
    pub fn project_keys(&self, embeddings: &[Vec<f32>]) -> Vec<Vec<f32>> {
        embeddings.iter().map(|e| self.project(&self.w_k, e)).collect()
    }

    /// Project all viewpoint embeddings through W_v.
    pub fn project_values(&self, embeddings: &[Vec<f32>]) -> Vec<Vec<f32>> {
        embeddings.iter().map(|e| self.project(&self.w_v, e)).collect()
    }
}

// ---------------------------------------------------------------------------
// CrossViewpointAttention
// ---------------------------------------------------------------------------

/// Cross-viewpoint attention with geometric bias.
///
/// Computes the full RuView attention pipeline:
///
/// 1. Project embeddings through W_q, W_k, W_v.
/// 2. Compute attention scores: `A = softmax((Q * K^T + G_bias) / sqrt(d))`.
/// 3. Weighted sum: `fused = A * V`.
///
/// The output is one fused embedding per input viewpoint (row of A * V).
/// To obtain a single fused embedding, use [`CrossViewpointAttention::fuse`]
/// which mean-pools the attended outputs.
pub struct CrossViewpointAttention {
    /// Projection weights for Q, K, V.
    pub weights: ProjectionWeights,
    /// Geometric bias parameters.
    pub bias: GeometricBias,
}

impl CrossViewpointAttention {
    /// Create a new cross-viewpoint attention module with identity projections.
    ///
    /// # Arguments
    ///
    /// - `embed_dim`: embedding dimension (e.g. 128 for AETHER).
    pub fn new(embed_dim: usize) -> Self {
        CrossViewpointAttention {
            weights: ProjectionWeights::identity(embed_dim),
            bias: GeometricBias::default(),
        }
    }

    /// Create with custom projection weights and bias.
    pub fn with_params(weights: ProjectionWeights, bias: GeometricBias) -> Self {
        CrossViewpointAttention { weights, bias }
    }

    /// Compute the full attention output for all viewpoints.
    ///
    /// # Arguments
    ///
    /// - `embeddings`: per-viewpoint embedding vectors, each of length `d_in`.
    /// - `viewpoint_geom`: per-viewpoint geometry descriptors (same length).
    ///
    /// # Returns
    ///
    /// `Ok(attended)` where `attended` is `N` vectors of length `d_out`, one per
    /// viewpoint after cross-viewpoint attention. Returns an error if dimensions
    /// are inconsistent.
    pub fn attend(
        &self,
        embeddings: &[Vec<f32>],
        viewpoint_geom: &[ViewpointGeometry],
    ) -> Result<Vec<Vec<f32>>, AttentionError> {
        let n = embeddings.len();
        if n == 0 {
            return Err(AttentionError::EmptyViewpoints);
        }

        // Validate embedding dimensions.
        for (idx, emb) in embeddings.iter().enumerate() {
            if emb.len() != self.weights.d_in {
                return Err(AttentionError::DimensionMismatch {
                    expected: self.weights.d_in,
                    actual: emb.len(),
                });
            }
            let _ = idx; // suppress unused warning
        }

        let d = self.weights.d_out;
        let scale = 1.0 / (d as f32).sqrt();

        // Project through W_q, W_k, W_v.
        let queries = self.weights.project_queries(embeddings);
        let keys = self.weights.project_keys(embeddings);
        let values = self.weights.project_values(embeddings);

        // Build geometric bias matrix.
        let g_bias = self.bias.build_matrix(viewpoint_geom);

        // Compute attention scores: (Q * K^T + G_bias) / sqrt(d), then softmax.
        let mut attention_weights = vec![0.0_f32; n * n];
        for i in 0..n {
            // Compute raw scores for row i.
            let mut max_score = f32::NEG_INFINITY;
            for j in 0..n {
                let dot: f32 = queries[i].iter().zip(&keys[j]).map(|(q, k)| q * k).sum();
                let score = (dot + g_bias[i * n + j]) * scale;
                attention_weights[i * n + j] = score;
                if score > max_score {
                    max_score = score;
                }
            }

            // Softmax: subtract max for numerical stability, then exponentiate.
            let mut sum_exp = 0.0_f32;
            for j in 0..n {
                let val = (attention_weights[i * n + j] - max_score).exp();
                attention_weights[i * n + j] = val;
                sum_exp += val;
            }
            let safe_sum = sum_exp.max(f32::EPSILON);
            for j in 0..n {
                attention_weights[i * n + j] /= safe_sum;
            }
        }

        // Weighted sum: attended[i] = sum_j (attention_weights[i,j] * values[j]).
        let mut attended = Vec::with_capacity(n);
        for i in 0..n {
            let mut output = vec![0.0_f32; d];
            for j in 0..n {
                let w = attention_weights[i * n + j];
                for k in 0..d {
                    output[k] += w * values[j][k];
                }
            }
            attended.push(output);
        }

        Ok(attended)
    }

    /// Fuse multiple viewpoint embeddings into a single embedding.
    ///
    /// Applies cross-viewpoint attention, then mean-pools the attended outputs
    /// to produce a single fused embedding of dimension `d_out`.
    ///
    /// # Arguments
    ///
    /// - `embeddings`: per-viewpoint embedding vectors.
    /// - `viewpoint_geom`: per-viewpoint geometry descriptors.
    ///
    /// # Returns
    ///
    /// A single fused embedding of length `d_out`.
    pub fn fuse(
        &self,
        embeddings: &[Vec<f32>],
        viewpoint_geom: &[ViewpointGeometry],
    ) -> Result<Vec<f32>, AttentionError> {
        let attended = self.attend(embeddings, viewpoint_geom)?;
        let n = attended.len();
        let d = self.weights.d_out;
        let mut fused = vec![0.0_f32; d];

        for row in &attended {
            for k in 0..d {
                fused[k] += row[k];
            }
        }
        let n_f = n as f32;
        for k in 0..d {
            fused[k] /= n_f;
        }

        Ok(fused)
    }

    /// Extract the raw attention weight matrix (for diagnostics).
    ///
    /// Returns the `N x N` attention weight matrix (row-major, each row sums to 1).
    pub fn attention_weights(
        &self,
        embeddings: &[Vec<f32>],
        viewpoint_geom: &[ViewpointGeometry],
    ) -> Result<Vec<f32>, AttentionError> {
        let n = embeddings.len();
        if n == 0 {
            return Err(AttentionError::EmptyViewpoints);
        }

        let d = self.weights.d_out;
        let scale = 1.0 / (d as f32).sqrt();

        let queries = self.weights.project_queries(embeddings);
        let keys = self.weights.project_keys(embeddings);
        let g_bias = self.bias.build_matrix(viewpoint_geom);

        let mut weights = vec![0.0_f32; n * n];
        for i in 0..n {
            let mut max_score = f32::NEG_INFINITY;
            for j in 0..n {
                let dot: f32 = queries[i].iter().zip(&keys[j]).map(|(q, k)| q * k).sum();
                let score = (dot + g_bias[i * n + j]) * scale;
                weights[i * n + j] = score;
                if score > max_score {
                    max_score = score;
                }
            }

            let mut sum_exp = 0.0_f32;
            for j in 0..n {
                let val = (weights[i * n + j] - max_score).exp();
                weights[i * n + j] = val;
                sum_exp += val;
            }
            let safe_sum = sum_exp.max(f32::EPSILON);
            for j in 0..n {
                weights[i * n + j] /= safe_sum;
            }
        }

        Ok(weights)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_geom(n: usize) -> Vec<ViewpointGeometry> {
        (0..n)
            .map(|i| {
                let angle = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
                let r = 3.0;
                ViewpointGeometry {
                    azimuth: angle,
                    position: (r * angle.cos(), r * angle.sin()),
                }
            })
            .collect()
    }

    fn make_test_embeddings(n: usize, dim: usize) -> Vec<Vec<f32>> {
        (0..n)
            .map(|i| {
                (0..dim).map(|d| ((i * dim + d) as f32 * 0.01).sin()).collect()
            })
            .collect()
    }

    #[test]
    fn fuse_produces_correct_dimension() {
        let dim = 16;
        let n = 4;
        let attn = CrossViewpointAttention::new(dim);
        let embeddings = make_test_embeddings(n, dim);
        let geom = make_test_geom(n);
        let fused = attn.fuse(&embeddings, &geom).unwrap();
        assert_eq!(fused.len(), dim, "fused embedding must have length {dim}");
    }

    #[test]
    fn attend_produces_n_outputs() {
        let dim = 8;
        let n = 3;
        let attn = CrossViewpointAttention::new(dim);
        let embeddings = make_test_embeddings(n, dim);
        let geom = make_test_geom(n);
        let attended = attn.attend(&embeddings, &geom).unwrap();
        assert_eq!(attended.len(), n, "must produce one output per viewpoint");
        for row in &attended {
            assert_eq!(row.len(), dim);
        }
    }

    #[test]
    fn attention_weights_sum_to_one() {
        let dim = 8;
        let n = 4;
        let attn = CrossViewpointAttention::new(dim);
        let embeddings = make_test_embeddings(n, dim);
        let geom = make_test_geom(n);
        let weights = attn.attention_weights(&embeddings, &geom).unwrap();
        assert_eq!(weights.len(), n * n);
        for i in 0..n {
            let row_sum: f32 = (0..n).map(|j| weights[i * n + j]).sum();
            assert!(
                (row_sum - 1.0).abs() < 1e-5,
                "row {i} sums to {row_sum}, expected 1.0"
            );
        }
    }

    #[test]
    fn attention_weights_are_non_negative() {
        let dim = 8;
        let n = 3;
        let attn = CrossViewpointAttention::new(dim);
        let embeddings = make_test_embeddings(n, dim);
        let geom = make_test_geom(n);
        let weights = attn.attention_weights(&embeddings, &geom).unwrap();
        for w in &weights {
            assert!(*w >= 0.0, "attention weight must be non-negative, got {w}");
        }
    }

    #[test]
    fn empty_viewpoints_returns_error() {
        let attn = CrossViewpointAttention::new(8);
        let result = attn.fuse(&[], &[]);
        assert!(result.is_err());
    }

    #[test]
    fn dimension_mismatch_returns_error() {
        let attn = CrossViewpointAttention::new(8);
        let embeddings = vec![vec![1.0_f32; 4]]; // wrong dim
        let geom = make_test_geom(1);
        let result = attn.fuse(&embeddings, &geom);
        assert!(result.is_err());
    }

    #[test]
    fn geometric_bias_pair_computation() {
        let bias = GeometricBias::new(1.0, 1.0, 5.0);
        // Same position: theta=0, d=0 -> cos(0) + exp(0) = 2.0
        let val = bias.compute_pair(0.0, 0.0);
        assert!((val - 2.0).abs() < 1e-5, "self-bias should be 2.0, got {val}");

        // Orthogonal, far apart: theta=PI/2, d=5.0
        let val_orth = bias.compute_pair(std::f32::consts::FRAC_PI_2, 5.0);
        // cos(PI/2) ~ 0 + exp(-1) ~ 0.368
        assert!(val_orth < 1.0, "orthogonal far-apart viewpoints should have low bias");
    }

    #[test]
    fn geometric_bias_matrix_is_symmetric_for_symmetric_layout() {
        let bias = GeometricBias::default();
        let geom = make_test_geom(4);
        let matrix = bias.build_matrix(&geom);
        let n = 4;
        for i in 0..n {
            for j in 0..n {
                assert!(
                    (matrix[i * n + j] - matrix[j * n + i]).abs() < 1e-5,
                    "bias matrix must be symmetric for symmetric layout: [{i},{j}]={} vs [{j},{i}]={}",
                    matrix[i * n + j],
                    matrix[j * n + i]
                );
            }
        }
    }

    #[test]
    fn single_viewpoint_fuse_returns_projection() {
        let dim = 8;
        let attn = CrossViewpointAttention::new(dim);
        let embeddings = vec![vec![1.0_f32; dim]];
        let geom = make_test_geom(1);
        let fused = attn.fuse(&embeddings, &geom).unwrap();
        // With identity projection and single viewpoint, fused == input.
        for (i, v) in fused.iter().enumerate() {
            assert!(
                (v - 1.0).abs() < 1e-5,
                "single-viewpoint fuse should return input, dim {i}: {v}"
            );
        }
    }

    #[test]
    fn projection_weights_custom_transform() {
        // Verify that non-identity weights change the output.
        let dim = 4;
        // Swap first two dimensions in Q.
        let mut w_q = vec![0.0_f32; dim * dim];
        w_q[0 * dim + 1] = 1.0; // row 0 picks dim 1
        w_q[1 * dim + 0] = 1.0; // row 1 picks dim 0
        w_q[2 * dim + 2] = 1.0;
        w_q[3 * dim + 3] = 1.0;
        let w_id = {
            let mut eye = vec![0.0_f32; dim * dim];
            for i in 0..dim {
                eye[i * dim + i] = 1.0;
            }
            eye
        };
        let weights = ProjectionWeights::new(w_q, w_id.clone(), w_id, dim, dim).unwrap();
        let queries = weights.project_queries(&[vec![1.0, 2.0, 3.0, 4.0]]);
        assert_eq!(queries[0], vec![2.0, 1.0, 3.0, 4.0]);
    }

    #[test]
    fn geometric_bias_with_large_distance_decays() {
        let bias = GeometricBias::new(0.0, 1.0, 2.0); // only distance component
        let close = bias.compute_pair(0.0, 0.5);
        let far = bias.compute_pair(0.0, 10.0);
        assert!(close > far, "closer viewpoints should have higher distance bias");
    }
}
