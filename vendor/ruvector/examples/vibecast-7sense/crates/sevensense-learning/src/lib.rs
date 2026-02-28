//! # sevensense-learning
//!
//! Graph Neural Network (GNN) based learning and embedding refinement for 7sense.
//!
//! This crate provides:
//! - GNN models (GCN, GraphSAGE, GAT) for graph-based learning
//! - Embedding refinement through message passing
//! - Contrastive learning with InfoNCE and triplet loss
//! - Elastic Weight Consolidation (EWC) for continual learning
//! - Graph attention mechanisms for relationship modeling
//!
//! ## Architecture
//!
//! The crate follows Domain-Driven Design principles:
//! - `domain`: Core entities and repository traits
//! - `application`: Business logic and services
//! - `infrastructure`: GNN implementations and attention mechanisms
//!
//! ## Example
//!
//! ```rust,ignore
//! use sevensense_learning::{LearningService, LearningConfig, GnnModelType};
//!
//! let config = LearningConfig::default();
//! let service = LearningService::new(config);
//!
//! // Train on transition graph
//! let metrics = service.train_epoch(&graph).await?;
//!
//! // Refine embeddings
//! let refined = service.refine_embeddings(&embeddings).await?;
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod loss;
pub mod ewc;

// Re-exports for convenience
pub use domain::entities::{
    LearningSession, GnnModelType, TrainingStatus, TrainingMetrics,
    TransitionGraph, RefinedEmbedding, EmbeddingId, Timestamp,
    GraphNode, GraphEdge, HyperParameters, LearningConfig,
};
pub use domain::repository::LearningRepository;
pub use application::services::LearningService;
pub use infrastructure::gnn_model::{GnnModel, GnnLayer, Aggregator};
pub use infrastructure::attention::{AttentionLayer, MultiHeadAttention};
pub use loss::{info_nce_loss, triplet_loss, margin_ranking_loss, contrastive_loss};
pub use ewc::{EwcState, FisherInformation, EwcRegularizer};

/// Crate version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::domain::entities::*;
    pub use crate::domain::repository::*;
    pub use crate::application::services::*;
    pub use crate::infrastructure::gnn_model::*;
    pub use crate::infrastructure::attention::*;
    pub use crate::loss::*;
    pub use crate::ewc::*;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crate_exports() {
        // Verify all public types are accessible
        let _: GnnModelType = GnnModelType::Gcn;
        let _: TrainingStatus = TrainingStatus::Pending;
    }
}
