//! Ingestion pipeline with deduplication.
//!
//! The pipeline receives captured frames, passes them through the safety
//! gate, checks for duplicates, generates embeddings, and stores the
//! results in the vector store.

pub mod dedup;
pub mod ingestion;

pub use dedup::FrameDeduplicator;
pub use ingestion::{IngestResult, IngestionPipeline, PipelineStats};
