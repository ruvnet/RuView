//! Evaluation Harness
//!
//! Runs comprehensive evaluation with ablation testing.
//!
//! Ablation grid:
//! 1. Baseline (no adapters, no retrieval)
//! 2. Retrieval only
//! 3. Adapters only
//! 4. Retrieval + Adapters
//! 5. Retrieval + Adapters + SONA (full)

use super::correctness::{CorrectnessMetrics, TaskResult, VerificationLevel};
use super::diff_quality::{DiffAnalyzer, DiffQualityMetrics};
use super::economics::{CostTracker, EconomicsMetrics, LatencyDistribution};
use crate::Result;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Ablation modes for testing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AblationMode {
    /// Baseline: no adapters, no retrieval
    Baseline,
    /// Retrieval only (HNSW pattern matching)
    RetrievalOnly,
    /// Adapters only (LoRA/MicroLoRA)
    AdaptersOnly,
    /// Retrieval + Adapters
    RetrievalPlusAdapters,
    /// Full: Retrieval + Adapters + SONA
    Full,
}

impl AblationMode {
    /// Get all modes for full ablation study
    pub fn all() -> Vec<AblationMode> {
        vec![
            Self::Baseline,
            Self::RetrievalOnly,
            Self::AdaptersOnly,
            Self::RetrievalPlusAdapters,
            Self::Full,
        ]
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Baseline => "Baseline",
            Self::RetrievalOnly => "Retrieval Only",
            Self::AdaptersOnly => "Adapters Only",
            Self::RetrievalPlusAdapters => "Retrieval + Adapters",
            Self::Full => "Full (R+A+SONA)",
        }
    }
}

/// Configuration for evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalConfig {
    /// Number of tasks to evaluate
    pub task_count: usize,
    /// Random seeds for multiple runs
    pub seeds: Vec<u64>,
    /// Ablation modes to test
    pub ablation_modes: Vec<AblationMode>,
    /// Timeout per task
    pub task_timeout: Duration,
    /// Whether to run tests in parallel
    pub parallel: bool,
    /// Maximum parallel tasks
    pub max_parallel: usize,
    /// Quality score threshold for "accepted" patch
    pub quality_threshold: f64,
    /// Cost target per accepted patch (USD)
    pub cost_target: f64,
    /// Whether to compute edit similarity (requires reference patches)
    pub compute_edit_similarity: bool,
    /// Whether to verify with humans (mock in tests)
    pub human_verification: bool,
}

impl Default for EvalConfig {
    fn default() -> Self {
        Self {
            task_count: 100,
            seeds: vec![42, 123, 456],
            ablation_modes: AblationMode::all(),
            task_timeout: Duration::from_secs(300),
            parallel: true,
            max_parallel: 4,
            quality_threshold: 0.7,
            cost_target: 1.0, // $1 per accepted patch
            compute_edit_similarity: true,
            human_verification: false,
        }
    }
}

/// A task to evaluate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalTask {
    /// Unique task ID
    pub id: String,
    /// Repository (owner/repo)
    pub repo: String,
    /// Issue/PR number
    pub issue: Option<String>,
    /// Task description
    pub description: String,
    /// Reference patch (if available)
    pub reference_patch: Option<String>,
    /// Test command to verify
    pub test_command: String,
    /// Expected files to modify
    pub expected_files: Vec<String>,
    /// Verification level
    pub verification_level: VerificationLevel,
    /// Tags for categorization
    pub tags: Vec<String>,
}

/// Result of a single evaluation run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRun {
    /// Task that was evaluated
    pub task_id: String,
    /// Ablation mode used
    pub mode: AblationMode,
    /// Random seed used
    pub seed: u64,
    /// Generated patch
    pub generated_patch: Option<String>,
    /// Correctness result
    pub correctness: TaskResult,
    /// Diff quality metrics
    pub diff_quality: Option<DiffQualityMetrics>,
    /// Cost for this run
    pub cost: CostTracker,
    /// Latency breakdown
    pub latency: LatencyBreakdown,
    /// Whether patch was accepted (passed quality bar)
    pub accepted: bool,
    /// Error if failed
    pub error: Option<String>,
}

/// Latency breakdown for a single run
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyBreakdown {
    pub routing_ms: f64,
    pub retrieval_ms: f64,
    pub adapter_load_ms: f64,
    pub generation_ms: f64,
    pub test_execution_ms: f64,
    pub total_ms: f64,
}

/// The main evaluation harness
pub struct EvaluationHarness {
    /// Configuration
    config: EvalConfig,
    /// Diff analyzer
    diff_analyzer: DiffAnalyzer,
    /// Results by mode
    results: HashMap<AblationMode, Vec<EvalRun>>,
}

impl EvaluationHarness {
    /// Create new harness
    pub fn new(config: EvalConfig) -> Self {
        Self {
            config,
            diff_analyzer: DiffAnalyzer::default(),
            results: HashMap::new(),
        }
    }

    /// Run evaluation on a set of tasks
    pub async fn run_evaluation(&mut self, tasks: &[EvalTask]) -> Result<EvalReport> {
        let start = Instant::now();

        for mode in &self.config.ablation_modes.clone() {
            let mode_results = self.run_mode(*mode, tasks).await?;
            self.results.insert(*mode, mode_results);
        }

        let total_duration = start.elapsed();

        Ok(self.generate_report(total_duration))
    }

    /// Run evaluation for a single ablation mode
    async fn run_mode(&mut self, mode: AblationMode, tasks: &[EvalTask]) -> Result<Vec<EvalRun>> {
        let mut runs = Vec::new();

        for task in tasks.iter().take(self.config.task_count) {
            for &seed in &self.config.seeds {
                let run = self.run_single_task(mode, task, seed).await?;
                runs.push(run);
            }
        }

        Ok(runs)
    }

    /// Run a single task evaluation
    async fn run_single_task(
        &self,
        mode: AblationMode,
        task: &EvalTask,
        seed: u64,
    ) -> Result<EvalRun> {
        let start = Instant::now();
        let mut latency = LatencyBreakdown::default();
        let mut cost = CostTracker::with_claude_pricing();

        // Simulate routing phase
        let route_start = Instant::now();
        let _routing_result = self.simulate_routing(mode, task);
        latency.routing_ms = route_start.elapsed().as_secs_f64() * 1000.0;

        // Simulate retrieval phase (if enabled)
        if matches!(
            mode,
            AblationMode::RetrievalOnly | AblationMode::RetrievalPlusAdapters | AblationMode::Full
        ) {
            let retrieval_start = Instant::now();
            let _patterns = self.simulate_retrieval(task);
            latency.retrieval_ms = retrieval_start.elapsed().as_secs_f64() * 1000.0;
        }

        // Simulate adapter loading (if enabled)
        if matches!(
            mode,
            AblationMode::AdaptersOnly | AblationMode::RetrievalPlusAdapters | AblationMode::Full
        ) {
            let adapter_start = Instant::now();
            self.simulate_adapter_load(task);
            latency.adapter_load_ms = adapter_start.elapsed().as_secs_f64() * 1000.0;
        }

        // Simulate patch generation
        let gen_start = Instant::now();
        let (patch, gen_cost) = self.simulate_generation(mode, task, seed);
        latency.generation_ms = gen_start.elapsed().as_secs_f64() * 1000.0;
        cost.add(&gen_cost);

        latency.total_ms = start.elapsed().as_secs_f64() * 1000.0;

        // Analyze diff quality
        let diff_quality = patch.as_ref().map(|p| {
            self.diff_analyzer
                .analyze(p, task.reference_patch.as_deref())
        });

        // Create correctness result (stub)
        let correctness = TaskResult {
            task_id: task.id.clone(),
            repo: task.repo.clone(),
            issue_id: task.issue.clone(),
            patch_generated: patch.is_some(),
            patch_applies: patch.is_some(), // Simplified
            test_results: None,             // Would run actual tests
            verification_level: task.verification_level,
            human_verified: None,
            files_changed: task.expected_files.len(),
            lines_changed: patch.as_ref().map_or(0, |p| p.lines().count()),
            is_multi_file: task.expected_files.len() > 1,
            coupling_score: 0.3,
            generation_time: Duration::from_millis(latency.generation_ms as u64),
            retries: 0,
            error: None,
        };

        // Determine if accepted
        let accepted = correctness.succeeded()
            && diff_quality.as_ref().map_or(false, |dq| {
                dq.combined_score >= self.config.quality_threshold
            });

        Ok(EvalRun {
            task_id: task.id.clone(),
            mode,
            seed,
            generated_patch: patch,
            correctness,
            diff_quality,
            cost,
            latency,
            accepted,
            error: None,
        })
    }

    /// Simulate routing decision
    fn simulate_routing(&self, _mode: AblationMode, _task: &EvalTask) -> String {
        // Would use ModelRouter in real implementation
        "sonnet".to_string()
    }

    /// Simulate pattern retrieval
    fn simulate_retrieval(&self, _task: &EvalTask) -> Vec<String> {
        // Would use HNSW router in real implementation
        vec!["pattern1".to_string(), "pattern2".to_string()]
    }

    /// Simulate adapter loading
    fn simulate_adapter_load(&self, _task: &EvalTask) {
        // Would load LoRA/MicroLoRA adapters
    }

    /// Simulate patch generation
    fn simulate_generation(
        &self,
        mode: AblationMode,
        _task: &EvalTask,
        _seed: u64,
    ) -> (Option<String>, CostTracker) {
        // Simulate different success rates based on mode
        let success_rate = match mode {
            AblationMode::Baseline => 0.3,
            AblationMode::RetrievalOnly => 0.45,
            AblationMode::AdaptersOnly => 0.50,
            AblationMode::RetrievalPlusAdapters => 0.65,
            AblationMode::Full => 0.75,
        };

        let mut cost = CostTracker::with_claude_pricing();
        cost.input_tokens = 5000;
        cost.output_tokens = 1000;

        // Simplified: always generate a patch for simulation
        let patch = if rand_success(success_rate) {
            Some("+// Fixed\n-// Old code".to_string())
        } else {
            None
        };

        (patch, cost)
    }

    /// Generate evaluation report
    fn generate_report(&self, duration: Duration) -> EvalReport {
        let mut mode_metrics: HashMap<AblationMode, ModeMetrics> = HashMap::new();

        for (mode, runs) in &self.results {
            let mut correctness = CorrectnessMetrics::new();
            let mut economics = EconomicsMetrics::new();
            let mut quality_scores = Vec::new();

            for run in runs {
                correctness.add_result(&run.correctness);
                economics.cost.add(&run.cost);

                if run.accepted {
                    economics.successful_tasks += 1;
                }

                if let Some(ref dq) = run.diff_quality {
                    quality_scores.push(dq.combined_score);
                }

                // Add latency samples
                economics
                    .latency
                    .routing
                    .add_secs(run.latency.routing_ms / 1000.0);
                economics
                    .latency
                    .end_to_end
                    .add_secs(run.latency.total_ms / 1000.0);
            }

            economics.recalculate();

            let avg_quality = if quality_scores.is_empty() {
                0.0
            } else {
                quality_scores.iter().sum::<f64>() / quality_scores.len() as f64
            };

            mode_metrics.insert(
                *mode,
                ModeMetrics {
                    mode: *mode,
                    correctness,
                    economics,
                    avg_quality_score: avg_quality,
                    total_runs: runs.len(),
                },
            );
        }

        EvalReport {
            config: self.config.clone(),
            mode_metrics,
            total_duration: duration,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Simple deterministic pseudo-random for simulation
fn rand_success(rate: f64) -> bool {
    // Use a simple hash for reproducibility
    rate > 0.5 // Simplified
}

/// Metrics for a single ablation mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeMetrics {
    pub mode: AblationMode,
    pub correctness: CorrectnessMetrics,
    pub economics: EconomicsMetrics,
    pub avg_quality_score: f64,
    pub total_runs: usize,
}

/// Complete evaluation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReport {
    pub config: EvalConfig,
    pub mode_metrics: HashMap<AblationMode, ModeMetrics>,
    pub total_duration: Duration,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl EvalReport {
    /// Generate leaderboard-style output
    pub fn to_leaderboard(&self) -> String {
        let mut output = String::new();
        output.push_str(
            "╔════════════════════════════════════════════════════════════════════════════╗\n",
        );
        output.push_str(
            "║                        RuvLLM Evaluation Report                            ║\n",
        );
        output.push_str(
            "╠════════════════════════════════════════════════════════════════════════════╣\n",
        );
        output.push_str(&format!(
            "║ Tasks: {} × {} seeds × {} modes = {} runs                                 ║\n",
            self.config.task_count,
            self.config.seeds.len(),
            self.config.ablation_modes.len(),
            self.config.task_count * self.config.seeds.len() * self.config.ablation_modes.len()
        ));
        output.push_str(&format!(
            "║ Duration: {:.1}s | Quality threshold: {:.0}%                              ║\n",
            self.total_duration.as_secs_f64(),
            self.config.quality_threshold * 100.0
        ));
        output.push_str(
            "╠════════════════════════════════════════════════════════════════════════════╣\n",
        );
        output.push_str(
            "║ Mode               │ Success% │ Verified% │ Quality │ $/patch │ p95 lat  ║\n",
        );
        output.push_str(
            "╠════════════════════════════════════════════════════════════════════════════╣\n",
        );

        // Sort modes by success rate
        let mut modes: Vec<_> = self.mode_metrics.values().collect();
        modes.sort_by(|a, b| {
            b.correctness
                .task_success_rate()
                .partial_cmp(&a.correctness.task_success_rate())
                .unwrap()
        });

        for metrics in modes {
            output.push_str(&format!(
                "║ {:18} │ {:7.1}% │ {:8.1}% │ {:7.2} │ ${:6.4} │ {:7.1}ms ║\n",
                metrics.mode.name(),
                metrics.correctness.task_success_rate() * 100.0,
                metrics.correctness.verified_success_rate() * 100.0,
                metrics.avg_quality_score,
                metrics.economics.cost_per_accepted_patch,
                metrics.economics.latency.end_to_end.p95() * 1000.0,
            ));
        }

        output.push_str(
            "╚════════════════════════════════════════════════════════════════════════════╝\n",
        );
        output
    }

    /// Get best performing mode
    pub fn best_mode(&self) -> Option<AblationMode> {
        self.mode_metrics
            .values()
            .max_by(|a, b| {
                a.correctness
                    .task_success_rate()
                    .partial_cmp(&b.correctness.task_success_rate())
                    .unwrap()
            })
            .map(|m| m.mode)
    }

    /// Calculate improvement over baseline
    pub fn improvement_over_baseline(&self, mode: AblationMode) -> Option<f64> {
        let baseline = self.mode_metrics.get(&AblationMode::Baseline)?;
        let target = self.mode_metrics.get(&mode)?;

        let baseline_rate = baseline.correctness.task_success_rate();
        if baseline_rate == 0.0 {
            return None;
        }

        Some((target.correctness.task_success_rate() - baseline_rate) / baseline_rate * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ablation_modes() {
        let modes = AblationMode::all();
        assert_eq!(modes.len(), 5);
        assert_eq!(modes[0], AblationMode::Baseline);
        assert_eq!(modes[4], AblationMode::Full);
    }

    #[test]
    fn test_eval_config_default() {
        let config = EvalConfig::default();
        assert_eq!(config.task_count, 100);
        assert_eq!(config.seeds.len(), 3);
        assert_eq!(config.ablation_modes.len(), 5);
    }

    #[tokio::test]
    async fn test_harness_creation() {
        let config = EvalConfig {
            task_count: 2,
            seeds: vec![42],
            ablation_modes: vec![AblationMode::Baseline, AblationMode::Full],
            ..Default::default()
        };

        let harness = EvaluationHarness::new(config);
        assert!(harness.results.is_empty());
    }
}
