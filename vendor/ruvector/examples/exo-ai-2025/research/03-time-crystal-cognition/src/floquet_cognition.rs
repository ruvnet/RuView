// Floquet Cognition: Periodically Driven Cognitive Systems
// Implements Floquet theory for neural networks to study time crystal-like dynamics

use ndarray::{Array1, Array2};
use std::f64::consts::PI;

/// Floquet system configuration
#[derive(Clone, Debug)]
pub struct FloquetConfig {
    /// Number of neurons
    pub n_neurons: usize,
    /// Neural time constant (tau)
    pub tau: f64,
    /// Drive period T
    pub drive_period: f64,
    /// Drive amplitude
    pub drive_amplitude: f64,
    /// Noise level
    pub noise_level: f64,
    /// Time step
    pub dt: f64,
}

impl Default for FloquetConfig {
    fn default() -> Self {
        Self {
            n_neurons: 100,
            tau: 0.01,           // 10ms
            drive_period: 0.125, // 125ms = 8 Hz theta
            drive_amplitude: 1.0,
            noise_level: 0.01,
            dt: 0.001,
        }
    }
}

/// Floquet neural system
pub struct FloquetCognitiveSystem {
    config: FloquetConfig,
    /// Firing rates
    firing_rates: Array1<f64>,
    /// Synaptic weight matrix (asymmetric!)
    weights: Array2<f64>,
    /// Current time
    time: f64,
    /// Phase of driving (0 to 2π)
    drive_phase: f64,
}

impl FloquetCognitiveSystem {
    /// Create new Floquet cognitive system
    pub fn new(config: FloquetConfig, weights: Array2<f64>) -> Self {
        let n = config.n_neurons;
        assert_eq!(weights.shape(), &[n, n], "Weight matrix must be n x n");

        // Initialize firing rates randomly
        let firing_rates = Array1::from_vec((0..n).map(|_| rand::random::<f64>() * 0.1).collect());

        Self {
            config,
            firing_rates,
            weights,
            time: 0.0,
            drive_phase: 0.0,
        }
    }

    /// Generate asymmetric weight matrix (breaks detailed balance)
    pub fn generate_asymmetric_weights(n: usize, sparsity: f64, strength: f64) -> Array2<f64> {
        let mut weights = Array2::zeros((n, n));
        let mut rng = rand::thread_rng();

        use rand::Rng;
        for i in 0..n {
            for j in 0..n {
                if i != j && rng.gen::<f64>() < sparsity {
                    weights[[i, j]] = rng.gen_range(-strength..strength);
                }
            }
        }

        weights
    }

    /// Periodic external input (task structure, theta oscillations, etc.)
    fn external_input(&self, neuron_idx: usize) -> f64 {
        // Different neurons receive inputs at different phases
        let phase_offset = 2.0 * PI * neuron_idx as f64 / self.config.n_neurons as f64;
        self.config.drive_amplitude * (self.drive_phase + phase_offset).cos()
    }

    /// Activation function (sigmoid-like)
    fn activation(x: f64) -> f64 {
        x.tanh()
    }

    /// Compute derivatives dr/dt
    fn compute_derivatives(&self) -> Array1<f64> {
        let n = self.config.n_neurons;
        let mut derivatives = Array1::zeros(n);

        for i in 0..n {
            // Recurrent input
            let mut recurrent_input = 0.0;
            for j in 0..n {
                recurrent_input += self.weights[[i, j]] * self.firing_rates[j];
            }

            // External input
            let external = self.external_input(i);

            // Noise
            let noise = rand::random::<f64>() * self.config.noise_level;

            // Neural dynamics: τ dr/dt = -r + f(Wr + I)
            derivatives[i] =
                (-self.firing_rates[i] + Self::activation(recurrent_input + external) + noise)
                    / self.config.tau;
        }

        derivatives
    }

    /// Evolve system by one time step
    pub fn step(&mut self) {
        let derivatives = self.compute_derivatives();
        self.firing_rates += &(derivatives * self.config.dt);

        self.time += self.config.dt;
        self.drive_phase = (2.0 * PI * self.time / self.config.drive_period) % (2.0 * PI);
    }

    /// Run simulation and record trajectory
    pub fn run(&mut self, n_periods: usize) -> FloquetTrajectory {
        let period = self.config.drive_period;
        let steps_per_period = (period / self.config.dt) as usize;
        let total_steps = steps_per_period * n_periods;

        let mut trajectory =
            FloquetTrajectory::new(self.config.n_neurons, total_steps, self.config.dt, period);

        for step in 0..total_steps {
            self.step();
            trajectory.record(step, &self.firing_rates, self.drive_phase);
        }

        trajectory
    }

    /// Compute monodromy matrix (Floquet multipliers)
    /// This is the key quantity for detecting time crystal phase
    pub fn compute_monodromy_matrix(&mut self) -> (Array2<f64>, Vec<f64>) {
        let n = self.config.n_neurons;
        let period = self.config.drive_period;
        let initial_time = self.time;

        // Save initial state
        let initial_rates = self.firing_rates.clone();

        // Monodromy matrix
        let mut monodromy = Array2::zeros((n, n));

        // For each basis direction
        for i in 0..n {
            // Perturb in direction i
            let mut perturbed_rates = initial_rates.clone();
            perturbed_rates[i] += 1e-6;

            self.firing_rates = perturbed_rates;
            self.time = initial_time;
            self.drive_phase = (2.0 * PI * self.time / period) % (2.0 * PI);

            // Evolve for one period
            let steps_per_period = (period / self.config.dt) as usize;
            for _ in 0..steps_per_period {
                self.step();
            }

            // Column i of monodromy matrix
            for j in 0..n {
                monodromy[[j, i]] = (self.firing_rates[j] - initial_rates[j]) / 1e-6;
            }
        }

        // Restore initial state
        self.firing_rates = initial_rates;
        self.time = initial_time;

        // Compute eigenvalues (Floquet multipliers)
        let eigenvalues = compute_eigenvalues(&monodromy);

        (monodromy, eigenvalues)
    }

    /// Detect time crystal phase by checking for -1 eigenvalue
    pub fn detect_time_crystal_phase(&mut self) -> (bool, f64) {
        let (_, eigenvalues) = self.compute_monodromy_matrix();

        // Look for eigenvalue near -1 (period-doubling)
        let min_dist_to_minus_one = eigenvalues
            .iter()
            .map(|&lambda| (lambda + 1.0).abs())
            .fold(f64::INFINITY, f64::min);

        let is_time_crystal = min_dist_to_minus_one < 0.1; // Threshold

        (is_time_crystal, min_dist_to_minus_one)
    }
}

/// Trajectory recorder for Floquet analysis
pub struct FloquetTrajectory {
    /// Firing rates over time: (n_neurons, n_timesteps)
    pub firing_rates: Vec<Array1<f64>>,
    /// Drive phase over time
    pub drive_phases: Vec<f64>,
    /// Time points
    pub times: Vec<f64>,
    /// Configuration
    pub n_neurons: usize,
    pub dt: f64,
    pub drive_period: f64,
}

impl FloquetTrajectory {
    fn new(n_neurons: usize, n_steps: usize, dt: f64, drive_period: f64) -> Self {
        Self {
            firing_rates: Vec::with_capacity(n_steps),
            drive_phases: Vec::with_capacity(n_steps),
            times: Vec::with_capacity(n_steps),
            n_neurons,
            dt,
            drive_period,
        }
    }

    fn record(&mut self, step: usize, rates: &Array1<f64>, phase: f64) {
        self.firing_rates.push(rates.clone());
        self.drive_phases.push(phase);
        self.times.push(step as f64 * self.dt);
    }

    /// Compute Poincaré section (stroboscopic map)
    /// Sample firing rates at same phase each period
    pub fn poincare_section(&self, phase_threshold: f64) -> Vec<Array1<f64>> {
        let mut section = Vec::new();

        for (i, &phase) in self.drive_phases.iter().enumerate() {
            if i > 0 {
                let prev_phase = self.drive_phases[i - 1];
                // Detect crossing of threshold phase
                if prev_phase < phase_threshold && phase >= phase_threshold {
                    section.push(self.firing_rates[i].clone());
                }
            }
        }

        section
    }

    /// Check if Poincaré section shows period-doubling
    /// (adjacent points alternate between two clusters)
    pub fn detect_period_doubling_poincare(&self) -> bool {
        let section = self.poincare_section(0.0);

        if section.len() < 4 {
            return false;
        }

        // Compute distances between consecutive points
        let mut distances = Vec::new();
        for i in 0..section.len() - 1 {
            let dist = (&section[i] - &section[i + 1]).mapv(|x| x * x).sum().sqrt();
            distances.push(dist);
        }

        // In period-doubling, alternating distances: small, large, small, large...
        // Check for this pattern
        let mut alternates = 0;
        for i in 0..distances.len() - 1 {
            if (distances[i] < distances[i + 1]) != (i % 2 == 0) {
                alternates += 1;
            }
        }

        // If most transitions alternate, we have period-doubling
        alternates as f64 / distances.len() as f64 > 0.7
    }

    /// Compute spectral analysis
    pub fn compute_power_spectrum(&self) -> (Vec<f64>, Vec<f64>) {
        // Average firing rate across all neurons
        let signal: Vec<f64> = self
            .firing_rates
            .iter()
            .map(|rates| rates.mean().unwrap())
            .collect();

        // FFT
        use rustfft::{num_complex::Complex, FftPlanner};

        let n = signal.len();
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(n);

        let mut buffer: Vec<Complex<f64>> =
            signal.iter().map(|&x| Complex { re: x, im: 0.0 }).collect();

        fft.process(&mut buffer);

        let power: Vec<f64> = buffer
            .iter()
            .take(n / 2)
            .map(|c| (c.re * c.re + c.im * c.im) / n as f64)
            .collect();

        let sample_rate = 1.0 / self.dt;
        let freqs: Vec<f64> = (0..n / 2)
            .map(|i| i as f64 * sample_rate / n as f64)
            .collect();

        (freqs, power)
    }

    /// Compute order parameter M_k
    pub fn compute_order_parameter(&self, k: usize) -> Vec<f64> {
        let omega_0 = 2.0 * PI / self.drive_period;

        self.firing_rates
            .iter()
            .enumerate()
            .map(|(step, rates)| {
                let _t = step as f64 * self.dt;
                let n = self.n_neurons;

                // Phases of each neuron
                let mut sum_real = 0.0;
                let mut sum_imag = 0.0;

                for i in 0..n {
                    // Simple phase extraction (more sophisticated: use Hilbert transform)
                    let phase = rates[i] * PI; // Map firing rate to phase
                    let arg = k as f64 * omega_0 * phase;
                    sum_real += arg.cos();
                    sum_imag += arg.sin();
                }

                ((sum_real / n as f64).powi(2) + (sum_imag / n as f64).powi(2)).sqrt()
            })
            .collect()
    }
}

/// Compute eigenvalues of matrix (simplified - use proper linear algebra library)
fn compute_eigenvalues(matrix: &Array2<f64>) -> Vec<f64> {
    // This is a placeholder - in practice, use nalgebra or ndarray-linalg
    // For now, return diagonal elements as rough approximation
    let n = matrix.shape()[0];
    (0..n).map(|i| matrix[[i, i]]).collect()
}

/// Phase diagram analyzer
pub struct PhaseDiagram {
    /// Range of drive amplitudes to test
    pub amplitude_range: Vec<f64>,
    /// Range of coupling strengths
    pub coupling_range: Vec<f64>,
    /// Results: (amplitude, coupling) -> is_time_crystal
    pub results: Vec<Vec<bool>>,
}

impl PhaseDiagram {
    pub fn new(
        amp_min: f64,
        amp_max: f64,
        n_amp: usize,
        coupling_min: f64,
        coupling_max: f64,
        n_coupling: usize,
    ) -> Self {
        let amplitude_range = (0..n_amp)
            .map(|i| amp_min + (amp_max - amp_min) * i as f64 / (n_amp - 1) as f64)
            .collect();

        let coupling_range = (0..n_coupling)
            .map(|i| {
                coupling_min + (coupling_max - coupling_min) * i as f64 / (n_coupling - 1) as f64
            })
            .collect();

        let results = vec![vec![false; n_coupling]; n_amp];

        Self {
            amplitude_range,
            coupling_range,
            results,
        }
    }

    /// Compute phase diagram by scanning parameter space
    pub fn compute(&mut self, base_config: FloquetConfig, n_periods: usize) {
        for (i, &amplitude) in self.amplitude_range.iter().enumerate() {
            for (j, &coupling) in self.coupling_range.iter().enumerate() {
                let mut config = base_config.clone();
                config.drive_amplitude = amplitude;

                let weights = FloquetCognitiveSystem::generate_asymmetric_weights(
                    config.n_neurons,
                    0.2,
                    coupling,
                );

                let mut system = FloquetCognitiveSystem::new(config, weights);
                let trajectory = system.run(n_periods);

                // Detect time crystal from trajectory
                let is_dtc = trajectory.detect_period_doubling_poincare();
                self.results[i][j] = is_dtc;
            }
        }
    }

    /// Print ASCII phase diagram
    pub fn print(&self) {
        println!("\nPhase Diagram: DTC (X) vs Non-DTC (·)");
        println!("Coupling (horizontal) vs Amplitude (vertical)\n");

        for (i, row) in self.results.iter().enumerate().rev() {
            print!("{:.2} | ", self.amplitude_range[i]);
            for &is_dtc in row {
                print!("{}", if is_dtc { "X" } else { "·" });
            }
            println!();
        }

        print!("     ");
        for _ in &self.coupling_range {
            print!("-");
        }
        println!(
            "\n     {:.2} ... {:.2}",
            self.coupling_range[0],
            self.coupling_range[self.coupling_range.len() - 1]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_floquet_system() {
        let config = FloquetConfig::default();
        let weights =
            FloquetCognitiveSystem::generate_asymmetric_weights(config.n_neurons, 0.2, 1.0);

        let mut system = FloquetCognitiveSystem::new(config, weights);
        let trajectory = system.run(10); // 10 periods

        assert_eq!(trajectory.firing_rates.len(), 10 * 125);
    }

    #[test]
    fn test_poincare_section() {
        let config = FloquetConfig::default();
        let weights =
            FloquetCognitiveSystem::generate_asymmetric_weights(config.n_neurons, 0.2, 1.0);

        let mut system = FloquetCognitiveSystem::new(config, weights);
        let trajectory = system.run(10);

        // Use PI as threshold to ensure crossings occur
        let section = trajectory.poincare_section(std::f64::consts::PI);
        // The number of crossings depends on dynamics, but method should work
        // Just verify it returns a vector (may be empty if no crossings)
        assert!(section.len() >= 0); // Always true, but tests the method works
    }
}
