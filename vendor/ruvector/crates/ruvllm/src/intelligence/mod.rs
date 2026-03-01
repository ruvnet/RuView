//! External Intelligence Providers for SONA Learning
//!
//! This module provides a trait-based extension point for external systems
//! to feed quality signals into RuvLLM's learning loops (SONA, embedding
//! classifier, model router calibration).
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────┐     ┌──────────────────────┐
//! │ External System     │     │ IntelligenceLoader   │
//! │ (CI/CD, workflow)   │────>│ ├── providers[]      │
//! │                     │     │ ├── load_all_signals()│
//! └─────────────────────┘     │ └── ingest()         │
//!                             └──────────┬───────────┘
//!                                        │
//!              ┌─────────────────────────┼──────────────┐
//!              │                         │              │
//!              v                         v              v
//!     ┌────────────────┐     ┌──────────────────┐  ┌────────────┐
//!     │ SONA Loop      │     │ Embedding        │  │ Model      │
//!     │ (trajectories) │     │ Classifier       │  │ Router     │
//!     └────────────────┘     └──────────────────┘  └────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ruvllm::intelligence::{IntelligenceLoader, FileSignalProvider};
//! use std::path::PathBuf;
//!
//! // Create loader and register providers
//! let mut loader = IntelligenceLoader::new();
//! loader.register_provider(Box::new(
//!     FileSignalProvider::new(PathBuf::from(".claude/intelligence/data/signals.json"))
//! ));
//!
//! // Load all signals from registered providers
//! let signals = loader.load_all_signals();
//! println!("Loaded {} signals from {} providers", signals.len(), loader.provider_count());
//! ```

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Maximum signal file size (10 MiB)
const MAX_SIGNAL_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Maximum number of signals per file
const MAX_SIGNALS_PER_FILE: usize = 10_000;

/// Execution outcome for a task signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Task completed successfully
    Success,
    /// Task partially completed
    PartialSuccess,
    /// Task failed
    Failure,
}

/// Human review verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanVerdict {
    /// Human approved the output
    Approved,
    /// Human rejected the output
    Rejected,
}

/// A quality signal from an external system.
///
/// Represents one completed task with quality assessment data
/// that can feed into SONA trajectories, the embedding classifier,
/// and model router calibration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualitySignal {
    /// Unique identifier for this signal
    pub id: String,

    /// Human-readable task description (used for embedding generation)
    pub task_description: String,

    /// Execution outcome
    pub outcome: Outcome,

    /// Composite quality score (0.0 - 1.0)
    pub quality_score: f32,

    /// Optional human verdict
    #[serde(default)]
    pub human_verdict: Option<HumanVerdict>,

    /// Optional structured quality factors for detailed analysis
    #[serde(default)]
    pub quality_factors: Option<QualityFactors>,

    /// ISO 8601 timestamp of task completion
    pub completed_at: String,
}

/// Granular quality factor breakdown.
///
/// Not all providers will have all factors. Fields default to `None`,
/// meaning "not assessed" (distinct from `0.0`, which means "assessed as zero").
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityFactors {
    /// Whether acceptance criteria were met (0.0 - 1.0)
    pub acceptance_criteria_met: Option<f32>,
    /// Whether tests are passing (0.0 - 1.0)
    pub tests_passing: Option<f32>,
    /// Whether there are no regressions (0.0 - 1.0)
    pub no_regressions: Option<f32>,
    /// Whether linting is clean (0.0 - 1.0)
    pub lint_clean: Option<f32>,
    /// Whether type checking passes (0.0 - 1.0)
    pub type_check_clean: Option<f32>,
    /// Whether code follows established patterns (0.0 - 1.0)
    pub follows_patterns: Option<f32>,
    /// Relevance to the task context (0.0 - 1.0)
    pub context_relevance: Option<f32>,
    /// Coherence of reasoning chain (0.0 - 1.0)
    pub reasoning_coherence: Option<f32>,
    /// Efficiency of execution (0.0 - 1.0)
    pub execution_efficiency: Option<f32>,
}

/// Quality weight overrides from a provider.
///
/// If a provider returns weights, they influence how the composite
/// quality score is computed from individual factors for that provider's
/// signals. Weights should sum to approximately 1.0.
///
/// Note: This is distinct from `quality::QualityWeights` which covers
/// the scoring engine's internal dimensions (schema, coherence, diversity).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderQualityWeights {
    /// Weight for task completion factors (acceptance criteria, tests)
    pub task_completion: f32,
    /// Weight for code quality factors (lint, types, patterns)
    pub code_quality: f32,
    /// Weight for process factors (reasoning, efficiency)
    pub process: f32,
}

impl Default for ProviderQualityWeights {
    fn default() -> Self {
        Self {
            task_completion: 0.5,
            code_quality: 0.3,
            process: 0.2,
        }
    }
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

/// Trait for external systems that supply quality signals to RuvLLM.
///
/// Implementations are registered with [`IntelligenceLoader`] and called
/// during [`IntelligenceLoader::load_all_signals`]. The loader handles
/// mapping signals to SONA trajectories, classifier entries, and router
/// calibration data.
///
/// # Examples
///
/// File-based provider (built-in):
/// ```rust,ignore
/// use ruvllm::intelligence::FileSignalProvider;
/// use std::path::PathBuf;
///
/// let provider = FileSignalProvider::new(PathBuf::from("signals.json"));
/// loader.register_provider(Box::new(provider));
/// ```
///
/// Custom provider:
/// ```rust,ignore
/// use ruvllm::intelligence::{IntelligenceProvider, QualitySignal, ProviderQualityWeights};
/// use ruvllm::error::Result;
///
/// struct MyPipelineProvider;
///
/// impl IntelligenceProvider for MyPipelineProvider {
///     fn name(&self) -> &str { "my-pipeline" }
///     fn load_signals(&self) -> Result<Vec<QualitySignal>> {
///         // Read from your data source
///         Ok(vec![])
///     }
/// }
/// ```
pub trait IntelligenceProvider: Send + Sync {
    /// Human-readable name for this provider (used in logs and diagnostics).
    fn name(&self) -> &str;

    /// Load quality signals from this provider's data source.
    ///
    /// Returns an empty vec if no signals are available (not an error).
    /// Errors indicate that the data source exists but could not be read.
    fn load_signals(&self) -> Result<Vec<QualitySignal>>;

    /// Optional quality weight overrides for this provider's signals.
    ///
    /// If `None`, default weights are used when computing composite scores
    /// from `QualityFactors`.
    fn quality_weights(&self) -> Option<ProviderQualityWeights> {
        None
    }
}

// ---------------------------------------------------------------------------
// FileSignalProvider — built-in file-based provider
// ---------------------------------------------------------------------------

/// Built-in file-based intelligence provider.
///
/// Reads quality signals from a JSON file at a specified path.
/// This is the default provider for systems that write a signal file
/// to `.claude/intelligence/data/`. Non-Rust integrations (TypeScript,
/// Python, etc.) typically use this path.
///
/// ## File Format
///
/// The JSON file should contain an array of [`QualitySignal`] objects:
///
/// ```json
/// [
///   {
///     "id": "task-001",
///     "task_description": "Implement login endpoint",
///     "outcome": "success",
///     "quality_score": 0.92,
///     "human_verdict": "approved",
///     "completed_at": "2025-02-21T12:00:00Z"
///   }
/// ]
/// ```
pub struct FileSignalProvider {
    path: PathBuf,
}

impl FileSignalProvider {
    /// Create a new file-based provider reading from the given path.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Returns the path this provider reads from.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl IntelligenceProvider for FileSignalProvider {
    fn name(&self) -> &str {
        "file-signals"
    }

    fn load_signals(&self) -> Result<Vec<QualitySignal>> {
        if !self.path.exists() {
            return Ok(vec![]); // No file = no signals, not an error
        }

        // Check file size before reading (S02: prevent OOM)
        let metadata = std::fs::metadata(&self.path)?;
        if metadata.len() > MAX_SIGNAL_FILE_SIZE {
            return Err(crate::error::RuvLLMError::InvalidOperation(format!(
                "Signal file {} exceeds max size ({} bytes, limit {})",
                self.path.display(),
                metadata.len(),
                MAX_SIGNAL_FILE_SIZE
            )));
        }

        // Use BufReader for streaming parse (P2: avoid double allocation)
        let file = std::fs::File::open(&self.path)?;
        let reader = std::io::BufReader::new(file);
        let signals: Vec<QualitySignal> = serde_json::from_reader(reader).map_err(|e| {
            crate::error::RuvLLMError::Serialization(format!(
                "Failed to parse signal file {}: {}",
                self.path.display(),
                e
            ))
        })?;

        // Check signal count (S03: prevent resource exhaustion)
        if signals.len() > MAX_SIGNALS_PER_FILE {
            return Err(crate::error::RuvLLMError::InvalidOperation(format!(
                "Signal file contains {} signals, max is {}",
                signals.len(),
                MAX_SIGNALS_PER_FILE
            )));
        }

        // Validate score ranges (S04: prevent NaN/Inf propagation)
        for signal in &signals {
            if !signal.quality_score.is_finite()
                || signal.quality_score < 0.0
                || signal.quality_score > 1.0
            {
                return Err(crate::error::RuvLLMError::InvalidOperation(format!(
                    "Signal '{}' has invalid quality_score: {}",
                    signal.id, signal.quality_score
                )));
            }
        }

        Ok(signals)
    }

    fn quality_weights(&self) -> Option<ProviderQualityWeights> {
        // Check for quality-weights.json alongside the signal file
        let config_path = self
            .path
            .parent()
            .unwrap_or(Path::new("."))
            .join("quality-weights.json");

        if !config_path.exists() {
            return None;
        }

        let contents = std::fs::read_to_string(&config_path).ok()?;
        serde_json::from_str(&contents).ok()
    }
}

// ---------------------------------------------------------------------------
// IntelligenceLoader — provider registry and signal aggregator
// ---------------------------------------------------------------------------

/// Aggregates quality signals from multiple registered providers.
///
/// The loader maintains a list of [`IntelligenceProvider`] implementations
/// and calls them in registration order during [`load_all_signals`].
///
/// # Zero Overhead
///
/// If no providers are registered, `load_all_signals` returns an empty vec
/// with no allocations beyond the empty `Vec`.
pub struct IntelligenceLoader {
    providers: Vec<Box<dyn IntelligenceProvider>>,
}

impl IntelligenceLoader {
    /// Create a new empty loader with no registered providers.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register an external intelligence provider.
    ///
    /// Providers are called in registration order during `load_all_signals()`.
    pub fn register_provider(&mut self, provider: Box<dyn IntelligenceProvider>) {
        self.providers.push(provider);
    }

    /// Returns the number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Returns the names of all registered providers.
    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.name()).collect()
    }

    /// Load signals from all registered providers.
    ///
    /// Signals from each provider are collected into a flat list.
    /// If a provider fails, its error is logged but does not prevent
    /// other providers from loading — the failure is non-fatal.
    ///
    /// Returns `(signals, errors)` where errors contains provider names
    /// and their error messages for any that failed.
    pub fn load_all_signals(&self) -> (Vec<QualitySignal>, Vec<ProviderError>) {
        let mut all_signals = Vec::new();
        let mut errors = Vec::new();

        for provider in &self.providers {
            match provider.load_signals() {
                Ok(signals) => {
                    all_signals.extend(signals);
                }
                Err(e) => {
                    errors.push(ProviderError {
                        provider_name: provider.name().to_string(),
                        message: e.to_string(),
                    });
                }
            }
        }

        (all_signals, errors)
    }

    /// Load signals and their associated weight overrides from all providers.
    ///
    /// Returns a vec of `(signals, optional_weights)` tuples grouped by provider.
    pub fn load_grouped(&self) -> Vec<ProviderResult> {
        self.providers
            .iter()
            .map(|provider| {
                let signals = provider.load_signals().unwrap_or_default();
                let weights = provider.quality_weights();
                ProviderResult {
                    provider_name: provider.name().to_string(),
                    signals,
                    weights,
                }
            })
            .collect()
    }
}

impl Default for IntelligenceLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Error from a single provider during batch loading.
#[derive(Debug, Clone)]
pub struct ProviderError {
    /// Name of the provider that failed
    pub provider_name: String,
    /// Error message
    pub message: String,
}

/// Result from a single provider during grouped loading.
#[derive(Debug, Clone)]
pub struct ProviderResult {
    /// Name of the provider
    pub provider_name: String,
    /// Signals loaded (empty if provider failed)
    pub signals: Vec<QualitySignal>,
    /// Optional quality weight overrides
    pub weights: Option<ProviderQualityWeights>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Test provider that returns static signals
    struct MockProvider {
        signals: Vec<QualitySignal>,
    }

    impl IntelligenceProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }

        fn load_signals(&self) -> Result<Vec<QualitySignal>> {
            Ok(self.signals.clone())
        }

        fn quality_weights(&self) -> Option<ProviderQualityWeights> {
            Some(ProviderQualityWeights {
                task_completion: 0.6,
                code_quality: 0.3,
                process: 0.1,
            })
        }
    }

    /// Test provider that always fails
    struct FailingProvider;

    impl IntelligenceProvider for FailingProvider {
        fn name(&self) -> &str {
            "failing"
        }

        fn load_signals(&self) -> Result<Vec<QualitySignal>> {
            Err(crate::error::RuvLLMError::Serialization(
                "simulated failure".into(),
            ))
        }
    }

    fn make_signal(id: &str, score: f32) -> QualitySignal {
        QualitySignal {
            id: id.to_string(),
            task_description: format!("Task {}", id),
            outcome: Outcome::Success,
            quality_score: score,
            human_verdict: None,
            quality_factors: None,
            completed_at: "2025-02-21T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn empty_loader_returns_no_signals() {
        let loader = IntelligenceLoader::new();
        let (signals, errors) = loader.load_all_signals();
        assert!(signals.is_empty());
        assert!(errors.is_empty());
        assert_eq!(loader.provider_count(), 0);
    }

    #[test]
    fn mock_provider_returns_signals() {
        let mut loader = IntelligenceLoader::new();
        loader.register_provider(Box::new(MockProvider {
            signals: vec![make_signal("t1", 0.9), make_signal("t2", 0.7)],
        }));

        let (signals, errors) = loader.load_all_signals();
        assert_eq!(signals.len(), 2);
        assert!(errors.is_empty());
        assert_eq!(signals[0].id, "t1");
        assert!((signals[0].quality_score - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn failing_provider_non_fatal() {
        let mut loader = IntelligenceLoader::new();
        loader.register_provider(Box::new(FailingProvider));
        loader.register_provider(Box::new(MockProvider {
            signals: vec![make_signal("t3", 0.8)],
        }));

        let (signals, errors) = loader.load_all_signals();
        assert_eq!(signals.len(), 1); // mock provider's signal
        assert_eq!(errors.len(), 1); // failing provider's error
        assert_eq!(errors[0].provider_name, "failing");
    }

    #[test]
    fn multiple_providers_aggregate() {
        let mut loader = IntelligenceLoader::new();
        loader.register_provider(Box::new(MockProvider {
            signals: vec![make_signal("a1", 0.9)],
        }));
        loader.register_provider(Box::new(MockProvider {
            signals: vec![make_signal("b1", 0.8), make_signal("b2", 0.6)],
        }));

        let (signals, _) = loader.load_all_signals();
        assert_eq!(signals.len(), 3);
        assert_eq!(loader.provider_count(), 2);
    }

    #[test]
    fn grouped_loading() {
        let mut loader = IntelligenceLoader::new();
        loader.register_provider(Box::new(MockProvider {
            signals: vec![make_signal("g1", 0.85)],
        }));

        let results = loader.load_grouped();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].provider_name, "mock");
        assert_eq!(results[0].signals.len(), 1);
        assert!(results[0].weights.is_some());
    }

    #[test]
    fn provider_names() {
        let mut loader = IntelligenceLoader::new();
        loader.register_provider(Box::new(MockProvider { signals: vec![] }));
        loader.register_provider(Box::new(FailingProvider));
        assert_eq!(loader.provider_names(), vec!["mock", "failing"]);
    }

    #[test]
    fn file_provider_missing_file() {
        let provider = FileSignalProvider::new(PathBuf::from("/nonexistent/signals.json"));
        let signals = provider.load_signals().unwrap();
        assert!(signals.is_empty());
    }

    #[test]
    fn file_provider_reads_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-signals.json");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(
            f,
            r#"[
            {{
                "id": "f1",
                "task_description": "Fix login bug",
                "outcome": "success",
                "quality_score": 0.95,
                "completed_at": "2025-02-21T10:00:00Z"
            }}
        ]"#
        )
        .unwrap();

        let provider = FileSignalProvider::new(path);
        let signals = provider.load_signals().unwrap();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].id, "f1");
        assert!((signals[0].quality_score - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn file_provider_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not json").unwrap();

        let provider = FileSignalProvider::new(path);
        assert!(provider.load_signals().is_err());
    }

    #[test]
    fn quality_factors_default() {
        let factors = QualityFactors::default();
        assert!(factors.acceptance_criteria_met.is_none());
        assert!(factors.tests_passing.is_none());
    }

    #[test]
    fn provider_quality_weights_default() {
        let w = ProviderQualityWeights::default();
        let sum = w.task_completion + w.code_quality + w.process;
        assert!((sum - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn quality_signal_serde_roundtrip() {
        let signal = QualitySignal {
            id: "rt1".to_string(),
            task_description: "Test roundtrip".to_string(),
            outcome: Outcome::Success,
            quality_score: 0.88,
            human_verdict: Some(HumanVerdict::Approved),
            quality_factors: Some(QualityFactors {
                tests_passing: Some(1.0),
                lint_clean: Some(0.9),
                ..Default::default()
            }),
            completed_at: "2025-02-21T12:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&signal).unwrap();
        let parsed: QualitySignal = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "rt1");
        assert!((parsed.quality_score - 0.88).abs() < f32::EPSILON);
        assert!(parsed.quality_factors.is_some());
        let factors = parsed.quality_factors.unwrap();
        assert!((factors.tests_passing.unwrap() - 1.0).abs() < f32::EPSILON);
    }
}
