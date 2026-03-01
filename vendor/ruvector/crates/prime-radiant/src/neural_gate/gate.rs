//! Neural coherence gate implementation.

use super::config::NeuralGateConfig;
use super::decision::{DecisionConfidence, DecisionTrigger, HysteresisState, NeuralDecision};
use super::encoding::{HdcMemory, Hypervector, WitnessEncoding};
use std::collections::VecDeque;

/// State of the neural coherence gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateState {
    /// Gate is uninitialized.
    Uninitialized,
    /// Gate is ready.
    Ready,
    /// Gate is processing.
    Processing,
    /// Gate is in broadcast mode.
    Broadcasting,
}

/// Hysteresis tracker for stable decisions.
#[derive(Debug)]
struct HysteresisTracker {
    /// Current state.
    state: HysteresisState,
    /// Smoothed energy value.
    smoothed_energy: f32,
    /// Time entered current state.
    state_entered_ms: u64,
    /// Low threshold.
    low_threshold: f32,
    /// High threshold.
    high_threshold: f32,
    /// Minimum dwell time.
    min_dwell_ms: u64,
    /// Smoothing factor.
    smoothing: f32,
}

impl HysteresisTracker {
    fn new(config: &super::config::HysteresisConfig) -> Self {
        Self {
            state: HysteresisState::Low,
            smoothed_energy: 0.0,
            state_entered_ms: current_time_ms(),
            low_threshold: config.low_threshold,
            high_threshold: config.high_threshold,
            min_dwell_ms: config.min_dwell_time_ms,
            smoothing: config.smoothing_factor,
        }
    }

    fn update(&mut self, energy: f32) -> Option<HysteresisState> {
        // Apply exponential smoothing
        self.smoothed_energy =
            self.smoothing * self.smoothed_energy + (1.0 - self.smoothing) * energy;

        let now = current_time_ms();
        let dwell_time = now - self.state_entered_ms;

        // Check if we've dwelled long enough to consider switching
        if dwell_time < self.min_dwell_ms {
            return None;
        }

        let old_state = self.state;

        // Determine new state based on smoothed energy
        let new_state = match self.state {
            HysteresisState::Low => {
                if self.smoothed_energy > self.high_threshold {
                    HysteresisState::High
                } else if self.smoothed_energy > self.low_threshold {
                    HysteresisState::Transition
                } else {
                    HysteresisState::Low
                }
            }
            HysteresisState::Transition => {
                if self.smoothed_energy > self.high_threshold {
                    HysteresisState::High
                } else if self.smoothed_energy < self.low_threshold {
                    HysteresisState::Low
                } else {
                    HysteresisState::Transition
                }
            }
            HysteresisState::High => {
                if self.smoothed_energy < self.low_threshold {
                    HysteresisState::Low
                } else if self.smoothed_energy < self.high_threshold {
                    HysteresisState::Transition
                } else {
                    HysteresisState::High
                }
            }
        };

        if new_state != old_state {
            self.state = new_state;
            self.state_entered_ms = now;
            Some(new_state)
        } else {
            None
        }
    }
}

/// Dendritic coincidence detector.
#[derive(Debug)]
struct DendriticDetector {
    /// Active synapses (timestamp of last spike).
    synapses: VecDeque<(u64, u64)>, // (synapse_id, timestamp_ms)
    /// Coincidence window in ms.
    window_ms: u64,
    /// Threshold for coincidence detection.
    threshold: usize,
}

impl DendriticDetector {
    fn new(window_us: u64, threshold: usize) -> Self {
        Self {
            synapses: VecDeque::with_capacity(100),
            window_ms: window_us / 1000,
            threshold,
        }
    }

    fn receive_spike(&mut self, synapse_id: u64) {
        let now = current_time_ms();

        // Remove old spikes
        while let Some(&(_, ts)) = self.synapses.front() {
            if now - ts > self.window_ms {
                self.synapses.pop_front();
            } else {
                break;
            }
        }

        // Add new spike
        self.synapses.push_back((synapse_id, now));
    }

    fn check_coincidence(&self) -> Option<usize> {
        let now = current_time_ms();

        // Count unique synapses that fired within window
        let active: std::collections::HashSet<u64> = self
            .synapses
            .iter()
            .filter(|(_, ts)| now - ts <= self.window_ms)
            .map(|(id, _)| *id)
            .collect();

        if active.len() >= self.threshold {
            Some(active.len())
        } else {
            None
        }
    }

    fn clear(&mut self) {
        self.synapses.clear();
    }
}

/// Global workspace for conscious access.
#[derive(Debug)]
struct GlobalWorkspace {
    /// Buffer of recent decisions.
    buffer: VecDeque<NeuralDecision>,
    /// Capacity.
    capacity: usize,
    /// Broadcast threshold.
    broadcast_threshold: f32,
    /// Broadcast listeners (count).
    listener_count: usize,
}

impl GlobalWorkspace {
    fn new(config: &super::config::WorkspaceConfig) -> Self {
        Self {
            buffer: VecDeque::with_capacity(config.buffer_capacity),
            capacity: config.buffer_capacity,
            broadcast_threshold: config.broadcast_threshold,
            listener_count: 0,
        }
    }

    fn broadcast(&mut self, decision: NeuralDecision) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(decision);
        self.listener_count += 1; // Simulate notification
    }

    fn recent_decisions(&self, count: usize) -> Vec<&NeuralDecision> {
        self.buffer.iter().rev().take(count).collect()
    }

    fn should_broadcast(&self, confidence: f32) -> bool {
        confidence >= self.broadcast_threshold
    }
}

/// Context for gate evaluation.
#[derive(Debug, Clone)]
pub struct EvaluationContext {
    /// Evidence source IDs.
    pub evidence_sources: Vec<u64>,
    /// Timestamp.
    pub timestamp_ms: u64,
    /// Additional metadata.
    pub metadata: std::collections::HashMap<String, String>,
}

impl EvaluationContext {
    /// Create a new context.
    pub fn new() -> Self {
        Self {
            evidence_sources: Vec::new(),
            timestamp_ms: current_time_ms(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Add an evidence source.
    pub fn with_evidence(mut self, source_id: u64) -> Self {
        self.evidence_sources.push(source_id);
        self
    }
}

impl Default for EvaluationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Neural coherence gate using biologically-inspired mechanisms.
pub struct NeuralCoherenceGate {
    /// Configuration.
    config: NeuralGateConfig,
    /// Hysteresis tracker.
    hysteresis: HysteresisTracker,
    /// Dendritic coincidence detector.
    dendrite: DendriticDetector,
    /// Global workspace.
    workspace: GlobalWorkspace,
    /// HDC memory for witness encoding.
    hdc_memory: HdcMemory,
    /// State.
    state: GateState,
    /// Total evaluations.
    total_evaluations: u64,
}

impl NeuralCoherenceGate {
    /// Create a new neural coherence gate.
    pub fn new(config: NeuralGateConfig) -> Self {
        let hysteresis = HysteresisTracker::new(&config.hysteresis);
        let dendrite =
            DendriticDetector::new(config.coincidence_window_us, config.num_branches / 2);
        let workspace = GlobalWorkspace::new(&config.workspace);
        let hdc_memory = HdcMemory::new(config.hdc_dimension, config.memory_capacity);

        Self {
            config,
            hysteresis,
            dendrite,
            workspace,
            hdc_memory,
            state: GateState::Ready,
            total_evaluations: 0,
        }
    }

    /// Create with default configuration.
    pub fn default_gate() -> Self {
        Self::new(NeuralGateConfig::default())
    }

    /// Get the current state.
    pub fn state(&self) -> GateState {
        self.state
    }

    /// Evaluate whether to allow an action.
    pub fn evaluate(&mut self, energy: f32, context: &EvaluationContext) -> NeuralDecision {
        self.state = GateState::Processing;
        self.total_evaluations += 1;

        // Process evidence through dendritic detector
        for &source in &context.evidence_sources {
            self.dendrite.receive_spike(source);
        }

        // Check for dendritic coincidence
        let dendritic_fire = self.dendrite.check_coincidence();
        let dendritic_confidence = dendritic_fire
            .map(|count| (count as f32 / self.config.num_branches as f32).min(1.0))
            .unwrap_or(0.3);

        // Update hysteresis
        let state_change = self.hysteresis.update(energy);
        let hysteresis_state = self.hysteresis.state;

        // Determine trigger
        let trigger = if let Some(count) = dendritic_fire {
            DecisionTrigger::DendriticCoincidence {
                active_synapses: count,
                threshold: self.config.num_branches / 2,
            }
        } else if let Some(new_state) = state_change {
            DecisionTrigger::HysteresisChange {
                from_state: match new_state {
                    HysteresisState::High => HysteresisState::Transition,
                    HysteresisState::Low => HysteresisState::Transition,
                    HysteresisState::Transition => HysteresisState::Low,
                },
                to_state: new_state,
            }
        } else {
            DecisionTrigger::EnergyThreshold {
                threshold: self.hysteresis.low_threshold,
                upward: energy > self.hysteresis.smoothed_energy,
            }
        };

        // Compute confidence
        let energy_confidence = 1.0 - energy.min(1.0);
        let oscillator_confidence = 0.7; // Placeholder
        let confidence = DecisionConfidence::new(
            energy_confidence,
            dendritic_confidence,
            oscillator_confidence,
            context.evidence_sources.len(),
        );

        // Make decision
        let allow = match hysteresis_state {
            HysteresisState::Low => true,
            HysteresisState::Transition => confidence.overall > 0.5,
            HysteresisState::High => false,
        };

        let decision = NeuralDecision::new(
            allow,
            energy,
            self.hysteresis.smoothed_energy,
            hysteresis_state,
            trigger,
            confidence,
        );

        // Broadcast if significant
        if decision.should_broadcast && self.workspace.should_broadcast(confidence.overall) {
            self.state = GateState::Broadcasting;
            self.workspace.broadcast(decision.clone());
        }

        self.state = GateState::Ready;
        decision
    }

    /// Encode a witness record as a hypervector.
    pub fn encode_witness(
        &mut self,
        witness_id: &str,
        energy: f32,
        allow: bool,
        policy_hash: &[u8],
    ) -> WitnessEncoding {
        let encoding = WitnessEncoding::new(
            witness_id,
            energy,
            allow,
            policy_hash,
            self.config.hdc_dimension,
        );

        self.hdc_memory.store(encoding.clone());
        encoding
    }

    /// Find similar past witnesses.
    pub fn find_similar_witnesses(&self, query: &Hypervector, threshold: f32) -> Vec<String> {
        self.hdc_memory
            .retrieve(query, threshold)
            .into_iter()
            .map(|(id, _)| id)
            .collect()
    }

    /// Get recent decisions from the workspace.
    pub fn recent_decisions(&self, count: usize) -> Vec<&NeuralDecision> {
        self.workspace.recent_decisions(count)
    }

    /// Get gate statistics.
    pub fn stats(&self) -> GateStats {
        GateStats {
            state: self.state,
            hysteresis_state: self.hysteresis.state,
            smoothed_energy: self.hysteresis.smoothed_energy,
            total_evaluations: self.total_evaluations,
            encoded_witnesses: self.hdc_memory.len(),
        }
    }

    /// Reset the gate.
    pub fn reset(&mut self) {
        self.hysteresis = HysteresisTracker::new(&self.config.hysteresis);
        self.dendrite.clear();
        self.hdc_memory.clear();
        self.total_evaluations = 0;
        self.state = GateState::Ready;
    }
}

impl std::fmt::Debug for NeuralCoherenceGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NeuralCoherenceGate")
            .field("state", &self.state)
            .field("hysteresis_state", &self.hysteresis.state)
            .field("total_evaluations", &self.total_evaluations)
            .finish()
    }
}

/// Gate statistics.
#[derive(Debug, Clone, Copy)]
pub struct GateStats {
    /// Current state.
    pub state: GateState,
    /// Current hysteresis state.
    pub hysteresis_state: HysteresisState,
    /// Smoothed energy value.
    pub smoothed_energy: f32,
    /// Total evaluations.
    pub total_evaluations: u64,
    /// Number of encoded witnesses.
    pub encoded_witnesses: usize,
}

/// Get current time in milliseconds.
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_creation() {
        let gate = NeuralCoherenceGate::default_gate();
        assert_eq!(gate.state(), GateState::Ready);
    }

    #[test]
    fn test_evaluate_low_energy() {
        let mut gate = NeuralCoherenceGate::default_gate();
        let context = EvaluationContext::new();

        let decision = gate.evaluate(0.1, &context);
        assert!(decision.allow);
        assert_eq!(decision.hysteresis_state, HysteresisState::Low);
    }

    #[test]
    fn test_evaluate_high_energy() {
        let mut gate = NeuralCoherenceGate::default_gate();
        let context = EvaluationContext::new();

        // Need multiple evaluations to move through hysteresis
        for _ in 0..10 {
            gate.evaluate(0.9, &context);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        let decision = gate.evaluate(0.9, &context);
        // After sustained high energy, should deny
        assert!(!decision.allow || decision.hysteresis_state == HysteresisState::High);
    }

    #[test]
    fn test_witness_encoding() {
        let mut gate = NeuralCoherenceGate::default_gate();

        let encoding = gate.encode_witness("test", 0.5, true, &[1, 2, 3, 4]);

        assert_eq!(encoding.witness_id, "test");
        assert!(encoding.allow);
    }

    #[test]
    fn test_find_similar() {
        let mut gate = NeuralCoherenceGate::default_gate();

        gate.encode_witness("w1", 0.5, true, &[1, 2, 3, 4]);
        gate.encode_witness("w2", 0.6, true, &[1, 2, 3, 5]);

        let query = Hypervector::from_bytes(&[1, 2, 3, 4], gate.config.hdc_dimension);
        let similar = gate.find_similar_witnesses(&query, 0.5);

        assert!(!similar.is_empty());
    }
}
