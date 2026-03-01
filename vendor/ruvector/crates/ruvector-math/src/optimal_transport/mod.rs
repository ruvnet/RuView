//! Optimal Transport Algorithms
//!
//! This module provides implementations of optimal transport distances and solvers:
//!
//! - **Sliced Wasserstein Distance**: O(n log n) via random 1D projections
//! - **Sinkhorn Algorithm**: Log-stabilized entropic regularization
//! - **Gromov-Wasserstein**: Cross-space structure comparison
//!
//! ## Theory
//!
//! Optimal transport measures the minimum "cost" to transform one probability
//! distribution into another. The Wasserstein distance (Earth Mover's Distance)
//! is defined as:
//!
//! W_p(μ, ν) = (inf_{γ ∈ Π(μ,ν)} ∫∫ c(x,y)^p dγ(x,y))^{1/p}
//!
//! where Π(μ,ν) is the set of all couplings with marginals μ and ν.
//!
//! ## Use Cases in Vector Search
//!
//! - Cross-lingual document retrieval (comparing embedding distributions)
//! - Image region matching (comparing feature distributions)
//! - Time series pattern matching
//! - Document similarity via word embedding distributions

mod config;
mod gromov_wasserstein;
mod sinkhorn;
mod sliced_wasserstein;

pub use config::WassersteinConfig;
pub use gromov_wasserstein::GromovWasserstein;
pub use sinkhorn::{SinkhornSolver, TransportPlan};
pub use sliced_wasserstein::SlicedWasserstein;

/// Trait for optimal transport distance computations
pub trait OptimalTransport {
    /// Compute the optimal transport distance between two point clouds
    fn distance(&self, source: &[Vec<f64>], target: &[Vec<f64>]) -> f64;

    /// Compute the optimal transport distance with weights
    fn weighted_distance(
        &self,
        source: &[Vec<f64>],
        source_weights: &[f64],
        target: &[Vec<f64>],
        target_weights: &[f64],
    ) -> f64;
}
