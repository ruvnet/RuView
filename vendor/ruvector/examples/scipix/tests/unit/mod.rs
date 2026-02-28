// Unit test module organization for ruvector-scipix
//
// This module organizes all unit tests following Rust testing best practices.
// Each submodule tests a specific component in isolation with comprehensive coverage.

/// Configuration tests - Test config loading, validation, defaults
pub mod config_tests;

/// Error handling tests - Test error types, conversions, display
pub mod error_tests;

/// Preprocessing tests - Test image preprocessing pipeline
pub mod preprocess_tests;

/// Math parsing tests - Test mathematical expression parsing and recognition
pub mod math_tests;

/// Output formatting tests - Test LaTeX, MathML, and other format generation
pub mod output_tests;

/// OCR engine tests - Test OCR model loading and inference
pub mod ocr_tests;

#[cfg(test)]
mod common {
    use std::path::PathBuf;

    /// Get path to test fixtures directory
    pub fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
    }

    /// Get path to a specific test fixture
    pub fn fixture_path(name: &str) -> PathBuf {
        fixtures_dir().join(name)
    }

    /// Check if a fixture exists
    pub fn has_fixture(name: &str) -> bool {
        fixture_path(name).exists()
    }

    /// Normalize LaTeX string for comparison (remove whitespace)
    pub fn normalize_latex(latex: &str) -> String {
        latex
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_lowercase()
    }

    /// Calculate simple string similarity (0.0 to 1.0)
    pub fn string_similarity(a: &str, b: &str) -> f64 {
        if a == b {
            return 1.0;
        }
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }

        let max_len = a.len().max(b.len());
        let matching = a.chars().zip(b.chars()).filter(|(x, y)| x == y).count();

        matching as f64 / max_len as f64
    }
}
