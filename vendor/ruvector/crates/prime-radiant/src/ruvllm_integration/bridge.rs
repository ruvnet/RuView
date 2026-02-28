//! Bridge modules for synchronizing policies and learning between systems.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

use super::error::{Result, RuvLlmIntegrationError};

// ============================================================================
// POLICY BRIDGE
// ============================================================================

/// Bridge for synchronizing policies between Prime-Radiant and RuvLLM.
#[derive(Debug)]
pub struct PolicyBridge {
    /// Configuration
    config: PolicyBridgeConfig,

    /// Sync statistics
    syncs: AtomicU64,
    sync_failures: AtomicU64,
}

/// Configuration for the policy bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyBridgeConfig {
    /// Enable automatic synchronization
    pub auto_sync: bool,

    /// Sync interval in seconds
    pub sync_interval_secs: u64,

    /// Maximum policies to sync per batch
    pub batch_size: usize,

    /// Enable bidirectional sync
    pub bidirectional: bool,

    /// Conflict resolution strategy
    pub conflict_resolution: ConflictResolution,
}

impl Default for PolicyBridgeConfig {
    fn default() -> Self {
        Self {
            auto_sync: true,
            sync_interval_secs: 60,
            batch_size: 100,
            bidirectional: true,
            conflict_resolution: ConflictResolution::PreferNewest,
        }
    }
}

/// Conflict resolution strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ConflictResolution {
    /// Prefer the newest policy
    #[default]
    PreferNewest,
    /// Prefer Prime-Radiant policies
    PreferPrimeRadiant,
    /// Prefer RuvLLM policies
    PreferRuvLlm,
    /// Merge policies
    Merge,
}

/// Result of a policy synchronization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySyncResult {
    /// Number of policies synced to RuvLLM
    pub to_ruvllm: usize,
    /// Number of policies synced to Prime-Radiant
    pub to_prime_radiant: usize,
    /// Number of conflicts resolved
    pub conflicts_resolved: usize,
    /// Sync duration in milliseconds
    pub duration_ms: u64,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl PolicyBridge {
    /// Create a new policy bridge.
    pub fn new(config: PolicyBridgeConfig) -> Result<Self> {
        Ok(Self {
            config,
            syncs: AtomicU64::new(0),
            sync_failures: AtomicU64::new(0),
        })
    }

    /// Synchronize policies between systems.
    pub fn sync_policies(&self) -> Result<PolicySyncResult> {
        let start = std::time::Instant::now();

        // In a real implementation, this would:
        // 1. Fetch policies from Prime-Radiant governance
        // 2. Fetch policies from RuvLLM policy store
        // 3. Resolve conflicts using the configured strategy
        // 4. Update both systems

        self.syncs.fetch_add(1, Ordering::Relaxed);

        Ok(PolicySyncResult {
            to_ruvllm: 0,
            to_prime_radiant: 0,
            conflicts_resolved: 0,
            duration_ms: start.elapsed().as_millis() as u64,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Get the configuration.
    pub fn config(&self) -> &PolicyBridgeConfig {
        &self.config
    }

    /// Get sync statistics.
    pub fn stats(&self) -> (u64, u64) {
        (
            self.syncs.load(Ordering::Relaxed),
            self.sync_failures.load(Ordering::Relaxed),
        )
    }
}

// ============================================================================
// SONA BRIDGE
// ============================================================================

/// Bridge for connecting SONA learning loops between Prime-Radiant and RuvLLM.
#[derive(Debug)]
pub struct SonaBridge {
    /// Configuration
    config: SonaBridgeConfig,

    /// Feedback processed
    feedback_processed: AtomicU64,
}

/// Configuration for the SONA bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonaBridgeConfig {
    /// Enable learning feedback loop
    pub enable_feedback: bool,

    /// Feedback batch size
    pub batch_size: usize,

    /// Learning rate multiplier
    pub learning_rate_multiplier: f64,

    /// Enable EWC (Elastic Weight Consolidation) synchronization
    pub sync_ewc: bool,

    /// Enable micro-LoRA weight sharing
    pub share_lora_weights: bool,
}

impl Default for SonaBridgeConfig {
    fn default() -> Self {
        Self {
            enable_feedback: true,
            batch_size: 32,
            learning_rate_multiplier: 1.0,
            sync_ewc: true,
            share_lora_weights: false,
        }
    }
}

/// Learning feedback from one system to the other.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningFeedback {
    /// Source system
    pub source: FeedbackSource,

    /// Feedback type
    pub feedback_type: FeedbackType,

    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Session ID (if applicable)
    pub session_id: Option<String>,

    /// Success indicator
    pub success: bool,

    /// Coherence energy (from Prime-Radiant)
    pub coherence_energy: Option<f64>,

    /// Quality score (from RuvLLM)
    pub quality_score: Option<f64>,

    /// Additional context
    pub context: serde_json::Value,
}

/// Source of the learning feedback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedbackSource {
    /// From Prime-Radiant coherence engine
    PrimeRadiant,
    /// From RuvLLM inference engine
    RuvLlm,
    /// From human reviewer
    Human,
}

/// Type of learning feedback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedbackType {
    /// Coherence check result
    CoherenceResult,
    /// Inference quality feedback
    QualityFeedback,
    /// Gate decision feedback
    GateDecision,
    /// Human correction
    HumanCorrection,
    /// Threshold adjustment
    ThresholdAdjustment,
}

impl SonaBridge {
    /// Create a new SONA bridge.
    pub fn new(config: SonaBridgeConfig) -> Result<Self> {
        Ok(Self {
            config,
            feedback_processed: AtomicU64::new(0),
        })
    }

    /// Process learning feedback.
    pub fn process_feedback(&self, feedback: LearningFeedback) -> Result<()> {
        if !self.config.enable_feedback {
            return Ok(());
        }

        // Validate feedback
        self.validate_feedback(&feedback)?;

        // In a real implementation, this would:
        // 1. Transform the feedback into SONA-compatible format
        // 2. Apply learning rate multiplier
        // 3. Update both systems' learning loops
        // 4. Synchronize EWC importance weights if enabled

        self.feedback_processed.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Validate learning feedback.
    fn validate_feedback(&self, feedback: &LearningFeedback) -> Result<()> {
        // At least one metric should be present
        if feedback.coherence_energy.is_none() && feedback.quality_score.is_none() {
            return Err(RuvLlmIntegrationError::Config(
                "Feedback must contain either coherence_energy or quality_score".to_string(),
            ));
        }

        Ok(())
    }

    /// Get the configuration.
    pub fn config(&self) -> &SonaBridgeConfig {
        &self.config
    }

    /// Get feedback statistics.
    pub fn feedback_count(&self) -> u64 {
        self.feedback_processed.load(Ordering::Relaxed)
    }
}

impl LearningFeedback {
    /// Create coherence feedback from Prime-Radiant.
    pub fn coherence(energy: f64, success: bool) -> Self {
        Self {
            source: FeedbackSource::PrimeRadiant,
            feedback_type: FeedbackType::CoherenceResult,
            timestamp: chrono::Utc::now(),
            session_id: None,
            success,
            coherence_energy: Some(energy),
            quality_score: None,
            context: serde_json::Value::Null,
        }
    }

    /// Create quality feedback from RuvLLM.
    pub fn quality(score: f64, success: bool) -> Self {
        Self {
            source: FeedbackSource::RuvLlm,
            feedback_type: FeedbackType::QualityFeedback,
            timestamp: chrono::Utc::now(),
            session_id: None,
            success,
            coherence_energy: None,
            quality_score: Some(score),
            context: serde_json::Value::Null,
        }
    }

    /// Create human correction feedback.
    pub fn human_correction(success: bool, context: serde_json::Value) -> Self {
        Self {
            source: FeedbackSource::Human,
            feedback_type: FeedbackType::HumanCorrection,
            timestamp: chrono::Utc::now(),
            session_id: None,
            success,
            coherence_energy: None,
            quality_score: None,
            context,
        }
    }

    /// Set the session ID.
    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }
}
