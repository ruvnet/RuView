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
        }
    }

    /// Create with defaults (window = 60, z_threshold = 2.5).
    #[must_use]
    pub fn default_config() -> Self {
        Self::new(60, 2.5)
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

        // Clinical thresholds for respiratory rate (adult)
        if rr < 4.0 && reading.respiratory_rate.confidence > 0.3 {
            alerts.push(AnomalyAlert {
                vital_type: "respiratory".to_string(),
                alert_type: "apnea".to_string(),
                severity: 0.9,
                message: format!("Possible apnea detected: RR = {rr:.1} BPM"),
            });
        } else if rr > 30.0 && reading.respiratory_rate.confidence > 0.3 {
            alerts.push(AnomalyAlert {
                vital_type: "respiratory".to_string(),
                alert_type: "tachypnea".to_string(),
                severity: ((rr - 30.0) / 20.0).clamp(0.3, 1.0),
                message: format!("Elevated respiratory rate: RR = {rr:.1} BPM"),
            });
        } else if rr < 8.0 && reading.respiratory_rate.confidence > 0.3 {
            alerts.push(AnomalyAlert {
                vital_type: "respiratory".to_string(),
                alert_type: "bradypnea".to_string(),
                severity: ((8.0 - rr) / 8.0).clamp(0.3, 0.8),
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

        if hr > 100.0 && reading.heart_rate.confidence > 0.3 {
            alerts.push(AnomalyAlert {
                vital_type: "cardiac".to_string(),
                alert_type: "tachycardia".to_string(),
                severity: ((hr - 100.0) / 80.0).clamp(0.3, 1.0),
                message: format!("Elevated heart rate: HR = {hr:.1} BPM"),
            });
        } else if hr < 50.0 && reading.heart_rate.confidence > 0.3 {
            alerts.push(AnomalyAlert {
                vital_type: "cardiac".to_string(),
                alert_type: "bradycardia".to_string(),
                severity: ((50.0 - hr) / 30.0).clamp(0.3, 1.0),
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
