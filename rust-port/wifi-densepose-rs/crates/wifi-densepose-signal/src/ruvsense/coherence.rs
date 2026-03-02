//! Coherence Metric Computation (ADR-029 Section 2.5)
//!
//! Per-link coherence quantifies consistency of the current CSI observation
//! with a running reference template. The metric is computed as a weighted
//! mean of per-subcarrier Gaussian likelihoods:
//!
//!   score = sum(w_i * exp(-0.5 * z_i^2)) / sum(w_i)
//!
//! where z_i = |current_i - reference_i| / sqrt(variance_i) and
//! w_i = 1 / (variance_i + epsilon).
//!
//! Low-variance (stable) subcarriers dominate the score, making it
//! sensitive to environmental drift while tolerant of body-motion
//! subcarrier fluctuations.
//!
//! # RuVector Integration
//!
//! Uses `ruvector-solver` concepts for static/dynamic decomposition
//! of the CSI signal into environmental drift and body motion components.

/// Errors from coherence computation.
#[derive(Debug, thiserror::Error)]
pub enum CoherenceError {
    /// Input vectors are empty.
    #[error("Empty input for coherence computation")]
    EmptyInput,

    /// Length mismatch between current, reference, and variance vectors.
    #[error("Length mismatch: current={current}, reference={reference}, variance={variance}")]
    LengthMismatch {
        current: usize,
        reference: usize,
        variance: usize,
    },

    /// Invalid decay rate (must be in (0, 1)).
    #[error("Invalid EMA decay rate: {0} (must be in (0, 1))")]
    InvalidDecay(f32),
}

/// Drift profile classification for environmental changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftProfile {
    /// Environment is stable (no significant baseline drift).
    Stable,
    /// Slow linear drift (temperature, humidity changes).
    Linear,
    /// Sudden step change (door opened, furniture moved).
    StepChange,
}

/// Aggregate root for coherence state.
///
/// Maintains a running reference template (exponential moving average of
/// accepted CSI observations) and per-subcarrier variance estimates.
#[derive(Debug, Clone)]
pub struct CoherenceState {
    /// Per-subcarrier reference amplitude (EMA).
    reference: Vec<f32>,
    /// Per-subcarrier variance over recent window.
    variance: Vec<f32>,
    /// EMA decay rate for reference update (default 0.95).
    decay: f32,
    /// Current coherence score (0.0-1.0).
    current_score: f32,
    /// Frames since last accepted (coherent) measurement.
    stale_count: u64,
    /// Current drift profile classification.
    drift_profile: DriftProfile,
    /// Accept threshold for coherence score.
    accept_threshold: f32,
    /// Whether the reference has been initialized.
    initialized: bool,
}

impl CoherenceState {
    /// Create a new coherence state for the given number of subcarriers.
    pub fn new(n_subcarriers: usize, accept_threshold: f32) -> Self {
        Self {
            reference: vec![0.0; n_subcarriers],
            variance: vec![1.0; n_subcarriers],
            decay: 0.95,
            current_score: 1.0,
            stale_count: 0,
            drift_profile: DriftProfile::Stable,
            accept_threshold,
            initialized: false,
        }
    }

    /// Create with a custom EMA decay rate.
    pub fn with_decay(
        n_subcarriers: usize,
        accept_threshold: f32,
        decay: f32,
    ) -> std::result::Result<Self, CoherenceError> {
        if decay <= 0.0 || decay >= 1.0 {
            return Err(CoherenceError::InvalidDecay(decay));
        }
        let mut state = Self::new(n_subcarriers, accept_threshold);
        state.decay = decay;
        Ok(state)
    }

    /// Return the current coherence score.
    pub fn score(&self) -> f32 {
        self.current_score
    }

    /// Return the number of frames since last accepted measurement.
    pub fn stale_count(&self) -> u64 {
        self.stale_count
    }

    /// Return the current drift profile.
    pub fn drift_profile(&self) -> DriftProfile {
        self.drift_profile
    }

    /// Return a reference to the current reference template.
    pub fn reference(&self) -> &[f32] {
        &self.reference
    }

    /// Return a reference to the current variance estimates.
    pub fn variance(&self) -> &[f32] {
        &self.variance
    }

    /// Return whether the reference has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Initialize the reference from a calibration observation.
    ///
    /// Should be called with a static-environment CSI frame before
    /// sensing begins.
    pub fn initialize(&mut self, calibration: &[f32]) {
        self.reference = calibration.to_vec();
        self.variance = vec![1.0; calibration.len()];
        self.current_score = 1.0;
        self.stale_count = 0;
        self.initialized = true;
    }

    /// Update the coherence state with a new observation.
    ///
    /// Computes the coherence score, updates the reference template if
    /// the observation is accepted, and tracks staleness.
    pub fn update(
        &mut self,
        current: &[f32],
    ) -> std::result::Result<f32, CoherenceError> {
        if current.is_empty() {
            return Err(CoherenceError::EmptyInput);
        }

        if !self.initialized {
            self.initialize(current);
            return Ok(1.0);
        }

        if current.len() != self.reference.len() {
            return Err(CoherenceError::LengthMismatch {
                current: current.len(),
                reference: self.reference.len(),
                variance: self.variance.len(),
            });
        }

        // Compute coherence score
        let score = coherence_score(current, &self.reference, &self.variance);
        self.current_score = score;

        // Update reference if accepted
        if score >= self.accept_threshold {
            self.update_reference(current);
            self.stale_count = 0;
        } else {
            self.stale_count += 1;
        }

        // Update drift profile
        self.drift_profile = classify_drift(score, self.stale_count);

        Ok(score)
    }

    /// Update the reference template with EMA.
    fn update_reference(&mut self, observation: &[f32]) {
        let alpha = 1.0 - self.decay;
        for i in 0..self.reference.len() {
            let old_ref = self.reference[i];
            self.reference[i] = self.decay * old_ref + alpha * observation[i];

            // Update variance with Welford-style online estimate
            let diff = observation[i] - old_ref;
            self.variance[i] = self.decay * self.variance[i] + alpha * diff * diff;
            // Ensure variance does not collapse to zero
            if self.variance[i] < 1e-6 {
                self.variance[i] = 1e-6;
            }
        }
    }

    /// Reset the stale counter (e.g., after recalibration).
    pub fn reset_stale(&mut self) {
        self.stale_count = 0;
    }
}

/// Compute the coherence score between a current observation and a
/// reference template.
///
/// Uses z-score per subcarrier with variance-inverse weighting:
///
///   score = sum(w_i * exp(-0.5 * z_i^2)) / sum(w_i)
///
/// where z_i = |current_i - reference_i| / sqrt(variance_i)
/// and w_i = 1 / (variance_i + epsilon).
///
/// Returns a value in [0.0, 1.0] where 1.0 means perfect agreement.
pub fn coherence_score(
    current: &[f32],
    reference: &[f32],
    variance: &[f32],
) -> f32 {
    let n = current.len().min(reference.len()).min(variance.len());
    if n == 0 {
        return 0.0;
    }

    let epsilon = 1e-6_f32;
    let mut weighted_sum = 0.0_f32;
    let mut weight_sum = 0.0_f32;

    for i in 0..n {
        let var = variance[i].max(epsilon);
        let z = (current[i] - reference[i]).abs() / var.sqrt();
        let weight = 1.0 / (var + epsilon);
        let likelihood = (-0.5 * z * z).exp();
        weighted_sum += likelihood * weight;
        weight_sum += weight;
    }

    if weight_sum < epsilon {
        return 0.0;
    }

    (weighted_sum / weight_sum).clamp(0.0, 1.0)
}

/// Classify drift profile based on coherence history.
fn classify_drift(score: f32, stale_count: u64) -> DriftProfile {
    if score >= 0.85 {
        DriftProfile::Stable
    } else if stale_count < 10 {
        // Brief coherence loss -> likely step change
        DriftProfile::StepChange
    } else {
        // Extended low coherence -> linear drift
        DriftProfile::Linear
    }
}

/// Compute per-subcarrier z-scores for diagnostics.
///
/// Returns a vector of z-scores, one per subcarrier.
pub fn per_subcarrier_zscores(
    current: &[f32],
    reference: &[f32],
    variance: &[f32],
) -> Vec<f32> {
    let n = current.len().min(reference.len()).min(variance.len());
    (0..n)
        .map(|i| {
            let var = variance[i].max(1e-6);
            (current[i] - reference[i]).abs() / var.sqrt()
        })
        .collect()
}

/// Identify subcarriers that are outliers (z-score above threshold).
///
/// Returns indices of outlier subcarriers.
pub fn outlier_subcarriers(
    current: &[f32],
    reference: &[f32],
    variance: &[f32],
    z_threshold: f32,
) -> Vec<usize> {
    let z_scores = per_subcarrier_zscores(current, reference, variance);
    z_scores
        .iter()
        .enumerate()
        .filter(|(_, &z)| z > z_threshold)
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_coherence() {
        let current = vec![1.0, 2.0, 3.0, 4.0];
        let reference = vec![1.0, 2.0, 3.0, 4.0];
        let variance = vec![0.01, 0.01, 0.01, 0.01];
        let score = coherence_score(&current, &reference, &variance);
        assert!((score - 1.0).abs() < 0.01, "Perfect match should give ~1.0, got {}", score);
    }

    #[test]
    fn zero_coherence_large_deviation() {
        let current = vec![100.0, 200.0, 300.0];
        let reference = vec![0.0, 0.0, 0.0];
        let variance = vec![0.001, 0.001, 0.001];
        let score = coherence_score(&current, &reference, &variance);
        assert!(score < 0.01, "Large deviation should give ~0.0, got {}", score);
    }

    #[test]
    fn empty_input_gives_zero() {
        assert_eq!(coherence_score(&[], &[], &[]), 0.0);
    }

    #[test]
    fn state_initialize_and_score() {
        let mut state = CoherenceState::new(4, 0.85);
        assert!(!state.is_initialized());
        state.initialize(&[1.0, 2.0, 3.0, 4.0]);
        assert!(state.is_initialized());
        assert!((state.score() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn state_update_accepted() {
        let mut state = CoherenceState::new(4, 0.5);
        state.initialize(&[1.0, 2.0, 3.0, 4.0]);
        let score = state.update(&[1.01, 2.01, 3.01, 4.01]).unwrap();
        assert!(score > 0.8, "Small deviation should be accepted, got {}", score);
        assert_eq!(state.stale_count(), 0);
    }

    #[test]
    fn state_update_rejected() {
        let mut state = CoherenceState::new(4, 0.99);
        state.initialize(&[1.0, 2.0, 3.0, 4.0]);
        let _ = state.update(&[10.0, 20.0, 30.0, 40.0]).unwrap();
        assert!(state.stale_count() > 0);
    }

    #[test]
    fn auto_initialize_on_first_update() {
        let mut state = CoherenceState::new(3, 0.85);
        let score = state.update(&[5.0, 6.0, 7.0]).unwrap();
        assert!((score - 1.0).abs() < f32::EPSILON);
        assert!(state.is_initialized());
    }

    #[test]
    fn length_mismatch_error() {
        let mut state = CoherenceState::new(4, 0.85);
        state.initialize(&[1.0, 2.0, 3.0, 4.0]);
        let result = state.update(&[1.0, 2.0]);
        assert!(matches!(result, Err(CoherenceError::LengthMismatch { .. })));
    }

    #[test]
    fn empty_update_error() {
        let mut state = CoherenceState::new(4, 0.85);
        state.initialize(&[1.0, 2.0, 3.0, 4.0]);
        assert!(matches!(state.update(&[]), Err(CoherenceError::EmptyInput)));
    }

    #[test]
    fn invalid_decay_error() {
        assert!(matches!(
            CoherenceState::with_decay(4, 0.85, 0.0),
            Err(CoherenceError::InvalidDecay(_))
        ));
        assert!(matches!(
            CoherenceState::with_decay(4, 0.85, 1.0),
            Err(CoherenceError::InvalidDecay(_))
        ));
        assert!(matches!(
            CoherenceState::with_decay(4, 0.85, -0.5),
            Err(CoherenceError::InvalidDecay(_))
        ));
    }

    #[test]
    fn valid_decay() {
        let state = CoherenceState::with_decay(4, 0.85, 0.9).unwrap();
        assert!((state.score() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn drift_classification_stable() {
        assert_eq!(classify_drift(0.9, 0), DriftProfile::Stable);
    }

    #[test]
    fn drift_classification_step_change() {
        assert_eq!(classify_drift(0.3, 5), DriftProfile::StepChange);
    }

    #[test]
    fn drift_classification_linear() {
        assert_eq!(classify_drift(0.3, 20), DriftProfile::Linear);
    }

    #[test]
    fn per_subcarrier_zscores_correct() {
        let current = vec![2.0, 4.0];
        let reference = vec![1.0, 2.0];
        let variance = vec![1.0, 4.0];
        let z = per_subcarrier_zscores(&current, &reference, &variance);
        assert_eq!(z.len(), 2);
        assert!((z[0] - 1.0).abs() < 1e-5);
        assert!((z[1] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn outlier_subcarriers_detected() {
        let current = vec![1.0, 100.0, 1.0, 200.0];
        let reference = vec![1.0, 1.0, 1.0, 1.0];
        let variance = vec![1.0, 1.0, 1.0, 1.0];
        let outliers = outlier_subcarriers(&current, &reference, &variance, 3.0);
        assert!(outliers.contains(&1));
        assert!(outliers.contains(&3));
        assert!(!outliers.contains(&0));
        assert!(!outliers.contains(&2));
    }

    #[test]
    fn reset_stale_counter() {
        let mut state = CoherenceState::new(4, 0.99);
        state.initialize(&[1.0, 2.0, 3.0, 4.0]);
        let _ = state.update(&[10.0, 20.0, 30.0, 40.0]).unwrap();
        assert!(state.stale_count() > 0);
        state.reset_stale();
        assert_eq!(state.stale_count(), 0);
    }

    #[test]
    fn reference_and_variance_accessible() {
        let state = CoherenceState::new(3, 0.85);
        assert_eq!(state.reference().len(), 3);
        assert_eq!(state.variance().len(), 3);
    }

    #[test]
    fn coherence_score_with_high_variance() {
        let current = vec![5.0, 6.0, 7.0];
        let reference = vec![1.0, 2.0, 3.0];
        let variance = vec![100.0, 100.0, 100.0]; // high variance
        let score = coherence_score(&current, &reference, &variance);
        // With high variance, deviation is relatively small
        assert!(score > 0.5, "High variance should tolerate deviation, got {}", score);
    }
}
