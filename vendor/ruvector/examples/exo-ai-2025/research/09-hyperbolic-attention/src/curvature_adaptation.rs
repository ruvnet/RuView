//! Learnable Curvature Adaptation
//!
//! Implements adaptive curvature learning with coupled optimization
//! based on "Optimizing Curvature Learning" (2024) research.
//!
//! # Key Features
//!
//! - Learnable curvature per layer/head
//! - Coupled parameter-curvature updates
//! - Rescaling to maintain geometric consistency
//! - Multi-curvature product spaces

use std::f32::consts::E;

/// Learnable curvature parameter
#[derive(Clone, Debug)]
pub struct LearnableCurvature {
    /// Log-space parameter (ensures K > 0)
    log_k: f32,
    /// Learning rate for curvature updates
    curvature_lr: f32,
    /// Minimum curvature (for stability)
    min_curvature: f32,
    /// Maximum curvature (prevent extreme values)
    max_curvature: f32,
}

impl LearnableCurvature {
    /// Create new learnable curvature
    pub fn new(initial_curvature: f32) -> Self {
        assert!(initial_curvature > 0.0, "Curvature must be positive");

        Self {
            log_k: initial_curvature.ln(),
            curvature_lr: 0.01,
            min_curvature: 0.1,
            max_curvature: 10.0,
        }
    }

    /// Get current curvature value
    pub fn value(&self) -> f32 {
        self.log_k
            .exp()
            .clamp(self.min_curvature, self.max_curvature)
    }

    /// Update curvature given gradient
    pub fn update(&mut self, grad: f32) {
        self.log_k -= self.curvature_lr * grad;

        // Clip to prevent extreme values
        let k = self.value();
        self.log_k = k.ln();
    }

    /// Set learning rate
    pub fn with_lr(mut self, lr: f32) -> Self {
        self.curvature_lr = lr;
        self
    }

    /// Set bounds
    pub fn with_bounds(mut self, min: f32, max: f32) -> Self {
        assert!(min > 0.0 && max > min);
        self.min_curvature = min;
        self.max_curvature = max;
        self
    }

    /// Get magnitude (for consciousness metric)
    pub fn magnitude(&self) -> f32 {
        self.value().abs()
    }
}

/// Multi-curvature manager for product spaces
///
/// Manages multiple curvatures for different dimensions/layers
#[derive(Clone, Debug)]
pub struct MultiCurvature {
    curvatures: Vec<LearnableCurvature>,
    /// Weights for distance combination
    weights: Vec<f32>,
}

impl MultiCurvature {
    /// Create multi-curvature with uniform initialization
    pub fn new(num_components: usize, initial_curvature: f32) -> Self {
        let curvatures = (0..num_components)
            .map(|_| LearnableCurvature::new(initial_curvature))
            .collect();

        let weights = vec![1.0 / (num_components as f32).sqrt(); num_components];

        Self {
            curvatures,
            weights,
        }
    }

    /// Create with different initial curvatures
    pub fn from_values(curvature_values: Vec<f32>) -> Self {
        let curvatures = curvature_values
            .into_iter()
            .map(|k| LearnableCurvature::new(k))
            .collect::<Vec<_>>();

        let num = curvatures.len();
        let weights = vec![1.0 / (num as f32).sqrt(); num];

        Self {
            curvatures,
            weights,
        }
    }

    /// Get all curvature values
    pub fn values(&self) -> Vec<f32> {
        self.curvatures.iter().map(|c| c.value()).collect()
    }

    /// Update all curvatures
    pub fn update(&mut self, grads: &[f32]) {
        assert_eq!(grads.len(), self.curvatures.len());

        for (curvature, &grad) in self.curvatures.iter_mut().zip(grads) {
            curvature.update(grad);
        }
    }

    /// Get number of components
    pub fn num_components(&self) -> usize {
        self.curvatures.len()
    }

    /// Compute product distance
    ///
    /// d²((x₁,...,xₖ), (y₁,...,yₖ)) = Σᵢ wᵢ² dᵢ²(xᵢ, yᵢ)
    pub fn product_distance_squared(&self, distances_squared: &[f32]) -> f32 {
        assert_eq!(distances_squared.len(), self.weights.len());

        self.weights
            .iter()
            .zip(distances_squared)
            .map(|(w, d_sq)| w * w * d_sq)
            .sum()
    }
}

// =============================================================================
// COUPLED OPTIMIZATION
// =============================================================================

/// Curvature optimizer with coupled parameter updates
pub struct CoupledCurvatureOptimizer {
    curvature: LearnableCurvature,
    old_curvature: f32,
}

impl CoupledCurvatureOptimizer {
    /// Create new optimizer
    pub fn new(curvature: LearnableCurvature) -> Self {
        let old_curvature = curvature.value();
        Self {
            curvature,
            old_curvature,
        }
    }

    /// Update curvature and rescale parameters
    ///
    /// # Algorithm (from "Optimizing Curvature Learning" 2024):
    /// 1. Compute gradients in current manifold (curvature K_old)
    /// 2. Update parameters: θ_new = RiemannianSGD(θ, ∇_θ L, K_old)
    /// 3. Update curvature: K_new = K_old - α · ∂L/∂K
    /// 4. Rescale parameters to new manifold
    pub fn step(&mut self, curvature_grad: f32) -> f32 {
        self.old_curvature = self.curvature.value();
        self.curvature.update(curvature_grad);
        let new_curvature = self.curvature.value();

        // Return rescaling factor
        new_curvature / self.old_curvature
    }

    /// Rescale Poincaré ball coordinates to new curvature
    pub fn rescale_poincare(&self, coords: &[f32]) -> Vec<f32> {
        let scale = self.curvature.value() / self.old_curvature;
        coords.iter().map(|&x| x * scale).collect()
    }

    /// Get current curvature
    pub fn curvature(&self) -> f32 {
        self.curvature.value()
    }
}

// =============================================================================
// CURVATURE GRADIENT COMPUTATION
// =============================================================================

/// Compute gradient of distance w.r.t. curvature
///
/// For Poincaré ball distance:
/// d(x, y) = 2K · artanh(||(-x) ⊕_K y|| / K)
///
/// ∂d/∂K requires chain rule through Möbius addition
pub fn distance_gradient_wrt_curvature(x: &[f32], y: &[f32], curvature: f32) -> f32 {
    // Numerical gradient (for simplicity - could derive analytically)
    let eps = 1e-4;

    let dist_plus = crate::poincare_embedding::poincare_distance(x, y, curvature + eps);
    let dist_minus = crate::poincare_embedding::poincare_distance(x, y, curvature - eps);

    (dist_plus - dist_minus) / (2.0 * eps)
}

/// Compute gradient of loss w.r.t. curvature using chain rule
pub fn loss_gradient_wrt_curvature(
    loss_grad_distances: &[f32],
    distance_grads_curvature: &[f32],
) -> f32 {
    loss_grad_distances
        .iter()
        .zip(distance_grads_curvature)
        .map(|(dl_dd, dd_dk)| dl_dd * dd_dk)
        .sum()
}

// =============================================================================
// CURVATURE REGULARIZATION
// =============================================================================

/// Regularization term for curvature
///
/// Encourages moderate curvature values to prevent extreme geometries
#[derive(Clone, Debug)]
pub struct CurvatureRegularization {
    /// Target curvature (prefer values near this)
    target: f32,
    /// Regularization strength
    strength: f32,
}

impl CurvatureRegularization {
    pub fn new(target: f32, strength: f32) -> Self {
        Self { target, strength }
    }

    /// Compute regularization loss
    pub fn loss(&self, curvature: f32) -> f32 {
        self.strength * (curvature - self.target).powi(2)
    }

    /// Gradient of regularization w.r.t. curvature
    pub fn gradient(&self, curvature: f32) -> f32 {
        2.0 * self.strength * (curvature - self.target)
    }
}

// =============================================================================
// ADAPTIVE CURVATURE SELECTOR
// =============================================================================

/// Automatically select curvature based on data hierarchy
pub struct AdaptiveCurvatureSelector {
    /// Minimum observed distance
    min_dist: f32,
    /// Maximum observed distance
    max_dist: f32,
    /// Estimated hierarchy depth
    depth: usize,
}

impl AdaptiveCurvatureSelector {
    pub fn new() -> Self {
        Self {
            min_dist: f32::MAX,
            max_dist: 0.0,
            depth: 1,
        }
    }

    /// Update statistics from batch of distances
    pub fn update(&mut self, distances: &[f32]) {
        if let Some(&min) = distances.iter().min_by(|a, b| a.partial_cmp(b).unwrap()) {
            self.min_dist = self.min_dist.min(min);
        }

        if let Some(&max) = distances.iter().max_by(|a, b| a.partial_cmp(b).unwrap()) {
            self.max_dist = self.max_dist.max(max);
        }
    }

    /// Estimate optimal curvature
    ///
    /// Heuristic: K ≈ max_dist / ln(depth)
    pub fn suggest_curvature(&self) -> f32 {
        let depth_factor = (self.depth as f32).ln().max(1.0);
        (self.max_dist / depth_factor).max(0.1)
    }

    /// Set estimated hierarchy depth
    pub fn with_depth(mut self, depth: usize) -> Self {
        self.depth = depth.max(1);
        self
    }
}

impl Default for AdaptiveCurvatureSelector {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learnable_curvature_positive() {
        let curvature = LearnableCurvature::new(1.0);
        assert!(curvature.value() > 0.0);
    }

    #[test]
    fn test_curvature_update() {
        let mut curvature = LearnableCurvature::new(1.0);
        let initial = curvature.value();

        curvature.update(0.1); // Positive gradient -> decrease
        assert!(curvature.value() < initial);
    }

    #[test]
    fn test_curvature_bounds() {
        let mut curvature = LearnableCurvature::new(1.0).with_bounds(0.5, 2.0);

        // Try to push below minimum
        for _ in 0..100 {
            curvature.update(-10.0);
        }
        assert!(curvature.value() >= 0.5);

        // Try to push above maximum
        for _ in 0..100 {
            curvature.update(10.0);
        }
        assert!(curvature.value() <= 2.0);
    }

    #[test]
    fn test_multi_curvature() {
        let multi = MultiCurvature::new(3, 1.0);
        assert_eq!(multi.num_components(), 3);

        let values = multi.values();
        assert_eq!(values.len(), 3);
        assert!(values.iter().all(|&v| (v - 1.0).abs() < 1e-6));
    }

    #[test]
    fn test_coupled_optimizer() {
        let curvature = LearnableCurvature::new(1.0);
        let mut optimizer = CoupledCurvatureOptimizer::new(curvature);

        let initial = optimizer.curvature();
        optimizer.step(0.1);
        let updated = optimizer.curvature();

        assert!(updated != initial);
    }

    #[test]
    fn test_regularization() {
        let reg = CurvatureRegularization::new(1.0, 0.1);

        let loss_at_target = reg.loss(1.0);
        let loss_away = reg.loss(2.0);

        assert!(loss_at_target < loss_away);
    }

    #[test]
    fn test_adaptive_selector() {
        let mut selector = AdaptiveCurvatureSelector::new().with_depth(3);

        let distances = vec![0.1, 0.5, 1.0, 2.0];
        selector.update(&distances);

        let suggested = selector.suggest_curvature();
        assert!(suggested > 0.0);
    }

    #[test]
    fn test_product_distance() {
        let multi = MultiCurvature::new(2, 1.0);
        let distances_sq = vec![1.0, 4.0]; // d₁=1, d₂=2

        let product_dist_sq = multi.product_distance_squared(&distances_sq);

        // Should be weighted sum: w₁²·1 + w₂²·4
        // With w₁=w₂=1/√2: 0.5·1 + 0.5·4 = 2.5
        assert!((product_dist_sq - 2.5).abs() < 1e-6);
    }
}
