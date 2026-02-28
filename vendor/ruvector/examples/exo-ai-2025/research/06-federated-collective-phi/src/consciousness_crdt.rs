// consciousness_crdt.rs
// Conflict-Free Replicated Data Type for Consciousness State
// Implements OR-Set, LWW-Register, and custom Phenomenal CRDTs

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

/// Agent identifier
pub type AgentId = u64;

/// Timestamp for causality tracking
pub type Timestamp = u64;

/// Represents a quale (unit of phenomenal experience)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Quale {
    /// Sensory modality (vision, audio, proprioception, etc.)
    pub modality: String,
    /// Phenomenal content (e.g., "red", "middle-C", "warm")
    pub content: String,
    /// Intensity (0.0 to 1.0)
    pub intensity: u8, // 0-255 for efficiency
}

impl Quale {
    pub fn new(modality: String, content: String, intensity: f64) -> Self {
        Self {
            modality,
            content,
            intensity: (intensity.clamp(0.0, 1.0) * 255.0) as u8,
        }
    }

    pub fn intensity_f64(&self) -> f64 {
        self.intensity as f64 / 255.0
    }
}

/// G-Counter (Grow-only Counter) for Φ values
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhiCounter {
    /// Per-agent Φ values
    counts: HashMap<AgentId, f64>,
}

impl PhiCounter {
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    /// Increment local Φ value
    pub fn increment(&mut self, agent_id: AgentId, delta: f64) {
        *self.counts.entry(agent_id).or_insert(0.0) += delta;
    }

    /// Set local Φ value (must be monotonically increasing)
    pub fn set(&mut self, agent_id: AgentId, value: f64) {
        let current = self.counts.get(&agent_id).copied().unwrap_or(0.0);
        if value > current {
            self.counts.insert(agent_id, value);
        }
    }

    /// Merge with another PhiCounter (CRDT merge)
    pub fn merge(&mut self, other: &PhiCounter) {
        for (&agent_id, &value) in &other.counts {
            let current = self.counts.get(&agent_id).copied().unwrap_or(0.0);
            self.counts.insert(agent_id, current.max(value));
        }
    }

    /// Get total Φ across all agents
    pub fn total(&self) -> f64 {
        self.counts.values().sum()
    }

    /// Get Φ for specific agent
    pub fn get(&self, agent_id: AgentId) -> f64 {
        self.counts.get(&agent_id).copied().unwrap_or(0.0)
    }
}

impl Default for PhiCounter {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique identifier for OR-Set elements
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ElementId {
    agent_id: AgentId,
    timestamp: Timestamp,
}

/// OR-Set (Observed-Remove Set) for qualia
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualiaSet {
    /// Map from quale to set of element IDs
    elements: HashMap<Quale, HashSet<ElementId>>,
}

impl QualiaSet {
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
        }
    }

    /// Add a quale (with unique element ID)
    pub fn add(&mut self, quale: Quale, agent_id: AgentId, timestamp: Timestamp) {
        let elem_id = ElementId {
            agent_id,
            timestamp,
        };
        self.elements
            .entry(quale)
            .or_insert_with(HashSet::new)
            .insert(elem_id);
    }

    /// Remove a quale (marks for removal, actual removal on merge)
    pub fn remove(&mut self, quale: &Quale) {
        self.elements.remove(quale);
    }

    /// Merge with another QualiaSet (CRDT merge)
    pub fn merge(&mut self, other: &QualiaSet) {
        for (quale, elem_ids) in &other.elements {
            self.elements
                .entry(quale.clone())
                .or_insert_with(HashSet::new)
                .extend(elem_ids.iter().cloned());
        }
    }

    /// Get all current qualia
    pub fn qualia(&self) -> Vec<Quale> {
        self.elements.keys().cloned().collect()
    }

    /// Check if quale is present
    pub fn contains(&self, quale: &Quale) -> bool {
        self.elements.contains_key(quale)
    }

    /// Number of distinct qualia
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

impl Default for QualiaSet {
    fn default() -> Self {
        Self::new()
    }
}

/// LWW-Register (Last-Write-Wins Register) for attention focus
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttentionRegister {
    /// Current focus
    focus: Option<Quale>,
    /// Timestamp of current value
    timestamp: Timestamp,
    /// Agent who set the value
    agent_id: AgentId,
}

impl AttentionRegister {
    pub fn new() -> Self {
        Self {
            focus: None,
            timestamp: 0,
            agent_id: 0,
        }
    }

    /// Set attention focus
    pub fn set(&mut self, focus: Quale, agent_id: AgentId, timestamp: Timestamp) {
        if timestamp > self.timestamp {
            self.focus = Some(focus);
            self.timestamp = timestamp;
            self.agent_id = agent_id;
        }
    }

    /// Merge with another register (CRDT merge - LWW)
    pub fn merge(&mut self, other: &AttentionRegister) {
        match self.timestamp.cmp(&other.timestamp) {
            Ordering::Less => {
                self.focus = other.focus.clone();
                self.timestamp = other.timestamp;
                self.agent_id = other.agent_id;
            }
            Ordering::Equal => {
                // Tie-break by agent ID
                if other.agent_id > self.agent_id {
                    self.focus = other.focus.clone();
                    self.agent_id = other.agent_id;
                }
            }
            Ordering::Greater => {
                // Keep current value
            }
        }
    }

    /// Get current focus
    pub fn get(&self) -> Option<&Quale> {
        self.focus.as_ref()
    }
}

impl Default for AttentionRegister {
    fn default() -> Self {
        Self::new()
    }
}

/// Vector clock for causal ordering
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VectorClock {
    clocks: HashMap<AgentId, Timestamp>,
}

impl std::hash::Hash for VectorClock {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Sort keys for deterministic hashing
        let mut sorted: Vec<_> = self.clocks.iter().collect();
        sorted.sort_by_key(|(k, _)| *k);
        for (k, v) in sorted {
            k.hash(state);
            v.hash(state);
        }
    }
}

impl VectorClock {
    pub fn new() -> Self {
        Self {
            clocks: HashMap::new(),
        }
    }

    /// Increment local clock
    pub fn increment(&mut self, agent_id: AgentId) {
        *self.clocks.entry(agent_id).or_insert(0) += 1;
    }

    /// Merge with another vector clock
    pub fn merge(&mut self, other: &VectorClock) {
        for (&agent_id, &timestamp) in &other.clocks {
            let current = self.clocks.get(&agent_id).copied().unwrap_or(0);
            self.clocks.insert(agent_id, current.max(timestamp));
        }
    }

    /// Check if this clock happened before other
    pub fn happens_before(&self, other: &VectorClock) -> bool {
        let mut strictly_less = false;
        let mut all_less_or_equal = true;

        // Check all agents in self
        for (&agent_id, &self_time) in &self.clocks {
            let other_time = other.clocks.get(&agent_id).copied().unwrap_or(0);
            if self_time > other_time {
                all_less_or_equal = false;
            }
            if self_time < other_time {
                strictly_less = true;
            }
        }

        // Check all agents in other
        for (&agent_id, &other_time) in &other.clocks {
            let self_time = self.clocks.get(&agent_id).copied().unwrap_or(0);
            if self_time > other_time {
                return false; // Not happened before
            }
            if self_time < other_time {
                strictly_less = true;
            }
        }

        all_less_or_equal && strictly_less
    }

    /// Check if concurrent (neither happens before the other)
    pub fn concurrent(&self, other: &VectorClock) -> bool {
        !self.happens_before(other) && !other.happens_before(self)
    }
}

impl Default for VectorClock {
    fn default() -> Self {
        Self::new()
    }
}

/// Multi-Value Register for working memory
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkingMemory {
    /// Map from vector clock to qualia
    values: HashMap<VectorClock, HashSet<Quale>>,
}

impl WorkingMemory {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Add qualia at current vector clock
    pub fn add(&mut self, qualia: HashSet<Quale>, clock: VectorClock) {
        self.values.insert(clock, qualia);
    }

    /// Merge with another working memory
    pub fn merge(&mut self, other: &WorkingMemory) {
        for (clock, qualia) in &other.values {
            self.values.insert(clock.clone(), qualia.clone());
        }

        // Remove causally dominated values
        self.remove_dominated();
    }

    /// Remove values that are causally dominated
    fn remove_dominated(&mut self) {
        let clocks: Vec<VectorClock> = self.values.keys().cloned().collect();

        let mut to_remove = Vec::new();

        for i in 0..clocks.len() {
            for j in 0..clocks.len() {
                if i != j && clocks[i].happens_before(&clocks[j]) {
                    to_remove.push(clocks[i].clone());
                    break;
                }
            }
        }

        for clock in to_remove {
            self.values.remove(&clock);
        }
    }

    /// Get all concurrent qualia (maximal values)
    pub fn get_concurrent(&self) -> Vec<HashSet<Quale>> {
        self.values.values().cloned().collect()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl Default for WorkingMemory {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete Consciousness State as Phenomenal CRDT
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsciousnessState {
    /// Integrated information level (G-Counter)
    pub phi_value: PhiCounter,

    /// Phenomenal content (OR-Set)
    pub qualia_content: QualiaSet,

    /// Current attentional focus (LWW-Register)
    pub attention_focus: AttentionRegister,

    /// Working memory (Multi-Value Register)
    pub working_memory: WorkingMemory,

    /// Agent ID
    pub agent_id: AgentId,

    /// Local timestamp
    pub timestamp: Timestamp,
}

impl ConsciousnessState {
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            phi_value: PhiCounter::new(),
            qualia_content: QualiaSet::new(),
            attention_focus: AttentionRegister::new(),
            working_memory: WorkingMemory::new(),
            agent_id,
            timestamp: 0,
        }
    }

    /// Update Φ value
    pub fn update_phi(&mut self, phi: f64) {
        self.phi_value.set(self.agent_id, phi);
        self.timestamp += 1;
    }

    /// Add quale to phenomenal content
    pub fn add_quale(&mut self, quale: Quale) {
        self.qualia_content
            .add(quale, self.agent_id, self.timestamp);
        self.timestamp += 1;
    }

    /// Set attention focus
    pub fn set_attention(&mut self, quale: Quale) {
        self.attention_focus
            .set(quale, self.agent_id, self.timestamp);
        self.timestamp += 1;
    }

    /// Add to working memory
    pub fn add_to_working_memory(&mut self, qualia: HashSet<Quale>) {
        let mut clock = VectorClock::new();
        clock.increment(self.agent_id);
        self.working_memory.add(qualia, clock);
        self.timestamp += 1;
    }

    /// Merge with another consciousness state (CRDT merge operation)
    pub fn merge(&mut self, other: &ConsciousnessState) {
        self.phi_value.merge(&other.phi_value);
        self.qualia_content.merge(&other.qualia_content);
        self.attention_focus.merge(&other.attention_focus);
        self.working_memory.merge(&other.working_memory);
    }

    /// Get total collective Φ
    pub fn total_phi(&self) -> f64 {
        self.phi_value.total()
    }

    /// Get number of distinct qualia
    pub fn qualia_count(&self) -> usize {
        self.qualia_content.len()
    }

    /// Check if consciousness is active (Φ > 0)
    pub fn is_conscious(&self) -> bool {
        self.total_phi() > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phi_counter_merge() {
        let mut counter1 = PhiCounter::new();
        counter1.set(1, 8.2);
        counter1.set(2, 7.9);

        let mut counter2 = PhiCounter::new();
        counter2.set(2, 8.1); // Higher value for agent 2
        counter2.set(3, 7.5);

        counter1.merge(&counter2);

        assert_eq!(counter1.get(1), 8.2);
        assert_eq!(counter1.get(2), 8.1); // Should take max
        assert_eq!(counter1.get(3), 7.5);
        assert_eq!(counter1.total(), 8.2 + 8.1 + 7.5);
    }

    #[test]
    fn test_qualia_set_merge() {
        let mut set1 = QualiaSet::new();
        let quale1 = Quale::new("vision".to_string(), "red".to_string(), 0.8);
        set1.add(quale1.clone(), 1, 100);

        let mut set2 = QualiaSet::new();
        let quale2 = Quale::new("vision".to_string(), "blue".to_string(), 0.6);
        set2.add(quale2.clone(), 2, 101);

        set1.merge(&set2);

        assert!(set1.contains(&quale1));
        assert!(set1.contains(&quale2));
        assert_eq!(set1.len(), 2);
    }

    #[test]
    fn test_attention_register_lww() {
        let mut reg1 = AttentionRegister::new();
        let focus1 = Quale::new("vision".to_string(), "red apple".to_string(), 1.0);
        reg1.set(focus1.clone(), 1, 100);

        let mut reg2 = AttentionRegister::new();
        let focus2 = Quale::new("vision".to_string(), "blue sky".to_string(), 0.9);
        reg2.set(focus2.clone(), 2, 101); // Later timestamp

        reg1.merge(&reg2);

        assert_eq!(reg1.get(), Some(&focus2)); // Should pick later write
    }

    #[test]
    fn test_vector_clock_causality() {
        let mut clock1 = VectorClock::new();
        clock1.increment(1);
        clock1.increment(1);

        let mut clock2 = VectorClock::new();
        clock2.increment(1);
        clock2.increment(1);
        clock2.increment(1);

        assert!(clock1.happens_before(&clock2));
        assert!(!clock2.happens_before(&clock1));

        let mut clock3 = VectorClock::new();
        clock3.increment(2);

        assert!(clock1.concurrent(&clock3));
    }

    #[test]
    fn test_consciousness_state_merge() {
        let mut state1 = ConsciousnessState::new(1);
        state1.update_phi(8.2);
        state1.add_quale(Quale::new("vision".to_string(), "red".to_string(), 0.8));

        let mut state2 = ConsciousnessState::new(2);
        state2.update_phi(7.9);
        state2.add_quale(Quale::new("audio".to_string(), "C note".to_string(), 0.6));

        state1.merge(&state2);

        assert_eq!(state1.total_phi(), 8.2 + 7.9);
        assert_eq!(state1.qualia_count(), 2);
        assert!(state1.is_conscious());
    }

    #[test]
    fn test_working_memory_concurrent() {
        let mut wm = WorkingMemory::new();

        let mut clock1 = VectorClock::new();
        clock1.increment(1);

        let mut qualia1 = HashSet::new();
        qualia1.insert(Quale::new("vision".to_string(), "red".to_string(), 0.8));

        wm.add(qualia1, clock1);

        let mut clock2 = VectorClock::new();
        clock2.increment(2);

        let mut qualia2 = HashSet::new();
        qualia2.insert(Quale::new("audio".to_string(), "beep".to_string(), 0.5));

        wm.add(qualia2, clock2);

        let concurrent = wm.get_concurrent();
        assert_eq!(concurrent.len(), 2); // Both are concurrent (maximal)
    }
}
