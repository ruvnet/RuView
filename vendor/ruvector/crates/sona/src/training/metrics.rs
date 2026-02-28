//! Training Metrics for SONA
//!
//! Comprehensive analytics for training sessions.

use serde::{Deserialize, Serialize};

/// Training metrics collection
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TrainingMetrics {
    /// Pipeline/agent name
    pub name: String,
    /// Total examples processed
    pub total_examples: usize,
    /// Total training sessions
    pub training_sessions: u64,
    /// Patterns learned
    pub patterns_learned: usize,
    /// Quality samples for averaging
    pub quality_samples: Vec<f32>,
    /// Validation quality (if validation was run)
    pub validation_quality: Option<f32>,
    /// Performance metrics
    pub performance: PerformanceMetrics,
}

impl TrainingMetrics {
    /// Create new metrics
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Add quality sample
    pub fn add_quality_sample(&mut self, quality: f32) {
        self.quality_samples.push(quality);
        // Keep last 10000 samples
        if self.quality_samples.len() > 10000 {
            self.quality_samples.remove(0);
        }
    }

    /// Get average quality
    pub fn avg_quality(&self) -> f32 {
        if self.quality_samples.is_empty() {
            0.0
        } else {
            self.quality_samples.iter().sum::<f32>() / self.quality_samples.len() as f32
        }
    }

    /// Get quality percentile
    pub fn quality_percentile(&self, percentile: f32) -> f32 {
        if self.quality_samples.is_empty() {
            return 0.0;
        }

        let mut sorted = self.quality_samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let idx = ((percentile / 100.0) * (sorted.len() - 1) as f32) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    /// Get quality statistics
    pub fn quality_stats(&self) -> QualityMetrics {
        if self.quality_samples.is_empty() {
            return QualityMetrics::default();
        }

        let avg = self.avg_quality();
        let min = self
            .quality_samples
            .iter()
            .cloned()
            .fold(f32::MAX, f32::min);
        let max = self
            .quality_samples
            .iter()
            .cloned()
            .fold(f32::MIN, f32::max);

        let variance = self
            .quality_samples
            .iter()
            .map(|q| (q - avg).powi(2))
            .sum::<f32>()
            / self.quality_samples.len() as f32;
        let std_dev = variance.sqrt();

        QualityMetrics {
            avg,
            min,
            max,
            std_dev,
            p25: self.quality_percentile(25.0),
            p50: self.quality_percentile(50.0),
            p75: self.quality_percentile(75.0),
            p95: self.quality_percentile(95.0),
            sample_count: self.quality_samples.len(),
        }
    }

    /// Reset metrics
    pub fn reset(&mut self) {
        self.total_examples = 0;
        self.training_sessions = 0;
        self.patterns_learned = 0;
        self.quality_samples.clear();
        self.validation_quality = None;
        self.performance = PerformanceMetrics::default();
    }

    /// Merge with another metrics instance
    pub fn merge(&mut self, other: &TrainingMetrics) {
        self.total_examples += other.total_examples;
        self.training_sessions += other.training_sessions;
        self.patterns_learned = other.patterns_learned; // Take latest
        self.quality_samples.extend(&other.quality_samples);

        // Keep last 10000
        if self.quality_samples.len() > 10000 {
            let excess = self.quality_samples.len() - 10000;
            self.quality_samples.drain(0..excess);
        }
    }
}

/// Quality metrics summary
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// Average quality
    pub avg: f32,
    /// Minimum quality
    pub min: f32,
    /// Maximum quality
    pub max: f32,
    /// Standard deviation
    pub std_dev: f32,
    /// 25th percentile
    pub p25: f32,
    /// 50th percentile (median)
    pub p50: f32,
    /// 75th percentile
    pub p75: f32,
    /// 95th percentile
    pub p95: f32,
    /// Number of samples
    pub sample_count: usize,
}

impl std::fmt::Display for QualityMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "avg={:.4}, std={:.4}, min={:.4}, max={:.4}, p50={:.4}, p95={:.4} (n={})",
            self.avg, self.std_dev, self.min, self.max, self.p50, self.p95, self.sample_count
        )
    }
}

/// Performance metrics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Total training time in seconds
    pub total_training_secs: f64,
    /// Average batch processing time in milliseconds
    pub avg_batch_time_ms: f64,
    /// Average example processing time in microseconds
    pub avg_example_time_us: f64,
    /// Peak memory usage in MB
    pub peak_memory_mb: usize,
    /// Examples per second throughput
    pub examples_per_sec: f64,
    /// Pattern extraction time in milliseconds
    pub pattern_extraction_ms: f64,
}

impl PerformanceMetrics {
    /// Calculate throughput
    pub fn calculate_throughput(&mut self, examples: usize, duration_secs: f64) {
        if duration_secs > 0.0 {
            self.examples_per_sec = examples as f64 / duration_secs;
            self.avg_example_time_us = (duration_secs * 1_000_000.0) / examples as f64;
        }
    }
}

/// Epoch statistics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpochStats {
    /// Epoch number (0-indexed)
    pub epoch: usize,
    /// Examples processed in this epoch
    pub examples_processed: usize,
    /// Average quality for this epoch
    pub avg_quality: f32,
    /// Duration in seconds
    pub duration_secs: f64,
}

impl std::fmt::Display for EpochStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Epoch {}: {} examples, avg_quality={:.4}, {:.2}s",
            self.epoch + 1,
            self.examples_processed,
            self.avg_quality,
            self.duration_secs
        )
    }
}

/// Training result summary
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrainingResult {
    /// Pipeline name
    pub pipeline_name: String,
    /// Number of epochs completed
    pub epochs_completed: usize,
    /// Total examples processed
    pub total_examples: usize,
    /// Patterns learned
    pub patterns_learned: usize,
    /// Final average quality
    pub final_avg_quality: f32,
    /// Total duration in seconds
    pub total_duration_secs: f64,
    /// Per-epoch statistics
    pub epoch_stats: Vec<EpochStats>,
    /// Validation quality (if validation was run)
    pub validation_quality: Option<f32>,
}

impl TrainingResult {
    /// Get examples per second
    pub fn examples_per_sec(&self) -> f64 {
        if self.total_duration_secs > 0.0 {
            self.total_examples as f64 / self.total_duration_secs
        } else {
            0.0
        }
    }

    /// Get average epoch duration
    pub fn avg_epoch_duration(&self) -> f64 {
        if self.epochs_completed > 0 {
            self.total_duration_secs / self.epochs_completed as f64
        } else {
            0.0
        }
    }

    /// Check if training improved quality
    pub fn quality_improved(&self) -> bool {
        if self.epoch_stats.len() < 2 {
            return false;
        }
        let first = self.epoch_stats.first().unwrap().avg_quality;
        let last = self.epoch_stats.last().unwrap().avg_quality;
        last > first
    }

    /// Get quality improvement
    pub fn quality_improvement(&self) -> f32 {
        if self.epoch_stats.len() < 2 {
            return 0.0;
        }
        let first = self.epoch_stats.first().unwrap().avg_quality;
        let last = self.epoch_stats.last().unwrap().avg_quality;
        last - first
    }
}

impl std::fmt::Display for TrainingResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TrainingResult(pipeline={}, epochs={}, examples={}, patterns={}, \
             final_quality={:.4}, duration={:.2}s, throughput={:.1}/s)",
            self.pipeline_name,
            self.epochs_completed,
            self.total_examples,
            self.patterns_learned,
            self.final_avg_quality,
            self.total_duration_secs,
            self.examples_per_sec()
        )
    }
}

/// Comparison metrics between training runs
#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct TrainingComparison {
    /// Baseline result name
    pub baseline_name: String,
    /// Comparison result name
    pub comparison_name: String,
    /// Quality difference (comparison - baseline)
    pub quality_diff: f32,
    /// Quality improvement percentage
    pub quality_improvement_pct: f32,
    /// Throughput difference
    pub throughput_diff: f64,
    /// Duration difference in seconds
    pub duration_diff: f64,
}

#[allow(dead_code)]
impl TrainingComparison {
    /// Compare two training results
    pub fn compare(baseline: &TrainingResult, comparison: &TrainingResult) -> Self {
        let quality_diff = comparison.final_avg_quality - baseline.final_avg_quality;
        let quality_improvement_pct = if baseline.final_avg_quality > 0.0 {
            (quality_diff / baseline.final_avg_quality) * 100.0
        } else {
            0.0
        };

        Self {
            baseline_name: baseline.pipeline_name.clone(),
            comparison_name: comparison.pipeline_name.clone(),
            quality_diff,
            quality_improvement_pct,
            throughput_diff: comparison.examples_per_sec() - baseline.examples_per_sec(),
            duration_diff: comparison.total_duration_secs - baseline.total_duration_secs,
        }
    }
}

impl std::fmt::Display for TrainingComparison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let quality_sign = if self.quality_diff >= 0.0 { "+" } else { "" };
        let throughput_sign = if self.throughput_diff >= 0.0 { "+" } else { "" };

        write!(
            f,
            "Comparison {} vs {}: quality {}{:.4} ({}{:.1}%), throughput {}{:.1}/s",
            self.comparison_name,
            self.baseline_name,
            quality_sign,
            self.quality_diff,
            quality_sign,
            self.quality_improvement_pct,
            throughput_sign,
            self.throughput_diff
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = TrainingMetrics::new("test");
        assert_eq!(metrics.name, "test");
        assert_eq!(metrics.total_examples, 0);
    }

    #[test]
    fn test_quality_samples() {
        let mut metrics = TrainingMetrics::new("test");

        for i in 0..10 {
            metrics.add_quality_sample(i as f32 / 10.0);
        }

        assert_eq!(metrics.quality_samples.len(), 10);
        assert!((metrics.avg_quality() - 0.45).abs() < 0.01);
    }

    #[test]
    fn test_quality_percentiles() {
        let mut metrics = TrainingMetrics::new("test");

        for i in 0..100 {
            metrics.add_quality_sample(i as f32 / 100.0);
        }

        assert!((metrics.quality_percentile(50.0) - 0.5).abs() < 0.02);
        assert!((metrics.quality_percentile(95.0) - 0.95).abs() < 0.02);
    }

    #[test]
    fn test_quality_stats() {
        let mut metrics = TrainingMetrics::new("test");
        metrics.add_quality_sample(0.5);
        metrics.add_quality_sample(0.7);
        metrics.add_quality_sample(0.9);

        let stats = metrics.quality_stats();
        assert!((stats.avg - 0.7).abs() < 0.01);
        assert!((stats.min - 0.5).abs() < 0.01);
        assert!((stats.max - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_training_result() {
        let result = TrainingResult {
            pipeline_name: "test".into(),
            epochs_completed: 3,
            total_examples: 1000,
            patterns_learned: 50,
            final_avg_quality: 0.85,
            total_duration_secs: 10.0,
            epoch_stats: vec![
                EpochStats {
                    epoch: 0,
                    examples_processed: 333,
                    avg_quality: 0.75,
                    duration_secs: 3.0,
                },
                EpochStats {
                    epoch: 1,
                    examples_processed: 333,
                    avg_quality: 0.80,
                    duration_secs: 3.5,
                },
                EpochStats {
                    epoch: 2,
                    examples_processed: 334,
                    avg_quality: 0.85,
                    duration_secs: 3.5,
                },
            ],
            validation_quality: Some(0.82),
        };

        assert_eq!(result.examples_per_sec(), 100.0);
        assert!(result.quality_improved());
        assert!((result.quality_improvement() - 0.10).abs() < 0.01);
    }

    #[test]
    fn test_training_comparison() {
        let baseline = TrainingResult {
            pipeline_name: "baseline".into(),
            epochs_completed: 2,
            total_examples: 500,
            patterns_learned: 25,
            final_avg_quality: 0.70,
            total_duration_secs: 5.0,
            epoch_stats: vec![],
            validation_quality: None,
        };

        let improved = TrainingResult {
            pipeline_name: "improved".into(),
            epochs_completed: 2,
            total_examples: 500,
            patterns_learned: 30,
            final_avg_quality: 0.85,
            total_duration_secs: 4.0,
            epoch_stats: vec![],
            validation_quality: None,
        };

        let comparison = TrainingComparison::compare(&baseline, &improved);
        assert!((comparison.quality_diff - 0.15).abs() < 0.01);
        assert!(comparison.quality_improvement_pct > 20.0);
        assert!(comparison.throughput_diff > 0.0);
    }
}
