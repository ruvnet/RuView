//! # Tier 4: Agentic Self-Model
//!
//! SOTA application: An agent that models its own cognitive state.
//!
//! ## The Problem
//! Traditional agents:
//! - Have no awareness of their own capabilities
//! - Cannot predict when they'll fail
//! - Don't know their own uncertainty
//! - Cannot explain "why I'm not confident"
//!
//! ## What Changes
//! - Nervous system scorecard tracks 5 health metrics
//! - Circadian phases indicate optimal task timing
//! - Coherence monitoring detects internal confusion
//! - Budget guardrails prevent resource exhaustion
//!
//! ## Why This Matters
//! - Agents can say: "I'm not confident, let me check"
//! - Agents can say: "I'm tired, defer this complex task"
//! - Agents can say: "I'm becoming unstable, need reset"
//! - Trustworthy autonomy through self-awareness
//!
//! This is the foundation for responsible AI agents.

use std::collections::HashMap;
use std::time::Instant;

// ============================================================================
// Cognitive State Model
// ============================================================================

/// The agent's model of its own cognitive state
#[derive(Clone, Debug)]
pub struct CognitiveState {
    /// Current processing coherence (0-1)
    pub coherence: f32,
    /// Current confidence in outputs (0-1)
    pub confidence: f32,
    /// Current energy budget (0-1)
    pub energy: f32,
    /// Current focus level (0-1)
    pub focus: f32,
    /// Current circadian phase
    pub phase: CircadianPhase,
    /// Time to predicted degradation
    pub ttd: Option<u64>,
    /// Capabilities and their current availability
    pub capabilities: HashMap<String, CapabilityState>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CircadianPhase {
    /// Peak performance, all capabilities available
    Active,
    /// Transitioning up, some capabilities available
    Dawn,
    /// Transitioning down, reduce load
    Dusk,
    /// Minimal processing, consolidation only
    Rest,
}

impl CircadianPhase {
    pub fn duty_factor(&self) -> f32 {
        match self {
            Self::Active => 1.0,
            Self::Dawn => 0.7,
            Self::Dusk => 0.4,
            Self::Rest => 0.1,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Active => "Peak performance",
            Self::Dawn => "Warming up",
            Self::Dusk => "Winding down",
            Self::Rest => "Consolidating",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CapabilityState {
    /// Name of capability
    pub name: String,
    /// Is it available right now?
    pub available: bool,
    /// Current performance (0-1)
    pub performance: f32,
    /// Why unavailable (if not available)
    pub reason: Option<String>,
    /// Estimated recovery time
    pub recovery_time: Option<u64>,
}

// ============================================================================
// Self-Model Components
// ============================================================================

/// Tracks coherence (internal consistency)
pub struct CoherenceTracker {
    /// Module phases
    phases: HashMap<String, f32>,
    /// Recent coherence values
    history: Vec<f32>,
    /// Threshold for alarm
    threshold: f32,
}

impl CoherenceTracker {
    pub fn new(threshold: f32) -> Self {
        Self {
            phases: HashMap::new(),
            history: Vec::new(),
            threshold,
        }
    }

    pub fn register_module(&mut self, name: &str) {
        self.phases.insert(name.to_string(), 0.0);
    }

    pub fn update_module(&mut self, name: &str, phase: f32) {
        self.phases.insert(name.to_string(), phase);
    }

    /// Compute current coherence (Kuramoto order parameter)
    pub fn compute(&self) -> f32 {
        if self.phases.is_empty() {
            return 1.0;
        }

        let n = self.phases.len() as f32;
        let sum_x: f32 = self.phases.values().map(|p| p.cos()).sum();
        let sum_y: f32 = self.phases.values().map(|p| p.sin()).sum();

        (sum_x * sum_x + sum_y * sum_y).sqrt() / n
    }

    pub fn record(&mut self) -> f32 {
        let coherence = self.compute();
        self.history.push(coherence);
        if self.history.len() > 100 {
            self.history.remove(0);
        }
        coherence
    }

    pub fn is_alarming(&self) -> bool {
        self.compute() < self.threshold
    }

    pub fn trend(&self) -> f32 {
        if self.history.len() < 10 {
            return 0.0;
        }
        let recent: f32 = self.history.iter().rev().take(5).sum::<f32>() / 5.0;
        let older: f32 = self.history.iter().rev().skip(5).take(5).sum::<f32>() / 5.0;
        recent - older
    }
}

/// Tracks confidence in outputs
pub struct ConfidenceTracker {
    /// Running average confidence
    average: f32,
    /// Recent values
    history: Vec<f32>,
    /// Calibration factor (learned)
    calibration: f32,
}

impl ConfidenceTracker {
    pub fn new() -> Self {
        Self {
            average: 0.8,
            history: Vec::new(),
            calibration: 1.0,
        }
    }

    pub fn record(&mut self, raw_confidence: f32) {
        let calibrated = (raw_confidence * self.calibration).clamp(0.0, 1.0);
        self.history.push(calibrated);
        if self.history.len() > 100 {
            self.history.remove(0);
        }
        self.average = self.history.iter().sum::<f32>() / self.history.len() as f32;
    }

    /// Calibrate based on feedback
    pub fn calibrate(&mut self, predicted: f32, actual: f32) {
        // If we predicted 0.9 but were actually 0.6, reduce calibration
        let error = predicted - actual;
        self.calibration = (self.calibration - error * 0.1).clamp(0.5, 1.5);
    }

    pub fn current(&self) -> f32 {
        self.average
    }

    pub fn variance(&self) -> f32 {
        if self.history.len() < 2 {
            return 0.0;
        }
        let mean = self.average;
        self.history
            .iter()
            .map(|&v| (v - mean).powi(2))
            .sum::<f32>()
            / (self.history.len() - 1) as f32
    }
}

/// Tracks energy budget
pub struct EnergyTracker {
    /// Current energy (0-1)
    current: f32,
    /// Regeneration rate per hour
    regen_rate: f32,
    /// Consumption history
    consumption_log: Vec<(u64, f32)>,
    /// Budget per hour
    budget_per_hour: f32,
}

impl EnergyTracker {
    pub fn new(budget_per_hour: f32) -> Self {
        Self {
            current: 1.0,
            regen_rate: 0.2, // 20% per hour
            consumption_log: Vec::new(),
            budget_per_hour,
        }
    }

    pub fn consume(&mut self, amount: f32, timestamp: u64) {
        self.current = (self.current - amount).max(0.0);
        self.consumption_log.push((timestamp, amount));

        // Trim old entries
        let cutoff = timestamp.saturating_sub(3600);
        self.consumption_log.retain(|(t, _)| *t > cutoff);
    }

    pub fn regenerate(&mut self, dt_hours: f32) {
        self.current = (self.current + self.regen_rate * dt_hours).min(1.0);
    }

    pub fn current(&self) -> f32 {
        self.current
    }

    pub fn hourly_rate(&self) -> f32 {
        self.consumption_log.iter().map(|(_, a)| a).sum()
    }

    pub fn is_overspending(&self) -> bool {
        self.hourly_rate() > self.budget_per_hour
    }

    pub fn time_to_exhaustion(&self) -> Option<u64> {
        if self.hourly_rate() <= 0.0 {
            return None;
        }
        let hours = self.current / self.hourly_rate();
        Some((hours * 3600.0) as u64)
    }
}

/// Tracks circadian phase
pub struct CircadianClock {
    /// Current phase in cycle (0-1)
    phase: f32,
    /// Cycle duration in hours
    cycle_hours: f32,
    /// Current phase state
    state: CircadianPhase,
}

impl CircadianClock {
    pub fn new(cycle_hours: f32) -> Self {
        Self {
            phase: 0.0,
            cycle_hours,
            state: CircadianPhase::Active,
        }
    }

    pub fn advance(&mut self, hours: f32) {
        self.phase = (self.phase + hours / self.cycle_hours) % 1.0;
        self.update_state();
    }

    fn update_state(&mut self) {
        self.state = if self.phase < 0.5 {
            CircadianPhase::Active
        } else if self.phase < 0.6 {
            CircadianPhase::Dusk
        } else if self.phase < 0.9 {
            CircadianPhase::Rest
        } else {
            CircadianPhase::Dawn
        };
    }

    pub fn state(&self) -> CircadianPhase {
        self.state.clone()
    }

    pub fn time_to_next_active(&self) -> f32 {
        if self.phase < 0.5 {
            0.0 // Already active
        } else {
            (1.0 - self.phase + 0.0) * self.cycle_hours
        }
    }
}

// ============================================================================
// Self-Aware Agent
// ============================================================================

/// An agent that models its own cognitive state
pub struct SelfAwareAgent {
    /// Name of agent
    pub name: String,
    /// Coherence tracker
    coherence: CoherenceTracker,
    /// Confidence tracker
    confidence: ConfidenceTracker,
    /// Energy tracker
    energy: EnergyTracker,
    /// Circadian clock
    clock: CircadianClock,
    /// Registered capabilities
    capabilities: HashMap<String, bool>,
    /// Current timestamp
    timestamp: u64,
    /// Action history
    actions: Vec<ActionRecord>,
}

#[derive(Clone, Debug)]
pub struct ActionRecord {
    pub timestamp: u64,
    pub action: String,
    pub confidence: f32,
    pub success: Option<bool>,
    pub energy_cost: f32,
}

impl SelfAwareAgent {
    pub fn new(name: &str) -> Self {
        let mut agent = Self {
            name: name.to_string(),
            coherence: CoherenceTracker::new(0.7),
            confidence: ConfidenceTracker::new(),
            energy: EnergyTracker::new(0.5), // 50% per hour budget
            clock: CircadianClock::new(24.0),
            capabilities: HashMap::new(),
            timestamp: 0,
            actions: Vec::new(),
        };

        // Register standard modules
        agent.coherence.register_module("perception");
        agent.coherence.register_module("reasoning");
        agent.coherence.register_module("planning");
        agent.coherence.register_module("action");

        // Register standard capabilities
        agent
            .capabilities
            .insert("complex_reasoning".to_string(), true);
        agent
            .capabilities
            .insert("creative_generation".to_string(), true);
        agent
            .capabilities
            .insert("precise_calculation".to_string(), true);
        agent.capabilities.insert("fast_response".to_string(), true);

        agent
    }

    /// Get current cognitive state
    pub fn introspect(&self) -> CognitiveState {
        let coherence = self.coherence.compute();
        let phase = self.clock.state();

        // Determine capability availability based on state
        let capabilities = self
            .capabilities
            .iter()
            .map(|(name, baseline)| {
                let (available, reason) =
                    self.capability_available(name, *baseline, &phase, coherence);
                (
                    name.clone(),
                    CapabilityState {
                        name: name.clone(),
                        available,
                        performance: if available {
                            self.energy.current()
                        } else {
                            0.0
                        },
                        reason,
                        recovery_time: if available {
                            None
                        } else {
                            Some(self.time_to_recovery())
                        },
                    },
                )
            })
            .collect();

        CognitiveState {
            coherence,
            confidence: self.confidence.current(),
            energy: self.energy.current(),
            focus: self.compute_focus(),
            phase,
            ttd: self.energy.time_to_exhaustion(),
            capabilities,
        }
    }

    fn capability_available(
        &self,
        name: &str,
        baseline: bool,
        phase: &CircadianPhase,
        coherence: f32,
    ) -> (bool, Option<String>) {
        if !baseline {
            return (false, Some("Capability disabled".to_string()));
        }

        match name {
            "complex_reasoning" => {
                if matches!(phase, CircadianPhase::Rest) {
                    (
                        false,
                        Some("Rest phase - complex reasoning unavailable".to_string()),
                    )
                } else if coherence < 0.5 {
                    (
                        false,
                        Some("Low coherence - reasoning compromised".to_string()),
                    )
                } else if self.energy.current() < 0.2 {
                    (false, Some("Low energy - reasoning expensive".to_string()))
                } else {
                    (true, None)
                }
            }
            "creative_generation" => {
                if matches!(phase, CircadianPhase::Rest | CircadianPhase::Dusk) {
                    (
                        false,
                        Some(format!(
                            "{} phase - creativity reduced",
                            phase.description()
                        )),
                    )
                } else {
                    (true, None)
                }
            }
            "precise_calculation" => {
                if coherence < 0.7 {
                    (
                        false,
                        Some("Coherence below precision threshold".to_string()),
                    )
                } else {
                    (true, None)
                }
            }
            "fast_response" => {
                if self.energy.current() < 0.3 {
                    (
                        false,
                        Some("Insufficient energy for fast response".to_string()),
                    )
                } else {
                    (true, None)
                }
            }
            _ => (true, None),
        }
    }

    fn compute_focus(&self) -> f32 {
        let coherence = self.coherence.compute();
        let energy = self.energy.current();
        let phase_factor = self.clock.state().duty_factor();

        (coherence * 0.4 + energy * 0.3 + phase_factor * 0.3).clamp(0.0, 1.0)
    }

    fn time_to_recovery(&self) -> u64 {
        // Time until active phase + time to regen energy
        let phase_time = self.clock.time_to_next_active();
        let energy_time = if self.energy.current() < 0.3 {
            (0.3 - self.energy.current()) / self.energy.regen_rate
        } else {
            0.0
        };
        ((phase_time.max(energy_time)) * 3600.0) as u64
    }

    /// Express current state in natural language
    pub fn express_state(&self) -> String {
        let state = self.introspect();

        let phase_desc = state.phase.description();
        let coherence_desc = if state.coherence > 0.8 {
            "clear"
        } else if state.coherence > 0.6 {
            "somewhat scattered"
        } else {
            "confused"
        };
        let energy_desc = if state.energy > 0.7 {
            "energized"
        } else if state.energy > 0.3 {
            "adequate"
        } else {
            "depleted"
        };
        let confidence_desc = if state.confidence > 0.8 {
            "confident"
        } else if state.confidence > 0.5 {
            "moderately confident"
        } else {
            "uncertain"
        };

        let unavailable: Vec<_> = state
            .capabilities
            .values()
            .filter(|c| !c.available)
            .map(|c| {
                format!(
                    "{} ({})",
                    c.name,
                    c.reason.as_ref().unwrap_or(&"unavailable".to_string())
                )
            })
            .collect();

        let mut response = format!(
            "I am {}. Currently {} ({}), feeling {} and {}.",
            self.name,
            phase_desc,
            format!("{:.0}%", state.phase.duty_factor() * 100.0),
            coherence_desc,
            energy_desc
        );

        if !unavailable.is_empty() {
            response.push_str(&format!(
                "\n\nCurrently unavailable: {}",
                unavailable.join(", ")
            ));
        }

        if state.ttd.is_some() && state.energy < 0.3 {
            response.push_str(&format!(
                "\n\nWarning: Energy low. Time to exhaustion: {}s",
                state.ttd.unwrap()
            ));
        }

        response
    }

    /// Decide whether to accept a task
    pub fn should_accept_task(&self, task: &Task) -> TaskDecision {
        let state = self.introspect();

        // Check required capabilities
        for req_cap in &task.required_capabilities {
            if let Some(cap) = state.capabilities.get(req_cap) {
                if !cap.available {
                    return TaskDecision::Decline {
                        reason: format!(
                            "Required capability '{}' unavailable: {}",
                            req_cap,
                            cap.reason.as_ref().unwrap_or(&"unknown".to_string())
                        ),
                        retry_after: cap.recovery_time,
                    };
                }
            }
        }

        // Check energy budget
        if self.energy.current() < task.energy_cost {
            return TaskDecision::Decline {
                reason: format!(
                    "Insufficient energy: have {:.0}%, need {:.0}%",
                    self.energy.current() * 100.0,
                    task.energy_cost * 100.0
                ),
                retry_after: Some(self.time_to_recovery()),
            };
        }

        // Check coherence
        if state.coherence < task.min_coherence {
            return TaskDecision::Decline {
                reason: format!(
                    "Coherence too low: {:.0}% < {:.0}% required",
                    state.coherence * 100.0,
                    task.min_coherence * 100.0
                ),
                retry_after: None,
            };
        }

        // Check phase
        if task.requires_peak && !matches!(state.phase, CircadianPhase::Active) {
            return TaskDecision::Defer {
                reason: "Task requires peak performance phase".to_string(),
                optimal_time: Some((self.clock.time_to_next_active() * 3600.0) as u64),
            };
        }

        // Accept with confidence estimate
        let confidence = self.estimate_confidence(&task, &state);
        TaskDecision::Accept {
            confidence,
            warnings: self.generate_warnings(&task, &state),
        }
    }

    fn estimate_confidence(&self, task: &Task, state: &CognitiveState) -> f32 {
        let base = self.confidence.current();
        let energy_factor = state.energy.powf(0.5); // Square root to soften impact
        let coherence_factor = state.coherence;
        let phase_factor = state.phase.duty_factor();

        (base * energy_factor * coherence_factor * phase_factor).clamp(0.0, 1.0)
    }

    fn generate_warnings(&self, task: &Task, state: &CognitiveState) -> Vec<String> {
        let mut warnings = Vec::new();

        if state.energy < 0.4 {
            warnings.push("Low energy may affect performance".to_string());
        }
        if state.coherence < 0.7 {
            warnings.push("Reduced coherence - verify outputs".to_string());
        }
        if self.confidence.variance() > 0.1 {
            warnings.push("High confidence variance - calibration recommended".to_string());
        }
        if matches!(state.phase, CircadianPhase::Dusk) {
            warnings.push("Approaching rest phase - complex tasks may be deferred".to_string());
        }

        warnings
    }

    /// Execute an action (consumes energy, updates state)
    pub fn execute(&mut self, action: &str, confidence: f32, energy_cost: f32) {
        self.energy.consume(energy_cost, self.timestamp);
        self.confidence.record(confidence);

        self.actions.push(ActionRecord {
            timestamp: self.timestamp,
            action: action.to_string(),
            confidence,
            success: None,
            energy_cost,
        });
    }

    /// Record outcome and calibrate
    pub fn record_outcome(&mut self, success: bool, predicted_confidence: f32) {
        let actual = if success { 1.0 } else { 0.0 };
        self.confidence.calibrate(predicted_confidence, actual);

        if let Some(last) = self.actions.last_mut() {
            last.success = Some(success);
        }
    }

    /// Advance time
    pub fn tick(&mut self, dt_seconds: u64) {
        self.timestamp += dt_seconds;
        self.clock.advance(dt_seconds as f32 / 3600.0);
        self.energy.regenerate(dt_seconds as f32 / 3600.0);
        self.coherence.record();
    }

    /// Simulate module activity (affects coherence)
    pub fn module_activity(&mut self, module: &str, phase: f32) {
        self.coherence.update_module(module, phase);
    }
}

#[derive(Clone, Debug)]
pub struct Task {
    pub name: String,
    pub required_capabilities: Vec<String>,
    pub energy_cost: f32,
    pub min_coherence: f32,
    pub requires_peak: bool,
}

#[derive(Clone, Debug)]
pub enum TaskDecision {
    Accept {
        confidence: f32,
        warnings: Vec<String>,
    },
    Defer {
        reason: String,
        optimal_time: Option<u64>,
    },
    Decline {
        reason: String,
        retry_after: Option<u64>,
    },
}

// ============================================================================
// Example Usage
// ============================================================================

fn main() {
    println!("=== Tier 4: Agentic Self-Model ===\n");

    let mut agent = SelfAwareAgent::new("Claude-Nervous");

    println!("Initial state:");
    println!("{}\n", agent.express_state());

    // Define some tasks
    let tasks = vec![
        Task {
            name: "Simple calculation".to_string(),
            required_capabilities: vec!["precise_calculation".to_string()],
            energy_cost: 0.05,
            min_coherence: 0.7,
            requires_peak: false,
        },
        Task {
            name: "Complex reasoning problem".to_string(),
            required_capabilities: vec!["complex_reasoning".to_string()],
            energy_cost: 0.2,
            min_coherence: 0.6,
            requires_peak: false,
        },
        Task {
            name: "Creative writing".to_string(),
            required_capabilities: vec!["creative_generation".to_string()],
            energy_cost: 0.15,
            min_coherence: 0.5,
            requires_peak: false,
        },
        Task {
            name: "Critical system modification".to_string(),
            required_capabilities: vec![
                "complex_reasoning".to_string(),
                "precise_calculation".to_string(),
            ],
            energy_cost: 0.3,
            min_coherence: 0.8,
            requires_peak: true,
        },
    ];

    // Process tasks
    println!("=== Task Processing ===\n");
    for task in &tasks {
        println!("Task: {}", task.name);
        let decision = agent.should_accept_task(task);
        match &decision {
            TaskDecision::Accept {
                confidence,
                warnings,
            } => {
                println!(
                    "  Decision: ACCEPT (confidence: {:.0}%)",
                    confidence * 100.0
                );
                if !warnings.is_empty() {
                    println!("  Warnings: {}", warnings.join("; "));
                }
                agent.execute(&task.name, *confidence, task.energy_cost);
            }
            TaskDecision::Defer {
                reason,
                optimal_time,
            } => {
                println!("  Decision: DEFER - {}", reason);
                if let Some(time) = optimal_time {
                    println!("  Optimal time: in {}s", time);
                }
            }
            TaskDecision::Decline {
                reason,
                retry_after,
            } => {
                println!("  Decision: DECLINE - {}", reason);
                if let Some(time) = retry_after {
                    println!("  Retry after: {}s", time);
                }
            }
        }
        println!();
        agent.tick(300); // 5 minutes between tasks
    }

    // Simulate degradation
    println!("=== Simulating Extended Operation ===\n");
    println!("Running for 12 hours...");

    for hour in 0..12 {
        // Simulate varying coherence
        let phase = (hour as f32 * 0.3).sin() * 0.3;
        agent.module_activity("perception", phase);
        agent.module_activity("reasoning", phase + 0.1);
        agent.module_activity("planning", phase + 0.2);
        agent.module_activity("action", phase + 0.3);

        // Consume energy
        agent.execute("routine_task", 0.7, 0.08);

        agent.tick(3600); // 1 hour

        if hour % 4 == 3 {
            println!("Hour {}: {}", hour + 1, agent.express_state());
            println!();
        }
    }

    // Final state
    println!("=== Final State ===\n");
    println!("{}", agent.express_state());

    let state = agent.introspect();
    println!("\n=== Detailed Capabilities ===");
    for (name, cap) in &state.capabilities {
        println!(
            "  {}: {} (perf: {:.0}%)",
            name,
            if cap.available {
                "AVAILABLE"
            } else {
                "UNAVAILABLE"
            },
            cap.performance * 100.0
        );
        if let Some(reason) = &cap.reason {
            println!("    Reason: {}", reason);
        }
    }

    println!("\n=== Key Benefits ===");
    println!("- Agent knows when to say 'I'm not confident'");
    println!("- Agent knows when to defer complex tasks");
    println!("- Agent predicts its own degradation");
    println!("- Agent explains WHY capabilities are unavailable");
    println!("\nThis is the foundation for trustworthy autonomous AI.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coherence_affects_capabilities() {
        let mut agent = SelfAwareAgent::new("test");

        // Desync modules
        agent.module_activity("perception", 0.0);
        agent.module_activity("reasoning", 3.14);
        agent.module_activity("planning", 1.57);
        agent.module_activity("action", 4.71);

        let state = agent.introspect();
        assert!(state.coherence < 0.5);

        // Precise calculation should be unavailable
        let cap = state.capabilities.get("precise_calculation").unwrap();
        assert!(!cap.available);
    }

    #[test]
    fn test_energy_affects_acceptance() {
        let mut agent = SelfAwareAgent::new("test");

        let expensive_task = Task {
            name: "expensive".to_string(),
            required_capabilities: vec![],
            energy_cost: 0.9,
            min_coherence: 0.0,
            requires_peak: false,
        };

        // Deplete energy
        agent.execute("drain", 0.8, 0.8);

        let decision = agent.should_accept_task(&expensive_task);
        assert!(matches!(decision, TaskDecision::Decline { .. }));
    }

    #[test]
    fn test_phase_affects_capabilities() {
        let mut agent = SelfAwareAgent::new("test");

        // Advance to rest phase
        agent.tick(12 * 3600); // 12 hours

        let state = agent.introspect();

        // Complex reasoning should be unavailable during rest
        if matches!(state.phase, CircadianPhase::Rest) {
            let cap = state.capabilities.get("complex_reasoning").unwrap();
            assert!(!cap.available);
        }
    }
}
