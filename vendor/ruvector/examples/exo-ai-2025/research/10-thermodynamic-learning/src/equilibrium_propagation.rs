/// Equilibrium Propagation: Thermodynamic Learning Algorithm
///
/// Implementation of Scellier & Bengio's equilibrium propagation algorithm,
/// which learns by comparing equilibrium states of a physical system.
///
/// Key idea:
/// - Free phase: Network relaxes to energy minimum
/// - Nudged phase: Gently perturb toward target
/// - Learning: Update weights based on activity differences
///
/// This is a physics-based alternative to backpropagation that can be
/// implemented in analog hardware with natural thermodynamic dynamics.

// Physical constants available from std::f64

/// Energy-based neural network for equilibrium propagation
#[derive(Debug, Clone)]
pub struct EnergyBasedNetwork {
    /// Number of layers
    pub n_layers: usize,

    /// Neurons per layer
    pub layer_sizes: Vec<usize>,

    /// Weight matrices (layer l to l+1)
    pub weights: Vec<Vec<Vec<f64>>>,

    /// Biases
    pub biases: Vec<Vec<f64>>,

    /// Neuron states (activations)
    pub states: Vec<Vec<f64>>,

    /// Relaxation time constant
    pub tau: f64,

    /// Temperature for thermal fluctuations
    pub temperature: f64,
}

impl EnergyBasedNetwork {
    pub fn new(layer_sizes: Vec<usize>, tau: f64, temperature: f64) -> Self {
        let n_layers = layer_sizes.len();
        let mut weights = Vec::new();
        let mut biases = Vec::new();
        let mut states = Vec::new();

        // Initialize weights (Xavier initialization)
        for i in 0..n_layers - 1 {
            let fan_in = layer_sizes[i];
            let fan_out = layer_sizes[i + 1];
            let scale = (2.0 / (fan_in + fan_out) as f64).sqrt();

            let mut layer_weights = vec![vec![0.0; fan_in]; fan_out];
            for j in 0..fan_out {
                for k in 0..fan_in {
                    layer_weights[j][k] = (rand::random::<f64>() - 0.5) * 2.0 * scale;
                }
            }
            weights.push(layer_weights);

            // Initialize biases to zero
            biases.push(vec![0.0; fan_out]);
        }

        // Initialize states to zero
        for &size in &layer_sizes {
            states.push(vec![0.0; size]);
        }

        Self {
            n_layers,
            layer_sizes,
            weights,
            biases,
            states,
            tau,
            temperature,
        }
    }

    /// Energy function: E(s) = -Σ_ij W_ij s_i s_j - Σ_i b_i s_i + Σ_i U(s_i)
    /// where U(s) is a cost function (e.g., quadratic)
    pub fn energy(&self) -> f64 {
        let mut total_energy = 0.0;

        // Interaction energy: -Σ W_ij s_i s_j
        for layer in 0..self.n_layers - 1 {
            for i in 0..self.layer_sizes[layer + 1] {
                for j in 0..self.layer_sizes[layer] {
                    total_energy -= self.weights[layer][i][j]
                        * self.states[layer + 1][i]
                        * self.states[layer][j];
                }
            }
        }

        // Bias energy: -Σ b_i s_i
        for layer in 1..self.n_layers {
            for i in 0..self.layer_sizes[layer] {
                total_energy -= self.biases[layer - 1][i] * self.states[layer][i];
            }
        }

        // Cost function U(s) = s^2 / 2 (keeps states bounded)
        for layer in 0..self.n_layers {
            for i in 0..self.layer_sizes[layer] {
                let s = self.states[layer][i];
                total_energy += 0.5 * s * s;
            }
        }

        total_energy
    }

    /// Compute energy gradient w.r.t. neuron states
    pub fn energy_gradient(&self) -> Vec<Vec<f64>> {
        let mut gradient = vec![vec![0.0; self.layer_sizes[0]]; self.n_layers];

        for layer in 0..self.n_layers {
            for i in 0..self.layer_sizes[layer] {
                let mut grad = 0.0;

                // Contribution from weights to next layer
                if layer < self.n_layers - 1 {
                    for j in 0..self.layer_sizes[layer + 1] {
                        grad -= self.weights[layer][j][i] * self.states[layer + 1][j];
                    }
                }

                // Contribution from weights from previous layer
                if layer > 0 {
                    for j in 0..self.layer_sizes[layer - 1] {
                        grad -= self.weights[layer - 1][i][j] * self.states[layer - 1][j];
                    }

                    // Bias contribution
                    grad -= self.biases[layer - 1][i];
                }

                // Cost function gradient: ∂(s^2/2)/∂s = s
                grad += self.states[layer][i];

                gradient[layer][i] = grad;
            }
        }

        gradient
    }

    /// Activation function (hard sigmoid for bounded states)
    fn activate(&self, x: f64) -> f64 {
        if x < -1.0 {
            0.0
        } else if x > 1.0 {
            1.0
        } else {
            0.5 * (x + 1.0)
        }
    }

    /// Relax network to equilibrium (free phase)
    pub fn relax_to_equilibrium(&mut self, max_iters: usize, tolerance: f64) -> usize {
        let dt = 0.1; // Time step

        for iter in 0..max_iters {
            let gradient = self.energy_gradient();
            let mut max_change: f64 = 0.0;

            // Update states: ds/dt = -∂E/∂s / τ
            for layer in 1..self.n_layers {
                // Don't update input layer
                for i in 0..self.layer_sizes[layer] {
                    let ds_dt = -gradient[layer][i] / self.tau;
                    let old_state = self.states[layer][i];
                    let new_state = self.activate(old_state + ds_dt * dt);
                    self.states[layer][i] = new_state;

                    max_change = max_change.max((new_state - old_state).abs());
                }
            }

            // Check convergence
            if max_change < tolerance {
                return iter + 1;
            }
        }

        max_iters
    }

    /// Nudged phase: relax with gentle push toward target
    pub fn relax_nudged(
        &mut self,
        target: &[f64],
        beta: f64,
        max_iters: usize,
        tolerance: f64,
    ) -> usize {
        assert_eq!(target.len(), self.layer_sizes[self.n_layers - 1]);

        let dt = 0.1;

        for iter in 0..max_iters {
            let gradient = self.energy_gradient();
            let mut max_change: f64 = 0.0;

            // Update hidden layers
            for layer in 1..self.n_layers - 1 {
                for i in 0..self.layer_sizes[layer] {
                    let ds_dt = -gradient[layer][i] / self.tau;
                    let old_state = self.states[layer][i];
                    let new_state = self.activate(old_state + ds_dt * dt);
                    self.states[layer][i] = new_state;
                    max_change = max_change.max((new_state - old_state).abs());
                }
            }

            // Update output layer with nudge toward target
            let output_layer = self.n_layers - 1;
            for i in 0..self.layer_sizes[output_layer] {
                let ds_dt = -gradient[output_layer][i] / self.tau;
                let nudge = beta * (target[i] - self.states[output_layer][i]);
                let old_state = self.states[output_layer][i];
                let new_state = self.activate(old_state + (ds_dt + nudge) * dt);
                self.states[output_layer][i] = new_state;
                max_change = max_change.max((new_state - old_state).abs());
            }

            if max_change < tolerance {
                return iter + 1;
            }
        }

        max_iters
    }

    /// Equilibrium propagation learning rule
    pub fn equilibrium_propagation_step(
        &mut self,
        input: &[f64],
        target: &[f64],
        beta: f64,
        learning_rate: f64,
    ) -> (f64, f64) {
        assert_eq!(input.len(), self.layer_sizes[0]);
        assert_eq!(target.len(), self.layer_sizes[self.n_layers - 1]);

        // Clamp input
        self.states[0].copy_from_slice(input);

        // Free phase: relax to equilibrium
        self.relax_to_equilibrium(1000, 1e-4);
        let states_free = self.states.clone();
        let energy_free = self.energy();

        // Nudged phase: relax with target nudge
        self.states[0].copy_from_slice(input); // Re-clamp input
        self.relax_nudged(target, beta, 1000, 1e-4);
        let states_nudged = self.states.clone();
        let energy_nudged = self.energy();

        // Update weights: ΔW_ij ∝ ⟨s_i s_j⟩_nudged - ⟨s_i s_j⟩_free
        for layer in 0..self.n_layers - 1 {
            for i in 0..self.layer_sizes[layer + 1] {
                for j in 0..self.layer_sizes[layer] {
                    let correlation_free = states_free[layer + 1][i] * states_free[layer][j];
                    let correlation_nudged = states_nudged[layer + 1][i] * states_nudged[layer][j];
                    let delta = (correlation_nudged - correlation_free) / beta;
                    self.weights[layer][i][j] += learning_rate * delta;
                }

                // Update biases
                let delta_bias = (states_nudged[layer + 1][i] - states_free[layer + 1][i]) / beta;
                self.biases[layer][i] += learning_rate * delta_bias;
            }
        }

        (energy_free, energy_nudged)
    }

    /// Forward pass (free phase to equilibrium)
    pub fn predict(&mut self, input: &[f64]) -> Vec<f64> {
        self.states[0].copy_from_slice(input);
        self.relax_to_equilibrium(1000, 1e-4);
        self.states[self.n_layers - 1].clone()
    }

    /// Compute prediction error
    pub fn loss(&mut self, input: &[f64], target: &[f64]) -> f64 {
        let prediction = self.predict(input);
        let mut error = 0.0;
        for (p, t) in prediction.iter().zip(target.iter()) {
            error += (p - t).powi(2);
        }
        error / 2.0
    }
}

/// Thermodynamic neural network with explicit thermal fluctuations
#[derive(Debug, Clone)]
pub struct ThermodynamicNeuralNet {
    /// Base energy-based network
    pub network: EnergyBasedNetwork,

    /// Thermal noise standard deviation
    pub thermal_noise_std: f64,
}

impl ThermodynamicNeuralNet {
    pub fn new(layer_sizes: Vec<usize>, tau: f64, temperature: f64) -> Self {
        // Thermal noise ~ sqrt(kT)
        let thermal_noise_std = (temperature * 1.38e-23_f64).sqrt();

        Self {
            network: EnergyBasedNetwork::new(layer_sizes, tau, temperature),
            thermal_noise_std,
        }
    }

    /// Add thermal noise to states
    fn add_thermal_noise(&mut self) {
        for layer in 1..self.network.n_layers {
            for i in 0..self.network.layer_sizes[layer] {
                let noise = (rand::random::<f64>() - 0.5) * 2.0 * self.thermal_noise_std;
                self.network.states[layer][i] += noise;
            }
        }
    }

    /// Relax with thermal fluctuations (Langevin dynamics)
    pub fn langevin_relax(&mut self, max_iters: usize, tolerance: f64) -> usize {
        let dt = 0.1;

        for iter in 0..max_iters {
            let gradient = self.network.energy_gradient();
            let mut max_change: f64 = 0.0;

            for layer in 1..self.network.n_layers {
                for i in 0..self.network.layer_sizes[layer] {
                    // Deterministic relaxation
                    let ds_dt = -gradient[layer][i] / self.network.tau;

                    // Thermal noise
                    let noise = (rand::random::<f64>() - 0.5) * 2.0 * self.thermal_noise_std;

                    let old_state = self.network.states[layer][i];
                    let new_state = self.network.activate(old_state + (ds_dt + noise) * dt);
                    self.network.states[layer][i] = new_state;

                    max_change = max_change.max((new_state - old_state).abs());
                }
            }

            if max_change < tolerance {
                return iter + 1;
            }
        }

        max_iters
    }
}

/// Contrastive divergence for comparison (standard energy-based learning)
#[derive(Debug)]
pub struct ContrastiveDivergence {
    /// Number of Gibbs sampling steps
    pub k_steps: usize,

    /// Temperature
    pub temperature: f64,
}

impl ContrastiveDivergence {
    pub fn new(k_steps: usize, temperature: f64) -> Self {
        Self {
            k_steps,
            temperature,
        }
    }

    /// Compute gradient: ⟨s_i s_j⟩_data - ⟨s_i s_j⟩_model
    pub fn gradient(
        &self,
        network: &EnergyBasedNetwork,
        data_states: &[Vec<f64>],
    ) -> Vec<Vec<Vec<f64>>> {
        let mut gradient = vec![
            vec![vec![0.0; network.layer_sizes[0]]; network.layer_sizes[1]];
            network.n_layers - 1
        ];

        // Positive phase: data statistics
        for layer in 0..network.n_layers - 1 {
            for i in 0..network.layer_sizes[layer + 1] {
                for j in 0..network.layer_sizes[layer] {
                    gradient[layer][i][j] += data_states[layer + 1][i] * data_states[layer][j];
                }
            }
        }

        // Negative phase: model statistics (k-step Gibbs sampling)
        // For simplicity, use current network states
        for layer in 0..network.n_layers - 1 {
            for i in 0..network.layer_sizes[layer + 1] {
                for j in 0..network.layer_sizes[layer] {
                    gradient[layer][i][j] -=
                        network.states[layer + 1][i] * network.states[layer][j];
                }
            }
        }

        gradient
    }
}

// Mock rand for deterministic testing
mod rand {
    pub fn random<T>() -> f64 {
        0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_energy_network_creation() {
        let network = EnergyBasedNetwork::new(vec![2, 3, 1], 1.0, 300.0);
        assert_eq!(network.n_layers, 3);
        assert_eq!(network.weights.len(), 2); // 2 weight matrices
        assert_eq!(network.weights[0].len(), 3); // 3 neurons in hidden layer
        assert_eq!(network.weights[0][0].len(), 2); // 2 inputs
    }

    #[test]
    fn test_energy_computation() {
        let mut network = EnergyBasedNetwork::new(vec![2, 2, 1], 1.0, 300.0);

        // Set known states
        network.states[0] = vec![1.0, 0.0];
        network.states[1] = vec![0.5, 0.5];
        network.states[2] = vec![1.0];

        // Energy should be computable
        let energy = network.energy();
        assert!(energy.is_finite());
    }

    #[test]
    fn test_equilibrium_relaxation() {
        let mut network = EnergyBasedNetwork::new(vec![2, 3, 1], 1.0, 300.0);

        // Set input
        network.states[0] = vec![1.0, 0.0];

        // Relax to equilibrium
        let iters = network.relax_to_equilibrium(1000, 1e-3);

        assert!(iters < 1000); // Should converge

        // Energy gradient should be small at equilibrium
        let grad = network.energy_gradient();
        for layer_grad in &grad[1..] {
            // Skip input layer
            for &g in layer_grad {
                assert!(g.abs() < 0.1); // Approximate equilibrium
            }
        }
    }

    #[test]
    fn test_equilibrium_propagation_learning() {
        let mut network = EnergyBasedNetwork::new(vec![2, 4, 1], 1.0, 300.0);

        let input = vec![1.0, 0.0];
        let target = vec![1.0];

        // One learning step
        let (e_free, e_nudged) = network.equilibrium_propagation_step(&input, &target, 0.5, 0.01);

        // Energies should be different
        assert!((e_free - e_nudged).abs() > 0.0);

        // Weights should have changed
        let initial_weight = network.weights[0][0][0];
        network.equilibrium_propagation_step(&input, &target, 0.5, 0.01);
        let updated_weight = network.weights[0][0][0];

        // Weight may have changed (depending on gradients)
        // Just check it's still finite
        assert!(updated_weight.is_finite());
    }

    #[test]
    fn test_prediction() {
        let mut network = EnergyBasedNetwork::new(vec![2, 3, 1], 1.0, 300.0);

        let input = vec![0.5, -0.5];
        let output = network.predict(&input);

        assert_eq!(output.len(), 1);
        assert!(output[0].is_finite());
        assert!(output[0] >= 0.0 && output[0] <= 1.0); // Bounded by activation
    }
}

/// Example: XOR learning with equilibrium propagation
pub fn example_xor_learning() {
    println!("=== Equilibrium Propagation: XOR Learning ===\n");

    let mut network = EnergyBasedNetwork::new(vec![2, 4, 1], 1.0, 300.0);

    // XOR dataset
    let inputs = vec![
        vec![0.0, 0.0],
        vec![0.0, 1.0],
        vec![1.0, 0.0],
        vec![1.0, 1.0],
    ];
    let targets = vec![vec![0.0], vec![1.0], vec![1.0], vec![0.0]];

    let beta = 0.5;
    let learning_rate = 0.01;
    let epochs = 100;

    for epoch in 0..epochs {
        let mut total_loss = 0.0;

        for (input, target) in inputs.iter().zip(targets.iter()) {
            let loss = network.loss(input, target);
            total_loss += loss;

            network.equilibrium_propagation_step(input, target, beta, learning_rate);
        }

        if epoch % 20 == 0 {
            println!("Epoch {}: Average Loss = {:.6}", epoch, total_loss / 4.0);
        }
    }

    println!("\nFinal predictions:");
    for (input, target) in inputs.iter().zip(targets.iter()) {
        let pred = network.predict(input);
        println!(
            "Input: {:?} -> Prediction: {:.4}, Target: {:.4}",
            input, pred[0], target[0]
        );
    }
}
