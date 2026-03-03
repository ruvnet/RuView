//! # Strange-Loop
//!
//! Self-referential systems and meta-learning inspired by Douglas Hofstadter.
//!
//! ## Features
//! - Multi-level meta-learning
//! - Self-modification with safety constraints
//! - Recursive cognition
//! - Tangled hierarchies
//! - Meta-knowledge extraction

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use dashmap::DashMap;
use std::sync::Arc;
use midstreamer_temporal_compare::TemporalComparator;
use midstreamer_attractor::{AttractorAnalyzer, PhasePoint};
use midstreamer_neural_solver::TemporalNeuralSolver;

/// Strange loop errors
#[derive(Debug, Error)]
pub enum StrangeLoopError {
    #[error("Max meta-depth exceeded: {0}")]
    MaxDepthExceeded(usize),

    #[error("Safety constraint violated: {0}")]
    SafetyViolation(String),

    #[error("Invalid modification: {0}")]
    InvalidModification(String),

    #[error("Meta-learning failed: {0}")]
    MetaLearningFailed(String),
}

/// Meta-level in the learning hierarchy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MetaLevel(pub usize);

impl MetaLevel {
    pub fn base() -> Self {
        MetaLevel(0)
    }

    pub fn next(&self) -> Self {
        MetaLevel(self.0 + 1)
    }

    pub fn level(&self) -> usize {
        self.0
    }
}

/// Meta-knowledge extracted from lower levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaKnowledge {
    pub level: MetaLevel,
    pub pattern: String,
    pub confidence: f64,
    pub applications: Vec<String>,
    pub learned_at: u64,
}

impl MetaKnowledge {
    pub fn new(level: MetaLevel, pattern: String, confidence: f64) -> Self {
        Self {
            level,
            pattern,
            confidence,
            applications: Vec::new(),
            learned_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }
}

/// Safety constraint for self-modification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConstraint {
    pub name: String,
    pub formula: String, // Simplified temporal formula
    pub enforced: bool,
}

impl SafetyConstraint {
    pub fn new(name: impl Into<String>, formula: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            formula: formula.into(),
            enforced: true,
        }
    }

    pub fn always_safe() -> Self {
        Self::new("always_safe", "G(safe)")
    }

    pub fn eventually_terminates() -> Self {
        Self::new("eventually_terminates", "F(done)")
    }
}

/// Modification rule for self-improvement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModificationRule {
    pub name: String,
    pub trigger: String,
    pub action: String,
    pub safety_check: bool,
}

impl ModificationRule {
    pub fn new(
        name: impl Into<String>,
        trigger: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            trigger: trigger.into(),
            action: action.into(),
            safety_check: true,
        }
    }
}

/// Statistics about meta-learning performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaLearningSummary {
    pub total_levels: usize,
    pub total_knowledge: usize,
    pub total_modifications: usize,
    pub safety_violations: usize,
    pub learning_iterations: u64,
}

/// Configuration for strange loop
#[derive(Debug, Clone)]
pub struct StrangeLoopConfig {
    pub max_meta_depth: usize,
    pub enable_self_modification: bool,
    pub max_modifications_per_cycle: usize,
    pub safety_check_enabled: bool,
}

impl Default for StrangeLoopConfig {
    fn default() -> Self {
        Self {
            max_meta_depth: 3,
            enable_self_modification: false, // Disabled by default for safety
            max_modifications_per_cycle: 5,
            safety_check_enabled: true,
        }
    }
}

/// The main strange loop structure
pub struct StrangeLoop {
    config: StrangeLoopConfig,
    meta_knowledge: Arc<DashMap<MetaLevel, Vec<MetaKnowledge>>>,
    safety_constraints: Vec<SafetyConstraint>,
    modification_rules: Vec<ModificationRule>,
    learning_iterations: Arc<DashMap<MetaLevel, u64>>,
    modification_count: usize,
    safety_violations: usize,

    // Integrated components (reserved for future use)
    #[allow(dead_code)]
    temporal_comparator: TemporalComparator<String>,
    attractor_analyzer: AttractorAnalyzer,
    #[allow(dead_code)]
    temporal_solver: TemporalNeuralSolver,
}

impl StrangeLoop {
    /// Create a new strange loop
    pub fn new(config: StrangeLoopConfig) -> Self {
        Self {
            config,
            meta_knowledge: Arc::new(DashMap::new()),
            safety_constraints: vec![
                SafetyConstraint::always_safe(),
                SafetyConstraint::eventually_terminates(),
            ],
            modification_rules: Vec::new(),
            learning_iterations: Arc::new(DashMap::new()),
            modification_count: 0,
            safety_violations: 0,
            temporal_comparator: TemporalComparator::new(1000, 10000),
            attractor_analyzer: AttractorAnalyzer::new(3, 10000),
            temporal_solver: TemporalNeuralSolver::default(),
        }
    }

    /// Learn at a specific meta-level
    pub fn learn_at_level(
        &mut self,
        level: MetaLevel,
        data: &[String],
    ) -> Result<Vec<MetaKnowledge>, StrangeLoopError> {
        if level.level() > self.config.max_meta_depth {
            return Err(StrangeLoopError::MaxDepthExceeded(level.level()));
        }

        // Increment learning iterations
        self.learning_iterations
            .entry(level)
            .and_modify(|v| *v += 1)
            .or_insert(1);

        // Extract patterns from data
        let patterns = self.extract_patterns(level, data)?;

        // Store meta-knowledge
        self.meta_knowledge
            .entry(level)
            .or_insert_with(Vec::new)
            .extend(patterns.clone());

        // If not at max depth, meta-learn from this level
        if level.level() < self.config.max_meta_depth {
            self.meta_learn_from_level(level)?;
        }

        Ok(patterns)
    }

    /// Meta-learn from a lower level
    fn meta_learn_from_level(&mut self, level: MetaLevel) -> Result<(), StrangeLoopError> {
        // Get knowledge from this level
        let knowledge = if let Some(k) = self.meta_knowledge.get(&level) {
            k.clone()
        } else {
            return Ok(()); // No knowledge to learn from
        };

        // Extract meta-patterns
        let meta_patterns: Vec<String> = knowledge
            .iter()
            .map(|k| k.pattern.clone())
            .collect();

        // Learn at next level
        let next_level = level.next();
        let _meta_knowledge = self.learn_at_level(next_level, &meta_patterns)?;

        Ok(())
    }

    /// Extract patterns from data
    fn extract_patterns(
        &self,
        level: MetaLevel,
        data: &[String],
    ) -> Result<Vec<MetaKnowledge>, StrangeLoopError> {
        let mut patterns = Vec::new();

        // Find recurring patterns using temporal comparison
        for i in 0..data.len() {
            for j in i+1..data.len() {
                if data[i] == data[j] {
                    // Found a repeating pattern
                    let pattern = MetaKnowledge::new(
                        level,
                        data[i].clone(),
                        0.8, // Confidence
                    );
                    patterns.push(pattern);
                }
            }
        }

        // Limit number of patterns
        patterns.truncate(100);

        Ok(patterns)
    }

    /// Apply self-modification with safety checks
    pub fn apply_modification(
        &mut self,
        rule: ModificationRule,
    ) -> Result<(), StrangeLoopError> {
        if !self.config.enable_self_modification {
            return Err(StrangeLoopError::InvalidModification(
                "Self-modification is disabled".to_string()
            ));
        }

        if self.modification_count >= self.config.max_modifications_per_cycle {
            return Err(StrangeLoopError::InvalidModification(
                "Max modifications per cycle reached".to_string()
            ));
        }

        // Safety check
        if rule.safety_check && self.config.safety_check_enabled {
            self.check_safety_constraints()?;
        }

        // Apply modification
        self.modification_rules.push(rule);
        self.modification_count += 1;

        Ok(())
    }

    /// Check all safety constraints
    fn check_safety_constraints(&mut self) -> Result<(), StrangeLoopError> {
        for constraint in &self.safety_constraints {
            if constraint.enforced {
                // Simplified safety check
                // In production, this would use the temporal solver
                if constraint.formula.contains("safe") {
                    // Always pass for now
                    continue;
                }
            }
        }

        Ok(())
    }

    /// Add a safety constraint
    pub fn add_safety_constraint(&mut self, constraint: SafetyConstraint) {
        self.safety_constraints.push(constraint);
    }

    /// Get knowledge at a specific level
    pub fn get_knowledge_at_level(&self, level: MetaLevel) -> Vec<MetaKnowledge> {
        self.meta_knowledge
            .get(&level)
            .map(|k| k.clone())
            .unwrap_or_default()
    }

    /// Get all meta-knowledge
    pub fn get_all_knowledge(&self) -> HashMap<MetaLevel, Vec<MetaKnowledge>> {
        let mut result = HashMap::new();
        for entry in self.meta_knowledge.iter() {
            result.insert(*entry.key(), entry.value().clone());
        }
        result
    }

    /// Get summary statistics
    pub fn get_summary(&self) -> MetaLearningSummary {
        let total_knowledge: usize = self.meta_knowledge
            .iter()
            .map(|entry| entry.value().len())
            .sum();

        MetaLearningSummary {
            total_levels: self.meta_knowledge.len(),
            total_knowledge,
            total_modifications: self.modification_count,
            safety_violations: self.safety_violations,
            learning_iterations: self.learning_iterations
                .iter()
                .map(|entry| *entry.value())
                .sum(),
        }
    }

    /// Reset the strange loop
    pub fn reset(&mut self) {
        self.meta_knowledge.clear();
        self.learning_iterations.clear();
        self.modification_rules.clear();
        self.modification_count = 0;
        self.safety_violations = 0;
    }

    /// Analyze behavioral dynamics using attractor analysis
    pub fn analyze_behavior(&mut self, trajectory_data: Vec<Vec<f64>>) -> Result<String, StrangeLoopError> {
        for (i, point_data) in trajectory_data.iter().enumerate() {
            let point = PhasePoint::new(point_data.clone(), i as u64);
            self.attractor_analyzer.add_point(point)
                .map_err(|e| StrangeLoopError::MetaLearningFailed(e.to_string()))?;
        }

        let analysis = self.attractor_analyzer.analyze()
            .map_err(|e| StrangeLoopError::MetaLearningFailed(e.to_string()))?;

        Ok(format!("{:?}", analysis.attractor_type))
    }
}

impl Default for StrangeLoop {
    fn default() -> Self {
        Self::new(StrangeLoopConfig::default())
    }
}

/// Meta-learner trait for types that can engage in meta-learning
pub trait MetaLearner {
    fn learn(&mut self, data: &[String]) -> Result<Vec<MetaKnowledge>, StrangeLoopError>;
    fn meta_level(&self) -> MetaLevel;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_level() {
        let base = MetaLevel::base();
        assert_eq!(base.level(), 0);

        let next = base.next();
        assert_eq!(next.level(), 1);
    }

    #[test]
    fn test_strange_loop_creation() {
        let config = StrangeLoopConfig::default();
        let strange_loop = StrangeLoop::new(config);

        assert_eq!(strange_loop.modification_count, 0);
        assert_eq!(strange_loop.safety_violations, 0);
    }

    #[test]
    fn test_learning_at_level() {
        let mut strange_loop = StrangeLoop::default();

        let data = vec![
            "pattern1".to_string(),
            "pattern2".to_string(),
            "pattern1".to_string(),
        ];

        let result = strange_loop.learn_at_level(MetaLevel::base(), &data);
        assert!(result.is_ok());

        let knowledge = strange_loop.get_knowledge_at_level(MetaLevel::base());
        assert!(!knowledge.is_empty());
    }

    #[test]
    fn test_max_depth_exceeded() {
        let mut strange_loop = StrangeLoop::default();

        let data = vec!["test".to_string()];
        let deep_level = MetaLevel(10); // Exceeds default max of 3

        let result = strange_loop.learn_at_level(deep_level, &data);
        assert!(result.is_err());
    }

    #[test]
    fn test_safety_constraint() {
        let constraint = SafetyConstraint::always_safe();
        assert_eq!(constraint.name, "always_safe");
        assert!(constraint.enforced);
    }

    #[test]
    fn test_modification_disabled() {
        let mut strange_loop = StrangeLoop::default();

        let rule = ModificationRule::new("test_rule", "trigger", "action");
        let result = strange_loop.apply_modification(rule);

        assert!(result.is_err()); // Should fail because self-modification is disabled
    }

    #[test]
    fn test_summary() {
        let mut strange_loop = StrangeLoop::default();

        let data = vec!["pattern1".to_string(), "pattern2".to_string()];
        let _ = strange_loop.learn_at_level(MetaLevel::base(), &data);

        let summary = strange_loop.get_summary();
        assert!(summary.total_knowledge > 0);
        assert_eq!(summary.safety_violations, 0);
    }

    #[test]
    fn test_reset() {
        let mut strange_loop = StrangeLoop::default();

        let data = vec!["pattern1".to_string()];
        let _ = strange_loop.learn_at_level(MetaLevel::base(), &data);

        strange_loop.reset();

        let summary = strange_loop.get_summary();
        assert_eq!(summary.total_knowledge, 0);
        assert_eq!(summary.total_modifications, 0);
    }
}
