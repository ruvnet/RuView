/// Reversible Neural Networks: Toward Zero-Dissipation Learning
///
/// Landauer's principle states that irreversible computation dissipates at least
/// kT ln(2) per bit. Reversible computation can be arbitrarily energy-efficient.
///
/// This module implements:
/// - Reversible layers (bijective transformations)
/// - Coupling layers (RealNVP architecture)
/// - Invertible activation functions
/// - Orthogonal weight constraints
/// - Energy tracking for reversible operations
use std::f64::consts::{LN_2, PI};

/// Reversible layer trait - must be bijective
pub trait ReversibleLayer {
    /// Forward transformation
    fn forward(&self, input: &[f64]) -> Vec<f64>;

    /// Inverse transformation (must satisfy inverse(forward(x)) = x)
    fn inverse(&self, output: &[f64]) -> Vec<f64>;

    /// Jacobian determinant (for probability calculations)
    fn log_det_jacobian(&self, input: &[f64]) -> f64;

    /// Check reversibility (for testing)
    fn verify_reversibility(&self, input: &[f64], epsilon: f64) -> bool {
        let output = self.forward(input);
        let reconstructed = self.inverse(&output);

        for (x, x_recon) in input.iter().zip(reconstructed.iter()) {
            if (x - x_recon).abs() > epsilon {
                return false;
            }
        }
        true
    }
}

/// Invertible activation functions
#[derive(Debug, Clone)]
pub enum InvertibleActivation {
    LeakyReLU { alpha: f64 },
    Tanh,
    Sigmoid,
    Identity,
}

impl InvertibleActivation {
    pub fn activate(&self, x: f64) -> f64 {
        match self {
            InvertibleActivation::LeakyReLU { alpha } => {
                if x >= 0.0 {
                    x
                } else {
                    alpha * x
                }
            }
            InvertibleActivation::Tanh => x.tanh(),
            InvertibleActivation::Sigmoid => 1.0 / (1.0 + (-x).exp()),
            InvertibleActivation::Identity => x,
        }
    }

    pub fn inverse(&self, y: f64) -> f64 {
        match self {
            InvertibleActivation::LeakyReLU { alpha } => {
                if y >= 0.0 {
                    y
                } else {
                    y / alpha
                }
            }
            InvertibleActivation::Tanh => {
                // arctanh(y) = 0.5 * ln((1+y)/(1-y))
                0.5 * ((1.0 + y) / (1.0 - y)).ln()
            }
            InvertibleActivation::Sigmoid => {
                // logit(y) = ln(y / (1-y))
                (y / (1.0 - y)).ln()
            }
            InvertibleActivation::Identity => y,
        }
    }

    pub fn derivative(&self, x: f64) -> f64 {
        match self {
            InvertibleActivation::LeakyReLU { alpha } => {
                if x >= 0.0 {
                    1.0
                } else {
                    *alpha
                }
            }
            InvertibleActivation::Tanh => {
                let t = x.tanh();
                1.0 - t * t
            }
            InvertibleActivation::Sigmoid => {
                let s = self.activate(x);
                s * (1.0 - s)
            }
            InvertibleActivation::Identity => 1.0,
        }
    }
}

/// Coupling layer (RealNVP architecture)
/// Split input: x = [x1, x2]
/// Transform: y1 = x1, y2 = x2 * exp(s(x1)) + t(x1)
/// Where s and t are neural networks
#[derive(Debug, Clone)]
pub struct CouplingLayer {
    /// Split point
    pub split: usize,

    /// Scale network: two layers [layer1, layer2]
    pub scale_weights_1: Vec<Vec<f64>>,
    pub scale_bias_1: Vec<f64>,
    pub scale_weights_2: Vec<Vec<f64>>,
    pub scale_bias_2: Vec<f64>,

    /// Translation network: two layers [layer1, layer2]
    pub translate_weights_1: Vec<Vec<f64>>,
    pub translate_bias_1: Vec<f64>,
    pub translate_weights_2: Vec<Vec<f64>>,
    pub translate_bias_2: Vec<f64>,

    /// Activation function
    pub activation: InvertibleActivation,
}

impl CouplingLayer {
    pub fn new(dim: usize, hidden_dim: usize, split: usize) -> Self {
        assert!(split < dim);

        let dim1 = split;
        let dim2 = dim - split;

        // Initialize scale network: dim1 -> hidden -> dim2
        // Layer 1: dim1 -> hidden_dim
        let scale_weights_1 = vec![vec![(rand::random::<f64>() - 0.5) * 0.1; dim1]; hidden_dim];
        let scale_bias_1 = vec![0.0; hidden_dim];

        // Layer 2: hidden_dim -> dim2
        let scale_weights_2 = vec![vec![(rand::random::<f64>() - 0.5) * 0.1; hidden_dim]; dim2];
        let scale_bias_2 = vec![0.0; dim2];

        // Initialize translation network
        // Layer 1: dim1 -> hidden_dim
        let translate_weights_1 = vec![vec![(rand::random::<f64>() - 0.5) * 0.1; dim1]; hidden_dim];
        let translate_bias_1 = vec![0.0; hidden_dim];

        // Layer 2: hidden_dim -> dim2
        let translate_weights_2 = vec![vec![(rand::random::<f64>() - 0.5) * 0.1; hidden_dim]; dim2];
        let translate_bias_2 = vec![0.0; dim2];

        Self {
            split,
            scale_weights_1,
            scale_bias_1,
            scale_weights_2,
            scale_bias_2,
            translate_weights_1,
            translate_bias_1,
            translate_weights_2,
            translate_bias_2,
            activation: InvertibleActivation::LeakyReLU { alpha: 0.1 },
        }
    }

    fn scale_network(&self, x1: &[f64]) -> Vec<f64> {
        // Two-layer network
        let mut hidden = vec![0.0; self.scale_bias_1.len()];
        for i in 0..hidden.len() {
            for j in 0..x1.len() {
                hidden[i] += self.scale_weights_1[i][j] * x1[j];
            }
            hidden[i] += self.scale_bias_1[i];
            hidden[i] = self.activation.activate(hidden[i]);
        }

        let mut output = vec![0.0; self.scale_bias_2.len()];
        for i in 0..output.len() {
            for j in 0..hidden.len() {
                output[i] += self.scale_weights_2[i][j] * hidden[j];
            }
            output[i] += self.scale_bias_2[i];
        }

        output
    }

    fn translate_network(&self, x1: &[f64]) -> Vec<f64> {
        let mut hidden = vec![0.0; self.translate_bias_1.len()];
        for i in 0..hidden.len() {
            for j in 0..x1.len() {
                hidden[i] += self.translate_weights_1[i][j] * x1[j];
            }
            hidden[i] += self.translate_bias_1[i];
            hidden[i] = self.activation.activate(hidden[i]);
        }

        let mut output = vec![0.0; self.translate_bias_2.len()];
        for i in 0..output.len() {
            for j in 0..hidden.len() {
                output[i] += self.translate_weights_2[i][j] * hidden[j];
            }
            output[i] += self.translate_bias_2[i];
        }

        output
    }
}

impl ReversibleLayer for CouplingLayer {
    fn forward(&self, input: &[f64]) -> Vec<f64> {
        let (x1, x2) = input.split_at(self.split);

        let s = self.scale_network(x1);
        let t = self.translate_network(x1);

        let mut output = Vec::new();

        // y1 = x1 (identity)
        output.extend_from_slice(x1);

        // y2 = x2 * exp(s) + t
        for i in 0..x2.len() {
            output.push(x2[i] * s[i].exp() + t[i]);
        }

        output
    }

    fn inverse(&self, output: &[f64]) -> Vec<f64> {
        let (y1, y2) = output.split_at(self.split);

        let s = self.scale_network(y1);
        let t = self.translate_network(y1);

        let mut input = Vec::new();

        // x1 = y1 (identity)
        input.extend_from_slice(y1);

        // x2 = (y2 - t) * exp(-s)
        for i in 0..y2.len() {
            input.push((y2[i] - t[i]) * (-s[i]).exp());
        }

        input
    }

    fn log_det_jacobian(&self, input: &[f64]) -> f64 {
        let x1 = &input[..self.split];
        let s = self.scale_network(x1);

        // Jacobian is triangular, det = product of diagonal = exp(sum(s))
        s.iter().sum()
    }
}

/// Orthogonal linear layer (preserves energy)
/// W is orthogonal: W^T W = I
#[derive(Debug, Clone)]
pub struct OrthogonalLayer {
    /// Orthogonal weight matrix (stored as rotation angles)
    pub rotation_angles: Vec<f64>,
    pub dim: usize,
}

impl OrthogonalLayer {
    pub fn new(dim: usize) -> Self {
        // Number of rotation angles for dim × dim orthogonal matrix
        let n_rotations = dim * (dim - 1) / 2;
        let rotation_angles = (0..n_rotations)
            .map(|_| (rand::random::<f64>() - 0.5) * 2.0 * PI)
            .collect();

        Self {
            rotation_angles,
            dim,
        }
    }

    /// Build orthogonal matrix from rotation angles (Givens rotations)
    fn get_matrix(&self) -> Vec<Vec<f64>> {
        let mut matrix = vec![vec![0.0; self.dim]; self.dim];

        // Start with identity
        for i in 0..self.dim {
            matrix[i][i] = 1.0;
        }

        // Apply Givens rotations
        let mut angle_idx = 0;
        for i in 0..self.dim {
            for j in (i + 1)..self.dim {
                if angle_idx < self.rotation_angles.len() {
                    let theta = self.rotation_angles[angle_idx];
                    let c = theta.cos();
                    let s = theta.sin();

                    // Apply rotation in (i,j) plane
                    let mut new_matrix = matrix.clone();
                    for k in 0..self.dim {
                        new_matrix[k][i] = c * matrix[k][i] - s * matrix[k][j];
                        new_matrix[k][j] = s * matrix[k][i] + c * matrix[k][j];
                    }
                    matrix = new_matrix;

                    angle_idx += 1;
                }
            }
        }

        matrix
    }

    fn matrix_multiply(&self, matrix: &[Vec<f64>], vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; vec.len()];
        for i in 0..matrix.len() {
            for j in 0..vec.len() {
                result[i] += matrix[i][j] * vec[j];
            }
        }
        result
    }

    fn transpose(&self, matrix: &[Vec<f64>]) -> Vec<Vec<f64>> {
        let mut transposed = vec![vec![0.0; matrix.len()]; matrix[0].len()];
        for i in 0..matrix.len() {
            for j in 0..matrix[0].len() {
                transposed[j][i] = matrix[i][j];
            }
        }
        transposed
    }
}

impl ReversibleLayer for OrthogonalLayer {
    fn forward(&self, input: &[f64]) -> Vec<f64> {
        let matrix = self.get_matrix();
        self.matrix_multiply(&matrix, input)
    }

    fn inverse(&self, output: &[f64]) -> Vec<f64> {
        // For orthogonal matrix: W^-1 = W^T
        let matrix = self.get_matrix();
        let transposed = self.transpose(&matrix);
        self.matrix_multiply(&transposed, output)
    }

    fn log_det_jacobian(&self, _input: &[f64]) -> f64 {
        // Orthogonal matrix has determinant ±1, so log|det| = 0
        0.0
    }
}

/// Reversible neural network (stack of reversible layers)
pub struct ReversibleNetwork {
    pub layers: Vec<Box<dyn ReversibleLayer>>,
    pub dim: usize,
}

impl ReversibleNetwork {
    pub fn new(dim: usize) -> Self {
        Self {
            layers: Vec::new(),
            dim,
        }
    }

    pub fn add_coupling_layer(&mut self, hidden_dim: usize, split: usize) {
        self.layers
            .push(Box::new(CouplingLayer::new(self.dim, hidden_dim, split)));
    }

    pub fn add_orthogonal_layer(&mut self) {
        self.layers.push(Box::new(OrthogonalLayer::new(self.dim)));
    }

    /// Forward pass through all layers
    pub fn forward(&self, input: &[f64]) -> Vec<f64> {
        let mut x = input.to_vec();
        for layer in &self.layers {
            x = layer.forward(&x);
        }
        x
    }

    /// Inverse pass (reconstruct input from output)
    pub fn inverse(&self, output: &[f64]) -> Vec<f64> {
        let mut x = output.to_vec();
        for layer in self.layers.iter().rev() {
            x = layer.inverse(&x);
        }
        x
    }

    /// Total log determinant of Jacobian
    pub fn log_det_jacobian(&self, input: &[f64]) -> f64 {
        let mut total_log_det = 0.0;
        let mut x = input.to_vec();

        for layer in &self.layers {
            total_log_det += layer.log_det_jacobian(&x);
            x = layer.forward(&x);
        }

        total_log_det
    }

    /// Verify end-to-end reversibility
    pub fn verify_reversibility(&self, input: &[f64], epsilon: f64) -> bool {
        let output = self.forward(input);
        let reconstructed = self.inverse(&output);

        for (x, x_recon) in input.iter().zip(reconstructed.iter()) {
            if (x - x_recon).abs() > epsilon {
                return false;
            }
        }
        true
    }
}

/// Energy tracker for reversible computation
#[derive(Debug, Clone)]
pub struct ReversibleEnergyTracker {
    /// Temperature (K)
    pub temperature: f64,

    /// Total energy dissipated (J)
    pub energy_dissipated: f64,

    /// Number of reversible operations
    pub reversible_ops: usize,

    /// Number of irreversible operations (measurements)
    pub irreversible_ops: usize,
}

impl ReversibleEnergyTracker {
    pub fn new(temperature: f64) -> Self {
        Self {
            temperature,
            energy_dissipated: 0.0,
            reversible_ops: 0,
            irreversible_ops: 0,
        }
    }

    /// Record reversible operation (adiabatic, near-zero energy)
    pub fn record_reversible(&mut self, adiabatic_factor: f64) {
        // Energy ~ 1/τ² for adiabatic time τ
        let k = 1.380649e-23;
        let energy = k * self.temperature / (adiabatic_factor * adiabatic_factor);
        self.energy_dissipated += energy;
        self.reversible_ops += 1;
    }

    /// Record irreversible operation (measurement/readout)
    pub fn record_irreversible(&mut self, bits: f64) {
        let k = 1.380649e-23;
        let energy = k * self.temperature * LN_2 * bits;
        self.energy_dissipated += energy;
        self.irreversible_ops += 1;
    }

    /// Energy saved compared to irreversible computation
    pub fn energy_savings(&self, total_bits: f64) -> f64 {
        let k = 1.380649e-23;
        let irreversible_cost = k * self.temperature * LN_2 * total_bits;
        irreversible_cost - self.energy_dissipated
    }

    pub fn report(&self) -> String {
        format!(
            "Reversible Computation Energy Report:\n\
             ------------------------------------\n\
             Temperature: {:.2} K\n\
             Total energy dissipated: {:.3e} J\n\
             Reversible operations: {}\n\
             Irreversible operations: {}\n\
             Avg energy per op: {:.3e} J\n",
            self.temperature,
            self.energy_dissipated,
            self.reversible_ops,
            self.irreversible_ops,
            self.energy_dissipated / (self.reversible_ops + self.irreversible_ops) as f64
        )
    }
}

// Mock rand
mod rand {
    pub fn random<T>() -> f64 {
        0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invertible_activation() {
        let leaky_relu = InvertibleActivation::LeakyReLU { alpha: 0.1 };

        let x = 2.0;
        let y = leaky_relu.activate(x);
        let x_recon = leaky_relu.inverse(y);
        assert!((x - x_recon).abs() < 1e-10);

        let x_neg = -2.0;
        let y_neg = leaky_relu.activate(x_neg);
        let x_neg_recon = leaky_relu.inverse(y_neg);
        assert!((x_neg - x_neg_recon).abs() < 1e-10);
    }

    #[test]
    fn test_coupling_layer_reversibility() {
        let layer = CouplingLayer::new(4, 8, 2);
        let input = vec![1.0, -0.5, 0.3, 0.7];

        assert!(layer.verify_reversibility(&input, 1e-6));
    }

    #[test]
    fn test_orthogonal_layer_reversibility() {
        let layer = OrthogonalLayer::new(4);
        let input = vec![1.0, 2.0, 3.0, 4.0];

        assert!(layer.verify_reversibility(&input, 1e-6));
    }

    #[test]
    fn test_orthogonal_layer_energy_preservation() {
        let layer = OrthogonalLayer::new(4);
        let input = vec![1.0, 2.0, 3.0, 4.0];

        // Compute input energy (L2 norm squared)
        let input_energy: f64 = input.iter().map(|x| x * x).sum();

        let output = layer.forward(&input);
        let output_energy: f64 = output.iter().map(|x| x * x).sum();

        // Orthogonal transformation preserves energy
        assert!((input_energy - output_energy).abs() < 1e-6);
    }

    #[test]
    fn test_reversible_network() {
        let mut network = ReversibleNetwork::new(4);
        network.add_coupling_layer(8, 2);
        network.add_orthogonal_layer();
        network.add_coupling_layer(8, 2);

        let input = vec![1.0, -0.5, 0.3, 0.7];

        assert!(network.verify_reversibility(&input, 1e-5));
    }

    #[test]
    fn test_energy_tracker() {
        let mut tracker = ReversibleEnergyTracker::new(300.0);

        // Perform 1000 reversible operations
        for _ in 0..1000 {
            tracker.record_reversible(100.0);
        }

        // Perform 10 irreversible operations (1 bit each)
        for _ in 0..10 {
            tracker.record_irreversible(1.0);
        }

        // Most energy should come from irreversible ops
        let k = 1.380649e-23;
        let landauer_per_bit = k * 300.0 * LN_2;
        let expected_irreversible = 10.0 * landauer_per_bit;

        assert!(tracker.energy_dissipated > expected_irreversible);
        assert!(tracker.energy_dissipated < expected_irreversible * 2.0);
    }
}

/// Example: Reversible autoencoder
pub fn example_reversible_autoencoder() {
    println!("=== Reversible Neural Network Example ===\n");

    let mut network = ReversibleNetwork::new(8);

    // Build network: coupling + orthogonal + coupling
    network.add_coupling_layer(16, 4);
    network.add_orthogonal_layer();
    network.add_coupling_layer(16, 4);
    network.add_orthogonal_layer();

    println!("Network architecture:");
    println!("  - Coupling layer (split at 4, hidden dim 16)");
    println!("  - Orthogonal layer (8x8)");
    println!("  - Coupling layer (split at 4, hidden dim 16)");
    println!("  - Orthogonal layer (8x8)\n");

    // Test reversibility
    let input = vec![1.0, -0.5, 0.3, 0.7, -0.2, 0.9, 0.1, -0.4];
    println!("Input: {:?}\n", input);

    let output = network.forward(&input);
    println!("Encoded: {:?}\n", output);

    let reconstructed = network.inverse(&output);
    println!("Reconstructed: {:?}\n", reconstructed);

    // Check reconstruction error
    let mut error = 0.0;
    for (x, x_recon) in input.iter().zip(reconstructed.iter()) {
        error += (x - x_recon).abs();
    }
    println!("Reconstruction error: {:.2e}\n", error);

    // Energy tracking
    let mut tracker = ReversibleEnergyTracker::new(300.0);

    // Forward pass (reversible)
    for _ in 0..network.layers.len() {
        tracker.record_reversible(100.0);
    }

    // Readout (irreversible)
    tracker.record_irreversible(8.0 * 32.0); // 8 values × 32 bits

    println!("{}", tracker.report());

    // Compare to fully irreversible computation
    let total_bits = 8.0 * 32.0 * network.layers.len() as f64;
    let savings = tracker.energy_savings(total_bits);
    println!(
        "Energy savings vs irreversible: {:.3e} J ({:.1}%)",
        savings,
        100.0 * savings / (tracker.energy_dissipated + savings)
    );
}
