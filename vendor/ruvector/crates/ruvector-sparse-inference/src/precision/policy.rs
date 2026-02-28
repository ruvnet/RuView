//! Graduation Policy - Rules for lane transitions
//!
//! Implements the control theory for when signals should move between precision lanes.

use super::lanes::{LaneConfig, PrecisionLane};
use serde::{Deserialize, Serialize};

/// Metrics used for graduation decisions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraduationMetrics {
    /// Novelty score (0.0 to 1.0) - how different from recent patterns
    pub novelty: f32,

    /// Drift score (0.0 to 1.0) - how much the signal has drifted
    pub drift: f32,

    /// Number of steps drift has persisted
    pub drift_steps: usize,

    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,

    /// Stability score (0.0 to 1.0) - inverse of variance
    pub stability: f32,

    /// Number of stable steps
    pub stable_steps: usize,

    /// Velocity (rate of change)
    pub velocity: f32,

    /// Active set size (number of active neurons)
    pub active_set_size: usize,

    /// Uncertainty score (0.0 to 1.0)
    pub uncertainty: f32,

    /// Current cost usage (0.0 to 1.0)
    pub cost_usage: f32,

    /// Whether action is needed
    pub action_needed: bool,
}

impl GraduationMetrics {
    /// Create new metrics with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Update metrics with a new observation
    pub fn update(&mut self, observation: &ObservationMetrics, ema_alpha: f32) {
        // Exponential moving average for smooth updates
        self.novelty = ema_alpha * observation.novelty + (1.0 - ema_alpha) * self.novelty;
        self.drift = ema_alpha * observation.drift + (1.0 - ema_alpha) * self.drift;
        self.confidence = ema_alpha * observation.confidence + (1.0 - ema_alpha) * self.confidence;
        self.stability = ema_alpha * observation.stability + (1.0 - ema_alpha) * self.stability;
        self.velocity = ema_alpha * observation.velocity + (1.0 - ema_alpha) * self.velocity;
        self.uncertainty =
            ema_alpha * observation.uncertainty + (1.0 - ema_alpha) * self.uncertainty;

        self.active_set_size = observation.active_set_size;
        self.action_needed = observation.action_needed;

        // Update drift persistence
        if observation.drift > 0.1 {
            self.drift_steps += 1;
        } else {
            self.drift_steps = 0;
        }

        // Update stability persistence
        if observation.stability > 0.8 {
            self.stable_steps += 1;
        } else {
            self.stable_steps = 0;
        }
    }
}

/// Raw observation metrics from a single step
#[derive(Debug, Clone, Default)]
pub struct ObservationMetrics {
    pub novelty: f32,
    pub drift: f32,
    pub confidence: f32,
    pub stability: f32,
    pub velocity: f32,
    pub uncertainty: f32,
    pub active_set_size: usize,
    pub action_needed: bool,
}

/// Decision from graduation policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraduationDecision {
    /// Stay in current lane
    Stay,
    /// Escalate to higher precision lane
    Escalate(PrecisionLane),
    /// Demote to lower precision lane
    Demote(PrecisionLane),
}

/// Graduation policy for lane transitions
#[derive(Debug, Clone)]
pub struct GraduationPolicy {
    /// Current precision lane
    pub current_lane: PrecisionLane,
    /// Configuration
    pub config: LaneConfig,
    /// Accumulated metrics
    pub metrics: GraduationMetrics,
    /// EMA smoothing factor
    pub ema_alpha: f32,
}

impl GraduationPolicy {
    /// Create a new graduation policy
    pub fn new(initial_lane: PrecisionLane, config: LaneConfig) -> Self {
        Self {
            current_lane: initial_lane,
            config,
            metrics: GraduationMetrics::new(),
            ema_alpha: 0.3,
        }
    }

    /// Evaluate and return graduation decision
    pub fn evaluate(&mut self, observation: &ObservationMetrics) -> GraduationDecision {
        // Update metrics
        self.metrics.update(observation, self.ema_alpha);

        // Check for escalation
        if self.should_escalate() {
            if let Some(next_lane) = self.next_higher_lane() {
                return GraduationDecision::Escalate(next_lane);
            }
        }

        // Check for demotion
        if self.should_demote() {
            if let Some(prev_lane) = self.next_lower_lane() {
                return GraduationDecision::Demote(prev_lane);
            }
        }

        GraduationDecision::Stay
    }

    /// Apply a graduation decision
    pub fn apply_decision(&mut self, decision: GraduationDecision) {
        match decision {
            GraduationDecision::Stay => {}
            GraduationDecision::Escalate(lane) | GraduationDecision::Demote(lane) => {
                self.current_lane = lane;
                // Reset stability counters on lane change
                self.metrics.stable_steps = 0;
                self.metrics.drift_steps = 0;
            }
        }
    }

    /// Check if escalation conditions are met
    fn should_escalate(&self) -> bool {
        // Escalate when:
        // 1. Novelty exceeds threshold
        let novelty_trigger = self.metrics.novelty > self.config.novelty_threshold;

        // 2. Drift persists
        let drift_trigger = self.metrics.drift_steps >= self.config.drift_persistence_threshold;

        // 3. Confidence and stability pass
        let quality_pass = self.metrics.confidence >= self.config.confidence_threshold
            && self.metrics.stability >= 0.5;

        // 4. Cost budget allows
        let budget_allows = self.metrics.cost_usage < self.config.escalation_budget;

        // Escalate if any trigger fires AND quality/budget conditions are met
        (novelty_trigger || drift_trigger) && quality_pass && budget_allows
    }

    /// Check if demotion conditions are met
    fn should_demote(&self) -> bool {
        // Demote when:
        // 1. Stability returns
        let stability_returned = self.metrics.stable_steps >= self.config.min_stability_steps;

        // 2. Velocity stalls
        let velocity_stalled = self.metrics.velocity.abs() < 0.01;

        // 3. Active set shrinks (not using the precision)
        let active_set_shrunk = self.metrics.active_set_size < 10;

        // 4. High uncertainty but no action needed
        let uncertain_idle = self.metrics.uncertainty > 0.7 && !self.metrics.action_needed;

        // Demote if stability AND (velocity stall OR active shrink OR uncertain idle)
        stability_returned && (velocity_stalled || active_set_shrunk || uncertain_idle)
    }

    /// Get the next higher precision lane
    fn next_higher_lane(&self) -> Option<PrecisionLane> {
        match self.current_lane {
            PrecisionLane::Bit3 => Some(PrecisionLane::Bit5),
            PrecisionLane::Bit5 => Some(PrecisionLane::Bit7),
            PrecisionLane::Bit7 => Some(PrecisionLane::Float32),
            PrecisionLane::Float32 => None,
        }
    }

    /// Get the next lower precision lane
    fn next_lower_lane(&self) -> Option<PrecisionLane> {
        match self.current_lane {
            PrecisionLane::Bit3 => None,
            PrecisionLane::Bit5 => Some(PrecisionLane::Bit3),
            PrecisionLane::Bit7 => Some(PrecisionLane::Bit5),
            PrecisionLane::Float32 => Some(PrecisionLane::Bit7),
        }
    }
}

/// Event processor with precision lane awareness
pub struct LanedEventProcessor {
    /// Graduation policy
    policy: GraduationPolicy,
    /// Event counter
    event_count: usize,
}

impl LanedEventProcessor {
    /// Create a new event processor
    pub fn new(config: LaneConfig) -> Self {
        Self {
            policy: GraduationPolicy::new(config.default_lane, config),
            event_count: 0,
        }
    }

    /// Process an event through the appropriate precision lane
    pub fn process_event(&mut self, event: &Event) -> ProcessResult {
        self.event_count += 1;

        // 3-bit reflex check (always runs first)
        let reflex_result = self.reflex_3bit(event);
        if !reflex_result.boundary_crossed {
            return ProcessResult::Reflexed(reflex_result);
        }

        // 5-bit embedding update (event-driven)
        let embed_result = self.embed_5bit(event);

        // Check for graduation to 7-bit
        let observation = self.compute_observation(&reflex_result, &embed_result);
        let decision = self.policy.evaluate(&observation);

        if matches!(decision, GraduationDecision::Escalate(PrecisionLane::Bit7))
            || self.policy.current_lane == PrecisionLane::Bit7
        {
            // 7-bit reasoning
            let reason_result = self.reason_7bit(event, &embed_result);
            self.policy.apply_decision(decision);
            return ProcessResult::Reasoned(reason_result);
        }

        self.policy.apply_decision(decision);
        ProcessResult::Embedded(embed_result)
    }

    fn reflex_3bit(&self, _event: &Event) -> ReflexResult {
        // 3-bit reflex processing
        ReflexResult {
            boundary_crossed: true, // Simplified
            health_ok: true,
            anomaly_detected: false,
        }
    }

    fn embed_5bit(&self, _event: &Event) -> EmbedResult {
        // 5-bit embedding update
        EmbedResult {
            embedding_delta: vec![0.0; 64],
            drift_detected: false,
        }
    }

    fn reason_7bit(&self, _event: &Event, _embed: &EmbedResult) -> ReasonResult {
        // 7-bit reasoning
        ReasonResult {
            should_write_memory: false,
            summary: String::new(),
            actions: Vec::new(),
        }
    }

    fn compute_observation(
        &self,
        _reflex: &ReflexResult,
        _embed: &EmbedResult,
    ) -> ObservationMetrics {
        ObservationMetrics::default()
    }

    /// Get current lane
    pub fn current_lane(&self) -> PrecisionLane {
        self.policy.current_lane
    }
}

/// Simple event type for processing
#[derive(Debug, Clone)]
pub struct Event {
    pub data: Vec<f32>,
    pub timestamp: u64,
}

/// Result of 3-bit reflex processing
#[derive(Debug, Clone)]
pub struct ReflexResult {
    pub boundary_crossed: bool,
    pub health_ok: bool,
    pub anomaly_detected: bool,
}

/// Result of 5-bit embedding
#[derive(Debug, Clone)]
pub struct EmbedResult {
    pub embedding_delta: Vec<f32>,
    pub drift_detected: bool,
}

/// Result of 7-bit reasoning
#[derive(Debug, Clone)]
pub struct ReasonResult {
    pub should_write_memory: bool,
    pub summary: String,
    pub actions: Vec<String>,
}

/// Overall processing result
#[derive(Debug)]
pub enum ProcessResult {
    Reflexed(ReflexResult),
    Embedded(EmbedResult),
    Reasoned(ReasonResult),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graduation_policy_creation() {
        let config = LaneConfig::default();
        let policy = GraduationPolicy::new(PrecisionLane::Bit5, config);

        assert_eq!(policy.current_lane, PrecisionLane::Bit5);
    }

    #[test]
    fn test_escalation_on_novelty() {
        let config = LaneConfig {
            novelty_threshold: 0.3,
            confidence_threshold: 0.5,
            ..Default::default()
        };
        let mut policy = GraduationPolicy::new(PrecisionLane::Bit5, config);
        // Set higher EMA alpha for faster response in tests
        policy.ema_alpha = 1.0;

        // High novelty, good confidence (use high values to overcome any thresholds)
        let observation = ObservationMetrics {
            novelty: 0.9,
            confidence: 0.9,
            stability: 0.6,
            ..Default::default()
        };

        let decision = policy.evaluate(&observation);
        assert!(matches!(
            decision,
            GraduationDecision::Escalate(PrecisionLane::Bit7)
        ));
    }

    #[test]
    fn test_demotion_on_stability() {
        let mut config = LaneConfig::default();
        config.min_stability_steps = 2;

        let mut policy = GraduationPolicy::new(PrecisionLane::Bit7, config);

        // Build up stable steps
        for _ in 0..5 {
            let observation = ObservationMetrics {
                stability: 0.9,
                velocity: 0.001,
                active_set_size: 5,
                ..Default::default()
            };
            policy.evaluate(&observation);
        }

        let observation = ObservationMetrics {
            stability: 0.9,
            velocity: 0.001,
            active_set_size: 5,
            ..Default::default()
        };

        let decision = policy.evaluate(&observation);
        assert!(matches!(
            decision,
            GraduationDecision::Demote(PrecisionLane::Bit5)
        ));
    }
}
