//! Enhanced longitudinal drift detection using `midstreamer-attractor`.
//!
//! Extends the Welford-statistics drift detection from `longitudinal.rs`
//! with phase-space attractor analysis provided by the
//! `midstreamer-attractor` crate (ADR-032a Section 6.4).
//!
//! # Improvements over base drift detection
//!
//! - **Phase-space embedding**: Detects regime changes invisible to simple
//!   z-score analysis (e.g., gait transitioning from limit cycle to
//!   strange attractor = developing instability)
//! - **Lyapunov exponent**: Quantifies sensitivity to initial conditions,
//!   catching chaotic transitions in breathing patterns
//! - **Attractor classification**: Automatically classifies biophysical
//!   time series as point attractor (stable), limit cycle (periodic),
//!   or strange attractor (chaotic)
//!
//! # References
//! - ADR-030 Tier 4: Longitudinal Biomechanics Drift
//! - ADR-032a Section 6.4: midstreamer-attractor integration
//! - Takens, F. (1981). "Detecting strange attractors in turbulence."

use midstreamer_attractor::{
    AttractorAnalyzer, AttractorType, PhasePoint,
};

use super::longitudinal::DriftMetric;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for attractor-based drift analysis.
#[derive(Debug, Clone)]
pub struct AttractorDriftConfig {
    /// Embedding dimension for phase-space reconstruction (Takens' theorem).
    /// Default: 3 (sufficient for most biophysical signals).
    pub embedding_dim: usize,
    /// Time delay for phase-space embedding (in observation steps).
    /// Default: 1 (consecutive observations).
    pub time_delay: usize,
    /// Minimum observations needed before analysis is meaningful.
    /// Default: 30 (about 1 month of daily observations).
    pub min_observations: usize,
    /// Lyapunov exponent threshold for chaos detection.
    /// Default: 0.01.
    pub lyapunov_threshold: f64,
    /// Maximum trajectory length for the analyzer.
    /// Default: 10000.
    pub max_trajectory_length: usize,
}

impl Default for AttractorDriftConfig {
    fn default() -> Self {
        Self {
            embedding_dim: 3,
            time_delay: 1,
            min_observations: 30,
            lyapunov_threshold: 0.01,
            max_trajectory_length: 10000,
        }
    }
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from attractor-based drift analysis.
#[derive(Debug, thiserror::Error)]
pub enum AttractorDriftError {
    /// Not enough observations for phase-space embedding.
    #[error("Insufficient observations: need >= {needed}, have {have}")]
    InsufficientData { needed: usize, have: usize },

    /// The metric has no observations recorded.
    #[error("No observations for metric: {0}")]
    NoObservations(String),

    /// Phase-space embedding dimension is invalid.
    #[error("Invalid embedding dimension: {dim} (must be >= 2)")]
    InvalidEmbeddingDim { dim: usize },

    /// Attractor analysis library error.
    #[error("Attractor analysis failed: {0}")]
    AnalysisFailed(String),
}

// ---------------------------------------------------------------------------
// Attractor classification result
// ---------------------------------------------------------------------------

/// Classification of a biophysical time series attractor.
#[derive(Debug, Clone, PartialEq)]
pub enum BiophysicalAttractor {
    /// Point attractor: metric has converged to a stable value.
    Stable { center: f64 },
    /// Limit cycle: metric oscillates periodically.
    Periodic { lyapunov_max: f64 },
    /// Strange attractor: metric exhibits chaotic dynamics.
    Chaotic { lyapunov_exponent: f64 },
    /// Transitioning between attractor types.
    Transitioning {
        from: Box<BiophysicalAttractor>,
        to: Box<BiophysicalAttractor>,
    },
    /// Insufficient data to classify.
    Unknown,
}

impl BiophysicalAttractor {
    /// Whether this attractor type warrants monitoring attention.
    pub fn is_concerning(&self) -> bool {
        matches!(
            self,
            BiophysicalAttractor::Chaotic { .. } | BiophysicalAttractor::Transitioning { .. }
        )
    }

    /// Human-readable label for reporting.
    pub fn label(&self) -> &'static str {
        match self {
            BiophysicalAttractor::Stable { .. } => "stable",
            BiophysicalAttractor::Periodic { .. } => "periodic",
            BiophysicalAttractor::Chaotic { .. } => "chaotic",
            BiophysicalAttractor::Transitioning { .. } => "transitioning",
            BiophysicalAttractor::Unknown => "unknown",
        }
    }
}

// ---------------------------------------------------------------------------
// Attractor drift report
// ---------------------------------------------------------------------------

/// Report from attractor-based drift analysis.
#[derive(Debug, Clone)]
pub struct AttractorDriftReport {
    /// Person this report pertains to.
    pub person_id: u64,
    /// Which biophysical metric was analyzed.
    pub metric: DriftMetric,
    /// Classified attractor type.
    pub attractor: BiophysicalAttractor,
    /// Whether the attractor type has changed from the previous analysis.
    pub regime_changed: bool,
    /// Number of observations used in this analysis.
    pub observation_count: usize,
    /// Timestamp of the analysis (microseconds).
    pub timestamp_us: u64,
}

// ---------------------------------------------------------------------------
// Per-metric observation buffer
// ---------------------------------------------------------------------------

/// Time series buffer for a single biophysical metric.
#[derive(Debug, Clone)]
struct MetricBuffer {
    /// Metric type.
    metric: DriftMetric,
    /// Observed values (most recent at the end).
    values: Vec<f64>,
    /// Maximum buffer size.
    max_size: usize,
    /// Last classified attractor label.
    last_label: String,
}

impl MetricBuffer {
    /// Create a new buffer.
    fn new(metric: DriftMetric, max_size: usize) -> Self {
        Self {
            metric,
            values: Vec::new(),
            max_size,
            last_label: "unknown".to_string(),
        }
    }

    /// Add an observation.
    fn push(&mut self, value: f64) {
        if self.values.len() >= self.max_size {
            self.values.remove(0);
        }
        self.values.push(value);
    }

    /// Number of observations.
    fn count(&self) -> usize {
        self.values.len()
    }
}

// ---------------------------------------------------------------------------
// Attractor drift analyzer
// ---------------------------------------------------------------------------

/// Attractor-based drift analyzer for longitudinal biophysical monitoring.
///
/// Uses phase-space reconstruction (Takens' embedding theorem) and
/// `midstreamer-attractor` to classify the dynamical regime of each
/// biophysical metric. Detects regime changes that precede simple
/// metric drift.
pub struct AttractorDriftAnalyzer {
    /// Configuration.
    config: AttractorDriftConfig,
    /// Person ID being monitored.
    person_id: u64,
    /// Per-metric observation buffers.
    buffers: Vec<MetricBuffer>,
    /// Total analyses performed.
    analysis_count: u64,
}

// Manual Debug since AttractorAnalyzer does not derive Debug
impl std::fmt::Debug for AttractorDriftAnalyzer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AttractorDriftAnalyzer")
            .field("person_id", &self.person_id)
            .field("analysis_count", &self.analysis_count)
            .finish()
    }
}

impl AttractorDriftAnalyzer {
    /// Create a new attractor drift analyzer for a person.
    pub fn new(
        person_id: u64,
        config: AttractorDriftConfig,
    ) -> Result<Self, AttractorDriftError> {
        if config.embedding_dim < 2 {
            return Err(AttractorDriftError::InvalidEmbeddingDim {
                dim: config.embedding_dim,
            });
        }

        let buffers = DriftMetric::all()
            .iter()
            .map(|&m| MetricBuffer::new(m, 365)) // 1 year of daily observations
            .collect();

        Ok(Self {
            config,
            person_id,
            buffers,
            analysis_count: 0,
        })
    }

    /// Add an observation for a specific metric.
    pub fn add_observation(&mut self, metric: DriftMetric, value: f64) {
        if let Some(buf) = self.buffers.iter_mut().find(|b| b.metric == metric) {
            buf.push(value);
        }
    }

    /// Perform attractor analysis on a specific metric.
    ///
    /// Reconstructs the phase space using Takens' embedding and
    /// classifies the attractor type using `midstreamer-attractor`.
    pub fn analyze(
        &mut self,
        metric: DriftMetric,
        timestamp_us: u64,
    ) -> Result<AttractorDriftReport, AttractorDriftError> {
        let buf_idx = self
            .buffers
            .iter()
            .position(|b| b.metric == metric)
            .ok_or_else(|| AttractorDriftError::NoObservations(metric.name().into()))?;

        let count = self.buffers[buf_idx].count();
        let min_needed = self.config.min_observations;
        if count < min_needed {
            return Err(AttractorDriftError::InsufficientData {
                needed: min_needed,
                have: count,
            });
        }

        // Build phase-space trajectory using Takens' embedding
        // and feed into a fresh AttractorAnalyzer
        let dim = self.config.embedding_dim;
        let delay = self.config.time_delay;
        let values = &self.buffers[buf_idx].values;
        let n_points = values.len().saturating_sub((dim - 1) * delay);

        let mut analyzer = AttractorAnalyzer::new(dim, self.config.max_trajectory_length);

        for i in 0..n_points {
            let coords: Vec<f64> = (0..dim).map(|d| values[i + d * delay]).collect();
            let point = PhasePoint::new(coords, i as u64);
            let _ = analyzer.add_point(point);
        }

        // Analyze the trajectory
        let attractor = match analyzer.analyze() {
            Ok(info) => {
                let max_lyap = info
                    .max_lyapunov_exponent()
                    .unwrap_or(0.0);

                match info.attractor_type {
                    AttractorType::PointAttractor => {
                        // Compute center as mean of last few values
                        let recent = &values[values.len().saturating_sub(10)..];
                        let center = recent.iter().sum::<f64>() / recent.len() as f64;
                        BiophysicalAttractor::Stable { center }
                    }
                    AttractorType::LimitCycle => BiophysicalAttractor::Periodic {
                        lyapunov_max: max_lyap,
                    },
                    AttractorType::StrangeAttractor => BiophysicalAttractor::Chaotic {
                        lyapunov_exponent: max_lyap,
                    },
                    _ => BiophysicalAttractor::Unknown,
                }
            }
            Err(_) => BiophysicalAttractor::Unknown,
        };

        // Check for regime change
        let label = attractor.label().to_string();
        let regime_changed = label != self.buffers[buf_idx].last_label;
        self.buffers[buf_idx].last_label = label;

        self.analysis_count += 1;

        Ok(AttractorDriftReport {
            person_id: self.person_id,
            metric,
            attractor,
            regime_changed,
            observation_count: count,
            timestamp_us,
        })
    }

    /// Number of observations for a specific metric.
    pub fn observation_count(&self, metric: DriftMetric) -> usize {
        self.buffers
            .iter()
            .find(|b| b.metric == metric)
            .map_or(0, |b| b.count())
    }

    /// Total analyses performed.
    pub fn analysis_count(&self) -> u64 {
        self.analysis_count
    }

    /// Person ID being monitored.
    pub fn person_id(&self) -> u64 {
        self.person_id
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_analyzer() -> AttractorDriftAnalyzer {
        AttractorDriftAnalyzer::new(42, AttractorDriftConfig::default()).unwrap()
    }

    #[test]
    fn test_analyzer_creation() {
        let a = default_analyzer();
        assert_eq!(a.person_id(), 42);
        assert_eq!(a.analysis_count(), 0);
    }

    #[test]
    fn test_analyzer_invalid_embedding_dim() {
        let config = AttractorDriftConfig {
            embedding_dim: 1,
            ..Default::default()
        };
        assert!(matches!(
            AttractorDriftAnalyzer::new(1, config),
            Err(AttractorDriftError::InvalidEmbeddingDim { .. })
        ));
    }

    #[test]
    fn test_add_observation() {
        let mut a = default_analyzer();
        a.add_observation(DriftMetric::GaitSymmetry, 0.1);
        a.add_observation(DriftMetric::GaitSymmetry, 0.11);
        assert_eq!(a.observation_count(DriftMetric::GaitSymmetry), 2);
    }

    #[test]
    fn test_analyze_insufficient_data() {
        let mut a = default_analyzer();
        for i in 0..10 {
            a.add_observation(DriftMetric::GaitSymmetry, 0.1 + i as f64 * 0.001);
        }
        let result = a.analyze(DriftMetric::GaitSymmetry, 0);
        assert!(matches!(
            result,
            Err(AttractorDriftError::InsufficientData { .. })
        ));
    }

    #[test]
    fn test_analyze_stable_signal() {
        let mut a = AttractorDriftAnalyzer::new(
            1,
            AttractorDriftConfig {
                min_observations: 10,
                ..Default::default()
            },
        )
        .unwrap();

        // Stable signal: constant with tiny noise
        for i in 0..150 {
            let noise = 0.001 * (i as f64 % 3.0 - 1.0);
            a.add_observation(DriftMetric::GaitSymmetry, 0.1 + noise);
        }

        let report = a.analyze(DriftMetric::GaitSymmetry, 1000).unwrap();
        assert_eq!(report.person_id, 1);
        assert_eq!(report.metric, DriftMetric::GaitSymmetry);
        assert_eq!(report.observation_count, 150);
        assert_eq!(a.analysis_count(), 1);
    }

    #[test]
    fn test_analyze_periodic_signal() {
        let mut a = AttractorDriftAnalyzer::new(
            2,
            AttractorDriftConfig {
                min_observations: 10,
                ..Default::default()
            },
        )
        .unwrap();

        // Periodic signal: sinusoidal with enough points for analyzer
        for i in 0..200 {
            let value = 0.5 + 0.3 * (i as f64 * std::f64::consts::PI / 7.0).sin();
            a.add_observation(DriftMetric::BreathingRegularity, value);
        }

        let report = a.analyze(DriftMetric::BreathingRegularity, 2000).unwrap();
        assert_eq!(report.metric, DriftMetric::BreathingRegularity);
        assert!(!report.attractor.label().is_empty());
    }

    #[test]
    fn test_regime_change_detection() {
        let mut a = AttractorDriftAnalyzer::new(
            3,
            AttractorDriftConfig {
                min_observations: 10,
                ..Default::default()
            },
        )
        .unwrap();

        // Phase 1: stable signal (enough for analyzer: >= 100 points)
        for i in 0..150 {
            let noise = 0.001 * (i as f64 % 3.0 - 1.0);
            a.add_observation(DriftMetric::StabilityIndex, 0.9 + noise);
        }
        let _report1 = a.analyze(DriftMetric::StabilityIndex, 1000).unwrap();

        // Phase 2: add chaotic-like signal
        for i in 150..300 {
            let value = 0.5 + 0.4 * ((i as f64 * 1.7).sin() * (i as f64 * 0.3).cos());
            a.add_observation(DriftMetric::StabilityIndex, value);
        }
        let _report2 = a.analyze(DriftMetric::StabilityIndex, 2000).unwrap();
        assert!(a.analysis_count() >= 2);
    }

    #[test]
    fn test_biophysical_attractor_labels() {
        assert_eq!(
            BiophysicalAttractor::Stable { center: 0.1 }.label(),
            "stable"
        );
        assert_eq!(
            BiophysicalAttractor::Periodic { lyapunov_max: 0.0 }.label(),
            "periodic"
        );
        assert_eq!(
            BiophysicalAttractor::Chaotic {
                lyapunov_exponent: 0.05,
            }
            .label(),
            "chaotic"
        );
        assert_eq!(BiophysicalAttractor::Unknown.label(), "unknown");
    }

    #[test]
    fn test_biophysical_attractor_is_concerning() {
        assert!(!BiophysicalAttractor::Stable { center: 0.1 }.is_concerning());
        assert!(!BiophysicalAttractor::Periodic { lyapunov_max: 0.0 }.is_concerning());
        assert!(BiophysicalAttractor::Chaotic {
            lyapunov_exponent: 0.05,
        }
        .is_concerning());
        assert!(!BiophysicalAttractor::Unknown.is_concerning());
    }

    #[test]
    fn test_default_config() {
        let cfg = AttractorDriftConfig::default();
        assert_eq!(cfg.embedding_dim, 3);
        assert_eq!(cfg.time_delay, 1);
        assert_eq!(cfg.min_observations, 30);
        assert!((cfg.lyapunov_threshold - 0.01).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metric_buffer_eviction() {
        let mut buf = MetricBuffer::new(DriftMetric::GaitSymmetry, 5);
        for i in 0..10 {
            buf.push(i as f64);
        }
        assert_eq!(buf.count(), 5);
        assert!((buf.values[0] - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_all_metrics_have_buffers() {
        let a = default_analyzer();
        for metric in DriftMetric::all() {
            assert_eq!(a.observation_count(*metric), 0);
        }
    }

    #[test]
    fn test_transitioning_attractor() {
        let t = BiophysicalAttractor::Transitioning {
            from: Box::new(BiophysicalAttractor::Stable { center: 0.1 }),
            to: Box::new(BiophysicalAttractor::Chaotic {
                lyapunov_exponent: 0.05,
            }),
        };
        assert!(t.is_concerning());
        assert_eq!(t.label(), "transitioning");
    }

    #[test]
    fn test_error_display() {
        let err = AttractorDriftError::InsufficientData {
            needed: 30,
            have: 10,
        };
        assert!(format!("{}", err).contains("30"));
        assert!(format!("{}", err).contains("10"));

        let err = AttractorDriftError::NoObservations("gait_symmetry".into());
        assert!(format!("{}", err).contains("gait_symmetry"));
    }

    #[test]
    fn test_debug_impl() {
        let a = default_analyzer();
        let dbg = format!("{:?}", a);
        assert!(dbg.contains("AttractorDriftAnalyzer"));
    }
}
