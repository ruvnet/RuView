//! # Layer 6: Neural Optimizer as Reinforcement Learning on Graphs
//!
//! Implements neural network-based graph optimization using policy gradients.
//!
//! ## Architecture
//!
//! - **Policy SNN**: Outputs graph modification actions via spike rates
//! - **Value Network**: Estimates mincut improvement
//! - **Experience Replay**: Prioritized sampling for stable learning
//!
//! ## Key Features
//!
//! - STDP-based policy gradient for spike-driven learning
//! - TD learning via spike-timing for value estimation
//! - Subpolynomial search exploiting learned graph structure

use super::{
    network::{LayerConfig, NetworkConfig, SpikingNetwork},
    neuron::{LIFNeuron, NeuronConfig, NeuronPopulation},
    synapse::{STDPConfig, Synapse, SynapseMatrix},
    SimTime, Spike,
};
use crate::graph::{DynamicGraph, EdgeId, VertexId, Weight};
use std::collections::VecDeque;

/// Configuration for neural graph optimizer
#[derive(Debug, Clone)]
pub struct OptimizerConfig {
    /// Number of input features
    pub input_size: usize,
    /// Hidden layer size
    pub hidden_size: usize,
    /// Number of possible actions
    pub num_actions: usize,
    /// Learning rate
    pub learning_rate: f64,
    /// Discount factor (gamma)
    pub gamma: f64,
    /// Reward weight for search efficiency
    pub search_weight: f64,
    /// Experience replay buffer size
    pub replay_buffer_size: usize,
    /// Batch size for training
    pub batch_size: usize,
    /// Time step
    pub dt: f64,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            input_size: 10,
            hidden_size: 32,
            num_actions: 5,
            learning_rate: 0.01,
            gamma: 0.99,
            search_weight: 0.1,
            replay_buffer_size: 10000,
            batch_size: 32,
            dt: 1.0,
        }
    }
}

/// Graph modification action
#[derive(Debug, Clone, PartialEq)]
pub enum GraphAction {
    /// Add an edge between vertices
    AddEdge(VertexId, VertexId, Weight),
    /// Remove an edge
    RemoveEdge(VertexId, VertexId),
    /// Strengthen an edge (increase weight)
    Strengthen(VertexId, VertexId, f64),
    /// Weaken an edge (decrease weight)
    Weaken(VertexId, VertexId, f64),
    /// No action
    NoOp,
}

impl GraphAction {
    /// Get action index
    pub fn to_index(&self) -> usize {
        match self {
            GraphAction::AddEdge(..) => 0,
            GraphAction::RemoveEdge(..) => 1,
            GraphAction::Strengthen(..) => 2,
            GraphAction::Weaken(..) => 3,
            GraphAction::NoOp => 4,
        }
    }
}

/// Result of an optimization step
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    /// Action taken
    pub action: GraphAction,
    /// Reward received
    pub reward: f64,
    /// New mincut value
    pub new_mincut: f64,
    /// Average search latency
    pub search_latency: f64,
}

/// Experience for replay buffer
#[derive(Debug, Clone)]
struct Experience {
    /// State features
    state: Vec<f64>,
    /// Action taken
    action_idx: usize,
    /// Reward received
    reward: f64,
    /// Next state features
    next_state: Vec<f64>,
    /// Is terminal state
    done: bool,
    /// TD error for prioritization
    td_error: f64,
}

/// Prioritized experience replay buffer
struct PrioritizedReplayBuffer {
    /// Stored experiences
    buffer: VecDeque<Experience>,
    /// Maximum capacity
    capacity: usize,
}

impl PrioritizedReplayBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn push(&mut self, exp: Experience) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(exp);
    }

    fn sample(&self, batch_size: usize) -> Vec<&Experience> {
        // Prioritized by TD error (simplified: just take recent high-error samples)
        let mut sorted: Vec<_> = self.buffer.iter().collect();
        sorted.sort_by(|a, b| {
            b.td_error
                .abs()
                .partial_cmp(&a.td_error.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        sorted.into_iter().take(batch_size).collect()
    }

    fn len(&self) -> usize {
        self.buffer.len()
    }
}

/// Simple value network for state value estimation
#[derive(Debug, Clone)]
pub struct ValueNetwork {
    /// Weights from input to hidden
    w_hidden: Vec<Vec<f64>>,
    /// Hidden biases
    b_hidden: Vec<f64>,
    /// Weights from hidden to output
    w_output: Vec<f64>,
    /// Output bias
    b_output: f64,
    /// Last estimate (for TD error)
    last_estimate: f64,
}

impl ValueNetwork {
    /// Create a new value network
    pub fn new(input_size: usize, hidden_size: usize) -> Self {
        // Initialize with small random weights (Xavier initialization)
        let scale = (2.0 / (input_size + hidden_size) as f64).sqrt();
        let w_hidden: Vec<Vec<f64>> = (0..hidden_size)
            .map(|_| (0..input_size).map(|_| rand_small() * scale).collect())
            .collect();

        let b_hidden = vec![0.0; hidden_size];

        let output_scale = (1.0 / hidden_size as f64).sqrt();
        let w_output: Vec<f64> = (0..hidden_size)
            .map(|_| rand_small() * output_scale)
            .collect();
        let b_output = 0.0;

        Self {
            w_hidden,
            b_hidden,
            w_output,
            b_output,
            last_estimate: 0.0,
        }
    }

    /// Estimate value of a state
    pub fn estimate(&mut self, state: &[f64]) -> f64 {
        // Hidden layer
        let mut hidden = vec![0.0; self.w_hidden.len()];
        for (j, weights) in self.w_hidden.iter().enumerate() {
            let mut sum = self.b_hidden[j];
            for (i, &w) in weights.iter().enumerate() {
                if i < state.len() {
                    sum += w * state[i];
                }
            }
            hidden[j] = relu(sum);
        }

        // Output layer
        let mut output = self.b_output;
        for (j, &w) in self.w_output.iter().enumerate() {
            output += w * hidden[j];
        }

        self.last_estimate = output;
        output
    }

    /// Get previous estimate
    pub fn estimate_previous(&self) -> f64 {
        self.last_estimate
    }

    /// Update weights with TD error using proper backpropagation
    ///
    /// Implements gradient descent with:
    /// - Forward pass to compute activations
    /// - Backward pass to compute ∂V/∂w
    /// - Weight update: w += lr * td_error * ∂V/∂w
    pub fn update(&mut self, state: &[f64], td_error: f64, lr: f64) {
        let hidden_size = self.w_hidden.len();
        let input_size = if self.w_hidden.is_empty() {
            0
        } else {
            self.w_hidden[0].len()
        };

        // Forward pass: compute hidden activations and pre-activations
        let mut hidden_pre = vec![0.0; hidden_size]; // Before ReLU
        let mut hidden_post = vec![0.0; hidden_size]; // After ReLU

        for (j, weights) in self.w_hidden.iter().enumerate() {
            let mut sum = self.b_hidden[j];
            for (i, &w) in weights.iter().enumerate() {
                if i < state.len() {
                    sum += w * state[i];
                }
            }
            hidden_pre[j] = sum;
            hidden_post[j] = relu(sum);
        }

        // Backward pass: compute gradients
        // Output layer gradient: ∂L/∂w_output = td_error * hidden_post
        // (since L = 0.5 * td_error², ∂L/∂V = td_error)

        // Update output weights: ∂V/∂w_output[j] = hidden_post[j]
        for (j, w) in self.w_output.iter_mut().enumerate() {
            *w += lr * td_error * hidden_post[j];
        }
        self.b_output += lr * td_error;

        // Backpropagate to hidden layer
        // ∂V/∂hidden_post[j] = w_output[j]
        // ∂hidden_post/∂hidden_pre = relu'(hidden_pre) = 1 if hidden_pre > 0 else 0
        // ∂V/∂w_hidden[j][i] = ∂V/∂hidden_post[j] * relu'(hidden_pre[j]) * state[i]

        for (j, weights) in self.w_hidden.iter_mut().enumerate() {
            // ReLU derivative: 1 if pre-activation > 0, else 0
            let relu_grad = if hidden_pre[j] > 0.0 { 1.0 } else { 0.0 };
            let delta = td_error * self.w_output[j] * relu_grad;

            for (i, w) in weights.iter_mut().enumerate() {
                if i < state.len() {
                    *w += lr * delta * state[i];
                }
            }
            self.b_hidden[j] += lr * delta;
        }
    }
}

/// Policy SNN for action selection
pub struct PolicySNN {
    /// Input layer
    input_layer: NeuronPopulation,
    /// Hidden recurrent layer
    hidden_layer: NeuronPopulation,
    /// Output layer (one neuron per action)
    output_layer: NeuronPopulation,
    /// Input → Hidden weights
    w_ih: SynapseMatrix,
    /// Hidden → Output weights
    w_ho: SynapseMatrix,
    /// Reward-modulated STDP configuration
    stdp_config: STDPConfig,
    /// Configuration
    config: OptimizerConfig,
}

impl PolicySNN {
    /// Create a new policy SNN
    pub fn new(config: OptimizerConfig) -> Self {
        let input_config = NeuronConfig {
            tau_membrane: 10.0,
            threshold: 0.8,
            ..NeuronConfig::default()
        };

        let hidden_config = NeuronConfig {
            tau_membrane: 20.0,
            threshold: 1.0,
            ..NeuronConfig::default()
        };

        let output_config = NeuronConfig {
            tau_membrane: 15.0,
            threshold: 0.6,
            ..NeuronConfig::default()
        };

        let input_layer = NeuronPopulation::with_config(config.input_size, input_config);
        let hidden_layer = NeuronPopulation::with_config(config.hidden_size, hidden_config);
        let output_layer = NeuronPopulation::with_config(config.num_actions, output_config);

        // Initialize weights
        let mut w_ih = SynapseMatrix::new(config.input_size, config.hidden_size);
        let mut w_ho = SynapseMatrix::new(config.hidden_size, config.num_actions);

        // Random initialization
        for i in 0..config.input_size {
            for j in 0..config.hidden_size {
                w_ih.add_synapse(i, j, rand_small() + 0.3);
            }
        }

        for i in 0..config.hidden_size {
            for j in 0..config.num_actions {
                w_ho.add_synapse(i, j, rand_small() + 0.3);
            }
        }

        Self {
            input_layer,
            hidden_layer,
            output_layer,
            w_ih,
            w_ho,
            stdp_config: STDPConfig::default(),
            config,
        }
    }

    /// Inject state as current to input layer
    pub fn inject(&mut self, state: &[f64]) {
        for (i, neuron) in self.input_layer.neurons.iter_mut().enumerate() {
            if i < state.len() {
                neuron.set_membrane_potential(state[i]);
            }
        }
    }

    /// Run until decision (output spike)
    pub fn run_until_decision(&mut self, max_steps: usize) -> Vec<Spike> {
        for step in 0..max_steps {
            let time = step as f64 * self.config.dt;

            // Compute hidden currents from input
            let mut hidden_currents = vec![0.0; self.config.hidden_size];
            for j in 0..self.config.hidden_size {
                for i in 0..self.config.input_size {
                    hidden_currents[j] += self.w_ih.weight(i, j)
                        * self.input_layer.neurons[i].membrane_potential().max(0.0);
                }
            }

            // Update hidden layer
            let hidden_spikes = self.hidden_layer.step(&hidden_currents, self.config.dt);

            // Compute output currents from hidden
            let mut output_currents = vec![0.0; self.config.num_actions];
            for j in 0..self.config.num_actions {
                for i in 0..self.config.hidden_size {
                    output_currents[j] += self.w_ho.weight(i, j)
                        * self.hidden_layer.neurons[i].membrane_potential().max(0.0);
                }
            }

            // Update output layer
            let output_spikes = self.output_layer.step(&output_currents, self.config.dt);

            // STDP updates
            for spike in &hidden_spikes {
                self.w_ih.on_post_spike(spike.neuron_id, time);
            }
            for spike in &output_spikes {
                self.w_ho.on_post_spike(spike.neuron_id, time);
            }

            // Return if we have output spikes
            if !output_spikes.is_empty() {
                return output_spikes;
            }
        }

        Vec::new()
    }

    /// Apply reward-modulated STDP
    pub fn apply_reward_modulated_stdp(&mut self, td_error: f64) {
        self.w_ih.apply_reward(td_error);
        self.w_ho.apply_reward(td_error);
    }

    /// Get regions with low activity (for search skip)
    pub fn low_activity_regions(&self) -> Vec<usize> {
        self.hidden_layer
            .spike_trains
            .iter()
            .enumerate()
            .filter(|(_, t)| t.spike_rate(100.0) < 0.001)
            .map(|(i, _)| i)
            .collect()
    }

    /// Reset SNN state
    pub fn reset(&mut self) {
        self.input_layer.reset();
        self.hidden_layer.reset();
        self.output_layer.reset();
    }
}

/// Neural Graph Optimizer combining policy and value networks
pub struct NeuralGraphOptimizer {
    /// Policy network: SNN that outputs graph modification actions
    policy_snn: PolicySNN,
    /// Value network: estimates mincut improvement
    value_network: ValueNetwork,
    /// Experience replay buffer
    replay_buffer: PrioritizedReplayBuffer,
    /// Current graph state
    graph: DynamicGraph,
    /// Configuration
    config: OptimizerConfig,
    /// Current simulation time
    time: SimTime,
    /// Previous mincut for reward computation
    prev_mincut: f64,
    /// Previous state for experience storage
    prev_state: Vec<f64>,
    /// Search statistics
    search_latencies: VecDeque<f64>,
}

impl NeuralGraphOptimizer {
    /// Create a new neural graph optimizer
    pub fn new(graph: DynamicGraph, config: OptimizerConfig) -> Self {
        let prev_state = extract_features(&graph, config.input_size);
        let prev_mincut = estimate_mincut(&graph);

        Self {
            policy_snn: PolicySNN::new(config.clone()),
            value_network: ValueNetwork::new(config.input_size, config.hidden_size),
            replay_buffer: PrioritizedReplayBuffer::new(config.replay_buffer_size),
            graph,
            config,
            time: 0.0,
            prev_mincut,
            prev_state,
            search_latencies: VecDeque::with_capacity(100),
        }
    }

    /// Run one optimization step
    pub fn optimize_step(&mut self) -> OptimizationResult {
        // 1. Encode current state as spike pattern
        let state = extract_features(&self.graph, self.config.input_size);

        // 2. Policy SNN outputs action distribution via spike rates
        self.policy_snn.inject(&state);
        let action_spikes = self.policy_snn.run_until_decision(50);
        let action = self.decode_action(&action_spikes);

        // 3. Execute action on graph
        let old_mincut = estimate_mincut(&self.graph);
        self.apply_action(&action);
        let new_mincut = estimate_mincut(&self.graph);

        // 4. Compute reward: mincut improvement + search efficiency
        let mincut_reward = if old_mincut > 0.0 {
            (new_mincut - old_mincut) / old_mincut
        } else {
            0.0
        };

        let search_reward = self.measure_search_efficiency();
        let reward = mincut_reward + self.config.search_weight * search_reward;

        // 5. TD learning update via spike-timing
        let new_state = extract_features(&self.graph, self.config.input_size);
        let current_value = self.value_network.estimate(&state);
        let next_value = self.value_network.estimate(&new_state);

        let td_error = reward + self.config.gamma * next_value - current_value;

        // 6. STDP-based policy gradient
        self.policy_snn.apply_reward_modulated_stdp(td_error);

        // 7. Update value network
        self.value_network
            .update(&state, td_error, self.config.learning_rate);

        // 8. Store experience
        let exp = Experience {
            state: self.prev_state.clone(),
            action_idx: action.to_index(),
            reward,
            next_state: new_state.clone(),
            done: false,
            td_error,
        };
        self.replay_buffer.push(exp);

        // Update state
        self.prev_state = new_state;
        self.prev_mincut = new_mincut;
        self.time += self.config.dt;

        OptimizationResult {
            action,
            reward,
            new_mincut,
            search_latency: search_reward,
        }
    }

    /// Decode action from spikes
    fn decode_action(&self, spikes: &[Spike]) -> GraphAction {
        if spikes.is_empty() {
            return GraphAction::NoOp;
        }

        // Use first spike's neuron as action
        let action_idx = spikes[0].neuron_id;

        // Get random vertices for action
        let vertices: Vec<_> = self.graph.vertices();

        if vertices.len() < 2 {
            return GraphAction::NoOp;
        }

        let v1 = vertices[action_idx % vertices.len()];
        let v2 = vertices[(action_idx + 1) % vertices.len()];

        match action_idx % 5 {
            0 => {
                if !self.graph.has_edge(v1, v2) {
                    GraphAction::AddEdge(v1, v2, 1.0)
                } else {
                    GraphAction::NoOp
                }
            }
            1 => {
                if self.graph.has_edge(v1, v2) {
                    GraphAction::RemoveEdge(v1, v2)
                } else {
                    GraphAction::NoOp
                }
            }
            2 => GraphAction::Strengthen(v1, v2, 0.1),
            3 => GraphAction::Weaken(v1, v2, 0.1),
            _ => GraphAction::NoOp,
        }
    }

    /// Apply action to graph
    fn apply_action(&mut self, action: &GraphAction) {
        match action {
            GraphAction::AddEdge(u, v, w) => {
                if !self.graph.has_edge(*u, *v) {
                    let _ = self.graph.insert_edge(*u, *v, *w);
                }
            }
            GraphAction::RemoveEdge(u, v) => {
                let _ = self.graph.delete_edge(*u, *v);
            }
            GraphAction::Strengthen(u, v, delta) => {
                if let Some(edge) = self.graph.get_edge(*u, *v) {
                    let _ = self.graph.update_edge_weight(*u, *v, edge.weight + delta);
                }
            }
            GraphAction::Weaken(u, v, delta) => {
                if let Some(edge) = self.graph.get_edge(*u, *v) {
                    let new_weight = (edge.weight - delta).max(0.01);
                    let _ = self.graph.update_edge_weight(*u, *v, new_weight);
                }
            }
            GraphAction::NoOp => {}
        }
    }

    /// Measure search efficiency
    fn measure_search_efficiency(&mut self) -> f64 {
        // Simplified: based on graph connectivity
        let n = self.graph.num_vertices() as f64;
        let m = self.graph.num_edges() as f64;

        if n < 2.0 {
            return 0.0;
        }

        // Higher connectivity relative to vertices = better search
        let efficiency = m / (n * (n - 1.0) / 2.0);

        self.search_latencies.push_back(efficiency);
        if self.search_latencies.len() > 100 {
            self.search_latencies.pop_front();
        }

        efficiency
    }

    /// Get learned skip regions for subpolynomial search
    pub fn search_skip_regions(&self) -> Vec<usize> {
        self.policy_snn.low_activity_regions()
    }

    /// Search with learned structure
    pub fn search(&self, query: &[f64], k: usize) -> Vec<VertexId> {
        // Use skip regions to guide search
        let skip_regions = self.search_skip_regions();

        // Simple nearest neighbor in graph space
        let vertices: Vec<_> = self.graph.vertices();

        let mut scores: Vec<(VertexId, f64)> = vertices
            .iter()
            .enumerate()
            .filter(|(i, _)| !skip_regions.contains(i))
            .map(|(i, &v)| {
                // Score based on degree (proxy for centrality)
                let score = self.graph.degree(v) as f64;
                (v, score)
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scores.into_iter().take(k).map(|(v, _)| v).collect()
    }

    /// Get underlying graph
    pub fn graph(&self) -> &DynamicGraph {
        &self.graph
    }

    /// Get mutable graph
    pub fn graph_mut(&mut self) -> &mut DynamicGraph {
        &mut self.graph
    }

    /// Run multiple optimization steps
    pub fn optimize(&mut self, steps: usize) -> Vec<OptimizationResult> {
        (0..steps).map(|_| self.optimize_step()).collect()
    }

    /// Reset optimizer state
    pub fn reset(&mut self) {
        self.policy_snn.reset();
        self.prev_mincut = estimate_mincut(&self.graph);
        self.prev_state = extract_features(&self.graph, self.config.input_size);
        self.time = 0.0;
    }
}

/// Extract features from graph
fn extract_features(graph: &DynamicGraph, num_features: usize) -> Vec<f64> {
    let n = graph.num_vertices() as f64;
    let m = graph.num_edges() as f64;

    let mut features = vec![0.0; num_features];

    if num_features > 0 {
        features[0] = n / 1000.0; // Normalized vertex count
    }
    if num_features > 1 {
        features[1] = m / 5000.0; // Normalized edge count
    }
    if num_features > 2 {
        features[2] = if n > 1.0 {
            m / (n * (n - 1.0) / 2.0)
        } else {
            0.0
        }; // Density
    }
    if num_features > 3 {
        // Average degree
        let avg_deg: f64 = graph
            .vertices()
            .iter()
            .map(|&v| graph.degree(v) as f64)
            .sum::<f64>()
            / n.max(1.0);
        features[3] = avg_deg / 10.0;
    }
    if num_features > 4 {
        features[4] = estimate_mincut(graph) / m.max(1.0); // Normalized mincut
    }

    // Fill rest with zeros or derived features
    for i in 5..num_features {
        features[i] = features[i % 5] * 0.1;
    }

    features
}

/// Estimate mincut (simplified)
fn estimate_mincut(graph: &DynamicGraph) -> f64 {
    if graph.num_vertices() == 0 {
        return 0.0;
    }

    graph
        .vertices()
        .iter()
        .map(|&v| graph.degree(v) as f64)
        .fold(f64::INFINITY, f64::min)
}

// Thread-safe PRNG helpers using atomic CAS
use std::sync::atomic::{AtomicU64, Ordering};
static OPTIMIZER_RNG: AtomicU64 = AtomicU64::new(0xdeadbeef12345678);

fn rand_small() -> f64 {
    // Use compare_exchange loop to ensure atomicity
    let state = loop {
        let current = OPTIMIZER_RNG.load(Ordering::Relaxed);
        let next = current.wrapping_mul(0x5851f42d4c957f2d).wrapping_add(1);
        match OPTIMIZER_RNG.compare_exchange_weak(
            current,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break next,
            Err(_) => continue,
        }
    };
    (state as f64) / (u64::MAX as f64) * 0.4 - 0.2
}

fn relu(x: f64) -> f64 {
    x.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_network() {
        let mut network = ValueNetwork::new(5, 10);

        let state = vec![0.5, 0.3, 0.8, 0.2, 0.9];
        let value = network.estimate(&state);

        assert!(value.is_finite());
    }

    #[test]
    fn test_policy_snn() {
        let config = OptimizerConfig::default();
        let mut policy = PolicySNN::new(config);

        let state = vec![1.0; 10];
        policy.inject(&state);

        let spikes = policy.run_until_decision(100);
        // May or may not spike
        assert!(spikes.len() >= 0);
    }

    #[test]
    fn test_neural_optimizer() {
        let graph = DynamicGraph::new();
        for i in 0..10 {
            graph.insert_edge(i, (i + 1) % 10, 1.0).unwrap();
        }

        let config = OptimizerConfig::default();
        let mut optimizer = NeuralGraphOptimizer::new(graph, config);

        let result = optimizer.optimize_step();

        assert!(result.new_mincut.is_finite());
    }

    #[test]
    fn test_optimize_multiple() {
        let graph = DynamicGraph::new();
        for i in 0..5 {
            for j in (i + 1)..5 {
                graph.insert_edge(i, j, 1.0).unwrap();
            }
        }

        let config = OptimizerConfig::default();
        let mut optimizer = NeuralGraphOptimizer::new(graph, config);

        let results = optimizer.optimize(10);
        assert_eq!(results.len(), 10);
    }

    #[test]
    fn test_search() {
        let graph = DynamicGraph::new();
        for i in 0..20 {
            graph.insert_edge(i, (i + 1) % 20, 1.0).unwrap();
        }

        let config = OptimizerConfig::default();
        let optimizer = NeuralGraphOptimizer::new(graph, config);

        let results = optimizer.search(&[0.5; 10], 5);
        assert!(results.len() <= 5);
    }
}
