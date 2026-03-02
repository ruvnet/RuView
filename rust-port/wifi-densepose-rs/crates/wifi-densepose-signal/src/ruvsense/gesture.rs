//! Gesture classification from per-person CSI perturbation patterns.
//!
//! Classifies gestures by comparing per-person CSI perturbation time
//! series against a library of gesture templates using Dynamic Time
//! Warping (DTW). Works through walls and darkness because it operates
//! on RF perturbations, not visual features.
//!
//! # Algorithm
//! 1. Collect per-person CSI perturbation over a gesture window (~1s)
//! 2. Normalize and project onto principal components
//! 3. Compare against stored gesture templates using DTW distance
//! 4. Classify as the nearest template if distance < threshold
//!
//! # Supported Gestures
//! Wave, point, beckon, push, circle, plus custom user-defined templates.
//!
//! # References
//! - ADR-030 Tier 6: Invisible Interaction Layer
//! - Sakoe & Chiba (1978), "Dynamic programming algorithm optimization
//!   for spoken word recognition" IEEE TASSP

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from gesture classification.
#[derive(Debug, thiserror::Error)]
pub enum GestureError {
    /// Gesture sequence too short.
    #[error("Sequence too short: need >= {needed} frames, got {got}")]
    SequenceTooShort { needed: usize, got: usize },

    /// No templates registered for classification.
    #[error("No gesture templates registered")]
    NoTemplates,

    /// Feature dimension mismatch.
    #[error("Feature dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    /// Invalid template name.
    #[error("Invalid template name: {0}")]
    InvalidTemplateName(String),
}

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// Built-in gesture categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GestureType {
    /// Waving hand (side to side).
    Wave,
    /// Pointing at a target.
    Point,
    /// Beckoning (come here).
    Beckon,
    /// Push forward motion.
    Push,
    /// Circular motion.
    Circle,
    /// User-defined custom gesture.
    Custom,
}

impl GestureType {
    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            GestureType::Wave => "wave",
            GestureType::Point => "point",
            GestureType::Beckon => "beckon",
            GestureType::Push => "push",
            GestureType::Circle => "circle",
            GestureType::Custom => "custom",
        }
    }
}

/// A gesture template: a reference time series for a known gesture.
#[derive(Debug, Clone)]
pub struct GestureTemplate {
    /// Unique template name (e.g., "wave_right", "push_forward").
    pub name: String,
    /// Gesture category.
    pub gesture_type: GestureType,
    /// Template feature sequence: `[n_frames][feature_dim]`.
    pub sequence: Vec<Vec<f64>>,
    /// Feature dimension.
    pub feature_dim: usize,
}

/// Result of gesture classification.
#[derive(Debug, Clone)]
pub struct GestureResult {
    /// Whether a gesture was recognized.
    pub recognized: bool,
    /// Matched gesture type (if recognized).
    pub gesture_type: Option<GestureType>,
    /// Matched template name (if recognized).
    pub template_name: Option<String>,
    /// DTW distance to best match.
    pub distance: f64,
    /// Confidence (0.0 to 1.0, based on relative distances).
    pub confidence: f64,
    /// Person ID this gesture belongs to.
    pub person_id: u64,
    /// Timestamp (microseconds).
    pub timestamp_us: u64,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the gesture classifier.
#[derive(Debug, Clone)]
pub struct GestureConfig {
    /// Feature dimension of perturbation vectors.
    pub feature_dim: usize,
    /// Minimum sequence length (frames) for a valid gesture.
    pub min_sequence_len: usize,
    /// Maximum DTW distance for a match (lower = stricter).
    pub max_distance: f64,
    /// DTW Sakoe-Chiba band width (constrains warping).
    pub band_width: usize,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            feature_dim: 8,
            min_sequence_len: 10,
            max_distance: 50.0,
            band_width: 5,
        }
    }
}

// ---------------------------------------------------------------------------
// Gesture classifier
// ---------------------------------------------------------------------------

/// Gesture classifier using DTW template matching.
///
/// Maintains a library of gesture templates and classifies new
/// perturbation sequences by finding the nearest template.
#[derive(Debug)]
pub struct GestureClassifier {
    config: GestureConfig,
    templates: Vec<GestureTemplate>,
}

impl GestureClassifier {
    /// Create a new gesture classifier.
    pub fn new(config: GestureConfig) -> Self {
        Self {
            config,
            templates: Vec::new(),
        }
    }

    /// Register a gesture template.
    pub fn add_template(&mut self, template: GestureTemplate) -> Result<(), GestureError> {
        if template.name.is_empty() {
            return Err(GestureError::InvalidTemplateName(
                "Template name cannot be empty".into(),
            ));
        }
        if template.feature_dim != self.config.feature_dim {
            return Err(GestureError::DimensionMismatch {
                expected: self.config.feature_dim,
                got: template.feature_dim,
            });
        }
        if template.sequence.len() < self.config.min_sequence_len {
            return Err(GestureError::SequenceTooShort {
                needed: self.config.min_sequence_len,
                got: template.sequence.len(),
            });
        }
        self.templates.push(template);
        Ok(())
    }

    /// Number of registered templates.
    pub fn template_count(&self) -> usize {
        self.templates.len()
    }

    /// Classify a perturbation sequence against registered templates.
    ///
    /// `sequence` is `[n_frames][feature_dim]` of perturbation features.
    pub fn classify(
        &self,
        sequence: &[Vec<f64>],
        person_id: u64,
        timestamp_us: u64,
    ) -> Result<GestureResult, GestureError> {
        if self.templates.is_empty() {
            return Err(GestureError::NoTemplates);
        }
        if sequence.len() < self.config.min_sequence_len {
            return Err(GestureError::SequenceTooShort {
                needed: self.config.min_sequence_len,
                got: sequence.len(),
            });
        }
        // Validate feature dimension
        for frame in sequence {
            if frame.len() != self.config.feature_dim {
                return Err(GestureError::DimensionMismatch {
                    expected: self.config.feature_dim,
                    got: frame.len(),
                });
            }
        }

        // Compute DTW distance to each template
        let mut best_dist = f64::INFINITY;
        let mut second_best_dist = f64::INFINITY;
        let mut best_idx: Option<usize> = None;

        for (idx, template) in self.templates.iter().enumerate() {
            let dist = dtw_distance(sequence, &template.sequence, self.config.band_width);
            if dist < best_dist {
                second_best_dist = best_dist;
                best_dist = dist;
                best_idx = Some(idx);
            } else if dist < second_best_dist {
                second_best_dist = dist;
            }
        }

        let recognized = best_dist <= self.config.max_distance;

        // Confidence: how much better is the best match vs second best
        let confidence = if recognized && second_best_dist.is_finite() && second_best_dist > 1e-10 {
            (1.0 - best_dist / second_best_dist).clamp(0.0, 1.0)
        } else if recognized {
            (1.0 - best_dist / self.config.max_distance).clamp(0.0, 1.0)
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
                distance: best_dist,
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
}

// ---------------------------------------------------------------------------
// Dynamic Time Warping
// ---------------------------------------------------------------------------

/// Compute DTW distance between two multivariate time series.
///
/// Uses the Sakoe-Chiba band constraint to limit warping.
/// Each frame is a vector of `feature_dim` dimensions.
fn dtw_distance(seq_a: &[Vec<f64>], seq_b: &[Vec<f64>], band_width: usize) -> f64 {
    let n = seq_a.len();
    let m = seq_b.len();

    if n == 0 || m == 0 {
        return f64::INFINITY;
    }

    // Cost matrix (only need 2 rows for memory efficiency)
    let mut prev = vec![f64::INFINITY; m + 1];
    let mut curr = vec![f64::INFINITY; m + 1];
    prev[0] = 0.0;

    for i in 1..=n {
        curr[0] = f64::INFINITY;

        let j_start = if band_width >= i {
            1
        } else {
            i.saturating_sub(band_width).max(1)
        };
        let j_end = (i + band_width).min(m);

        for j in 1..=m {
            if j < j_start || j > j_end {
                curr[j] = f64::INFINITY;
                continue;
            }

            let cost = euclidean_distance(&seq_a[i - 1], &seq_b[j - 1]);
            curr[j] = cost
                + prev[j] // insertion
                    .min(curr[j - 1]) // deletion
                    .min(prev[j - 1]); // match
        }

        std::mem::swap(&mut prev, &mut curr);
    }

    prev[m]
}

/// Euclidean distance between two feature vectors.
fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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

    fn small_config() -> GestureConfig {
        GestureConfig {
            feature_dim: 4,
            min_sequence_len: 5,
            max_distance: 10.0,
            band_width: 3,
        }
    }

    #[test]
    fn test_classifier_creation() {
        let classifier = GestureClassifier::new(small_config());
        assert_eq!(classifier.template_count(), 0);
    }

    #[test]
    fn test_add_template() {
        let mut classifier = GestureClassifier::new(small_config());
        let template = make_template("wave", GestureType::Wave, 10, 4, wave_pattern);
        classifier.add_template(template).unwrap();
        assert_eq!(classifier.template_count(), 1);
    }

    #[test]
    fn test_add_template_empty_name() {
        let mut classifier = GestureClassifier::new(small_config());
        let template = make_template("", GestureType::Wave, 10, 4, wave_pattern);
        assert!(matches!(
            classifier.add_template(template),
            Err(GestureError::InvalidTemplateName(_))
        ));
    }

    #[test]
    fn test_add_template_wrong_dim() {
        let mut classifier = GestureClassifier::new(small_config());
        let template = make_template("wave", GestureType::Wave, 10, 8, wave_pattern);
        assert!(matches!(
            classifier.add_template(template),
            Err(GestureError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn test_add_template_too_short() {
        let mut classifier = GestureClassifier::new(small_config());
        let template = make_template("wave", GestureType::Wave, 3, 4, wave_pattern);
        assert!(matches!(
            classifier.add_template(template),
            Err(GestureError::SequenceTooShort { .. })
        ));
    }

    #[test]
    fn test_classify_no_templates() {
        let classifier = GestureClassifier::new(small_config());
        let seq: Vec<Vec<f64>> = (0..10).map(|_| vec![0.0; 4]).collect();
        assert!(matches!(
            classifier.classify(&seq, 1, 0),
            Err(GestureError::NoTemplates)
        ));
    }

    #[test]
    fn test_classify_exact_match() {
        let mut classifier = GestureClassifier::new(small_config());
        let template = make_template("wave", GestureType::Wave, 10, 4, wave_pattern);
        classifier.add_template(template).unwrap();

        // Feed the exact same pattern
        let seq: Vec<Vec<f64>> = (0..10)
            .map(|t| (0..4).map(|d| wave_pattern(t, d)).collect())
            .collect();

        let result = classifier.classify(&seq, 1, 100_000).unwrap();
        assert!(result.recognized);
        assert_eq!(result.gesture_type, Some(GestureType::Wave));
        assert!(
            result.distance < 1e-10,
            "Exact match should have zero distance"
        );
    }

    #[test]
    fn test_classify_best_of_two() {
        let mut classifier = GestureClassifier::new(GestureConfig {
            max_distance: 100.0,
            ..small_config()
        });
        classifier
            .add_template(make_template(
                "wave",
                GestureType::Wave,
                10,
                4,
                wave_pattern,
            ))
            .unwrap();
        classifier
            .add_template(make_template(
                "push",
                GestureType::Push,
                10,
                4,
                push_pattern,
            ))
            .unwrap();

        // Feed a wave-like pattern
        let seq: Vec<Vec<f64>> = (0..10)
            .map(|t| (0..4).map(|d| wave_pattern(t, d) + 0.01).collect())
            .collect();

        let result = classifier.classify(&seq, 1, 0).unwrap();
        assert!(result.recognized);
        assert_eq!(result.gesture_type, Some(GestureType::Wave));
    }

    #[test]
    fn test_classify_no_match_high_distance() {
        let mut classifier = GestureClassifier::new(GestureConfig {
            max_distance: 0.001, // very strict
            ..small_config()
        });
        classifier
            .add_template(make_template(
                "wave",
                GestureType::Wave,
                10,
                4,
                wave_pattern,
            ))
            .unwrap();

        // Random-ish sequence
        let seq: Vec<Vec<f64>> = (0..10)
            .map(|t| vec![t as f64 * 10.0, 0.0, 0.0, 0.0])
            .collect();

        let result = classifier.classify(&seq, 1, 0).unwrap();
        assert!(!result.recognized);
        assert!(result.gesture_type.is_none());
    }

    #[test]
    fn test_dtw_identical_sequences() {
        let seq: Vec<Vec<f64>> = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]];
        let dist = dtw_distance(&seq, &seq, 3);
        assert!(
            dist < 1e-10,
            "Identical sequences should have zero DTW distance"
        );
    }

    #[test]
    fn test_dtw_different_sequences() {
        let a: Vec<Vec<f64>> = vec![vec![0.0], vec![0.0], vec![0.0]];
        let b: Vec<Vec<f64>> = vec![vec![10.0], vec![10.0], vec![10.0]];
        let dist = dtw_distance(&a, &b, 3);
        assert!(
            dist > 0.0,
            "Different sequences should have non-zero DTW distance"
        );
    }

    #[test]
    fn test_dtw_time_warped() {
        // Same shape but different speed
        let a: Vec<Vec<f64>> = vec![vec![0.0], vec![1.0], vec![2.0], vec![3.0]];
        let b: Vec<Vec<f64>> = vec![
            vec![0.0],
            vec![0.5],
            vec![1.0],
            vec![1.5],
            vec![2.0],
            vec![2.5],
            vec![3.0],
        ];
        let dist = dtw_distance(&a, &b, 4);
        // DTW should be relatively small despite different lengths
        assert!(dist < 2.0, "DTW should handle time warping, got {}", dist);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 3.0];
        let b = vec![4.0, 0.0];
        let d = euclidean_distance(&a, &b);
        assert!((d - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_gesture_type_names() {
        assert_eq!(GestureType::Wave.name(), "wave");
        assert_eq!(GestureType::Push.name(), "push");
        assert_eq!(GestureType::Circle.name(), "circle");
        assert_eq!(GestureType::Custom.name(), "custom");
    }
}
