//! # RuVector Math
//!
//! Advanced mathematics for next-generation vector search and AI governance, featuring:
//!
//! ## Core Modules
//!
//! - **Optimal Transport**: Wasserstein distances, Sinkhorn algorithm, Sliced Wasserstein
//! - **Information Geometry**: Fisher Information, Natural Gradient, K-FAC
//! - **Product Manifolds**: Mixed-curvature spaces (Euclidean × Hyperbolic × Spherical)
//! - **Spherical Geometry**: Geodesics on the n-sphere for cyclical patterns
//!
//! ## Theoretical CS Modules (New)
//!
//! - **Tropical Algebra**: Max-plus semiring for piecewise linear analysis and routing
//! - **Tensor Networks**: TT/Tucker/CP decomposition for memory compression
//! - **Spectral Methods**: Chebyshev polynomials for graph diffusion without eigendecomposition
//! - **Persistent Homology**: TDA for topological drift detection and coherence monitoring
//! - **Polynomial Optimization**: SOS certificates for provable bounds on attention policies
//!
//! ## Design Principles
//!
//! 1. **Pure Rust**: No BLAS/LAPACK dependencies for full WASM compatibility
//! 2. **SIMD-Ready**: Hot paths optimized for auto-vectorization
//! 3. **Numerically Stable**: Log-domain arithmetic, clamping, and stable softmax
//! 4. **Modular**: Each component usable independently
//! 5. **Mincut as Spine**: All modules designed to integrate with mincut governance
//!
//! ## Architecture: Mincut as Unifying Signal
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Mincut Governance                         │
//! │  (Structural tension meter for attention graphs)            │
//! └───────────────────────┬─────────────────────────────────────┘
//!                         │
//!     ┌───────────────────┼───────────────────┐
//!     ▼                   ▼                   ▼
//! ┌─────────┐       ┌───────────┐       ┌───────────┐
//! │ Tensor  │       │ Spectral  │       │   TDA     │
//! │ Networks│       │ Methods   │       │ Homology  │
//! │ (TT)    │       │(Chebyshev)│       │           │
//! └─────────┘       └───────────┘       └───────────┘
//! Compress          Smooth within       Monitor drift
//! representations   partitions          over time
//!
//!     ┌───────────────────┼───────────────────┐
//!     ▼                   ▼                   ▼
//! ┌─────────┐       ┌───────────┐       ┌───────────┐
//! │Tropical │       │    SOS    │       │ Optimal   │
//! │ Algebra │       │ Certs     │       │ Transport │
//! └─────────┘       └───────────┘       └───────────┘
//! Plan safe         Certify policy      Measure
//! routing paths     constraints         distributional
//!                                       distances
//! ```
//!
//! ## Quick Start
//!
//! ```rust
//! use ruvector_math::optimal_transport::{SlicedWasserstein, SinkhornSolver, OptimalTransport};
//! use ruvector_math::information_geometry::FisherInformation;
//! use ruvector_math::product_manifold::ProductManifold;
//!
//! // Sliced Wasserstein distance between point clouds
//! let sw = SlicedWasserstein::new(100).with_seed(42);
//! let points_a = vec![vec![0.0, 0.0], vec![1.0, 0.0]];
//! let points_b = vec![vec![0.5, 0.5], vec![1.5, 0.5]];
//! let dist = sw.distance(&points_a, &points_b);
//! assert!(dist > 0.0);
//!
//! // Sinkhorn optimal transport
//! let solver = SinkhornSolver::new(0.1, 100);
//! let cost_matrix = vec![vec![0.0, 1.0], vec![1.0, 0.0]];
//! let weights_a = vec![0.5, 0.5];
//! let weights_b = vec![0.5, 0.5];
//! let result = solver.solve(&cost_matrix, &weights_a, &weights_b).unwrap();
//! assert!(result.converged);
//!
//! // Product manifold operations (Euclidean only for simplicity)
//! let manifold = ProductManifold::new(2, 0, 0);
//! let point_a = vec![0.0, 0.0];
//! let point_b = vec![3.0, 4.0];
//! let dist = manifold.distance(&point_a, &point_b).unwrap();
//! assert!((dist - 5.0).abs() < 1e-10);
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

// Core modules
pub mod error;
pub mod information_geometry;
pub mod optimal_transport;
pub mod product_manifold;
pub mod spherical;
pub mod utils;

// New theoretical CS modules
pub mod homology;
pub mod optimization;
pub mod spectral;
pub mod tensor_networks;
pub mod tropical;

// Re-exports for convenience - Core
pub use error::{MathError, Result};
pub use information_geometry::{FisherInformation, KFACApproximation, NaturalGradient};
pub use optimal_transport::{
    GromovWasserstein, SinkhornSolver, SlicedWasserstein, TransportPlan, WassersteinConfig,
};
pub use product_manifold::{CurvatureType, ProductManifold, ProductManifoldConfig};
pub use spherical::{SphericalConfig, SphericalSpace};

// Re-exports - Tropical Algebra
pub use tropical::{LinearRegionCounter, TropicalNeuralAnalysis};
pub use tropical::{Tropical, TropicalMatrix, TropicalPolynomial, TropicalSemiring};

// Re-exports - Tensor Networks
pub use tensor_networks::{CPConfig, CPDecomposition, TuckerConfig, TuckerDecomposition};
pub use tensor_networks::{DenseTensor, TensorTrain, TensorTrainConfig};
pub use tensor_networks::{TensorNetwork, TensorNode};

// Re-exports - Spectral Methods
pub use spectral::ScaledLaplacian;
pub use spectral::{ChebyshevExpansion, ChebyshevPolynomial};
pub use spectral::{FilterType, GraphFilter, SpectralFilter};
pub use spectral::{GraphWavelet, SpectralClustering, SpectralWaveletTransform};

// Re-exports - Homology
pub use homology::{BirthDeathPair, PersistenceDiagram, PersistentHomology};
pub use homology::{BottleneckDistance, WassersteinDistance as HomologyWasserstein};
pub use homology::{Filtration, Simplex, SimplicialComplex, VietorisRips};

// Re-exports - Optimization
pub use optimization::{BoundsCertificate, NonnegativityCertificate};
pub use optimization::{Monomial, Polynomial, Term};
pub use optimization::{SOSDecomposition, SOSResult};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::error::*;
    pub use crate::homology::*;
    pub use crate::information_geometry::*;
    pub use crate::optimal_transport::*;
    pub use crate::optimization::*;
    pub use crate::product_manifold::*;
    pub use crate::spectral::*;
    pub use crate::spherical::*;
    pub use crate::tensor_networks::*;
    pub use crate::tropical::*;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crate_version() {
        let version = env!("CARGO_PKG_VERSION");
        assert!(!version.is_empty());
    }
}
