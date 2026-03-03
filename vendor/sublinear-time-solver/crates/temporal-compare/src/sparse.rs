use ndarray::{Array1, Array2};
use rand::{thread_rng, Rng};
use std::collections::HashMap;

/// Sparse Neural Network with dynamic sparsity
/// Only keeps top-k% connections active
pub struct SparseNetwork {
    // Sparse weight storage
    w1_indices: Vec<(usize, usize)>,  // Active connections
    w1_values: Vec<f32>,              // Weight values
    w2_indices: Vec<(usize, usize)>,
    w2_values: Vec<f32>,

    // Biases (dense)
    b1: Array1<f32>,
    b2: Array1<f32>,

    // Dimensions
    input_dim: usize,
    hidden_dim: usize,
    output_dim: usize,
    sparsity: f32,  // Fraction of weights to keep

    // Hidden activations
    hidden: Array1<f32>,

    // Pruning statistics
    pruning_threshold: f32,
    pruned_count: usize,
}

impl SparseNetwork {
    pub fn new(input: usize, hidden: usize, output: usize, sparsity: f32) -> Self {
        let mut rng = thread_rng();

        // Initialize with random sparse connections
        let n_connections1 = ((input * hidden) as f32 * sparsity) as usize;
        let n_connections2 = ((hidden * output) as f32 * sparsity) as usize;

        let mut w1_indices = Vec::new();
        let mut w1_values = Vec::new();

        // Random sparse initialization for layer 1
        let scale1 = (2.0 / input as f32).sqrt();
        let mut used1 = std::collections::HashSet::new();

        while w1_indices.len() < n_connections1 {
            let i = rng.gen_range(0..hidden);
            let j = rng.gen_range(0..input);
            if used1.insert((i, j)) {
                w1_indices.push((i, j));
                w1_values.push(rng.gen::<f32>() * scale1 - scale1/2.0);
            }
        }

        // Layer 2
        let mut w2_indices = Vec::new();
        let mut w2_values = Vec::new();
        let scale2 = (2.0 / hidden as f32).sqrt();
        let mut used2 = std::collections::HashSet::new();

        while w2_indices.len() < n_connections2 {
            let i = rng.gen_range(0..output);
            let j = rng.gen_range(0..hidden);
            if used2.insert((i, j)) {
                w2_indices.push((i, j));
                w2_values.push(rng.gen::<f32>() * scale2 - scale2/2.0);
            }
        }

        Self {
            w1_indices,
            w1_values,
            w2_indices,
            w2_values,
            b1: Array1::zeros(hidden),
            b2: Array1::zeros(output),
            input_dim: input,
            hidden_dim: hidden,
            output_dim: output,
            sparsity,
            hidden: Array1::zeros(hidden),
            pruning_threshold: 0.01,
            pruned_count: 0,
        }
    }

    /// Sparse forward pass
    pub fn forward(&mut self, x: &[f32]) -> Vec<f32> {
        // Reset hidden
        self.hidden.fill(0.0);

        // Sparse matrix-vector multiplication for layer 1
        for ((&(i, j), &val)) in self.w1_indices.iter().zip(&self.w1_values) {
            if j < x.len() {
                self.hidden[i] += x[j] * val;
            }
        }

        // Add bias and apply ReLU
        self.hidden = &self.hidden + &self.b1;
        self.hidden.mapv_inplace(|x| x.max(0.0));

        // Layer 2
        let mut output = vec![0.0; self.output_dim];
        for ((&(i, j), &val)) in self.w2_indices.iter().zip(&self.w2_values) {
            output[i] += self.hidden[j] * val;
        }

        // Add bias
        for i in 0..self.output_dim {
            output[i] += self.b2[i];
        }

        output
    }

    /// Dynamic pruning - remove small weights
    pub fn prune_weights(&mut self, threshold: f32) {
        // Prune layer 1
        let mut new_indices1 = Vec::new();
        let mut new_values1 = Vec::new();

        for (idx, val) in self.w1_indices.iter().zip(&self.w1_values) {
            if val.abs() > threshold {
                new_indices1.push(*idx);
                new_values1.push(*val);
            }
        }

        let pruned1 = self.w1_indices.len() - new_indices1.len();
        self.w1_indices = new_indices1;
        self.w1_values = new_values1;

        // Prune layer 2
        let mut new_indices2 = Vec::new();
        let mut new_values2 = Vec::new();

        for (idx, val) in self.w2_indices.iter().zip(&self.w2_values) {
            if val.abs() > threshold {
                new_indices2.push(*idx);
                new_values2.push(*val);
            }
        }

        let pruned2 = self.w2_indices.len() - new_indices2.len();
        self.w2_indices = new_indices2;
        self.w2_values = new_values2;

        self.pruned_count += pruned1 + pruned2;
        self.pruning_threshold = threshold;
    }

    /// Regrow connections (lottery ticket hypothesis)
    pub fn regrow_connections(&mut self, n_regrow: usize) {
        let mut rng = thread_rng();

        // Regrow layer 1
        let mut used1: std::collections::HashSet<_> = self.w1_indices.iter().cloned().collect();
        let scale1 = (2.0 / self.input_dim as f32).sqrt();

        for _ in 0..n_regrow/2 {
            let i = rng.gen_range(0..self.hidden_dim);
            let j = rng.gen_range(0..self.input_dim);
            if used1.insert((i, j)) {
                self.w1_indices.push((i, j));
                self.w1_values.push(rng.gen::<f32>() * scale1 - scale1/2.0);
            }
        }

        // Regrow layer 2
        let mut used2: std::collections::HashSet<_> = self.w2_indices.iter().cloned().collect();
        let scale2 = (2.0 / self.hidden_dim as f32).sqrt();

        for _ in 0..n_regrow/2 {
            let i = rng.gen_range(0..self.output_dim);
            let j = rng.gen_range(0..self.hidden_dim);
            if used2.insert((i, j)) {
                self.w2_indices.push((i, j));
                self.w2_values.push(rng.gen::<f32>() * scale2 - scale2/2.0);
            }
        }
    }

    /// Train with sparse gradient updates
    pub fn train(&mut self, x: &[Vec<f32>], y: &[f32], epochs: usize, lr: f32) {
        for epoch in 0..epochs {
            let mut total_loss = 0.0;

            for (xi, &yi) in x.iter().zip(y.iter()) {
                let output = self.forward(xi);
                let pred = if self.output_dim == 1 { output[0] } else { output[0] };
                let error = pred - yi;
                total_loss += error * error;

                // Sparse backpropagation
                self.sparse_backward(xi, error, lr);
            }

            // Dynamic sparsity - prune and regrow periodically
            if epoch > 0 && epoch % 10 == 0 {
                let old_count = self.w1_indices.len() + self.w2_indices.len();
                self.prune_weights(self.pruning_threshold);
                let pruned = old_count - (self.w1_indices.len() + self.w2_indices.len());
                if pruned > 0 {
                    self.regrow_connections(pruned);
                }
            }

            if epoch % 100 == 0 {
                println!("Sparse epoch {}: loss={:.6}, active_weights={}",
                         epoch, total_loss / x.len() as f32,
                         self.w1_indices.len() + self.w2_indices.len());
            }
        }
    }

    fn sparse_backward(&mut self, x: &[f32], error: f32, lr: f32) {
        // Output gradient
        let grad_out = error;

        // Update layer 2 weights (sparse)
        let mut grad_hidden = Array1::zeros(self.hidden_dim);

        for i in 0..self.w2_indices.len() {
            let (out_idx, hid_idx) = self.w2_indices[i];
            if out_idx == 0 {  // For single output
                // Update weight
                self.w2_values[i] -= lr * grad_out * self.hidden[hid_idx];
                // Accumulate gradient for hidden layer
                grad_hidden[hid_idx] += self.w2_values[i] * grad_out;
            }
        }

        // Update bias
        self.b2[0] -= lr * grad_out;

        // Apply ReLU gradient
        grad_hidden.mapv_inplace(|g| if self.hidden[0] > 0.0 { g } else { 0.0 });

        // Update layer 1 weights (sparse)
        for i in 0..self.w1_indices.len() {
            let (hid_idx, in_idx) = self.w1_indices[i];
            if in_idx < x.len() {
                self.w1_values[i] -= lr * grad_hidden[hid_idx] * x[in_idx];
            }
        }

        // Update hidden bias
        self.b1 = &self.b1 - &grad_hidden * lr;
    }

    pub fn predict(&mut self, x: &[Vec<f32>]) -> Vec<f32> {
        x.iter().map(|xi| {
            let output = self.forward(xi);
            if self.output_dim == 1 { output[0] } else { output[0] }
        }).collect()
    }

    pub fn predict_class(&mut self, x: &[Vec<f32>]) -> Vec<usize> {
        self.predict(x).iter().map(|&y| {
            if y < -0.25 { 0 }
            else if y > 0.25 { 2 }
            else { 1 }
        }).collect()
    }

    pub fn get_sparsity_stats(&self) -> (usize, usize, f32) {
        let active = self.w1_indices.len() + self.w2_indices.len();
        let total = self.input_dim * self.hidden_dim + self.hidden_dim * self.output_dim;
        let sparsity = active as f32 / total as f32;
        (active, self.pruned_count, sparsity)
    }
}

/// Lottery Ticket Network - finds winning sparse subnetworks
pub struct LotteryTicketNetwork {
    base_network: SparseNetwork,
    initial_weights1: HashMap<(usize, usize), f32>,
    initial_weights2: HashMap<(usize, usize), f32>,
    winning_mask1: HashMap<(usize, usize), bool>,
    winning_mask2: HashMap<(usize, usize), bool>,
    iteration: usize,
}

impl LotteryTicketNetwork {
    pub fn new(input: usize, hidden: usize, output: usize) -> Self {
        let base = SparseNetwork::new(input, hidden, output, 1.0); // Start dense

        // Store initial weights
        let mut initial_weights1 = HashMap::new();
        for (&idx, &val) in base.w1_indices.iter().zip(&base.w1_values) {
            initial_weights1.insert(idx, val);
        }

        let mut initial_weights2 = HashMap::new();
        for (&idx, &val) in base.w2_indices.iter().zip(&base.w2_values) {
            initial_weights2.insert(idx, val);
        }

        Self {
            base_network: base,
            initial_weights1,
            initial_weights2,
            winning_mask1: HashMap::new(),
            winning_mask2: HashMap::new(),
            iteration: 0,
        }
    }

    /// Iterative magnitude pruning to find winning tickets
    pub fn find_winning_ticket(&mut self, x: &[Vec<f32>], y: &[f32],
                               prune_rate: f32, iterations: usize) {
        for iter in 0..iterations {
            println!("Lottery iteration {}/{}", iter + 1, iterations);

            // Reset to initial weights with current mask
            self.reset_to_initial();

            // Train
            self.base_network.train(x, y, 100, 0.01);

            // Prune based on magnitude
            self.magnitude_prune(prune_rate);

            self.iteration += 1;
        }
    }

    fn reset_to_initial(&mut self) {
        // Reset to initial weights, keeping only winning connections
        let mut new_indices1 = Vec::new();
        let mut new_values1 = Vec::new();

        for (&idx, &init_val) in &self.initial_weights1 {
            if self.iteration == 0 || self.winning_mask1.get(&idx) == Some(&true) {
                new_indices1.push(idx);
                new_values1.push(init_val);
            }
        }

        self.base_network.w1_indices = new_indices1;
        self.base_network.w1_values = new_values1;

        // Layer 2
        let mut new_indices2 = Vec::new();
        let mut new_values2 = Vec::new();

        for (&idx, &init_val) in &self.initial_weights2 {
            if self.iteration == 0 || self.winning_mask2.get(&idx) == Some(&true) {
                new_indices2.push(idx);
                new_values2.push(init_val);
            }
        }

        self.base_network.w2_indices = new_indices2;
        self.base_network.w2_values = new_values2;
    }

    fn magnitude_prune(&mut self, prune_rate: f32) {
        // Collect all weight magnitudes
        let mut magnitudes: Vec<f32> = self.base_network.w1_values.iter()
            .chain(&self.base_network.w2_values)
            .map(|v| v.abs())
            .collect();

        magnitudes.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let cutoff_idx = (magnitudes.len() as f32 * prune_rate) as usize;
        let threshold = if cutoff_idx < magnitudes.len() {
            magnitudes[cutoff_idx]
        } else {
            0.0
        };

        // Update masks
        self.winning_mask1.clear();
        for (&idx, &val) in self.base_network.w1_indices.iter()
            .zip(&self.base_network.w1_values) {
            self.winning_mask1.insert(idx, val.abs() > threshold);
        }

        self.winning_mask2.clear();
        for (&idx, &val) in self.base_network.w2_indices.iter()
            .zip(&self.base_network.w2_values) {
            self.winning_mask2.insert(idx, val.abs() > threshold);
        }
    }

    pub fn predict(&mut self, x: &[Vec<f32>]) -> Vec<f32> {
        self.base_network.predict(x)
    }

    pub fn predict_class(&mut self, x: &[Vec<f32>]) -> Vec<usize> {
        self.base_network.predict_class(x)
    }
}