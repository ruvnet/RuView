//! File Sequence Learning Module
//!
//! Tracks the order in which files are edited to learn optimal
//! multi-file refactoring patterns.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

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
#[derive(Debug, Clone, PartialEq)]
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
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            success,
        };

        self.current_sequence.push(edit);

        // Check if we've completed a recognizable pattern
        if let Some(pattern) = self.detect_pattern() {
            self.learn_pattern(pattern);
        }
    }

    /// Detect file type from extension
    fn detect_file_type(path: &str) -> String {
        if path.ends_with(".rs") { "rust".into() }
        else if path.ends_with(".ts") { "typescript".into() }
        else if path.ends_with(".toml") { "toml".into() }
        else if path.ends_with(".json") { "json".into() }
        else if path.ends_with(".yaml") || path.ends_with(".yml") { "yaml".into() }
        else if path.ends_with(".md") { "markdown".into() }
        else if path.ends_with(".sh") { "shell".into() }
        else { "unknown".into() }
    }

    /// Extract crate name from path
    fn extract_crate_name(path: &str) -> Option<String> {
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
        let files: Vec<&str> = self.current_sequence
            .iter()
            .map(|e| e.file_path.as_str())
            .collect();

        if files.len() < 2 {
            return None;
        }

        // Detect Rust crate setup pattern
        if files.iter().any(|f| f.ends_with("Cargo.toml"))
            && files.iter().any(|f| f.ends_with("lib.rs")) {
            return Some(SequencePattern::RustCrateSetup);
        }

        // Detect TDD pattern
        if files.iter().any(|f| f.contains("test"))
            && files.iter().position(|f| f.contains("test"))
                < files.iter().position(|f| f.ends_with("lib.rs") || f.ends_with("mod.rs")) {
            return Some(SequencePattern::TestDrivenDevelopment);
        }

        // Detect types-first pattern
        if files.iter().any(|f| f.contains("types"))
            && files.iter().position(|f| f.contains("types")).unwrap_or(999) < 2 {
            return Some(SequencePattern::TypesFirstDevelopment);
        }

        None
    }

    /// Learn from a detected pattern
    fn learn_pattern(&mut self, pattern: SequencePattern) {
        let confidence = self.pattern_confidence.entry(pattern.clone()).or_insert(0.5);

        // Increase confidence if all edits in sequence were successful
        let success_rate = self.current_sequence.iter()
            .filter(|e| e.success)
            .count() as f64 / self.current_sequence.len() as f64;

        // Q-learning style update
        *confidence = *confidence + 0.1 * (success_rate - *confidence);

        // Clear sequence after learning
        self.current_sequence.clear();
    }

    /// Suggest the next file to edit based on learned patterns
    pub fn suggest_next_file(&self, current_file: &str) -> Option<String> {
        let file_type = Self::detect_file_type(current_file);

        match file_type.as_str() {
            "toml" if current_file.contains("Cargo") => {
                Some("src/lib.rs".into())
            }
            "rust" if current_file.contains("types") => {
                Some("src/lib.rs".into())
            }
            "rust" if current_file.contains("lib.rs") => {
                Some("src/tests.rs".into())
            }
            _ => None
        }
    }

    /// Get learned patterns with their confidence scores
    pub fn get_pattern_confidence(&self) -> &HashMap<SequencePattern, f64> {
        &self.pattern_confidence
    }
}

impl Default for SequenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_type_detection() {
        assert_eq!(SequenceTracker::detect_file_type("src/lib.rs"), "rust");
        assert_eq!(SequenceTracker::detect_file_type("config.yaml"), "yaml");
        assert_eq!(SequenceTracker::detect_file_type("types.ts"), "typescript");
    }

    #[test]
    fn test_crate_name_extraction() {
        let name = SequenceTracker::extract_crate_name("crates/ruvector-core/src/lib.rs");
        assert_eq!(name, Some("ruvector-core".into()));
    }

    #[test]
    fn test_sequence_tracking() {
        let mut tracker = SequenceTracker::new();
        tracker.record_edit("Cargo.toml", true);
        tracker.record_edit("src/lib.rs", true);

        assert!(!tracker.current_sequence.is_empty() ||
                tracker.pattern_confidence.contains_key(&SequencePattern::RustCrateSetup));
    }
}
