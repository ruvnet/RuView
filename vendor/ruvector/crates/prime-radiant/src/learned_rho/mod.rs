//! Learned Restriction Maps - GNN-based ρ Learning
//!
//! This module provides integration with `ruvector-gnn` for learning restriction maps
//! (ρ) from data. Instead of manually specifying how node states should be projected
//! for coherence checking, we learn these projections from known-coherent examples.
//!
//! # Architecture
//!
//! The learned restriction map uses:
//!
//! - **GNN layers**: Neural network layers for the projection function
//! - **EWC (Elastic Weight Consolidation)**: Prevents catastrophic forgetting
//! - **Replay buffer**: Experience replay for stable learning
//! - **LR scheduling**: Adaptive learning rates
//!
//! # Key Types
//!
//! - [`LearnedRestrictionMap`]: GNN-based restriction map
//! - [`RestrictionMapConfig`]: Configuration for learning
//! - [`TrainingBatch`]: Batch of training examples
//!
//! # Example
//!
//! ```rust,ignore
//! use prime_radiant::learned_rho::{LearnedRestrictionMap, RestrictionMapConfig};
//!
//! // Create learned restriction map
//! let mut rho = LearnedRestrictionMap::new(RestrictionMapConfig {
//!     input_dim: 128,
//!     output_dim: 64,
//!     ..Default::default()
//! });
//!
//! // Apply learned projection
//! let projected = rho.apply(&input_state);
//!
//! // Train on known-coherent examples
//! rho.train(&source, &target, &expected_residual);
//!
//! // Consolidate knowledge (compute Fisher information)
//! rho.consolidate();
//! ```

mod config;
mod error;
mod map;
mod training;

pub use config::{OptimizerConfig, RestrictionMapConfig, SchedulerConfig};
pub use error::{LearnedRhoError, LearnedRhoResult};
pub use map::{LearnedRestrictionMap, MapState};
pub use training::{TrainingBatch, TrainingMetrics, TrainingResult};
