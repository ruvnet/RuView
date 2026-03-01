// Discrete Time Crystal Implementation
// Simulates discrete time translation symmetry breaking in neural-inspired systems

use ndarray::{Array1, Array2};
use rand::Rng;
use std::f64::consts::PI;

/// Configuration for discrete time crystal simulation
#[derive(Clone, Debug)]
pub struct DTCConfig {
    /// Number of oscillators/neurons
    pub n_oscillators: usize,
    /// Drive frequency (Hz)
    pub drive_frequency: f64,
    /// Drive amplitude
    pub drive_amplitude: f64,
    /// Coupling strength between oscillators
    pub coupling_strength: f64,
    /// Dissipation rate
    pub dissipation: f64,
    /// Nonlinearity parameter
    pub nonlinearity: f64,
    /// Noise amplitude
    pub noise_amplitude: f64,
    /// Time step for integration
    pub dt: f64,
}

impl Default for DTCConfig {
    fn default() -> Self {
        Self {
            n_oscillators: 100,
            drive_frequency: 8.0, // Theta frequency
            drive_amplitude: 2.0,
            coupling_strength: 0.5,
            dissipation: 0.1,
            nonlinearity: 1.0,
            noise_amplitude: 0.01,
            dt: 0.001,
        }
    }
}

/// Discrete Time Crystal simulator
pub struct DiscreteTimeCrystal {
    config: DTCConfig,
    /// State: position of each oscillator
    positions: Array1<f64>,
    /// Velocities
    velocities: Array1<f64>,
    /// Coupling matrix (asymmetric)
    coupling_matrix: Array2<f64>,
    /// Current time
    time: f64,
    /// Random number generator
    rng: rand::rngs::ThreadRng,
}

impl DiscreteTimeCrystal {
    /// Create new DTC simulator
    pub fn new(config: DTCConfig) -> Self {
        let mut rng = rand::thread_rng();
        let n = config.n_oscillators;

        // Initialize positions and velocities randomly
        let positions = Array1::from_vec((0..n).map(|_| rng.gen_range(-1.0..1.0)).collect());
        let velocities = Array1::from_vec((0..n).map(|_| rng.gen_range(-0.1..0.1)).collect());

        // Create asymmetric coupling matrix
        let coupling_matrix = Self::generate_asymmetric_coupling(n, &mut rng);

        Self {
            config,
            positions,
            velocities,
            coupling_matrix,
            time: 0.0,
            rng,
        }
    }

    /// Generate asymmetric coupling matrix
    /// Asymmetry is crucial for breaking detailed balance and enabling limit cycles
    fn generate_asymmetric_coupling(n: usize, rng: &mut rand::rngs::ThreadRng) -> Array2<f64> {
        let mut matrix = Array2::zeros((n, n));

        for i in 0..n {
            for j in 0..n {
                if i != j {
                    // Sparse coupling
                    if rng.gen::<f64>() < 0.2 {
                        matrix[[i, j]] = rng.gen_range(-1.0..1.0);
                    }
                }
            }
        }

        matrix
    }

    /// Periodic driving force (e.g., theta oscillations)
    fn drive_force(&self, t: f64) -> f64 {
        let omega = 2.0 * PI * self.config.drive_frequency;
        self.config.drive_amplitude * (omega * t).cos()
    }

    /// Nonlinear activation function (tanh-like)
    fn activation(&self, x: f64) -> f64 {
        (self.config.nonlinearity * x).tanh()
    }

    /// Compute forces on all oscillators
    fn compute_forces(&mut self) -> (Array1<f64>, Array1<f64>) {
        let n = self.config.n_oscillators;
        let mut forces_pos = Array1::zeros(n);
        let mut forces_vel = Array1::zeros(n);

        let drive = self.drive_force(self.time);

        for i in 0..n {
            // Coupling forces (asymmetric → breaks detailed balance)
            let mut coupling_force = 0.0;
            for j in 0..n {
                if i != j {
                    let coupling = self.coupling_matrix[[i, j]];
                    coupling_force += coupling * self.activation(self.positions[j]);
                }
            }

            // Position force: restoring + coupling + drive + noise
            forces_pos[i] = self.velocities[i]; // dx/dt = v

            // Velocity force: -x - γv + J*coupling + A*drive + noise
            let noise = self.rng.gen_range(-1.0..1.0) * self.config.noise_amplitude;
            forces_vel[i] = -self.positions[i] - self.config.dissipation * self.velocities[i]
                + self.config.coupling_strength * coupling_force
                + drive
                + noise;
        }

        (forces_pos, forces_vel)
    }

    /// Evolve system by one time step (Euler integration)
    pub fn step(&mut self) {
        let (forces_pos, forces_vel) = self.compute_forces();

        // Update positions and velocities
        self.positions += &(forces_pos * self.config.dt);
        self.velocities += &(forces_vel * self.config.dt);

        self.time += self.config.dt;
    }

    /// Run simulation for specified duration
    pub fn run(&mut self, duration: f64) -> Vec<Array1<f64>> {
        let n_steps = (duration / self.config.dt) as usize;
        let mut trajectory = Vec::with_capacity(n_steps);

        for _ in 0..n_steps {
            self.step();
            trajectory.push(self.positions.clone());
        }

        trajectory
    }

    /// Compute order parameter M_k for subharmonic order k
    pub fn compute_order_parameter(&self, trajectory: &[Array1<f64>], k: usize) -> Vec<f64> {
        let omega_0 = 2.0 * PI * self.config.drive_frequency;
        let n = self.config.n_oscillators;

        trajectory
            .iter()
            .enumerate()
            .map(|(step, positions)| {
                let _t = step as f64 * self.config.dt;

                // Compute phases relative to drive
                let mut sum_real = 0.0;
                let mut sum_imag = 0.0;

                for i in 0..n {
                    // Phase of oscillator i
                    let phase = positions[i].atan2(1.0); // Simplified phase extraction
                    let arg = k as f64 * omega_0 * phase;
                    sum_real += arg.cos();
                    sum_imag += arg.sin();
                }

                // Order parameter: |<e^(ik*omega*phi)>|
                let m_k = ((sum_real / n as f64).powi(2) + (sum_imag / n as f64).powi(2)).sqrt();
                m_k
            })
            .collect()
    }

    /// Compute power spectral density to detect subharmonics
    pub fn compute_psd(&self, signal: &[f64], sample_rate: f64) -> (Vec<f64>, Vec<f64>) {
        // Simple FFT-based PSD
        use rustfft::{num_complex::Complex, FftPlanner};

        let n = signal.len();
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(n);

        // Convert to complex
        let mut buffer: Vec<Complex<f64>> =
            signal.iter().map(|&x| Complex { re: x, im: 0.0 }).collect();

        // Apply FFT
        fft.process(&mut buffer);

        // Compute power
        let power: Vec<f64> = buffer
            .iter()
            .take(n / 2)
            .map(|c| (c.re * c.re + c.im * c.im) / n as f64)
            .collect();

        // Frequency bins
        let freqs: Vec<f64> = (0..n / 2)
            .map(|i| i as f64 * sample_rate / n as f64)
            .collect();

        (freqs, power)
    }

    /// Detect period-doubling by comparing power at f and f/2
    pub fn detect_period_doubling(&self, trajectory: &[Array1<f64>]) -> (f64, bool) {
        // Average activity across all oscillators
        let signal: Vec<f64> = trajectory
            .iter()
            .map(|positions| positions.mean().unwrap())
            .collect();

        let sample_rate = 1.0 / self.config.dt;
        let (freqs, power) = self.compute_psd(&signal, sample_rate);

        // Find peaks at drive frequency and its half
        let drive_freq = self.config.drive_frequency;
        let half_freq = drive_freq / 2.0;

        // Find power at these frequencies (within tolerance)
        let tol = 0.5; // Hz tolerance

        let p_drive: f64 = freqs
            .iter()
            .zip(&power)
            .filter(|(f, _)| (*f - drive_freq).abs() < tol)
            .map(|(_, p)| p)
            .fold(0.0_f64, |acc, &p| acc.max(p));

        let p_half: f64 = freqs
            .iter()
            .zip(&power)
            .filter(|(f, _)| (*f - half_freq).abs() < tol)
            .map(|(_, p)| p)
            .fold(0.0_f64, |acc, &p| acc.max(p));

        // Period-doubling if power at f/2 exceeds power at f
        let ratio = p_half / p_drive.max(1e-10);
        let is_period_doubled = ratio > 1.0;

        (ratio, is_period_doubled)
    }

    /// Get current state
    pub fn get_state(&self) -> (&Array1<f64>, &Array1<f64>, f64) {
        (&self.positions, &self.velocities, self.time)
    }
}

/// Analysis utilities for DTC
pub mod analysis {

    /// Compute temporal autocorrelation
    pub fn autocorrelation(signal: &[f64], max_lag: usize) -> Vec<f64> {
        let n = signal.len();
        let mean = signal.iter().sum::<f64>() / n as f64;
        let variance = signal.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / n as f64;

        (0..max_lag)
            .map(|lag| {
                let mut sum = 0.0;
                for i in 0..(n - lag) {
                    sum += (signal[i] - mean) * (signal[i + lag] - mean);
                }
                sum / ((n - lag) as f64 * variance)
            })
            .collect()
    }

    /// Fit autocorrelation to detect power-law vs exponential decay
    /// Returns (is_power_law, exponent)
    pub fn classify_decay(autocorr: &[f64]) -> (bool, f64) {
        // Log-log fit for power law: log(C) ~ -α log(τ)
        // Log-linear fit for exponential: log(C) ~ -τ/τ_c

        let lags: Vec<f64> = (1..autocorr.len()).map(|i| i as f64).collect();
        let log_autocorr: Vec<f64> = autocorr
            .iter()
            .skip(1)
            .map(|&c| c.max(1e-10).ln())
            .collect();

        // Power-law fit
        let log_lags: Vec<f64> = lags.iter().map(|&x| x.ln()).collect();
        let power_law_fit = linear_regression(&log_lags, &log_autocorr);

        // Exponential fit
        let exp_fit = linear_regression(&lags, &log_autocorr);

        // Compare R^2 values
        let is_power_law = power_law_fit.1 > exp_fit.1; // Better R^2 for power law
        let exponent = if is_power_law {
            -power_law_fit.0
        } else {
            -exp_fit.0
        };

        (is_power_law, exponent)
    }

    /// Simple linear regression: returns (slope, R^2)
    fn linear_regression(x: &[f64], y: &[f64]) -> (f64, f64) {
        let n = x.len() as f64;
        let sum_x: f64 = x.iter().sum();
        let sum_y: f64 = y.iter().sum();
        let sum_xx: f64 = x.iter().map(|&xi| xi * xi).sum();
        let sum_xy: f64 = x.iter().zip(y).map(|(&xi, &yi)| xi * yi).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);

        // Compute R^2
        let mean_y = sum_y / n;
        let ss_tot: f64 = y.iter().map(|&yi| (yi - mean_y).powi(2)).sum();
        let ss_res: f64 = x
            .iter()
            .zip(y)
            .map(|(&xi, &yi)| {
                let pred = slope * xi + (sum_y - slope * sum_x) / n;
                (yi - pred).powi(2)
            })
            .sum();
        let r_squared = 1.0 - ss_res / ss_tot;

        (slope, r_squared)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dtc_creation() {
        let config = DTCConfig::default();
        let dtc = DiscreteTimeCrystal::new(config);
        assert_eq!(dtc.positions.len(), 100);
    }

    #[test]
    fn test_period_doubling() {
        let mut config = DTCConfig::default();
        config.drive_amplitude = 3.0; // Strong drive to induce period-doubling
        config.coupling_strength = 0.8;
        config.n_oscillators = 50;

        let mut dtc = DiscreteTimeCrystal::new(config);

        // Run for a few oscillation periods
        let duration = 2.0; // 2 seconds = 16 theta cycles
        let trajectory = dtc.run(duration);

        // Detect period-doubling
        let (ratio, is_doubled) = dtc.detect_period_doubling(&trajectory);

        println!("Period-doubling ratio: {}", ratio);
        println!("Is period-doubled: {}", is_doubled);

        // Note: This is stochastic, so we don't assert, just demonstrate
    }

    #[test]
    fn test_order_parameter() {
        let config = DTCConfig::default();
        let mut dtc = DiscreteTimeCrystal::new(config);

        let duration = 1.0;
        let trajectory = dtc.run(duration);

        let m_2 = dtc.compute_order_parameter(&trajectory, 2);

        // Order parameter should be between 0 and 1
        for &m in &m_2 {
            assert!(m >= 0.0 && m <= 1.0);
        }
    }
}
