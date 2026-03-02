//! Longitudinal biomechanics drift detection.
//!
//! Maintains per-person biophysical baselines over days/weeks using Welford
//! online statistics. Detects meaningful drift in gait symmetry, stability,
//! breathing regularity, micro-tremor, and activity level. Produces traceable
//! evidence reports that link to stored embedding trajectories.
//!
//! # Key Invariants
//! - Baseline requires >= 7 observation days before drift detection activates
//! - Drift alert requires > 2-sigma deviation sustained for >= 3 consecutive days
//! - Output is metric values and deviations, never diagnostic language
//! - Welford statistics use full history (no windowing) for stability
//!
//! # References
//! - Welford, B.P. (1962). "Note on a Method for Calculating Corrected
//!   Sums of Squares." Technometrics.
//! - ADR-030 Tier 4: Longitudinal Biomechanics Drift

use crate::ruvsense::field_model::WelfordStats;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from longitudinal monitoring operations.
#[derive(Debug, thiserror::Error)]
pub enum LongitudinalError {
    /// Not enough observation days for drift detection.
    #[error("Insufficient observation days: need >= {needed}, got {got}")]
    InsufficientDays { needed: u32, got: u32 },

    /// Person ID not found in the registry.
    #[error("Unknown person ID: {0}")]
    UnknownPerson(u64),

    /// Embedding dimension mismatch.
    #[error("Embedding dimension mismatch: expected {expected}, got {got}")]
    EmbeddingDimensionMismatch { expected: usize, got: usize },

    /// Invalid metric value.
    #[error("Invalid metric value for {metric}: {reason}")]
    InvalidMetric { metric: String, reason: String },
}

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// Biophysical metric types tracked per person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DriftMetric {
    /// Gait symmetry ratio (0.0 = perfectly symmetric, higher = asymmetric).
    GaitSymmetry,
    /// Stability index (lower = less stable).
    StabilityIndex,
    /// Breathing regularity (coefficient of variation of breath intervals).
    BreathingRegularity,
    /// Micro-tremor amplitude (mm, from high-frequency pose jitter).
    MicroTremor,
    /// Daily activity level (normalized 0-1).
    ActivityLevel,
}

impl DriftMetric {
    /// All metric variants.
    pub fn all() -> &'static [DriftMetric] {
        &[
            DriftMetric::GaitSymmetry,
            DriftMetric::StabilityIndex,
            DriftMetric::BreathingRegularity,
            DriftMetric::MicroTremor,
            DriftMetric::ActivityLevel,
        ]
    }

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            DriftMetric::GaitSymmetry => "gait_symmetry",
            DriftMetric::StabilityIndex => "stability_index",
            DriftMetric::BreathingRegularity => "breathing_regularity",
            DriftMetric::MicroTremor => "micro_tremor",
            DriftMetric::ActivityLevel => "activity_level",
        }
    }
}

/// Direction of drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftDirection {
    /// Metric is increasing relative to baseline.
    Increasing,
    /// Metric is decreasing relative to baseline.
    Decreasing,
}

/// Monitoring level for drift reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MonitoringLevel {
    /// Level 1: Raw biophysical metric value.
    Physiological = 1,
    /// Level 2: Personal baseline deviation.
    Drift = 2,
    /// Level 3: Pattern-matched risk correlation.
    RiskCorrelation = 3,
}

/// A drift report with traceable evidence.
#[derive(Debug, Clone)]
pub struct DriftReport {
    /// Person this report pertains to.
    pub person_id: u64,
    /// Which metric drifted.
    pub metric: DriftMetric,
    /// Direction of drift.
    pub direction: DriftDirection,
    /// Z-score relative to personal baseline.
    pub z_score: f64,
    /// Current metric value (today or most recent).
    pub current_value: f64,
    /// Baseline mean for this metric.
    pub baseline_mean: f64,
    /// Baseline standard deviation.
    pub baseline_std: f64,
    /// Number of consecutive days the drift has been sustained.
    pub sustained_days: u32,
    /// Monitoring level.
    pub level: MonitoringLevel,
    /// Timestamp (microseconds) when this report was generated.
    pub timestamp_us: u64,
}

/// Daily metric summary for one person.
#[derive(Debug, Clone)]
pub struct DailyMetricSummary {
    /// Person ID.
    pub person_id: u64,
    /// Day timestamp (start of day, microseconds).
    pub day_us: u64,
    /// Metric values for this day.
    pub metrics: Vec<(DriftMetric, f64)>,
    /// AETHER embedding centroid for this day.
    pub embedding_centroid: Option<Vec<f32>>,
}

// ---------------------------------------------------------------------------
// Personal baseline
// ---------------------------------------------------------------------------

/// Per-person longitudinal baseline with Welford statistics.
///
/// Tracks running mean and variance for each biophysical metric over
/// the person's entire observation history. Uses Welford's algorithm
/// for numerical stability.
#[derive(Debug, Clone)]
pub struct PersonalBaseline {
    /// Unique person identifier.
    pub person_id: u64,
    /// Per-metric Welford accumulators.
    pub gait_symmetry: WelfordStats,
    pub stability_index: WelfordStats,
    pub breathing_regularity: WelfordStats,
    pub micro_tremor: WelfordStats,
    pub activity_level: WelfordStats,
    /// Running centroid of AETHER embeddings.
    pub embedding_centroid: Vec<f32>,
    /// Number of observation days.
    pub observation_days: u32,
    /// Timestamp of last update (microseconds).
    pub updated_at_us: u64,
    /// Per-metric consecutive drift days counter.
    drift_counters: [u32; 5],
}

impl PersonalBaseline {
    /// Create a new baseline for a person.
    ///
    /// `embedding_dim` is typically 128 for AETHER embeddings.
    pub fn new(person_id: u64, embedding_dim: usize) -> Self {
        Self {
            person_id,
            gait_symmetry: WelfordStats::new(),
            stability_index: WelfordStats::new(),
            breathing_regularity: WelfordStats::new(),
            micro_tremor: WelfordStats::new(),
            activity_level: WelfordStats::new(),
            embedding_centroid: vec![0.0; embedding_dim],
            observation_days: 0,
            updated_at_us: 0,
            drift_counters: [0; 5],
        }
    }

    /// Get the Welford stats for a specific metric.
    pub fn stats_for(&self, metric: DriftMetric) -> &WelfordStats {
        match metric {
            DriftMetric::GaitSymmetry => &self.gait_symmetry,
            DriftMetric::StabilityIndex => &self.stability_index,
            DriftMetric::BreathingRegularity => &self.breathing_regularity,
            DriftMetric::MicroTremor => &self.micro_tremor,
            DriftMetric::ActivityLevel => &self.activity_level,
        }
    }

    /// Get mutable Welford stats for a specific metric.
    fn stats_for_mut(&mut self, metric: DriftMetric) -> &mut WelfordStats {
        match metric {
            DriftMetric::GaitSymmetry => &mut self.gait_symmetry,
            DriftMetric::StabilityIndex => &mut self.stability_index,
            DriftMetric::BreathingRegularity => &mut self.breathing_regularity,
            DriftMetric::MicroTremor => &mut self.micro_tremor,
            DriftMetric::ActivityLevel => &mut self.activity_level,
        }
    }

    /// Index of a metric in the drift_counters array.
    fn metric_index(metric: DriftMetric) -> usize {
        match metric {
            DriftMetric::GaitSymmetry => 0,
            DriftMetric::StabilityIndex => 1,
            DriftMetric::BreathingRegularity => 2,
            DriftMetric::MicroTremor => 3,
            DriftMetric::ActivityLevel => 4,
        }
    }

    /// Whether baseline has enough data for drift detection.
    pub fn is_ready(&self) -> bool {
        self.observation_days >= 7
    }

    /// Update baseline with a daily summary.
    ///
    /// Returns drift reports for any metrics that exceed thresholds.
    pub fn update_daily(
        &mut self,
        summary: &DailyMetricSummary,
        timestamp_us: u64,
    ) -> Vec<DriftReport> {
        self.observation_days += 1;
        self.updated_at_us = timestamp_us;

        // Update embedding centroid with EMA (decay = 0.95)
        if let Some(ref emb) = summary.embedding_centroid {
            if emb.len() == self.embedding_centroid.len() {
                let alpha = 0.05_f32; // 1 - 0.95
                for (c, e) in self.embedding_centroid.iter_mut().zip(emb.iter()) {
                    *c = (1.0 - alpha) * *c + alpha * *e;
                }
            }
        }

        let mut reports = Vec::new();

        let observation_days = self.observation_days;

        for &(metric, value) in &summary.metrics {
            // Update stats and extract values before releasing the mutable borrow
            let (z, baseline_mean, baseline_std) = {
                let stats = self.stats_for_mut(metric);
                stats.update(value);
                let z = stats.z_score(value);
                let mean = stats.mean;
                let std = stats.std_dev();
                (z, mean, std)
            };

            if !self.is_ready_at(observation_days) {
                continue;
            }

            let idx = Self::metric_index(metric);

            if z.abs() > 2.0 {
                self.drift_counters[idx] += 1;
            } else {
                self.drift_counters[idx] = 0;
            }

            if self.drift_counters[idx] >= 3 {
                let direction = if z > 0.0 {
                    DriftDirection::Increasing
                } else {
                    DriftDirection::Decreasing
                };

                let level = if self.drift_counters[idx] >= 7 {
                    MonitoringLevel::RiskCorrelation
                } else {
                    MonitoringLevel::Drift
                };

                reports.push(DriftReport {
                    person_id: self.person_id,
                    metric,
                    direction,
                    z_score: z,
                    current_value: value,
                    baseline_mean,
                    baseline_std,
                    sustained_days: self.drift_counters[idx],
                    level,
                    timestamp_us,
                });
            }
        }

        reports
    }

    /// Check readiness at a specific observation day count (internal helper).
    fn is_ready_at(&self, days: u32) -> bool {
        days >= 7
    }

    /// Get current drift counter for a metric.
    pub fn drift_days(&self, metric: DriftMetric) -> u32 {
        self.drift_counters[Self::metric_index(metric)]
    }
}

// ---------------------------------------------------------------------------
// Embedding history (simplified HNSW-indexed store)
// ---------------------------------------------------------------------------

/// Entry in the embedding history.
#[derive(Debug, Clone)]
pub struct EmbeddingEntry {
    /// Person ID.
    pub person_id: u64,
    /// Day timestamp (microseconds).
    pub day_us: u64,
    /// AETHER embedding vector.
    pub embedding: Vec<f32>,
}

/// Simplified embedding history store for longitudinal tracking.
///
/// In production, this would be backed by an HNSW index for fast
/// nearest-neighbor search. This implementation uses brute-force
/// cosine similarity for correctness.
#[derive(Debug)]
pub struct EmbeddingHistory {
    entries: Vec<EmbeddingEntry>,
    max_entries: usize,
    embedding_dim: usize,
}

impl EmbeddingHistory {
    /// Create a new embedding history store.
    pub fn new(embedding_dim: usize, max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
            embedding_dim,
        }
    }

    /// Add an embedding entry.
    pub fn push(&mut self, entry: EmbeddingEntry) -> Result<(), LongitudinalError> {
        if entry.embedding.len() != self.embedding_dim {
            return Err(LongitudinalError::EmbeddingDimensionMismatch {
                expected: self.embedding_dim,
                got: entry.embedding.len(),
            });
        }
        if self.entries.len() >= self.max_entries {
            self.entries.drain(..1); // FIFO eviction — acceptable for daily-rate inserts
        }
        self.entries.push(entry);
        Ok(())
    }

    /// Find the K nearest embeddings to a query vector (brute-force cosine).
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(usize, f32)> {
        let mut similarities: Vec<(usize, f32)> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (i, cosine_similarity(query, &e.embedding)))
            .collect();

        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        similarities.truncate(k);
        similarities
    }

    /// Number of entries stored.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get entry by index.
    pub fn get(&self, index: usize) -> Option<&EmbeddingEntry> {
        self.entries.get(index)
    }

    /// Get all entries for a specific person.
    pub fn entries_for_person(&self, person_id: u64) -> Vec<&EmbeddingEntry> {
        self.entries
            .iter()
            .filter(|e| e.person_id == person_id)
            .collect()
    }
}

/// Cosine similarity between two f32 vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    let denom = norm_a * norm_b;
    if denom < 1e-9 {
        0.0
    } else {
        dot / denom
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_daily_summary(person_id: u64, day: u64, values: [f64; 5]) -> DailyMetricSummary {
        DailyMetricSummary {
            person_id,
            day_us: day * 86_400_000_000,
            metrics: vec![
                (DriftMetric::GaitSymmetry, values[0]),
                (DriftMetric::StabilityIndex, values[1]),
                (DriftMetric::BreathingRegularity, values[2]),
                (DriftMetric::MicroTremor, values[3]),
                (DriftMetric::ActivityLevel, values[4]),
            ],
            embedding_centroid: None,
        }
    }

    #[test]
    fn test_personal_baseline_creation() {
        let baseline = PersonalBaseline::new(42, 128);
        assert_eq!(baseline.person_id, 42);
        assert_eq!(baseline.observation_days, 0);
        assert!(!baseline.is_ready());
        assert_eq!(baseline.embedding_centroid.len(), 128);
    }

    #[test]
    fn test_baseline_not_ready_before_7_days() {
        let mut baseline = PersonalBaseline::new(1, 128);
        for day in 0..6 {
            let summary = make_daily_summary(1, day, [0.1, 0.9, 0.15, 0.5, 0.7]);
            let reports = baseline.update_daily(&summary, day * 86_400_000_000);
            assert!(reports.is_empty(), "No drift before 7 days");
        }
        assert!(!baseline.is_ready());
    }

    #[test]
    fn test_baseline_ready_after_7_days() {
        let mut baseline = PersonalBaseline::new(1, 128);
        for day in 0..7 {
            let summary = make_daily_summary(1, day, [0.1, 0.9, 0.15, 0.5, 0.7]);
            baseline.update_daily(&summary, day * 86_400_000_000);
        }
        assert!(baseline.is_ready());
        assert_eq!(baseline.observation_days, 7);
    }

    #[test]
    fn test_stable_metrics_no_drift() {
        let mut baseline = PersonalBaseline::new(1, 128);

        // 20 days of stable metrics
        for day in 0..20 {
            let summary = make_daily_summary(1, day, [0.1, 0.9, 0.15, 0.5, 0.7]);
            let reports = baseline.update_daily(&summary, day * 86_400_000_000);
            assert!(
                reports.is_empty(),
                "Stable metrics should not trigger drift"
            );
        }
    }

    #[test]
    fn test_drift_detected_after_sustained_deviation() {
        let mut baseline = PersonalBaseline::new(1, 128);

        // 30 days of very stable gait symmetry = 0.1 with tiny noise
        // (more baseline days = stronger prior, so drift stays > 2-sigma longer)
        for day in 0..30 {
            let noise = 0.001 * (day as f64 % 3.0 - 1.0); // tiny variation
            let summary = make_daily_summary(1, day, [0.1 + noise, 0.9, 0.15, 0.5, 0.7]);
            baseline.update_daily(&summary, day * 86_400_000_000);
        }

        // Now inject a very large drift in gait symmetry (0.1 -> 5.0) for 5 days.
        // Even as Welford accumulates these, the z-score should stay well above 2.0
        // because 30 baseline days anchor the mean near 0.1 with small std dev.
        let mut any_drift = false;
        for day in 30..36 {
            let summary = make_daily_summary(1, day, [5.0, 0.9, 0.15, 0.5, 0.7]);
            let reports = baseline.update_daily(&summary, day * 86_400_000_000);
            if !reports.is_empty() {
                any_drift = true;
                let r = &reports[0];
                assert_eq!(r.metric, DriftMetric::GaitSymmetry);
                assert_eq!(r.direction, DriftDirection::Increasing);
                assert!(r.z_score > 2.0);
                assert!(r.sustained_days >= 3);
            }
        }
        assert!(any_drift, "Should detect drift after sustained deviation");
    }

    #[test]
    fn test_drift_resolves_when_metric_returns() {
        let mut baseline = PersonalBaseline::new(1, 128);

        // Stable baseline
        for day in 0..10 {
            let summary = make_daily_summary(1, day, [0.1, 0.9, 0.15, 0.5, 0.7]);
            baseline.update_daily(&summary, day * 86_400_000_000);
        }

        // Drift for 3 days
        for day in 10..13 {
            let summary = make_daily_summary(1, day, [0.9, 0.9, 0.15, 0.5, 0.7]);
            baseline.update_daily(&summary, day * 86_400_000_000);
        }

        // Return to normal
        for day in 13..16 {
            let summary = make_daily_summary(1, day, [0.1, 0.9, 0.15, 0.5, 0.7]);
            let reports = baseline.update_daily(&summary, day * 86_400_000_000);
            // After returning to normal, drift counter resets
            if day == 15 {
                assert!(reports.is_empty(), "Drift should resolve");
                assert_eq!(baseline.drift_days(DriftMetric::GaitSymmetry), 0);
            }
        }
    }

    #[test]
    fn test_monitoring_level_escalation() {
        let mut baseline = PersonalBaseline::new(1, 128);

        // 30 days of stable baseline with tiny noise to anchor stats
        for day in 0..30 {
            let noise = 0.001 * (day as f64 % 3.0 - 1.0);
            let summary = make_daily_summary(1, day, [0.1 + noise, 0.9, 0.15, 0.5, 0.7]);
            baseline.update_daily(&summary, day * 86_400_000_000);
        }

        // Sustained massive drift for 10+ days should escalate to RiskCorrelation.
        // Using value 10.0 (vs baseline ~0.1) to ensure z-score stays well above 2.0
        // even as Welford accumulates the drifted values.
        let mut max_level = MonitoringLevel::Physiological;
        for day in 30..42 {
            let summary = make_daily_summary(1, day, [10.0, 0.9, 0.15, 0.5, 0.7]);
            let reports = baseline.update_daily(&summary, day * 86_400_000_000);
            for r in &reports {
                if r.level > max_level {
                    max_level = r.level;
                }
            }
        }
        assert_eq!(
            max_level,
            MonitoringLevel::RiskCorrelation,
            "7+ days sustained drift should reach RiskCorrelation level"
        );
    }

    #[test]
    fn test_embedding_history_push_and_search() {
        let mut history = EmbeddingHistory::new(4, 100);

        history
            .push(EmbeddingEntry {
                person_id: 1,
                day_us: 0,
                embedding: vec![1.0, 0.0, 0.0, 0.0],
            })
            .unwrap();
        history
            .push(EmbeddingEntry {
                person_id: 1,
                day_us: 1,
                embedding: vec![0.9, 0.1, 0.0, 0.0],
            })
            .unwrap();
        history
            .push(EmbeddingEntry {
                person_id: 2,
                day_us: 0,
                embedding: vec![0.0, 0.0, 1.0, 0.0],
            })
            .unwrap();

        let results = history.search(&[1.0, 0.0, 0.0, 0.0], 2);
        assert_eq!(results.len(), 2);
        // First result should be exact match
        assert!((results[0].1 - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_embedding_history_dimension_mismatch() {
        let mut history = EmbeddingHistory::new(4, 100);
        let result = history.push(EmbeddingEntry {
            person_id: 1,
            day_us: 0,
            embedding: vec![1.0, 0.0], // wrong dim
        });
        assert!(matches!(
            result,
            Err(LongitudinalError::EmbeddingDimensionMismatch { .. })
        ));
    }

    #[test]
    fn test_embedding_history_fifo_eviction() {
        let mut history = EmbeddingHistory::new(2, 3);
        for i in 0..5 {
            history
                .push(EmbeddingEntry {
                    person_id: 1,
                    day_us: i,
                    embedding: vec![i as f32, 0.0],
                })
                .unwrap();
        }
        assert_eq!(history.len(), 3);
        // First entry should be day 2 (0 and 1 evicted)
        assert_eq!(history.get(0).unwrap().day_us, 2);
    }

    #[test]
    fn test_entries_for_person() {
        let mut history = EmbeddingHistory::new(2, 100);
        history
            .push(EmbeddingEntry {
                person_id: 1,
                day_us: 0,
                embedding: vec![1.0, 0.0],
            })
            .unwrap();
        history
            .push(EmbeddingEntry {
                person_id: 2,
                day_us: 0,
                embedding: vec![0.0, 1.0],
            })
            .unwrap();
        history
            .push(EmbeddingEntry {
                person_id: 1,
                day_us: 1,
                embedding: vec![0.9, 0.1],
            })
            .unwrap();

        let entries = history.entries_for_person(1);
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_drift_metric_names() {
        assert_eq!(DriftMetric::GaitSymmetry.name(), "gait_symmetry");
        assert_eq!(DriftMetric::ActivityLevel.name(), "activity_level");
        assert_eq!(DriftMetric::all().len(), 5);
    }

    #[test]
    fn test_cosine_similarity_unit_vectors() {
        let a = vec![1.0_f32, 0.0, 0.0];
        let b = vec![0.0_f32, 1.0, 0.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-6, "Orthogonal = 0");

        let c = vec![1.0_f32, 0.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 1.0).abs() < 1e-6, "Same = 1");
    }
}
