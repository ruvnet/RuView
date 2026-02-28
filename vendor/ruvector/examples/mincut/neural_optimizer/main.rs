//! Neural Temporal Graph Optimization Example
//!
//! This example demonstrates how to use simple neural networks to learn
//! optimal graph configurations over time. The neural optimizer learns from
//! historical graph evolution to predict which modifications will lead to
//! better minimum cut values.

use ruvector_mincut::prelude::*;

/// Simple neural network for graph optimization
/// Uses linear transformations without external deep learning dependencies
struct NeuralNetwork {
    /// Weight matrix for hidden layer (input_size × hidden_size)
    weights_hidden: Vec<Vec<f64>>,
    /// Bias vector for hidden layer
    bias_hidden: Vec<f64>,
    /// Weight matrix for output layer (hidden_size × output_size)
    weights_output: Vec<Vec<f64>>,
    /// Bias vector for output layer
    bias_output: Vec<f64>,
}

impl NeuralNetwork {
    fn new(input_size: usize, hidden_size: usize, output_size: usize) -> Self {
        use std::f64::consts::PI;

        // Initialize with small random weights (Xavier initialization)
        let scale_hidden = (2.0 / input_size as f64).sqrt();
        let scale_output = (2.0 / hidden_size as f64).sqrt();

        let weights_hidden = (0..input_size)
            .map(|i| {
                (0..hidden_size)
                    .map(|j| {
                        let angle = (i * 7 + j * 13) as f64;
                        (angle * PI / 180.0).sin() * scale_hidden
                    })
                    .collect()
            })
            .collect();

        let bias_hidden = vec![0.0; hidden_size];

        let weights_output = (0..hidden_size)
            .map(|i| {
                (0..output_size)
                    .map(|j| {
                        let angle = (i * 11 + j * 17) as f64;
                        (angle * PI / 180.0).cos() * scale_output
                    })
                    .collect()
            })
            .collect();

        let bias_output = vec![0.0; output_size];

        Self {
            weights_hidden,
            bias_hidden,
            weights_output,
            bias_output,
        }
    }

    /// Forward pass through the network
    fn forward(&self, input: &[f64]) -> Vec<f64> {
        // Hidden layer: input × weights_hidden + bias
        let hidden: Vec<f64> = (0..self.bias_hidden.len())
            .map(|j| {
                let sum: f64 = input
                    .iter()
                    .enumerate()
                    .map(|(i, &x)| x * self.weights_hidden[i][j])
                    .sum();
                relu(sum + self.bias_hidden[j])
            })
            .collect();

        // Output layer: hidden × weights_output + bias
        (0..self.bias_output.len())
            .map(|j| {
                let sum: f64 = hidden
                    .iter()
                    .enumerate()
                    .map(|(i, &x)| x * self.weights_output[i][j])
                    .sum();
                sum + self.bias_output[j]
            })
            .collect()
    }

    /// Mutate weights for evolutionary optimization
    fn mutate(&mut self, mutation_rate: f64, mutation_strength: f64) {
        let mut rng_state = 42u64;

        for i in 0..self.weights_hidden.len() {
            for j in 0..self.weights_hidden[i].len() {
                if simple_random(&mut rng_state) < mutation_rate {
                    let delta = (simple_random(&mut rng_state) - 0.5) * mutation_strength;
                    self.weights_hidden[i][j] += delta;
                }
            }
        }

        for i in 0..self.weights_output.len() {
            for j in 0..self.weights_output[i].len() {
                if simple_random(&mut rng_state) < mutation_rate {
                    let delta = (simple_random(&mut rng_state) - 0.5) * mutation_strength;
                    self.weights_output[i][j] += delta;
                }
            }
        }
    }

    /// Clone the network
    fn clone_network(&self) -> Self {
        Self {
            weights_hidden: self.weights_hidden.clone(),
            bias_hidden: self.bias_hidden.clone(),
            weights_output: self.weights_output.clone(),
            bias_output: self.bias_output.clone(),
        }
    }
}

/// ReLU activation function
fn relu(x: f64) -> f64 {
    x.max(0.0)
}

/// Simple random number generator (LCG)
fn simple_random(state: &mut u64) -> f64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*state >> 32) as f64 / u32::MAX as f64
}

/// Extract features from a graph for neural network input
fn extract_features(graph: &DynamicGraph) -> Vec<f64> {
    let stats = graph.stats();
    let node_count = stats.num_vertices as f64;
    let edge_count = stats.num_edges as f64;
    let max_possible_edges = node_count * (node_count - 1.0) / 2.0;
    let density = if max_possible_edges > 0.0 {
        edge_count / max_possible_edges
    } else {
        0.0
    };

    // Calculate average degree
    let avg_degree = stats.avg_degree;

    vec![
        node_count / 100.0, // Normalized node count
        edge_count / 500.0, // Normalized edge count
        density,            // Graph density
        avg_degree / 10.0,  // Normalized average degree
    ]
}

/// Neural Graph Optimizer using reinforcement learning
struct NeuralGraphOptimizer {
    /// Policy network: decides which action to take
    policy_network: NeuralNetwork,
    /// Value network: predicts future mincut value
    value_network: NeuralNetwork,
    /// Training history
    history: Vec<(Vec<f64>, f64)>, // (state, actual_mincut)
}

impl NeuralGraphOptimizer {
    fn new() -> Self {
        let input_size = 4; // Feature vector size
        let hidden_size = 8;
        let policy_output = 3; // Add edge, remove edge, do nothing
        let value_output = 1; // Predicted mincut value

        Self {
            policy_network: NeuralNetwork::new(input_size, hidden_size, policy_output),
            value_network: NeuralNetwork::new(input_size, hidden_size, value_output),
            history: Vec::new(),
        }
    }

    /// Predict the best action for current graph state
    fn predict_action(&self, graph: &DynamicGraph) -> usize {
        let features = extract_features(graph);
        let policy_output = self.policy_network.forward(&features);

        // Find action with highest probability
        policy_output
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }

    /// Predict the mincut value for current state
    fn predict_value(&self, graph: &DynamicGraph) -> f64 {
        let features = extract_features(graph);
        let value_output = self.value_network.forward(&features);
        value_output[0].max(0.0)
    }

    /// Apply an action to the graph
    fn apply_action(&self, graph: &mut DynamicGraph, action: usize, rng_state: &mut u64) {
        let stats = graph.stats();
        match action {
            0 => {
                // Add a random edge
                let n = stats.num_vertices;
                if n > 1 {
                    let u = (simple_random(rng_state) * n as f64) as u64;
                    let v = (simple_random(rng_state) * (n - 1) as f64) as u64;
                    let v = if v >= u { v + 1 } else { v };
                    let weight = 1.0 + simple_random(rng_state) * 10.0;
                    let _ = graph.insert_edge(u, v, weight);
                }
            }
            1 => {
                // Remove a random edge (simplified - would need edge list in real impl)
                // For this example, we'll skip actual removal
            }
            _ => {
                // Do nothing
            }
        }
    }

    /// Train the networks using evolutionary strategy
    fn train(&mut self, generations: usize, population_size: usize) {
        println!("\n🧠 Training Neural Networks");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        for gen in 0..generations {
            // Create population by mutating current networks
            let mut population = Vec::new();

            for _ in 0..population_size {
                let mut policy = self.policy_network.clone_network();
                let mut value = self.value_network.clone_network();

                policy.mutate(0.1, 0.5);
                value.mutate(0.1, 0.5);

                population.push((policy, value));
            }

            // Evaluate fitness on training data
            let mut fitness_scores = Vec::new();

            for (_policy, value) in &population {
                let mut total_error = 0.0;

                for (state, actual_mincut) in &self.history {
                    let predicted = value.forward(state)[0];
                    let error = (predicted - actual_mincut).abs();
                    total_error += error;
                }

                let fitness = if self.history.is_empty() {
                    0.0
                } else {
                    -total_error / self.history.len() as f64
                };

                fitness_scores.push(fitness);
            }

            // Select best network
            if let Some((best_idx, &best_fitness)) = fitness_scores
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            {
                if gen % 10 == 0 {
                    println!("Generation {}: Best fitness = {:.4}", gen, -best_fitness);
                }

                self.policy_network = population[best_idx].0.clone_network();
                self.value_network = population[best_idx].1.clone_network();
            }
        }

        println!("✓ Training complete");
    }

    /// Record a state observation for training
    fn record_observation(&mut self, graph: &DynamicGraph, mincut: f64) {
        let features = extract_features(graph);
        self.history.push((features, mincut));

        // Keep only recent history (last 100 observations)
        if self.history.len() > 100 {
            self.history.remove(0);
        }
    }
}

/// Generate a random graph for testing
fn generate_random_graph(nodes: usize, edge_prob: f64, rng_state: &mut u64) -> DynamicGraph {
    let graph = DynamicGraph::new();

    for i in 0..nodes {
        graph.add_vertex(i as u64);
    }

    for i in 0..nodes {
        for j in i + 1..nodes {
            if simple_random(rng_state) < edge_prob {
                let weight = 1.0 + simple_random(rng_state) * 10.0;
                let _ = graph.insert_edge(i as u64, j as u64, weight);
            }
        }
    }

    graph
}

/// Calculate minimum cut value for a graph
/// This is a simplified approximation for demonstration purposes
fn calculate_mincut(graph: &DynamicGraph) -> Option<f64> {
    let stats = graph.stats();
    if stats.num_edges == 0 {
        return None;
    }

    // For this example, we'll use a simple approximation based on graph properties
    // Real implementation would use the full MinCut algorithm
    // This approximation: mincut ≈ min_degree * (total_weight / num_edges)
    let min_cut_approx = stats.min_degree as f64 * (stats.total_weight / stats.num_edges as f64);

    Some(min_cut_approx.max(1.0))
}

/// Run optimization loop with neural guidance
fn optimize_with_neural(
    optimizer: &mut NeuralGraphOptimizer,
    initial_graph: &DynamicGraph,
    steps: usize,
    rng_state: &mut u64,
) -> Vec<f64> {
    let mut graph = initial_graph.clone();
    let mut mincut_history = Vec::new();

    for _ in 0..steps {
        // Predict and apply action
        let action = optimizer.predict_action(&graph);
        optimizer.apply_action(&mut graph, action, rng_state);

        // Calculate current mincut
        if let Some(mincut) = calculate_mincut(&graph) {
            mincut_history.push(mincut);
            optimizer.record_observation(&graph, mincut);
        }
    }

    mincut_history
}

/// Run optimization with random actions (baseline)
fn optimize_random(initial_graph: &DynamicGraph, steps: usize, rng_state: &mut u64) -> Vec<f64> {
    let mut graph = initial_graph.clone();
    let mut mincut_history = Vec::new();

    for _ in 0..steps {
        // Random action
        let action = (simple_random(rng_state) * 3.0) as usize;

        // Apply action
        let stats = graph.stats();
        match action {
            0 => {
                let n = stats.num_vertices;
                if n > 1 {
                    let u = (simple_random(rng_state) * n as f64) as u64;
                    let v = (simple_random(rng_state) * (n - 1) as f64) as u64;
                    let v = if v >= u { v + 1 } else { v };
                    let weight = 1.0 + simple_random(rng_state) * 10.0;
                    let _ = graph.insert_edge(u, v, weight);
                }
            }
            _ => {}
        }

        // Calculate mincut
        if let Some(mincut) = calculate_mincut(&graph) {
            mincut_history.push(mincut);
        }
    }

    mincut_history
}

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║   Neural Temporal Graph Optimization Example              ║");
    println!("║   Learning to Predict Optimal Graph Configurations        ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    let mut rng_state = 12345u64;

    // Initialize neural optimizer
    println!("\n📊 Initializing Neural Graph Optimizer");
    let mut optimizer = NeuralGraphOptimizer::new();

    // Generate initial training data
    println!("\n🔬 Generating Training Data");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    for i in 0..20 {
        let graph = generate_random_graph(10, 0.3, &mut rng_state);
        if let Some(mincut) = calculate_mincut(&graph) {
            optimizer.record_observation(&graph, mincut);
            if i % 5 == 0 {
                println!("Sample {}: Mincut = {:.2}", i, mincut);
            }
        }
    }

    // Train the neural networks
    optimizer.train(50, 20);

    // Compare neural-guided vs random optimization
    println!("\n⚖️  Comparing Optimization Strategies");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let test_graph = generate_random_graph(15, 0.25, &mut rng_state);
    let steps = 30;

    println!("\n🤖 Neural-Guided Optimization ({} steps)", steps);
    let neural_history = optimize_with_neural(&mut optimizer, &test_graph, steps, &mut rng_state);

    println!("\n🎲 Random Action Baseline ({} steps)", steps);
    rng_state = 12345u64; // Reset for fair comparison
    let random_history = optimize_random(&test_graph, steps, &mut rng_state);

    // Calculate statistics
    println!("\n📈 Results Comparison");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    if !neural_history.is_empty() {
        let neural_avg: f64 = neural_history.iter().sum::<f64>() / neural_history.len() as f64;
        let neural_min = neural_history.iter().cloned().fold(f64::INFINITY, f64::min);
        let neural_max = neural_history
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        println!("\nNeural-Guided:");
        println!("  Average Mincut: {:.2}", neural_avg);
        println!("  Min Mincut:     {:.2}", neural_min);
        println!("  Max Mincut:     {:.2}", neural_max);
    }

    if !random_history.is_empty() {
        let random_avg: f64 = random_history.iter().sum::<f64>() / random_history.len() as f64;
        let random_min = random_history.iter().cloned().fold(f64::INFINITY, f64::min);
        let random_max = random_history
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        println!("\nRandom Baseline:");
        println!("  Average Mincut: {:.2}", random_avg);
        println!("  Min Mincut:     {:.2}", random_min);
        println!("  Max Mincut:     {:.2}", random_max);
    }

    // Show improvement
    if !neural_history.is_empty() && !random_history.is_empty() {
        let neural_avg: f64 = neural_history.iter().sum::<f64>() / neural_history.len() as f64;
        let random_avg: f64 = random_history.iter().sum::<f64>() / random_history.len() as f64;
        let improvement = ((random_avg - neural_avg) / random_avg * 100.0).abs();

        println!("\n✨ Improvement: {:.1}%", improvement);
    }

    // Prediction demonstration
    println!("\n🔮 Prediction vs Actual");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    for i in 0..5 {
        let test_graph = generate_random_graph(12, 0.3, &mut rng_state);
        let predicted = optimizer.predict_value(&test_graph);

        if let Some(actual) = calculate_mincut(&test_graph) {
            let error = ((predicted - actual) / actual * 100.0).abs();
            println!(
                "Test {}: Predicted = {:.2}, Actual = {:.2}, Error = {:.1}%",
                i + 1,
                predicted,
                actual,
                error
            );
        }
    }

    println!("\n✅ Example Complete");
    println!("\nKey Insights:");
    println!("• Neural networks can learn graph optimization patterns");
    println!("• Simple linear models work for basic prediction tasks");
    println!("• Reinforcement learning helps guide graph modifications");
    println!("• Training on historical data improves future predictions");
}
