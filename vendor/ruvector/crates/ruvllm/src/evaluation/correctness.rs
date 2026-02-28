//! Correctness Metrics - Layer 1
//!
//! Measures whether patches actually work:
//! - Task success rate (passes repo test suite)
//! - Verified success rate (human validated)
//! - Long horizon success rate (multi-file, high coupling)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Verification level for a task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerificationLevel {
    /// Automated test suite only
    Automated,
    /// Human validated (like SWE-bench Verified)
    HumanVerified,
    /// Multi-reviewer consensus
    ConsensusVerified,
}

/// Result of running a test suite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuiteResult {
    /// Total tests in suite
    pub total_tests: usize,
    /// Tests that passed
    pub passed: usize,
    /// Tests that failed
    pub failed: usize,
    /// Tests that were skipped
    pub skipped: usize,
    /// Test execution time
    pub duration: Duration,
    /// Specific test failures (test name -> error message)
    pub failures: HashMap<String, String>,
    /// Whether this is a regression (tests that passed before now fail)
    pub regressions: Vec<String>,
}

impl TestSuiteResult {
    /// Calculate pass rate (0.0 to 1.0)
    pub fn pass_rate(&self) -> f64 {
        if self.total_tests == 0 {
            return 0.0;
        }
        self.passed as f64 / self.total_tests as f64
    }

    /// Check if all tests passed
    pub fn all_passed(&self) -> bool {
        self.failed == 0 && self.regressions.is_empty()
    }

    /// Check if this is a clean pass (no regressions, no failures)
    pub fn is_clean(&self) -> bool {
        self.failed == 0 && self.regressions.is_empty() && self.skipped == 0
    }
}

/// Result of a single task evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Task identifier
    pub task_id: String,
    /// Repository the task is from
    pub repo: String,
    /// Issue/PR number if applicable
    pub issue_id: Option<String>,
    /// Whether the patch was generated successfully
    pub patch_generated: bool,
    /// Whether the patch applies cleanly
    pub patch_applies: bool,
    /// Test suite results after applying patch
    pub test_results: Option<TestSuiteResult>,
    /// Verification level achieved
    pub verification_level: VerificationLevel,
    /// Human verification result (if applicable)
    pub human_verified: Option<bool>,
    /// Number of files changed
    pub files_changed: usize,
    /// Total lines changed (additions + deletions)
    pub lines_changed: usize,
    /// Whether this is a multi-file change
    pub is_multi_file: bool,
    /// Coupling score (0.0 = isolated, 1.0 = highly coupled)
    pub coupling_score: f64,
    /// Time to generate the patch
    pub generation_time: Duration,
    /// Number of retries needed
    pub retries: usize,
    /// Error message if failed
    pub error: Option<String>,
}

impl TaskResult {
    /// Check if task succeeded (patch works and tests pass)
    pub fn succeeded(&self) -> bool {
        self.patch_generated
            && self.patch_applies
            && self.test_results.as_ref().map_or(false, |t| t.all_passed())
    }

    /// Check if task is verified successful
    pub fn verified_success(&self) -> bool {
        self.succeeded() && self.human_verified.unwrap_or(false)
    }

    /// Check if this is a long-horizon task (multi-file, high coupling)
    pub fn is_long_horizon(&self) -> bool {
        self.is_multi_file && self.coupling_score > 0.5
    }
}

/// Aggregated correctness metrics across multiple tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectnessMetrics {
    /// Total tasks evaluated
    pub total_tasks: usize,
    /// Tasks where patch was generated
    pub patches_generated: usize,
    /// Tasks where patch applied cleanly
    pub patches_applied: usize,
    /// Tasks where tests passed
    pub tests_passed: usize,
    /// Human verified successes
    pub verified_successes: usize,
    /// Long horizon successes (multi-file, high coupling)
    pub long_horizon_successes: usize,
    /// Total long horizon tasks
    pub long_horizon_total: usize,
    /// Per-repo breakdown
    pub per_repo: HashMap<String, RepoMetrics>,
    /// Distribution of failure reasons
    pub failure_reasons: HashMap<String, usize>,
}

/// Per-repository metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepoMetrics {
    pub total: usize,
    pub succeeded: usize,
    pub verified: usize,
}

impl CorrectnessMetrics {
    /// Create new empty metrics
    pub fn new() -> Self {
        Self {
            total_tasks: 0,
            patches_generated: 0,
            patches_applied: 0,
            tests_passed: 0,
            verified_successes: 0,
            long_horizon_successes: 0,
            long_horizon_total: 0,
            per_repo: HashMap::new(),
            failure_reasons: HashMap::new(),
        }
    }

    /// Add a task result to the metrics
    pub fn add_result(&mut self, result: &TaskResult) {
        self.total_tasks += 1;

        if result.patch_generated {
            self.patches_generated += 1;
        }
        if result.patch_applies {
            self.patches_applied += 1;
        }
        if result.succeeded() {
            self.tests_passed += 1;
        }
        if result.verified_success() {
            self.verified_successes += 1;
        }

        // Long horizon tracking
        if result.is_long_horizon() {
            self.long_horizon_total += 1;
            if result.succeeded() {
                self.long_horizon_successes += 1;
            }
        }

        // Per-repo tracking
        let repo_metrics = self.per_repo.entry(result.repo.clone()).or_default();
        repo_metrics.total += 1;
        if result.succeeded() {
            repo_metrics.succeeded += 1;
        }
        if result.verified_success() {
            repo_metrics.verified += 1;
        }

        // Failure tracking
        if !result.succeeded() {
            let reason = if !result.patch_generated {
                "patch_generation_failed"
            } else if !result.patch_applies {
                "patch_apply_failed"
            } else {
                "tests_failed"
            };
            *self.failure_reasons.entry(reason.to_string()).or_insert(0) += 1;
        }
    }

    /// Task success rate (0.0 to 1.0)
    pub fn task_success_rate(&self) -> f64 {
        if self.total_tasks == 0 {
            return 0.0;
        }
        self.tests_passed as f64 / self.total_tasks as f64
    }

    /// Verified success rate (0.0 to 1.0)
    pub fn verified_success_rate(&self) -> f64 {
        if self.total_tasks == 0 {
            return 0.0;
        }
        self.verified_successes as f64 / self.total_tasks as f64
    }

    /// Long horizon success rate (0.0 to 1.0)
    pub fn long_horizon_success_rate(&self) -> f64 {
        if self.long_horizon_total == 0 {
            return 0.0;
        }
        self.long_horizon_successes as f64 / self.long_horizon_total as f64
    }

    /// Patch generation rate
    pub fn generation_rate(&self) -> f64 {
        if self.total_tasks == 0 {
            return 0.0;
        }
        self.patches_generated as f64 / self.total_tasks as f64
    }

    /// Patch application rate (of generated patches)
    pub fn application_rate(&self) -> f64 {
        if self.patches_generated == 0 {
            return 0.0;
        }
        self.patches_applied as f64 / self.patches_generated as f64
    }
}

impl Default for CorrectnessMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Correctness evaluator that runs test suites
pub struct CorrectnessEvaluator {
    /// Timeout for test suite execution
    pub test_timeout: Duration,
    /// Whether to run in isolated environment
    pub isolated: bool,
    /// Git clone depth for repo setup
    pub clone_depth: Option<usize>,
}

impl Default for CorrectnessEvaluator {
    fn default() -> Self {
        Self {
            test_timeout: Duration::from_secs(300), // 5 minutes
            isolated: true,
            clone_depth: Some(1),
        }
    }
}

impl CorrectnessEvaluator {
    /// Evaluate a single task
    pub async fn evaluate_task(
        &self,
        task_id: &str,
        repo: &str,
        patch: &str,
        _test_command: &str,
    ) -> TaskResult {
        // This is a stub - real implementation would:
        // 1. Clone the repo
        // 2. Apply the patch
        // 3. Run the test suite
        // 4. Collect results

        TaskResult {
            task_id: task_id.to_string(),
            repo: repo.to_string(),
            issue_id: None,
            patch_generated: !patch.is_empty(),
            patch_applies: false, // Would be set by git apply
            test_results: None,
            verification_level: VerificationLevel::Automated,
            human_verified: None,
            files_changed: 0,
            lines_changed: 0,
            is_multi_file: false,
            coupling_score: 0.0,
            generation_time: Duration::from_secs(0),
            retries: 0,
            error: Some("Not implemented - stub evaluator".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_success_rate_empty() {
        let metrics = CorrectnessMetrics::new();
        assert_eq!(metrics.task_success_rate(), 0.0);
    }

    #[test]
    fn test_task_success_rate_calculation() {
        let mut metrics = CorrectnessMetrics::new();

        // Add successful task
        let success = TaskResult {
            task_id: "1".into(),
            repo: "test/repo".into(),
            issue_id: None,
            patch_generated: true,
            patch_applies: true,
            test_results: Some(TestSuiteResult {
                total_tests: 10,
                passed: 10,
                failed: 0,
                skipped: 0,
                duration: Duration::from_secs(1),
                failures: HashMap::new(),
                regressions: vec![],
            }),
            verification_level: VerificationLevel::Automated,
            human_verified: Some(true),
            files_changed: 1,
            lines_changed: 10,
            is_multi_file: false,
            coupling_score: 0.2,
            generation_time: Duration::from_secs(5),
            retries: 0,
            error: None,
        };
        metrics.add_result(&success);

        // Add failed task
        let failure = TaskResult {
            task_id: "2".into(),
            repo: "test/repo".into(),
            issue_id: None,
            patch_generated: true,
            patch_applies: true,
            test_results: Some(TestSuiteResult {
                total_tests: 10,
                passed: 8,
                failed: 2,
                skipped: 0,
                duration: Duration::from_secs(1),
                failures: HashMap::from([("test1".into(), "assertion failed".into())]),
                regressions: vec![],
            }),
            verification_level: VerificationLevel::Automated,
            human_verified: None,
            files_changed: 2,
            lines_changed: 50,
            is_multi_file: true,
            coupling_score: 0.7,
            generation_time: Duration::from_secs(10),
            retries: 2,
            error: None,
        };
        metrics.add_result(&failure);

        assert_eq!(metrics.total_tasks, 2);
        assert_eq!(metrics.tests_passed, 1);
        assert_eq!(metrics.task_success_rate(), 0.5);
        assert_eq!(metrics.verified_success_rate(), 0.5);
    }

    #[test]
    fn test_long_horizon_tracking() {
        let mut metrics = CorrectnessMetrics::new();

        // Long horizon success
        let lh_success = TaskResult {
            task_id: "lh1".into(),
            repo: "test/repo".into(),
            issue_id: None,
            patch_generated: true,
            patch_applies: true,
            test_results: Some(TestSuiteResult {
                total_tests: 20,
                passed: 20,
                failed: 0,
                skipped: 0,
                duration: Duration::from_secs(5),
                failures: HashMap::new(),
                regressions: vec![],
            }),
            verification_level: VerificationLevel::Automated,
            human_verified: None,
            files_changed: 5,
            lines_changed: 200,
            is_multi_file: true,
            coupling_score: 0.8, // High coupling
            generation_time: Duration::from_secs(30),
            retries: 1,
            error: None,
        };
        metrics.add_result(&lh_success);

        assert_eq!(metrics.long_horizon_total, 1);
        assert_eq!(metrics.long_horizon_successes, 1);
        assert_eq!(metrics.long_horizon_success_rate(), 1.0);
    }
}
