//! Witness adapter for unifying RuvLLM and Prime-Radiant audit trails.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

use super::error::{Result, RuvLlmIntegrationError};

/// Adapter for bridging witness logs between RuvLLM and Prime-Radiant.
#[derive(Debug)]
pub struct WitnessAdapter {
    /// Configuration
    config: WitnessAdapterConfig,

    /// Statistics
    entries_recorded: AtomicU64,
    correlations_created: AtomicU64,
}

/// Configuration for the witness adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessAdapterConfig {
    /// Storage path for unified witness log
    pub storage_path: String,

    /// Correlation window in seconds
    pub correlation_window_secs: u64,

    /// Enable cross-system correlation
    pub enable_correlation: bool,

    /// Maximum entries to retain
    pub max_entries: usize,

    /// Embedding dimension for semantic search
    pub embedding_dim: usize,
}

impl Default for WitnessAdapterConfig {
    fn default() -> Self {
        Self {
            storage_path: ".prime-radiant/witness".to_string(),
            correlation_window_secs: super::DEFAULT_CORRELATION_WINDOW_SECS,
            enable_correlation: true,
            max_entries: 100_000,
            embedding_dim: 768,
        }
    }
}

/// Unified witness entry combining RuvLLM and Prime-Radiant records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedWitnessEntry {
    /// Unique entry ID
    pub id: Uuid,

    /// Correlation ID linking related entries
    pub correlation_id: Option<CorrelationId>,

    /// Source system
    pub source: WitnessSource,

    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Entry type
    pub entry_type: WitnessEntryType,

    /// Session ID (if applicable)
    pub session_id: Option<String>,

    /// Request type or operation
    pub operation: String,

    /// Latency breakdown
    pub latency: LatencyBreakdown,

    /// Coherence metrics (from Prime-Radiant)
    pub coherence: Option<CoherenceMetrics>,

    /// LLM metrics (from RuvLLM)
    pub llm: Option<LlmMetrics>,

    /// Embedding for semantic search
    pub embedding: Option<Vec<f32>>,

    /// Additional metadata
    pub metadata: serde_json::Value,
}

/// Source of the witness entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WitnessSource {
    /// From Prime-Radiant coherence engine
    PrimeRadiant,
    /// From RuvLLM inference engine
    RuvLlm,
    /// From both systems (correlated)
    Unified,
}

/// Type of witness entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WitnessEntryType {
    /// Inference request
    Inference,
    /// Coherence check
    CoherenceCheck,
    /// Gate decision
    GateDecision,
    /// Policy evaluation
    PolicyEvaluation,
    /// Human escalation
    Escalation,
    /// System event
    SystemEvent,
}

/// Latency breakdown.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyBreakdown {
    /// Prefill latency (ms)
    pub prefill_ms: f64,
    /// Decode latency (ms)
    pub decode_ms: f64,
    /// Coherence check latency (ms)
    pub coherence_ms: f64,
    /// Gate evaluation latency (ms)
    pub gate_ms: f64,
    /// Total latency (ms)
    pub total_ms: f64,
}

/// Coherence metrics from Prime-Radiant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceMetrics {
    /// Global coherence energy
    pub energy: f64,
    /// Maximum residual
    pub max_residual: f64,
    /// Number of affected nodes
    pub affected_nodes: usize,
    /// Assigned compute lane
    pub lane: String,
    /// Gate decision
    pub allowed: bool,
}

/// LLM metrics from RuvLLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMetrics {
    /// Model used
    pub model: String,
    /// Tokens generated
    pub tokens_generated: usize,
    /// Tokens per second
    pub tokens_per_second: f64,
    /// Adapter used (if any)
    pub adapter: Option<String>,
    /// Quantization level
    pub quantization: Option<String>,
}

/// Correlation ID for linking related witness entries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(pub Uuid);

impl CorrelationId {
    /// Create a new correlation ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

/// Witness correlation between systems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessCorrelation {
    /// Correlation ID
    pub id: CorrelationId,

    /// Entries in this correlation
    pub entries: Vec<Uuid>,

    /// Start timestamp
    pub start_time: chrono::DateTime<chrono::Utc>,

    /// End timestamp
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,

    /// Session ID
    pub session_id: Option<String>,

    /// Summary metrics
    pub summary: CorrelationSummary,
}

/// Summary metrics for a correlation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CorrelationSummary {
    /// Total entries
    pub total_entries: usize,
    /// Entries from Prime-Radiant
    pub prime_radiant_entries: usize,
    /// Entries from RuvLLM
    pub ruvllm_entries: usize,
    /// Total latency
    pub total_latency_ms: f64,
    /// Average coherence energy
    pub avg_energy: f64,
    /// Gate pass rate
    pub pass_rate: f64,
}

impl WitnessAdapter {
    /// Create a new witness adapter.
    pub fn new(config: WitnessAdapterConfig) -> Result<Self> {
        Ok(Self {
            config,
            entries_recorded: AtomicU64::new(0),
            correlations_created: AtomicU64::new(0),
        })
    }

    /// Record a unified witness entry.
    pub fn record(&self, entry: UnifiedWitnessEntry) -> Result<()> {
        // Validate entry
        self.validate_entry(&entry)?;

        // Record the entry (in a real implementation, this would persist to storage)
        self.entries_recorded.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Create a new correlation.
    pub fn create_correlation(&self, session_id: Option<String>) -> Result<WitnessCorrelation> {
        self.correlations_created.fetch_add(1, Ordering::Relaxed);

        Ok(WitnessCorrelation {
            id: CorrelationId::new(),
            entries: Vec::new(),
            start_time: chrono::Utc::now(),
            end_time: None,
            session_id,
            summary: CorrelationSummary::default(),
        })
    }

    /// Add an entry to a correlation.
    pub fn add_to_correlation(
        &self,
        correlation: &mut WitnessCorrelation,
        entry_id: Uuid,
    ) -> Result<()> {
        correlation.entries.push(entry_id);
        correlation.summary.total_entries += 1;
        Ok(())
    }

    /// Get adapter statistics.
    pub fn stats(&self) -> (u64, u64) {
        (
            self.entries_recorded.load(Ordering::Relaxed),
            self.correlations_created.load(Ordering::Relaxed),
        )
    }

    /// Validate a witness entry.
    fn validate_entry(&self, entry: &UnifiedWitnessEntry) -> Result<()> {
        if entry.operation.is_empty() {
            return Err(RuvLlmIntegrationError::Config(
                "Operation cannot be empty".to_string(),
            ));
        }

        if let Some(ref embedding) = entry.embedding {
            if embedding.len() != self.config.embedding_dim {
                return Err(RuvLlmIntegrationError::EmbeddingDimensionMismatch {
                    expected: self.config.embedding_dim,
                    actual: embedding.len(),
                });
            }
        }

        Ok(())
    }

    /// Get the configuration.
    pub fn config(&self) -> &WitnessAdapterConfig {
        &self.config
    }
}

impl UnifiedWitnessEntry {
    /// Create a new unified witness entry from Prime-Radiant.
    pub fn from_prime_radiant(
        operation: String,
        coherence: CoherenceMetrics,
        latency_ms: f64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            correlation_id: None,
            source: WitnessSource::PrimeRadiant,
            timestamp: chrono::Utc::now(),
            entry_type: WitnessEntryType::CoherenceCheck,
            session_id: None,
            operation,
            latency: LatencyBreakdown {
                coherence_ms: latency_ms,
                total_ms: latency_ms,
                ..Default::default()
            },
            coherence: Some(coherence),
            llm: None,
            embedding: None,
            metadata: serde_json::Value::Null,
        }
    }

    /// Create a new unified witness entry from RuvLLM.
    pub fn from_ruvllm(
        operation: String,
        llm: LlmMetrics,
        prefill_ms: f64,
        decode_ms: f64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            correlation_id: None,
            source: WitnessSource::RuvLlm,
            timestamp: chrono::Utc::now(),
            entry_type: WitnessEntryType::Inference,
            session_id: None,
            operation,
            latency: LatencyBreakdown {
                prefill_ms,
                decode_ms,
                total_ms: prefill_ms + decode_ms,
                ..Default::default()
            },
            coherence: None,
            llm: Some(llm),
            embedding: None,
            metadata: serde_json::Value::Null,
        }
    }

    /// Set the correlation ID.
    pub fn with_correlation(mut self, correlation_id: CorrelationId) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    /// Set the session ID.
    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Set the embedding.
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }
}
