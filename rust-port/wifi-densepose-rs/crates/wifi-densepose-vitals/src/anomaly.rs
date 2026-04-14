//! Vital sign anomaly detection.
//!
//! Monitors vital sign readings for anomalies (apnea, tachycardia,
//! bradycardia, sudden changes) using z-score detection with
//! running mean and standard deviation.
//!
//! Modeled on the DNA biomarker anomaly detection pattern from
//! `vendor/ruvector/examples/dna`, using Welford's online algorithm
//! for numerically stable running statistics.

use crate::types::VitalReading;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// EML Adaptive Thresholds (Improvement 2)
// ─────────────────────────────────────────────────────────────────────────────

/// Lightweight EML (exp-ln) model for learning personalized anomaly
/// thresholds from patient characteristics.
///
/// The EML operator `eml(x, y) = exp(x) - ln(y)` is the continuous-math
/// analog of the NAND gate. A depth-2 binary tree can learn personalized
/// clinical thresholds from (age, baseline_hr, baseline_rr) data.
///
/// Based on: Odrzywolel 2026, "All elementary functions from a single
/// operator" (arXiv:2603.21852v2)
#[derive(Debug, Clone)]
pub struct EmlThresholdModel {
    /// Leaf parameters: [bias, scale] for each leaf node.
    /// A depth-2 tree has 4 leaves, so 8 parameters.
    params: Vec<f64>,
    /// Whether the model has been trained.
    trained: bool,
}

impl EmlThresholdModel {
    /// Create a new untrained threshold model.
    ///
    /// Depth-2 EML tree with 3 inputs (age, baseline_hr, baseline_rr)
    /// and 1 output (threshold adjustment factor).
    /// Contains 13 trainable parameters.
    #[must_use]
    pub fn new() -> Self {
        // 4 leaves * 2 (bias+scale) + 3 internal node scales + 2 input routing
        Self {
            params: vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.01, 0.01, 0.01, 0.0, 0.0],
            trained: false,
        }
    }

    /// Predict a threshold adjustment factor from patient characteristics.
    ///
    /// Returns a multiplier near 1.0: values > 1.0 raise the threshold
    /// (more tolerant), values < 1.0 lower it (more sensitive).
    ///
    /// Inputs: `[age_normalized, baseline_hr_normalized, baseline_rr_normalized]`
    /// where each is scaled to approximately [0, 1].
    #[must_use]
    pub fn predict(&self, inputs: &[f64; 3]) -> f64 {
        if !self.trained {
            return 1.0; // No adjustment when untrained.
        }

        let p = &self.params;
        // Leaf evaluations.
        let leaf0 = p[1] * (inputs[0] + p[0]);
        let leaf1 = p[3] * (inputs[1] + p[2]);
        let leaf2 = p[5] * (inputs[2] + p[4]);
        let leaf3 = p[7] * (inputs[0] * inputs[1] + p[6]); // cross-term

        // Internal nodes: eml(left, right) = exp(left) - ln(right)
        let internal0 = p[8] * (leaf0.clamp(-5.0, 5.0).exp() - leaf1.abs().max(1e-10).ln());
        let internal1 = p[9] * (leaf2.clamp(-5.0, 5.0).exp() - leaf3.abs().max(1e-10).ln());
        let root = p[10] * (internal0.clamp(-5.0, 5.0).exp() - internal1.abs().max(1e-10).ln());

        // Sigmoid to keep the multiplier in a reasonable range [0.5, 2.0].
        let sigmoid = 1.0 / (1.0 + (-root + p[11]).exp());
        0.5 + 1.5 * sigmoid // Maps [0,1] -> [0.5, 2.0]
    }

    /// Train the model using coordinate descent on (inputs, target_factor) data.
    ///
    /// - `data`: Vec of (patient_features, optimal_threshold_factor) pairs.
    /// - `epochs`: number of coordinate descent passes.
    ///
    /// Returns final MSE.
    pub fn train(&mut self, data: &[([f64; 3], f64)], epochs: usize) -> f64 {
        if data.is_empty() {
            return f64::MAX;
        }

        // Temporarily mark as trained so predict() uses the model.
        self.trained = true;

        let mut best_loss = self.compute_loss(data);
        let mut step = 0.05;

        for epoch in 0..epochs {
            let _ = epoch;
            for i in 0..self.params.len() {
                let original = self.params[i];

                self.params[i] = original + step;
                let loss_plus = self.compute_loss(data);

                self.params[i] = original - step;
                let loss_minus = self.compute_loss(data);

                if loss_plus < best_loss && loss_plus <= loss_minus {
                    self.params[i] = original + step;
                    best_loss = loss_plus;
                } else if loss_minus < best_loss {
                    self.params[i] = original - step;
                    best_loss = loss_minus;
                } else {
                    self.params[i] = original;
                }
            }
            step *= 0.995;
        }

        best_loss
    }

    fn compute_loss(&self, data: &[([f64; 3], f64)]) -> f64 {
        let mut total = 0.0;
        for (inputs, target) in data {
            let pred = self.predict(inputs);
            let diff = pred - target;
            if diff.is_finite() {
                total += diff * diff;
            } else {
                total += 1e6;
            }
        }
        total / data.len() as f64
    }

    /// Whether the model has been trained with patient data.
    #[must_use]
    pub fn is_trained(&self) -> bool {
        self.trained
    }
}

/// An anomaly alert generated from vital sign analysis.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AnomalyAlert {
    /// Type of vital sign: `"respiratory"` or `"cardiac"`.
    pub vital_type: String,
    /// Type of anomaly: `"apnea"`, `"tachypnea"`, `"bradypnea"`,
    /// `"tachycardia"`, `"bradycardia"`, `"sudden_change"`.
    pub alert_type: String,
    /// Severity [0.0, 1.0].
    pub severity: f64,
    /// Human-readable description.
    pub message: String,
}

/// Welford online statistics accumulator.
#[derive(Debug, Clone)]
struct WelfordStats {
    count: u64,
    mean: f64,
    m2: f64,
}

impl WelfordStats {
    fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
        }
    }

    fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    fn variance(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        self.m2 / (self.count - 1) as f64
    }

    fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    fn z_score(&self, value: f64) -> f64 {
        let sd = self.std_dev();
        if sd < 1e-10 {
            return 0.0;
        }
        (value - self.mean) / sd
    }
}

/// Vital sign anomaly detector using z-score analysis with
/// running statistics.
///
/// Supports optional EML-based adaptive thresholds (Improvement 2):
/// when a trained [`EmlThresholdModel`] is attached, clinical thresholds
/// are personalized based on patient characteristics instead of using
/// fixed values (apnea < 4 BPM, tachypnea > 30 BPM, etc.).
pub struct VitalAnomalyDetector {
    /// Running statistics for respiratory rate.
    rr_stats: WelfordStats,
    /// Running statistics for heart rate.
    hr_stats: WelfordStats,
    /// Recent respiratory rate values for windowed analysis.
    rr_history: Vec<f64>,
    /// Recent heart rate values for windowed analysis.
    hr_history: Vec<f64>,
    /// Maximum window size for history.
    window: usize,
    /// Z-score threshold for anomaly detection.
    z_threshold: f64,
    /// Optional EML model for adaptive clinical thresholds.
    /// When trained, adjusts the fixed thresholds (4.0, 8.0, 30.0, 50.0, 100.0)
    /// based on patient characteristics (age, baseline HR, baseline RR).
    ///
    /// Based on: Odrzywolel 2026, arXiv:2603.21852v2
    eml_threshold_model: Option<EmlThresholdModel>,
    /// Patient characteristics for adaptive thresholds: [age_norm, baseline_hr_norm, baseline_rr_norm].
    patient_features: Option<[f64; 3]>,
}

impl VitalAnomalyDetector {
    /// Create a new anomaly detector.
    ///
    /// - `window`: number of recent readings to retain.
    /// - `z_threshold`: z-score threshold for anomaly alerts (default: 2.5).
    #[must_use]
    pub fn new(window: usize, z_threshold: f64) -> Self {
        Self {
            rr_stats: WelfordStats::new(),
            hr_stats: WelfordStats::new(),
            rr_history: Vec::with_capacity(window),
            hr_history: Vec::with_capacity(window),
            window,
            z_threshold,
            eml_threshold_model: None,
            patient_features: None,
        }
    }

    /// Create with defaults (window = 60, z_threshold = 2.5).
    #[must_use]
    pub fn default_config() -> Self {
        Self::new(60, 2.5)
    }

    /// Attach a trained EML threshold model for personalized anomaly
    /// thresholds based on patient characteristics.
    ///
    /// Call [`set_patient_features`] to provide the patient data.
    pub fn set_eml_threshold_model(&mut self, model: EmlThresholdModel) {
        self.eml_threshold_model = Some(model);
    }

    /// Set patient characteristics for adaptive thresholds.
    ///
    /// - `age`: patient age in years (will be normalized internally).
    /// - `baseline_hr`: resting heart rate in BPM.
    /// - `baseline_rr`: resting respiratory rate in BPM.
    pub fn set_patient_features(&mut self, age: f64, baseline_hr: f64, baseline_rr: f64) {
        // Normalize to [0, 1] ranges for the EML model.
        self.patient_features = Some([
            (age / 100.0).clamp(0.0, 1.0),
            (baseline_hr / 200.0).clamp(0.0, 1.0),
            (baseline_rr / 40.0).clamp(0.0, 1.0),
        ]);
    }

    /// Get the threshold adjustment factor from the EML model.
    /// Returns 1.0 (no adjustment) if no model is attached or not trained.
    fn threshold_factor(&self) -> f64 {
        match (&self.eml_threshold_model, &self.patient_features) {
            (Some(model), Some(features)) if model.is_trained() => model.predict(features),
            _ => 1.0,
        }
    }

    /// Check a vital sign reading for anomalies.
    ///
    /// Updates running statistics and returns a list of detected
    /// anomaly alerts (may be empty if all readings are normal).
    pub fn check(&mut self, reading: &VitalReading) -> Vec<AnomalyAlert> {
        let mut alerts = Vec::new();

        let rr = reading.respiratory_rate.value_bpm;
        let hr = reading.heart_rate.value_bpm;

        // Update histories
        self.rr_history.push(rr);
        if self.rr_history.len() > self.window {
            self.rr_history.remove(0);
        }
        self.hr_history.push(hr);
        if self.hr_history.len() > self.window {
            self.hr_history.remove(0);
        }

        // Update running statistics
        self.rr_stats.update(rr);
        self.hr_stats.update(hr);

        // Need at least a few readings before detecting anomalies
        if self.rr_stats.count < 5 {
            return alerts;
        }

        // --- Respiratory rate anomalies ---
        let rr_z = self.rr_stats.z_score(rr);

        // EML Improvement 2: Adaptive clinical thresholds.
        // When a trained EML model is attached, the fixed thresholds are
        // adjusted by a learned factor based on patient characteristics.
        // factor > 1.0 = more tolerant, factor < 1.0 = more sensitive.
        // Based on: Odrzywolel 2026, arXiv:2603.21852v2
        let factor = self.threshold_factor();
        let apnea_thresh = 4.0 * factor;
        let tachypnea_thresh = 30.0 * factor;
        let bradypnea_thresh = 8.0 * factor;
        let tachycardia_thresh = 100.0 * factor;
        let bradycardia_thresh = 50.0 / factor; // Inverse: more tolerant = lower floor.

        // Clinical thresholds for respiratory rate (adult),
        // optionally personalized via EML model.
        if rr < apnea_thresh && reading.respiratory_rate.confidence > 0.3 {
            alerts.push(AnomalyAlert {
                vital_type: "respiratory".to_string(),
                alert_type: "apnea".to_string(),
                severity: 0.9,
                message: format!("Possible apnea detected: RR = {rr:.1} BPM"),
            });
        } else if rr > tachypnea_thresh && reading.respiratory_rate.confidence > 0.3 {
            alerts.push(AnomalyAlert {
                vital_type: "respiratory".to_string(),
                alert_type: "tachypnea".to_string(),
                severity: ((rr - tachypnea_thresh) / 20.0).clamp(0.3, 1.0),
                message: format!("Elevated respiratory rate: RR = {rr:.1} BPM"),
            });
        } else if rr < bradypnea_thresh && reading.respiratory_rate.confidence > 0.3 {
            alerts.push(AnomalyAlert {
                vital_type: "respiratory".to_string(),
                alert_type: "bradypnea".to_string(),
                severity: ((bradypnea_thresh - rr) / bradypnea_thresh).clamp(0.3, 0.8),
                message: format!("Low respiratory rate: RR = {rr:.1} BPM"),
            });
        }

        // Z-score based sudden change detection for RR
        if rr_z.abs() > self.z_threshold {
            alerts.push(AnomalyAlert {
                vital_type: "respiratory".to_string(),
                alert_type: "sudden_change".to_string(),
                severity: (rr_z.abs() / (self.z_threshold * 2.0)).clamp(0.2, 1.0),
                message: format!(
                    "Sudden respiratory rate change: z-score = {rr_z:.2} (RR = {rr:.1} BPM)"
                ),
            });
        }

        // --- Heart rate anomalies ---
        let hr_z = self.hr_stats.z_score(hr);

        if hr > tachycardia_thresh && reading.heart_rate.confidence > 0.3 {
            alerts.push(AnomalyAlert {
                vital_type: "cardiac".to_string(),
                alert_type: "tachycardia".to_string(),
                severity: ((hr - tachycardia_thresh) / 80.0).clamp(0.3, 1.0),
                message: format!("Elevated heart rate: HR = {hr:.1} BPM"),
            });
        } else if hr < bradycardia_thresh && reading.heart_rate.confidence > 0.3 {
            alerts.push(AnomalyAlert {
                vital_type: "cardiac".to_string(),
                alert_type: "bradycardia".to_string(),
                severity: ((bradycardia_thresh - hr) / 30.0).clamp(0.3, 1.0),
                message: format!("Low heart rate: HR = {hr:.1} BPM"),
            });
        }

        // Z-score based sudden change detection for HR
        if hr_z.abs() > self.z_threshold {
            alerts.push(AnomalyAlert {
                vital_type: "cardiac".to_string(),
                alert_type: "sudden_change".to_string(),
                severity: (hr_z.abs() / (self.z_threshold * 2.0)).clamp(0.2, 1.0),
                message: format!(
                    "Sudden heart rate change: z-score = {hr_z:.2} (HR = {hr:.1} BPM)"
                ),
            });
        }

        alerts
    }

    /// Reset all accumulated statistics and history.
    ///
    /// Note: EML threshold model and patient features are preserved
    /// across resets since they represent learned/configured state.
    pub fn reset(&mut self) {
        self.rr_stats = WelfordStats::new();
        self.hr_stats = WelfordStats::new();
        self.rr_history.clear();
        self.hr_history.clear();
    }

    /// Number of readings processed so far.
    #[must_use]
    pub fn reading_count(&self) -> u64 {
        self.rr_stats.count
    }

    /// Current running mean for respiratory rate.
    #[must_use]
    pub fn rr_mean(&self) -> f64 {
        self.rr_stats.mean
    }

    /// Current running mean for heart rate.
    #[must_use]
    pub fn hr_mean(&self) -> f64 {
        self.hr_stats.mean
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{VitalEstimate, VitalReading, VitalStatus};

    fn make_reading(rr_bpm: f64, hr_bpm: f64) -> VitalReading {
        VitalReading {
            respiratory_rate: VitalEstimate {
                value_bpm: rr_bpm,
                confidence: 0.8,
                status: VitalStatus::Valid,
            },
            heart_rate: VitalEstimate {
                value_bpm: hr_bpm,
                confidence: 0.8,
                status: VitalStatus::Valid,
            },
            subcarrier_count: 56,
            signal_quality: 0.9,
            timestamp_secs: 0.0,
        }
    }

    #[test]
    fn no_alerts_for_normal_readings() {
        let mut det = VitalAnomalyDetector::new(30, 2.5);
        // Feed 20 normal readings
        for _ in 0..20 {
            let alerts = det.check(&make_reading(15.0, 72.0));
            // After warmup, should have no alerts
            if det.reading_count() > 5 {
                assert!(alerts.is_empty(), "normal readings should not trigger alerts");
            }
        }
    }

    #[test]
    fn detects_tachycardia() {
        let mut det = VitalAnomalyDetector::new(30, 2.5);
        // Warmup with normal
        for _ in 0..10 {
            det.check(&make_reading(15.0, 72.0));
        }
        // Elevated HR
        let alerts = det.check(&make_reading(15.0, 130.0));
        let tachycardia = alerts
            .iter()
            .any(|a| a.alert_type == "tachycardia");
        assert!(tachycardia, "should detect tachycardia at 130 BPM");
    }

    #[test]
    fn detects_bradycardia() {
        let mut det = VitalAnomalyDetector::new(30, 2.5);
        for _ in 0..10 {
            det.check(&make_reading(15.0, 72.0));
        }
        let alerts = det.check(&make_reading(15.0, 40.0));
        let brady = alerts.iter().any(|a| a.alert_type == "bradycardia");
        assert!(brady, "should detect bradycardia at 40 BPM");
    }

    #[test]
    fn detects_apnea() {
        let mut det = VitalAnomalyDetector::new(30, 2.5);
        for _ in 0..10 {
            det.check(&make_reading(15.0, 72.0));
        }
        let alerts = det.check(&make_reading(2.0, 72.0));
        let apnea = alerts.iter().any(|a| a.alert_type == "apnea");
        assert!(apnea, "should detect apnea at 2 BPM");
    }

    #[test]
    fn detects_tachypnea() {
        let mut det = VitalAnomalyDetector::new(30, 2.5);
        for _ in 0..10 {
            det.check(&make_reading(15.0, 72.0));
        }
        let alerts = det.check(&make_reading(35.0, 72.0));
        let tachypnea = alerts.iter().any(|a| a.alert_type == "tachypnea");
        assert!(tachypnea, "should detect tachypnea at 35 BPM");
    }

    #[test]
    fn detects_sudden_change() {
        let mut det = VitalAnomalyDetector::new(30, 2.0);
        // Build a stable baseline
        for _ in 0..30 {
            det.check(&make_reading(15.0, 72.0));
        }
        // Sudden jump (still in normal clinical range but statistically anomalous)
        let alerts = det.check(&make_reading(15.0, 95.0));
        let sudden = alerts.iter().any(|a| a.alert_type == "sudden_change");
        assert!(sudden, "should detect sudden HR change from 72 to 95 BPM");
    }

    #[test]
    fn reset_clears_state() {
        let mut det = VitalAnomalyDetector::new(30, 2.5);
        for _ in 0..10 {
            det.check(&make_reading(15.0, 72.0));
        }
        assert!(det.reading_count() > 0);
        det.reset();
        assert_eq!(det.reading_count(), 0);
    }

    #[test]
    fn welford_stats_basic() {
        let mut stats = WelfordStats::new();
        stats.update(10.0);
        stats.update(20.0);
        stats.update(30.0);
        assert!((stats.mean - 20.0).abs() < 1e-10);
        assert!(stats.std_dev() > 0.0);
    }

    #[test]
    fn welford_z_score() {
        let mut stats = WelfordStats::new();
        for i in 0..100 {
            stats.update(50.0 + (i % 3) as f64);
        }
        // A value far from the mean should have a high z-score
        let z = stats.z_score(100.0);
        assert!(z > 2.0, "z-score for extreme value should be > 2: {z}");
    }

    #[test]
    fn running_means_are_tracked() {
        let mut det = VitalAnomalyDetector::new(30, 2.5);
        for _ in 0..10 {
            det.check(&make_reading(16.0, 75.0));
        }
        assert!((det.rr_mean() - 16.0).abs() < 0.5);
        assert!((det.hr_mean() - 75.0).abs() < 0.5);
    }

    #[test]
    fn eml_threshold_model_untrained_no_change() {
        // An untrained EML model should not change anomaly detection behavior.
        let mut det = VitalAnomalyDetector::new(30, 2.5);
        let model = EmlThresholdModel::new();
        assert!(!model.is_trained());
        det.set_eml_threshold_model(model);
        det.set_patient_features(45.0, 72.0, 15.0);

        // Should still detect tachycardia at 130 BPM (unchanged threshold).
        for _ in 0..10 {
            det.check(&make_reading(15.0, 72.0));
        }
        let alerts = det.check(&make_reading(15.0, 130.0));
        let tachycardia = alerts.iter().any(|a| a.alert_type == "tachycardia");
        assert!(tachycardia, "untrained EML should not change behavior");
    }

    #[test]
    fn eml_threshold_model_without_patient_features() {
        // EML model should have no effect if patient features aren't set.
        let mut det = VitalAnomalyDetector::new(30, 2.5);
        let mut model = EmlThresholdModel::new();
        // Fake "training" by setting trained flag.
        model.trained = true;
        det.set_eml_threshold_model(model);
        // No patient features set.

        assert_eq!(det.threshold_factor(), 1.0);
    }

    #[test]
    fn eml_threshold_model_predict_returns_reasonable() {
        let model = EmlThresholdModel::new();
        // Untrained should return 1.0 (no adjustment).
        assert!((model.predict(&[0.5, 0.5, 0.5]) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn severity_is_clamped() {
        let mut det = VitalAnomalyDetector::new(30, 2.5);
        for _ in 0..10 {
            det.check(&make_reading(15.0, 72.0));
        }
        let alerts = det.check(&make_reading(15.0, 200.0));
        for alert in &alerts {
            assert!(
                alert.severity >= 0.0 && alert.severity <= 1.0,
                "severity should be in [0,1]: {}",
                alert.severity,
            );
        }
    }
}
