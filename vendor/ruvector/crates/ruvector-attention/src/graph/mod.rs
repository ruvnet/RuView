//! Graph attention mechanisms for GNN applications
//!
//! This module provides graph-specific attention implementations:
//! - Edge-featured attention (GAT with edge features)
//! - Rotary position embeddings for graphs (RoPE)
//! - Dual-space attention (Euclidean + Hyperbolic)

pub mod dual_space;
pub mod edge_featured;
pub mod rope;

pub use dual_space::{DualSpaceAttention, DualSpaceConfig};
pub use edge_featured::{EdgeFeaturedAttention, EdgeFeaturedConfig};
pub use rope::{GraphRoPE, RoPEConfig};
