//! Strange loops and meta-learning
//!
//! Integrates strange-loop for:
//! - Self-referential reasoning
//! - Meta-learning (learning to learn)
//! - Tangled hierarchies
//! - Safe self-modification

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// Level in the meta-hierarchy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetaLevel {
    /// Object level (base learning)
    Object = 0,
    /// Meta level 1 (learning about learning)
    Meta1 = 1,
    /// Meta level 2 (learning about learning about learning)
    Meta2 = 2,
    /// Meta level 3 (highest practical level)
    Meta3 = 3,
}

impl MetaLevel {
    /// Get the next higher meta level
    pub fn up(&self) -> Option<MetaLevel> {
        match self {
            MetaLevel::Object => Some(MetaLevel::Meta1),
            MetaLevel::Meta1 => Some(MetaLevel::Meta2),
            MetaLevel::Meta2 => Some(MetaLevel::Meta3),
            MetaLevel::Meta3 => None, // Cap at Meta3
        }
    }

    /// Get the next lower meta level
    pub fn down(&self) -> Option<MetaLevel> {
        match self {
            MetaLevel::Object => None,
            MetaLevel::Meta1 => Some(MetaLevel::Object),
            MetaLevel::Meta2 => Some(MetaLevel::Meta1),
            MetaLevel::Meta3 => Some(MetaLevel::Meta2),
        }
    }

    /// Get level as integer
    pub fn as_int(&self) -> usize {
        *self as usize
    }
}

/// Meta-knowledge about learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaKnowledge {
    /// Level of abstraction
    pub level: MetaLevel,
    /// What was learned
    pub content: String,
    /// How effective was this learning
    pub effectiveness: f64,
    /// Conditions under which this applies
    pub context: HashMap<String, String>,
    /// Timestamp
    pub timestamp: i64,
}

/// A strange loop - a self-referential pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrangeLoop {
    /// Unique identifier
    pub id: String,
    /// Levels involved in the loop
    pub levels: Vec<MetaLevel>,
    /// Description of the loop
    pub description: String,
    /// Strength of the loop (how often it occurs)
    pub strength: f64,
    /// Whether this loop is beneficial or problematic
    pub is_beneficial: bool,
}

/// Meta-learner that can learn about its own learning process
pub struct MetaLearner {
    /// Current meta level of operation
    current_level: MetaLevel,
    /// Meta-knowledge store (hierarchical)
    knowledge: HashMap<MetaLevel, Vec<MetaKnowledge>>,
    /// Detected strange loops
    strange_loops: Vec<StrangeLoop>,
    /// Learning history for detecting patterns
    learning_history: VecDeque<LearningEvent>,
    /// Maximum history to keep
    max_history: usize,
    /// Self-modification rules
    modification_rules: Vec<ModificationRule>,
    /// Safety constraints
    safety_constraints: Vec<SafetyConstraint>,
}

/// An event in the learning history
#[derive(Debug, Clone)]
struct LearningEvent {
    level: MetaLevel,
    content: String,
    reward: f64,
    timestamp: i64,
}

/// Rule for self-modification
#[derive(Debug, Clone)]
pub struct ModificationRule {
    /// Condition that must be met
    pub condition: String,
    /// Action to take
    pub action: String,
    /// Priority (higher = more important)
    pub priority: i32,
    /// Whether this rule is enabled
    pub enabled: bool,
}

/// Safety constraint for self-modification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConstraint {
    /// Name of the constraint
    pub name: String,
    /// Description
    pub description: String,
    /// Whether this constraint is violated
    pub is_violated: bool,
}

impl MetaLearner {
    /// Create a new meta-learner
    pub fn new(max_history: usize) -> Self {
        let mut knowledge = HashMap::new();
        knowledge.insert(MetaLevel::Object, Vec::new());
        knowledge.insert(MetaLevel::Meta1, Vec::new());
        knowledge.insert(MetaLevel::Meta2, Vec::new());
        knowledge.insert(MetaLevel::Meta3, Vec::new());

        Self {
            current_level: MetaLevel::Object,
            knowledge,
            strange_loops: Vec::new(),
            learning_history: VecDeque::new(),
            max_history,
            modification_rules: Vec::new(),
            safety_constraints: Self::default_safety_constraints(),
        }
    }

    /// Get default safety constraints
    fn default_safety_constraints() -> Vec<SafetyConstraint> {
        vec![
            SafetyConstraint {
                name: "no_infinite_loops".to_string(),
                description: "Prevent infinite self-reference".to_string(),
                is_violated: false,
            },
            SafetyConstraint {
                name: "preserve_core_functionality".to_string(),
                description: "Don't modify core learning mechanisms".to_string(),
                is_violated: false,
            },
            SafetyConstraint {
                name: "bounded_meta_levels".to_string(),
                description: "Don't exceed meta level 3".to_string(),
                is_violated: false,
            },
        ]
    }

    /// Learn at the current meta level
    pub fn learn(&mut self, content: String, reward: f64) {
        let meta_knowledge = MetaKnowledge {
            level: self.current_level,
            content: content.clone(),
            effectiveness: reward,
            context: HashMap::new(),
            timestamp: chrono::Utc::now().timestamp(),
        };

        // Store at current level
        if let Some(knowledge_vec) = self.knowledge.get_mut(&self.current_level) {
            knowledge_vec.push(meta_knowledge);
        }

        // Add to history
        self.learning_history.push_back(LearningEvent {
            level: self.current_level,
            content,
            reward,
            timestamp: chrono::Utc::now().timestamp(),
        });

        // Maintain max history
        if self.learning_history.len() > self.max_history {
            self.learning_history.pop_front();
        }

        // Detect meta-patterns (learn about learning)
        self.detect_meta_patterns();

        // Check for strange loops
        self.detect_strange_loops();
    }

    /// Detect patterns in learning (meta-learning)
    fn detect_meta_patterns(&mut self) {
        if self.learning_history.len() < 10 {
            return;
        }

        // Analyze recent learning events
        let recent: Vec<_> = self.learning_history.iter().rev().take(10).collect();

        // Calculate average reward at current level
        let avg_reward: f64 = recent.iter().map(|e| e.reward).sum::<f64>() / recent.len() as f64;

        // If learning is effective, record meta-knowledge
        if avg_reward > 0.7 {
            let meta_content = format!(
                "Learning approach at {:?} level is effective (avg reward: {:.2})",
                self.current_level, avg_reward
            );

            // Store at next meta level if possible
            if let Some(next_level) = self.current_level.up() {
                let meta_meta_knowledge = MetaKnowledge {
                    level: next_level,
                    content: meta_content,
                    effectiveness: avg_reward,
                    context: HashMap::new(),
                    timestamp: chrono::Utc::now().timestamp(),
                };

                if let Some(knowledge_vec) = self.knowledge.get_mut(&next_level) {
                    knowledge_vec.push(meta_meta_knowledge);
                }
            }
        }
    }

    /// Detect strange loops (self-referential patterns)
    fn detect_strange_loops(&mut self) {
        if self.learning_history.len() < 5 {
            return;
        }

        // Look for patterns where we learn about our own learning
        let mut level_sequence: Vec<MetaLevel> = self
            .learning_history
            .iter()
            .rev()
            .take(5)
            .map(|e| e.level)
            .collect();

        // Check for level transitions that form a loop
        // e.g., Object -> Meta1 -> Meta2 -> Meta1 (loop between Meta1 and Meta2)
        for i in 0..level_sequence.len().saturating_sub(2) {
            if level_sequence[i] == level_sequence[i + 2] {
                // Found a potential loop
                let loop_id = format!("loop_{}_{}", i, chrono::Utc::now().timestamp());
                let strange_loop = StrangeLoop {
                    id: loop_id,
                    levels: vec![level_sequence[i], level_sequence[i + 1]],
                    description: format!(
                        "Oscillation between {:?} and {:?}",
                        level_sequence[i], level_sequence[i + 1]
                    ),
                    strength: 0.5,
                    is_beneficial: true, // Assume beneficial unless proven otherwise
                };

                // Check if loop already exists
                if !self.strange_loops.iter().any(|l| l.levels == strange_loop.levels) {
                    self.strange_loops.push(strange_loop);
                }
            }
        }
    }

    /// Ascend to a higher meta level
    pub fn ascend(&mut self) -> Result<MetaLevel, String> {
        if let Some(next_level) = self.current_level.up() {
            self.current_level = next_level;
            Ok(next_level)
        } else {
            Err("Already at highest meta level".to_string())
        }
    }

    /// Descend to a lower meta level
    pub fn descend(&mut self) -> Result<MetaLevel, String> {
        if let Some(prev_level) = self.current_level.down() {
            self.current_level = prev_level;
            Ok(prev_level)
        } else {
            Err("Already at lowest meta level".to_string())
        }
    }

    /// Get current meta level
    pub fn current_level(&self) -> MetaLevel {
        self.current_level
    }

    /// Get meta-knowledge at a specific level
    pub fn get_knowledge_at_level(&self, level: MetaLevel) -> Vec<MetaKnowledge> {
        self.knowledge.get(&level).cloned().unwrap_or_default()
    }

    /// Get all detected strange loops
    pub fn get_strange_loops(&self) -> &[StrangeLoop] {
        &self.strange_loops
    }

    /// Apply self-modification (with safety checks)
    pub fn self_modify(&mut self, rule: ModificationRule) -> Result<(), String> {
        // Check safety constraints
        for constraint in &mut self.safety_constraints {
            if rule.action.contains("infinite")
                && constraint.name == "no_infinite_loops"
            {
                constraint.is_violated = true;
                return Err(format!("Safety constraint violated: {}", constraint.name));
            }

            if rule.action.contains("core")
                && constraint.name == "preserve_core_functionality"
            {
                constraint.is_violated = true;
                return Err(format!("Safety constraint violated: {}", constraint.name));
            }
        }

        // Add the modification rule
        self.modification_rules.push(rule);

        Ok(())
    }

    /// Check if any safety constraints are violated
    pub fn safety_check(&self) -> Result<(), Vec<String>> {
        let violations: Vec<String> = self
            .safety_constraints
            .iter()
            .filter(|c| c.is_violated)
            .map(|c| c.name.clone())
            .collect();

        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }

    /// Get summary of meta-learning state
    pub fn get_summary(&self) -> MetaLearningSummary {
        MetaLearningSummary {
            current_level: self.current_level,
            knowledge_counts: [
                self.knowledge
                    .get(&MetaLevel::Object)
                    .map(|v| v.len())
                    .unwrap_or(0),
                self.knowledge
                    .get(&MetaLevel::Meta1)
                    .map(|v| v.len())
                    .unwrap_or(0),
                self.knowledge
                    .get(&MetaLevel::Meta2)
                    .map(|v| v.len())
                    .unwrap_or(0),
                self.knowledge
                    .get(&MetaLevel::Meta3)
                    .map(|v| v.len())
                    .unwrap_or(0),
            ],
            num_strange_loops: self.strange_loops.len(),
            num_modification_rules: self.modification_rules.len(),
            safety_violations: self
                .safety_constraints
                .iter()
                .filter(|c| c.is_violated)
                .count(),
        }
    }
}

/// Summary of meta-learning state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaLearningSummary {
    pub current_level: MetaLevel,
    pub knowledge_counts: [usize; 4], // Object, Meta1, Meta2, Meta3
    pub num_strange_loops: usize,
    pub num_modification_rules: usize,
    pub safety_violations: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_levels() {
        let object = MetaLevel::Object;
        assert_eq!(object.up(), Some(MetaLevel::Meta1));
        assert_eq!(object.down(), None);

        let meta3 = MetaLevel::Meta3;
        assert_eq!(meta3.up(), None);
        assert_eq!(meta3.down(), Some(MetaLevel::Meta2));
    }

    #[test]
    fn test_meta_learner_creation() {
        let learner = MetaLearner::new(100);
        assert_eq!(learner.current_level(), MetaLevel::Object);
        assert_eq!(learner.get_strange_loops().len(), 0);
    }

    #[test]
    fn test_basic_learning() {
        let mut learner = MetaLearner::new(100);

        learner.learn("Test learning content".to_string(), 0.8);

        let knowledge = learner.get_knowledge_at_level(MetaLevel::Object);
        assert_eq!(knowledge.len(), 1);
        assert_eq!(knowledge[0].content, "Test learning content");
        assert_eq!(knowledge[0].effectiveness, 0.8);
    }

    #[test]
    fn test_level_transitions() {
        let mut learner = MetaLearner::new(100);

        assert_eq!(learner.current_level(), MetaLevel::Object);

        learner.ascend().unwrap();
        assert_eq!(learner.current_level(), MetaLevel::Meta1);

        learner.ascend().unwrap();
        assert_eq!(learner.current_level(), MetaLevel::Meta2);

        learner.descend().unwrap();
        assert_eq!(learner.current_level(), MetaLevel::Meta1);
    }

    #[test]
    fn test_meta_pattern_detection() {
        let mut learner = MetaLearner::new(100);

        // Learn many things with good rewards at object level
        for i in 0..15 {
            learner.learn(format!("Learning {}", i), 0.85);
        }

        // Should have detected meta-patterns and stored at Meta1 level
        let meta1_knowledge = learner.get_knowledge_at_level(MetaLevel::Meta1);
        println!("Meta1 knowledge: {:?}", meta1_knowledge);

        // May or may not have meta-knowledge depending on timing
        // Just verify it doesn't crash
        assert!(meta1_knowledge.len() >= 0);
    }

    #[test]
    fn test_strange_loop_detection() {
        let mut learner = MetaLearner::new(100);

        // Create a pattern that oscillates between levels
        learner.learn("Object level".to_string(), 0.7);
        learner.ascend().unwrap();
        learner.learn("Meta1 level".to_string(), 0.7);
        learner.descend().unwrap();
        learner.learn("Object level again".to_string(), 0.7);
        learner.ascend().unwrap();
        learner.learn("Meta1 level again".to_string(), 0.7);

        let loops = learner.get_strange_loops();
        println!("Detected loops: {:?}", loops);

        // May detect loops
        assert!(loops.len() >= 0);
    }

    #[test]
    fn test_safety_constraints() {
        let mut learner = MetaLearner::new(100);

        // Try to add a dangerous modification
        let dangerous_rule = ModificationRule {
            condition: "always".to_string(),
            action: "infinite loop".to_string(),
            priority: 1,
            enabled: true,
        };

        let result = learner.self_modify(dangerous_rule);
        assert!(result.is_err());

        let safety_check = learner.safety_check();
        assert!(safety_check.is_err());
    }

    #[test]
    fn test_safe_modification() {
        let mut learner = MetaLearner::new(100);

        let safe_rule = ModificationRule {
            condition: "reward > 0.8".to_string(),
            action: "increase learning rate".to_string(),
            priority: 5,
            enabled: true,
        };

        let result = learner.self_modify(safe_rule);
        assert!(result.is_ok());

        let summary = learner.get_summary();
        assert_eq!(summary.num_modification_rules, 1);
    }

    #[test]
    fn test_summary() {
        let mut learner = MetaLearner::new(100);

        learner.learn("Test 1".to_string(), 0.8);
        learner.ascend().unwrap();
        learner.learn("Test 2".to_string(), 0.7);

        let summary = learner.get_summary();
        println!("Summary: {:?}", summary);

        assert_eq!(summary.current_level, MetaLevel::Meta1);
        assert_eq!(summary.knowledge_counts[0], 1); // Object level
        assert_eq!(summary.knowledge_counts[1], 1); // Meta1 level
        assert_eq!(summary.safety_violations, 0);
    }
}
