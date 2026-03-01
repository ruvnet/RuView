//! Configuration types for RuvLLM integration.

use serde::{Deserialize, Serialize};

/// Configuration for LLM coherence gating.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCoherenceConfig {
    /// Coherence energy threshold for allowing responses (0.0-1.0)
    pub coherence_threshold: f64,

    /// Hallucination detection sensitivity (0.0-1.0)
    pub hallucination_sensitivity: f64,

    /// Maximum response length before escalation
    pub max_response_length: usize,

    /// Gating mode
    pub gating_mode: GatingMode,

    /// Response policy
    pub response_policy: ResponsePolicy,

    /// Coherence thresholds for different lanes
    pub lane_thresholds: CoherenceThresholds,

    /// Hallucination handling policy
    pub hallucination_policy: HallucinationPolicy,

    /// Enable semantic consistency checking
    pub semantic_consistency: bool,

    /// Enable citation verification
    pub citation_verification: bool,

    /// Enable factual grounding
    pub factual_grounding: bool,
}

impl Default for LlmCoherenceConfig {
    fn default() -> Self {
        Self {
            coherence_threshold: super::DEFAULT_COHERENCE_THRESHOLD,
            hallucination_sensitivity: super::DEFAULT_HALLUCINATION_SENSITIVITY,
            max_response_length: super::DEFAULT_MAX_RESPONSE_LENGTH,
            gating_mode: GatingMode::default(),
            response_policy: ResponsePolicy::default(),
            lane_thresholds: CoherenceThresholds::default(),
            hallucination_policy: HallucinationPolicy::default(),
            semantic_consistency: true,
            citation_verification: false,
            factual_grounding: true,
        }
    }
}

/// Gating mode for LLM responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GatingMode {
    /// Allow all responses (logging only)
    Permissive,

    /// Standard gating with thresholds
    #[default]
    Standard,

    /// Strict gating - any coherence violation blocks
    Strict,

    /// Adaptive gating based on context
    Adaptive,
}

/// Policy for handling LLM responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ResponsePolicy {
    /// Allow response if coherent
    #[default]
    AllowIfCoherent,

    /// Always require human review
    RequireReview,

    /// Escalate on any uncertainty
    EscalateOnUncertain,

    /// Block unless explicitly verified
    BlockUnlessVerified,
}

/// Coherence thresholds for different compute lanes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceThresholds {
    /// Threshold for reflex lane (lowest latency)
    pub reflex: f64,

    /// Threshold for retrieval lane
    pub retrieval: f64,

    /// Threshold for heavy computation lane
    pub heavy: f64,

    /// Threshold for human escalation
    pub human: f64,
}

impl Default for CoherenceThresholds {
    fn default() -> Self {
        Self {
            reflex: 0.9,
            retrieval: 0.7,
            heavy: 0.5,
            human: 0.3,
        }
    }
}

/// Policy for handling potential hallucinations.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HallucinationPolicy {
    /// Action when hallucination is detected
    pub action: HallucinationAction,

    /// Minimum confidence to trigger action
    pub confidence_threshold: f64,

    /// Whether to log all potential hallucinations
    pub log_all: bool,

    /// Maximum allowed hallucination rate before escalation
    pub max_rate: f64,
}

/// Action to take when hallucination is detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HallucinationAction {
    /// Log and allow
    LogOnly,

    /// Block the response
    #[default]
    Block,

    /// Escalate to human review
    Escalate,

    /// Retry with different prompt
    Retry,
}

impl LlmCoherenceConfig {
    /// Create a permissive configuration (logging only).
    pub fn permissive() -> Self {
        Self {
            gating_mode: GatingMode::Permissive,
            coherence_threshold: 0.0,
            hallucination_policy: HallucinationPolicy {
                action: HallucinationAction::LogOnly,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Create a strict configuration (blocks on any violation).
    pub fn strict() -> Self {
        Self {
            gating_mode: GatingMode::Strict,
            coherence_threshold: 0.95,
            hallucination_sensitivity: 0.9,
            response_policy: ResponsePolicy::BlockUnlessVerified,
            hallucination_policy: HallucinationPolicy {
                action: HallucinationAction::Block,
                confidence_threshold: 0.5,
                log_all: true,
                max_rate: 0.01,
            },
            semantic_consistency: true,
            citation_verification: true,
            factual_grounding: true,
            ..Default::default()
        }
    }

    /// Create an adaptive configuration.
    pub fn adaptive() -> Self {
        Self {
            gating_mode: GatingMode::Adaptive,
            response_policy: ResponsePolicy::EscalateOnUncertain,
            ..Default::default()
        }
    }
}
