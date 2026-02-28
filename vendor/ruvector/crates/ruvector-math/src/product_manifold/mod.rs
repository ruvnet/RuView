//! Product Manifolds: Mixed-Curvature Geometry
//!
//! Real-world data often combines multiple structural types:
//! - **Hierarchical**: Trees, taxonomies → Hyperbolic space (H^n)
//! - **Flat/Grid**: General embeddings → Euclidean space (E^n)
//! - **Cyclical**: Periodic patterns → Spherical space (S^n)
//!
//! Product manifolds combine these: M = H^h × E^e × S^s
//!
//! ## Benefits
//!
//! - **20x memory reduction** on taxonomy data vs pure Euclidean
//! - **Better hierarchy preservation** through hyperbolic components
//! - **Natural cyclical modeling** through spherical components
//!
//! ## References
//!
//! - Gu et al. (2019): Learning Mixed-Curvature Representations in Product Spaces
//! - Skopek et al. (2020): Mixed-Curvature VAEs

mod config;
mod manifold;
mod operations;

pub use config::{CurvatureType, ProductManifoldConfig};
pub use manifold::ProductManifold;

// Re-export batch operations (used internally by ProductManifold impl)
#[doc(hidden)]
pub mod ops {
    pub use super::operations::*;
}
