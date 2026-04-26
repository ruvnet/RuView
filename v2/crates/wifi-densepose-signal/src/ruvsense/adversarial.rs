//! Adversarial detection: physically impossible signal identification.
//!
//! Detects spoofed or injected WiFi signals by checking multi-link
//! consistency, field model constraint violations, and physical
//! plausibility. A single-link injection cannot fool a multistatic
//! mesh because it would violate geometric constraints across links.
//!
//! # Checks
//! 1. **Multi-link consistency**: A real body perturbs all links that
//!    traverse its location. An injection affects only the targeted link.
//! 2. **Field model constraints**: Perturbation must be consistent with
//!    the room's eigenmode structure.
//! 3. **Temporal continuity**: Real movement is smooth; injections cause
//!    discontinuities in embedding space.
//! 4. **Energy conservation**: Total perturbation energy across links
//!    must be consistent with the number and size of bodies present.
//!
//! # References
//! - ADR-030 Tier 7: Adversarial Detection

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from adversarial detection.
#[derive(Debug, thiserror::Error)]
pub enum AdversarialError {
    /// Insufficient links for multi-link consistency check.
    #[error("Insufficient links: need >= {needed}, got {got}")]
    InsufficientLinks { needed: usize, got: usize },

    /// Dimension mismatch.
    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    /// No baseline available for constraint checking.
    #[error("No baseline available — calibrate field model first")]
    NoBaseline,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for adversarial detection.
#[derive(Debug, Clone)]
pub struct AdversarialConfig {
    /// Number of links in the mesh.
    pub n_links: usize,
    /// Minimum links for multi-link consistency (default 4).
    pub min_links: usize,
    /// Consistency threshold: fraction of links that must agree (0.0-1.0).
    pub consistency_threshold: f64,
    /// Maximum allowed energy ratio between any single link and total.
    pub max_single_link_energy_ratio: f64,
    /// Maximum allowed temporal discontinuity in embedding space.
    pub max_temporal_discontinuity: f64,
    /// Maximum allowed perturbation energy per body.
    pub max_energy_per_body: f64,
}

impl Default for AdversarialConfig {
    fn default() -> Self {
        Self {
            n_links: 12,
            min_links: 4,
            consistency_threshold: 0.6,
            max_single_link_energy_ratio: 0.5,
            max_temporal_discontinuity: 5.0,
            max_energy_per_body: 100.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Detection results
// ---------------------------------------------------------------------------

/// Type of adversarial anomaly detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnomalyType {
    /// Single link shows perturbation inconsistent with other links.
    SingleLinkInjection,
    /// Perturbation violates field model eigenmode structure.
    FieldModelViolation,
    /// Sudden discontinuity in embedding trajectory.
    TemporalDiscontinuity,
    /// Total perturbation energy inconsistent with occupancy.
    EnergyViolation,
    /// Multiple anomaly types detected simultaneously.
    MultipleViolations,
}

impl AnomalyType {
    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            AnomalyType::SingleLinkInjection => "single_link_injection",
            AnomalyType::FieldModelViolation => "field_model_violation",
            AnomalyType::TemporalDiscontinuity => "temporal_discontinuity",
            AnomalyType::EnergyViolation => "energy_violation",
            AnomalyType::MultipleViolations => "multiple_violations",
        }
    }
}

/// Result of adversarial detection on one frame.
#[derive(Debug, Clone)]
pub struct AdversarialResult {
    /// Whether any anomaly was detected.
    pub anomaly_detected: bool,
    /// Type of anomaly (if detected).
    pub anomaly_type: Option<AnomalyType>,
    /// Anomaly score (0.0 = clean, 1.0 = definitely adversarial).
    pub anomaly_score: f64,
    /// Per-check results.
    pub checks: CheckResults,
    /// Affected link indices (if single-link injection).
    pub affected_links: Vec<usize>,
    /// Timestamp (microseconds).
    pub timestamp_us: u64,
}

/// Results of individual checks.
#[derive(Debug, Clone)]
pub struct CheckResults {
    /// Multi-link consistency score (0.0 = inconsistent, 1.0 = fully consistent).
    pub consistency_score: f64,
    /// Field model residual score (lower = more consistent with modes).
    pub field_model_residual: f64,
    /// Temporal continuity score (lower = smoother).
    pub temporal_continuity: f64,
    /// Energy conservation score (closer to 1.0 = consistent).
    pub energy_ratio: f64,
}

// ---------------------------------------------------------------------------
// Adversarial detector
// ---------------------------------------------------------------------------

/// Adversarial signal detector for the multistatic mesh.
///
/// Checks each frame for physical plausibility across multiple
/// independent criteria. A spoofed signal that passes one check
/// is unlikely to pass all of them.
#[derive(Debug)]
pub struct AdversarialDetector {
    config: AdversarialConfig,
    /// Previous frame's per-link energies (for temporal continuity).
    prev_energies: Option<Vec<f64>>,
    /// Previous frame's total energy.
    prev_total_energy: Option<f64>,
    /// Total frames processed.
    total_frames: u64,
    /// Total anomalies detected.
    anomaly_count: u64,
}

impl AdversarialDetector {
    /// Create a new adversarial detector.
    pub fn new(config: AdversarialConfig) -> Result<Self, AdversarialError> {
        if config.n_links < config.min_links {
            return Err(AdversarialError::InsufficientLinks {
                needed: config.min_links,
                got: config.n_links,
            });
        }
        Ok(Self {
            config,
            prev_energies: None,
            prev_total_energy: None,
            total_frames: 0,
            anomaly_count: 0,
        })
    }

    /// Check a frame for adversarial anomalies.
    ///
    /// `link_energies`: per-link perturbation energy (from field model).
    /// `n_bodies`: estimated number of bodies present.
    /// `timestamp_us`: frame timestamp.
    pub fn check(
        &mut self,
        link_energies: &[f64],
        n_bodies: usize,
        timestamp_us: u64,
    ) -> Result<AdversarialResult, AdversarialError> {
        if link_energies.len() != self.config.n_links {
            return Err(AdversarialError::DimensionMismatch {
                expected: self.config.n_links,
                got: link_energies.len(),
            });
        }

        self.total_frames += 1;

        let total_energy: f64 = link_energies.iter().sum();

        // Check 1: Multi-link consistency
        let consistency = self.check_consistency(link_energies, total_energy);

        // Check 2: Field model residual (simplified — check energy distribution)
        let field_residual = self.check_field_model(link_energies, total_energy);

        // Check 3: Temporal continuity
        let temporal = self.check_temporal(link_energies, total_energy);

        // Check 4: Energy conservation
        let energy_ratio = self.check_energy(total_energy, n_bodies);

        // Store for next frame
        self.prev_energies = Some(link_energies.to_vec());
        self.prev_total_energy = Some(total_energy);

        let checks = CheckResults {
            consistency_score: consistency,
            field_model_residual: field_residual,
            temporal_continuity: temporal,
            energy_ratio,
        };

        // Aggregate anomaly score
        let mut violations = Vec::new();

        if consistency < self.config.consistency_threshold {
            violations.push(AnomalyType::SingleLinkInjection);
        }
        if field_residual > 0.8 {
            violations.push(AnomalyType::FieldModelViolation);
        }
        if temporal > self.config.max_temporal_discontinuity {
            violations.push(AnomalyType::TemporalDiscontinuity);
        }
        if energy_ratio > 2.0 || (n_bodies > 0 && energy_ratio < 0.1) {
            violations.push(AnomalyType::EnergyViolation);
        }

        let anomaly_detected = !violations.is_empty();
        let anomaly_type = match violations.len() {
            0 => None,
            1 => Some(violations[0]),
            _ => Some(AnomalyType::MultipleViolations),
        };

        // Score: weighted combination
        let anomaly_score = ((1.0 - consistency) * 0.4
            + field_residual * 0.2
            + (temporal / self.config.max_temporal_discontinuity).min(1.0) * 0.2
            + ((energy_ratio - 1.0).abs() / 2.0).min(1.0) * 0.2)
            .clamp(0.0, 1.0);

        // Find affected links (highest single-link energy ratio)
        let affected_links = if anomaly_detected {
            self.find_anomalous_links(link_energies, total_energy)
        } else {
            Vec::new()
        };

        if anomaly_detected {
            self.anomaly_count += 1;
        }

        Ok(AdversarialResult {
            anomaly_detected,
            anomaly_type,
            anomaly_score,
            checks,
            affected_links,
            timestamp_us,
        })
    }

    /// Multi-link consistency: what fraction of links have correlated energy?
    ///
    /// A real body perturbs many links. An injection affects few.
    fn check_consistency(&self, energies: &[f64], total: f64) -> f64 {
        if total < 1e-15 {
            return 1.0; // No perturbation = consistent (empty room)
        }

        let mean = total / energies.len() as f64;
        let threshold = mean * 0.1; // link must have at least 10% of mean energy

        let active_count = energies.iter().filter(|&&e| e > threshold).count();
        active_count as f64 / energies.len() as f64
    }

    /// Field model check: is energy distribution consistent with physical propagation?
    ///
    /// In a real scenario, energy should be distributed across links
    /// based on geometry. A concentrated injection scores high residual.
    fn check_field_model(&self, energies: &[f64], total: f64) -> f64 {
        if total < 1e-15 {
            return 0.0;
        }

        // Compute Gini coefficient of energy distribution
        // Gini = 0 → perfectly uniform, Gini = 1 → all in one link
        let n = energies.len() as f64;
        let mut sorted: Vec<f64> = energies.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let numerator: f64 = sorted
            .iter()
            .enumerate()
            .map(|(i, &x)| (2.0 * (i + 1) as f64 - n - 1.0) * x)
            .sum();

        let gini = numerator / (n * total);
        gini.clamp(0.0, 1.0)
    }

    /// Temporal continuity: how much did per-link energies change from previous frame?
    fn check_temporal(&self, energies: &[f64], _total: f64) -> f64 {
        match &self.prev_energies {
            None => 0.0, // First frame, no temporal check
            Some(prev) => {
                let diff_energy: f64 = energies
                    .iter()
                    .zip(prev.iter())
                    .map(|(&a, &b)| (a - b) * (a - b))
                    .sum::<f64>()
                    .sqrt();
                diff_energy
            }
        }
    }

    /// Energy conservation: is total energy consistent with body count?
    fn check_energy(&self, total_energy: f64, n_bodies: usize) -> f64 {
        if n_bodies == 0 {
            // No bodies: any energy is suspicious
            return if total_energy > 1e-10 {
                total_energy
            } else {
                0.0
            };
        }
        let expected = n_bodies as f64 * self.config.max_energy_per_body;
        if expected < 1e-15 {
            return 0.0;
        }
        total_energy / expected
    }

    /// Find links that are anomalously high relative to the mean.
    fn find_anomalous_links(&self, energies: &[f64], total: f64) -> Vec<usize> {
        if total < 1e-15 {
            return Vec::new();
        }

        energies
            .iter()
            .enumerate()
            .filter(|(_, &e)| e / total > self.config.max_single_link_energy_ratio)
            .map(|(i, _)| i)
            .collect()
    }

    /// Total frames processed.
    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }

    /// Total anomalies detected.
    pub fn anomaly_count(&self) -> u64 {
        self.anomaly_count
    }

    /// Anomaly rate (anomalies / total frames).
    pub fn anomaly_rate(&self) -> f64 {
        if self.total_frames == 0 {
            0.0
        } else {
            self.anomaly_count as f64 / self.total_frames as f64
        }
    }

    /// Reset detector state.
    pub fn reset(&mut self) {
        self.prev_energies = None;
        self.prev_total_energy = None;
        self.total_frames = 0;
        self.anomaly_count = 0;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> AdversarialConfig {
        AdversarialConfig {
            n_links: 6,
            min_links: 4,
            consistency_threshold: 0.6,
            max_single_link_energy_ratio: 0.5,
            max_temporal_discontinuity: 5.0,
            max_energy_per_body: 10.0,
        }
    }

    #[test]
    fn test_detector_creation() {
        let det = AdversarialDetector::new(default_config()).unwrap();
        assert_eq!(det.total_frames(), 0);
        assert_eq!(det.anomaly_count(), 0);
    }

    #[test]
    fn test_insufficient_links() {
        let config = AdversarialConfig {
            n_links: 2,
            min_links: 4,
            ..default_config()
        };
        assert!(matches!(
            AdversarialDetector::new(config),
            Err(AdversarialError::InsufficientLinks { .. })
        ));
    }

    #[test]
    fn test_clean_frame_no_anomaly() {
        let mut det = AdversarialDetector::new(default_config()).unwrap();

        // Uniform energy across all links (real body)
        let energies = vec![1.0, 1.1, 0.9, 1.0, 1.05, 0.95];
        let result = det.check(&energies, 1, 0).unwrap();

        assert!(
            !result.anomaly_detected,
            "Uniform energy should not trigger anomaly"
        );
        assert!(result.anomaly_score < 0.5);
    }

    #[test]
    fn test_single_link_injection_detected() {
        let mut det = AdversarialDetector::new(default_config()).unwrap();

        // All energy on one link (injection)
        let energies = vec![10.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let result = det.check(&energies, 0, 0).unwrap();

        assert!(
            result.anomaly_detected,
            "Single-link injection should be detected"
        );
        assert!(result.affected_links.contains(&0));
    }

    #[test]
    fn test_empty_room_no_anomaly() {
        let mut det = AdversarialDetector::new(default_config()).unwrap();

        let energies = vec![0.0; 6];
        let result = det.check(&energies, 0, 0).unwrap();

        assert!(!result.anomaly_detected);
    }

    #[test]
    fn test_temporal_discontinuity() {
        let mut det = AdversarialDetector::new(AdversarialConfig {
            max_temporal_discontinuity: 1.0, // strict
            ..default_config()
        })
        .unwrap();

        // Frame 1: low energy
        let energies1 = vec![0.1; 6];
        det.check(&energies1, 0, 0).unwrap();

        // Frame 2: sudden massive energy (discontinuity)
        let energies2 = vec![100.0; 6];
        let result = det.check(&energies2, 0, 50_000).unwrap();

        assert!(
            result.anomaly_detected,
            "Temporal discontinuity should be detected"
        );
    }

    #[test]
    fn test_energy_violation_too_high() {
        let mut det = AdversarialDetector::new(default_config()).unwrap();

        // Way more energy than 1 body should produce
        let energies = vec![100.0; 6]; // total = 600, max_per_body = 10
        let result = det.check(&energies, 1, 0).unwrap();

        assert!(
            result.anomaly_detected,
            "Excessive energy should trigger anomaly"
        );
    }

    #[test]
    fn test_dimension_mismatch() {
        let mut det = AdversarialDetector::new(default_config()).unwrap();
        let result = det.check(&[1.0, 2.0], 0, 0);
        assert!(matches!(
            result,
            Err(AdversarialError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn test_anomaly_rate() {
        let mut det = AdversarialDetector::new(default_config()).unwrap();

        // 2 clean frames
        det.check(&vec![1.0; 6], 1, 0).unwrap();
        det.check(&vec![1.0; 6], 1, 50_000).unwrap();

        // 1 anomalous frame
        det.check(&vec![10.0, 0.0, 0.0, 0.0, 0.0, 0.0], 0, 100_000)
            .unwrap();

        assert_eq!(det.total_frames(), 3);
        assert!(det.anomaly_count() >= 1);
        assert!(det.anomaly_rate() > 0.0);
    }

    #[test]
    fn test_reset() {
        let mut det = AdversarialDetector::new(default_config()).unwrap();
        det.check(&vec![1.0; 6], 1, 0).unwrap();
        det.reset();

        assert_eq!(det.total_frames(), 0);
        assert_eq!(det.anomaly_count(), 0);
    }

    #[test]
    fn test_anomaly_type_names() {
        assert_eq!(
            AnomalyType::SingleLinkInjection.name(),
            "single_link_injection"
        );
        assert_eq!(
            AnomalyType::FieldModelViolation.name(),
            "field_model_violation"
        );
        assert_eq!(
            AnomalyType::TemporalDiscontinuity.name(),
            "temporal_discontinuity"
        );
        assert_eq!(AnomalyType::EnergyViolation.name(), "energy_violation");
        assert_eq!(
            AnomalyType::MultipleViolations.name(),
            "multiple_violations"
        );
    }

    #[test]
    fn test_gini_coefficient_uniform() {
        let det = AdversarialDetector::new(default_config()).unwrap();
        let energies = vec![1.0; 6];
        let total = 6.0;
        let gini = det.check_field_model(&energies, total);
        assert!(
            gini < 0.1,
            "Uniform distribution should have low Gini: {}",
            gini
        );
    }

    #[test]
    fn test_gini_coefficient_concentrated() {
        let det = AdversarialDetector::new(default_config()).unwrap();
        let energies = vec![6.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let total = 6.0;
        let gini = det.check_field_model(&energies, total);
        assert!(
            gini > 0.5,
            "Concentrated distribution should have high Gini: {}",
            gini
        );
    }
}
