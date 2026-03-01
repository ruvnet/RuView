//! # Training Module
//!
//! This module provides training data generation and fine-tuning utilities
//! for RuvLTRA models, including Claude Flow task datasets and MCP tool training.
//!
//! ## Submodules
//!
//! - [`claude_dataset`]: Task routing dataset generation
//! - [`grpo`]: GRPO (Group Relative Policy Optimization) for RL
//! - [`tool_dataset`]: MCP tool calling dataset generation (140+ tools)
//! - [`mcp_tools`]: MCP tool trainer with GRPO-based fine-tuning
//!
//! ## Example: Tool Use Fine-Tuning
//!
//! ```rust,ignore
//! use ruvllm::training::{McpToolTrainer, McpTrainingConfig, ToolDatasetConfig};
//!
//! // Create trainer
//! let config = McpTrainingConfig::default();
//! let mut trainer = McpToolTrainer::new(config)?;
//! trainer.load_tool_definitions()?;
//!
//! // Generate training data
//! let dataset = trainer.generate_tool_dataset(ToolDatasetConfig::comprehensive())?;
//! println!("Generated {} examples", dataset.len());
//!
//! // Evaluate baseline
//! let metrics = trainer.evaluate_tool_accuracy(&dataset.examples)?;
//! println!("Baseline accuracy: {:.2}%", metrics.tool_accuracy * 100.0);
//! ```

pub mod claude_dataset;
pub mod contrastive;
pub mod grpo;
pub mod mcp_tools;
pub mod real_trainer;
pub mod tool_dataset;

#[cfg(test)]
mod tests;

// Claude dataset exports
pub use claude_dataset::{
    AugmentationConfig, ClaudeTaskDataset, ClaudeTaskExample, ComplexityLevel, DatasetConfig,
    DatasetGenerator, DatasetStats, DomainType, TaskCategory, TaskMetadata,
};

// GRPO optimizer exports
pub use grpo::{
    GrpoBatch, GrpoConfig, GrpoOptimizer, GrpoSample, GrpoStats, GrpoUpdateResult, SampleGroup,
};

// MCP tool training exports
pub use mcp_tools::{
    EvaluationMetrics, McpToolTrainer, McpTrainingConfig, StepBuilder, ToolTrajectory,
    TrainingCheckpoint, TrainingResult, TrainingStats, TrajectoryBuilder, TrajectoryMetadata,
    TrajectoryStep,
};

// Tool dataset exports
pub use tool_dataset::{
    DifficultyLevel, DifficultyWeights, McpToolDef, ParamType, ToolCallDataset, ToolCallExample,
    ToolCategory as McpToolCategory, ToolDatasetConfig, ToolDatasetStats, ToolParam,
};

// Contrastive learning exports
pub use contrastive::{
    AgentEmbedding, ContrastiveConfig, ContrastiveTrainer, TrainingResult as ContrastiveResult,
    TrainingStats as ContrastiveStats, TrainingTriplet, AGENT_DESCRIPTIONS,
};

// Real trainer exports (Candle-based with GGUF export)
pub use real_trainer::{
    run_training_pipeline, EpochStats, GgufExportMetadata, GgufExportResult, GrpoEvaluator,
    GrpoFeedback, LayerMetadata, RealContrastiveTrainer, RealTrainingConfig, RealTrainingResult,
    TrainingConfigMeta,
};
