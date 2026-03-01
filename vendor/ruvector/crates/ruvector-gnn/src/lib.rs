//! # RuVector GNN
//!
//! Graph Neural Network capabilities for RuVector, providing tensor operations,
//! GNN layers, compression, and differentiable search.
//!
//! ## Forgetting Mitigation (Issue #17)
//!
//! This crate includes comprehensive forgetting mitigation for continual learning:
//!
//! - **Adam Optimizer**: Full implementation with momentum and bias correction
//! - **Replay Buffer**: Experience replay with reservoir sampling for uniform coverage
//! - **EWC (Elastic Weight Consolidation)**: Prevents catastrophic forgetting
//! - **Learning Rate Scheduling**: Multiple strategies including warmup and plateau detection
//!
//! ### Usage Example
//!
//! ```rust,ignore
//! use ruvector_gnn::{
//!     training::{Optimizer, OptimizerType},
//!     replay::ReplayBuffer,
//!     ewc::ElasticWeightConsolidation,
//!     scheduler::{LearningRateScheduler, SchedulerType},
//! };
//!
//! // Create Adam optimizer
//! let mut optimizer = Optimizer::new(OptimizerType::Adam {
//!     learning_rate: 0.001,
//!     beta1: 0.9,
//!     beta2: 0.999,
//!     epsilon: 1e-8,
//! });
//!
//! // Create replay buffer for experience replay
//! let mut replay = ReplayBuffer::new(10000);
//!
//! // Create EWC for preventing forgetting
//! let mut ewc = ElasticWeightConsolidation::new(0.4);
//!
//! // Create learning rate scheduler
//! let mut scheduler = LearningRateScheduler::new(
//!     SchedulerType::CosineAnnealing { t_max: 100, eta_min: 1e-6 },
//!     0.001
//! );
//! ```

#![warn(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod compress;
pub mod error;
pub mod ewc;
pub mod layer;
pub mod query;
pub mod replay;
pub mod scheduler;
pub mod search;
pub mod tensor;
pub mod training;

#[cfg(all(not(target_arch = "wasm32"), feature = "mmap"))]
pub mod mmap;

#[cfg(all(feature = "cold-tier", not(target_arch = "wasm32")))]
pub mod cold_tier;

// Re-export commonly used types
pub use compress::{CompressedTensor, CompressionLevel, TensorCompress};
pub use error::{GnnError, Result};
pub use ewc::ElasticWeightConsolidation;
pub use layer::RuvectorLayer;
pub use query::{QueryMode, QueryResult, RuvectorQuery, SubGraph};
pub use replay::{DistributionStats, ReplayBuffer, ReplayEntry};
pub use scheduler::{LearningRateScheduler, SchedulerType};
pub use search::{cosine_similarity, differentiable_search, hierarchical_forward};
pub use training::{
    info_nce_loss, local_contrastive_loss, sgd_step, Loss, LossType, OnlineConfig, Optimizer,
    OptimizerType, TrainConfig,
};

#[cfg(all(not(target_arch = "wasm32"), feature = "mmap"))]
pub use mmap::{AtomicBitmap, MmapGradientAccumulator, MmapManager};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        // Basic smoke test to ensure the crate compiles
        assert!(true);
    }
}
