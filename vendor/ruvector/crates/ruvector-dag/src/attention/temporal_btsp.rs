//! Temporal BTSP Attention: Behavioral Timescale Synaptic Plasticity
//!
//! This mechanism implements a biologically-inspired attention mechanism based on
//! eligibility traces and plateau potentials, allowing the system to learn from
//! temporal patterns in query execution.

use super::trait_def::{AttentionError, AttentionScores, DagAttentionMechanism};
use crate::dag::QueryDag;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct TemporalBTSPConfig {
    /// Duration of plateau state in milliseconds
    pub plateau_duration_ms: u64,
    /// Decay rate for eligibility traces (0.0 to 1.0)
    pub eligibility_decay: f32,
    /// Learning rate for trace updates
    pub learning_rate: f32,
    /// Temperature for softmax
    pub temperature: f32,
    /// Baseline attention for nodes without history
    pub baseline_attention: f32,
}

impl Default for TemporalBTSPConfig {
    fn default() -> Self {
        Self {
            plateau_duration_ms: 500,
            eligibility_decay: 0.95,
            learning_rate: 0.1,
            temperature: 0.1,
            baseline_attention: 0.5,
        }
    }
}

pub struct TemporalBTSPAttention {
    config: TemporalBTSPConfig,
    /// Eligibility traces for each node
    eligibility_traces: HashMap<usize, f32>,
    /// Timestamp of last plateau for each node
    last_plateau: HashMap<usize, Instant>,
    /// Total updates counter
    update_count: usize,
}

impl TemporalBTSPAttention {
    pub fn new(config: TemporalBTSPConfig) -> Self {
        Self {
            config,
            eligibility_traces: HashMap::new(),
            last_plateau: HashMap::new(),
            update_count: 0,
        }
    }

    /// Update eligibility trace for a node
    fn update_eligibility(&mut self, node_id: usize, signal: f32) {
        let trace = self.eligibility_traces.entry(node_id).or_insert(0.0);
        *trace = *trace * self.config.eligibility_decay + signal * self.config.learning_rate;

        // Clamp to [0, 1]
        *trace = trace.max(0.0).min(1.0);
    }

    /// Check if node is in plateau state
    fn is_plateau(&self, node_id: usize) -> bool {
        self.last_plateau
            .get(&node_id)
            .map(|t| t.elapsed().as_millis() < self.config.plateau_duration_ms as u128)
            .unwrap_or(false)
    }

    /// Trigger plateau for a node
    fn trigger_plateau(&mut self, node_id: usize) {
        self.last_plateau.insert(node_id, Instant::now());
    }

    /// Compute base attention from topology
    fn compute_topology_attention(&self, dag: &QueryDag) -> Vec<f32> {
        let n = dag.node_count();
        let mut scores = vec![self.config.baseline_attention; n];

        // Simple heuristic: nodes with higher cost get more attention
        for node in dag.nodes() {
            if node.id < n {
                let cost_factor = (node.estimated_cost as f32 / 100.0).min(1.0);
                let rows_factor = (node.estimated_rows as f32 / 1000.0).min(1.0);
                scores[node.id] = 0.5 * cost_factor + 0.5 * rows_factor;
            }
        }

        scores
    }

    /// Apply eligibility trace modulation
    fn apply_eligibility_modulation(&self, base_scores: &mut [f32]) {
        for (node_id, &trace) in &self.eligibility_traces {
            if *node_id < base_scores.len() {
                // Boost attention based on eligibility trace
                base_scores[*node_id] *= 1.0 + trace;
            }
        }
    }

    /// Apply plateau boosting
    fn apply_plateau_boost(&self, scores: &mut [f32]) {
        for (node_id, _) in &self.last_plateau {
            if *node_id < scores.len() && self.is_plateau(*node_id) {
                // Strong boost for nodes in plateau state
                scores[*node_id] *= 1.5;
            }
        }
    }

    /// Normalize scores using softmax
    fn normalize_scores(&self, scores: &mut [f32]) {
        if scores.is_empty() {
            return;
        }

        let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_sum: f32 = scores
            .iter()
            .map(|&s| ((s - max_score) / self.config.temperature).exp())
            .sum();

        if exp_sum > 0.0 {
            for score in scores.iter_mut() {
                *score = ((*score - max_score) / self.config.temperature).exp() / exp_sum;
            }
        } else {
            let uniform = 1.0 / scores.len() as f32;
            scores.fill(uniform);
        }
    }
}

impl DagAttentionMechanism for TemporalBTSPAttention {
    fn forward(&self, dag: &QueryDag) -> Result<AttentionScores, AttentionError> {
        if dag.nodes.is_empty() {
            return Err(AttentionError::InvalidDag("Empty DAG".to_string()));
        }

        // Step 1: Compute base attention from topology
        let mut scores = self.compute_topology_attention(dag);

        // Step 2: Modulate by eligibility traces
        self.apply_eligibility_modulation(&mut scores);

        // Step 3: Apply plateau boosting for recently active nodes
        self.apply_plateau_boost(&mut scores);

        // Step 4: Normalize
        self.normalize_scores(&mut scores);

        // Build result with metadata
        let mut result = AttentionScores::new(scores)
            .with_metadata("mechanism".to_string(), "temporal_btsp".to_string())
            .with_metadata("update_count".to_string(), self.update_count.to_string());

        let active_traces = self
            .eligibility_traces
            .values()
            .filter(|&&t| t > 0.01)
            .count();
        result
            .metadata
            .insert("active_traces".to_string(), active_traces.to_string());

        let active_plateaus = self
            .last_plateau
            .keys()
            .filter(|k| self.is_plateau(**k))
            .count();
        result
            .metadata
            .insert("active_plateaus".to_string(), active_plateaus.to_string());

        Ok(result)
    }

    fn name(&self) -> &'static str {
        "temporal_btsp"
    }

    fn complexity(&self) -> &'static str {
        "O(n + t)"
    }

    fn update(&mut self, dag: &QueryDag, execution_times: &HashMap<usize, f64>) {
        self.update_count += 1;

        // Update eligibility traces based on execution feedback
        for (node_id, &exec_time) in execution_times {
            let node = match dag.get_node(*node_id) {
                Some(n) => n,
                None => continue,
            };

            let expected_time = node.estimated_cost;

            // Compute reward signal: positive if faster than expected, negative if slower
            let time_ratio = exec_time / expected_time.max(0.001);
            let reward = if time_ratio < 1.0 {
                // Faster than expected - positive signal
                1.0 - time_ratio as f32
            } else {
                // Slower than expected - negative signal
                -(time_ratio as f32 - 1.0).min(1.0)
            };

            // Update eligibility trace
            self.update_eligibility(*node_id, reward);

            // Trigger plateau for nodes that significantly exceeded expectations
            if reward > 0.3 {
                self.trigger_plateau(*node_id);
            }
        }

        // Decay traces for nodes that weren't executed
        let executed_nodes: std::collections::HashSet<_> = execution_times.keys().collect();
        for node_id in 0..dag.node_count() {
            if !executed_nodes.contains(&node_id) {
                self.update_eligibility(node_id, 0.0);
            }
        }
    }

    fn reset(&mut self) {
        self.eligibility_traces.clear();
        self.last_plateau.clear();
        self.update_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::{OperatorNode, OperatorType};
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_eligibility_update() {
        let config = TemporalBTSPConfig::default();
        let mut attention = TemporalBTSPAttention::new(config);

        attention.update_eligibility(0, 0.5);
        assert!(attention.eligibility_traces.get(&0).unwrap() > &0.0);

        attention.update_eligibility(0, 0.5);
        assert!(attention.eligibility_traces.get(&0).unwrap() > &0.0);
    }

    #[test]
    fn test_plateau_state() {
        let mut config = TemporalBTSPConfig::default();
        config.plateau_duration_ms = 100;
        let mut attention = TemporalBTSPAttention::new(config);

        attention.trigger_plateau(0);
        assert!(attention.is_plateau(0));

        sleep(Duration::from_millis(150));
        assert!(!attention.is_plateau(0));
    }

    #[test]
    fn test_temporal_attention() {
        let config = TemporalBTSPConfig::default();
        let mut attention = TemporalBTSPAttention::new(config);

        let mut dag = QueryDag::new();
        for i in 0..3 {
            let mut node = OperatorNode::new(i, OperatorType::Scan);
            node.estimated_cost = 10.0;
            dag.add_node(node);
        }

        // Initial forward pass
        let result1 = attention.forward(&dag).unwrap();
        assert_eq!(result1.scores.len(), 3);

        // Simulate execution feedback
        let mut exec_times = HashMap::new();
        exec_times.insert(0, 5.0); // Faster than expected
        exec_times.insert(1, 15.0); // Slower than expected

        attention.update(&dag, &exec_times);

        // Second forward pass should show different attention
        let result2 = attention.forward(&dag).unwrap();
        assert_eq!(result2.scores.len(), 3);

        // Node 0 should have higher attention due to positive feedback
        assert!(attention.eligibility_traces.get(&0).unwrap() > &0.0);
    }
}
