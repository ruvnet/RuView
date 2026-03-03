//! Meta-learning engine using strange-loop for recursive self-improvement

use std::collections::HashMap;
use midstreamer_strange_loop::{StrangeLoop, StrangeLoopConfig, MetaLevel, MetaKnowledge};
use crate::{MitigationOutcome, FeedbackSignal};
use serde::{Deserialize, Serialize};

/// Adaptive rule learned from threat incidents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveRule {
    pub id: String,
    pub pattern: ThreatPattern,
    pub confidence: f64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub success_count: u64,
    pub failure_count: u64,
}

/// Threat pattern representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatPattern {
    pub features: HashMap<String, f64>,
    pub threat_type: String,
    pub severity_threshold: f64,
}

impl Default for ThreatPattern {
    fn default() -> Self {
        Self {
            features: HashMap::new(),
            threat_type: "unknown".to_string(),
            severity_threshold: 0.5,
        }
    }
}

impl ThreatPattern {
    pub fn from_features(features: &HashMap<String, f64>) -> Self {
        Self {
            features: features.clone(),
            threat_type: "detected".to_string(),
            severity_threshold: 0.5,
        }
    }
}

/// Meta-learning engine for autonomous response optimization
pub struct MetaLearningEngine {
    /// Strange-loop meta-learner (25 levels validated)
    learner: StrangeLoop,

    /// Learned patterns from successful detections
    learned_patterns: Vec<AdaptiveRule>,

    /// Pattern effectiveness tracking
    pattern_effectiveness: HashMap<String, EffectivenessMetrics>,

    /// Current optimization level (0-25)
    current_level: usize,

    /// Learning rate for pattern updates
    learning_rate: f64,
}

impl MetaLearningEngine {
    /// Create new meta-learning engine
    pub fn new() -> Self {
        let config = StrangeLoopConfig {
            max_meta_depth: 25,
            enable_self_modification: true,
            max_modifications_per_cycle: 10,
            safety_check_enabled: true,
        };

        Self {
            learner: StrangeLoop::new(config),
            learned_patterns: Vec::new(),
            pattern_effectiveness: HashMap::new(),
            current_level: 0,
            learning_rate: 0.1,
        }
    }

    /// Learn from mitigation outcome
    pub async fn learn_from_outcome(&mut self, outcome: &MitigationOutcome) {
        // Extract pattern from outcome
        let pattern = self.extract_pattern(outcome);

        // Update pattern effectiveness
        self.update_pattern_effectiveness(&pattern, outcome.success);

        // Apply meta-learning if pattern is significant
        if self.is_significant_pattern(&pattern) {
            self.apply_meta_learning(pattern).await;
        }
    }

    /// Learn from threat incident
    pub async fn learn_from_incident(&mut self, incident: &ThreatIncident) {
        // Extract features from incident
        let features = self.extract_incident_features(incident);

        // Create adaptive rule
        let rule = AdaptiveRule {
            id: uuid::Uuid::new_v4().to_string(),
            pattern: ThreatPattern::from_features(&features),
            confidence: 0.5, // Initial confidence
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            success_count: 0,
            failure_count: 0,
        };

        // Add to learned patterns
        self.learned_patterns.push(rule);

        // Trigger meta-learning optimization
        self.optimize_patterns().await;
    }

    /// Optimize strategies based on feedback signals
    pub fn optimize_strategy(&mut self, feedback: &[FeedbackSignal]) {
        for signal in feedback {
            // Update effectiveness metrics
            if let Some(metrics) = self.pattern_effectiveness.get_mut(&signal.strategy_id) {
                metrics.update(signal.effectiveness_score, signal.success);
            }
        }

        // Apply recursive optimization
        self.recursive_optimize(self.current_level);

        // Advance optimization level if ready
        if self.should_advance_level() {
            self.current_level = (self.current_level + 1).min(25);
        }
    }

    /// Get count of learned patterns
    pub fn learned_patterns_count(&self) -> usize {
        self.learned_patterns.len()
    }

    /// Get current optimization level
    pub fn current_optimization_level(&self) -> usize {
        self.current_level
    }

    /// Extract pattern from mitigation outcome
    fn extract_pattern(&self, outcome: &MitigationOutcome) -> LearnedPattern {
        LearnedPattern {
            id: uuid::Uuid::new_v4().to_string(),
            strategy_id: outcome.strategy_id.clone(),
            threat_type: outcome.threat_type.clone(),
            features: outcome.features.clone(),
            success: outcome.success,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Update pattern effectiveness tracking
    fn update_pattern_effectiveness(&mut self, pattern: &LearnedPattern, success: bool) {
        let metrics = self.pattern_effectiveness
            .entry(pattern.id.clone())
            .or_insert_with(EffectivenessMetrics::new);

        metrics.update(if success { 1.0 } else { 0.0 }, success);
    }

    /// Check if pattern is significant enough for meta-learning
    fn is_significant_pattern(&self, pattern: &LearnedPattern) -> bool {
        if let Some(metrics) = self.pattern_effectiveness.get(&pattern.id) {
            metrics.total_applications >= 5 && metrics.average_score > 0.6
        } else {
            false
        }
    }

    /// Apply meta-learning to pattern
    async fn apply_meta_learning(&mut self, pattern: LearnedPattern) {
        // Use strange-loop's learn_at_level for meta-learning
        let meta_level = MetaLevel(self.current_level);
        let confidence = self.calculate_pattern_confidence(&pattern);

        // Create knowledge strings from pattern
        let knowledge_data = vec![
            format!("pattern_id: {}", pattern.id),
            format!("threat_type: {}", pattern.threat_type),
            format!("confidence: {}", confidence),
        ];

        // Apply meta-learning at current level
        if let Ok(meta_knowledge_vec) = self.learner.learn_at_level(
            meta_level,
            &knowledge_data,
        ) {
            // Update learned patterns with first meta-knowledge (if any)
            if let Some(meta_knowledge) = meta_knowledge_vec.first() {
                self.update_learned_patterns_from_knowledge(&pattern.id, meta_knowledge.clone());
            }
        }
    }

    /// Calculate confidence for pattern
    fn calculate_pattern_confidence(&self, pattern: &LearnedPattern) -> f64 {
        if let Some(metrics) = self.pattern_effectiveness.get(&pattern.id) {
            metrics.average_score
        } else {
            0.5
        }
    }

    /// Update learned patterns from meta-knowledge
    fn update_learned_patterns_from_knowledge(&mut self, pattern_id: &str, knowledge: MetaKnowledge) {
        // Find and update existing rule or create new one
        if let Some(rule) = self.learned_patterns.iter_mut()
            .find(|r| r.id == pattern_id) {
            rule.confidence = knowledge.confidence;
            rule.updated_at = chrono::Utc::now();
        }
    }

    /// Extract features from incident
    fn extract_incident_features(&self, incident: &ThreatIncident) -> HashMap<String, f64> {
        let mut features = HashMap::new();

        features.insert("severity".to_string(), incident.severity as f64);
        features.insert("confidence".to_string(), incident.confidence);

        // Add type-specific features
        match &incident.threat_type {
            ThreatType::Anomaly(score) => {
                features.insert("anomaly_score".to_string(), *score);
            }
            ThreatType::Attack(attack_type) => {
                features.insert("attack_type_id".to_string(), attack_type.to_id() as f64);
            }
            ThreatType::Intrusion(level) => {
                features.insert("intrusion_level".to_string(), *level as f64);
            }
        }

        features
    }

    /// Optimize patterns using meta-learning
    async fn optimize_patterns(&mut self) {
        // Apply strange-loop recursive optimization
        for level in 0..=self.current_level {
            self.recursive_optimize(level);
        }

        // Prune low-confidence patterns
        self.learned_patterns.retain(|p| p.confidence > 0.3);
    }

    /// Recursive optimization at given level
    fn recursive_optimize(&mut self, level: usize) {
        // Meta-meta-learning: optimize the optimization strategy itself
        let optimization_effectiveness = self.calculate_optimization_effectiveness();

        // Adjust learning rate based on effectiveness
        if optimization_effectiveness > 0.8 {
            self.learning_rate *= 1.1; // Increase learning rate
        } else if optimization_effectiveness < 0.4 {
            self.learning_rate *= 0.9; // Decrease learning rate
        }

        // Apply recursive pattern refinement
        let learning_rate = self.learning_rate;
        for pattern in &mut self.learned_patterns {
            // Apply recursive refinement inline to avoid borrow checker issues
            let refinement = learning_rate * (level as f64 / 25.0);
            pattern.confidence = (pattern.confidence + refinement).clamp(0.0, 1.0);
        }
    }

    /// Calculate optimization effectiveness
    fn calculate_optimization_effectiveness(&self) -> f64 {
        if self.pattern_effectiveness.is_empty() {
            return 0.5;
        }

        let total: f64 = self.pattern_effectiveness.values()
            .map(|m| m.average_score)
            .sum();

        total / self.pattern_effectiveness.len() as f64
    }

    /// Refine confidence at given optimization level
    #[allow(dead_code)]
    fn refine_confidence(&self, current: f64, level: usize) -> f64 {
        // Apply recursive refinement
        let refinement = self.learning_rate * (level as f64 / 25.0);
        (current + refinement).clamp(0.0, 1.0)
    }

    /// Check if should advance to next optimization level
    fn should_advance_level(&self) -> bool {
        let effectiveness = self.calculate_optimization_effectiveness();
        effectiveness > 0.75 && self.learned_patterns.len() >= 10
    }
}

impl Default for MetaLearningEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Pattern learned from mitigation outcomes
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LearnedPattern {
    id: String,
    strategy_id: String,
    threat_type: String,
    features: HashMap<String, f64>,
    success: bool,
    timestamp: chrono::DateTime<chrono::Utc>,
}

/// Metrics for pattern effectiveness tracking
#[derive(Debug, Clone)]
struct EffectivenessMetrics {
    total_applications: u64,
    successful_applications: u64,
    average_score: f64,
    last_updated: chrono::DateTime<chrono::Utc>,
}

impl EffectivenessMetrics {
    fn new() -> Self {
        Self {
            total_applications: 0,
            successful_applications: 0,
            average_score: 0.0,
            last_updated: chrono::Utc::now(),
        }
    }

    fn update(&mut self, score: f64, success: bool) {
        self.total_applications += 1;
        if success {
            self.successful_applications += 1;
        }

        // Update running average
        self.average_score = (self.average_score * (self.total_applications - 1) as f64 + score)
            / self.total_applications as f64;

        self.last_updated = chrono::Utc::now();
    }
}

/// Threat incident for meta-learning
#[derive(Debug, Clone)]
pub struct ThreatIncident {
    pub id: String,
    pub threat_type: ThreatType,
    pub severity: u8,
    pub confidence: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Threat type enumeration
#[derive(Debug, Clone)]
pub enum ThreatType {
    Anomaly(f64),
    Attack(AttackType),
    Intrusion(u8),
}

/// Attack type enumeration
#[derive(Debug, Clone)]
pub enum AttackType {
    DDoS,
    SqlInjection,
    XSS,
    CSRF,
    Other(String),
}

impl AttackType {
    fn to_id(&self) -> u8 {
        match self {
            AttackType::DDoS => 1,
            AttackType::SqlInjection => 2,
            AttackType::XSS => 3,
            AttackType::CSRF => 4,
            AttackType::Other(_) => 99,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_meta_learning_creation() {
        let engine = MetaLearningEngine::new();
        assert_eq!(engine.current_level, 0);
        assert_eq!(engine.learned_patterns_count(), 0);
    }

    #[tokio::test]
    async fn test_pattern_learning() {
        let mut engine = MetaLearningEngine::new();

        let incident = ThreatIncident {
            id: "test-1".to_string(),
            threat_type: ThreatType::Anomaly(0.85),
            severity: 7,
            confidence: 0.9,
            timestamp: chrono::Utc::now(),
        };

        engine.learn_from_incident(&incident).await;
        assert!(engine.learned_patterns_count() > 0);
    }

    #[test]
    fn test_effectiveness_metrics() {
        let mut metrics = EffectivenessMetrics::new();

        metrics.update(0.8, true);
        assert_eq!(metrics.total_applications, 1);
        assert_eq!(metrics.successful_applications, 1);
        assert_eq!(metrics.average_score, 0.8);

        metrics.update(0.6, false);
        assert_eq!(metrics.total_applications, 2);
        assert_eq!(metrics.successful_applications, 1);
        assert_eq!(metrics.average_score, 0.7);
    }

    #[test]
    fn test_optimization_level_advancement() {
        let mut engine = MetaLearningEngine::new();

        // Add sufficient patterns
        for i in 0..15 {
            engine.learned_patterns.push(AdaptiveRule {
                id: format!("rule-{}", i),
                pattern: ThreatPattern::default(),
                confidence: 0.8,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                success_count: 10,
                failure_count: 2,
            });
        }

        // Should be ready to advance
        assert!(engine.should_advance_level());
    }
}
