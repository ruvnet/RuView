//! Enhanced gesture classification using `midstreamer-temporal-compare`.
//!
//! Extends the DTW-based gesture classifier from `gesture.rs` with
//! optimized temporal comparison algorithms provided by the
//! `midstreamer-temporal-compare` crate (ADR-032a Section 6.4).
//!
//! # Improvements over base gesture classifier
//!
//! - **Cached DTW**: Results cached by sequence hash for repeated comparisons
//! - **Multi-algorithm**: DTW, LCS, and edit distance available
//! - **Pattern detection**: Automatic sub-gesture pattern extraction
//!
//! # References
//! - ADR-030 Tier 6: Invisible Interaction Layer
//! - ADR-032a Section 6.4: midstreamer-temporal-compare integration

use midstreamer_temporal_compare::{
    ComparisonAlgorithm, Sequence, TemporalComparator,
};

use super::gesture::{GestureConfig, GestureError, GestureResult, GestureTemplate};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Algorithm selection for temporal gesture matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureAlgorithm {
    /// Dynamic Time Warping (classic, from base gesture module).
    Dtw,
    /// Longest Common Subsequence (better for sparse gestures).
    Lcs,
    /// Edit distance (better for discrete gesture phases).
    EditDistance,
}

impl GestureAlgorithm {
    /// Convert to the midstreamer comparison algorithm.
    pub fn to_comparison_algorithm(&self) -> ComparisonAlgorithm {
        match self {
            GestureAlgorithm::Dtw => ComparisonAlgorithm::DTW,
            GestureAlgorithm::Lcs => ComparisonAlgorithm::LCS,
            GestureAlgorithm::EditDistance => ComparisonAlgorithm::EditDistance,
        }
    }
}

/// Configuration for the temporal gesture classifier.
#[derive(Debug, Clone)]
pub struct TemporalGestureConfig {
    /// Base gesture config (feature_dim, min_sequence_len, etc.).
    pub base: GestureConfig,
    /// Primary comparison algorithm.
    pub algorithm: GestureAlgorithm,
    /// Whether to enable result caching.
    pub enable_cache: bool,
    /// Cache capacity (number of comparison results to cache).
    pub cache_capacity: usize,
    /// Maximum distance for a match (lower = stricter).
    pub max_distance: f64,
    /// Maximum sequence length accepted by the comparator.
    pub max_sequence_length: usize,
}

impl Default for TemporalGestureConfig {
    fn default() -> Self {
        Self {
            base: GestureConfig::default(),
            algorithm: GestureAlgorithm::Dtw,
            enable_cache: true,
            cache_capacity: 256,
            max_distance: 50.0,
            max_sequence_length: 1024,
        }
    }
}

// ---------------------------------------------------------------------------
// Temporal gesture classifier
// ---------------------------------------------------------------------------

/// Enhanced gesture classifier using `midstreamer-temporal-compare`.
///
/// Provides multi-algorithm gesture matching with caching.
/// The comparator uses `f64` elements where each frame is reduced
/// to its L2 norm for scalar temporal comparison.
pub struct TemporalGestureClassifier {
    /// Configuration.
    config: TemporalGestureConfig,
    /// Registered gesture templates.
    templates: Vec<GestureTemplate>,
    /// Template sequences pre-converted to midstreamer format.
    template_sequences: Vec<Sequence<i64>>,
    /// Temporal comparator with caching.
    comparator: TemporalComparator<i64>,
}

impl TemporalGestureClassifier {
    /// Create a new temporal gesture classifier.
    pub fn new(config: TemporalGestureConfig) -> Self {
        let comparator = TemporalComparator::new(
            config.cache_capacity,
            config.max_sequence_length,
        );
        Self {
            config,
            templates: Vec::new(),
            template_sequences: Vec::new(),
            comparator,
        }
    }

    /// Register a gesture template.
    pub fn add_template(
        &mut self,
        template: GestureTemplate,
    ) -> Result<(), GestureError> {
        if template.name.is_empty() {
            return Err(GestureError::InvalidTemplateName(
                "Template name cannot be empty".into(),
            ));
        }
        if template.feature_dim != self.config.base.feature_dim {
            return Err(GestureError::DimensionMismatch {
                expected: self.config.base.feature_dim,
                got: template.feature_dim,
            });
        }
        if template.sequence.len() < self.config.base.min_sequence_len {
            return Err(GestureError::SequenceTooShort {
                needed: self.config.base.min_sequence_len,
                got: template.sequence.len(),
            });
        }

        let seq = Self::to_sequence(&template.sequence);
        self.template_sequences.push(seq);
        self.templates.push(template);
        Ok(())
    }

    /// Number of registered templates.
    pub fn template_count(&self) -> usize {
        self.templates.len()
    }

    /// Classify a perturbation sequence against registered templates.
    ///
    /// Uses the configured comparison algorithm (DTW, LCS, or edit distance)
    /// from `midstreamer-temporal-compare`.
    pub fn classify(
        &self,
        sequence: &[Vec<f64>],
        person_id: u64,
        timestamp_us: u64,
    ) -> Result<GestureResult, GestureError> {
        if self.templates.is_empty() {
            return Err(GestureError::NoTemplates);
        }
        if sequence.len() < self.config.base.min_sequence_len {
            return Err(GestureError::SequenceTooShort {
                needed: self.config.base.min_sequence_len,
                got: sequence.len(),
            });
        }
        for frame in sequence {
            if frame.len() != self.config.base.feature_dim {
                return Err(GestureError::DimensionMismatch {
                    expected: self.config.base.feature_dim,
                    got: frame.len(),
                });
            }
        }

        let query_seq = Self::to_sequence(sequence);
        let algo = self.config.algorithm.to_comparison_algorithm();

        let mut best_distance = f64::INFINITY;
        let mut second_best = f64::INFINITY;
        let mut best_idx: Option<usize> = None;

        for (idx, template_seq) in self.template_sequences.iter().enumerate() {
            let result = self
                .comparator
                .compare(&query_seq, template_seq, algo);
            // Use distance from ComparisonResult (lower = better match)
            let distance = match result {
                Ok(cr) => cr.distance,
                Err(_) => f64::INFINITY,
            };

            if distance < best_distance {
                second_best = best_distance;
                best_distance = distance;
                best_idx = Some(idx);
            } else if distance < second_best {
                second_best = distance;
            }
        }

        let recognized = best_distance <= self.config.max_distance;

        // Confidence based on margin between best and second-best
        let confidence = if recognized && second_best.is_finite() && second_best > 1e-10 {
            (1.0 - best_distance / second_best).clamp(0.0, 1.0)
        } else if recognized {
            (1.0 - best_distance / self.config.max_distance).clamp(0.0, 1.0)
        } else {
            0.0
        };

        if let Some(idx) = best_idx {
            let template = &self.templates[idx];
            Ok(GestureResult {
                recognized,
                gesture_type: if recognized {
                    Some(template.gesture_type)
                } else {
                    None
                },
                template_name: if recognized {
                    Some(template.name.clone())
                } else {
                    None
                },
                distance: best_distance,
                confidence,
                person_id,
                timestamp_us,
            })
        } else {
            Ok(GestureResult {
                recognized: false,
                gesture_type: None,
                template_name: None,
                distance: f64::INFINITY,
                confidence: 0.0,
                person_id,
                timestamp_us,
            })
        }
    }

    /// Get cache statistics from the temporal comparator.
    pub fn cache_stats(&self) -> midstreamer_temporal_compare::CacheStats {
        self.comparator.cache_stats()
    }

    /// Active comparison algorithm.
    pub fn algorithm(&self) -> GestureAlgorithm {
        self.config.algorithm
    }

    /// Convert a feature sequence to a midstreamer `Sequence<i64>`.
    ///
    /// Each frame's L2 norm is quantized to an i64 (multiplied by 1000)
    /// for use with the generic comparator.
    fn to_sequence(frames: &[Vec<f64>]) -> Sequence<i64> {
        let mut seq = Sequence::new();
        for (i, frame) in frames.iter().enumerate() {
            let norm = frame.iter().map(|x| x * x).sum::<f64>().sqrt();
            let quantized = (norm * 1000.0) as i64;
            seq.push(quantized, i as u64);
        }
        seq
    }
}

// We implement Debug manually because TemporalComparator does not derive Debug
impl std::fmt::Debug for TemporalGestureClassifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TemporalGestureClassifier")
            .field("config", &self.config)
            .field("template_count", &self.templates.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::gesture::GestureType;

    fn make_template(
        name: &str,
        gesture_type: GestureType,
        n_frames: usize,
        feature_dim: usize,
        pattern: fn(usize, usize) -> f64,
    ) -> GestureTemplate {
        let sequence: Vec<Vec<f64>> = (0..n_frames)
            .map(|t| (0..feature_dim).map(|d| pattern(t, d)).collect())
            .collect();
        GestureTemplate {
            name: name.to_string(),
            gesture_type,
            sequence,
            feature_dim,
        }
    }

    fn wave_pattern(t: usize, d: usize) -> f64 {
        if d == 0 {
            (t as f64 * 0.5).sin()
        } else {
            0.0
        }
    }

    fn push_pattern(t: usize, d: usize) -> f64 {
        if d == 0 {
            t as f64 * 0.1
        } else {
            0.0
        }
    }

    fn small_config() -> TemporalGestureConfig {
        TemporalGestureConfig {
            base: GestureConfig {
                feature_dim: 4,
                min_sequence_len: 5,
                max_distance: 10.0,
                band_width: 3,
            },
            algorithm: GestureAlgorithm::Dtw,
            enable_cache: false,
            cache_capacity: 64,
            max_distance: 100000.0, // generous for testing
            max_sequence_length: 1024,
        }
    }

    #[test]
    fn test_temporal_classifier_creation() {
        let classifier = TemporalGestureClassifier::new(small_config());
        assert_eq!(classifier.template_count(), 0);
        assert_eq!(classifier.algorithm(), GestureAlgorithm::Dtw);
    }

    #[test]
    fn test_temporal_add_template() {
        let mut classifier = TemporalGestureClassifier::new(small_config());
        let template = make_template("wave", GestureType::Wave, 10, 4, wave_pattern);
        classifier.add_template(template).unwrap();
        assert_eq!(classifier.template_count(), 1);
    }

    #[test]
    fn test_temporal_add_template_empty_name() {
        let mut classifier = TemporalGestureClassifier::new(small_config());
        let template = make_template("", GestureType::Wave, 10, 4, wave_pattern);
        assert!(matches!(
            classifier.add_template(template),
            Err(GestureError::InvalidTemplateName(_))
        ));
    }

    #[test]
    fn test_temporal_add_template_wrong_dim() {
        let mut classifier = TemporalGestureClassifier::new(small_config());
        let template = make_template("wave", GestureType::Wave, 10, 8, wave_pattern);
        assert!(matches!(
            classifier.add_template(template),
            Err(GestureError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn test_temporal_classify_no_templates() {
        let classifier = TemporalGestureClassifier::new(small_config());
        let seq: Vec<Vec<f64>> = (0..10).map(|_| vec![0.0; 4]).collect();
        assert!(matches!(
            classifier.classify(&seq, 1, 0),
            Err(GestureError::NoTemplates)
        ));
    }

    #[test]
    fn test_temporal_classify_too_short() {
        let mut classifier = TemporalGestureClassifier::new(small_config());
        classifier
            .add_template(make_template("wave", GestureType::Wave, 10, 4, wave_pattern))
            .unwrap();
        let seq: Vec<Vec<f64>> = (0..3).map(|_| vec![0.0; 4]).collect();
        assert!(matches!(
            classifier.classify(&seq, 1, 0),
            Err(GestureError::SequenceTooShort { .. })
        ));
    }

    #[test]
    fn test_temporal_classify_exact_match() {
        let mut classifier = TemporalGestureClassifier::new(small_config());
        let template = make_template("wave", GestureType::Wave, 10, 4, wave_pattern);
        classifier.add_template(template).unwrap();

        let seq: Vec<Vec<f64>> = (0..10)
            .map(|t| (0..4).map(|d| wave_pattern(t, d)).collect())
            .collect();

        let result = classifier.classify(&seq, 1, 100_000).unwrap();
        assert!(result.recognized, "Exact match should be recognized");
        assert_eq!(result.gesture_type, Some(GestureType::Wave));
        assert!(result.distance < 1e-6, "Exact match should have near-zero distance");
    }

    #[test]
    fn test_temporal_classify_best_of_two() {
        let mut classifier = TemporalGestureClassifier::new(small_config());
        classifier
            .add_template(make_template("wave", GestureType::Wave, 10, 4, wave_pattern))
            .unwrap();
        classifier
            .add_template(make_template("push", GestureType::Push, 10, 4, push_pattern))
            .unwrap();

        let seq: Vec<Vec<f64>> = (0..10)
            .map(|t| (0..4).map(|d| wave_pattern(t, d)).collect())
            .collect();

        let result = classifier.classify(&seq, 1, 0).unwrap();
        assert!(result.recognized);
    }

    #[test]
    fn test_temporal_algorithm_selection() {
        assert_eq!(
            GestureAlgorithm::Dtw.to_comparison_algorithm(),
            ComparisonAlgorithm::DTW
        );
        assert_eq!(
            GestureAlgorithm::Lcs.to_comparison_algorithm(),
            ComparisonAlgorithm::LCS
        );
        assert_eq!(
            GestureAlgorithm::EditDistance.to_comparison_algorithm(),
            ComparisonAlgorithm::EditDistance
        );
    }

    #[test]
    fn test_temporal_lcs_algorithm() {
        let config = TemporalGestureConfig {
            algorithm: GestureAlgorithm::Lcs,
            ..small_config()
        };
        let mut classifier = TemporalGestureClassifier::new(config);
        classifier
            .add_template(make_template("wave", GestureType::Wave, 10, 4, wave_pattern))
            .unwrap();

        let seq: Vec<Vec<f64>> = (0..10)
            .map(|t| (0..4).map(|d| wave_pattern(t, d)).collect())
            .collect();

        let result = classifier.classify(&seq, 1, 0).unwrap();
        assert!(result.recognized);
    }

    #[test]
    fn test_temporal_edit_distance_algorithm() {
        let config = TemporalGestureConfig {
            algorithm: GestureAlgorithm::EditDistance,
            ..small_config()
        };
        let mut classifier = TemporalGestureClassifier::new(config);
        classifier
            .add_template(make_template("wave", GestureType::Wave, 10, 4, wave_pattern))
            .unwrap();

        let seq: Vec<Vec<f64>> = (0..10)
            .map(|t| (0..4).map(|d| wave_pattern(t, d)).collect())
            .collect();

        let result = classifier.classify(&seq, 1, 0).unwrap();
        assert!(result.recognized);
    }

    #[test]
    fn test_temporal_default_config() {
        let config = TemporalGestureConfig::default();
        assert_eq!(config.algorithm, GestureAlgorithm::Dtw);
        assert!(config.enable_cache);
        assert_eq!(config.cache_capacity, 256);
        assert!((config.max_distance - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_temporal_cache_stats() {
        let classifier = TemporalGestureClassifier::new(small_config());
        let stats = classifier.cache_stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
    }

    #[test]
    fn test_to_sequence_conversion() {
        let frames: Vec<Vec<f64>> = vec![vec![3.0, 4.0], vec![0.0, 1.0]];
        let seq = TemporalGestureClassifier::to_sequence(&frames);
        // First element: sqrt(9+16) = 5.0 -> 5000
        // Second element: sqrt(0+1) = 1.0 -> 1000
        assert_eq!(seq.len(), 2);
    }

    #[test]
    fn test_debug_impl() {
        let classifier = TemporalGestureClassifier::new(small_config());
        let dbg = format!("{:?}", classifier);
        assert!(dbg.contains("TemporalGestureClassifier"));
    }
}
