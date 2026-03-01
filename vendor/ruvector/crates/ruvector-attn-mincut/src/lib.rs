//! # ruvector-attn-mincut
//!
//! Dynamic min-cut gating as an alternative to softmax attention.
//!
//! Instead of applying softmax uniformly over all Q*K^T logits, this crate
//! builds a weighted directed graph from the logits and computes a minimum
//! cut (via Dinic's max-flow algorithm) to gate irrelevant edges. Surviving
//! edges are then normalised with row-softmax and multiplied by V.
//!
//! ## Key features
//!
//! - **Graph construction** from attention logits (`graph` module).
//! - **Dinic's max-flow / min-cut** solver (`mincut` module).
//! - **Gating operators**: standard softmax and min-cut gated (`gating` module).
//! - **Temporal hysteresis** to stabilise gating over time (`hysteresis` module).
//! - **Witness logging** with SHA-256 hashing for determinism verification (`witness` module).
//! - **Configuration** with sane defaults (`config` module).

pub mod config;
pub mod gating;
pub mod graph;
pub mod hysteresis;
pub mod mincut;
pub mod witness;

// Re-export primary types for ergonomic usage.
pub use config::MinCutConfig;
pub use gating::{attn_mincut, attn_softmax, AttentionOutput};
pub use graph::{graph_from_logits, AttentionGraph, Edge};
pub use hysteresis::HysteresisTracker;
pub use mincut::{dynamic_min_cut, CutResult, DinicSolver, GatingResult};
pub use witness::{hash_tensor, witness_log, WitnessEntry};
