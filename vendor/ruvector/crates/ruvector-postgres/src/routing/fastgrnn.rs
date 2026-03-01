// FastGRNN - Fast Gated Recurrent Neural Network
//
// Lightweight RNN for real-time routing decisions with minimal compute overhead.
// Based on "FastGRNN: A Fast, Accurate, Stable and Tiny Kilobyte Sized Gated Recurrent Neural Network"

use std::f32;

/// FastGRNN cell for sequence processing with gating mechanisms
#[derive(Clone)]
pub struct FastGRNN {
    /// Input dimension
    input_dim: usize,
    /// Hidden state dimension
    hidden_dim: usize,
    /// Gate weights for input
    w_gate: Vec<f32>,
    /// Gate weights for hidden state
    u_gate: Vec<f32>,
    /// Update weights for input
    w_update: Vec<f32>,
    /// Update weights for hidden state
    u_update: Vec<f32>,
    /// Biases for gate and update
    bias_gate: Vec<f32>,
    bias_update: Vec<f32>,
    /// Zeta parameter for gate scaling
    zeta: f32,
    /// Nu parameter for update scaling
    nu: f32,
}

impl FastGRNN {
    /// Create a new FastGRNN cell with specified dimensions
    pub fn new(input_dim: usize, hidden_dim: usize) -> Self {
        // Initialize with small random weights (Xavier initialization)
        let scale = (2.0 / (input_dim + hidden_dim) as f32).sqrt();

        Self {
            input_dim,
            hidden_dim,
            w_gate: vec![0.1 * scale; input_dim * hidden_dim],
            u_gate: vec![0.1 * scale; hidden_dim * hidden_dim],
            w_update: vec![0.1 * scale; input_dim * hidden_dim],
            u_update: vec![0.1 * scale; hidden_dim * hidden_dim],
            bias_gate: vec![0.0; hidden_dim],
            bias_update: vec![0.0; hidden_dim],
            zeta: 1.0,
            nu: 1.0,
        }
    }

    /// Create FastGRNN from pre-trained weights
    pub fn from_weights(
        input_dim: usize,
        hidden_dim: usize,
        w_gate: Vec<f32>,
        u_gate: Vec<f32>,
        w_update: Vec<f32>,
        u_update: Vec<f32>,
        bias_gate: Vec<f32>,
        bias_update: Vec<f32>,
        zeta: f32,
        nu: f32,
    ) -> Self {
        Self {
            input_dim,
            hidden_dim,
            w_gate,
            u_gate,
            w_update,
            u_update,
            bias_gate,
            bias_update,
            zeta,
            nu,
        }
    }

    /// Perform one step of FastGRNN computation
    ///
    /// # Arguments
    /// * `input` - Input vector of size input_dim
    /// * `hidden` - Previous hidden state of size hidden_dim
    ///
    /// # Returns
    /// New hidden state of size hidden_dim
    pub fn step(&self, input: &[f32], hidden: &[f32]) -> Vec<f32> {
        assert_eq!(input.len(), self.input_dim, "Input dimension mismatch");
        assert_eq!(hidden.len(), self.hidden_dim, "Hidden dimension mismatch");

        let mut new_hidden = vec![0.0; self.hidden_dim];

        // Compute gate: g = sigmoid(W_g * x + U_g * h + b_g)
        let mut gate = vec![0.0; self.hidden_dim];
        self.matmul_add(&self.w_gate, input, &mut gate);
        self.matmul_add(&self.u_gate, hidden, &mut gate);
        for i in 0..self.hidden_dim {
            gate[i] = self.sigmoid(gate[i] + self.bias_gate[i]);
        }

        // Compute update: c = tanh(W_u * x + U_u * h + b_u)
        let mut update = vec![0.0; self.hidden_dim];
        self.matmul_add(&self.w_update, input, &mut update);
        self.matmul_add(&self.u_update, hidden, &mut update);
        for i in 0..self.hidden_dim {
            update[i] = self.tanh(update[i] + self.bias_update[i]);
        }

        // Compute new hidden: h' = (zeta * g + nu) ⊙ h + (1 - zeta * g - nu) ⊙ c
        for i in 0..self.hidden_dim {
            let gate_factor = self.zeta * gate[i] + self.nu;
            let gate_factor = gate_factor.min(1.0).max(0.0); // Clip to [0, 1]
            new_hidden[i] = gate_factor * hidden[i] + (1.0 - gate_factor) * update[i];
        }

        new_hidden
    }

    /// Process a single input and return hidden state (for single-step inference)
    pub fn forward_single(&self, input: &[f32]) -> Vec<f32> {
        let hidden = vec![0.0; self.hidden_dim];
        self.step(input, &hidden)
    }

    /// Process a sequence of inputs
    pub fn forward_sequence(&self, inputs: &[Vec<f32>]) -> Vec<Vec<f32>> {
        let mut hidden = vec![0.0; self.hidden_dim];
        let mut outputs = Vec::with_capacity(inputs.len());

        for input in inputs {
            hidden = self.step(input, &hidden);
            outputs.push(hidden.clone());
        }

        outputs
    }

    /// Matrix-vector multiplication with accumulation: result += W * input
    fn matmul_add(&self, weights: &[f32], input: &[f32], result: &mut [f32]) {
        let rows = result.len();
        let cols = input.len();

        for i in 0..rows {
            for j in 0..cols {
                result[i] += weights[i * cols + j] * input[j];
            }
        }
    }

    /// Sigmoid activation function
    fn sigmoid(&self, x: f32) -> f32 {
        1.0 / (1.0 + (-x).exp())
    }

    /// Hyperbolic tangent activation function
    fn tanh(&self, x: f32) -> f32 {
        x.tanh()
    }

    /// Get input dimension
    pub fn input_dim(&self) -> usize {
        self.input_dim
    }

    /// Get hidden dimension
    pub fn hidden_dim(&self) -> usize {
        self.hidden_dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fastgrnn_creation() {
        let grnn = FastGRNN::new(10, 5);
        assert_eq!(grnn.input_dim(), 10);
        assert_eq!(grnn.hidden_dim(), 5);
    }

    #[test]
    fn test_fastgrnn_step() {
        let grnn = FastGRNN::new(4, 3);
        let input = vec![1.0, 0.5, -0.5, 0.0];
        let hidden = vec![0.1, 0.2, 0.3];

        let new_hidden = grnn.step(&input, &hidden);
        assert_eq!(new_hidden.len(), 3);

        // Check that output is bounded (due to tanh and sigmoid)
        for &h in &new_hidden {
            assert!(h.abs() <= 2.0, "Hidden state should be bounded");
        }
    }

    #[test]
    fn test_fastgrnn_forward_single() {
        let grnn = FastGRNN::new(4, 3);
        let input = vec![1.0, 0.5, -0.5, 0.0];

        let output = grnn.forward_single(&input);
        assert_eq!(output.len(), 3);
    }

    #[test]
    fn test_fastgrnn_sequence() {
        let grnn = FastGRNN::new(4, 3);
        let inputs = vec![
            vec![1.0, 0.5, -0.5, 0.0],
            vec![0.5, 1.0, 0.0, -0.5],
            vec![-0.5, 0.0, 1.0, 0.5],
        ];

        let outputs = grnn.forward_sequence(&inputs);
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].len(), 3);
    }

    #[test]
    fn test_sigmoid() {
        let grnn = FastGRNN::new(1, 1);
        assert!((grnn.sigmoid(0.0) - 0.5).abs() < 1e-6);
        assert!(grnn.sigmoid(10.0) > 0.99);
        assert!(grnn.sigmoid(-10.0) < 0.01);
    }

    #[test]
    fn test_tanh() {
        let grnn = FastGRNN::new(1, 1);
        assert!(grnn.tanh(0.0).abs() < 1e-6);
        assert!(grnn.tanh(10.0) > 0.99);
        assert!(grnn.tanh(-10.0) < -0.99);
    }

    #[test]
    #[should_panic(expected = "Input dimension mismatch")]
    fn test_wrong_input_dimension() {
        let grnn = FastGRNN::new(4, 3);
        let input = vec![1.0, 0.5]; // Wrong size
        let hidden = vec![0.1, 0.2, 0.3];
        grnn.step(&input, &hidden);
    }

    #[test]
    #[should_panic(expected = "Hidden dimension mismatch")]
    fn test_wrong_hidden_dimension() {
        let grnn = FastGRNN::new(4, 3);
        let input = vec![1.0, 0.5, -0.5, 0.0];
        let hidden = vec![0.1, 0.2]; // Wrong size
        grnn.step(&input, &hidden);
    }
}
