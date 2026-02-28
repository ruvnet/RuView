//! Information Bottleneck
//!
//! Variational Information Bottleneck (VIB) components for attention.
//!
//! ## Key Concepts
//!
//! 1. **KL Divergence**: Measure compression quality
//! 2. **Rate-Distortion**: Balance compression vs. reconstruction
//! 3. **Per-Layer Bottleneck**: Add IB loss term to each attention layer
//!
//! ## Applications
//!
//! - Preventing attention from memorizing noise
//! - Encouraging sparse, meaningful attention patterns
//! - Regularizing attention weights

mod bottleneck;
mod kl_divergence;

pub use bottleneck::{IBConfig, InformationBottleneck};
pub use kl_divergence::{DiagonalGaussian, KLDivergence};

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_exists() {
        assert!(true);
    }
}
