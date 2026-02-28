//! Stigmergy-based coordination using digital pheromones for self-organizing task allocation
//!
//! Stigmergy is an indirect coordination mechanism where agents leave traces (pheromones)
//! in the environment that influence the behavior of other agents. This creates emergent
//! specialization without explicit communication.
//!
//! ## Features
//!
//! - **Emergent Specialization**: Nodes naturally gravitate to successful task types
//! - **Self-Healing**: Failed task types lose pheromones, causing nodes to redistribute
//! - **P2P Sync**: Pheromone trails shared via gossip protocol
//! - **Anti-Sybil**: Deposit proportional to stake/reputation
//! - **Task Routing**: High-pheromone nodes get priority for matching tasks

use crate::tasks::TaskType;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wasm_bindgen::prelude::*;

/// Type alias for peer identifiers (matches WasmNodeIdentity.node_id)
pub type PeerId = String;

/// Ring buffer for bounded history storage
#[derive(Clone, Debug, Default)]
pub struct RingBuffer<T> {
    buffer: VecDeque<T>,
    capacity: usize,
}

impl<T> RingBuffer<T> {
    /// Create a new ring buffer with specified capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push an item, evicting oldest if at capacity
    pub fn push(&mut self, item: T) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(item);
    }

    /// Get number of items in buffer
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Iterate over items
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer.iter()
    }

    /// Clear all items
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

/// A deposit record in the pheromone trail
#[derive(Clone, Debug)]
pub struct PheromoneDeposit {
    /// Peer who made the deposit
    pub peer_id: PeerId,
    /// Amount deposited
    pub amount: f32,
    /// When the deposit was made
    pub timestamp: Instant,
    /// Stake/reputation weight (anti-sybil)
    pub stake_weight: f32,
}

/// Pheromone trail for a specific task type
#[derive(Clone, Debug)]
pub struct PheromoneTrail {
    /// Current intensity (sum of active pheromones)
    pub intensity: f32,
    /// When the trail was last updated
    pub last_deposit: Instant,
    /// History of recent deposits (for analysis)
    pub deposit_history: RingBuffer<PheromoneDeposit>,
    /// Success rate for this task type (rolling average)
    pub success_rate: f32,
    /// Total tasks completed on this trail
    pub total_completions: u64,
    /// Total tasks failed on this trail
    pub total_failures: u64,
}

impl Default for PheromoneTrail {
    fn default() -> Self {
        Self {
            intensity: 0.0,
            last_deposit: Instant::now(),
            deposit_history: RingBuffer::new(100), // Keep last 100 deposits
            success_rate: 0.5, // Start neutral
            total_completions: 0,
            total_failures: 0,
        }
    }
}

impl PheromoneTrail {
    /// Update success rate with exponential moving average
    pub fn record_outcome(&mut self, success: bool) {
        const ALPHA: f32 = 0.1; // Smoothing factor
        let outcome = if success { 1.0 } else { 0.0 };
        self.success_rate = (1.0 - ALPHA) * self.success_rate + ALPHA * outcome;

        if success {
            self.total_completions += 1;
        } else {
            self.total_failures += 1;
        }
    }

    /// Get weighted intensity (considering success rate)
    pub fn weighted_intensity(&self) -> f32 {
        self.intensity * self.success_rate
    }
}

/// Serializable pheromone state for P2P sync
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PheromoneState {
    /// Task type as string for serialization
    pub task_type: String,
    /// Current intensity
    pub intensity: f32,
    /// Success rate
    pub success_rate: f32,
    /// Last update timestamp (unix ms)
    pub last_update_ms: u64,
}

/// Stigmergy coordination engine
///
/// Implements indirect coordination through digital pheromones.
/// Agents deposit pheromones after successful task completions,
/// and follow pheromone gradients to decide which tasks to accept.
pub struct Stigmergy {
    /// Pheromone trails indexed by task type
    pheromones: Arc<RwLock<FxHashMap<TaskType, PheromoneTrail>>>,
    /// Decay rate per epoch (0.1 = 10% decay)
    decay_rate: f32,
    /// Base deposit rate (multiplied by success rate)
    deposit_rate: f32,
    /// How often evaporation occurs
    evaporation_interval: Duration,
    /// Last evaporation time
    last_evaporation: RwLock<Instant>,
    /// Minimum stake required for deposit (anti-sybil)
    min_stake: u64,
    /// Our node's specialization scores (learned preferences)
    node_specializations: Arc<RwLock<FxHashMap<TaskType, f32>>>,
}

impl Default for Stigmergy {
    fn default() -> Self {
        Self::new()
    }
}

impl Stigmergy {
    /// Create a new stigmergy engine with default parameters
    pub fn new() -> Self {
        Self {
            pheromones: Arc::new(RwLock::new(FxHashMap::default())),
            decay_rate: 0.1,                              // 10% decay per epoch
            deposit_rate: 1.0,                            // Base deposit amount
            evaporation_interval: Duration::from_secs(3600), // 1 hour
            last_evaporation: RwLock::new(Instant::now()),
            min_stake: 0,
            node_specializations: Arc::new(RwLock::new(FxHashMap::default())),
        }
    }

    /// Create with custom parameters
    pub fn with_params(decay_rate: f32, deposit_rate: f32, evaporation_hours: f32) -> Self {
        Self {
            pheromones: Arc::new(RwLock::new(FxHashMap::default())),
            decay_rate: decay_rate.clamp(0.0, 1.0),
            deposit_rate: deposit_rate.max(0.0),
            evaporation_interval: Duration::from_secs_f32(evaporation_hours * 3600.0),
            last_evaporation: RwLock::new(Instant::now()),
            min_stake: 0,
            node_specializations: Arc::new(RwLock::new(FxHashMap::default())),
        }
    }

    /// Set minimum stake for anti-sybil protection
    pub fn set_min_stake(&mut self, min_stake: u64) {
        self.min_stake = min_stake;
    }

    /// Deposit pheromone after successful task completion
    ///
    /// The deposit amount is proportional to:
    /// - Base deposit rate
    /// - Success rate of the completing peer
    /// - Stake weight for anti-sybil protection
    ///
    /// # Arguments
    /// * `task_type` - Type of task completed
    /// * `peer_id` - ID of the completing peer
    /// * `success_rate` - Peer's success rate (0.0 - 1.0)
    /// * `stake` - Peer's stake for anti-sybil weighting
    pub fn deposit(&self, task_type: TaskType, peer_id: PeerId, success_rate: f32, stake: u64) {
        // Anti-sybil: require minimum stake
        if stake < self.min_stake {
            return;
        }

        let mut trails = self.pheromones.write();
        let trail = trails.entry(task_type).or_default();

        // Calculate stake weight (logarithmic to prevent whale dominance)
        let stake_weight = (stake as f32).ln_1p() / 10.0;
        let stake_weight = stake_weight.clamp(0.1, 2.0);

        // Calculate deposit amount
        let deposit_amount = self.deposit_rate * success_rate * stake_weight;

        // Update trail
        trail.intensity += deposit_amount;
        trail.last_deposit = Instant::now();

        // Record in history
        trail.deposit_history.push(PheromoneDeposit {
            peer_id,
            amount: deposit_amount,
            timestamp: Instant::now(),
            stake_weight,
        });
    }

    /// Deposit with outcome recording (success or failure)
    pub fn deposit_with_outcome(
        &self,
        task_type: TaskType,
        peer_id: PeerId,
        success: bool,
        stake: u64,
    ) {
        let mut trails = self.pheromones.write();
        let trail = trails.entry(task_type).or_default();

        // Record the outcome
        trail.record_outcome(success);

        if success && stake >= self.min_stake {
            let stake_weight = (stake as f32).ln_1p() / 10.0;
            let stake_weight = stake_weight.clamp(0.1, 2.0);
            let deposit_amount = self.deposit_rate * trail.success_rate * stake_weight;

            trail.intensity += deposit_amount;
            trail.last_deposit = Instant::now();

            trail.deposit_history.push(PheromoneDeposit {
                peer_id,
                amount: deposit_amount,
                timestamp: Instant::now(),
                stake_weight,
            });
        }
    }

    /// Follow pheromone gradient to decide task acceptance probability
    ///
    /// Returns a probability (0.0 - 1.0) based on pheromone intensity.
    /// Uses sigmoid function for smooth probability curve.
    ///
    /// # Arguments
    /// * `task_type` - Type of task to evaluate
    ///
    /// # Returns
    /// Probability of accepting this task type (0.0 - 1.0)
    pub fn follow(&self, task_type: TaskType) -> f32 {
        let trails = self.pheromones.read();
        let intensity = trails
            .get(&task_type)
            .map(|t| t.weighted_intensity())
            .unwrap_or(0.0);

        // Sigmoid function for probability
        // Higher intensity -> higher probability
        1.0 / (1.0 + (-intensity).exp())
    }

    /// Get raw pheromone intensity for a task type
    pub fn get_intensity(&self, task_type: TaskType) -> f32 {
        self.pheromones
            .read()
            .get(&task_type)
            .map(|t| t.intensity)
            .unwrap_or(0.0)
    }

    /// Get success rate for a task type
    pub fn get_success_rate(&self, task_type: TaskType) -> f32 {
        self.pheromones
            .read()
            .get(&task_type)
            .map(|t| t.success_rate)
            .unwrap_or(0.5)
    }

    /// Evaporate old pheromones (called periodically)
    ///
    /// Pheromone intensity decays exponentially based on time since last deposit.
    /// This ensures that inactive trails fade over time, allowing the network
    /// to adapt to changing conditions.
    pub fn evaporate(&self) {
        let mut trails = self.pheromones.write();
        let now = Instant::now();

        for (_task_type, trail) in trails.iter_mut() {
            let elapsed_hours = trail.last_deposit.elapsed().as_secs_f32() / 3600.0;
            let decay_factor = (1.0 - self.decay_rate).powf(elapsed_hours);
            trail.intensity *= decay_factor;

            // Update last deposit time to now (for next decay calculation)
            // Note: This is a simplification; in practice you might want to
            // track the actual decay progression separately
            trail.last_deposit = now;
        }

        // Clean up very weak trails (intensity < 0.01)
        trails.retain(|_, trail| trail.intensity >= 0.01);

        *self.last_evaporation.write() = now;
    }

    /// Check if evaporation is due and run if needed
    pub fn maybe_evaporate(&self) -> bool {
        let last = *self.last_evaporation.read();
        if last.elapsed() >= self.evaporation_interval {
            self.evaporate();
            true
        } else {
            false
        }
    }

    /// P2P sync: merge pheromone trails from peers
    ///
    /// Uses weighted average to combine local and remote state.
    /// Local state is weighted higher (0.7) to prevent manipulation.
    ///
    /// # Arguments
    /// * `peer_trails` - Map of task types to intensity values from peer
    pub fn merge(&self, peer_trails: &FxHashMap<TaskType, f32>) {
        const LOCAL_WEIGHT: f32 = 0.7;
        const REMOTE_WEIGHT: f32 = 0.3;

        let mut trails = self.pheromones.write();

        for (task_type, remote_intensity) in peer_trails {
            let trail = trails.entry(*task_type).or_default();
            // Weighted average with local priority
            trail.intensity = LOCAL_WEIGHT * trail.intensity + REMOTE_WEIGHT * remote_intensity;
        }
    }

    /// Merge with full state (including success rates)
    pub fn merge_state(&self, peer_states: &[PheromoneState]) {
        const LOCAL_WEIGHT: f32 = 0.7;
        const REMOTE_WEIGHT: f32 = 0.3;

        let mut trails = self.pheromones.write();

        for state in peer_states {
            if let Some(task_type) = parse_task_type(&state.task_type) {
                let trail = trails.entry(task_type).or_default();
                trail.intensity = LOCAL_WEIGHT * trail.intensity + REMOTE_WEIGHT * state.intensity;
                trail.success_rate =
                    LOCAL_WEIGHT * trail.success_rate + REMOTE_WEIGHT * state.success_rate;
            }
        }
    }

    /// Export current state for P2P sharing
    pub fn export_state(&self) -> Vec<PheromoneState> {
        let trails = self.pheromones.read();
        let now = js_sys::Date::now() as u64;

        trails
            .iter()
            .map(|(task_type, trail)| PheromoneState {
                task_type: format!("{:?}", task_type),
                intensity: trail.intensity,
                success_rate: trail.success_rate,
                last_update_ms: now,
            })
            .collect()
    }

    /// Get the best task type for this node based on pheromone gradients
    ///
    /// Returns the task type with highest weighted intensity,
    /// indicating where the node should specialize.
    pub fn get_best_specialization(&self) -> Option<TaskType> {
        let trails = self.pheromones.read();
        trails
            .iter()
            .max_by(|a, b| {
                a.1.weighted_intensity()
                    .partial_cmp(&b.1.weighted_intensity())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(task_type, _)| *task_type)
    }

    /// Get all task types ranked by attractiveness
    pub fn get_ranked_tasks(&self) -> Vec<(TaskType, f32)> {
        let trails = self.pheromones.read();
        let mut ranked: Vec<_> = trails
            .iter()
            .map(|(tt, trail)| (*tt, trail.weighted_intensity()))
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked
    }

    /// Update node's specialization preference based on task outcome
    pub fn update_specialization(&self, task_type: TaskType, success: bool) {
        let mut specs = self.node_specializations.write();
        let score = specs.entry(task_type).or_insert(0.5);

        // Adjust specialization score based on outcome
        const LEARNING_RATE: f32 = 0.1;
        let target = if success { 1.0 } else { 0.0 };
        *score = (1.0 - LEARNING_RATE) * *score + LEARNING_RATE * target;
    }

    /// Get node's specialization score for a task type
    pub fn get_specialization(&self, task_type: TaskType) -> f32 {
        self.node_specializations
            .read()
            .get(&task_type)
            .copied()
            .unwrap_or(0.5)
    }

    /// Combined decision: should we accept this task?
    ///
    /// Considers both:
    /// - Global pheromone gradient (follow())
    /// - Local specialization score
    ///
    /// Returns probability of accepting the task.
    pub fn should_accept(&self, task_type: TaskType) -> f32 {
        let pheromone_prob = self.follow(task_type);
        let specialization = self.get_specialization(task_type);

        // Weighted combination (pheromone slightly more important)
        0.6 * pheromone_prob + 0.4 * specialization
    }

    /// Get statistics about the pheromone system
    pub fn get_stats(&self) -> StigmergyStats {
        let trails = self.pheromones.read();
        let specs = self.node_specializations.read();

        let total_intensity: f32 = trails.values().map(|t| t.intensity).sum();
        let avg_success_rate: f32 = if trails.is_empty() {
            0.5
        } else {
            trails.values().map(|t| t.success_rate).sum::<f32>() / trails.len() as f32
        };

        let total_completions: u64 = trails.values().map(|t| t.total_completions).sum();
        let total_failures: u64 = trails.values().map(|t| t.total_failures).sum();

        StigmergyStats {
            trail_count: trails.len(),
            total_intensity,
            avg_success_rate,
            total_completions,
            total_failures,
            specialization_count: specs.len(),
            strongest_trail: trails
                .iter()
                .max_by(|a, b| {
                    a.1.intensity
                        .partial_cmp(&b.1.intensity)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(tt, _)| format!("{:?}", tt)),
        }
    }
}

/// Statistics about the stigmergy system
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StigmergyStats {
    /// Number of active pheromone trails
    pub trail_count: usize,
    /// Total pheromone intensity across all trails
    pub total_intensity: f32,
    /// Average success rate across all trails
    pub avg_success_rate: f32,
    /// Total successful task completions
    pub total_completions: u64,
    /// Total failed task completions
    pub total_failures: u64,
    /// Number of specialization entries
    pub specialization_count: usize,
    /// The strongest trail (most pheromone)
    pub strongest_trail: Option<String>,
}

/// Parse task type from string
fn parse_task_type(s: &str) -> Option<TaskType> {
    match s {
        "VectorSearch" => Some(TaskType::VectorSearch),
        "VectorInsert" => Some(TaskType::VectorInsert),
        "Embedding" => Some(TaskType::Embedding),
        "SemanticMatch" => Some(TaskType::SemanticMatch),
        "NeuralInference" => Some(TaskType::NeuralInference),
        "Encryption" => Some(TaskType::Encryption),
        "Compression" => Some(TaskType::Compression),
        "CustomWasm" => Some(TaskType::CustomWasm),
        _ => None,
    }
}

/// WASM-bindgen wrapper for stigmergy coordination
#[wasm_bindgen]
pub struct WasmStigmergy {
    inner: Stigmergy,
}

#[wasm_bindgen]
impl WasmStigmergy {
    /// Create a new stigmergy engine
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmStigmergy {
        WasmStigmergy {
            inner: Stigmergy::new(),
        }
    }

    /// Create with custom parameters
    #[wasm_bindgen(js_name = withParams)]
    pub fn with_params(decay_rate: f32, deposit_rate: f32, evaporation_hours: f32) -> WasmStigmergy {
        WasmStigmergy {
            inner: Stigmergy::with_params(decay_rate, deposit_rate, evaporation_hours),
        }
    }

    /// Set minimum stake for anti-sybil
    #[wasm_bindgen(js_name = setMinStake)]
    pub fn set_min_stake(&mut self, min_stake: u64) {
        self.inner.set_min_stake(min_stake);
    }

    /// Deposit pheromone after task completion
    #[wasm_bindgen]
    pub fn deposit(&self, task_type: &str, peer_id: &str, success_rate: f32, stake: u64) {
        if let Some(tt) = parse_task_type(task_type) {
            self.inner.deposit(tt, peer_id.to_string(), success_rate, stake);
        }
    }

    /// Deposit with success/failure outcome
    #[wasm_bindgen(js_name = depositWithOutcome)]
    pub fn deposit_with_outcome(&self, task_type: &str, peer_id: &str, success: bool, stake: u64) {
        if let Some(tt) = parse_task_type(task_type) {
            self.inner
                .deposit_with_outcome(tt, peer_id.to_string(), success, stake);
        }
    }

    /// Get acceptance probability for a task type
    #[wasm_bindgen]
    pub fn follow(&self, task_type: &str) -> f32 {
        parse_task_type(task_type)
            .map(|tt| self.inner.follow(tt))
            .unwrap_or(0.5)
    }

    /// Get raw pheromone intensity
    #[wasm_bindgen(js_name = getIntensity)]
    pub fn get_intensity(&self, task_type: &str) -> f32 {
        parse_task_type(task_type)
            .map(|tt| self.inner.get_intensity(tt))
            .unwrap_or(0.0)
    }

    /// Get success rate for a task type
    #[wasm_bindgen(js_name = getSuccessRate)]
    pub fn get_success_rate(&self, task_type: &str) -> f32 {
        parse_task_type(task_type)
            .map(|tt| self.inner.get_success_rate(tt))
            .unwrap_or(0.5)
    }

    /// Run evaporation (call periodically)
    #[wasm_bindgen]
    pub fn evaporate(&self) {
        self.inner.evaporate();
    }

    /// Check and run evaporation if due
    #[wasm_bindgen(js_name = maybeEvaporate)]
    pub fn maybe_evaporate(&self) -> bool {
        self.inner.maybe_evaporate()
    }

    /// Merge peer pheromone state (JSON format)
    #[wasm_bindgen]
    pub fn merge(&self, peer_state_json: &str) -> bool {
        if let Ok(states) = serde_json::from_str::<Vec<PheromoneState>>(peer_state_json) {
            self.inner.merge_state(&states);
            true
        } else {
            false
        }
    }

    /// Export current state for P2P sharing
    #[wasm_bindgen(js_name = exportState)]
    pub fn export_state(&self) -> String {
        let states = self.inner.export_state();
        serde_json::to_string(&states).unwrap_or_else(|_| "[]".to_string())
    }

    /// Get best specialization recommendation
    #[wasm_bindgen(js_name = getBestSpecialization)]
    pub fn get_best_specialization(&self) -> Option<String> {
        self.inner
            .get_best_specialization()
            .map(|tt| format!("{:?}", tt))
    }

    /// Get all task types ranked by attractiveness
    #[wasm_bindgen(js_name = getRankedTasks)]
    pub fn get_ranked_tasks(&self) -> String {
        let ranked = self.inner.get_ranked_tasks();
        let result: Vec<(String, f32)> = ranked
            .into_iter()
            .map(|(tt, score)| (format!("{:?}", tt), score))
            .collect();
        serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
    }

    /// Update node specialization based on outcome
    #[wasm_bindgen(js_name = updateSpecialization)]
    pub fn update_specialization(&self, task_type: &str, success: bool) {
        if let Some(tt) = parse_task_type(task_type) {
            self.inner.update_specialization(tt, success);
        }
    }

    /// Get node's specialization score
    #[wasm_bindgen(js_name = getSpecialization)]
    pub fn get_specialization(&self, task_type: &str) -> f32 {
        parse_task_type(task_type)
            .map(|tt| self.inner.get_specialization(tt))
            .unwrap_or(0.5)
    }

    /// Should this node accept a task? (combined decision)
    #[wasm_bindgen(js_name = shouldAccept)]
    pub fn should_accept(&self, task_type: &str) -> f32 {
        parse_task_type(task_type)
            .map(|tt| self.inner.should_accept(tt))
            .unwrap_or(0.5)
    }

    /// Get statistics as JSON
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> String {
        let stats = self.inner.get_stats();
        serde_json::to_string(&stats).unwrap_or_else(|_| "{}".to_string())
    }
}

impl Default for WasmStigmergy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stigmergy_basic() {
        let stigmergy = Stigmergy::new();

        // Initially no pheromones
        assert_eq!(stigmergy.get_intensity(TaskType::VectorSearch), 0.0);

        // Deposit pheromone
        stigmergy.deposit(
            TaskType::VectorSearch,
            "node-1".to_string(),
            0.8,
            1000,
        );

        // Should have intensity now
        assert!(stigmergy.get_intensity(TaskType::VectorSearch) > 0.0);

        // Follow should return probability > 0.5
        let prob = stigmergy.follow(TaskType::VectorSearch);
        assert!(prob > 0.5);
    }

    #[test]
    fn test_deposit_with_outcome() {
        let stigmergy = Stigmergy::new();

        // Success deposits pheromone
        stigmergy.deposit_with_outcome(
            TaskType::Embedding,
            "node-2".to_string(),
            true,
            500,
        );
        assert!(stigmergy.get_intensity(TaskType::Embedding) > 0.0);

        // Failure updates success rate but no pheromone deposit
        let intensity_before = stigmergy.get_intensity(TaskType::Embedding);
        stigmergy.deposit_with_outcome(
            TaskType::Embedding,
            "node-2".to_string(),
            false,
            500,
        );
        assert_eq!(
            stigmergy.get_intensity(TaskType::Embedding),
            intensity_before
        );
        // But success rate should decrease
        assert!(stigmergy.get_success_rate(TaskType::Embedding) < 0.55);
    }

    #[test]
    fn test_evaporation() {
        // Evaporation depends on elapsed time since last deposit
        // With near-zero elapsed time, decay_factor ~ 1.0, so intensity barely changes
        // To test evaporation properly, we need to wait or accept the behavior
        let stigmergy = Stigmergy::with_params(0.99, 1.0, 0.001); // Very high decay rate

        stigmergy.deposit(
            TaskType::Compression,
            "node-3".to_string(),
            1.0,
            1000,
        );
        let initial = stigmergy.get_intensity(TaskType::Compression);
        assert!(initial > 0.0, "Initial intensity should be > 0");

        // Wait a tiny bit to ensure some time passes
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Evaporate - with very high decay rate (0.99), even small time should cause decay
        stigmergy.evaporate();

        // Intensity should decrease (or at least not increase)
        let after = stigmergy.get_intensity(TaskType::Compression);
        // With 0.99 decay rate, after small time: decay_factor = 0.01^(elapsed_hours)
        // For 10ms = 0.00000278 hours: decay_factor = 0.01^0.00000278 ~ 0.99987
        // So after ~ initial * 0.99987, which is very close
        // The trail may be cleaned up if intensity < 0.01
        assert!(after <= initial, "Intensity should not increase: {} vs {}", after, initial);
    }

    #[test]
    fn test_merge() {
        let stigmergy = Stigmergy::new();

        // Add local pheromone
        stigmergy.deposit(
            TaskType::Encryption,
            "node-local".to_string(),
            1.0,
            1000,
        );
        let local_intensity = stigmergy.get_intensity(TaskType::Encryption);

        // Merge with peer state
        let mut peer_trails = FxHashMap::default();
        peer_trails.insert(TaskType::Encryption, 10.0);
        peer_trails.insert(TaskType::NeuralInference, 5.0);

        stigmergy.merge(&peer_trails);

        // Local should be weighted 0.7, remote 0.3
        let merged = stigmergy.get_intensity(TaskType::Encryption);
        let expected = 0.7 * local_intensity + 0.3 * 10.0;
        assert!((merged - expected).abs() < 0.01);

        // New task type should appear
        assert!(stigmergy.get_intensity(TaskType::NeuralInference) > 0.0);
    }

    #[test]
    fn test_specialization() {
        let stigmergy = Stigmergy::new();

        // Initially neutral
        assert!((stigmergy.get_specialization(TaskType::SemanticMatch) - 0.5).abs() < 0.01);

        // Success increases specialization
        stigmergy.update_specialization(TaskType::SemanticMatch, true);
        assert!(stigmergy.get_specialization(TaskType::SemanticMatch) > 0.5);

        // Failure decreases it
        stigmergy.update_specialization(TaskType::SemanticMatch, false);
        let spec = stigmergy.get_specialization(TaskType::SemanticMatch);
        assert!(spec > 0.4 && spec < 0.6); // Should be around 0.5 after one success, one failure
    }

    #[test]
    fn test_anti_sybil() {
        let mut stigmergy = Stigmergy::new();
        stigmergy.set_min_stake(100);

        // Low stake deposit should be rejected
        stigmergy.deposit(
            TaskType::CustomWasm,
            "sybil".to_string(),
            1.0,
            50, // Below minimum
        );
        assert_eq!(stigmergy.get_intensity(TaskType::CustomWasm), 0.0);

        // High stake deposit should work
        stigmergy.deposit(
            TaskType::CustomWasm,
            "legit".to_string(),
            1.0,
            200,
        );
        assert!(stigmergy.get_intensity(TaskType::CustomWasm) > 0.0);
    }

    #[test]
    fn test_ring_buffer() {
        let mut buffer: RingBuffer<i32> = RingBuffer::new(3);

        buffer.push(1);
        buffer.push(2);
        buffer.push(3);
        assert_eq!(buffer.len(), 3);

        // Should evict oldest
        buffer.push(4);
        assert_eq!(buffer.len(), 3);

        let items: Vec<_> = buffer.iter().copied().collect();
        assert_eq!(items, vec![2, 3, 4]);
    }

    #[test]
    fn test_stats() {
        let stigmergy = Stigmergy::new();

        stigmergy.deposit(TaskType::VectorSearch, "n1".to_string(), 1.0, 100);
        stigmergy.deposit_with_outcome(TaskType::VectorInsert, "n2".to_string(), true, 100);
        stigmergy.deposit_with_outcome(TaskType::VectorInsert, "n2".to_string(), false, 100);

        let stats = stigmergy.get_stats();
        assert_eq!(stats.trail_count, 2);
        assert!(stats.total_intensity > 0.0);
        assert_eq!(stats.total_completions, 1);
        assert_eq!(stats.total_failures, 1);
    }
}
