//! Adaptive mitigation with self-improving strategy selection

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::meta_learning::ThreatIncident;
use crate::{MitigationAction, MitigationOutcome, ThreatContext, Result, ResponseError};
use serde::{Deserialize, Serialize};

/// Adaptive mitigator with strategy selection and effectiveness tracking
pub struct AdaptiveMitigator {
    /// Available mitigation strategies
    strategies: Vec<MitigationStrategy>,

    /// Effectiveness scores per strategy
    effectiveness_scores: HashMap<String, f64>,

    /// Strategy application history
    application_history: Vec<StrategyApplication>,

    /// Strategy selector
    selector: Arc<RwLock<StrategySelector>>,
}

impl AdaptiveMitigator {
    /// Create new adaptive mitigator
    pub fn new() -> Self {
        let strategies = Self::initialize_strategies();
        let effectiveness_scores = strategies.iter()
            .map(|s| (s.id.clone(), 0.5))
            .collect();

        Self {
            strategies,
            effectiveness_scores,
            application_history: Vec::new(),
            selector: Arc::new(RwLock::new(StrategySelector::new())),
        }
    }

    /// Apply mitigation to threat
    pub async fn apply_mitigation(&self, threat: &ThreatIncident) -> Result<MitigationOutcome> {
        // Select best strategy for threat
        let strategy = self.select_strategy(threat).await?;

        // Create threat context
        let context = ThreatContext::from_incident(threat);

        // Execute mitigation actions
        let start = std::time::Instant::now();
        let result = strategy.execute(&context).await;
        let duration = start.elapsed();

        // Build outcome
        let outcome = match result {
            Ok(actions_applied) => {
                MitigationOutcome {
                    strategy_id: strategy.id.clone(),
                    threat_type: Self::threat_type_string(&threat.threat_type),
                    features: Self::extract_features(threat),
                    success: true,
                    actions_applied,
                    duration,
                    timestamp: chrono::Utc::now(),
                }
            }
            Err(_e) => {
                MitigationOutcome {
                    strategy_id: strategy.id.clone(),
                    threat_type: Self::threat_type_string(&threat.threat_type),
                    features: Self::extract_features(threat),
                    success: false,
                    actions_applied: Vec::new(),
                    duration,
                    timestamp: chrono::Utc::now(),
                }
            }
        };

        Ok(outcome)
    }

    /// Update effectiveness score for strategy
    pub fn update_effectiveness(&mut self, strategy_id: &str, success: bool) {
        if let Some(score) = self.effectiveness_scores.get_mut(strategy_id) {
            // Exponential moving average
            let alpha = 0.3;
            let new_value = if success { 1.0 } else { 0.0 };
            *score = alpha * new_value + (1.0 - alpha) * *score;
        }

        // Record application
        self.application_history.push(StrategyApplication {
            strategy_id: strategy_id.to_string(),
            success,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Get count of active strategies
    pub fn active_strategies_count(&self) -> usize {
        self.strategies.iter()
            .filter(|s| self.effectiveness_scores.get(&s.id).is_some_and(|&score| score > 0.3))
            .count()
    }

    /// Select best strategy for threat
    async fn select_strategy(&self, threat: &ThreatIncident) -> Result<MitigationStrategy> {
        let mut selector = self.selector.write().await;

        // Get candidate strategies
        let candidates: Vec<_> = self.strategies.iter()
            .filter(|s| s.applicable_to(threat))
            .collect();

        if candidates.is_empty() {
            return Err(ResponseError::StrategyNotFound(
                "No applicable strategies found".to_string()
            ));
        }

        // Select based on effectiveness scores
        let best = candidates.iter()
            .max_by(|a, b| {
                let score_a = self.effectiveness_scores.get(&a.id).unwrap_or(&0.0);
                let score_b = self.effectiveness_scores.get(&b.id).unwrap_or(&0.0);
                score_a.partial_cmp(score_b).unwrap()
            })
            .unwrap();

        // Update selector statistics
        selector.record_selection(&best.id);

        Ok((*best).clone())
    }

    /// Initialize default mitigation strategies
    fn initialize_strategies() -> Vec<MitigationStrategy> {
        vec![
            MitigationStrategy::block_request(),
            MitigationStrategy::rate_limit(),
            MitigationStrategy::require_verification(),
            MitigationStrategy::alert_human(),
            MitigationStrategy::update_rules(),
            MitigationStrategy::quarantine_source(),
            MitigationStrategy::adaptive_throttle(),
        ]
    }

    /// Convert threat type to string
    fn threat_type_string(threat_type: &crate::meta_learning::ThreatType) -> String {
        match threat_type {
            crate::meta_learning::ThreatType::Anomaly(_) => "anomaly".to_string(),
            crate::meta_learning::ThreatType::Attack(attack) => format!("attack_{:?}", attack),
            crate::meta_learning::ThreatType::Intrusion(_) => "intrusion".to_string(),
        }
    }

    /// Extract features from threat
    fn extract_features(threat: &ThreatIncident) -> HashMap<String, f64> {
        let mut features = HashMap::new();
        features.insert("severity".to_string(), threat.severity as f64);
        features.insert("confidence".to_string(), threat.confidence);
        features
    }
}

impl Default for AdaptiveMitigator {
    fn default() -> Self {
        Self::new()
    }
}

/// Mitigation strategy with actions and applicability rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MitigationStrategy {
    pub id: String,
    pub name: String,
    pub description: String,
    pub actions: Vec<MitigationAction>,
    pub min_severity: u8,
    pub applicable_threats: Vec<String>,
    pub priority: u8,
}

impl MitigationStrategy {
    /// Check if strategy applies to threat
    pub fn applicable_to(&self, threat: &ThreatIncident) -> bool {
        threat.severity >= self.min_severity
    }

    /// Execute strategy actions
    pub async fn execute(&self, context: &ThreatContext) -> Result<Vec<String>> {
        let mut applied_actions = Vec::new();

        for action in &self.actions {
            match action.execute(context).await {
                Ok(action_id) => {
                    applied_actions.push(action_id);
                }
                Err(e) => {
                    tracing::warn!("Action failed: {:?}", e);
                    // Continue with remaining actions
                }
            }
        }

        Ok(applied_actions)
    }

    /// Create block request strategy
    pub fn block_request() -> Self {
        Self {
            id: "block_request".to_string(),
            name: "Block Request".to_string(),
            description: "Immediately block the threatening request".to_string(),
            actions: vec![
                MitigationAction::BlockRequest {
                    reason: "Threat detected".to_string(),
                }
            ],
            min_severity: 7,
            applicable_threats: vec!["attack".to_string(), "intrusion".to_string()],
            priority: 9,
        }
    }

    /// Create rate limit strategy
    pub fn rate_limit() -> Self {
        Self {
            id: "rate_limit".to_string(),
            name: "Rate Limit".to_string(),
            description: "Apply rate limiting to source".to_string(),
            actions: vec![
                MitigationAction::RateLimitUser {
                    duration: std::time::Duration::from_secs(300),
                }
            ],
            min_severity: 5,
            applicable_threats: vec!["anomaly".to_string(), "attack".to_string()],
            priority: 6,
        }
    }

    /// Create verification requirement strategy
    pub fn require_verification() -> Self {
        Self {
            id: "require_verification".to_string(),
            name: "Require Verification".to_string(),
            description: "Require additional verification from user".to_string(),
            actions: vec![
                MitigationAction::RequireVerification {
                    challenge_type: ChallengeType::Captcha,
                }
            ],
            min_severity: 4,
            applicable_threats: vec!["anomaly".to_string()],
            priority: 5,
        }
    }

    /// Create human alert strategy
    pub fn alert_human() -> Self {
        Self {
            id: "alert_human".to_string(),
            name: "Alert Human".to_string(),
            description: "Alert security team for manual review".to_string(),
            actions: vec![
                MitigationAction::AlertHuman {
                    priority: AlertPriority::High,
                }
            ],
            min_severity: 8,
            applicable_threats: vec!["attack".to_string(), "intrusion".to_string()],
            priority: 8,
        }
    }

    /// Create rule update strategy
    pub fn update_rules() -> Self {
        Self {
            id: "update_rules".to_string(),
            name: "Update Rules".to_string(),
            description: "Dynamically update detection rules".to_string(),
            actions: vec![
                MitigationAction::UpdateRules {
                    new_patterns: Vec::new(),
                }
            ],
            min_severity: 3,
            applicable_threats: vec!["anomaly".to_string()],
            priority: 3,
        }
    }

    /// Create quarantine strategy
    pub fn quarantine_source() -> Self {
        Self {
            id: "quarantine_source".to_string(),
            name: "Quarantine Source".to_string(),
            description: "Isolate threat source".to_string(),
            actions: vec![
                MitigationAction::BlockRequest {
                    reason: "Source quarantined".to_string(),
                }
            ],
            min_severity: 9,
            applicable_threats: vec!["attack".to_string(), "intrusion".to_string()],
            priority: 10,
        }
    }

    /// Create adaptive throttle strategy
    pub fn adaptive_throttle() -> Self {
        Self {
            id: "adaptive_throttle".to_string(),
            name: "Adaptive Throttle".to_string(),
            description: "Dynamically adjust rate limits".to_string(),
            actions: vec![
                MitigationAction::RateLimitUser {
                    duration: std::time::Duration::from_secs(60),
                }
            ],
            min_severity: 3,
            applicable_threats: vec!["anomaly".to_string()],
            priority: 4,
        }
    }
}

/// Strategy selector with selection tracking
struct StrategySelector {
    selection_counts: HashMap<String, u64>,
    last_selected: Option<String>,
}

impl StrategySelector {
    fn new() -> Self {
        Self {
            selection_counts: HashMap::new(),
            last_selected: None,
        }
    }

    fn record_selection(&mut self, strategy_id: &str) {
        *self.selection_counts.entry(strategy_id.to_string()).or_insert(0) += 1;
        self.last_selected = Some(strategy_id.to_string());
    }
}

/// Record of strategy application
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StrategyApplication {
    strategy_id: String,
    success: bool,
    timestamp: chrono::DateTime<chrono::Utc>,
}

/// Challenge type for verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChallengeType {
    Captcha,
    TwoFactor,
    EmailVerification,
    PhoneVerification,
}

/// Alert priority levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta_learning::{ThreatIncident, ThreatType};

    #[tokio::test]
    async fn test_mitigator_creation() {
        let mitigator = AdaptiveMitigator::new();
        assert!(mitigator.active_strategies_count() > 0);
    }

    #[tokio::test]
    async fn test_strategy_selection() {
        let mitigator = AdaptiveMitigator::new();

        let threat = ThreatIncident {
            id: "test-1".to_string(),
            threat_type: ThreatType::Anomaly(0.85),
            severity: 7,
            confidence: 0.9,
            timestamp: chrono::Utc::now(),
        };

        let strategy = mitigator.select_strategy(&threat).await;
        assert!(strategy.is_ok());
    }

    #[test]
    fn test_effectiveness_update() {
        let mut mitigator = AdaptiveMitigator::new();
        let strategy_id = "block_request";

        let initial = mitigator.effectiveness_scores.get(strategy_id).copied().unwrap();

        mitigator.update_effectiveness(strategy_id, true);
        let updated = mitigator.effectiveness_scores.get(strategy_id).copied().unwrap();

        assert!(updated > initial);
    }

    #[test]
    fn test_strategy_applicability() {
        let strategy = MitigationStrategy::block_request();

        let high_severity = ThreatIncident {
            id: "test".to_string(),
            threat_type: ThreatType::Anomaly(0.9),
            severity: 9,
            confidence: 0.9,
            timestamp: chrono::Utc::now(),
        };

        let low_severity = ThreatIncident {
            id: "test".to_string(),
            threat_type: ThreatType::Anomaly(0.5),
            severity: 3,
            confidence: 0.5,
            timestamp: chrono::Utc::now(),
        };

        assert!(strategy.applicable_to(&high_severity));
        assert!(!strategy.applicable_to(&low_severity));
    }
}
