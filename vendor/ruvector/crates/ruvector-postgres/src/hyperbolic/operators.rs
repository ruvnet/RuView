// PostgreSQL Functions for Hyperbolic Operations
// Exposes hyperbolic geometry functions to SQL

use pgrx::prelude::*;

use super::{lorentz::LorentzModel, poincare::PoincareBall, DEFAULT_CURVATURE};

/// Compute Poincaré distance between two vectors
///
/// # Arguments
/// * `a` - First vector
/// * `b` - Second vector
/// * `curvature` - Curvature of hyperbolic space (default: -1.0)
///
/// # Returns
/// Poincaré distance as f32
///
/// # Example
/// ```sql
/// SELECT ruvector_poincare_distance(
///     ARRAY[0.3, 0.4]::real[],
///     ARRAY[0.1, 0.2]::real[],
///     -1.0
/// );
/// ```
#[pg_extern(immutable, parallel_safe)]
fn ruvector_poincare_distance(
    a: Vec<f32>,
    b: Vec<f32>,
    curvature: default!(f32, "DEFAULT_CURVATURE"),
) -> f32 {
    if a.is_empty() || b.is_empty() {
        error!("Vectors cannot be empty");
    }
    if a.len() != b.len() {
        error!("Vectors must have the same dimension");
    }
    if curvature >= 0.0 {
        error!("Curvature must be negative");
    }

    let ball = PoincareBall::new(curvature);
    ball.distance(&a, &b)
}

/// Compute Lorentz/hyperboloid distance between two vectors
///
/// # Arguments
/// * `a` - First vector (on hyperboloid)
/// * `b` - Second vector (on hyperboloid)
/// * `curvature` - Curvature of hyperbolic space (default: -1.0)
///
/// # Returns
/// Lorentz distance as f32
///
/// # Example
/// ```sql
/// SELECT ruvector_lorentz_distance(
///     ARRAY[1.5, 1.0, 0.5]::real[],
///     ARRAY[2.0, 1.5, 0.5]::real[],
///     -1.0
/// );
/// ```
#[pg_extern(immutable, parallel_safe)]
fn ruvector_lorentz_distance(
    a: Vec<f32>,
    b: Vec<f32>,
    curvature: default!(f32, "DEFAULT_CURVATURE"),
) -> f32 {
    if a.len() < 2 || b.len() < 2 {
        error!("Lorentz vectors must have at least 2 dimensions");
    }
    if a.len() != b.len() {
        error!("Vectors must have the same dimension");
    }
    if curvature >= 0.0 {
        error!("Curvature must be negative");
    }

    let model = LorentzModel::new(curvature);
    model.distance(&a, &b)
}

/// Perform Möbius addition in Poincaré ball
///
/// # Arguments
/// * `a` - First vector
/// * `b` - Second vector
/// * `curvature` - Curvature of hyperbolic space (default: -1.0)
///
/// # Returns
/// Result of Möbius addition
///
/// # Example
/// ```sql
/// SELECT ruvector_mobius_add(
///     ARRAY[0.3, 0.4]::real[],
///     ARRAY[0.1, 0.1]::real[],
///     -1.0
/// );
/// ```
#[pg_extern(immutable, parallel_safe)]
fn ruvector_mobius_add(
    a: Vec<f32>,
    b: Vec<f32>,
    curvature: default!(f32, "DEFAULT_CURVATURE"),
) -> Vec<f32> {
    if a.is_empty() || b.is_empty() {
        error!("Vectors cannot be empty");
    }
    if a.len() != b.len() {
        error!("Vectors must have the same dimension");
    }
    if curvature >= 0.0 {
        error!("Curvature must be negative");
    }

    let ball = PoincareBall::new(curvature);
    ball.mobius_add(&a, &b)
}

/// Exponential map in Poincaré ball
/// Maps tangent vector at base point to the manifold
///
/// # Arguments
/// * `base` - Base point on the manifold
/// * `tangent` - Tangent vector at base point
/// * `curvature` - Curvature of hyperbolic space (default: -1.0)
///
/// # Returns
/// Point on the manifold
///
/// # Example
/// ```sql
/// SELECT ruvector_exp_map(
///     ARRAY[0.2, 0.3]::real[],
///     ARRAY[0.1, 0.1]::real[],
///     -1.0
/// );
/// ```
#[pg_extern(immutable, parallel_safe)]
fn ruvector_exp_map(
    base: Vec<f32>,
    tangent: Vec<f32>,
    curvature: default!(f32, "DEFAULT_CURVATURE"),
) -> Vec<f32> {
    if base.is_empty() || tangent.is_empty() {
        error!("Vectors cannot be empty");
    }
    if base.len() != tangent.len() {
        error!("Vectors must have the same dimension");
    }
    if curvature >= 0.0 {
        error!("Curvature must be negative");
    }

    let ball = PoincareBall::new(curvature);
    ball.exp_map(&base, &tangent)
}

/// Logarithmic map in Poincaré ball
/// Maps point on manifold to tangent space at base point
///
/// # Arguments
/// * `base` - Base point on the manifold
/// * `target` - Target point on the manifold
/// * `curvature` - Curvature of hyperbolic space (default: -1.0)
///
/// # Returns
/// Tangent vector at base point
///
/// # Example
/// ```sql
/// SELECT ruvector_log_map(
///     ARRAY[0.2, 0.3]::real[],
///     ARRAY[0.4, 0.5]::real[],
///     -1.0
/// );
/// ```
#[pg_extern(immutable, parallel_safe)]
fn ruvector_log_map(
    base: Vec<f32>,
    target: Vec<f32>,
    curvature: default!(f32, "DEFAULT_CURVATURE"),
) -> Vec<f32> {
    if base.is_empty() || target.is_empty() {
        error!("Vectors cannot be empty");
    }
    if base.len() != target.len() {
        error!("Vectors must have the same dimension");
    }
    if curvature >= 0.0 {
        error!("Curvature must be negative");
    }

    let ball = PoincareBall::new(curvature);
    ball.log_map(&base, &target)
}

/// Convert from Poincaré ball to Lorentz hyperboloid coordinates
///
/// # Arguments
/// * `poincare` - Vector in Poincaré ball
/// * `curvature` - Curvature of hyperbolic space (default: -1.0)
///
/// # Returns
/// Vector in Lorentz hyperboloid coordinates
///
/// # Example
/// ```sql
/// SELECT ruvector_poincare_to_lorentz(
///     ARRAY[0.3, 0.4]::real[],
///     -1.0
/// );
/// ```
#[pg_extern(immutable, parallel_safe)]
fn ruvector_poincare_to_lorentz(
    poincare: Vec<f32>,
    curvature: default!(f32, "DEFAULT_CURVATURE"),
) -> Vec<f32> {
    if poincare.is_empty() {
        error!("Vector cannot be empty");
    }
    if curvature >= 0.0 {
        error!("Curvature must be negative");
    }

    let model = LorentzModel::new(curvature);
    model.from_poincare(&poincare)
}

/// Convert from Lorentz hyperboloid to Poincaré ball coordinates
///
/// # Arguments
/// * `lorentz` - Vector in Lorentz hyperboloid coordinates
/// * `curvature` - Curvature of hyperbolic space (default: -1.0)
///
/// # Returns
/// Vector in Poincaré ball
///
/// # Example
/// ```sql
/// SELECT ruvector_lorentz_to_poincare(
///     ARRAY[1.5, 1.0, 0.5]::real[],
///     -1.0
/// );
/// ```
#[pg_extern(immutable, parallel_safe)]
fn ruvector_lorentz_to_poincare(
    lorentz: Vec<f32>,
    curvature: default!(f32, "DEFAULT_CURVATURE"),
) -> Vec<f32> {
    if lorentz.len() < 2 {
        error!("Lorentz vector must have at least 2 dimensions");
    }
    if curvature >= 0.0 {
        error!("Curvature must be negative");
    }

    let model = LorentzModel::new(curvature);
    model.to_poincare(&lorentz)
}

/// Compute Minkowski inner product for Lorentz model
///
/// # Arguments
/// * `a` - First vector
/// * `b` - Second vector
///
/// # Returns
/// Minkowski inner product
///
/// # Example
/// ```sql
/// SELECT ruvector_minkowski_dot(
///     ARRAY[2.0, 1.0, 1.0]::real[],
///     ARRAY[3.0, 2.0, 1.0]::real[]
/// );
/// ```
#[pg_extern(immutable, parallel_safe)]
fn ruvector_minkowski_dot(a: Vec<f32>, b: Vec<f32>) -> f32 {
    if a.len() < 2 || b.len() < 2 {
        error!("Vectors must have at least 2 dimensions");
    }
    if a.len() != b.len() {
        error!("Vectors must have the same dimension");
    }

    let model = LorentzModel::new(DEFAULT_CURVATURE);
    model.minkowski_dot(&a, &b)
}

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    const TOL: f32 = 1e-4;

    #[pg_test]
    fn test_poincare_distance_basic() {
        let a = vec![0.0, 0.0];
        let b = vec![0.5, 0.0];
        let dist = ruvector_poincare_distance(a, b, DEFAULT_CURVATURE);
        assert!(dist > 0.0);
        assert!(dist < f32::INFINITY);
    }

    #[pg_test]
    fn test_poincare_distance_symmetric() {
        let a = vec![0.3, 0.4];
        let b = vec![0.1, 0.2];
        let d1 = ruvector_poincare_distance(a.clone(), b.clone(), DEFAULT_CURVATURE);
        let d2 = ruvector_poincare_distance(b, a, DEFAULT_CURVATURE);
        assert!((d1 - d2).abs() < TOL);
    }

    #[pg_test]
    fn test_poincare_distance_same() {
        let a = vec![0.3, 0.4];
        let dist = ruvector_poincare_distance(a.clone(), a, DEFAULT_CURVATURE);
        assert!(dist < TOL);
    }

    #[pg_test]
    fn test_lorentz_distance_basic() {
        let a = vec![1.5, 1.0, 0.5];
        let b = vec![2.0, 1.5, 0.5];
        let dist = ruvector_lorentz_distance(a, b, DEFAULT_CURVATURE);
        assert!(dist > 0.0);
        assert!(dist < f32::INFINITY);
    }

    #[pg_test]
    fn test_mobius_add_identity() {
        let a = vec![0.3, 0.4];
        let origin = vec![0.0, 0.0];
        let result = ruvector_mobius_add(a.clone(), origin, DEFAULT_CURVATURE);
        for i in 0..a.len() {
            assert!((result[i] - a[i]).abs() < TOL);
        }
    }

    #[pg_test]
    fn test_exp_log_inverse() {
        let base = vec![0.2, 0.3];
        let tangent = vec![0.1, 0.1];

        let point = ruvector_exp_map(base.clone(), tangent.clone(), DEFAULT_CURVATURE);
        let recovered = ruvector_log_map(base, point, DEFAULT_CURVATURE);

        for i in 0..tangent.len() {
            assert!((recovered[i] - tangent[i]).abs() < TOL);
        }
    }

    #[pg_test]
    fn test_poincare_lorentz_conversion() {
        let poincare = vec![0.3, 0.4];
        let lorentz = ruvector_poincare_to_lorentz(poincare.clone(), DEFAULT_CURVATURE);
        let recovered = ruvector_lorentz_to_poincare(lorentz, DEFAULT_CURVATURE);

        for i in 0..poincare.len() {
            assert!((recovered[i] - poincare[i]).abs() < TOL);
        }
    }

    #[pg_test]
    fn test_minkowski_dot_basic() {
        let a = vec![2.0, 1.0, 1.0];
        let b = vec![3.0, 2.0, 1.0];
        let result = ruvector_minkowski_dot(a, b);
        // -2*3 + 1*2 + 1*1 = -3
        assert!((result - (-3.0)).abs() < TOL);
    }

    #[pg_test]
    #[should_panic(expected = "Vectors cannot be empty")]
    fn test_poincare_distance_empty() {
        let _ = ruvector_poincare_distance(vec![], vec![0.1], DEFAULT_CURVATURE);
    }

    #[pg_test]
    #[should_panic(expected = "Vectors must have the same dimension")]
    fn test_poincare_distance_different_dims() {
        let _ = ruvector_poincare_distance(vec![0.1], vec![0.1, 0.2], DEFAULT_CURVATURE);
    }

    #[pg_test]
    #[should_panic(expected = "Curvature must be negative")]
    fn test_poincare_distance_positive_curvature() {
        let _ = ruvector_poincare_distance(vec![0.1], vec![0.2], 1.0);
    }
}
