//! Quality tracking and metrics for the adaptive KV cache.
//!
//! Monitors:
//! - Quantization quality (PPL degradation)
//! - Memory efficiency
//! - Cache hit rates per tier
//! - Adaptive threshold convergence

#[cfg(feature = "no_std_gateway")]
use alloc::{collections::VecDeque, vec::Vec};

#[cfg(not(feature = "no_std_gateway"))]
use std::collections::VecDeque;
#[cfg(not(feature = "no_std_gateway"))]
use std::vec::Vec;

use super::tier::CacheTier;

/// Memory usage statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryStats {
    /// Hot tier memory usage in bytes
    pub hot_bytes: usize,
    /// Warm tier memory usage in bytes
    pub warm_bytes: usize,
    /// Archive tier memory usage in bytes
    pub archive_bytes: usize,
    /// Total memory usage in bytes
    pub total_bytes: usize,
    /// Compression ratio compared to FP16
    pub compression_ratio: f32,
}

impl MemoryStats {
    /// Calculate percentage of memory in each tier
    pub fn tier_percentages(&self) -> (f32, f32, f32) {
        if self.total_bytes == 0 {
            return (0.0, 0.0, 0.0);
        }

        let hot_pct = self.hot_bytes as f32 / self.total_bytes as f32 * 100.0;
        let warm_pct = self.warm_bytes as f32 / self.total_bytes as f32 * 100.0;
        let archive_pct = self.archive_bytes as f32 / self.total_bytes as f32 * 100.0;

        (hot_pct, warm_pct, archive_pct)
    }

    /// Calculate memory saved compared to FP16 baseline
    pub fn memory_saved(
        &self,
        baseline_tokens: usize,
        head_dim: usize,
        num_heads: usize,
        num_layers: usize,
    ) -> usize {
        let fp16_bytes = baseline_tokens * head_dim * num_heads * num_layers * 2 * 2; // 2 bytes * 2 (kv)
        fp16_bytes.saturating_sub(self.total_bytes)
    }
}

/// Quality feedback for adaptive threshold tuning
#[derive(Debug, Clone)]
pub struct QualityFeedback {
    /// Quality score (0.0 - 1.0, higher is better)
    pub score: f32,
    /// Measured PPL (perplexity)
    pub ppl: Option<f32>,
    /// Task accuracy if available
    pub task_accuracy: Option<f32>,
    /// Which tier caused the most degradation
    pub worst_tier: Option<CacheTier>,
    /// Timestamp (in arbitrary units)
    pub timestamp: u64,
}

impl QualityFeedback {
    /// Create feedback from PPL measurement
    pub fn from_ppl(ppl: f32, baseline_ppl: f32) -> Self {
        // Convert PPL to score: score = 1.0 - (ppl - baseline) / baseline
        // Clamped to [0, 1]
        let ppl_delta = (ppl - baseline_ppl) / baseline_ppl;
        let score = (1.0 - ppl_delta).clamp(0.0, 1.0);

        Self {
            score,
            ppl: Some(ppl),
            task_accuracy: None,
            worst_tier: None,
            timestamp: 0,
        }
    }

    /// Create feedback from task accuracy
    pub fn from_accuracy(accuracy: f32) -> Self {
        Self {
            score: accuracy,
            ppl: None,
            task_accuracy: Some(accuracy),
            worst_tier: None,
            timestamp: 0,
        }
    }

    /// Set timestamp
    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    /// Set worst tier
    pub fn with_worst_tier(mut self, tier: CacheTier) -> Self {
        self.worst_tier = Some(tier);
        self
    }
}

/// Aggregated quality metric
#[derive(Debug, Clone, Copy, Default)]
pub struct QualityMetric {
    /// Average quality score
    pub avg_score: f32,
    /// Minimum observed score
    pub min_score: f32,
    /// Maximum observed score
    pub max_score: f32,
    /// Standard deviation
    pub std_dev: f32,
    /// Number of samples
    pub sample_count: usize,
    /// Trend (positive = improving, negative = degrading)
    pub trend: f32,
}

impl QualityMetric {
    /// Check if quality meets target
    pub fn meets_target(&self, target: f32) -> bool {
        self.avg_score >= target
    }

    /// Check if quality is stable
    pub fn is_stable(&self, threshold: f32) -> bool {
        self.std_dev < threshold
    }

    /// Check if quality is improving
    pub fn is_improving(&self) -> bool {
        self.trend > 0.0
    }
}

/// Per-tier quality metrics
#[derive(Debug, Clone, Default)]
pub struct TierMetrics {
    /// Hot tier metrics
    pub hot: QualityMetric,
    /// Warm tier metrics
    pub warm: QualityMetric,
    /// Archive tier metrics
    pub archive: QualityMetric,
}

/// Quality tracker for adaptive threshold tuning
pub struct QualityTracker {
    /// Quality target (1.0 - acceptable PPL degradation)
    quality_target: f32,
    /// Rolling window of quality feedback
    history: VecDeque<QualityFeedback>,
    /// Maximum history size
    max_history: usize,
    /// Cumulative statistics
    sum_score: f32,
    sum_sq_score: f32,
    count: usize,
    /// Per-tier statistics
    tier_counts: [usize; 3],
    tier_sums: [f32; 3],
}

impl QualityTracker {
    /// Create a new quality tracker
    pub fn new(quality_target: f32) -> Self {
        Self {
            quality_target,
            history: VecDeque::with_capacity(1000),
            max_history: 1000,
            sum_score: 0.0,
            sum_sq_score: 0.0,
            count: 0,
            tier_counts: [0; 3],
            tier_sums: [0.0; 3],
        }
    }

    /// Record quality feedback
    pub fn record(&mut self, feedback: QualityFeedback) {
        // Update cumulative stats
        self.sum_score += feedback.score;
        self.sum_sq_score += feedback.score * feedback.score;
        self.count += 1;

        // Update tier-specific stats
        if let Some(tier) = feedback.worst_tier {
            let idx = match tier {
                CacheTier::Hot => 0,
                CacheTier::Warm => 1,
                CacheTier::Archive => 2,
            };
            self.tier_counts[idx] += 1;
            self.tier_sums[idx] += feedback.score;
        }

        // Add to history
        self.history.push_back(feedback);

        // Maintain history size
        while self.history.len() > self.max_history {
            if let Some(old) = self.history.pop_front() {
                // Adjust cumulative stats (approximate)
                self.sum_score -= old.score;
                self.sum_sq_score -= old.score * old.score;
                self.count = self.count.saturating_sub(1);
            }
        }
    }

    /// Get current aggregate metrics
    pub fn current_metrics(&self) -> QualityMetric {
        if self.count == 0 {
            return QualityMetric {
                avg_score: 1.0,
                min_score: 1.0,
                max_score: 1.0,
                std_dev: 0.0,
                sample_count: 0,
                trend: 0.0,
            };
        }

        let avg = self.sum_score / self.count as f32;
        let variance = (self.sum_sq_score / self.count as f32) - (avg * avg);
        let std_dev = variance.max(0.0).sqrt();

        let (min_score, max_score) = self
            .history
            .iter()
            .fold((f32::MAX, f32::MIN), |(min, max), f| {
                (min.min(f.score), max.max(f.score))
            });

        let trend = self.compute_trend();

        QualityMetric {
            avg_score: avg,
            min_score,
            max_score,
            std_dev,
            sample_count: self.count,
            trend,
        }
    }

    /// Compute quality trend
    fn compute_trend(&self) -> f32 {
        if self.history.len() < 10 {
            return 0.0;
        }

        let recent_count = 10.min(self.history.len() / 2);
        let earlier_count = recent_count;

        let recent_avg: f32 = self
            .history
            .iter()
            .rev()
            .take(recent_count)
            .map(|f| f.score)
            .sum::<f32>()
            / recent_count as f32;

        let earlier_avg: f32 = self
            .history
            .iter()
            .rev()
            .skip(recent_count)
            .take(earlier_count)
            .map(|f| f.score)
            .sum::<f32>()
            / earlier_count as f32;

        recent_avg - earlier_avg
    }

    /// Get per-tier metrics
    pub fn tier_metrics(&self) -> TierMetrics {
        let tier_metric = |idx: usize| -> QualityMetric {
            if self.tier_counts[idx] == 0 {
                return QualityMetric::default();
            }

            QualityMetric {
                avg_score: self.tier_sums[idx] / self.tier_counts[idx] as f32,
                min_score: 0.0, // Would need per-tier history for accurate min/max
                max_score: 1.0,
                std_dev: 0.0,
                sample_count: self.tier_counts[idx],
                trend: 0.0,
            }
        };

        TierMetrics {
            hot: tier_metric(0),
            warm: tier_metric(1),
            archive: tier_metric(2),
        }
    }

    /// Check if adaptation should be triggered
    pub fn should_adapt(&self) -> bool {
        let metrics = self.current_metrics();

        // Adapt if quality is degrading or below target
        metrics.avg_score < self.quality_target || metrics.trend < -0.01
    }

    /// Get recommendation for tier boundary adjustment
    pub fn boundary_adjustment_factor(&self) -> f32 {
        let metrics = self.current_metrics();

        if metrics.avg_score < self.quality_target {
            // Quality too low: expand hot buffer
            1.1 + (self.quality_target - metrics.avg_score)
        } else if metrics.avg_score > self.quality_target * 1.05 {
            // Quality is good: can be more aggressive
            0.95 - (metrics.avg_score - self.quality_target * 1.05) * 0.1
        } else {
            1.0 // No adjustment needed
        }
    }

    /// Get quality target
    pub fn quality_target(&self) -> f32 {
        self.quality_target
    }

    /// Set quality target
    pub fn set_quality_target(&mut self, target: f32) {
        self.quality_target = target.clamp(0.0, 1.0);
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.history.clear();
        self.sum_score = 0.0;
        self.sum_sq_score = 0.0;
        self.count = 0;
        self.tier_counts = [0; 3];
        self.tier_sums = [0.0; 3];
    }

    /// Get history length
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Get recent feedback entries
    pub fn recent_feedback(&self, n: usize) -> Vec<&QualityFeedback> {
        self.history.iter().rev().take(n).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_stats() {
        let stats = MemoryStats {
            hot_bytes: 100,
            warm_bytes: 200,
            archive_bytes: 300,
            total_bytes: 600,
            compression_ratio: 4.0,
        };

        let (hot, warm, archive) = stats.tier_percentages();
        assert!((hot - 16.67).abs() < 0.1);
        assert!((warm - 33.33).abs() < 0.1);
        assert!((archive - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_quality_feedback_from_ppl() {
        let feedback = QualityFeedback::from_ppl(10.5, 10.0);
        assert!(feedback.score > 0.9);
        assert!(feedback.score < 1.0);
        assert_eq!(feedback.ppl, Some(10.5));
    }

    #[test]
    fn test_quality_feedback_from_accuracy() {
        let feedback = QualityFeedback::from_accuracy(0.85);
        assert_eq!(feedback.score, 0.85);
        assert_eq!(feedback.task_accuracy, Some(0.85));
    }

    #[test]
    fn test_quality_tracker_record() {
        let mut tracker = QualityTracker::new(0.95);

        for i in 0..10 {
            let feedback = QualityFeedback::from_accuracy(0.9 + i as f32 * 0.01);
            tracker.record(feedback);
        }

        let metrics = tracker.current_metrics();
        assert_eq!(metrics.sample_count, 10);
        assert!(metrics.avg_score > 0.9);
    }

    #[test]
    fn test_quality_tracker_trend() {
        let mut tracker = QualityTracker::new(0.95);

        // Add improving quality
        for i in 0..20 {
            let feedback = QualityFeedback::from_accuracy(0.8 + i as f32 * 0.01);
            tracker.record(feedback);
        }

        let metrics = tracker.current_metrics();
        assert!(
            metrics.trend > 0.0,
            "Expected positive trend, got {}",
            metrics.trend
        );
    }

    #[test]
    fn test_quality_tracker_adaptation() {
        let mut tracker = QualityTracker::new(0.95);

        // Add poor quality
        for _ in 0..5 {
            let feedback = QualityFeedback::from_accuracy(0.85);
            tracker.record(feedback);
        }

        assert!(tracker.should_adapt());
        assert!(tracker.boundary_adjustment_factor() > 1.0);

        // Now add good quality (must exceed target * 1.05 = 0.9975)
        tracker.reset();
        for _ in 0..5 {
            let feedback = QualityFeedback::from_accuracy(1.0);
            tracker.record(feedback);
        }

        assert!(
            tracker.boundary_adjustment_factor() < 1.0,
            "Expected factor < 1.0 for high quality, got {}",
            tracker.boundary_adjustment_factor()
        );
    }

    #[test]
    fn test_quality_tracker_reset() {
        let mut tracker = QualityTracker::new(0.95);

        tracker.record(QualityFeedback::from_accuracy(0.9));
        tracker.record(QualityFeedback::from_accuracy(0.9));
        assert_eq!(tracker.history_len(), 2);

        tracker.reset();
        assert_eq!(tracker.history_len(), 0);
    }

    #[test]
    fn test_quality_metric_checks() {
        let metric = QualityMetric {
            avg_score: 0.96,
            min_score: 0.90,
            max_score: 0.99,
            std_dev: 0.02,
            sample_count: 100,
            trend: 0.01,
        };

        assert!(metric.meets_target(0.95));
        assert!(!metric.meets_target(0.97));
        assert!(metric.is_stable(0.05));
        assert!(!metric.is_stable(0.01));
        assert!(metric.is_improving());
    }
}
