//! Mitigation actions and execution framework

use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use crate::Result;
use crate::adaptive::{ChallengeType, AlertPriority};
use crate::meta_learning::ThreatIncident;

/// Mitigation actions that can be taken against threats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MitigationAction {
    /// Block the threatening request
    BlockRequest {
        reason: String,
    },

    /// Apply rate limiting to user/source
    RateLimitUser {
        duration: Duration,
    },

    /// Require additional verification
    RequireVerification {
        challenge_type: ChallengeType,
    },

    /// Alert human operator
    AlertHuman {
        priority: AlertPriority,
    },

    /// Update detection rules
    UpdateRules {
        new_patterns: Vec<Pattern>,
    },
}

impl MitigationAction {
    /// Execute mitigation action
    pub async fn execute(&self, context: &ThreatContext) -> Result<String> {
        match self {
            MitigationAction::BlockRequest { reason } => {
                self.execute_block(context, reason).await
            }
            MitigationAction::RateLimitUser { duration } => {
                self.execute_rate_limit(context, *duration).await
            }
            MitigationAction::RequireVerification { challenge_type } => {
                self.execute_verification(context, challenge_type).await
            }
            MitigationAction::AlertHuman { priority } => {
                self.execute_alert(context, priority).await
            }
            MitigationAction::UpdateRules { new_patterns } => {
                self.execute_rule_update(context, new_patterns).await
            }
        }
    }

    /// Rollback mitigation action
    pub fn rollback(&self, action_id: &str) -> Result<()> {
        // Implementation would coordinate with actual enforcement systems
        tracing::info!("Rolling back action: {}", action_id);
        Ok(())
    }

    /// Execute block request action
    async fn execute_block(&self, context: &ThreatContext, reason: &str) -> Result<String> {
        tracing::info!(
            "Blocking request from {} - Reason: {}",
            context.source_id,
            reason
        );

        // Record block action
        let action_id = uuid::Uuid::new_v4().to_string();

        // In production, this would integrate with firewall/WAF
        // For now, we simulate the action
        metrics::counter!("mitigation.blocks").increment(1);

        Ok(action_id)
    }

    /// Execute rate limit action
    async fn execute_rate_limit(&self, context: &ThreatContext, duration: Duration) -> Result<String> {
        tracing::info!(
            "Rate limiting {} for {:?}",
            context.source_id,
            duration
        );

        let action_id = uuid::Uuid::new_v4().to_string();

        // In production, integrate with rate limiter (Redis, etc.)
        metrics::counter!("mitigation.rate_limits").increment(1);

        Ok(action_id)
    }

    /// Execute verification requirement action
    async fn execute_verification(&self, context: &ThreatContext, challenge: &ChallengeType) -> Result<String> {
        tracing::info!(
            "Requiring {:?} verification for {}",
            challenge,
            context.source_id
        );

        let action_id = uuid::Uuid::new_v4().to_string();

        // In production, integrate with verification service
        metrics::counter!("mitigation.verifications").increment(1);

        Ok(action_id)
    }

    /// Execute human alert action
    async fn execute_alert(&self, context: &ThreatContext, priority: &AlertPriority) -> Result<String> {
        tracing::warn!(
            "Alerting security team - Priority: {:?} - Threat: {}",
            priority,
            context.threat_id
        );

        let action_id = uuid::Uuid::new_v4().to_string();

        // In production, integrate with alerting system (PagerDuty, etc.)
        metrics::counter!("mitigation.alerts").increment(1);

        Ok(action_id)
    }

    /// Execute rule update action
    async fn execute_rule_update(&self, _context: &ThreatContext, patterns: &[Pattern]) -> Result<String> {
        tracing::info!(
            "Updating rules with {} new patterns",
            patterns.len()
        );

        let action_id = uuid::Uuid::new_v4().to_string();

        // In production, update detection engine rules
        metrics::counter!("mitigation.rule_updates").increment(1);

        Ok(action_id)
    }
}

/// Trait for mitigation implementations
#[async_trait::async_trait]
pub trait Mitigation: Send + Sync {
    /// Execute the mitigation
    async fn execute(&self, context: &ThreatContext) -> Result<MitigationOutcome>;

    /// Rollback the mitigation
    fn rollback(&self) -> Result<()>;
}

/// Context for mitigation execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatContext {
    pub threat_id: String,
    pub source_id: String,
    pub threat_type: String,
    pub severity: u8,
    pub confidence: f64,
    pub metadata: HashMap<String, String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ThreatContext {
    /// Create context from threat incident
    pub fn from_incident(incident: &ThreatIncident) -> Self {
        Self {
            threat_id: incident.id.clone(),
            source_id: format!("source_{}", incident.id),
            threat_type: format!("{:?}", incident.threat_type),
            severity: incident.severity,
            confidence: incident.confidence,
            metadata: HashMap::new(),
            timestamp: incident.timestamp,
        }
    }

    /// Add metadata to context
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Outcome of mitigation execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MitigationOutcome {
    pub strategy_id: String,
    pub threat_type: String,
    pub features: HashMap<String, f64>,
    pub success: bool,
    pub actions_applied: Vec<String>,
    pub duration: Duration,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl MitigationOutcome {
    /// Calculate effectiveness score
    pub fn effectiveness_score(&self) -> f64 {
        if self.success {
            // Higher score for faster mitigations
            let time_factor = 1.0 - (self.duration.as_millis() as f64 / 1000.0).min(1.0);
            0.7 + 0.3 * time_factor
        } else {
            0.0
        }
    }

    /// Check if outcome requires rollback
    pub fn requires_rollback(&self) -> bool {
        !self.success && !self.actions_applied.is_empty()
    }
}

/// Pattern for rule updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub id: String,
    pub pattern_type: PatternType,
    pub confidence: f64,
    pub features: HashMap<String, f64>,
}

/// Pattern type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternType {
    Signature,
    Anomaly,
    Behavioral,
    Statistical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_block_action() {
        let context = ThreatContext {
            threat_id: "test-1".to_string(),
            source_id: "source-1".to_string(),
            threat_type: "anomaly".to_string(),
            severity: 8,
            confidence: 0.9,
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
        };

        let action = MitigationAction::BlockRequest {
            reason: "Test block".to_string(),
        };

        let result = action.execute(&context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limit_action() {
        let context = ThreatContext {
            threat_id: "test-2".to_string(),
            source_id: "source-2".to_string(),
            threat_type: "anomaly".to_string(),
            severity: 5,
            confidence: 0.7,
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
        };

        let action = MitigationAction::RateLimitUser {
            duration: Duration::from_secs(300),
        };

        let result = action.execute(&context).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_effectiveness_score() {
        let outcome = MitigationOutcome {
            strategy_id: "test".to_string(),
            threat_type: "anomaly".to_string(),
            features: HashMap::new(),
            success: true,
            actions_applied: vec!["action-1".to_string()],
            duration: Duration::from_millis(50),
            timestamp: chrono::Utc::now(),
        };

        let score = outcome.effectiveness_score();
        assert!(score > 0.7);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_context_creation() {
        let incident = crate::meta_learning::ThreatIncident {
            id: "test-3".to_string(),
            threat_type: crate::meta_learning::ThreatType::Anomaly(0.85),
            severity: 7,
            confidence: 0.9,
            timestamp: chrono::Utc::now(),
        };

        let context = ThreatContext::from_incident(&incident);
        assert_eq!(context.threat_id, "test-3");
        assert_eq!(context.severity, 7);
    }
}
