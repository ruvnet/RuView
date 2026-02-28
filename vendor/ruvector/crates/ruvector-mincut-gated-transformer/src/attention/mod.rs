//! Attention mechanisms for the transformer.
//!
//! Implements efficient attention variants inspired by:
//! - **Sliding Window Attention** - O(n W) complexity with fixed window size W
//! - **Dynamic Sparse Attention** (Jiang et al., 2024) - 90% FLOPs reduction via top-k selection
//! - **Spike-Driven Attention** (Yao et al., 2023, 2024) - Event-driven sparse compute
//! - **Spectral Attention** (Kreuzer et al., 2021) - Graph-based attention with spectral methods
//!
//! Provides sliding window attention as the default, with optional
//! linear attention for longer sequences, spike-driven attention
//! for energy-efficient inference, and mincut-aware sparse attention.
//!
//! ## References
//!
//! - Jiang, H., et al. (2024). MInference 1.0. NeurIPS 2024.
//! - Yao, M., et al. (2023). Spike-driven Transformer. NeurIPS 2023.
//! - Yao, M., et al. (2024). Spike-driven Transformer V2. ICLR 2024.
//! - Kreuzer, D., et al. (2021). Rethinking Graph Transformers with Spectral Attention. NeurIPS 2021.

pub mod window;

#[cfg(feature = "linear_attention")]
pub mod linear;

#[cfg(feature = "spike_attention")]
pub mod spike_driven;

pub use window::{SlidingWindowAttention, WindowAttentionConfig};

#[cfg(feature = "spike_attention")]
pub use spike_driven::{SpikeDrivenAttention, SpikeDrivenConfig, SpikeTrain};

#[cfg(feature = "sparse_attention")]
pub use window::{apply_mincut_sparse_mask, sparse_attention_with_mincut_mask};
