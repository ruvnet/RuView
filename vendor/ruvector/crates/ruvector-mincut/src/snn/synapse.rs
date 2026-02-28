//! # Synaptic Connections with STDP Learning
//!
//! Implements spike-timing dependent plasticity (STDP) for synaptic weight updates.
//!
//! ## STDP Learning Rule
//!
//! ```text
//! ΔW = A+ * exp(-Δt/τ+)  if Δt > 0 (pre before post → LTP)
//! ΔW = A- * exp(Δt/τ-)   if Δt < 0 (post before pre → LTD)
//! ```
//!
//! Where Δt = t_post - t_pre
//!
//! ## Integration with MinCut
//!
//! Synaptic weights directly map to graph edge weights:
//! - Strong synapse → strong edge → less likely in mincut
//! - STDP learning → edge weight evolution → dynamic mincut

use super::{SimTime, Spike};
use crate::graph::{DynamicGraph, VertexId, Weight};
use std::collections::HashMap;

/// Configuration for STDP learning
#[derive(Debug, Clone)]
pub struct STDPConfig {
    /// LTP amplitude (potentiation)
    pub a_plus: f64,
    /// LTD amplitude (depression)
    pub a_minus: f64,
    /// LTP time constant (ms)
    pub tau_plus: f64,
    /// LTD time constant (ms)
    pub tau_minus: f64,
    /// Minimum weight
    pub w_min: f64,
    /// Maximum weight
    pub w_max: f64,
    /// Learning rate
    pub learning_rate: f64,
    /// Eligibility trace time constant
    pub tau_eligibility: f64,
}

impl Default for STDPConfig {
    fn default() -> Self {
        Self {
            a_plus: 0.01,
            a_minus: 0.012,
            tau_plus: 20.0,
            tau_minus: 20.0,
            w_min: 0.0,
            w_max: 1.0,
            learning_rate: 1.0,
            tau_eligibility: 1000.0,
        }
    }
}

/// A single synapse between two neurons
#[derive(Debug, Clone)]
pub struct Synapse {
    /// Pre-synaptic neuron ID
    pub pre: usize,
    /// Post-synaptic neuron ID
    pub post: usize,
    /// Synaptic weight
    pub weight: f64,
    /// Transmission delay (ms)
    pub delay: f64,
    /// Eligibility trace for reward-modulated STDP
    pub eligibility: f64,
    /// Last update time
    pub last_update: SimTime,
}

impl Synapse {
    /// Create a new synapse
    pub fn new(pre: usize, post: usize, weight: f64) -> Self {
        Self {
            pre,
            post,
            weight,
            delay: 1.0,
            eligibility: 0.0,
            last_update: 0.0,
        }
    }

    /// Create synapse with delay
    pub fn with_delay(pre: usize, post: usize, weight: f64, delay: f64) -> Self {
        Self {
            pre,
            post,
            weight,
            delay,
            eligibility: 0.0,
            last_update: 0.0,
        }
    }

    /// Compute STDP weight change
    pub fn stdp_update(&mut self, t_pre: SimTime, t_post: SimTime, config: &STDPConfig) -> f64 {
        let dt = t_post - t_pre;

        let dw = if dt > 0.0 {
            // Pre before post → LTP
            config.a_plus * (-dt / config.tau_plus).exp()
        } else {
            // Post before pre → LTD
            -config.a_minus * (dt / config.tau_minus).exp()
        };

        // Apply learning rate and clip
        let delta = config.learning_rate * dw;
        self.weight = (self.weight + delta).clamp(config.w_min, config.w_max);

        // Update eligibility trace
        self.eligibility += dw;

        delta
    }

    /// Decay eligibility trace
    pub fn decay_eligibility(&mut self, dt: f64, tau: f64) {
        self.eligibility *= (-dt / tau).exp();
    }

    /// Apply reward-modulated update (R-STDP)
    pub fn reward_modulated_update(&mut self, reward: f64, config: &STDPConfig) {
        let delta = reward * self.eligibility * config.learning_rate;
        self.weight = (self.weight + delta).clamp(config.w_min, config.w_max);
        // Reset eligibility after reward
        self.eligibility *= 0.5;
    }
}

/// Matrix of synaptic connections
#[derive(Debug, Clone)]
pub struct SynapseMatrix {
    /// Number of pre-synaptic neurons
    pub n_pre: usize,
    /// Number of post-synaptic neurons
    pub n_post: usize,
    /// Synapses indexed by (pre, post)
    synapses: HashMap<(usize, usize), Synapse>,
    /// STDP configuration
    pub config: STDPConfig,
    /// Track last spike times for pre-synaptic neurons
    pre_spike_times: Vec<SimTime>,
    /// Track last spike times for post-synaptic neurons
    post_spike_times: Vec<SimTime>,
}

impl SynapseMatrix {
    /// Create a new synapse matrix
    pub fn new(n_pre: usize, n_post: usize) -> Self {
        Self {
            n_pre,
            n_post,
            synapses: HashMap::new(),
            config: STDPConfig::default(),
            pre_spike_times: vec![f64::NEG_INFINITY; n_pre],
            post_spike_times: vec![f64::NEG_INFINITY; n_post],
        }
    }

    /// Create with custom STDP config
    pub fn with_config(n_pre: usize, n_post: usize, config: STDPConfig) -> Self {
        Self {
            n_pre,
            n_post,
            synapses: HashMap::new(),
            config,
            pre_spike_times: vec![f64::NEG_INFINITY; n_pre],
            post_spike_times: vec![f64::NEG_INFINITY; n_post],
        }
    }

    /// Add a synapse
    pub fn add_synapse(&mut self, pre: usize, post: usize, weight: f64) {
        if pre < self.n_pre && post < self.n_post {
            self.synapses
                .insert((pre, post), Synapse::new(pre, post, weight));
        }
    }

    /// Get synapse if it exists
    pub fn get_synapse(&self, pre: usize, post: usize) -> Option<&Synapse> {
        self.synapses.get(&(pre, post))
    }

    /// Get mutable synapse if it exists
    pub fn get_synapse_mut(&mut self, pre: usize, post: usize) -> Option<&mut Synapse> {
        self.synapses.get_mut(&(pre, post))
    }

    /// Get weight of a synapse (0 if doesn't exist)
    pub fn weight(&self, pre: usize, post: usize) -> f64 {
        self.get_synapse(pre, post).map(|s| s.weight).unwrap_or(0.0)
    }

    /// Compute weighted sum for all post-synaptic neurons given pre-synaptic activations
    ///
    /// This is optimized to iterate only over existing synapses, avoiding O(n²) lookups.
    /// pre_activations[i] is the activation of pre-synaptic neuron i.
    /// Returns vector of weighted sums for each post-synaptic neuron.
    #[inline]
    pub fn compute_weighted_sums(&self, pre_activations: &[f64]) -> Vec<f64> {
        let mut sums = vec![0.0; self.n_post];

        // Iterate only over existing synapses (sparse operation)
        for (&(pre, post), synapse) in &self.synapses {
            if pre < pre_activations.len() {
                sums[post] += synapse.weight * pre_activations[pre];
            }
        }

        sums
    }

    /// Compute weighted sum for a single post-synaptic neuron
    #[inline]
    pub fn weighted_sum_for_post(&self, post: usize, pre_activations: &[f64]) -> f64 {
        let mut sum = 0.0;
        for pre in 0..self.n_pre.min(pre_activations.len()) {
            if let Some(synapse) = self.synapses.get(&(pre, post)) {
                sum += synapse.weight * pre_activations[pre];
            }
        }
        sum
    }

    /// Set weight of a synapse (creates if doesn't exist)
    pub fn set_weight(&mut self, pre: usize, post: usize, weight: f64) {
        if let Some(synapse) = self.get_synapse_mut(pre, post) {
            synapse.weight = weight;
        } else {
            self.add_synapse(pre, post, weight);
        }
    }

    /// Record a pre-synaptic spike and perform STDP updates
    pub fn on_pre_spike(&mut self, pre: usize, time: SimTime) {
        if pre >= self.n_pre {
            return;
        }

        self.pre_spike_times[pre] = time;

        // LTD: pre spike after recent post spikes
        for post in 0..self.n_post {
            if let Some(synapse) = self.synapses.get_mut(&(pre, post)) {
                let t_post = self.post_spike_times[post];
                if t_post > f64::NEG_INFINITY {
                    synapse.stdp_update(time, t_post, &self.config);
                }
            }
        }
    }

    /// Record a post-synaptic spike and perform STDP updates
    pub fn on_post_spike(&mut self, post: usize, time: SimTime) {
        if post >= self.n_post {
            return;
        }

        self.post_spike_times[post] = time;

        // LTP: post spike after recent pre spikes
        for pre in 0..self.n_pre {
            if let Some(synapse) = self.synapses.get_mut(&(pre, post)) {
                let t_pre = self.pre_spike_times[pre];
                if t_pre > f64::NEG_INFINITY {
                    synapse.stdp_update(t_pre, time, &self.config);
                }
            }
        }
    }

    /// Process multiple spikes with STDP
    pub fn process_spikes(&mut self, spikes: &[Spike]) {
        for spike in spikes {
            // Assume neuron IDs map directly
            // Pre-synaptic: lower half, Post-synaptic: upper half (example mapping)
            if spike.neuron_id < self.n_pre {
                self.on_pre_spike(spike.neuron_id, spike.time);
            }
            if spike.neuron_id < self.n_post {
                self.on_post_spike(spike.neuron_id, spike.time);
            }
        }
    }

    /// Decay all eligibility traces
    pub fn decay_eligibility(&mut self, dt: f64) {
        for synapse in self.synapses.values_mut() {
            synapse.decay_eligibility(dt, self.config.tau_eligibility);
        }
    }

    /// Apply reward-modulated learning to all synapses
    pub fn apply_reward(&mut self, reward: f64) {
        for synapse in self.synapses.values_mut() {
            synapse.reward_modulated_update(reward, &self.config);
        }
    }

    /// Get all synapses as an iterator
    pub fn iter(&self) -> impl Iterator<Item = (&(usize, usize), &Synapse)> {
        self.synapses.iter()
    }

    /// Get number of synapses
    pub fn num_synapses(&self) -> usize {
        self.synapses.len()
    }

    /// Compute total synaptic input to a post-synaptic neuron
    pub fn input_to(&self, post: usize, pre_activities: &[f64]) -> f64 {
        let mut total = 0.0;
        for pre in 0..self.n_pre.min(pre_activities.len()) {
            total += self.weight(pre, post) * pre_activities[pre];
        }
        total
    }

    /// Create dense weight matrix
    pub fn to_dense(&self) -> Vec<Vec<f64>> {
        let mut matrix = vec![vec![0.0; self.n_post]; self.n_pre];
        for ((pre, post), synapse) in &self.synapses {
            matrix[*pre][*post] = synapse.weight;
        }
        matrix
    }

    /// Initialize from dense matrix
    pub fn from_dense(matrix: &[Vec<f64>]) -> Self {
        let n_pre = matrix.len();
        let n_post = matrix.first().map(|r| r.len()).unwrap_or(0);

        let mut sm = Self::new(n_pre, n_post);

        for (pre, row) in matrix.iter().enumerate() {
            for (post, &weight) in row.iter().enumerate() {
                if weight != 0.0 {
                    sm.add_synapse(pre, post, weight);
                }
            }
        }

        sm
    }

    /// Synchronize weights with a DynamicGraph
    /// Maps neurons to vertices via a mapping function
    pub fn sync_to_graph<F>(&self, graph: &mut DynamicGraph, neuron_to_vertex: F)
    where
        F: Fn(usize) -> VertexId,
    {
        for ((pre, post), synapse) in &self.synapses {
            let u = neuron_to_vertex(*pre);
            let v = neuron_to_vertex(*post);

            if graph.has_edge(u, v) {
                let _ = graph.update_edge_weight(u, v, synapse.weight);
            }
        }
    }

    /// Load weights from a DynamicGraph
    pub fn sync_from_graph<F>(&mut self, graph: &DynamicGraph, vertex_to_neuron: F)
    where
        F: Fn(VertexId) -> usize,
    {
        for edge in graph.edges() {
            let pre = vertex_to_neuron(edge.source);
            let post = vertex_to_neuron(edge.target);

            if pre < self.n_pre && post < self.n_post {
                self.set_weight(pre, post, edge.weight);
            }
        }
    }

    /// Get high-correlation pairs (synapses with weight above threshold)
    pub fn high_correlation_pairs(&self, threshold: f64) -> Vec<(usize, usize)> {
        self.synapses
            .iter()
            .filter(|(_, s)| s.weight >= threshold)
            .map(|((pre, post), _)| (*pre, *post))
            .collect()
    }
}

/// Asymmetric STDP for causal relationship encoding
#[derive(Debug, Clone)]
pub struct AsymmetricSTDP {
    /// Forward (causal) time constant
    pub tau_forward: f64,
    /// Backward time constant
    pub tau_backward: f64,
    /// Forward amplitude (typically larger for causality)
    pub a_forward: f64,
    /// Backward amplitude
    pub a_backward: f64,
}

impl Default for AsymmetricSTDP {
    fn default() -> Self {
        Self {
            tau_forward: 15.0,
            tau_backward: 30.0, // Longer backward window
            a_forward: 0.015,   // Stronger forward (causal)
            a_backward: 0.008,  // Weaker backward
        }
    }
}

impl AsymmetricSTDP {
    /// Compute weight change for causal relationship encoding
    /// Positive Δt (pre→post) is weighted more heavily
    pub fn compute_dw(&self, dt: f64) -> f64 {
        if dt > 0.0 {
            // Pre before post → causal relationship
            self.a_forward * (-dt / self.tau_forward).exp()
        } else {
            // Post before pre → anti-causal
            -self.a_backward * (dt / self.tau_backward).exp()
        }
    }

    /// Update weight matrix for causal discovery
    pub fn update_weights(&self, matrix: &mut SynapseMatrix, neuron_id: usize, time: SimTime) {
        let w_min = matrix.config.w_min;
        let w_max = matrix.config.w_max;
        let n_pre = matrix.n_pre;
        let n_post = matrix.n_post;

        // Collect pre-spike times first to avoid borrow conflicts
        let pre_times: Vec<_> = (0..n_pre)
            .map(|pre| {
                matrix
                    .pre_spike_times
                    .get(pre)
                    .copied()
                    .unwrap_or(f64::NEG_INFINITY)
            })
            .collect();

        // This neuron just spiked - update all synapses involving it (incoming)
        for pre in 0..n_pre {
            let t_pre = pre_times[pre];
            if t_pre > f64::NEG_INFINITY {
                let dt = time - t_pre;
                let dw = self.compute_dw(dt);
                if let Some(synapse) = matrix.get_synapse_mut(pre, neuron_id) {
                    synapse.weight = (synapse.weight + dw).clamp(w_min, w_max);
                }
            }
        }

        // Collect post-spike times
        let post_times: Vec<_> = (0..n_post)
            .map(|post| {
                matrix
                    .post_spike_times
                    .get(post)
                    .copied()
                    .unwrap_or(f64::NEG_INFINITY)
            })
            .collect();

        for post in 0..n_post {
            let t_post = post_times[post];
            if t_post > f64::NEG_INFINITY {
                let dt = t_post - time; // Reversed for outgoing
                let dw = self.compute_dw(dt);
                if let Some(synapse) = matrix.get_synapse_mut(neuron_id, post) {
                    synapse.weight = (synapse.weight + dw).clamp(w_min, w_max);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synapse_creation() {
        let synapse = Synapse::new(0, 1, 0.5);
        assert_eq!(synapse.pre, 0);
        assert_eq!(synapse.post, 1);
        assert_eq!(synapse.weight, 0.5);
    }

    #[test]
    fn test_stdp_ltp() {
        let mut synapse = Synapse::new(0, 1, 0.5);
        let config = STDPConfig::default();

        // Pre before post → LTP
        let dw = synapse.stdp_update(10.0, 15.0, &config);
        assert!(dw > 0.0);
        assert!(synapse.weight > 0.5);
    }

    #[test]
    fn test_stdp_ltd() {
        let mut synapse = Synapse::new(0, 1, 0.5);
        let config = STDPConfig::default();

        // Post before pre → LTD
        let dw = synapse.stdp_update(15.0, 10.0, &config);
        assert!(dw < 0.0);
        assert!(synapse.weight < 0.5);
    }

    #[test]
    fn test_synapse_matrix() {
        let mut matrix = SynapseMatrix::new(10, 10);
        matrix.add_synapse(0, 1, 0.5);
        matrix.add_synapse(1, 2, 0.3);

        assert_eq!(matrix.num_synapses(), 2);
        assert!((matrix.weight(0, 1) - 0.5).abs() < 0.001);
        assert!((matrix.weight(1, 2) - 0.3).abs() < 0.001);
        assert_eq!(matrix.weight(2, 3), 0.0);
    }

    #[test]
    fn test_spike_processing() {
        let mut matrix = SynapseMatrix::new(5, 5);

        // Fully connected
        for i in 0..5 {
            for j in 0..5 {
                if i != j {
                    matrix.add_synapse(i, j, 0.5);
                }
            }
        }

        // Pre spike then post spike → LTP
        matrix.on_pre_spike(0, 10.0);
        matrix.on_post_spike(1, 15.0);

        // Should have strengthened 0→1 connection
        assert!(matrix.weight(0, 1) > 0.5);
    }

    #[test]
    fn test_asymmetric_stdp() {
        let stdp = AsymmetricSTDP::default();

        // Causal (dt > 0) should have larger effect
        let dw_causal = stdp.compute_dw(5.0);
        let dw_anticausal = stdp.compute_dw(-5.0);

        assert!(dw_causal > 0.0);
        assert!(dw_anticausal < 0.0);
        assert!(dw_causal.abs() > dw_anticausal.abs());
    }

    #[test]
    fn test_dense_conversion() {
        let mut matrix = SynapseMatrix::new(3, 3);
        matrix.add_synapse(0, 1, 0.5);
        matrix.add_synapse(1, 2, 0.7);

        let dense = matrix.to_dense();
        assert_eq!(dense.len(), 3);
        assert!((dense[0][1] - 0.5).abs() < 0.001);
        assert!((dense[1][2] - 0.7).abs() < 0.001);

        let recovered = SynapseMatrix::from_dense(&dense);
        assert_eq!(recovered.num_synapses(), 2);
    }
}
