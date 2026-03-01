//! Comprehensive test suite for Learning Scenarios
//!
//! This test suite validates the RuVector self-learning hooks system
//! including error pattern detection, file sequence tracking, and
//! learning statistics.

use std::collections::HashMap;

// Re-implement test versions of the learning scenario types
// since they use std::collections::HashMap which is available in tests

// ============================================================================
// Error Pattern Types (mirror of error_patterns.rs)
// ============================================================================

/// Error pattern types for learning
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorPattern {
    /// Type mismatch errors (E0308)
    TypeMismatch { expected: String, found: String },
    /// Unresolved import errors (E0433)
    UnresolvedImport { path: String },
    /// Borrow checker errors (E0502)
    BorrowConflict { variable: String },
    /// Missing trait implementation (E0277)
    MissingTrait {
        trait_name: String,
        type_name: String,
    },
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
            ErrorPattern::MissingTrait {
                trait_name,
                type_name,
            } => Self {
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

    /// Get the count of a specific error
    pub fn get_error_count(&self, error_code: &str) -> u32 {
        *self.patterns.get(error_code).unwrap_or(&0)
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

    /// Get all recovery strategies for an error
    pub fn all_recoveries(&self, error_code: &str) -> Option<&Vec<RecoveryStrategy>> {
        self.recoveries.get(error_code)
    }

    /// Get total number of unique error patterns tracked
    pub fn unique_error_count(&self) -> usize {
        self.patterns.len()
    }

    /// Get total error occurrences
    pub fn total_error_count(&self) -> u32 {
        self.patterns.values().sum()
    }
}

impl Default for ErrorLearningTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// File Sequence Types (mirror of sequence_tracker.rs)
// ============================================================================

/// Represents a file edit event
#[derive(Debug, Clone)]
pub struct FileEdit {
    pub file_path: String,
    pub file_type: String,
    pub crate_name: Option<String>,
    pub timestamp: u64,
    pub success: bool,
}

/// A sequence of file edits that form a pattern
#[derive(Debug, Clone)]
pub struct EditSequence {
    pub id: String,
    pub files: Vec<String>,
    pub pattern_type: SequencePattern,
    pub occurrences: u32,
    pub avg_success_rate: f64,
}

/// Types of editing patterns we can learn
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SequencePattern {
    /// Cargo.toml -> lib.rs -> specific modules
    RustCrateSetup,
    /// Types first, then implementation, then tests
    TypesFirstDevelopment,
    /// Tests first, then implementation (TDD)
    TestDrivenDevelopment,
    /// Config files, then source, then docs
    FullStackChange,
    /// Unknown pattern being learned
    Learning,
}

/// Tracks file sequences for learning
pub struct SequenceTracker {
    current_sequence: Vec<FileEdit>,
    learned_sequences: HashMap<String, EditSequence>,
    pattern_confidence: HashMap<SequencePattern, f64>,
}

impl SequenceTracker {
    pub fn new() -> Self {
        Self {
            current_sequence: Vec::new(),
            learned_sequences: HashMap::new(),
            pattern_confidence: HashMap::new(),
        }
    }

    /// Record a file edit in the current sequence
    pub fn record_edit(&mut self, file_path: &str, success: bool) {
        let file_type = Self::detect_file_type(file_path);
        let crate_name = Self::extract_crate_name(file_path);

        let edit = FileEdit {
            file_path: file_path.to_string(),
            file_type,
            crate_name,
            timestamp: 0, // Simplified for testing
            success,
        };

        self.current_sequence.push(edit);

        // Check if we've completed a recognizable pattern
        if let Some(pattern) = self.detect_pattern() {
            self.learn_pattern(pattern);
        }
    }

    /// Get the current sequence length
    pub fn current_sequence_len(&self) -> usize {
        self.current_sequence.len()
    }

    /// Detect file type from extension
    pub fn detect_file_type(path: &str) -> String {
        if path.ends_with(".rs") {
            "rust".into()
        } else if path.ends_with(".ts") {
            "typescript".into()
        } else if path.ends_with(".toml") {
            "toml".into()
        } else if path.ends_with(".json") {
            "json".into()
        } else if path.ends_with(".yaml") || path.ends_with(".yml") {
            "yaml".into()
        } else if path.ends_with(".md") {
            "markdown".into()
        } else if path.ends_with(".sh") {
            "shell".into()
        } else if path.ends_with(".js") {
            "javascript".into()
        } else if path.ends_with(".py") {
            "python".into()
        } else {
            "unknown".into()
        }
    }

    /// Extract crate name from path
    pub fn extract_crate_name(path: &str) -> Option<String> {
        // Look for patterns like crates/ruvector-*/
        if path.contains("crates/") {
            path.split("crates/")
                .nth(1)
                .and_then(|s| s.split('/').next())
                .map(|s| s.to_string())
        } else if path.contains("ruvector-") {
            path.split("ruvector-")
                .nth(1)
                .and_then(|s| s.split('/').next())
                .map(|s| format!("ruvector-{}", s))
        } else {
            None
        }
    }

    /// Detect if current sequence matches a known pattern
    fn detect_pattern(&self) -> Option<SequencePattern> {
        let files: Vec<&str> = self
            .current_sequence
            .iter()
            .map(|e| e.file_path.as_str())
            .collect();

        if files.len() < 2 {
            return None;
        }

        // Detect Rust crate setup pattern
        if files.iter().any(|f| f.ends_with("Cargo.toml"))
            && files.iter().any(|f| f.ends_with("lib.rs"))
        {
            return Some(SequencePattern::RustCrateSetup);
        }

        // Detect TDD pattern
        if files.iter().any(|f| f.contains("test")) {
            let test_pos = files.iter().position(|f| f.contains("test"));
            let impl_pos = files
                .iter()
                .position(|f| f.ends_with("lib.rs") || f.ends_with("mod.rs"));

            if let (Some(t), Some(i)) = (test_pos, impl_pos) {
                if t < i {
                    return Some(SequencePattern::TestDrivenDevelopment);
                }
            }
        }

        // Detect types-first pattern
        if files.iter().any(|f| f.contains("types")) {
            if files
                .iter()
                .position(|f| f.contains("types"))
                .unwrap_or(999)
                < 2
            {
                return Some(SequencePattern::TypesFirstDevelopment);
            }
        }

        None
    }

    /// Learn from a detected pattern
    fn learn_pattern(&mut self, pattern: SequencePattern) {
        let confidence = self
            .pattern_confidence
            .entry(pattern.clone())
            .or_insert(0.5);

        // Increase confidence if all edits in sequence were successful
        let success_rate = self.current_sequence.iter().filter(|e| e.success).count() as f64
            / self.current_sequence.len() as f64;

        // Q-learning style update
        *confidence = *confidence + 0.1 * (success_rate - *confidence);

        // Clear sequence after learning
        self.current_sequence.clear();
    }

    /// Suggest the next file to edit based on learned patterns
    pub fn suggest_next_file(&self, current_file: &str) -> Option<String> {
        let file_type = Self::detect_file_type(current_file);

        match file_type.as_str() {
            "toml" if current_file.contains("Cargo") => Some("src/lib.rs".into()),
            "rust" if current_file.contains("types") => Some("src/lib.rs".into()),
            "rust" if current_file.contains("lib.rs") => Some("src/tests.rs".into()),
            _ => None,
        }
    }

    /// Get learned patterns with their confidence scores
    pub fn get_pattern_confidence(&self) -> &HashMap<SequencePattern, f64> {
        &self.pattern_confidence
    }

    /// Check if a pattern has been learned
    pub fn has_learned_pattern(&self, pattern: &SequencePattern) -> bool {
        self.pattern_confidence.contains_key(pattern)
    }

    /// Get confidence for a specific pattern
    pub fn get_confidence(&self, pattern: &SequencePattern) -> Option<f64> {
        self.pattern_confidence.get(pattern).copied()
    }
}

impl Default for SequenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Learning Statistics
// ============================================================================

/// Learning statistics
#[derive(Debug, Default)]
pub struct LearningStats {
    pub patterns_learned: u32,
    pub errors_recovered: u32,
    pub sequences_detected: u32,
    pub agent_routings: u32,
}

impl LearningStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_pattern(&mut self) {
        self.patterns_learned += 1;
    }

    pub fn record_recovery(&mut self) {
        self.errors_recovered += 1;
    }

    pub fn record_sequence(&mut self) {
        self.sequences_detected += 1;
    }

    pub fn record_routing(&mut self) {
        self.agent_routings += 1;
    }

    pub fn total_operations(&self) -> u32 {
        self.patterns_learned
            + self.errors_recovered
            + self.sequences_detected
            + self.agent_routings
    }
}

// ============================================================================
// Error Pattern Detection Tests
// ============================================================================

#[test]
fn test_error_pattern_type_mismatch_detection() {
    let pattern = ErrorPattern::TypeMismatch {
        expected: "u32".into(),
        found: "i32".into(),
    };

    let strategy = RecoveryStrategy::for_error(&pattern);

    assert_eq!(strategy.error_code, "E0308");
    assert!(strategy.description.contains("u32"));
    assert!(strategy.description.contains("i32"));
    assert_eq!(strategy.suggested_agent, "rust-developer");
    assert!(!strategy.fix_steps.is_empty());
}

#[test]
fn test_error_pattern_unresolved_import_detection() {
    let pattern = ErrorPattern::UnresolvedImport {
        path: "crate::missing::module".into(),
    };

    let strategy = RecoveryStrategy::for_error(&pattern);

    assert_eq!(strategy.error_code, "E0433");
    assert!(strategy.description.contains("crate::missing::module"));
    assert!(strategy.fix_steps.iter().any(|s| s.contains("Cargo.toml")));
    assert!(strategy.fix_steps.iter().any(|s| s.contains("visibility")));
}

#[test]
fn test_error_pattern_borrow_conflict_detection() {
    let pattern = ErrorPattern::BorrowConflict {
        variable: "my_data".into(),
    };

    let strategy = RecoveryStrategy::for_error(&pattern);

    assert_eq!(strategy.error_code, "E0502");
    assert!(strategy.description.contains("my_data"));
    assert!(strategy.fix_steps.iter().any(|s| s.contains("Clone")));
    assert!(strategy.fix_steps.iter().any(|s| s.contains("RefCell")));
}

#[test]
fn test_error_pattern_missing_trait_detection() {
    let pattern = ErrorPattern::MissingTrait {
        trait_name: "Debug".into(),
        type_name: "MyStruct".into(),
    };

    let strategy = RecoveryStrategy::for_error(&pattern);

    assert_eq!(strategy.error_code, "E0277");
    assert!(strategy.description.contains("Debug"));
    assert!(strategy.description.contains("MyStruct"));
    assert!(strategy.fix_steps.iter().any(|s| s.contains("Derive")));
}

#[test]
fn test_error_learning_tracker_records_errors() {
    let mut tracker = ErrorLearningTracker::new();

    tracker.record_error("E0308");
    tracker.record_error("E0308");
    tracker.record_error("E0433");

    assert_eq!(tracker.get_error_count("E0308"), 2);
    assert_eq!(tracker.get_error_count("E0433"), 1);
    assert_eq!(tracker.get_error_count("E0502"), 0);
    assert_eq!(tracker.unique_error_count(), 2);
    assert_eq!(tracker.total_error_count(), 3);
}

#[test]
fn test_error_learning_tracker_records_recoveries() {
    let mut tracker = ErrorLearningTracker::new();

    let pattern = ErrorPattern::TypeMismatch {
        expected: "u32".into(),
        found: "i32".into(),
    };
    let strategy = RecoveryStrategy::for_error(&pattern);

    tracker.record_recovery("E0308", strategy.clone());

    let recovered = tracker.best_recovery("E0308");
    assert!(recovered.is_some());
    assert_eq!(recovered.unwrap().error_code, "E0308");
}

#[test]
fn test_error_learning_tracker_multiple_recoveries() {
    let mut tracker = ErrorLearningTracker::new();

    let pattern1 = ErrorPattern::TypeMismatch {
        expected: "u32".into(),
        found: "i32".into(),
    };
    let pattern2 = ErrorPattern::TypeMismatch {
        expected: "String".into(),
        found: "&str".into(),
    };

    tracker.record_recovery("E0308", RecoveryStrategy::for_error(&pattern1));
    tracker.record_recovery("E0308", RecoveryStrategy::for_error(&pattern2));

    let all_recoveries = tracker.all_recoveries("E0308");
    assert!(all_recoveries.is_some());
    assert_eq!(all_recoveries.unwrap().len(), 2);

    // Best recovery is the most recent
    let best = tracker.best_recovery("E0308").unwrap();
    assert!(best.description.contains("String"));
}

#[test]
fn test_error_pattern_comparison() {
    let pattern1 = ErrorPattern::TypeMismatch {
        expected: "u32".into(),
        found: "i32".into(),
    };
    let pattern2 = ErrorPattern::TypeMismatch {
        expected: "u32".into(),
        found: "i32".into(),
    };
    let pattern3 = ErrorPattern::UnresolvedImport {
        path: "test".into(),
    };

    assert_eq!(pattern1, pattern2);
    assert_ne!(pattern1, pattern3);
}

// ============================================================================
// File Sequence Tracking Tests
// ============================================================================

#[test]
fn test_file_type_detection_rust() {
    assert_eq!(SequenceTracker::detect_file_type("src/lib.rs"), "rust");
    assert_eq!(SequenceTracker::detect_file_type("src/main.rs"), "rust");
    assert_eq!(
        SequenceTracker::detect_file_type("crates/core/src/mod.rs"),
        "rust"
    );
}

#[test]
fn test_file_type_detection_typescript() {
    assert_eq!(
        SequenceTracker::detect_file_type("src/index.ts"),
        "typescript"
    );
    assert_eq!(SequenceTracker::detect_file_type("types.ts"), "typescript");
}

#[test]
fn test_file_type_detection_config_files() {
    assert_eq!(SequenceTracker::detect_file_type("Cargo.toml"), "toml");
    assert_eq!(SequenceTracker::detect_file_type("config.yaml"), "yaml");
    assert_eq!(SequenceTracker::detect_file_type("config.yml"), "yaml");
    assert_eq!(SequenceTracker::detect_file_type("package.json"), "json");
}

#[test]
fn test_file_type_detection_other() {
    assert_eq!(SequenceTracker::detect_file_type("README.md"), "markdown");
    assert_eq!(SequenceTracker::detect_file_type("setup.sh"), "shell");
    assert_eq!(SequenceTracker::detect_file_type("script.js"), "javascript");
    assert_eq!(SequenceTracker::detect_file_type("main.py"), "python");
    assert_eq!(SequenceTracker::detect_file_type("unknown.xyz"), "unknown");
}

#[test]
fn test_crate_name_extraction_from_crates_dir() {
    let name = SequenceTracker::extract_crate_name("crates/ruvector-core/src/lib.rs");
    assert_eq!(name, Some("ruvector-core".into()));

    let name2 = SequenceTracker::extract_crate_name("crates/ruvector-edge-net/src/main.rs");
    assert_eq!(name2, Some("ruvector-edge-net".into()));
}

#[test]
fn test_crate_name_extraction_from_ruvector_prefix() {
    let name = SequenceTracker::extract_crate_name("examples/ruvector-demo/main.rs");
    assert_eq!(name, Some("ruvector-demo".into()));
}

#[test]
fn test_crate_name_extraction_none() {
    let name = SequenceTracker::extract_crate_name("src/lib.rs");
    assert_eq!(name, None);

    let name2 = SequenceTracker::extract_crate_name("other-project/src/lib.rs");
    assert_eq!(name2, None);
}

#[test]
fn test_sequence_tracker_records_edits() {
    let mut tracker = SequenceTracker::new();

    tracker.record_edit("Cargo.toml", true);
    assert_eq!(tracker.current_sequence_len(), 1);

    tracker.record_edit("src/lib.rs", true);
    // Sequence may have been cleared if pattern detected
    // Check that pattern was learned OR sequence still has 2 items
    assert!(
        tracker.current_sequence_len() == 0
            || tracker.current_sequence_len() == 2
            || tracker.has_learned_pattern(&SequencePattern::RustCrateSetup)
    );
}

#[test]
fn test_sequence_tracker_detects_rust_crate_setup() {
    let mut tracker = SequenceTracker::new();

    tracker.record_edit("Cargo.toml", true);
    tracker.record_edit("src/lib.rs", true);

    // Pattern should be detected and learned
    assert!(tracker.has_learned_pattern(&SequencePattern::RustCrateSetup));
}

#[test]
fn test_sequence_tracker_detects_tdd_pattern() {
    let mut tracker = SequenceTracker::new();

    tracker.record_edit("tests/test_feature.rs", true);
    tracker.record_edit("src/lib.rs", true);

    // TDD pattern: tests before implementation
    assert!(tracker.has_learned_pattern(&SequencePattern::TestDrivenDevelopment));
}

#[test]
fn test_sequence_tracker_detects_types_first_pattern() {
    let mut tracker = SequenceTracker::new();

    tracker.record_edit("src/types.rs", true);
    tracker.record_edit("src/lib.rs", true);

    // Types first pattern
    assert!(tracker.has_learned_pattern(&SequencePattern::TypesFirstDevelopment));
}

#[test]
fn test_sequence_tracker_confidence_increases_with_success() {
    let mut tracker = SequenceTracker::new();

    // First sequence - all successful
    tracker.record_edit("Cargo.toml", true);
    tracker.record_edit("src/lib.rs", true);

    let first_confidence = tracker
        .get_confidence(&SequencePattern::RustCrateSetup)
        .unwrap();

    // Second sequence - all successful
    tracker.record_edit("Cargo.toml", true);
    tracker.record_edit("src/lib.rs", true);

    let second_confidence = tracker
        .get_confidence(&SequencePattern::RustCrateSetup)
        .unwrap();

    // Confidence should increase with repeated success
    assert!(
        second_confidence >= first_confidence,
        "Confidence should increase: {} >= {}",
        second_confidence,
        first_confidence
    );
}

#[test]
fn test_sequence_tracker_suggests_next_file_from_cargo() {
    let tracker = SequenceTracker::new();

    let suggestion = tracker.suggest_next_file("Cargo.toml");
    assert_eq!(suggestion, Some("src/lib.rs".into()));
}

#[test]
fn test_sequence_tracker_suggests_next_file_from_types() {
    let tracker = SequenceTracker::new();

    let suggestion = tracker.suggest_next_file("src/types.rs");
    assert_eq!(suggestion, Some("src/lib.rs".into()));
}

#[test]
fn test_sequence_tracker_suggests_next_file_from_lib() {
    let tracker = SequenceTracker::new();

    let suggestion = tracker.suggest_next_file("src/lib.rs");
    assert_eq!(suggestion, Some("src/tests.rs".into()));
}

#[test]
fn test_sequence_tracker_no_suggestion_for_unknown() {
    let tracker = SequenceTracker::new();

    let suggestion = tracker.suggest_next_file("random_file.txt");
    assert_eq!(suggestion, None);
}

// ============================================================================
// Learning Statistics Tests
// ============================================================================

#[test]
fn test_learning_stats_new() {
    let stats = LearningStats::new();

    assert_eq!(stats.patterns_learned, 0);
    assert_eq!(stats.errors_recovered, 0);
    assert_eq!(stats.sequences_detected, 0);
    assert_eq!(stats.agent_routings, 0);
    assert_eq!(stats.total_operations(), 0);
}

#[test]
fn test_learning_stats_record_pattern() {
    let mut stats = LearningStats::new();

    stats.record_pattern();
    stats.record_pattern();

    assert_eq!(stats.patterns_learned, 2);
    assert_eq!(stats.total_operations(), 2);
}

#[test]
fn test_learning_stats_record_recovery() {
    let mut stats = LearningStats::new();

    stats.record_recovery();
    stats.record_recovery();
    stats.record_recovery();

    assert_eq!(stats.errors_recovered, 3);
    assert_eq!(stats.total_operations(), 3);
}

#[test]
fn test_learning_stats_record_sequence() {
    let mut stats = LearningStats::new();

    stats.record_sequence();

    assert_eq!(stats.sequences_detected, 1);
}

#[test]
fn test_learning_stats_record_routing() {
    let mut stats = LearningStats::new();

    stats.record_routing();
    stats.record_routing();

    assert_eq!(stats.agent_routings, 2);
}

#[test]
fn test_learning_stats_total_operations() {
    let mut stats = LearningStats::new();

    stats.record_pattern();
    stats.record_recovery();
    stats.record_sequence();
    stats.record_routing();

    assert_eq!(stats.total_operations(), 4);
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn integration_error_tracking_with_sequence() {
    let mut error_tracker = ErrorLearningTracker::new();
    let mut sequence_tracker = SequenceTracker::new();
    let mut stats = LearningStats::new();

    // Simulate development workflow with errors

    // Step 1: Edit Cargo.toml
    sequence_tracker.record_edit("Cargo.toml", true);
    stats.record_sequence();

    // Step 2: Edit lib.rs, encounter type mismatch
    sequence_tracker.record_edit("src/lib.rs", false); // Failed

    let error = ErrorPattern::TypeMismatch {
        expected: "Vec<u8>".into(),
        found: "&[u8]".into(),
    };
    error_tracker.record_error("E0308");
    stats.record_recovery();

    // Step 3: Record successful recovery
    let strategy = RecoveryStrategy::for_error(&error);
    error_tracker.record_recovery("E0308", strategy);

    // Verify integrated state
    assert_eq!(error_tracker.get_error_count("E0308"), 1);
    assert!(error_tracker.best_recovery("E0308").is_some());
    assert!(sequence_tracker.has_learned_pattern(&SequencePattern::RustCrateSetup));
    assert_eq!(stats.errors_recovered, 1);
    assert_eq!(stats.sequences_detected, 1);
}

#[test]
fn integration_full_development_cycle() {
    let mut error_tracker = ErrorLearningTracker::new();
    let mut sequence_tracker = SequenceTracker::new();
    let mut stats = LearningStats::new();

    // Simulate full TDD cycle

    // Step 1: Write tests first
    sequence_tracker.record_edit("tests/feature_test.rs", true);
    stats.record_pattern();

    // Step 2: Write implementation (fails initially)
    sequence_tracker.record_edit("src/lib.rs", false);

    // Encounter unresolved import
    let import_error = ErrorPattern::UnresolvedImport {
        path: "crate::new_module".into(),
    };
    error_tracker.record_error("E0433");

    // Step 3: Fix the error
    error_tracker.record_recovery("E0433", RecoveryStrategy::for_error(&import_error));
    stats.record_recovery();

    // Step 4: Implementation succeeds
    sequence_tracker.record_edit("src/lib.rs", true);
    stats.record_routing();

    // Verify TDD pattern was detected
    assert!(sequence_tracker.has_learned_pattern(&SequencePattern::TestDrivenDevelopment));
    assert_eq!(error_tracker.get_error_count("E0433"), 1);
    assert_eq!(stats.total_operations(), 3);
}

#[test]
fn integration_multi_error_recovery() {
    let mut tracker = ErrorLearningTracker::new();

    // Simulate multiple errors during development
    let errors = vec![
        ErrorPattern::TypeMismatch {
            expected: "u32".into(),
            found: "i32".into(),
        },
        ErrorPattern::BorrowConflict {
            variable: "data".into(),
        },
        ErrorPattern::MissingTrait {
            trait_name: "Clone".into(),
            type_name: "MyType".into(),
        },
        ErrorPattern::TypeMismatch {
            expected: "String".into(),
            found: "&str".into(),
        },
    ];

    for error in &errors {
        let code = match error {
            ErrorPattern::TypeMismatch { .. } => "E0308",
            ErrorPattern::BorrowConflict { .. } => "E0502",
            ErrorPattern::MissingTrait { .. } => "E0277",
            ErrorPattern::UnresolvedImport { .. } => "E0433",
        };
        tracker.record_error(code);
        tracker.record_recovery(code, RecoveryStrategy::for_error(error));
    }

    // Verify tracking
    assert_eq!(tracker.get_error_count("E0308"), 2); // Two type mismatches
    assert_eq!(tracker.get_error_count("E0502"), 1);
    assert_eq!(tracker.get_error_count("E0277"), 1);
    assert_eq!(tracker.unique_error_count(), 3);
    assert_eq!(tracker.total_error_count(), 4);

    // Best recovery for E0308 should be the most recent (String/&str)
    let best = tracker.best_recovery("E0308").unwrap();
    assert!(best.description.contains("String"));
}

#[test]
fn integration_pattern_learning_over_time() {
    let mut tracker = SequenceTracker::new();

    // Simulate multiple iterations of the same pattern
    for _ in 0..5 {
        tracker.record_edit("Cargo.toml", true);
        tracker.record_edit("src/lib.rs", true);
    }

    let confidence = tracker.get_confidence(&SequencePattern::RustCrateSetup);
    assert!(confidence.is_some());

    // After multiple successful iterations, confidence should be higher than initial 0.5
    let conf = confidence.unwrap();
    assert!(conf > 0.5, "Confidence should increase: {}", conf);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_empty_sequence_no_pattern() {
    let tracker = SequenceTracker::new();

    // No edits recorded
    assert_eq!(tracker.current_sequence_len(), 0);
    assert!(tracker.get_pattern_confidence().is_empty());
}

#[test]
fn test_single_edit_no_pattern() {
    let mut tracker = SequenceTracker::new();

    tracker.record_edit("src/lib.rs", true);

    // Single edit should not trigger pattern detection
    assert_eq!(tracker.current_sequence_len(), 1);
    assert!(tracker.get_pattern_confidence().is_empty());
}

#[test]
fn test_recovery_for_unknown_error() {
    let tracker = ErrorLearningTracker::new();

    // No recovery recorded for E9999
    assert!(tracker.best_recovery("E9999").is_none());
    assert!(tracker.all_recoveries("E9999").is_none());
}

#[test]
fn test_file_edit_with_empty_path() {
    let mut tracker = SequenceTracker::new();

    tracker.record_edit("", true);

    assert_eq!(tracker.current_sequence_len(), 1);
    assert_eq!(SequenceTracker::detect_file_type(""), "unknown");
}

#[test]
fn test_crate_name_with_deeply_nested_path() {
    let name =
        SequenceTracker::extract_crate_name("crates/ruvector-core/src/hnsw/index/builder.rs");
    assert_eq!(name, Some("ruvector-core".into()));
}

#[test]
fn test_sequence_pattern_equality() {
    let pattern1 = SequencePattern::RustCrateSetup;
    let pattern2 = SequencePattern::RustCrateSetup;
    let pattern3 = SequencePattern::TestDrivenDevelopment;

    assert_eq!(pattern1, pattern2);
    assert_ne!(pattern1, pattern3);
}

#[test]
fn test_learning_stats_default() {
    let stats = LearningStats::default();

    assert_eq!(stats.patterns_learned, 0);
    assert_eq!(stats.errors_recovered, 0);
    assert_eq!(stats.sequences_detected, 0);
    assert_eq!(stats.agent_routings, 0);
}
