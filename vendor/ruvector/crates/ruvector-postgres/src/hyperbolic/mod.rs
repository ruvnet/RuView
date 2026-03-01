// Hyperbolic Embeddings Module
// Implements Poincaré ball and Lorentz hyperboloid models for hierarchical embeddings

pub mod lorentz;
pub mod operators;
pub mod poincare;

pub use lorentz::LorentzModel;
pub use poincare::PoincareBall;

/// Default curvature for hyperbolic space
pub const DEFAULT_CURVATURE: f32 = -1.0;

/// Epsilon for numerical stability
pub const EPSILON: f32 = 1e-8;

/// Maximum value to prevent overflow
pub const MAX_NORM: f32 = 1.0 - 1e-5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_CURVATURE, -1.0);
        assert!(EPSILON > 0.0);
        assert!(MAX_NORM < 1.0);
    }
}
