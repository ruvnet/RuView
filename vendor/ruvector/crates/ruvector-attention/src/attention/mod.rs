//! Attention mechanism implementations.
//!
//! This module provides concrete implementations of various attention mechanisms
//! including scaled dot-product attention and multi-head attention.

pub mod multi_head;
pub mod scaled_dot_product;

pub use multi_head::MultiHeadAttention;
pub use scaled_dot_product::ScaledDotProductAttention;
