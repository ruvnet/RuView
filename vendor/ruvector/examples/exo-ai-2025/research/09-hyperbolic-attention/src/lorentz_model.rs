//! Lorentz (Hyperboloid) Model Implementation
//!
//! Superior numerical stability compared to Poincaré ball.
//! No boundary singularities, natural linear transformations.
//!
//! # Mathematical Background
//!
//! Hyperboloid: ℍⁿ = {x ∈ ℝⁿ⁺¹ : ⟨x,x⟩_L = -K², x₀ > 0}
//! Minkowski inner product: ⟨x,y⟩_L = -x₀y₀ + x₁y₁ + ... + xₙyₙ
//! Distance: d(x,y) = K · arcosh(-⟨x,y⟩_L / K²)

use std::f32::consts::PI;

const EPS: f32 = 1e-10;

/// Point on Lorentz hyperboloid
#[derive(Clone, Debug)]
pub struct LorentzPoint {
    /// Coordinates in ℝⁿ⁺¹ (x₀ is time-like, x₁..xₙ space-like)
    pub coords: Vec<f32>,
    pub curvature: f32, // K parameter
}

impl LorentzPoint {
    /// Create new point with constraint validation
    pub fn new(coords: Vec<f32>, curvature: f32) -> Result<Self, &'static str> {
        if curvature <= 0.0 {
            return Err("Curvature must be positive");
        }

        if coords.is_empty() {
            return Err("Coordinates cannot be empty");
        }

        let inner = minkowski_inner(&coords, &coords);
        let k_sq = curvature * curvature;

        if (inner + k_sq).abs() > 1e-3 {
            return Err("Point not on hyperboloid: ⟨x,x⟩_L ≠ -K²");
        }

        if coords[0] <= 0.0 {
            return Err("Time component must be positive");
        }

        Ok(Self { coords, curvature })
    }

    /// Create from space-like coordinates (automatically compute time component)
    pub fn from_spatial(spatial: Vec<f32>, curvature: f32) -> Self {
        let k_sq = curvature * curvature;
        let spatial_norm_sq: f32 = spatial.iter().map(|x| x * x).sum();
        let time = (k_sq + spatial_norm_sq).sqrt();

        let mut coords = vec![time];
        coords.extend(spatial);

        Self { coords, curvature }
    }

    /// Project to Poincaré ball for visualization
    pub fn to_poincare(&self) -> Vec<f32> {
        let k = self.curvature;
        // Stereographic projection: x_i / (K + x_0)
        let denom = k + self.coords[0];
        self.coords[1..].iter().map(|&x| k * x / denom).collect()
    }

    /// Dimension (excluding time component)
    pub fn spatial_dim(&self) -> usize {
        self.coords.len() - 1
    }
}

// =============================================================================
// MINKOWSKI OPERATIONS
// =============================================================================

/// Minkowski inner product: ⟨x,y⟩_L = -x₀y₀ + x₁y₁ + ... + xₙyₙ
#[inline]
pub fn minkowski_inner(x: &[f32], y: &[f32]) -> f32 {
    debug_assert_eq!(x.len(), y.len());
    debug_assert!(!x.is_empty());

    let time_part = -x[0] * y[0];
    let space_part: f32 = x[1..].iter().zip(&y[1..]).map(|(xi, yi)| xi * yi).sum();

    time_part + space_part
}

/// Lorentz distance: d(x,y) = K · arcosh(-⟨x,y⟩_L / K²)
///
/// Numerically stable formula using log.
pub fn lorentz_distance(x: &[f32], y: &[f32], curvature: f32) -> f32 {
    let k_sq = curvature * curvature;
    let inner = minkowski_inner(x, y);
    let arg = -inner / k_sq;

    // arcosh(z) = ln(z + sqrt(z² - 1))
    // Stable for z >= 1
    let arg_clamped = arg.max(1.0);
    curvature * (arg_clamped + (arg_clamped * arg_clamped - 1.0).sqrt()).ln()
}

/// Project point onto hyperboloid constraint
///
/// Ensures ⟨x,x⟩_L = -K² and x₀ > 0
pub fn project_to_hyperboloid(coords: &mut Vec<f32>, curvature: f32) {
    if coords.is_empty() {
        return;
    }

    let k_sq = curvature * curvature;
    let spatial_norm_sq: f32 = coords[1..].iter().map(|x| x * x).sum();
    coords[0] = (k_sq + spatial_norm_sq).sqrt().max(EPS);
}

// =============================================================================
// HYPERBOLIC OPERATIONS
// =============================================================================

/// Exponential map on hyperboloid: exp_x(v)
///
/// Formula: exp_x(v) = cosh(||v|| / K) x + sinh(||v|| / K) · v / ||v||
///
/// where ||v|| is Minkowski norm: √⟨v,v⟩_L
pub fn lorentz_exp(x: &[f32], v: &[f32], curvature: f32) -> Vec<f32> {
    debug_assert_eq!(x.len(), v.len());

    let v_norm_sq = minkowski_inner(v, v);

    // Handle zero vector
    if v_norm_sq.abs() < EPS {
        return x.to_vec();
    }

    let v_norm = v_norm_sq.abs().sqrt();
    let theta = v_norm / curvature;

    let cosh_theta = theta.cosh();
    let sinh_theta = theta.sinh();
    let scale = sinh_theta / v_norm;

    x.iter()
        .zip(v.iter())
        .map(|(&xi, &vi)| cosh_theta * xi + scale * vi)
        .collect()
}

/// Logarithmic map on hyperboloid: log_x(y)
///
/// Formula: log_x(y) = d(x,y) / sinh(d(x,y)/K) · (y + (⟨x,y⟩_L/K²) x)
pub fn lorentz_log(x: &[f32], y: &[f32], curvature: f32) -> Vec<f32> {
    debug_assert_eq!(x.len(), y.len());

    let k = curvature;
    let k_sq = k * k;
    let dist = lorentz_distance(x, y, k);

    if dist < EPS {
        return vec![0.0; x.len()];
    }

    let theta = dist / k;
    let inner_xy = minkowski_inner(x, y);
    let scale = theta / theta.sinh();

    x.iter()
        .zip(y.iter())
        .map(|(&xi, &yi)| scale * (yi + (inner_xy / k_sq) * xi))
        .collect()
}

/// Parallel transport of tangent vector v from x to y
///
/// Preserves Minkowski inner products.
pub fn parallel_transport(x: &[f32], y: &[f32], v: &[f32], curvature: f32) -> Vec<f32> {
    let k_sq = curvature * curvature;
    let inner_xy = minkowski_inner(x, y);

    // λ = -⟨x,y⟩_L / K²
    let lambda = -inner_xy / k_sq;

    // P_{x→y}(v) = v + ((λ-1)/K²)(⟨x,v⟩_L y + ⟨y,v⟩_L x)
    let inner_xv = minkowski_inner(x, v);
    let inner_yv = minkowski_inner(y, v);
    let coef = (lambda - 1.0) / k_sq;

    v.iter()
        .zip(y.iter())
        .zip(x.iter())
        .map(|((&vi, &yi), &xi)| vi + coef * (inner_xv * yi + inner_yv * xi))
        .collect()
}

// =============================================================================
// LORENTZ TRANSFORMATIONS
// =============================================================================

/// Lorentz boost: translation along time-like direction
///
/// Moves point x by velocity v (in tangent space).
pub fn lorentz_boost(x: &[f32], v: &[f32], curvature: f32) -> Vec<f32> {
    // Boost = exponential map
    lorentz_exp(x, v, curvature)
}

/// Lorentz rotation: rotation in space-like plane
///
/// Rotates spatial coordinates by angle θ in plane (i, j).
pub fn lorentz_rotation(x: &[f32], angle: f32, plane_i: usize, plane_j: usize) -> Vec<f32> {
    let mut result = x.to_vec();

    if plane_i == 0 || plane_j == 0 {
        // Don't rotate time component
        return result;
    }

    let cos_theta = angle.cos();
    let sin_theta = angle.sin();

    let xi = x[plane_i];
    let xj = x[plane_j];

    result[plane_i] = cos_theta * xi - sin_theta * xj;
    result[plane_j] = sin_theta * xi + cos_theta * xj;

    result
}

// =============================================================================
// CONVERSION FUNCTIONS
// =============================================================================

/// Convert from Poincaré ball to Lorentz hyperboloid
///
/// Formula: (x₀, x₁, ..., xₙ) where
///   x₀ = K(1 + ||p||²/K²) / (1 - ||p||²/K²)
///   xᵢ = 2Kpᵢ / (1 - ||p||²/K²)  for i ≥ 1
pub fn poincare_to_lorentz(poincare: &[f32], curvature: f32) -> Vec<f32> {
    let k = curvature;
    let k_sq = k * k;
    let p_norm_sq: f32 = poincare.iter().map(|x| x * x).sum();

    let denom = 1.0 - p_norm_sq / k_sq;
    let time = k * (1.0 + p_norm_sq / k_sq) / denom;

    let mut coords = vec![time];
    coords.extend(poincare.iter().map(|&pi| 2.0 * k * pi / denom));

    coords
}

/// Convert from Lorentz hyperboloid to Poincaré ball
///
/// Inverse stereographic projection.
pub fn lorentz_to_poincare(lorentz: &[f32], curvature: f32) -> Vec<f32> {
    let k = curvature;
    let denom = k + lorentz[0];

    lorentz[1..].iter().map(|&xi| k * xi / denom).collect()
}

// =============================================================================
// BATCH OPERATIONS
// =============================================================================

/// Compute all distances from query to database
pub fn batch_lorentz_distances(query: &[f32], database: &[Vec<f32>], curvature: f32) -> Vec<f32> {
    database
        .iter()
        .map(|point| lorentz_distance(query, point, curvature))
        .collect()
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const APPROX_EPS: f32 = 1e-3;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < APPROX_EPS
    }

    #[test]
    fn test_minkowski_inner_product() {
        let x = vec![2.0, 1.0, 0.0];
        let y = vec![3.0, 0.0, 1.0];

        // ⟨x,y⟩_L = -2*3 + 1*0 + 0*1 = -6
        let inner = minkowski_inner(&x, &y);
        assert!(approx_eq(inner, -6.0));
    }

    #[test]
    fn test_hyperboloid_constraint() {
        let k = 1.0;
        let spatial = vec![0.5, 0.3];
        let point = LorentzPoint::from_spatial(spatial, k);

        let inner = minkowski_inner(&point.coords, &point.coords);
        assert!(approx_eq(inner, -k * k));
    }

    #[test]
    fn test_lorentz_distance_symmetry() {
        let k = 1.0;
        let x = LorentzPoint::from_spatial(vec![0.1, 0.2], k);
        let y = LorentzPoint::from_spatial(vec![0.3, 0.1], k);

        let d1 = lorentz_distance(&x.coords, &y.coords, k);
        let d2 = lorentz_distance(&y.coords, &x.coords, k);

        assert!(approx_eq(d1, d2));
    }

    #[test]
    fn test_exp_log_inverse() {
        let k = 1.0;
        let x = LorentzPoint::from_spatial(vec![0.1, 0.2], k);
        let y = LorentzPoint::from_spatial(vec![0.3, 0.1], k);

        let v = lorentz_log(&x.coords, &y.coords, k);
        let y_recon = lorentz_exp(&x.coords, &v, k);

        for (a, b) in y_recon.iter().zip(&y.coords) {
            assert!(approx_eq(*a, *b));
        }
    }

    #[test]
    fn test_poincare_lorentz_conversion() {
        let k = 1.0;
        let poincare = vec![0.5, 0.3];

        let lorentz = poincare_to_lorentz(&poincare, k);
        let poincare_recon = lorentz_to_poincare(&lorentz, k);

        for (a, b) in poincare.iter().zip(&poincare_recon) {
            assert!(approx_eq(*a, *b));
        }
    }

    #[test]
    fn test_project_to_hyperboloid() {
        let k = 1.0;
        let mut coords = vec![1.5, 0.5, 0.3];

        project_to_hyperboloid(&mut coords, k);

        let inner = minkowski_inner(&coords, &coords);
        assert!(approx_eq(inner, -k * k));
    }

    #[test]
    fn test_parallel_transport_preserves_norm() {
        let k = 1.0;
        let x = LorentzPoint::from_spatial(vec![0.1, 0.0], k);
        let y = LorentzPoint::from_spatial(vec![0.2, 0.0], k);
        let v = vec![0.0, 0.1, 0.2]; // Tangent vector at x

        let v_transported = parallel_transport(&x.coords, &y.coords, &v, k);

        let norm_before = minkowski_inner(&v, &v);
        let norm_after = minkowski_inner(&v_transported, &v_transported);

        assert!(approx_eq(norm_before, norm_after));
    }
}
