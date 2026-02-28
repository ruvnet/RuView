//! # Tier 2: Self-Optimizing Software and Workflows
//!
//! Agents that monitor agents.
//!
//! ## What Changes
//! - Systems watch structure and timing, not just outputs
//! - Learning adjusts coordination patterns
//! - Reflex gates prevent cascading failures
//!
//! ## Why This Matters
//! - Software becomes self-stabilizing
//! - Less ops, fewer incidents
//! - Debugging shifts from logs to structural witnesses
//!
//! This is a natural extension of RuVector as connective tissue.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// A software component being monitored
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ComponentId(pub String);

/// Structural observation about a component
#[derive(Clone, Debug)]
pub struct StructuralEvent {
    pub timestamp_us: u64,
    pub component: ComponentId,
    pub event_type: StructuralEventType,
    pub latency_us: Option<u64>,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum StructuralEventType {
    /// Request received
    RequestStart { request_id: u64 },
    /// Request completed
    RequestEnd { request_id: u64, success: bool },
    /// Component called another
    Call {
        target: ComponentId,
        request_id: u64,
    },
    /// Component received call result
    CallReturn {
        source: ComponentId,
        request_id: u64,
        success: bool,
    },
    /// Resource usage spike
    ResourceSpike { resource: String, value: f32 },
    /// Queue depth changed
    QueueDepth { depth: usize },
    /// Circuit breaker state change
    CircuitBreaker { state: CircuitState },
}

#[derive(Clone, Debug, PartialEq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Witness log for structural debugging
#[derive(Clone, Debug)]
pub struct StructuralWitness {
    pub timestamp: u64,
    pub trigger: String,
    pub component_states: HashMap<ComponentId, ComponentState>,
    pub causal_chain: Vec<(ComponentId, StructuralEventType)>,
    pub decision: String,
    pub action_taken: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ComponentState {
    pub latency_p99_us: u64,
    pub error_rate: f32,
    pub queue_depth: usize,
    pub circuit_state: CircuitState,
}

/// Coordination pattern learned over time
#[derive(Clone, Debug)]
pub struct CoordinationPattern {
    pub name: String,
    pub participants: Vec<ComponentId>,
    pub expected_sequence: Vec<(ComponentId, ComponentId)>,
    pub expected_latency_us: u64,
    pub tolerance: f32,
    pub occurrences: u64,
}

/// Reflex gate to prevent cascading failures
pub struct CascadeReflex {
    pub trigger_threshold: f32, // Error rate threshold
    pub propagation_window_us: u64,
    pub recent_errors: VecDeque<(u64, ComponentId)>,
    pub circuit_breakers: HashMap<ComponentId, CircuitBreaker>,
}

pub struct CircuitBreaker {
    pub state: CircuitState,
    pub failure_count: u32,
    pub failure_threshold: u32,
    pub reset_timeout_us: u64,
    pub last_failure: u64,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, timeout_us: u64) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            failure_threshold: threshold,
            reset_timeout_us: timeout_us,
            last_failure: 0,
        }
    }

    pub fn record_failure(&mut self, timestamp: u64) {
        self.failure_count += 1;
        self.last_failure = timestamp;

        if self.failure_count >= self.failure_threshold {
            self.state = CircuitState::Open;
        }
    }

    pub fn record_success(&mut self) {
        if self.state == CircuitState::HalfOpen {
            self.state = CircuitState::Closed;
            self.failure_count = 0;
        }
    }

    pub fn check(&mut self, timestamp: u64) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if timestamp - self.last_failure > self.reset_timeout_us {
                    self.state = CircuitState::HalfOpen;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }
}

impl CascadeReflex {
    pub fn new(threshold: f32, window_us: u64) -> Self {
        Self {
            trigger_threshold: threshold,
            propagation_window_us: window_us,
            recent_errors: VecDeque::new(),
            circuit_breakers: HashMap::new(),
        }
    }

    /// Check for cascading failure pattern
    pub fn check(&mut self, event: &StructuralEvent) -> Option<StructuralWitness> {
        // Track errors
        if matches!(&event.event_type, StructuralEventType::RequestEnd { success, .. } if !success)
        {
            self.recent_errors
                .push_back((event.timestamp_us, event.component.clone()));

            // Record in circuit breaker
            self.circuit_breakers
                .entry(event.component.clone())
                .or_insert_with(|| CircuitBreaker::new(5, 30_000_000))
                .record_failure(event.timestamp_us);
        }

        // Clean old errors
        let cutoff = event
            .timestamp_us
            .saturating_sub(self.propagation_window_us);
        while self
            .recent_errors
            .front()
            .map(|e| e.0 < cutoff)
            .unwrap_or(false)
        {
            self.recent_errors.pop_front();
        }

        // Count affected components
        let mut affected: HashMap<ComponentId, u32> = HashMap::new();
        for (_, comp) in &self.recent_errors {
            *affected.entry(comp.clone()).or_default() += 1;
        }

        // Detect cascade (multiple components failing together)
        if affected.len() >= 3 {
            let witness = StructuralWitness {
                timestamp: event.timestamp_us,
                trigger: "Cascade detected".to_string(),
                component_states: affected
                    .keys()
                    .map(|c| {
                        (
                            c.clone(),
                            ComponentState {
                                latency_p99_us: 0,
                                error_rate: *affected.get(c).unwrap_or(&0) as f32 / 10.0,
                                queue_depth: 0,
                                circuit_state: self
                                    .circuit_breakers
                                    .get(c)
                                    .map(|cb| cb.state.clone())
                                    .unwrap_or(CircuitState::Closed),
                            },
                        )
                    })
                    .collect(),
                causal_chain: self
                    .recent_errors
                    .iter()
                    .map(|(_, c)| {
                        (
                            c.clone(),
                            StructuralEventType::RequestEnd {
                                request_id: 0,
                                success: false,
                            },
                        )
                    })
                    .collect(),
                decision: format!("Open circuit breakers for {} components", affected.len()),
                action_taken: Some("SHED_LOAD".to_string()),
            };

            // Open all affected circuit breakers
            for comp in affected.keys() {
                if let Some(cb) = self.circuit_breakers.get_mut(comp) {
                    cb.state = CircuitState::Open;
                }
            }

            return Some(witness);
        }

        None
    }
}

/// Pattern learner that discovers coordination patterns
pub struct PatternLearner {
    pub observed_sequences: HashMap<String, CoordinationPattern>,
    pub current_traces: HashMap<u64, Vec<(u64, ComponentId, ComponentId)>>,
    pub learning_rate: f32,
}

impl PatternLearner {
    pub fn new() -> Self {
        Self {
            observed_sequences: HashMap::new(),
            current_traces: HashMap::new(),
            learning_rate: 0.1,
        }
    }

    /// Observe a call between components
    pub fn observe_call(
        &mut self,
        caller: ComponentId,
        callee: ComponentId,
        request_id: u64,
        timestamp: u64,
    ) {
        self.current_traces
            .entry(request_id)
            .or_default()
            .push((timestamp, caller, callee));
    }

    /// Complete a trace and learn from it
    pub fn complete_trace(&mut self, request_id: u64) -> Option<String> {
        let trace = self.current_traces.remove(&request_id)?;

        if trace.len() < 2 {
            return None;
        }

        // Create pattern signature
        let participants: Vec<ComponentId> = trace
            .iter()
            .flat_map(|(_, from, to)| vec![from.clone(), to.clone()])
            .collect();

        let sequence: Vec<(ComponentId, ComponentId)> = trace
            .iter()
            .map(|(_, from, to)| (from.clone(), to.clone()))
            .collect();

        let total_latency =
            trace.last().map(|l| l.0).unwrap_or(0) - trace.first().map(|f| f.0).unwrap_or(0);

        let signature = format!("{:?}", sequence);

        // Update or create pattern
        let next_pattern_id = self.observed_sequences.len();
        let pattern = self
            .observed_sequences
            .entry(signature.clone())
            .or_insert_with(|| CoordinationPattern {
                name: format!("Pattern_{}", next_pattern_id),
                participants: participants.clone(),
                expected_sequence: sequence.clone(),
                expected_latency_us: total_latency,
                tolerance: 0.5,
                occurrences: 0,
            });

        pattern.occurrences += 1;
        pattern.expected_latency_us =
            ((1.0 - self.learning_rate) * pattern.expected_latency_us as f32
                + self.learning_rate * total_latency as f32) as u64;

        Some(pattern.name.clone())
    }

    /// Check if a trace violates learned patterns
    pub fn check_violation(&self, trace: &[(u64, ComponentId, ComponentId)]) -> Option<String> {
        if trace.len() < 2 {
            return None;
        }

        let sequence: Vec<(ComponentId, ComponentId)> = trace
            .iter()
            .map(|(_, from, to)| (from.clone(), to.clone()))
            .collect();

        let signature = format!("{:?}", sequence);

        if let Some(pattern) = self.observed_sequences.get(&signature) {
            let latency =
                trace.last().map(|l| l.0).unwrap_or(0) - trace.first().map(|f| f.0).unwrap_or(0);

            let deviation = (latency as f32 - pattern.expected_latency_us as f32).abs()
                / pattern.expected_latency_us as f32;

            if deviation > pattern.tolerance {
                return Some(format!(
                    "{} latency deviation: expected {}us, got {}us ({:.0}%)",
                    pattern.name,
                    pattern.expected_latency_us,
                    latency,
                    deviation * 100.0
                ));
            }
        }

        None
    }
}

/// Main self-optimizing system
pub struct SelfOptimizingSystem {
    /// Reflex gate for cascade prevention
    pub cascade_reflex: CascadeReflex,
    /// Pattern learner for coordination
    pub pattern_learner: PatternLearner,
    /// Component latency trackers
    pub latency_trackers: HashMap<ComponentId, VecDeque<u64>>,
    /// Witness log for debugging
    pub witnesses: Vec<StructuralWitness>,
    /// Optimization actions taken
    pub optimizations: Vec<String>,
}

impl SelfOptimizingSystem {
    pub fn new() -> Self {
        Self {
            cascade_reflex: CascadeReflex::new(0.1, 1_000_000),
            pattern_learner: PatternLearner::new(),
            latency_trackers: HashMap::new(),
            witnesses: Vec::new(),
            optimizations: Vec::new(),
        }
    }

    /// Process a structural event
    pub fn observe(&mut self, event: StructuralEvent) -> Option<StructuralWitness> {
        // 1. Check reflex (cascade prevention)
        if let Some(witness) = self.cascade_reflex.check(&event) {
            self.witnesses.push(witness.clone());
            return Some(witness);
        }

        // 2. Track patterns
        match &event.event_type {
            StructuralEventType::Call { target, request_id } => {
                self.pattern_learner.observe_call(
                    event.component.clone(),
                    target.clone(),
                    *request_id,
                    event.timestamp_us,
                );
            }
            StructuralEventType::RequestEnd {
                request_id,
                success: true,
            } => {
                if let Some(pattern_name) = self.pattern_learner.complete_trace(*request_id) {
                    // Pattern learned/reinforced
                    if self
                        .pattern_learner
                        .observed_sequences
                        .get(&pattern_name)
                        .map(|p| p.occurrences == 10)
                        .unwrap_or(false)
                    {
                        self.optimizations
                            .push(format!("Learned pattern: {}", pattern_name));
                    }
                }
            }
            _ => {}
        }

        // 3. Track latency
        if let Some(latency) = event.latency_us {
            self.latency_trackers
                .entry(event.component.clone())
                .or_insert_with(|| VecDeque::with_capacity(100))
                .push_back(latency);

            let tracker = self.latency_trackers.get_mut(&event.component).unwrap();
            if tracker.len() > 100 {
                tracker.pop_front();
            }

            // Check for latency regression
            if tracker.len() >= 10 {
                let recent: Vec<_> = tracker.iter().rev().take(10).collect();
                let avg: u64 = recent.iter().copied().sum::<u64>() / 10;
                let old_avg: u64 = tracker.iter().take(10).sum::<u64>() / 10;

                if avg > old_avg * 2 {
                    let witness = StructuralWitness {
                        timestamp: event.timestamp_us,
                        trigger: format!("Latency regression: {:?}", event.component),
                        component_states: HashMap::new(),
                        causal_chain: vec![],
                        decision: "Investigate latency spike".to_string(),
                        action_taken: None,
                    };
                    self.witnesses.push(witness.clone());
                    return Some(witness);
                }
            }
        }

        None
    }

    /// Get system health summary
    pub fn health_summary(&self) -> SystemHealth {
        let open_circuits: Vec<_> = self
            .cascade_reflex
            .circuit_breakers
            .iter()
            .filter(|(_, cb)| cb.state == CircuitState::Open)
            .map(|(id, _)| id.clone())
            .collect();

        SystemHealth {
            components_monitored: self.latency_trackers.len(),
            patterns_learned: self.pattern_learner.observed_sequences.len(),
            open_circuit_breakers: open_circuits,
            recent_witnesses: self.witnesses.len(),
            optimizations_applied: self.optimizations.len(),
        }
    }
}

#[derive(Debug)]
pub struct SystemHealth {
    pub components_monitored: usize,
    pub patterns_learned: usize,
    pub open_circuit_breakers: Vec<ComponentId>,
    pub recent_witnesses: usize,
    pub optimizations_applied: usize,
}

fn main() {
    println!("=== Tier 2: Self-Optimizing Software and Workflows ===\n");

    let mut system = SelfOptimizingSystem::new();

    // Simulate normal operation - learning coordination patterns
    println!("Learning phase - observing normal coordination...");
    for req in 0..50 {
        let base_time = req * 10_000;

        // Simulate: API -> Auth -> DB pattern
        system.observe(StructuralEvent {
            timestamp_us: base_time,
            component: ComponentId("api".into()),
            event_type: StructuralEventType::RequestStart { request_id: req },
            latency_us: None,
            error: None,
        });

        system.observe(StructuralEvent {
            timestamp_us: base_time + 100,
            component: ComponentId("api".into()),
            event_type: StructuralEventType::Call {
                target: ComponentId("auth".into()),
                request_id: req,
            },
            latency_us: None,
            error: None,
        });

        system.observe(StructuralEvent {
            timestamp_us: base_time + 500,
            component: ComponentId("auth".into()),
            event_type: StructuralEventType::Call {
                target: ComponentId("db".into()),
                request_id: req,
            },
            latency_us: None,
            error: None,
        });

        system.observe(StructuralEvent {
            timestamp_us: base_time + 2000,
            component: ComponentId("api".into()),
            event_type: StructuralEventType::RequestEnd {
                request_id: req,
                success: true,
            },
            latency_us: Some(2000),
            error: None,
        });
    }

    let health = system.health_summary();
    println!("  Patterns learned: {}", health.patterns_learned);
    println!("  Components monitored: {}", health.components_monitored);
    println!("  Optimizations: {:?}", system.optimizations);

    // Simulate cascade failure
    println!("\nSimulating cascade failure...");
    for req in 50..60 {
        let base_time = 500_000 + req * 1_000;

        // Multiple components fail together
        for comp in ["api", "auth", "db", "cache"] {
            system.observe(StructuralEvent {
                timestamp_us: base_time + 100,
                component: ComponentId(comp.into()),
                event_type: StructuralEventType::RequestEnd {
                    request_id: req,
                    success: false,
                },
                latency_us: Some(50_000), // Slow failure
                error: Some("Connection timeout".into()),
            });
        }
    }

    // Check for cascade detection
    if let Some(last_witness) = system.witnesses.last() {
        println!("\n  CASCADE DETECTED!");
        println!("    Trigger: {}", last_witness.trigger);
        println!("    Decision: {}", last_witness.decision);
        println!("    Action: {:?}", last_witness.action_taken);
    }

    let health = system.health_summary();
    println!(
        "\n  Circuit breakers opened: {:?}",
        health.open_circuit_breakers
    );
    println!("  Witnesses logged: {}", health.recent_witnesses);

    println!("\n=== Key Benefits ===");
    println!("- Systems watch structure and timing, not just outputs");
    println!("- Reflex gates prevent cascading failures");
    println!("- Structural witnesses replace log diving");
    println!("- Patterns learned automatically for anomaly detection");
    println!("\nRuVector as connective tissue for self-stabilizing software.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker() {
        let mut cb = CircuitBreaker::new(3, 1000);

        assert!(cb.check(0));
        cb.record_failure(0);
        cb.record_failure(1);
        assert!(cb.check(2));
        cb.record_failure(2);
        assert!(!cb.check(3)); // Now open
        assert!(cb.check(1004)); // After timeout, half-open
    }

    #[test]
    fn test_pattern_learning() {
        let mut learner = PatternLearner::new();

        learner.observe_call(ComponentId("a".into()), ComponentId("b".into()), 1, 0);
        learner.observe_call(ComponentId("b".into()), ComponentId("c".into()), 1, 100);

        let pattern = learner.complete_trace(1);
        assert!(pattern.is_some());
    }

    #[test]
    fn test_cascade_detection() {
        let mut system = SelfOptimizingSystem::new();

        // Create cascade of failures
        for i in 0..5 {
            for comp in ["a", "b", "c", "d"] {
                system.observe(StructuralEvent {
                    timestamp_us: i * 100,
                    component: ComponentId(comp.into()),
                    event_type: StructuralEventType::RequestEnd {
                        request_id: i,
                        success: false,
                    },
                    latency_us: None,
                    error: None,
                });
            }
        }

        assert!(!system.witnesses.is_empty());
    }
}
