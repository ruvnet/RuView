//! # Tier 1: Medical and Wearable Systems
//!
//! Monitoring, assistive devices, prosthetics.
//!
//! ## What Changes
//! - Continuous sensing with sparse spikes
//! - One-shot learning for personalization
//! - Homeostasis instead of static thresholds
//!
//! ## Why This Matters
//! - Devices adapt to the person, not the average
//! - Low energy, always-on, private by default
//! - Early detection beats intervention
//!
//! This is practical and defensible.

use std::collections::HashMap;

/// Physiological measurement
#[derive(Clone, Debug)]
pub struct BioSignal {
    pub timestamp_ms: u64,
    pub signal_type: SignalType,
    pub value: f32,
    pub source: SignalSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SignalType {
    HeartRate,
    HeartRateVariability,
    SpO2,
    SkinConductance,
    Temperature,
    Motion,
    Sleep,
    Stress,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SignalSource {
    Wrist,
    Chest,
    Finger,
    Derived,
}

/// Alert for user or medical professional
#[derive(Clone, Debug)]
pub struct HealthAlert {
    pub signal: BioSignal,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub recommendation: String,
    pub confidence: f32,
}

#[derive(Clone, Debug)]
pub enum AlertType {
    /// Immediate attention needed
    Acute { condition: String },
    /// Trend requiring monitoring
    Trend {
        direction: TrendDirection,
        duration_hours: f32,
    },
    /// Deviation from personal baseline
    PersonalAnomaly { baseline: f32, deviation: f32 },
    /// Lifestyle recommendation
    Wellness { category: String },
}

#[derive(Clone, Debug)]
pub enum TrendDirection {
    Rising,
    Falling,
    Unstable,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    Info,
    Warning,
    Urgent,
    Emergency,
}

/// Personal baseline learned through one-shot learning
#[derive(Clone, Debug)]
pub struct PersonalBaseline {
    pub signal_type: SignalType,
    pub mean: f32,
    pub std_dev: f32,
    pub circadian_pattern: Vec<f32>, // 24 hourly values
    pub adaptation_rate: f32,
    pub samples_seen: u64,
}

impl PersonalBaseline {
    pub fn new(signal_type: SignalType) -> Self {
        Self {
            signal_type,
            mean: 0.0,
            std_dev: 1.0,
            circadian_pattern: vec![0.0; 24],
            adaptation_rate: 0.1,
            samples_seen: 0,
        }
    }

    /// One-shot learning update using BTSP-style adaptation
    pub fn learn_one_shot(&mut self, value: f32, hour_of_day: usize) {
        // Fast initial learning, slower adaptation later
        let rate = if self.samples_seen < 100 {
            0.5 // Fast initialization
        } else {
            self.adaptation_rate
        };

        // Update mean (eligibility trace style)
        let error = value - self.mean;
        self.mean += rate * error;

        // Update std dev
        let variance_error = error.abs() - self.std_dev;
        self.std_dev += rate * 0.5 * variance_error;
        self.std_dev = self.std_dev.max(0.1); // Minimum std dev

        // Update circadian pattern
        if hour_of_day < 24 {
            self.circadian_pattern[hour_of_day] =
                self.circadian_pattern[hour_of_day] * (1.0 - rate) + value * rate;
        }

        self.samples_seen += 1;
    }

    /// Check if value is anomalous for this person
    pub fn is_anomalous(&self, value: f32, hour_of_day: usize) -> Option<f32> {
        let expected = if hour_of_day < 24 && self.samples_seen > 100 {
            self.circadian_pattern[hour_of_day]
        } else {
            self.mean
        };

        let z_score = (value - expected).abs() / self.std_dev;

        if z_score > 2.5 {
            Some(z_score)
        } else {
            None
        }
    }
}

/// Homeostatic controller that maintains optimal ranges
pub struct HomeostaticController {
    pub target: f32,
    pub tolerance: f32,
    pub integral: f32,
    pub last_error: f32,
    pub kp: f32,
    pub ki: f32,
    pub kd: f32,
}

impl HomeostaticController {
    pub fn new(target: f32, tolerance: f32) -> Self {
        Self {
            target,
            tolerance,
            integral: 0.0,
            last_error: 0.0,
            kp: 1.0,
            ki: 0.1,
            kd: 0.05,
        }
    }

    /// Compute homeostatic response
    pub fn respond(&mut self, current: f32) -> HomeostasisResponse {
        let error = current - self.target;

        // Within tolerance - no action needed
        if error.abs() <= self.tolerance {
            self.integral *= 0.9; // Decay integral
            return HomeostasisResponse::Stable;
        }

        // PID-style response
        self.integral += error;
        self.integral = self.integral.clamp(-10.0, 10.0);

        let derivative = error - self.last_error;
        self.last_error = error;

        let response = self.kp * error + self.ki * self.integral + self.kd * derivative;

        if response.abs() > 5.0 {
            HomeostasisResponse::Urgent(response)
        } else if response.abs() > 2.0 {
            HomeostasisResponse::Adjust(response)
        } else {
            HomeostasisResponse::Monitor
        }
    }
}

#[derive(Clone, Debug)]
pub enum HomeostasisResponse {
    Stable,
    Monitor,
    Adjust(f32),
    Urgent(f32),
}

/// Sparse spike encoder for low-power continuous sensing
pub struct SparseEncoder {
    pub last_value: f32,
    pub threshold: f32,
    pub spike_count: u64,
}

impl SparseEncoder {
    pub fn new(threshold: f32) -> Self {
        Self {
            last_value: 0.0,
            threshold,
            spike_count: 0,
        }
    }

    /// Only emit spike if change exceeds threshold
    pub fn encode(&mut self, value: f32) -> Option<f32> {
        let delta = (value - self.last_value).abs();

        if delta > self.threshold {
            self.last_value = value;
            self.spike_count += 1;
            Some(value)
        } else {
            None
        }
    }

    pub fn compression_ratio(&self, total_samples: u64) -> f32 {
        if self.spike_count == 0 {
            return f32::INFINITY;
        }
        total_samples as f32 / self.spike_count as f32
    }
}

/// Main medical wearable system
pub struct MedicalWearableSystem {
    /// Personal baselines per signal type (one-shot learned)
    pub baselines: HashMap<SignalType, PersonalBaseline>,
    /// Homeostatic controllers
    pub homeostasis: HashMap<SignalType, HomeostaticController>,
    /// Sparse encoders for low power
    pub encoders: HashMap<SignalType, SparseEncoder>,
    /// Recent alerts
    pub alert_history: Vec<HealthAlert>,
    /// Privacy: all processing local
    pub samples_processed: u64,
}

impl MedicalWearableSystem {
    pub fn new() -> Self {
        let mut baselines = HashMap::new();
        let mut homeostasis = HashMap::new();
        let mut encoders = HashMap::new();

        // Initialize for common signals
        for signal_type in [
            SignalType::HeartRate,
            SignalType::SpO2,
            SignalType::Temperature,
            SignalType::SkinConductance,
        ] {
            baselines.insert(
                signal_type.clone(),
                PersonalBaseline::new(signal_type.clone()),
            );

            let (target, tolerance) = match signal_type {
                SignalType::HeartRate => (70.0, 15.0),
                SignalType::SpO2 => (98.0, 3.0),
                SignalType::Temperature => (36.5, 0.5),
                SignalType::SkinConductance => (5.0, 2.0),
                _ => (0.0, 1.0),
            };
            homeostasis.insert(
                signal_type.clone(),
                HomeostaticController::new(target, tolerance),
            );

            let threshold = match signal_type {
                SignalType::HeartRate => 3.0,
                SignalType::SpO2 => 1.0,
                SignalType::Temperature => 0.1,
                _ => 0.5,
            };
            encoders.insert(signal_type, SparseEncoder::new(threshold));
        }

        Self {
            baselines,
            homeostasis,
            encoders,
            alert_history: Vec::new(),
            samples_processed: 0,
        }
    }

    /// Process a biosignal through the nervous system
    pub fn process(&mut self, signal: BioSignal) -> Option<HealthAlert> {
        self.samples_processed += 1;
        let hour = ((signal.timestamp_ms / 3_600_000) % 24) as usize;

        // 1. Sparse encoding (low power)
        let encoder = self.encoders.get_mut(&signal.signal_type);
        let significant = encoder.map(|e| e.encode(signal.value)).flatten();

        if significant.is_none() {
            // No significant change - save power
            return None;
        }

        // 2. One-shot learning to update personal baseline
        if let Some(baseline) = self.baselines.get_mut(&signal.signal_type) {
            baseline.learn_one_shot(signal.value, hour);

            // 3. Check for personal anomaly
            if let Some(z_score) = baseline.is_anomalous(signal.value, hour) {
                let alert = HealthAlert {
                    signal: signal.clone(),
                    alert_type: AlertType::PersonalAnomaly {
                        baseline: baseline.mean,
                        deviation: z_score,
                    },
                    severity: if z_score > 4.0 {
                        AlertSeverity::Urgent
                    } else {
                        AlertSeverity::Warning
                    },
                    recommendation: format!(
                        "{:?} is {:.1} std devs from your personal baseline",
                        signal.signal_type, z_score
                    ),
                    confidence: 0.7 + 0.3 * (baseline.samples_seen as f32 / 1000.0).min(1.0),
                };
                self.alert_history.push(alert.clone());
                return Some(alert);
            }
        }

        // 4. Homeostatic check
        if let Some(controller) = self.homeostasis.get_mut(&signal.signal_type) {
            match controller.respond(signal.value) {
                HomeostasisResponse::Urgent(response) => {
                    let alert = HealthAlert {
                        signal: signal.clone(),
                        alert_type: AlertType::Acute {
                            condition: format!("{:?} critical", signal.signal_type),
                        },
                        severity: AlertSeverity::Emergency,
                        recommendation: format!(
                            "Immediate attention: response magnitude {:.1}",
                            response
                        ),
                        confidence: 0.9,
                    };
                    self.alert_history.push(alert.clone());
                    return Some(alert);
                }
                HomeostasisResponse::Adjust(response) => {
                    let alert = HealthAlert {
                        signal: signal.clone(),
                        alert_type: AlertType::Wellness {
                            category: "homeostasis".to_string(),
                        },
                        severity: AlertSeverity::Info,
                        recommendation: format!(
                            "Consider adjustment: {:?} trending {}",
                            signal.signal_type,
                            if response > 0.0 { "high" } else { "low" }
                        ),
                        confidence: 0.6,
                    };
                    return Some(alert);
                }
                _ => {}
            }
        }

        None
    }

    /// Get power savings from sparse encoding
    pub fn power_efficiency(&self) -> HashMap<SignalType, f32> {
        self.encoders
            .iter()
            .map(|(st, enc)| (st.clone(), enc.compression_ratio(self.samples_processed)))
            .collect()
    }

    /// Get personalization status
    pub fn personalization_status(&self) -> HashMap<SignalType, String> {
        self.baselines
            .iter()
            .map(|(st, bl)| {
                let status = if bl.samples_seen < 10 {
                    "Initializing"
                } else if bl.samples_seen < 100 {
                    "Learning"
                } else if bl.samples_seen < 1000 {
                    "Adapting"
                } else {
                    "Personalized"
                };
                (
                    st.clone(),
                    format!("{} ({} samples)", status, bl.samples_seen),
                )
            })
            .collect()
    }
}

fn main() {
    println!("=== Tier 1: Medical and Wearable Systems ===\n");

    let mut system = MedicalWearableSystem::new();

    // Simulate a day of normal readings (personalization phase)
    println!("Personalization phase (simulating 24 hours)...");
    for hour in 0..24 {
        for minute in 0..60 {
            let timestamp = (hour * 3600 + minute * 60) * 1000;

            // Heart rate varies by time of day
            let base_hr = 60.0 + 10.0 * (hour as f32 / 24.0 * std::f32::consts::PI).sin();
            let hr_noise = (minute as f32 * 0.1).sin() * 5.0;

            let signal = BioSignal {
                timestamp_ms: timestamp,
                signal_type: SignalType::HeartRate,
                value: base_hr + hr_noise,
                source: SignalSource::Wrist,
            };

            let _ = system.process(signal);
        }
    }

    let status = system.personalization_status();
    println!("  Personalization status:");
    for (signal, s) in &status {
        println!("    {:?}: {}", signal, s);
    }

    let efficiency = system.power_efficiency();
    println!("\n  Power efficiency (compression ratio):");
    for (signal, ratio) in &efficiency {
        println!("    {:?}: {:.1}x reduction", signal, ratio);
    }

    // Simulate anomaly detection
    println!("\nAnomaly detection phase...");

    // Normal reading - should not alert
    let normal = BioSignal {
        timestamp_ms: 86_400_000 + 3600_000 * 10, // 10am next day
        signal_type: SignalType::HeartRate,
        value: 72.0,
        source: SignalSource::Wrist,
    };
    if let Some(alert) = system.process(normal) {
        println!("  Unexpected alert: {:?}", alert);
    } else {
        println!("  Normal reading - no alert (as expected)");
    }

    // Anomalous reading - should alert
    let anomaly = BioSignal {
        timestamp_ms: 86_400_000 + 3600_000 * 10 + 1000,
        signal_type: SignalType::HeartRate,
        value: 120.0, // Much higher than personal baseline
        source: SignalSource::Wrist,
    };
    if let Some(alert) = system.process(anomaly) {
        println!("\n  PERSONAL ANOMALY DETECTED!");
        println!("    Type: {:?}", alert.alert_type);
        println!("    Severity: {:?}", alert.severity);
        println!("    Recommendation: {}", alert.recommendation);
        println!("    Confidence: {:.1}%", alert.confidence * 100.0);
    }

    // Emergency - low SpO2
    println!("\nEmergency scenario...");
    let emergency = BioSignal {
        timestamp_ms: 86_400_000 + 3600_000 * 10 + 2000,
        signal_type: SignalType::SpO2,
        value: 88.0, // Dangerously low
        source: SignalSource::Finger,
    };
    if let Some(alert) = system.process(emergency) {
        println!("  EMERGENCY ALERT!");
        println!("    Type: {:?}", alert.alert_type);
        println!("    Severity: {:?}", alert.severity);
        println!("    Recommendation: {}", alert.recommendation);
    }

    println!("\n=== Key Benefits ===");
    println!("- Adapts to the person, not population averages");
    println!("- Low power through sparse spike encoding");
    println!("- Privacy by default (all processing local)");
    println!("- Early detection through personal baselines");
    println!("- Circadian-aware anomaly detection");
    println!("\nThis is practical and defensible.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_one_shot_learning() {
        let mut baseline = PersonalBaseline::new(SignalType::HeartRate);

        // Fast initial learning
        for _ in 0..10 {
            baseline.learn_one_shot(70.0, 12);
        }

        assert!((baseline.mean - 70.0).abs() < 5.0);
    }

    #[test]
    fn test_sparse_encoding() {
        let mut encoder = SparseEncoder::new(5.0);

        // Small changes should not generate spikes
        assert!(encoder.encode(0.0).is_some()); // First value always spikes
        assert!(encoder.encode(2.0).is_none()); // Below threshold
        assert!(encoder.encode(10.0).is_some()); // Above threshold
    }

    #[test]
    fn test_homeostasis() {
        let mut controller = HomeostaticController::new(98.0, 3.0);

        // Within tolerance
        assert!(matches!(
            controller.respond(97.0),
            HomeostasisResponse::Stable
        ));

        // Outside tolerance
        assert!(matches!(
            controller.respond(85.0),
            HomeostasisResponse::Urgent(_)
        ));
    }

    #[test]
    fn test_personal_anomaly_detection() {
        let mut baseline = PersonalBaseline::new(SignalType::HeartRate);

        // Train baseline
        for i in 0..200 {
            baseline.learn_one_shot(70.0 + (i % 10) as f32 * 0.5, 12);
        }

        // Normal should not be anomalous
        assert!(baseline.is_anomalous(72.0, 12).is_none());

        // Extreme value should be anomalous
        assert!(baseline.is_anomalous(150.0, 12).is_some());
    }
}
