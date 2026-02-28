//! # OSpipe
//!
//! RuVector-enhanced personal AI memory system integrating with Screenpipe.
//!
//! OSpipe captures screen content, audio transcriptions, and UI events,
//! processes them through a safety-aware ingestion pipeline, and stores
//! them as searchable vector embeddings for personal AI memory recall.
//!
//! ## Architecture
//!
//! ```text
//! Screenpipe -> Capture -> Safety Gate -> Dedup -> Embed -> VectorStore
//!                                                             |
//!                                      Search Router <--------+
//!                                      (Semantic / Keyword / Hybrid)
//! ```
//!
//! ## Modules
//!
//! - [`capture`] - Captured frame data structures (OCR, transcription, UI events)
//! - [`storage`] - HNSW-backed vector storage and embedding engine
//! - [`search`] - Query routing and hybrid search (semantic + keyword)
//! - [`pipeline`] - Ingestion pipeline with deduplication
//! - [`safety`] - PII detection and content redaction
//! - [`config`] - Configuration for all subsystems
//! - [`error`] - Unified error types

pub mod capture;
pub mod config;
pub mod error;
pub mod graph;
pub mod learning;
#[cfg(not(target_arch = "wasm32"))]
pub mod persistence;
pub mod pipeline;
pub mod quantum;
pub mod safety;
pub mod search;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
pub mod storage;

pub mod wasm;
