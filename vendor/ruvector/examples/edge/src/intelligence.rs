//! Distributed intelligence synchronization
//!
//! Sync Q-learning patterns, trajectories, and learning state across swarm agents.

use crate::{Result, SwarmError, compression::TensorCodec};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

/// Learning pattern with Q-value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub state: String,
    pub action: String,
    pub q_value: f64,
    pub visits: u64,
    pub last_update: u64,
    pub confidence: f64,
}

impl Pattern {
    pub fn new(state: &str, action: &str) -> Self {
        Self {
            state: state.to_string(),
            action: action.to_string(),
            q_value: 0.0,
            visits: 0,
            last_update: 0,
            confidence: 0.0,
        }
    }

    /// Merge with another pattern (federated learning style)
    pub fn merge(&mut self, other: &Pattern, weight: f64) {
        let total_visits = self.visits + other.visits;
        if total_visits > 0 {
            // Weighted average based on visits
            let self_weight = self.visits as f64 / total_visits as f64;
            let other_weight = other.visits as f64 / total_visits as f64;

            self.q_value = self.q_value * self_weight + other.q_value * other_weight * weight;
            self.visits = total_visits;
            self.confidence = (self.confidence + other.confidence * weight) / 2.0;
        }
    }
}

/// Learning trajectory for decision transformer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    pub id: String,
    pub steps: Vec<TrajectoryStep>,
    pub total_reward: f64,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryStep {
    pub state: String,
    pub action: String,
    pub reward: f64,
    pub timestamp: u64,
}

/// Complete learning state for sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningState {
    pub agent_id: String,
    pub patterns: HashMap<String, Pattern>,
    pub trajectories: Vec<Trajectory>,
    pub algorithm_stats: HashMap<String, AlgorithmStats>,
    pub version: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgorithmStats {
    pub algorithm: String,
    pub updates: u64,
    pub avg_reward: f64,
    pub convergence: f64,
}

impl Default for LearningState {
    fn default() -> Self {
        Self {
            agent_id: String::new(),
            patterns: HashMap::new(),
            trajectories: Vec::new(),
            algorithm_stats: HashMap::new(),
            version: 0,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        }
    }
}

/// Intelligence synchronization manager
pub struct IntelligenceSync {
    local_state: Arc<RwLock<LearningState>>,
    peer_states: Arc<RwLock<HashMap<String, LearningState>>>,
    codec: TensorCodec,
    merge_threshold: f64,
}

impl IntelligenceSync {
    /// Create new intelligence sync manager
    pub fn new(agent_id: &str) -> Self {
        let mut state = LearningState::default();
        state.agent_id = agent_id.to_string();

        Self {
            local_state: Arc::new(RwLock::new(state)),
            peer_states: Arc::new(RwLock::new(HashMap::new())),
            codec: TensorCodec::new(),
            merge_threshold: 0.1, // Only merge if delta > 10%
        }
    }

    /// Get local learning state
    pub fn get_state(&self) -> LearningState {
        self.local_state.read().clone()
    }

    /// Update local pattern
    pub fn update_pattern(&self, state: &str, action: &str, reward: f64) {
        let mut local = self.local_state.write();
        let key = format!("{}|{}", state, action);

        let pattern = local.patterns.entry(key).or_insert_with(|| Pattern::new(state, action));

        // Q-learning update
        let alpha = 0.1;
        pattern.q_value = pattern.q_value + alpha * (reward - pattern.q_value);
        pattern.visits += 1;
        pattern.last_update = chrono::Utc::now().timestamp_millis() as u64;
        pattern.confidence = 1.0 - (1.0 / (pattern.visits as f64 + 1.0));

        local.version += 1;
    }

    /// Serialize state for network transfer
    pub fn serialize_state(&self) -> Result<Vec<u8>> {
        let state = self.local_state.read();
        let json = serde_json::to_vec(&*state)
            .map_err(|e| SwarmError::Serialization(e.to_string()))?;

        // Compress for transfer
        self.codec.compress(&json)
    }

    /// Deserialize and merge peer state
    pub fn merge_peer_state(&self, peer_id: &str, data: &[u8]) -> Result<MergeResult> {
        // Decompress
        let json = self.codec.decompress(data)?;
        let peer_state: LearningState = serde_json::from_slice(&json)
            .map_err(|e| SwarmError::Serialization(e.to_string()))?;

        // Store peer state
        {
            let mut peers = self.peer_states.write();
            peers.insert(peer_id.to_string(), peer_state.clone());
        }

        // Merge patterns
        let mut local = self.local_state.write();
        let mut merged_count = 0;
        let mut new_count = 0;

        for (key, peer_pattern) in &peer_state.patterns {
            if let Some(local_pattern) = local.patterns.get_mut(key) {
                // Merge existing pattern
                let delta = (peer_pattern.q_value - local_pattern.q_value).abs();
                if delta > self.merge_threshold {
                    local_pattern.merge(peer_pattern, 0.5);
                    merged_count += 1;
                }
            } else {
                // New pattern from peer
                local.patterns.insert(key.clone(), peer_pattern.clone());
                new_count += 1;
            }
        }

        local.version += 1;

        Ok(MergeResult {
            peer_id: peer_id.to_string(),
            merged_patterns: merged_count,
            new_patterns: new_count,
            local_version: local.version,
        })
    }

    /// Get best action for state using aggregated knowledge
    pub fn get_best_action(&self, state: &str, actions: &[String]) -> Option<(String, f64)> {
        let local = self.local_state.read();

        let mut best_action = None;
        let mut best_q = f64::NEG_INFINITY;

        for action in actions {
            let key = format!("{}|{}", state, action);
            if let Some(pattern) = local.patterns.get(&key) {
                if pattern.q_value > best_q {
                    best_q = pattern.q_value;
                    best_action = Some((action.clone(), pattern.confidence));
                }
            }
        }

        best_action
    }

    /// Get sync delta (only changed patterns since version)
    pub fn get_delta(&self, since_version: u64) -> LearningState {
        let local = self.local_state.read();

        let mut delta = LearningState {
            agent_id: local.agent_id.clone(),
            version: local.version,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            ..Default::default()
        };

        // Only include patterns updated since version
        for (key, pattern) in &local.patterns {
            if pattern.last_update > since_version {
                delta.patterns.insert(key.clone(), pattern.clone());
            }
        }

        delta
    }

    /// Get aggregated stats across all peers
    pub fn get_swarm_stats(&self) -> SwarmStats {
        let local = self.local_state.read();
        let peers = self.peer_states.read();

        let mut total_patterns = local.patterns.len();
        let mut total_visits = 0u64;
        let mut avg_confidence = 0.0;

        for pattern in local.patterns.values() {
            total_visits += pattern.visits;
            avg_confidence += pattern.confidence;
        }

        for peer in peers.values() {
            total_patterns += peer.patterns.len();
        }

        let pattern_count = local.patterns.len();
        if pattern_count > 0 {
            avg_confidence /= pattern_count as f64;
        }

        SwarmStats {
            total_agents: peers.len() + 1,
            total_patterns,
            total_visits,
            avg_confidence,
            local_version: local.version,
        }
    }
}

/// Result of merging peer state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    pub peer_id: String,
    pub merged_patterns: usize,
    pub new_patterns: usize,
    pub local_version: u64,
}

/// Aggregated swarm statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmStats {
    pub total_agents: usize,
    pub total_patterns: usize,
    pub total_visits: u64,
    pub avg_confidence: f64,
    pub local_version: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_update() {
        let sync = IntelligenceSync::new("test-agent");

        sync.update_pattern("edit_ts", "coder", 0.8);
        sync.update_pattern("edit_ts", "coder", 0.9);

        let state = sync.get_state();
        let pattern = state.patterns.get("edit_ts|coder").unwrap();

        assert!(pattern.q_value > 0.0);
        assert_eq!(pattern.visits, 2);
    }

    #[test]
    fn test_best_action() {
        let sync = IntelligenceSync::new("test-agent");

        sync.update_pattern("edit_ts", "coder", 0.5);
        sync.update_pattern("edit_ts", "reviewer", 0.9);

        let actions = vec!["coder".to_string(), "reviewer".to_string()];
        let best = sync.get_best_action("edit_ts", &actions);

        assert!(best.is_some());
        assert_eq!(best.unwrap().0, "reviewer");
    }
}
