//! # sevensense-analysis
//!
//! Analysis bounded context for 7sense bioacoustic analysis platform.
//!
//! This crate provides clustering, motif detection, sequence analysis, and anomaly
//! detection capabilities for bioacoustic embeddings.
//!
//! ## Features
//!
//! - **Clustering**: HDBSCAN and K-means clustering for grouping similar vocalizations
//! - **Prototype Extraction**: Identify representative embeddings (exemplars) for each cluster
//! - **Motif Detection**: Discover recurring patterns in vocalization sequences
//! - **Sequence Analysis**: Markov chain analysis, transition matrices, entropy computation
//! - **Anomaly Detection**: Identify unusual or novel vocalizations
//!
//! ## Architecture
//!
//! This crate follows Domain-Driven Design (DDD) with hexagonal architecture:
//!
//! - `domain/` - Core domain entities, value objects, and repository traits
//! - `application/` - Application services orchestrating domain operations
//! - `infrastructure/` - Concrete implementations (HDBSCAN, Markov chains, etc.)
//!
//! ## Example
//!
//! ```rust,ignore
//! use sevensense_analysis::{
//!     application::ClusteringService,
//!     domain::{ClusteringConfig, ClusteringMethod},
//! };
//!
//! let service = ClusteringService::new(ClusteringConfig::default());
//! let embeddings = vec![/* ... */];
//! let clusters = service.run_hdbscan(&embeddings).await?;
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![allow(clippy::module_name_repetitions)]

pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod metrics;

// Re-export primary types for convenience
pub use domain::entities::{
    Anomaly, AnomalyType, Cluster, ClusterId, EmbeddingId, Motif, MotifOccurrence, Prototype,
    RecordingId, SegmentId, SequenceAnalysis,
};
pub use domain::repository::{ClusterRepository, MotifRepository, SequenceRepository};
pub use domain::events::{
    AnalysisEvent, ClusterAssigned, ClustersDiscovered, MotifDetected, SequenceAnalyzed,
};
pub use domain::value_objects::{
    ClusteringConfig, ClusteringMethod, ClusteringParameters, MotifConfig, SequenceMetrics,
    TransitionMatrix,
};

pub use application::services::{
    AnomalyDetectionService, ClusteringService, MotifDetectionService, SequenceAnalysisService,
};

pub use metrics::{
    ClusteringMetrics, SequenceEntropy, SilhouetteScore, VMeasure,
};

/// Crate version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::domain::entities::*;
    pub use crate::domain::repository::*;
    pub use crate::domain::value_objects::*;
    pub use crate::application::services::*;
    pub use crate::metrics::*;
}
