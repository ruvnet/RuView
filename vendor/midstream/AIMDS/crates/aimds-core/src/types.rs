//! Core type definitions for AIMDS

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Severity level for detected threats
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ThreatSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Detection result from pattern matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub severity: ThreatSeverity,
    pub threat_type: ThreatType,
    pub confidence: f64,
    pub input_hash: String,
    pub matched_patterns: Vec<String>,
    pub context: serde_json::Value,
}

/// Types of threats that can be detected
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreatType {
    PromptInjection,
    JailbreakAttempt,
    DataExfiltration,
    ModelManipulation,
    PolicyViolation,
    BehavioralAnomaly,
    Unknown,
}

/// Analysis result from behavioral analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub detection_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub is_threat: bool,
    pub threat_score: f64,
    pub policy_violations: Vec<PolicyViolation>,
    pub behavioral_anomalies: Vec<BehavioralAnomaly>,
    pub ltl_verification: Option<LtlVerification>,
    pub recommended_action: RecommendedAction,
}

/// Policy violation details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyViolation {
    pub policy_id: String,
    pub violation_type: String,
    pub severity: ThreatSeverity,
    pub description: String,
}

/// Behavioral anomaly detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralAnomaly {
    pub anomaly_type: String,
    pub deviation_score: f64,
    pub baseline_comparison: String,
    pub temporal_pattern: Vec<f64>,
}

/// Linear Temporal Logic verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LtlVerification {
    pub formula: String,
    pub is_satisfied: bool,
    pub counterexample: Option<String>,
    pub proof_trace: Vec<String>,
}

/// Recommended action based on analysis
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecommendedAction {
    Allow,
    Block,
    Sanitize,
    RateLimit,
    RequireHumanReview,
    Quarantine,
}

/// Response strategy from meta-learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseStrategy {
    pub analysis_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub action: RecommendedAction,
    pub mitigation_steps: Vec<MitigationStep>,
    pub confidence: f64,
    pub learning_context: serde_json::Value,
}

/// Individual mitigation step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MitigationStep {
    pub step_type: MitigationType,
    pub priority: u8,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Types of mitigation strategies
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MitigationType {
    InputSanitization,
    OutputFiltering,
    RateLimiting,
    SessionTermination,
    ModelIsolation,
    AlertGeneration,
    AdaptiveLearning,
}

/// Prompt input structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptInput {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub content: String,
    pub context: serde_json::Value,
    pub session_id: Option<String>,
    pub user_id: Option<String>,
}

impl PromptInput {
    pub fn new(content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            content,
            context: serde_json::json!({}),
            session_id: None,
            user_id: None,
        }
    }

    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = context;
        self
    }

    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }
}

/// Sanitized output structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizedOutput {
    pub original_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub sanitized_content: String,
    pub modifications: Vec<String>,
    pub is_safe: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_input_creation() {
        let input = PromptInput::new("Test prompt".to_string())
            .with_session("session-123".to_string());

        assert_eq!(input.content, "Test prompt");
        assert_eq!(input.session_id, Some("session-123".to_string()));
    }

    #[test]
    fn test_threat_severity_ordering() {
        assert!(ThreatSeverity::Critical > ThreatSeverity::High);
        assert!(ThreatSeverity::High > ThreatSeverity::Medium);
        assert!(ThreatSeverity::Medium > ThreatSeverity::Low);
    }
}
