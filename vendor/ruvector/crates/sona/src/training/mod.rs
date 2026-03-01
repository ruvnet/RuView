//! SONA Training System
//!
//! Templated training pipelines for specialized model adaptation.
//!
//! ## Overview
//!
//! The training module provides:
//! - **Training Templates**: Pre-configured training setups for common use cases
//! - **Agent Factory**: Create and manage multiple specialized agents
//! - **Training Pipelines**: Structured workflows for different verticals
//! - **Federated Learning**: Distributed training across ephemeral agents
//! - **Metrics & Results**: Comprehensive training analytics
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ruvector_sona::training::{TrainingTemplate, AgentFactory, TrainingPipeline};
//!
//! // Use a preset template
//! let template = TrainingTemplate::code_agent();
//! let pipeline = TrainingPipeline::from_template(template);
//!
//! // Train on examples
//! for example in examples {
//!     pipeline.add_example(example);
//! }
//! let results = pipeline.train()?;
//! ```
//!
//! ## Federated Learning
//!
//! ```rust,ignore
//! use ruvector_sona::training::{EphemeralAgent, FederatedCoordinator};
//!
//! // Create coordinator
//! let mut coordinator = FederatedCoordinator::default_coordinator("main", 3072);
//!
//! // Ephemeral agents process tasks
//! let mut agent = EphemeralAgent::default_federated("agent-1", 3072);
//! agent.process_trajectory(embedding, activations, quality, route, context);
//!
//! // Export state before termination
//! let export = agent.export_state();
//! coordinator.aggregate(export);
//! ```

mod factory;
mod federated;
mod metrics;
mod pipeline;
mod templates;

pub use factory::{
    AgentFactory, AgentHandle, AgentStats, ManagedAgent, SharedAgentFactory, SimpleExample,
    TrainingExample as FactoryTrainingExample,
};
pub use federated::{
    AgentContribution, AgentExport, AgentExportStats, AggregationResult, CoordinatorStats,
    EphemeralAgent, FederatedCoordinator, FederatedTopology, TrajectoryExport,
};
pub use metrics::{
    EpochStats, PerformanceMetrics, QualityMetrics, TrainingMetrics, TrainingResult,
};
pub use pipeline::{
    BatchConfig, PipelineStage, TrainingCallback, TrainingExample, TrainingPipeline,
};
pub use templates::{
    AgentType, DataSizeHint, TaskDomain, TemplatePreset, TrainingMethod, TrainingTemplate,
    VerticalConfig,
};
