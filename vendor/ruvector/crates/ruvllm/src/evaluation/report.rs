//! Evaluation Report Generation
//!
//! Formats evaluation results for different outputs:
//! - Leaderboard (console)
//! - JSON (programmatic)
//! - Markdown (documentation)

use super::harness::{AblationMode, EvalReport, ModeMetrics};
use serde::{Deserialize, Serialize};

/// Entry in a leaderboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    /// Rank (1 = best)
    pub rank: usize,
    /// Ablation mode
    pub mode: AblationMode,
    /// Task success rate
    pub success_rate: f64,
    /// Verified success rate
    pub verified_rate: f64,
    /// Long horizon success rate
    pub long_horizon_rate: f64,
    /// Average diff quality score
    pub quality_score: f64,
    /// Cost per accepted patch
    pub cost_per_patch: f64,
    /// p95 latency in milliseconds
    pub p95_latency_ms: f64,
    /// Improvement over baseline (%)
    pub improvement_pct: Option<f64>,
}

/// Comparison between two ablation configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AblationComparison {
    /// Baseline mode
    pub baseline: AblationMode,
    /// Target mode being compared
    pub target: AblationMode,
    /// Success rate delta (target - baseline)
    pub success_delta: f64,
    /// Quality score delta
    pub quality_delta: f64,
    /// Cost delta (negative = cheaper)
    pub cost_delta: f64,
    /// Latency delta (negative = faster)
    pub latency_delta: f64,
    /// Statistical significance (p-value)
    pub p_value: Option<f64>,
    /// Whether improvement is significant
    pub is_significant: bool,
}

impl EvalReport {
    /// Generate leaderboard entries sorted by success rate
    pub fn to_leaderboard_entries(&self) -> Vec<LeaderboardEntry> {
        let mut entries: Vec<_> = self
            .mode_metrics
            .iter()
            .map(|(mode, metrics)| {
                LeaderboardEntry {
                    rank: 0, // Will be set after sorting
                    mode: *mode,
                    success_rate: metrics.correctness.task_success_rate(),
                    verified_rate: metrics.correctness.verified_success_rate(),
                    long_horizon_rate: metrics.correctness.long_horizon_success_rate(),
                    quality_score: metrics.avg_quality_score,
                    cost_per_patch: metrics.economics.cost_per_accepted_patch,
                    p95_latency_ms: metrics.economics.latency.end_to_end.p95() * 1000.0,
                    improvement_pct: self.improvement_over_baseline(*mode),
                }
            })
            .collect();

        // Sort by success rate (descending)
        entries.sort_by(|a, b| b.success_rate.partial_cmp(&a.success_rate).unwrap());

        // Assign ranks
        for (i, entry) in entries.iter_mut().enumerate() {
            entry.rank = i + 1;
        }

        entries
    }

    /// Compare all modes against baseline
    pub fn compare_all_to_baseline(&self) -> Vec<AblationComparison> {
        let baseline = match self.mode_metrics.get(&AblationMode::Baseline) {
            Some(b) => b,
            None => return vec![],
        };

        self.mode_metrics
            .iter()
            .filter(|(mode, _)| **mode != AblationMode::Baseline)
            .map(|(mode, metrics)| self.compare_modes(baseline, metrics, *mode))
            .collect()
    }

    /// Compare two specific modes
    fn compare_modes(
        &self,
        baseline: &ModeMetrics,
        target: &ModeMetrics,
        target_mode: AblationMode,
    ) -> AblationComparison {
        let success_delta =
            target.correctness.task_success_rate() - baseline.correctness.task_success_rate();

        let quality_delta = target.avg_quality_score - baseline.avg_quality_score;

        let cost_delta =
            target.economics.cost_per_accepted_patch - baseline.economics.cost_per_accepted_patch;

        let latency_delta =
            target.economics.latency.end_to_end.p95() - baseline.economics.latency.end_to_end.p95();

        // Simple significance check (would use proper stats in production)
        let is_significant = success_delta.abs() > 0.05;

        AblationComparison {
            baseline: AblationMode::Baseline,
            target: target_mode,
            success_delta,
            quality_delta,
            cost_delta,
            latency_delta,
            p_value: None, // Would compute with proper statistical test
            is_significant,
        }
    }

    /// Generate markdown report
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str("# RuvLLM Evaluation Report\n\n");
        md.push_str(&format!(
            "**Generated:** {}\n\n",
            self.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        ));

        // Configuration
        md.push_str("## Configuration\n\n");
        md.push_str(&format!("- Tasks: {}\n", self.config.task_count));
        md.push_str(&format!("- Seeds: {:?}\n", self.config.seeds));
        md.push_str(&format!(
            "- Quality threshold: {:.0}%\n",
            self.config.quality_threshold * 100.0
        ));
        md.push_str(&format!(
            "- Cost target: ${:.2}\n\n",
            self.config.cost_target
        ));

        // Leaderboard
        md.push_str("## Results Leaderboard\n\n");
        md.push_str("| Rank | Mode | Success% | Verified% | Quality | $/patch | p95 lat |\n");
        md.push_str("|------|------|----------|-----------|---------|---------|--------|\n");

        for entry in self.to_leaderboard_entries() {
            md.push_str(&format!(
                "| {} | {} | {:.1}% | {:.1}% | {:.2} | ${:.4} | {:.1}ms |\n",
                entry.rank,
                entry.mode.name(),
                entry.success_rate * 100.0,
                entry.verified_rate * 100.0,
                entry.quality_score,
                entry.cost_per_patch,
                entry.p95_latency_ms,
            ));
        }

        md.push('\n');

        // Ablation Analysis
        md.push_str("## Ablation Analysis\n\n");
        md.push_str("Improvements over baseline:\n\n");

        for comparison in self.compare_all_to_baseline() {
            let direction = if comparison.success_delta > 0.0 {
                "↑"
            } else {
                "↓"
            };
            let sig = if comparison.is_significant { "**" } else { "" };

            md.push_str(&format!(
                "- **{}**: {}{:+.1}%{} success rate\n",
                comparison.target.name(),
                sig,
                comparison.success_delta * 100.0,
                sig,
            ));

            if comparison.success_delta > 0.0 {
                md.push_str(&format!(
                    "  - Quality: {:+.2}, Cost: ${:+.4}, Latency: {:+.1}ms\n",
                    comparison.quality_delta,
                    comparison.cost_delta,
                    comparison.latency_delta * 1000.0,
                ));
            }
        }

        md.push('\n');

        // Key Findings
        md.push_str("## Key Findings\n\n");

        if let Some(best) = self.best_mode() {
            md.push_str(&format!("- **Best performing mode:** {}\n", best.name()));

            if let Some(improvement) = self.improvement_over_baseline(best) {
                md.push_str(&format!(
                    "- **Improvement over baseline:** {:.1}%\n",
                    improvement
                ));
            }
        }

        // Recommendations
        md.push_str("\n## Recommendations\n\n");
        md.push_str("1. Use Full mode (Retrieval + Adapters + SONA) for maximum accuracy\n");
        md.push_str("2. Use Retrieval Only mode for cost-sensitive deployments\n");
        md.push_str("3. Monitor p95 latency under load - consider batching for high throughput\n");

        md
    }

    /// Generate JSON report
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Generate compact summary
    pub fn summary(&self) -> String {
        let best = self.best_mode().unwrap_or(AblationMode::Baseline);
        let best_metrics = self.mode_metrics.get(&best);

        let (success, cost) = match best_metrics {
            Some(m) => (
                m.correctness.task_success_rate() * 100.0,
                m.economics.cost_per_accepted_patch,
            ),
            None => (0.0, 0.0),
        };

        let improvement = self.improvement_over_baseline(best).unwrap_or(0.0);

        format!(
            "Best: {} ({:.1}% success, ${:.4}/patch, +{:.1}% vs baseline)",
            best.name(),
            success,
            cost,
            improvement
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::correctness::CorrectnessMetrics;
    use crate::evaluation::economics::EconomicsMetrics;
    use crate::evaluation::harness::EvalConfig;
    use std::collections::HashMap;
    use std::time::Duration;

    fn create_test_report() -> EvalReport {
        let mut mode_metrics = HashMap::new();

        // Baseline
        let mut baseline_correctness = CorrectnessMetrics::new();
        baseline_correctness.total_tasks = 100;
        baseline_correctness.tests_passed = 30;

        let mut baseline_economics = EconomicsMetrics::new();
        baseline_economics.successful_tasks = 30;
        baseline_economics.cost.input_tokens = 5_000_000;
        baseline_economics.recalculate();

        mode_metrics.insert(
            AblationMode::Baseline,
            ModeMetrics {
                mode: AblationMode::Baseline,
                correctness: baseline_correctness,
                economics: baseline_economics,
                avg_quality_score: 0.6,
                total_runs: 100,
            },
        );

        // Full mode
        let mut full_correctness = CorrectnessMetrics::new();
        full_correctness.total_tasks = 100;
        full_correctness.tests_passed = 75;

        let mut full_economics = EconomicsMetrics::new();
        full_economics.successful_tasks = 75;
        full_economics.cost.input_tokens = 6_000_000;
        full_economics.recalculate();

        mode_metrics.insert(
            AblationMode::Full,
            ModeMetrics {
                mode: AblationMode::Full,
                correctness: full_correctness,
                economics: full_economics,
                avg_quality_score: 0.82,
                total_runs: 100,
            },
        );

        EvalReport {
            config: EvalConfig::default(),
            mode_metrics,
            total_duration: Duration::from_secs(300),
            timestamp: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_leaderboard_entries() {
        let report = create_test_report();
        let entries = report.to_leaderboard_entries();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].mode, AblationMode::Full); // Higher success rate
        assert_eq!(entries[0].rank, 1);
        assert_eq!(entries[1].mode, AblationMode::Baseline);
        assert_eq!(entries[1].rank, 2);
    }

    #[test]
    fn test_ablation_comparison() {
        let report = create_test_report();
        let comparisons = report.compare_all_to_baseline();

        assert_eq!(comparisons.len(), 1);
        let full_comparison = &comparisons[0];

        assert_eq!(full_comparison.target, AblationMode::Full);
        assert!(full_comparison.success_delta > 0.0); // Full is better
        assert!(full_comparison.is_significant);
    }

    #[test]
    fn test_markdown_generation() {
        let report = create_test_report();
        let md = report.to_markdown();

        assert!(md.contains("# RuvLLM Evaluation Report"));
        assert!(md.contains("Results Leaderboard"));
        assert!(md.contains("Ablation Analysis"));
        assert!(md.contains("Full"));
        assert!(md.contains("Baseline"));
    }

    #[test]
    fn test_summary() {
        let report = create_test_report();
        let summary = report.summary();

        assert!(summary.contains("Full"));
        assert!(summary.contains("success"));
    }
}
