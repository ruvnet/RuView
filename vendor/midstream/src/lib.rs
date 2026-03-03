//! MidStream: Real-Time Large Language Model Streaming Platform
//! 
//! This library provides functionality for real-time LLM response streaming,
//! inflight data analysis, and integration with external tools.
//! 
//! # Example
//! 
//! ```rust,no_run
//! use midstream::{Midstream, HyprSettings, HyprServiceImpl, StreamProcessor, LLMClient};
//! use futures::stream::BoxStream;
//! use futures::stream::iter;
//! use std::time::Duration;
//! 
//! // Example LLM client implementation
//! struct ExampleLLMClient;
//! 
//! impl LLMClient for ExampleLLMClient {
//!     fn stream(&self) -> BoxStream<'static, String> {
//!         Box::pin(iter(vec![
//!             "Processing".to_string(),
//!             "the".to_string(),
//!             "stream".to_string(),
//!         ]))
//!     }
//! }
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize settings
//!     let settings = HyprSettings::new()?;
//!     
//!     // Create hyprstream service
//!     let hypr_service = HyprServiceImpl::new(&settings).await?;
//!     
//!     // Create LLM client
//!     let llm_client = ExampleLLMClient;
//!     
//!     // Initialize Midstream
//!     let midstream = Midstream::new(
//!         Box::new(llm_client),
//!         Box::new(hypr_service),
//!     );
//!     
//!     // Process stream
//!     let messages = midstream.process_stream().await?;
//!     println!("Processed messages: {:?}", messages);
//!     
//!     // Get metrics
//!     let metrics = midstream.get_metrics().await;
//!     println!("Collected metrics: {:?}", metrics);
//!     
//!     // Get average sentiment for last 5 minutes
//!     let avg = midstream.get_average_sentiment(Duration::from_secs(300)).await?;
//!     println!("Average sentiment: {}", avg);
//!     
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod midstream;
pub mod hypr_service;
pub mod tests;
pub mod lean_agentic;

pub use config::HyprSettings;
pub use midstream::{
    Midstream,
    StreamProcessor,
    LLMMessage,
    LLMClient,
    HyprService,
    ToolIntegration,
    Intent,
    MetricRecord,
    TimeWindow,
    AggregateFunction,
};
pub use hypr_service::HyprServiceImpl;

// Lean Agentic Learning System exports
pub use lean_agentic::{
    LeanAgenticSystem,
    LeanAgenticConfig,
    FormalReasoner,
    Theorem,
    Proof,
    ProofStep,
    AgenticLoop,
    Action,
    Observation,
    Plan,
    LearningSignal,
    KnowledgeGraph,
    Entity,
    Relation,
    StreamLearner,
    OnlineModel,
    AdaptationStrategy,
    AgentState,
    Context as AgentContext,
    Reward,
};
