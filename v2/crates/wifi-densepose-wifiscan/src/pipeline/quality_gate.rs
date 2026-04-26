//! Stage 6: Signal quality gate.
//!
//! Evaluates signal quality using three factors inspired by the ruQu
//! three-filter architecture (structural integrity, distribution drift,
//! evidence accumulation):
//!
//! - **Structural**: number of active BSSIDs (graph connectivity proxy).
//! - **Shift**: RSSI drift from running baseline.
//! - **Evidence**: accumulated weighted variance evidence.
//!
//! This is a pure-Rust implementation. When the `ruqu` crate becomes
//! available, the inner filter can be replaced with `FilterPipeline`.

/// Configuration for the quality gate.
#[derive(Debug, Clone)]
pub struct QualityGateConfig {
    /// Minimum active BSSIDs for a "Permit" verdict.
    pub min_bssids: usize,
    /// Evidence threshold for "Permit" (accumulated variance).
    pub evidence_threshold: f64,
    /// RSSI drift threshold (dBm) for triggering a "Warn".
    pub drift_threshold: f64,
    /// Maximum evidence decay per frame.
    pub evidence_decay: f64,
}

impl Default for QualityGateConfig {
    fn default() -> Self {
        Self {
            min_bssids: 3,
            evidence_threshold: 0.5,
            drift_threshold: 10.0,
            evidence_decay: 0.95,
        }
    }
}

/// Quality gate combining structural, shift, and evidence filters.
pub struct QualityGate {
    config: QualityGateConfig,
    /// Accumulated evidence score.
    evidence: f64,
    /// Running mean RSSI baseline for drift detection.
    prev_mean_rssi: Option<f64>,
    /// EMA smoothing factor for drift baseline.
    alpha: f64,
}

impl QualityGate {
    /// Create a quality gate with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(QualityGateConfig::default())
    }

    /// Create a quality gate with custom configuration.
    #[must_use]
    pub fn with_config(config: QualityGateConfig) -> Self {
        Self {
            config,
            evidence: 0.0,
            prev_mean_rssi: None,
            alpha: 0.3,
        }
    }

    /// Evaluate signal quality.
    ///
    /// - `bssid_count`: number of active BSSIDs.
    /// - `mean_rssi_dbm`: mean RSSI across all BSSIDs.
    /// - `mean_correlation`: mean cross-BSSID correlation (spectral gap proxy).
    /// - `motion_score`: smoothed motion score from the estimator.
    ///
    /// Returns a `QualityResult` with verdict and quality score.
    pub fn evaluate(
        &mut self,
        bssid_count: usize,
        mean_rssi_dbm: f64,
        mean_correlation: f64,
        motion_score: f32,
    ) -> QualityResult {
        // --- Filter 1: Structural (BSSID count) ---
        let structural_ok = bssid_count >= self.config.min_bssids;

        // --- Filter 2: Shift (RSSI drift detection) ---
        let drift = if let Some(prev) = self.prev_mean_rssi {
            (mean_rssi_dbm - prev).abs()
        } else {
            0.0
        };
        // Update baseline with EMA
        self.prev_mean_rssi = Some(match self.prev_mean_rssi {
            Some(prev) => self.alpha * mean_rssi_dbm + (1.0 - self.alpha) * prev,
            None => mean_rssi_dbm,
        });
        let drift_detected = drift > self.config.drift_threshold;

        // --- Filter 3: Evidence accumulation ---
        // Motion and correlation both contribute positive evidence.
        let evidence_input = f64::from(motion_score) * 0.7 + mean_correlation * 0.3;
        self.evidence = self.evidence * self.config.evidence_decay + evidence_input;

        // --- Quality score ---
        let quality = compute_quality_score(
            bssid_count,
            f64::from(motion_score),
            mean_correlation,
            drift_detected,
        );

        // --- Verdict decision ---
        let verdict = if !structural_ok {
            Verdict::Deny("insufficient BSSIDs".to_string())
        } else if self.evidence < self.config.evidence_threshold * 0.5 || drift_detected {
            Verdict::Defer
        } else {
            Verdict::Permit
        };

        QualityResult {
            verdict,
            quality,
            drift_detected,
        }
    }

    /// Reset the gate state.
    pub fn reset(&mut self) {
        self.evidence = 0.0;
        self.prev_mean_rssi = None;
    }
}

impl Default for QualityGate {
    fn default() -> Self {
        Self::new()
    }
}

/// Quality verdict from the gate.
#[derive(Debug, Clone)]
pub struct QualityResult {
    /// Filter decision.
    pub verdict: Verdict,
    /// Signal quality score [0, 1].
    pub quality: f64,
    /// Whether environmental drift was detected.
    pub drift_detected: bool,
}

/// Simplified quality gate verdict.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// Reading passed all quality gates and is reliable.
    Permit,
    /// Reading failed quality checks with a reason.
    Deny(String),
    /// Evidence still accumulating.
    Defer,
}

impl Verdict {
    /// Returns true if this verdict permits the reading.
    #[must_use]
    pub fn is_permit(&self) -> bool {
        matches!(self, Self::Permit)
    }
}

/// Compute a quality score from pipeline metrics.
#[allow(clippy::cast_precision_loss)]
fn compute_quality_score(
    n_active: usize,
    weighted_variance: f64,
    mean_correlation: f64,
    drift: bool,
) -> f64 {
    // 1. Number of active BSSIDs (more = better, diminishing returns)
    let bssid_factor = (n_active as f64 / 10.0).min(1.0);

    // 2. Evidence strength (higher weighted variance = more signal)
    let evidence_factor = (weighted_variance * 10.0).min(1.0);

    // 3. Correlation coherence (moderate correlation is best)
    let corr_factor = 1.0 - (mean_correlation - 0.5).abs() * 2.0;

    // 4. Drift penalty
    let drift_penalty = if drift { 0.7 } else { 1.0 };

    let raw =
        (bssid_factor * 0.3 + evidence_factor * 0.4 + corr_factor.max(0.0) * 0.3) * drift_penalty;
    raw.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_gate_creates_ok() {
        let gate = QualityGate::new();
        assert!((gate.evidence - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn evaluate_with_good_signal() {
        let mut gate = QualityGate::new();
        // Pump several frames to build evidence.
        for _ in 0..20 {
            gate.evaluate(10, -60.0, 0.5, 0.3);
        }
        let result = gate.evaluate(10, -60.0, 0.5, 0.3);
        assert!(result.quality > 0.0, "quality should be positive");
        assert!(result.verdict.is_permit(), "should permit good signal");
    }

    #[test]
    fn too_few_bssids_denied() {
        let mut gate = QualityGate::new();
        let result = gate.evaluate(1, -60.0, 0.5, 0.3);
        assert!(
            matches!(result.verdict, Verdict::Deny(_)),
            "too few BSSIDs should be denied"
        );
    }

    #[test]
    fn quality_increases_with_more_bssids() {
        let q_few = compute_quality_score(3, 0.1, 0.5, false);
        let q_many = compute_quality_score(10, 0.1, 0.5, false);
        assert!(q_many > q_few, "more BSSIDs should give higher quality");
    }

    #[test]
    fn drift_reduces_quality() {
        let q_stable = compute_quality_score(5, 0.1, 0.5, false);
        let q_drift = compute_quality_score(5, 0.1, 0.5, true);
        assert!(q_drift < q_stable, "drift should reduce quality");
    }

    #[test]
    fn verdict_is_permit_check() {
        assert!(Verdict::Permit.is_permit());
        assert!(!Verdict::Deny("test".to_string()).is_permit());
        assert!(!Verdict::Defer.is_permit());
    }

    #[test]
    fn default_creates_gate() {
        let _gate = QualityGate::default();
    }

    #[test]
    fn reset_clears_state() {
        let mut gate = QualityGate::new();
        gate.evaluate(10, -60.0, 0.5, 0.3);
        gate.reset();
        assert!(gate.prev_mean_rssi.is_none());
        assert!((gate.evidence - 0.0).abs() < f64::EPSILON);
    }
}
