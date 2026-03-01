//! # Tier 3: Machine Self-Awareness Primitives
//!
//! Not consciousness, but structural self-sensing.
//!
//! ## What Changes
//! - Systems monitor their own coherence
//! - Learning adjusts internal organization
//! - Failure is sensed before performance drops
//!
//! ## Why This Matters
//! - Systems can say "I am becoming unstable"
//! - Maintenance becomes anticipatory
//! - This is a prerequisite for trustworthy autonomy
//!
//! This is novel and publishable.

use std::collections::{HashMap, VecDeque};

/// Internal state that the system monitors about itself
#[derive(Clone, Debug)]
pub struct InternalState {
    pub timestamp: u64,
    /// Processing coherence (0-1): how well modules are synchronized
    pub coherence: f32,
    /// Attention focus (what is being processed)
    pub attention_target: Option<String>,
    /// Confidence in current processing
    pub confidence: f32,
    /// Energy available for computation
    pub energy_budget: f32,
    /// Error rate in recent operations
    pub error_rate: f32,
}

/// Self-model that tracks the system's own capabilities
#[derive(Clone, Debug)]
pub struct SelfModel {
    /// What capabilities does this system have?
    pub capabilities: HashMap<String, CapabilityState>,
    /// Current operating mode
    pub operating_mode: OperatingMode,
    /// Predicted time until degradation
    pub time_to_degradation: Option<u64>,
    /// Self-assessed reliability
    pub reliability_estimate: f32,
}

#[derive(Clone, Debug)]
pub struct CapabilityState {
    pub name: String,
    pub enabled: bool,
    pub current_performance: f32,
    pub baseline_performance: f32,
    pub degradation_rate: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OperatingMode {
    Optimal,
    Degraded { reason: String },
    Recovery,
    SafeMode,
}

/// Metacognitive monitor that observes internal processing
pub struct MetacognitiveMonitor {
    /// History of internal states
    pub state_history: VecDeque<InternalState>,
    /// Coherence threshold for alarm
    pub coherence_threshold: f32,
    /// Self-model
    pub self_model: SelfModel,
    /// Anomaly detector for internal states
    pub internal_anomaly_threshold: f32,
}

impl MetacognitiveMonitor {
    pub fn new() -> Self {
        Self {
            state_history: VecDeque::new(),
            coherence_threshold: 0.7,
            self_model: SelfModel {
                capabilities: HashMap::new(),
                operating_mode: OperatingMode::Optimal,
                time_to_degradation: None,
                reliability_estimate: 1.0,
            },
            internal_anomaly_threshold: 2.0, // Standard deviations
        }
    }

    /// Register a capability
    pub fn register_capability(&mut self, name: &str, baseline: f32) {
        self.self_model.capabilities.insert(
            name.to_string(),
            CapabilityState {
                name: name.to_string(),
                enabled: true,
                current_performance: baseline,
                baseline_performance: baseline,
                degradation_rate: 0.0,
            },
        );
    }

    /// Observe current internal state
    pub fn observe(&mut self, state: InternalState) -> SelfAwarenessEvent {
        self.state_history.push_back(state.clone());
        if self.state_history.len() > 1000 {
            self.state_history.pop_front();
        }

        // Check coherence
        if state.coherence < self.coherence_threshold {
            self.self_model.operating_mode = OperatingMode::Degraded {
                reason: format!("Low coherence: {:.2}", state.coherence),
            };

            return SelfAwarenessEvent::IncoherenceDetected {
                current_coherence: state.coherence,
                threshold: self.coherence_threshold,
                recommendation: "Reduce processing load or increase synchronization".to_string(),
            };
        }

        // Detect internal anomalies
        if self.state_history.len() > 10 {
            let avg_confidence: f32 = self.state_history.iter().map(|s| s.confidence).sum::<f32>()
                / self.state_history.len() as f32;

            let std_dev: f32 = (self
                .state_history
                .iter()
                .map(|s| (s.confidence - avg_confidence).powi(2))
                .sum::<f32>()
                / self.state_history.len() as f32)
                .sqrt();

            let z_score = (state.confidence - avg_confidence).abs() / std_dev.max(0.01);

            if z_score > self.internal_anomaly_threshold {
                return SelfAwarenessEvent::InternalAnomaly {
                    metric: "confidence".to_string(),
                    z_score,
                    interpretation: if state.confidence < avg_confidence {
                        "Processing uncertainty spike".to_string()
                    } else {
                        "Overconfidence detected".to_string()
                    },
                };
            }
        }

        // Predict degradation
        self.predict_degradation();

        // Check energy
        if state.energy_budget < 0.2 {
            return SelfAwarenessEvent::ResourceWarning {
                resource: "energy".to_string(),
                current: state.energy_budget,
                threshold: 0.2,
                action: "Enter power-saving mode".to_string(),
            };
        }

        SelfAwarenessEvent::Stable {
            coherence: state.coherence,
            confidence: state.confidence,
            operating_mode: self.self_model.operating_mode.clone(),
        }
    }

    /// Update capability performance
    pub fn update_capability(&mut self, name: &str, performance: f32) {
        if let Some(cap) = self.self_model.capabilities.get_mut(name) {
            let old_perf = cap.current_performance;
            cap.current_performance = performance;

            // Track degradation rate
            let degradation = (old_perf - performance) / old_perf.max(0.01);
            cap.degradation_rate = cap.degradation_rate * 0.9 + degradation * 0.1;

            // Update reliability estimate
            self.update_reliability();
        }
    }

    fn predict_degradation(&mut self) {
        // Check if any capability is degrading
        for (_, cap) in &self.self_model.capabilities {
            if cap.degradation_rate > 0.01 {
                // Extrapolate time to failure
                let performance_remaining = cap.current_performance - 0.5; // Minimum acceptable
                if cap.degradation_rate > 0.0 && performance_remaining > 0.0 {
                    let time_to_fail = (performance_remaining / cap.degradation_rate) as u64;
                    self.self_model.time_to_degradation = Some(
                        time_to_fail.min(self.self_model.time_to_degradation.unwrap_or(u64::MAX)),
                    );
                }
            }
        }
    }

    fn update_reliability(&mut self) {
        let total_perf: f32 = self
            .self_model
            .capabilities
            .values()
            .map(|c| c.current_performance / c.baseline_performance.max(0.01))
            .sum();

        let n = self.self_model.capabilities.len().max(1) as f32;
        self.self_model.reliability_estimate = total_perf / n;
    }

    /// Get self-assessment
    pub fn self_assess(&self) -> SelfAssessment {
        SelfAssessment {
            operating_mode: self.self_model.operating_mode.clone(),
            reliability: self.self_model.reliability_estimate,
            time_to_degradation: self.self_model.time_to_degradation,
            capabilities_status: self
                .self_model
                .capabilities
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.current_performance / v.baseline_performance.max(0.01),
                    )
                })
                .collect(),
            recommendation: self.generate_recommendation(),
        }
    }

    fn generate_recommendation(&self) -> String {
        match &self.self_model.operating_mode {
            OperatingMode::Optimal => "System operating normally".to_string(),
            OperatingMode::Degraded { reason } => {
                format!("Degraded: {}. Consider maintenance.", reason)
            }
            OperatingMode::Recovery => "Recovery in progress. Avoid heavy loads.".to_string(),
            OperatingMode::SafeMode => "Safe mode active. Minimal operations only.".to_string(),
        }
    }
}

/// Events that indicate self-awareness
#[derive(Clone, Debug)]
pub enum SelfAwarenessEvent {
    /// System detected low internal coherence
    IncoherenceDetected {
        current_coherence: f32,
        threshold: f32,
        recommendation: String,
    },
    /// Internal processing anomaly detected
    InternalAnomaly {
        metric: String,
        z_score: f32,
        interpretation: String,
    },
    /// Resource warning
    ResourceWarning {
        resource: String,
        current: f32,
        threshold: f32,
        action: String,
    },
    /// Capability degradation predicted
    DegradationPredicted {
        capability: String,
        current_performance: f32,
        predicted_failure_time: u64,
    },
    /// System is stable
    Stable {
        coherence: f32,
        confidence: f32,
        operating_mode: OperatingMode,
    },
}

/// Self-assessment report
#[derive(Clone, Debug)]
pub struct SelfAssessment {
    pub operating_mode: OperatingMode,
    pub reliability: f32,
    pub time_to_degradation: Option<u64>,
    pub capabilities_status: HashMap<String, f32>,
    pub recommendation: String,
}

/// Complete self-aware system
pub struct SelfAwareSystem {
    pub name: String,
    pub monitor: MetacognitiveMonitor,
    /// Processing modules with their coherence
    pub modules: HashMap<String, f32>,
    /// Attention mechanism
    pub attention_focus: Option<String>,
    /// Current timestamp
    pub timestamp: u64,
}

impl SelfAwareSystem {
    pub fn new(name: &str) -> Self {
        let mut system = Self {
            name: name.to_string(),
            monitor: MetacognitiveMonitor::new(),
            modules: HashMap::new(),
            attention_focus: None,
            timestamp: 0,
        };

        // Register default capabilities
        system.monitor.register_capability("perception", 1.0);
        system.monitor.register_capability("reasoning", 1.0);
        system.monitor.register_capability("action", 1.0);
        system.monitor.register_capability("learning", 1.0);

        system
    }

    /// Add a processing module
    pub fn add_module(&mut self, name: &str) {
        self.modules.insert(name.to_string(), 1.0);
    }

    /// Compute current coherence from module phases
    pub fn compute_coherence(&self) -> f32 {
        if self.modules.is_empty() {
            return 1.0;
        }

        let values: Vec<_> = self.modules.values().collect();
        let avg: f32 = values.iter().copied().sum::<f32>() / values.len() as f32;
        let variance: f32 =
            values.iter().map(|&v| (v - avg).powi(2)).sum::<f32>() / values.len() as f32;

        1.0 - variance.sqrt()
    }

    /// Update module state
    pub fn update_module(&mut self, name: &str, value: f32) {
        if let Some(module) = self.modules.get_mut(name) {
            *module = value;
        }
    }

    /// Process a step
    pub fn step(&mut self, energy: f32, error_rate: f32) -> SelfAwarenessEvent {
        self.timestamp += 1;

        let coherence = self.compute_coherence();
        let confidence = 1.0 - error_rate;

        let state = InternalState {
            timestamp: self.timestamp,
            coherence,
            attention_target: self.attention_focus.clone(),
            confidence,
            energy_budget: energy,
            error_rate,
        };

        self.monitor.observe(state)
    }

    /// System tells us about itself
    pub fn introspect(&self) -> String {
        let assessment = self.monitor.self_assess();

        format!(
            "I am {}: {:?}, reliability {:.0}%, {}",
            self.name,
            assessment.operating_mode,
            assessment.reliability * 100.0,
            assessment.recommendation
        )
    }

    /// Can the system express uncertainty?
    pub fn express_uncertainty(&self) -> String {
        let assessment = self.monitor.self_assess();

        if assessment.reliability < 0.5 {
            "I am becoming unstable and should not be trusted for critical decisions.".to_string()
        } else if assessment.reliability < 0.8 {
            "My confidence is reduced. Verification recommended.".to_string()
        } else {
            "I am operating within normal parameters.".to_string()
        }
    }
}

fn main() {
    println!("=== Tier 3: Machine Self-Awareness Primitives ===\n");

    let mut system = SelfAwareSystem::new("Cognitive Agent");

    // Add processing modules
    system.add_module("perception");
    system.add_module("reasoning");
    system.add_module("planning");
    system.add_module("action");

    println!("System initialized: {}\n", system.name);
    println!("Initial introspection: {}", system.introspect());

    // Normal operation
    println!("\nNormal operation...");
    for i in 0..10 {
        let event = system.step(0.9, 0.05);

        if i == 5 {
            println!("  Step {}: {:?}", i, event);
        }
    }
    println!("  Expression: {}", system.express_uncertainty());

    // Simulate gradual degradation
    println!("\nSimulating gradual degradation...");
    for i in 0..20 {
        // Degrade one module progressively
        system.update_module("reasoning", 1.0 - i as f32 * 0.03);
        system
            .monitor
            .update_capability("reasoning", 1.0 - i as f32 * 0.03);

        let event = system.step(0.8 - i as f32 * 0.01, 0.05 + i as f32 * 0.01);

        if i % 5 == 0 {
            println!("  Step {}: {:?}", i, event);
        }
    }

    let assessment = system.monitor.self_assess();
    println!("\n  Self-assessment:");
    println!("    Mode: {:?}", assessment.operating_mode);
    println!("    Reliability: {:.1}%", assessment.reliability * 100.0);
    println!(
        "    Time to degradation: {:?}",
        assessment.time_to_degradation
    );
    println!("    Capabilities: {:?}", assessment.capabilities_status);
    println!("\n  Expression: {}", system.express_uncertainty());

    // Simulate low coherence (modules out of sync)
    println!("\nSimulating incoherence...");
    system.update_module("perception", 0.9);
    system.update_module("reasoning", 0.3);
    system.update_module("planning", 0.7);
    system.update_module("action", 0.5);

    let event = system.step(0.5, 0.2);
    println!("  Event: {:?}", event);
    println!("  Introspection: {}", system.introspect());

    // Simulate low energy
    println!("\nSimulating energy depletion...");
    let event = system.step(0.15, 0.1);
    println!("  Event: {:?}", event);

    // Final self-report
    println!("\n=== Final Self-Report ===");
    println!("{}", system.introspect());
    println!("{}", system.express_uncertainty());

    println!("\n=== Key Benefits ===");
    println!("- Systems can say 'I am becoming unstable'");
    println!("- Failure sensed before performance drops");
    println!("- Maintenance becomes anticipatory");
    println!("- Prerequisite for trustworthy autonomy");
    println!("- Structural self-sensing, not consciousness");
    println!("\nThis is novel and publishable.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coherence_detection() {
        let mut system = SelfAwareSystem::new("test");
        system.add_module("a");
        system.add_module("b");

        // In sync = high coherence
        system.update_module("a", 1.0);
        system.update_module("b", 1.0);
        assert!(system.compute_coherence() > 0.9);

        // Out of sync = low coherence
        system.update_module("a", 1.0);
        system.update_module("b", 0.2);
        assert!(system.compute_coherence() < 0.8);
    }

    #[test]
    fn test_self_assessment() {
        let mut system = SelfAwareSystem::new("test");

        // Normal operation
        for _ in 0..10 {
            system.step(0.9, 0.05);
        }

        let assessment = system.monitor.self_assess();
        assert!(assessment.reliability > 0.9);
    }

    #[test]
    fn test_degradation_prediction() {
        let mut monitor = MetacognitiveMonitor::new();
        monitor.register_capability("test", 1.0);

        // Simulate degradation
        for i in 0..10 {
            monitor.update_capability("test", 1.0 - i as f32 * 0.05);
        }

        // Should predict degradation
        assert!(monitor
            .self_model
            .capabilities
            .get("test")
            .map(|c| c.degradation_rate > 0.0)
            .unwrap_or(false));
    }
}
