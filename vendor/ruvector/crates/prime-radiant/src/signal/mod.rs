//! # Signal Ingestion Module
//!
//! Validates and normalizes incoming events before they enter the coherence engine.
//!
//! ## Responsibilities
//!
//! - Validate incoming signals against schema
//! - Normalize to canonical form
//! - Route to appropriate processing pipeline
//! - Emit domain events for ingested signals

// TODO: Implement signal validation and normalization
// This is a placeholder for the signal ingestion bounded context

use serde::{Deserialize, Serialize};

/// A raw signal before validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSignal {
    /// Signal identifier.
    pub id: String,
    /// Signal type.
    pub signal_type: String,
    /// Raw payload.
    pub payload: serde_json::Value,
    /// Timestamp (Unix millis).
    pub timestamp_ms: u64,
    /// Source identifier.
    pub source: String,
}

/// A validated and normalized signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedSignal {
    /// Signal identifier.
    pub id: String,
    /// Signal type.
    pub signal_type: SignalType,
    /// Normalized payload.
    pub payload: NormalizedPayload,
    /// Timestamp (Unix millis).
    pub timestamp_ms: u64,
    /// Source identifier.
    pub source: String,
    /// Validation metadata.
    pub validation: ValidationMetadata,
}

/// Signal type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignalType {
    /// State update for a node.
    StateUpdate,
    /// Edge addition.
    EdgeAdd,
    /// Edge removal.
    EdgeRemove,
    /// Observation for evidence accumulation.
    Observation,
    /// Policy update.
    PolicyUpdate,
    /// Query request.
    Query,
}

/// Normalized payload for processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NormalizedPayload {
    /// State update payload.
    StateUpdate { node_id: String, state: Vec<f32> },
    /// Edge modification payload.
    EdgeMod {
        source: String,
        target: String,
        weight: Option<f32>,
    },
    /// Observation payload.
    Observation {
        hypothesis_id: String,
        observed: bool,
    },
    /// Generic JSON payload.
    Json(serde_json::Value),
}

/// Metadata from signal validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationMetadata {
    /// Whether the signal passed validation.
    pub valid: bool,
    /// Validation warnings.
    pub warnings: Vec<String>,
    /// Schema version used.
    pub schema_version: String,
    /// Normalization applied.
    pub normalizations: Vec<String>,
}

impl Default for ValidationMetadata {
    fn default() -> Self {
        Self {
            valid: true,
            warnings: Vec::new(),
            schema_version: "1.0.0".to_string(),
            normalizations: Vec::new(),
        }
    }
}
