//! Swarm Intelligence Module for Edge-Net
//!
//! Provides collective intelligence capabilities for the P2P AI network:
//!
//! - **Entropy-Based Consensus**: Negotiate decisions by minimizing belief entropy
//! - **Collective Memory**: Hippocampal-inspired pattern consolidation and sharing
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    Swarm Intelligence Layer                          │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────────────┐  ┌─────────────────────────────────────┐  │
//! │  │  Entropy Consensus  │  │        Collective Memory            │  │
//! │  │                     │  │                                     │  │
//! │  │  - Belief mixing    │  │  - Pattern sharing (RAC events)     │  │
//! │  │  - Shannon entropy  │  │  - Consolidation queue              │  │
//! │  │  - Convergence      │  │  - Hippocampal replay               │  │
//! │  │  - Annealing        │  │  - HNSW indexing                    │  │
//! │  └─────────────────────┘  └─────────────────────────────────────┘  │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────────────────────────────────────────────────────┐   │
//! │  │                     Integration Points                       │   │
//! │  │                                                              │   │
//! │  │  - RAC CoherenceEngine: Event logging, authority policies    │   │
//! │  │  - NetworkLearning: Pattern extraction, trajectories         │   │
//! │  │  - Network P2P: GUN.js/WebRTC message broadcast              │   │
//! │  └─────────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ### Entropy Consensus
//!
//! ```rust,ignore
//! use ruvector_edge_net::swarm::{EntropyConsensus, Decision};
//!
//! // Create consensus for task routing decision
//! let consensus = EntropyConsensus::with_threshold(0.1);
//!
//! // Add options with initial beliefs
//! consensus.set_belief(1, 0.6);  // Route to node 1
//! consensus.set_belief(2, 0.4);  // Route to node 2
//!
//! // Negotiate with peer beliefs
//! let peer_beliefs = peer.get_beliefs();
//! consensus.negotiate(&peer_beliefs);
//!
//! // Check for convergence
//! if consensus.converged() {
//!     let decision = consensus.get_decision().unwrap();
//!     println!("Consensus reached: route to node {}", decision);
//! }
//! ```
//!
//! ### Collective Memory
//!
//! ```rust,ignore
//! use ruvector_edge_net::swarm::{CollectiveMemory, Pattern, RacEvent};
//!
//! let memory = CollectiveMemory::new("node-1");
//!
//! // Share a learned pattern
//! let pattern = Pattern::new(
//!     "task-routing-v1".to_string(),
//!     vec![0.5, 0.3, 0.2],  // Embedding
//!     0.95,                  // Quality
//!     100,                   // Sample count
//!     "node-1".to_string(),
//! );
//! let rac_event = memory.share_pattern(&pattern);
//! swarm.publish(TOPIC_MODEL_SYNC, &serialize(&rac_event)?);
//!
//! // Receive pattern from peer
//! let peer_event = deserialize::<RacEvent>(&data)?;
//! if memory.receive_pattern(&peer_event) {
//!     println!("Pattern accepted for consolidation");
//! }
//!
//! // Consolidate during idle periods
//! let consolidated = memory.consolidate();
//! println!("Consolidated {} patterns", consolidated);
//! ```
//!
//! ## Integration with RAC
//!
//! The swarm module uses RAC (RuVector Adversarial Coherence) for:
//!
//! 1. **Pattern Assertions**: Shared patterns are RAC Assert events
//! 2. **Challenge/Support**: Disputed patterns can be challenged
//! 3. **Authority Policies**: Only trusted nodes can deprecate patterns
//! 4. **Audit Trail**: All pattern sharing is logged in Merkle tree
//!
//! ## References
//!
//! - DeGroot consensus model
//! - Complementary learning systems theory
//! - Federated learning pattern aggregation

pub mod consensus;
pub mod collective;
pub mod stigmergy;

// Re-export main types
pub use consensus::{
    EntropyConsensus,
    EntropyConsensusConfig,
    Decision,
    ConsensusPhase,
    ConsensusCoordinator,
};

pub use collective::{
    CollectiveMemory,
    CollectiveMemoryConfig,
    CollectiveStats,
    Pattern,
    HnswIndex,
    ClaimType,
    RacEvent,
    Swarm,
    TOPIC_MODEL_SYNC,
};

pub use stigmergy::{
    PeerId,
    PheromoneDeposit,
    PheromoneState,
    PheromoneTrail,
    RingBuffer,
    Stigmergy,
    StigmergyStats,
    WasmStigmergy,
};

use wasm_bindgen::prelude::*;
use rustc_hash::FxHashMap;

// ============================================================================
// Integrated Swarm Intelligence
// ============================================================================

/// Unified swarm intelligence coordinator
#[wasm_bindgen]
pub struct SwarmIntelligence {
    /// Entropy-based consensus engine
    consensus: EntropyConsensus,
    /// Collective memory for pattern sharing
    memory: CollectiveMemory,
    /// Local node ID
    node_id: String,
    /// Active consensus topics
    active_topics: std::sync::RwLock<FxHashMap<String, EntropyConsensus>>,
}

#[wasm_bindgen]
impl SwarmIntelligence {
    /// Create new swarm intelligence coordinator
    #[wasm_bindgen(constructor)]
    pub fn new(node_id: &str) -> Self {
        Self {
            consensus: EntropyConsensus::new(),
            memory: CollectiveMemory::new(node_id),
            node_id: node_id.to_string(),
            active_topics: std::sync::RwLock::new(FxHashMap::default()),
        }
    }

    /// Get node ID
    #[wasm_bindgen(js_name = nodeId)]
    pub fn node_id(&self) -> String {
        self.node_id.clone()
    }

    /// Start a new consensus round for a topic
    #[wasm_bindgen(js_name = startConsensus)]
    pub fn start_consensus(&self, topic: &str, threshold: f32) {
        let config = EntropyConsensusConfig {
            entropy_threshold: threshold.clamp(0.01, 2.0),
            ..Default::default()
        };
        let consensus = EntropyConsensus::with_config(config);
        self.active_topics.write().unwrap().insert(topic.to_string(), consensus);
    }

    /// Set belief for a topic's decision
    #[wasm_bindgen(js_name = setBelief)]
    pub fn set_belief(&self, topic: &str, decision_id: u64, probability: f32) {
        if let Some(consensus) = self.active_topics.write().unwrap().get(topic) {
            consensus.set_belief(decision_id, probability);
        }
    }

    /// Negotiate beliefs for a topic
    #[wasm_bindgen(js_name = negotiateBeliefs)]
    pub fn negotiate_beliefs(&self, topic: &str, beliefs_json: &str) -> bool {
        let beliefs: FxHashMap<u64, f32> = match serde_json::from_str(beliefs_json) {
            Ok(b) => b,
            Err(_) => return false,
        };

        if let Some(consensus) = self.active_topics.write().unwrap().get(topic) {
            consensus.negotiate(&beliefs);
            true
        } else {
            false
        }
    }

    /// Check if topic has reached consensus
    #[wasm_bindgen(js_name = hasConsensus)]
    pub fn has_consensus(&self, topic: &str) -> bool {
        self.active_topics.read().unwrap()
            .get(topic)
            .map(|c| c.converged())
            .unwrap_or(false)
    }

    /// Get consensus decision for topic
    #[wasm_bindgen(js_name = getConsensusDecision)]
    pub fn get_consensus_decision(&self, topic: &str) -> Option<u64> {
        self.active_topics.read().unwrap()
            .get(topic)
            .and_then(|c| c.get_decision())
    }

    /// Add pattern to collective memory
    #[wasm_bindgen(js_name = addPattern)]
    pub fn add_pattern(&self, pattern_json: &str) -> bool {
        let pattern: Pattern = match serde_json::from_str(pattern_json) {
            Ok(p) => p,
            Err(_) => return false,
        };
        self.memory.add_pattern(pattern)
    }

    /// Search collective memory
    #[wasm_bindgen(js_name = searchPatterns)]
    pub fn search_patterns(&self, query_json: &str, k: usize) -> String {
        self.memory.search(query_json, k)
    }

    /// Run memory consolidation
    #[wasm_bindgen]
    pub fn consolidate(&self) -> usize {
        self.memory.consolidate()
    }

    /// Run hippocampal replay
    #[wasm_bindgen]
    pub fn replay(&self) -> usize {
        self.memory.hippocampal_replay()
    }

    /// Get collective memory pattern count
    #[wasm_bindgen(js_name = patternCount)]
    pub fn pattern_count(&self) -> usize {
        self.memory.pattern_count()
    }

    /// Get queue size
    #[wasm_bindgen(js_name = queueSize)]
    pub fn queue_size(&self) -> usize {
        self.memory.queue_size()
    }

    /// Get combined statistics as JSON
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> String {
        let memory_stats = self.memory.get_stats();
        let active_topics = self.active_topics.read().unwrap().len();

        format!(
            r#"{{"node_id":"{}","active_topics":{},"memory":{}}}"#,
            self.node_id, active_topics, memory_stats
        )
    }
}

impl SwarmIntelligence {
    /// Get reference to memory
    pub fn memory(&self) -> &CollectiveMemory {
        &self.memory
    }

    /// Get consensus for a topic
    pub fn get_consensus(&self, topic: &str) -> Option<EntropyConsensus> {
        self.active_topics.read().unwrap()
            .get(topic)
            .map(|c| {
                // Create new consensus with same config
                let config = EntropyConsensusConfig {
                    entropy_threshold: c.get_entropy_threshold(),
                    ..Default::default()
                };
                EntropyConsensus::with_config(config)
            })
    }

    /// Set multiple beliefs for a topic at once (avoids intermediate normalization)
    pub fn set_beliefs(&self, topic: &str, beliefs: &[(u64, f32)]) {
        if let Some(consensus) = self.active_topics.write().unwrap().get(topic) {
            consensus.set_beliefs(beliefs);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swarm_intelligence_creation() {
        let swarm = SwarmIntelligence::new("node-1");
        assert_eq!(swarm.node_id(), "node-1");
        assert_eq!(swarm.pattern_count(), 0);
    }

    #[test]
    fn test_consensus_lifecycle() {
        let swarm = SwarmIntelligence::new("node-1");

        // Start consensus with a threshold that will allow convergence
        // Entropy of 0.95:0.05 distribution is ~0.286, so use threshold > 0.3
        swarm.start_consensus("task-routing", 0.5);

        // Set beliefs using set_beliefs to avoid intermediate normalization
        // Use very concentrated beliefs to ensure convergence
        swarm.set_beliefs("task-routing", &[(1, 0.95), (2, 0.05)]);

        // Check convergence (concentrated beliefs should converge)
        assert!(swarm.has_consensus("task-routing"), "Should have consensus for task-routing");
        assert_eq!(swarm.get_consensus_decision("task-routing"), Some(1));
    }

    #[test]
    fn test_pattern_lifecycle() {
        let swarm = SwarmIntelligence::new("node-1");

        // Add pattern
        let pattern_json = r#"{
            "id": "test-pattern",
            "embedding": [1.0, 2.0, 3.0],
            "quality": 0.9,
            "samples": 100,
            "evidence": [],
            "source_node": "node-1",
            "created_at": 0,
            "optimal_allocation": 0.5,
            "optimal_energy": 100,
            "task_type": null
        }"#;

        assert!(swarm.add_pattern(pattern_json));
        assert_eq!(swarm.queue_size(), 1);

        // Consolidate
        let consolidated = swarm.consolidate();
        assert!(consolidated > 0 || swarm.pattern_count() > 0 || swarm.queue_size() == 0);
    }

    #[test]
    fn test_stats() {
        let swarm = SwarmIntelligence::new("test-node");
        swarm.start_consensus("topic-1", 0.1);

        let stats = swarm.get_stats();
        assert!(stats.contains("test-node"));
        assert!(stats.contains("active_topics"));
        assert!(stats.contains("memory"));
    }
}
