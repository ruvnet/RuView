//! Cross-viewpoint embedding fusion for multistatic WiFi sensing (ADR-031).
//!
//! This module implements the RuView fusion pipeline that combines per-viewpoint
//! AETHER embeddings into a single fused embedding using learned cross-viewpoint
//! attention with geometric bias.
//!
//! # Submodules
//!
//! - [`attention`]: Cross-viewpoint scaled dot-product attention with geometric
//!   bias encoding angular separation and baseline distance between viewpoint pairs.
//! - [`geometry`]: Geometric Diversity Index (GDI) computation and Cramer-Rao
//!   bound estimation for array geometry quality assessment.
//! - [`coherence`]: Coherence gating that determines whether the environment is
//!   stable enough for a model update based on phase consistency.
//! - [`fusion`]: `MultistaticArray` aggregate root that orchestrates the full
//!   fusion pipeline from per-viewpoint embeddings to a single fused output.

pub mod attention;
pub mod coherence;
pub mod fusion;
pub mod geometry;

// Re-export primary types at the module root for ergonomic imports.
pub use attention::{CrossViewpointAttention, GeometricBias};
pub use coherence::{CoherenceGate, CoherenceState};
pub use fusion::{FusedEmbedding, FusionConfig, MultistaticArray, ViewpointEmbedding};
pub use geometry::{CramerRaoBound, GeometricDiversityIndex};
