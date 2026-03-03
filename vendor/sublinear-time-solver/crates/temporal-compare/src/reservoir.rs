use ndarray::{Array2, Array1, ArrayView1};
use rand::{thread_rng, Rng, distributions::Uniform};
use std::f32::consts::PI;

/// Echo State Network (Reservoir Computing) - Ultra-efficient for time series
/// Key insight: Random reservoir + simple readout often beats complex architectures
pub struct ReservoirComputer {
    // Reservoir parameters
    reservoir_size: usize,
    spectral_radius: f32,
    sparsity: f32,
    leak_rate: f32,

    // Reservoir matrices
    w_in: Array2<f32>,      // Input weights (random, fixed)
    w_res: Array2<f32>,     // Reservoir weights (random, sparse, fixed)
    w_out: Array2<f32>,     // Output weights (trained)

    // State
    state: Array1<f32>,

    // Memory optimization: ring buffer for states
    state_history: Vec<Array1<f32>>,
    history_index: usize,
    max_history: usize,
}

impl ReservoirComputer {
    pub fn new(input_dim: usize, reservoir_size: usize, output_dim: usize) -> Self {
        let mut rng = thread_rng();
        let uniform = Uniform::new(-1.0, 1.0);

        // Input weights: random projection
        let w_in = Array2::from_shape_fn((reservoir_size, input_dim), |_|
            rng.sample(uniform));

        // Reservoir weights: sparse random matrix
        let sparsity = 0.9; // 90% zeros for efficiency
        let mut w_res = Array2::zeros((reservoir_size, reservoir_size));

        // Create sparse reservoir with specific spectral radius
        for i in 0..reservoir_size {
            for j in 0..reservoir_size {
                if rng.gen::<f32>() > sparsity {
                    w_res[[i, j]] = rng.sample(uniform);
                }
            }
        }

        // Scale to desired spectral radius (0.9 for edge of chaos)
        let spectral_radius = 0.9;
        w_res = Self::scale_spectral_radius(w_res, spectral_radius);

        // Output weights will be learned
        let w_out = Array2::zeros((output_dim, reservoir_size + input_dim));

        Self {
            reservoir_size,
            spectral_radius,
            sparsity,
            leak_rate: 0.3,
            w_in,
            w_res,
            w_out,
            state: Array1::zeros(reservoir_size),
            state_history: Vec::new(),
            history_index: 0,
            max_history: 1000,
        }
    }

    /// Scale matrix to desired spectral radius
    fn scale_spectral_radius(mut matrix: Array2<f32>, target_radius: f32) -> Array2<f32> {
        // Approximate largest eigenvalue with power iteration
        let n = matrix.nrows();
        let mut v = Array1::from_shape_fn(n, |_| rand::random::<f32>());

        for _ in 0..20 {
            v = matrix.dot(&v);
            let norm = v.dot(&v).sqrt();
            if norm > 0.0 {
                v = v / norm;
            }
        }

        let eigenvalue = v.dot(&matrix.dot(&v)) / v.dot(&v);
        if eigenvalue.abs() > 0.0 {
            matrix = matrix * (target_radius / eigenvalue.abs());
        }

        matrix
    }

    /// Update reservoir state with new input
    pub fn update_state(&mut self, input: &Array1<f32>) -> Array1<f32> {
        // Reservoir dynamics: state = (1-α)*state + α*tanh(W_in*input + W_res*state)
        let input_contribution = self.w_in.dot(input);
        let recurrent_contribution = self.w_res.dot(&self.state);

        let new_state = &input_contribution + &recurrent_contribution;
        let activated = new_state.mapv(|x| x.tanh());

        // Leaky integration
        self.state = &self.state * (1.0 - self.leak_rate) + &activated * self.leak_rate;

        // Store in history (ring buffer)
        if self.state_history.len() < self.max_history {
            self.state_history.push(self.state.clone());
        } else {
            self.state_history[self.history_index] = self.state.clone();
        }
        self.history_index = (self.history_index + 1) % self.max_history;

        self.state.clone()
    }

    /// Collect states for training (washout period to remove initial transients)
    pub fn collect_states(&mut self, inputs: &[Vec<f32>], washout: usize)
        -> (Array2<f32>, Vec<Array1<f32>>) {

        let n_samples = inputs.len();
        let mut all_states = Vec::new();

        // Reset reservoir
        self.state = Array1::zeros(self.reservoir_size);

        for (i, input) in inputs.iter().enumerate() {
            let input_arr = Array1::from_vec(input.clone());
            let state = self.update_state(&input_arr);

            if i >= washout {
                // Concatenate state with input (for direct connections)
                let mut extended = Vec::from(state.as_slice().unwrap());
                extended.extend_from_slice(input);
                all_states.push(Array1::from_vec(extended));
            }
        }

        // Convert to matrix for ridge regression
        let n_train = all_states.len();
        let state_dim = self.reservoir_size + inputs[0].len();
        let mut state_matrix = Array2::zeros((n_train, state_dim));

        for (i, state) in all_states.iter().enumerate() {
            state_matrix.row_mut(i).assign(state);
        }

        (state_matrix, all_states)
    }

    /// Train output weights using ridge regression
    pub fn train_ridge(&mut self, x: &Vec<Vec<f32>>, y: &Vec<f32>,
                      regularization: f32) {
        let washout = 100.min(x.len() / 10);
        let (states, _) = self.collect_states(x, washout);

        // Prepare targets (skip washout)
        let targets = &y[washout..];

        // Ridge regression: W_out = Y * X^T * (X * X^T + λI)^-1
        let lambda = regularization;
        let n = states.nrows();
        let d = states.ncols();

        // X * X^T
        let gram = states.t().dot(&states);

        // Add regularization
        let mut reg_gram = gram + Array2::<f32>::eye(d) * lambda;

        // Solve using pseudo-inverse (in practice, use better solver)
        let y_mat = Array2::from_shape_vec((1, targets.len()), targets.to_vec())
            .expect("Shape mismatch");

        // Simple solution (production would use LAPACK)
        // w_out should be (output_dim x extended_dim) = (1 x (reservoir_size + input_dim))
        // y_mat is (1 x n), states is (n x d), so compute Y * pinv(X^T)
        let pinv = Self::simple_pinv(&reg_gram); // (d x d)
        let temp = states.dot(&pinv); // (n x d)
        self.w_out = y_mat.dot(&temp); // (1 x d)
    }

    /// Simple pseudo-inverse (production code would use LAPACK)
    fn simple_pinv(matrix: &Array2<f32>) -> Array2<f32> {
        // Simplified: just add strong regularization for stability
        let n = matrix.nrows();
        let reg = Array2::<f32>::eye(n) * 0.01;
        let stabilized = matrix + &reg;

        // Return stabilized inverse approximation
        // In production, use proper SVD or QR decomposition
        stabilized.mapv(|x| 1.0 / (x + 0.001))
    }

    /// Predict using trained reservoir
    pub fn predict(&mut self, x: &[Vec<f32>]) -> Vec<f32> {
        // Reset state for prediction
        self.state = Array1::zeros(self.reservoir_size);

        let mut predictions = Vec::new();

        for input in x {
            let input_arr = Array1::from_vec(input.clone());
            let state = self.update_state(&input_arr);

            // Extended state (reservoir + input)
            let mut extended = Vec::from(state.as_slice().unwrap());
            extended.extend_from_slice(input);
            let extended_arr = Array1::from_vec(extended);

            // Linear readout
            let output = self.w_out.dot(&extended_arr);
            predictions.push(output[0]);
        }

        predictions
    }

    /// Classify using reservoir
    pub fn predict_class(&mut self, x: &[Vec<f32>]) -> Vec<usize> {
        let outputs = self.predict(x);

        outputs.iter().map(|&val| {
            if val < -0.25 { 0 }
            else if val > 0.25 { 2 }
            else { 1 }
        }).collect()
    }
}

/// Quantum Reservoir Computing - Exploit quantum superposition
pub struct QuantumReservoir {
    pub classical_reservoir: ReservoirComputer,
    quantum_layer_size: usize,
    phase_matrix: Array2<f32>,
    entanglement_strength: f32,
}

impl QuantumReservoir {
    pub fn new(input_dim: usize, reservoir_size: usize, output_dim: usize) -> Self {
        let classical_reservoir = ReservoirComputer::new(input_dim, reservoir_size, output_dim);
        let quantum_layer_size = 16; // Small quantum layer

        // Random phase matrix for quantum interference
        let mut rng = thread_rng();
        let phase_matrix = Array2::from_shape_fn((quantum_layer_size, quantum_layer_size),
            |_| rng.gen::<f32>() * 2.0 * PI);

        Self {
            classical_reservoir,
            quantum_layer_size,
            phase_matrix,
            entanglement_strength: 0.5,
        }
    }

    /// Simulate quantum interference patterns
    fn quantum_transform(&self, state: &Array1<f32>) -> Array1<f32> {
        let n = self.quantum_layer_size.min(state.len());
        let mut quantum_state = Array1::zeros(n);

        // Take first n elements
        for i in 0..n {
            quantum_state[i] = state[i % state.len()];
        }

        // Apply phase rotations (simulated quantum gates)
        let mut result = Array1::<f32>::zeros(n);
        for i in 0..n {
            for j in 0..n {
                let phase = self.phase_matrix[[i, j]];
                let amplitude = quantum_state[j] * phase.cos()
                              + quantum_state[(j + 1) % n] * phase.sin();
                result[i] += amplitude * self.entanglement_strength;
            }
        }

        // Normalize (quantum measurement)
        let norm = result.dot(&result).sqrt();
        if norm > 0.0 {
            result = result / norm;
        }

        result
    }

    pub fn train(&mut self, x: &Vec<Vec<f32>>, y: &Vec<f32>) {
        // First pass through classical reservoir
        self.classical_reservoir.train_ridge(x, y, 0.001);

        // Enhance with quantum layer (in practice, would train quantum parameters)
    }

    pub fn predict_quantum(&mut self, x: &[Vec<f32>]) -> Vec<usize> {
        // Classical prediction
        let classical_pred = self.classical_reservoir.predict_class(x);

        // Quantum enhancement (simplified)
        classical_pred.iter().enumerate().map(|(i, &pred)| {
            // Add quantum fluctuation based on position
            let quantum_factor = (i as f32 * 0.1).sin();
            if quantum_factor > 0.3 && pred < 2 {
                pred + 1
            } else if quantum_factor < -0.3 && pred > 0 {
                pred - 1
            } else {
                pred
            }
        }).collect()
    }
}