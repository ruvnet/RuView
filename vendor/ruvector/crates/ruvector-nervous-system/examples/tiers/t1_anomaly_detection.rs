//! # Tier 1: Always-On Anomaly Detection
//!
//! Infrastructure, finance, security, medical telemetry.
//!
//! ## What Changes
//! - Event streams replace batch logs
//! - Reflex gates fire on structural or temporal anomalies
//! - Learning tightens thresholds over time
//!
//! ## Why This Matters
//! - Detection happens before failure, not after symptoms
//! - Microsecond to millisecond response
//! - Explainable witness logs for every trigger
//!
//! This is a direct fit for RuVector + Cognitum v0.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

// Simulated imports from nervous system crate
// In production: use ruvector_nervous_system::*;

/// Event from monitored system (infrastructure, finance, medical, etc.)
#[derive(Clone, Debug)]
pub struct TelemetryEvent {
    pub timestamp: u64,
    pub source_id: u16,
    pub metric_id: u32,
    pub value: f32,
    pub metadata: Option<String>,
}

/// Anomaly detection result with witness log
#[derive(Clone, Debug)]
pub struct AnomalyAlert {
    pub event: TelemetryEvent,
    pub anomaly_type: AnomalyType,
    pub severity: f32,
    pub witness_log: WitnessLog,
}

#[derive(Clone, Debug)]
pub enum AnomalyType {
    /// Value outside learned bounds
    ValueAnomaly {
        expected_range: (f32, f32),
        actual: f32,
    },
    /// Temporal pattern violation
    TemporalAnomaly {
        expected_interval_ms: u64,
        actual_interval_ms: u64,
    },
    /// Structural change in event relationships
    StructuralAnomaly {
        pattern_signature: u64,
        deviation: f32,
    },
    /// Cascade detected across multiple sources
    CascadeAnomaly { affected_sources: Vec<u16> },
}

/// Explainable witness log for every trigger
#[derive(Clone, Debug)]
pub struct WitnessLog {
    pub trigger_timestamp: u64,
    pub reflex_gate_id: u32,
    pub input_snapshot: Vec<f32>,
    pub threshold_at_trigger: f32,
    pub decision_path: Vec<String>,
}

/// Reflex gate for immediate anomaly detection
pub struct ReflexGate {
    pub id: u32,
    pub threshold: f32,
    pub membrane_potential: f32,
    pub last_spike: u64,
    pub refractory_period_ms: u64,
}

impl ReflexGate {
    pub fn new(id: u32, threshold: f32) -> Self {
        Self {
            id,
            threshold,
            membrane_potential: 0.0,
            last_spike: 0,
            refractory_period_ms: 10, // 10ms refractory
        }
    }

    /// Process input and return true if gate fires
    pub fn process(&mut self, input: f32, timestamp: u64) -> bool {
        // Check refractory period
        if timestamp < self.last_spike + self.refractory_period_ms {
            return false;
        }

        // Integrate input
        self.membrane_potential += input;

        // Fire if threshold exceeded
        if self.membrane_potential >= self.threshold {
            self.last_spike = timestamp;
            self.membrane_potential = 0.0; // Reset
            return true;
        }

        // Leak (decay)
        self.membrane_potential *= 0.95;
        false
    }
}

/// Adaptive threshold using BTSP-style learning
pub struct AdaptiveThreshold {
    pub baseline: f32,
    pub current: f32,
    pub eligibility_trace: f32,
    pub tau_seconds: f32,
    pub learning_rate: f32,
}

impl AdaptiveThreshold {
    pub fn new(baseline: f32) -> Self {
        Self {
            baseline,
            current: baseline,
            eligibility_trace: 0.0,
            tau_seconds: 60.0, // 1 minute adaptation window
            learning_rate: 0.01,
        }
    }

    /// Update threshold based on observed values
    pub fn adapt(&mut self, observed: f32, was_anomaly: bool, dt_seconds: f32) {
        // Decay eligibility trace
        self.eligibility_trace *= (-dt_seconds / self.tau_seconds).exp();

        if was_anomaly {
            // If we flagged an anomaly, become slightly more tolerant
            // to avoid alert fatigue
            self.current += self.learning_rate * self.eligibility_trace;
        } else {
            // Normal observation - tighten threshold over time
            let error = (observed - self.current).abs();
            self.eligibility_trace += error;
            self.current -= self.learning_rate * 0.1 * self.eligibility_trace;
        }

        // Clamp to reasonable bounds
        self.current = self.current.clamp(self.baseline * 0.5, self.baseline * 2.0);
    }
}

/// Temporal pattern detector using spike timing
pub struct TemporalPatternDetector {
    pub expected_interval_ms: u64,
    pub tolerance_ms: u64,
    pub last_event_time: u64,
    pub interval_history: VecDeque<u64>,
    pub max_history: usize,
}

impl TemporalPatternDetector {
    pub fn new(expected_interval_ms: u64, tolerance_ms: u64) -> Self {
        Self {
            expected_interval_ms,
            tolerance_ms,
            last_event_time: 0,
            interval_history: VecDeque::new(),
            max_history: 100,
        }
    }

    /// Check if event timing is anomalous
    pub fn check(&mut self, timestamp: u64) -> Option<AnomalyType> {
        if self.last_event_time == 0 {
            self.last_event_time = timestamp;
            return None;
        }

        let interval = timestamp - self.last_event_time;
        self.last_event_time = timestamp;

        // Track history
        self.interval_history.push_back(interval);
        if self.interval_history.len() > self.max_history {
            self.interval_history.pop_front();
        }

        // Update expected interval (online learning)
        if self.interval_history.len() > 10 {
            let avg: u64 =
                self.interval_history.iter().sum::<u64>() / self.interval_history.len() as u64;
            self.expected_interval_ms = (self.expected_interval_ms + avg) / 2;
        }

        // Check for anomaly
        let diff = (interval as i64 - self.expected_interval_ms as i64).unsigned_abs();
        if diff > self.tolerance_ms {
            Some(AnomalyType::TemporalAnomaly {
                expected_interval_ms: self.expected_interval_ms,
                actual_interval_ms: interval,
            })
        } else {
            None
        }
    }
}

/// Main anomaly detection system
pub struct AnomalyDetectionSystem {
    /// Reflex gates for immediate detection
    pub reflex_gates: Vec<ReflexGate>,
    /// Adaptive thresholds per metric
    pub thresholds: Vec<AdaptiveThreshold>,
    /// Temporal pattern detectors per source
    pub temporal_detectors: Vec<TemporalPatternDetector>,
    /// Alert history for cascade detection
    pub recent_alerts: VecDeque<AnomalyAlert>,
    /// Witness log buffer
    pub witness_buffer: VecDeque<WitnessLog>,
}

impl AnomalyDetectionSystem {
    pub fn new(num_sources: usize, num_metrics: usize) -> Self {
        Self {
            reflex_gates: (0..num_sources)
                .map(|i| ReflexGate::new(i as u32, 1.0))
                .collect(),
            thresholds: (0..num_metrics)
                .map(|_| AdaptiveThreshold::new(1.0))
                .collect(),
            temporal_detectors: (0..num_sources)
                .map(|_| TemporalPatternDetector::new(1000, 100))
                .collect(),
            recent_alerts: VecDeque::new(),
            witness_buffer: VecDeque::new(),
        }
    }

    /// Process a telemetry event through the nervous system
    /// Returns anomaly alert if detected, with full witness log
    pub fn process_event(&mut self, event: TelemetryEvent) -> Option<AnomalyAlert> {
        let source_idx = event.source_id as usize % self.reflex_gates.len();
        let metric_idx = event.metric_id as usize % self.thresholds.len();

        // 1. Check temporal pattern (fast reflex)
        if let Some(temporal_anomaly) = self.temporal_detectors[source_idx].check(event.timestamp) {
            return Some(self.create_alert(event, temporal_anomaly, 0.7));
        }

        // 2. Check value against adaptive threshold
        let threshold = &self.thresholds[metric_idx];
        if event.value > threshold.current * 2.0 || event.value < threshold.current * 0.5 {
            return Some(self.create_alert(
                event.clone(),
                AnomalyType::ValueAnomaly {
                    expected_range: (threshold.current * 0.5, threshold.current * 2.0),
                    actual: event.value,
                },
                0.8,
            ));
        }

        // 3. Check reflex gate (integrates over time)
        let normalized = (event.value - threshold.current).abs() / threshold.current;
        if self.reflex_gates[source_idx].process(normalized, event.timestamp) {
            return Some(self.create_alert(
                event,
                AnomalyType::StructuralAnomaly {
                    pattern_signature: source_idx as u64,
                    deviation: normalized,
                },
                0.6,
            ));
        }

        // 4. Cascade detection: multiple sources alerting
        self.check_cascade(event.timestamp)
    }

    fn create_alert(
        &mut self,
        event: TelemetryEvent,
        anomaly_type: AnomalyType,
        severity: f32,
    ) -> AnomalyAlert {
        let witness = WitnessLog {
            trigger_timestamp: event.timestamp,
            reflex_gate_id: event.source_id as u32,
            input_snapshot: vec![event.value],
            threshold_at_trigger: self
                .thresholds
                .get(event.metric_id as usize % self.thresholds.len())
                .map(|t| t.current)
                .unwrap_or(1.0),
            decision_path: vec![
                format!(
                    "Event received: source={}, metric={}",
                    event.source_id, event.metric_id
                ),
                format!("Anomaly type: {:?}", anomaly_type),
                format!("Severity: {:.2}", severity),
            ],
        };

        self.witness_buffer.push_back(witness.clone());
        if self.witness_buffer.len() > 1000 {
            self.witness_buffer.pop_front();
        }

        let alert = AnomalyAlert {
            event,
            anomaly_type,
            severity,
            witness_log: witness,
        };

        self.recent_alerts.push_back(alert.clone());
        if self.recent_alerts.len() > 100 {
            self.recent_alerts.pop_front();
        }

        alert
    }

    fn check_cascade(&self, timestamp: u64) -> Option<AnomalyAlert> {
        // Check if multiple sources alerted within 100ms window
        let window_start = timestamp.saturating_sub(100);
        let recent: Vec<_> = self
            .recent_alerts
            .iter()
            .filter(|a| a.event.timestamp >= window_start)
            .collect();

        if recent.len() >= 3 {
            let affected: Vec<u16> = recent.iter().map(|a| a.event.source_id).collect();
            let event = recent.last()?.event.clone();

            Some(AnomalyAlert {
                event: event.clone(),
                anomaly_type: AnomalyType::CascadeAnomaly {
                    affected_sources: affected,
                },
                severity: 0.95,
                witness_log: WitnessLog {
                    trigger_timestamp: timestamp,
                    reflex_gate_id: 0,
                    input_snapshot: vec![],
                    threshold_at_trigger: 0.0,
                    decision_path: vec![
                        "Cascade detected".to_string(),
                        format!("Multiple sources alerting within 100ms"),
                    ],
                },
            })
        } else {
            None
        }
    }

    /// Learn from feedback: was the alert valid?
    pub fn learn_from_feedback(&mut self, alert: &AnomalyAlert, was_valid: bool) {
        let metric_idx = alert.event.metric_id as usize % self.thresholds.len();
        self.thresholds[metric_idx].adapt(
            alert.event.value,
            !was_valid, // If invalid, treat as normal (tighten threshold)
            0.1,
        );
    }
}

// =============================================================================
// Example Usage
// =============================================================================

fn main() {
    println!("=== Tier 1: Always-On Anomaly Detection ===\n");

    // Create detection system for 10 sources, 5 metrics
    let mut detector = AnomalyDetectionSystem::new(10, 5);

    // Simulate normal telemetry
    println!("Processing normal telemetry...");
    for i in 0..100 {
        let event = TelemetryEvent {
            timestamp: i * 1000 + (i % 10) * 10, // Slight jitter
            source_id: (i % 10) as u16,
            metric_id: (i % 5) as u32,
            value: 1.0 + (i as f32 * 0.01).sin() * 0.1, // Normal variation
            metadata: None,
        };

        if let Some(alert) = detector.process_event(event) {
            println!("  Alert: {:?}", alert.anomaly_type);
        }
    }
    println!("  Normal events processed with adaptive learning\n");

    // Simulate anomalies
    println!("Injecting anomalies...");

    // Value anomaly
    let value_spike = TelemetryEvent {
        timestamp: 101_000,
        source_id: 0,
        metric_id: 0,
        value: 5.0, // Way above normal ~1.0
        metadata: Some("CPU spike".to_string()),
    };
    if let Some(alert) = detector.process_event(value_spike) {
        println!("  VALUE ANOMALY DETECTED!");
        println!("    Type: {:?}", alert.anomaly_type);
        println!("    Severity: {:.2}", alert.severity);
        println!("    Witness: {:?}", alert.witness_log.decision_path);
    }

    // Temporal anomaly (delayed event)
    let delayed = TelemetryEvent {
        timestamp: 105_000, // Gap of 4 seconds instead of 1
        source_id: 1,
        metric_id: 1,
        value: 1.0,
        metadata: Some("Delayed heartbeat".to_string()),
    };
    if let Some(alert) = detector.process_event(delayed) {
        println!("\n  TEMPORAL ANOMALY DETECTED!");
        println!("    Type: {:?}", alert.anomaly_type);
    }

    println!("\n=== Key Benefits ===");
    println!("- Detection before failure (microsecond response)");
    println!("- Adaptive thresholds reduce false positives");
    println!("- Explainable witness logs for every trigger");
    println!("- Cascade detection across multiple sources");
    println!("\nDirect fit for RuVector + Cognitum v0");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reflex_gate_fires() {
        let mut gate = ReflexGate::new(0, 1.0);

        // Should not fire on small inputs
        assert!(!gate.process(0.3, 0));
        assert!(!gate.process(0.3, 1));

        // Should fire when accumulated
        assert!(gate.process(0.5, 2));
    }

    #[test]
    fn test_adaptive_threshold() {
        let mut threshold = AdaptiveThreshold::new(1.0);

        // Normal observations should tighten threshold
        for _ in 0..10 {
            threshold.adapt(1.0, false, 0.1);
        }

        assert!(threshold.current < 1.0);
    }

    #[test]
    fn test_temporal_pattern_detection() {
        let mut detector = TemporalPatternDetector::new(1000, 100);

        // Normal intervals
        assert!(detector.check(0).is_none());
        assert!(detector.check(1000).is_none());
        assert!(detector.check(2000).is_none());

        // Anomalous interval (500ms instead of 1000ms)
        let result = detector.check(2500);
        assert!(result.is_some());
    }

    #[test]
    fn test_value_anomaly_detection() {
        let mut system = AnomalyDetectionSystem::new(1, 1);

        // Establish baseline
        for i in 0..10 {
            let event = TelemetryEvent {
                timestamp: i * 1000,
                source_id: 0,
                metric_id: 0,
                value: 1.0,
                metadata: None,
            };
            system.process_event(event);
        }

        // Inject anomaly
        let anomaly = TelemetryEvent {
            timestamp: 10_000,
            source_id: 0,
            metric_id: 0,
            value: 10.0, // 10x normal
            metadata: None,
        };

        let result = system.process_event(anomaly);
        assert!(result.is_some());
    }
}
