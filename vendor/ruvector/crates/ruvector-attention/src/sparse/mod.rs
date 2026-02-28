//! Sparse attention mechanisms for efficient computation on long sequences
//!
//! This module provides sparse attention patterns that reduce complexity from O(n²) to sub-quadratic.

pub mod flash;
pub mod linear;
pub mod local_global;
pub mod mask;

pub use flash::FlashAttention;
pub use linear::LinearAttention;
pub use local_global::LocalGlobalAttention;
pub use mask::{AttentionMask, SparseMaskBuilder};
