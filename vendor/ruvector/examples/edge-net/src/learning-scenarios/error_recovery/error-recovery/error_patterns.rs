//! Error Pattern Learning Module
//!
//! This module intentionally includes patterns that might cause errors
//! to teach the self-learning system about error recovery strategies.

use std::collections::HashMap;

/// Error pattern types for learning
#[derive(Debug, Clone)]
pub enum ErrorPattern {
    /// Type mismatch errors (E0308)
    TypeMismatch { expected: String, found: String },
    /// Unresolved import errors (E0433)
    UnresolvedImport { path: String },
    /// Borrow checker errors (E0502)
    BorrowConflict { variable: String },
    /// Missing trait implementation (E0277)
    MissingTrait { trait_name: String, type_name: String },
}

/// Recovery strategy for each error type
#[derive(Debug, Clone)]
pub struct RecoveryStrategy {
    pub error_code: String,
    pub description: String,
    pub fix_steps: Vec<String>,
    pub suggested_agent: String,
}

impl RecoveryStrategy {
    pub fn for_error(pattern: &ErrorPattern) -> Self {
        match pattern {
            ErrorPattern::TypeMismatch { expected, found } => Self {
                error_code: "E0308".into(),
                description: format!("Expected {}, found {}", expected, found),
                fix_steps: vec![
                    "Check variable type annotations".into(),
                    "Add explicit type conversion".into(),
                    "Use .into() or .as_ref() as needed".into(),
                ],
                suggested_agent: "rust-developer".into(),
            },
            ErrorPattern::UnresolvedImport { path } => Self {
                error_code: "E0433".into(),
                description: format!("Failed to resolve: {}", path),
                fix_steps: vec![
                    "Add missing dependency to Cargo.toml".into(),
                    "Check module path spelling".into(),
                    "Ensure pub visibility".into(),
                ],
                suggested_agent: "rust-developer".into(),
            },
            ErrorPattern::BorrowConflict { variable } => Self {
                error_code: "E0502".into(),
                description: format!("Borrow conflict on {}", variable),
                fix_steps: vec![
                    "Clone the value if ownership is needed".into(),
                    "Use RefCell for interior mutability".into(),
                    "Restructure code to limit borrow scope".into(),
                ],
                suggested_agent: "rust-developer".into(),
            },
            ErrorPattern::MissingTrait { trait_name, type_name } => Self {
                error_code: "E0277".into(),
                description: format!("{} not implemented for {}", trait_name, type_name),
                fix_steps: vec![
                    "Derive the trait if possible".into(),
                    "Implement the trait manually".into(),
                    "Use a wrapper type that implements it".into(),
                ],
                suggested_agent: "rust-developer".into(),
            },
        }
    }
}

/// Learning tracker for error patterns
pub struct ErrorLearningTracker {
    patterns: HashMap<String, u32>,
    recoveries: HashMap<String, Vec<RecoveryStrategy>>,
}

impl ErrorLearningTracker {
    pub fn new() -> Self {
        Self {
            patterns: HashMap::new(),
            recoveries: HashMap::new(),
        }
    }

    /// Record an error occurrence for learning
    pub fn record_error(&mut self, error_code: &str) {
        *self.patterns.entry(error_code.to_string()).or_insert(0) += 1;
    }

    /// Record a successful recovery for learning
    pub fn record_recovery(&mut self, error_code: &str, strategy: RecoveryStrategy) {
        self.recoveries
            .entry(error_code.to_string())
            .or_default()
            .push(strategy);
    }

    /// Get the most successful recovery strategy for an error
    pub fn best_recovery(&self, error_code: &str) -> Option<&RecoveryStrategy> {
        self.recoveries.get(error_code).and_then(|v| v.last())
    }
}

impl Default for ErrorLearningTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_strategy_for_type_mismatch() {
        let pattern = ErrorPattern::TypeMismatch {
            expected: "u32".into(),
            found: "i32".into(),
        };
        let strategy = RecoveryStrategy::for_error(&pattern);
        assert_eq!(strategy.error_code, "E0308");
        assert_eq!(strategy.suggested_agent, "rust-developer");
    }

    #[test]
    fn test_error_learning_tracker() {
        let mut tracker = ErrorLearningTracker::new();
        tracker.record_error("E0308");
        tracker.record_error("E0308");

        assert_eq!(tracker.patterns.get("E0308"), Some(&2));
    }
}
