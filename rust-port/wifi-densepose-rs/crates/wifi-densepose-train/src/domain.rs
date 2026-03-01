//! Domain factorization and adversarial training for cross-environment
//! generalization (MERIDIAN Phase 2, ADR-027).
//!
//! Components: [`GradientReversalLayer`], [`DomainFactorizer`],
//! [`DomainClassifier`], and [`AdversarialSchedule`].
//!
//! All computations are pure Rust on `&[f32]` slices (no `tch`, no GPU).

// ---------------------------------------------------------------------------
// Helper math functions
// ---------------------------------------------------------------------------

/// GELU activation (Hendrycks & Gimpel, 2016 approximation).
pub fn gelu(x: f32) -> f32 {
    let c = (2.0_f32 / std::f32::consts::PI).sqrt();
    x * 0.5 * (1.0 + (c * (x + 0.044715 * x * x * x)).tanh())
}

/// Layer normalization: `(x - mean) / sqrt(var + eps)`. No affine parameters.
pub fn layer_norm(x: &[f32]) -> Vec<f32> {
    let n = x.len() as f32;
    if n == 0.0 { return vec![]; }
    let mean = x.iter().sum::<f32>() / n;
    let var = x.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
    let inv_std = 1.0 / (var + 1e-5_f32).sqrt();
    x.iter().map(|v| (v - mean) * inv_std).collect()
}

/// Global mean pool: average `n_items` vectors of length `dim` from a flat buffer.
pub fn global_mean_pool(features: &[f32], n_items: usize, dim: usize) -> Vec<f32> {
    assert_eq!(features.len(), n_items * dim);
    assert!(n_items > 0);
    let mut out = vec![0.0_f32; dim];
    let scale = 1.0 / n_items as f32;
    for i in 0..n_items {
        let off = i * dim;
        for j in 0..dim { out[j] += features[off + j]; }
    }
    for v in out.iter_mut() { *v *= scale; }
    out
}

fn relu_vec(x: &[f32]) -> Vec<f32> {
    x.iter().map(|v| v.max(0.0)).collect()
}

// ---------------------------------------------------------------------------
// Linear layer (pure Rust, Kaiming-uniform init)
// ---------------------------------------------------------------------------

/// Fully-connected layer: `y = x W^T + b`. Kaiming-uniform initialization.
#[derive(Debug, Clone)]
pub struct Linear {
    /// Weight `[out, in]` row-major.
    pub weight: Vec<f32>,
    /// Bias `[out]`.
    pub bias: Vec<f32>,
    /// Input dimension.
    pub in_features: usize,
    /// Output dimension.
    pub out_features: usize,
}

/// Global instance counter to ensure distinct seeds for layers with same dimensions.
static INSTANCE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl Linear {
    /// New layer with deterministic Kaiming-uniform weights.
    ///
    /// Each call produces unique weights even for identical `(in_features, out_features)`
    /// because an atomic instance counter is mixed into the seed.
    pub fn new(in_features: usize, out_features: usize) -> Self {
        let instance = INSTANCE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let bound = (1.0 / in_features as f64).sqrt() as f32;
        let n = out_features * in_features;
        let mut seed: u64 = (in_features as u64)
            .wrapping_mul(6364136223846793005)
            .wrapping_add(out_features as u64)
            .wrapping_add(instance.wrapping_mul(2654435761));
        let mut next = || -> f32 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as f32) / (u32::MAX as f32 / 2.0) - 1.0
        };
        let weight: Vec<f32> = (0..n).map(|_| next() * bound).collect();
        let bias: Vec<f32> = (0..out_features).map(|_| next() * bound).collect();
        Linear { weight, bias, in_features, out_features }
    }

    /// Forward: `y = x W^T + b`.
    pub fn forward(&self, x: &[f32]) -> Vec<f32> {
        assert_eq!(x.len(), self.in_features);
        (0..self.out_features).map(|o| {
            let row = o * self.in_features;
            let mut s = self.bias[o];
            for i in 0..self.in_features { s += self.weight[row + i] * x[i]; }
            s
        }).collect()
    }
}

// ---------------------------------------------------------------------------
// GradientReversalLayer
// ---------------------------------------------------------------------------

/// Gradient Reversal Layer (Ganin & Lempitsky, ICML 2015).
///
/// Forward: identity. Backward: `-lambda * grad`.
#[derive(Debug, Clone)]
pub struct GradientReversalLayer {
    /// Reversal scaling factor, annealed via [`AdversarialSchedule`].
    pub lambda: f32,
}

impl GradientReversalLayer {
    /// Create a new GRL.
    pub fn new(lambda: f32) -> Self { Self { lambda } }

    /// Forward pass (identity).
    pub fn forward(&self, x: &[f32]) -> Vec<f32> { x.to_vec() }

    /// Backward pass: returns `-lambda * grad`.
    pub fn backward(&self, grad: &[f32]) -> Vec<f32> {
        grad.iter().map(|g| -self.lambda * g).collect()
    }
}

// ---------------------------------------------------------------------------
// DomainFactorizer
// ---------------------------------------------------------------------------

/// Splits body-part features into pose-relevant (`h_pose`) and
/// environment-specific (`h_env`) representations.
///
/// - **PoseEncoder**: per-part `Linear(64,128) -> LayerNorm -> GELU -> Linear(128,64)`
/// - **EnvEncoder**: `GlobalMeanPool(17x64->64) -> Linear(64,32)`
#[derive(Debug, Clone)]
pub struct DomainFactorizer {
    /// Pose encoder FC1.
    pub pose_fc1: Linear,
    /// Pose encoder FC2.
    pub pose_fc2: Linear,
    /// Environment encoder FC.
    pub env_fc: Linear,
    /// Number of body parts.
    pub n_parts: usize,
    /// Feature dim per part.
    pub part_dim: usize,
}

impl DomainFactorizer {
    /// Create with `n_parts` body parts of `part_dim` features each.
    pub fn new(n_parts: usize, part_dim: usize) -> Self {
        Self {
            pose_fc1: Linear::new(part_dim, 128),
            pose_fc2: Linear::new(128, part_dim),
            env_fc: Linear::new(part_dim, 32),
            n_parts, part_dim,
        }
    }

    /// Factorize into `(h_pose [n_parts*part_dim], h_env [32])`.
    pub fn factorize(&self, body_part_features: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let expected = self.n_parts * self.part_dim;
        assert_eq!(body_part_features.len(), expected);

        let mut h_pose = Vec::with_capacity(expected);
        for i in 0..self.n_parts {
            let off = i * self.part_dim;
            let part = &body_part_features[off..off + self.part_dim];
            let z = self.pose_fc1.forward(part);
            let z = layer_norm(&z);
            let z: Vec<f32> = z.iter().map(|v| gelu(*v)).collect();
            let z = self.pose_fc2.forward(&z);
            h_pose.extend_from_slice(&z);
        }

        let pooled = global_mean_pool(body_part_features, self.n_parts, self.part_dim);
        let h_env = self.env_fc.forward(&pooled);
        (h_pose, h_env)
    }
}

// ---------------------------------------------------------------------------
// DomainClassifier
// ---------------------------------------------------------------------------

/// Predicts which environment a sample came from.
///
/// `MeanPool(17x64->64) -> Linear(64,32) -> ReLU -> Linear(32, n_domains)`
#[derive(Debug, Clone)]
pub struct DomainClassifier {
    /// Hidden layer.
    pub fc1: Linear,
    /// Output layer.
    pub fc2: Linear,
    /// Number of body parts for mean pooling.
    pub n_parts: usize,
    /// Feature dim per part.
    pub part_dim: usize,
    /// Number of domain classes.
    pub n_domains: usize,
}

impl DomainClassifier {
    /// Create a domain classifier for `n_domains` environments.
    pub fn new(n_parts: usize, part_dim: usize, n_domains: usize) -> Self {
        Self {
            fc1: Linear::new(part_dim, 32),
            fc2: Linear::new(32, n_domains),
            n_parts, part_dim, n_domains,
        }
    }

    /// Classify: returns raw domain logits of length `n_domains`.
    pub fn classify(&self, h_pose: &[f32]) -> Vec<f32> {
        assert_eq!(h_pose.len(), self.n_parts * self.part_dim);
        let pooled = global_mean_pool(h_pose, self.n_parts, self.part_dim);
        let z = relu_vec(&self.fc1.forward(&pooled));
        self.fc2.forward(&z)
    }
}

// ---------------------------------------------------------------------------
// AdversarialSchedule
// ---------------------------------------------------------------------------

/// Lambda annealing: `lambda(p) = 2 / (1 + exp(-10p)) - 1`, p = epoch/max_epochs.
#[derive(Debug, Clone)]
pub struct AdversarialSchedule {
    /// Maximum training epochs.
    pub max_epochs: usize,
}

impl AdversarialSchedule {
    /// Create schedule.
    pub fn new(max_epochs: usize) -> Self {
        assert!(max_epochs > 0);
        Self { max_epochs }
    }

    /// Compute lambda for `epoch`. Returns value in [0, 1].
    pub fn lambda(&self, epoch: usize) -> f32 {
        let p = epoch as f64 / self.max_epochs as f64;
        (2.0 / (1.0 + (-10.0 * p).exp()) - 1.0) as f32
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grl_forward_is_identity() {
        let grl = GradientReversalLayer::new(0.5);
        let x = vec![1.0, -2.0, 3.0, 0.0, -0.5];
        assert_eq!(grl.forward(&x), x);
    }

    #[test]
    fn grl_backward_negates_with_lambda() {
        let grl = GradientReversalLayer::new(0.7);
        let grad = vec![1.0, -2.0, 3.0, 0.0, 4.0];
        let rev = grl.backward(&grad);
        for (r, g) in rev.iter().zip(&grad) {
            assert!((r - (-0.7 * g)).abs() < 1e-6);
        }
    }

    #[test]
    fn grl_lambda_zero_gives_zero_grad() {
        let rev = GradientReversalLayer::new(0.0).backward(&[1.0, 2.0, 3.0]);
        assert!(rev.iter().all(|v| v.abs() < 1e-7));
    }

    #[test]
    fn factorizer_output_dimensions() {
        let f = DomainFactorizer::new(17, 64);
        let (h_pose, h_env) = f.factorize(&vec![0.1; 17 * 64]);
        assert_eq!(h_pose.len(), 17 * 64, "h_pose should be 17*64");
        assert_eq!(h_env.len(), 32, "h_env should be 32");
    }

    #[test]
    fn factorizer_values_finite() {
        let f = DomainFactorizer::new(17, 64);
        let (hp, he) = f.factorize(&vec![0.5; 17 * 64]);
        assert!(hp.iter().all(|v| v.is_finite()));
        assert!(he.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn classifier_output_equals_n_domains() {
        for nd in [1, 3, 5, 8] {
            let c = DomainClassifier::new(17, 64, nd);
            let logits = c.classify(&vec![0.1; 17 * 64]);
            assert_eq!(logits.len(), nd);
            assert!(logits.iter().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn schedule_lambda_zero_approx_zero() {
        let s = AdversarialSchedule::new(100);
        assert!(s.lambda(0).abs() < 0.01, "lambda(0) ~ 0");
    }

    #[test]
    fn schedule_lambda_at_half() {
        let s = AdversarialSchedule::new(100);
        // p=0.5 => 2/(1+exp(-5))-1 ≈ 0.9866
        let lam = s.lambda(50);
        assert!((lam - 0.9866).abs() < 0.02, "lambda(0.5)~0.987, got {lam}");
    }

    #[test]
    fn schedule_lambda_one_approx_one() {
        let s = AdversarialSchedule::new(100);
        assert!((s.lambda(100) - 1.0).abs() < 0.001, "lambda(1.0) ~ 1");
    }

    #[test]
    fn schedule_monotonically_increasing() {
        let s = AdversarialSchedule::new(100);
        let mut prev = s.lambda(0);
        for e in 1..=100 {
            let cur = s.lambda(e);
            assert!(cur >= prev - 1e-7, "not monotone at epoch {e}");
            prev = cur;
        }
    }

    #[test]
    fn gelu_reference_values() {
        assert!(gelu(0.0).abs() < 1e-6, "gelu(0)=0");
        assert!((gelu(1.0) - 0.8412).abs() < 0.01, "gelu(1)~0.841");
        assert!((gelu(-1.0) + 0.1588).abs() < 0.01, "gelu(-1)~-0.159");
        assert!(gelu(5.0) > 4.5, "gelu(5)~5");
        assert!(gelu(-5.0).abs() < 0.01, "gelu(-5)~0");
    }

    #[test]
    fn layer_norm_zero_mean_unit_var() {
        let normed = layer_norm(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let n = normed.len() as f32;
        let mean = normed.iter().sum::<f32>() / n;
        let var = normed.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
        assert!(mean.abs() < 1e-5, "mean~0, got {mean}");
        assert!((var - 1.0).abs() < 0.01, "var~1, got {var}");
    }

    #[test]
    fn layer_norm_constant_gives_zeros() {
        let normed = layer_norm(&vec![3.0; 16]);
        assert!(normed.iter().all(|v| v.abs() < 1e-4));
    }

    #[test]
    fn layer_norm_empty() {
        assert!(layer_norm(&[]).is_empty());
    }

    #[test]
    fn mean_pool_simple() {
        let p = global_mean_pool(&[1.0, 2.0, 3.0, 5.0, 6.0, 7.0], 2, 3);
        assert!((p[0] - 3.0).abs() < 1e-6);
        assert!((p[1] - 4.0).abs() < 1e-6);
        assert!((p[2] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn linear_dimensions_and_finite() {
        let l = Linear::new(64, 128);
        let out = l.forward(&vec![0.1; 64]);
        assert_eq!(out.len(), 128);
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn full_pipeline() {
        let fact = DomainFactorizer::new(17, 64);
        let grl = GradientReversalLayer::new(0.5);
        let cls = DomainClassifier::new(17, 64, 4);

        let feat = vec![0.2_f32; 17 * 64];
        let (hp, he) = fact.factorize(&feat);
        assert_eq!(hp.len(), 17 * 64);
        assert_eq!(he.len(), 32);

        let hp_grl = grl.forward(&hp);
        assert_eq!(hp_grl, hp);

        let logits = cls.classify(&hp_grl);
        assert_eq!(logits.len(), 4);
        assert!(logits.iter().all(|v| v.is_finite()));
    }
}
