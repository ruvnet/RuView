//! Optimal Transport Attention
//!
//! Fast attention mechanisms using Optimal Transport theory.
//!
//! ## Key Optimizations
//!
//! 1. **Sliced Wasserstein**: Random 1D projections with cached sorted orders
//! 2. **Centroid OT**: Cluster keys into M centroids, transport to prototypes only
//! 3. **Two-Stage**: Cheap prefilter + expensive OT kernel on candidates
//! 4. **Histogram CDF**: Replace sorting with binned CDFs for SIMD-friendly ops
//!
//! ## Performance Targets
//!
//! - Candidates C: 32-64
//! - Projections P: 8-16
//! - Centroids M: 16-32

mod cached_projections;
mod centroid_ot;
mod sliced_wasserstein;

pub use cached_projections::{ProjectionCache, WindowCache};
pub use centroid_ot::{CentroidCache, CentroidOTAttention, CentroidOTConfig};
pub use sliced_wasserstein::{SlicedWassersteinAttention, SlicedWassersteinConfig};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exists() {
        // Basic module test
        assert!(true);
    }
}
