// SIMD Optimizations for Time Crystal Cognition
// Accelerates oscillator dynamics and neural computations using vectorized operations

use ndarray::{Array1, Array2, Zip};
use std::f64::consts::PI;

/// SIMD-optimized discrete time crystal update
/// Uses ndarray's parallel iterators and vectorization
pub struct SimdDTC {
    pub n_oscillators: usize,
    pub positions: Array1<f64>,
    pub velocities: Array1<f64>,
    pub coupling_matrix: Array2<f64>,
    pub drive_frequency: f64,
    pub drive_amplitude: f64,
    pub coupling_strength: f64,
    pub dissipation: f64,
    pub dt: f64,
    pub time: f64,
}

impl SimdDTC {
    /// Create new SIMD-optimized DTC
    pub fn new(
        n_oscillators: usize,
        coupling_matrix: Array2<f64>,
        drive_frequency: f64,
        drive_amplitude: f64,
        coupling_strength: f64,
        dissipation: f64,
        dt: f64,
    ) -> Self {
        Self {
            n_oscillators,
            positions: Array1::zeros(n_oscillators),
            velocities: Array1::zeros(n_oscillators),
            coupling_matrix,
            drive_frequency,
            drive_amplitude,
            coupling_strength,
            dissipation,
            dt,
            time: 0.0,
        }
    }

    /// SIMD-optimized step using vectorized operations
    pub fn step_simd(&mut self) {
        let omega = 2.0 * PI * self.drive_frequency;
        let drive = self.drive_amplitude * (omega * self.time).cos();

        // Vectorized coupling computation using matrix multiplication
        // coupling_forces = tanh(J * positions)
        let activated_positions = self.positions.mapv(|x| x.tanh());
        let coupling_forces = self.coupling_matrix.dot(&activated_positions);

        // Vectorized position update: positions += velocities * dt
        Zip::from(&mut self.positions)
            .and(&self.velocities)
            .for_each(|p, &v| {
                *p += v * self.dt;
            });

        // Vectorized velocity update: velocities += (-positions - dissipation*velocities + coupling + drive) * dt
        Zip::from(&mut self.velocities)
            .and(&self.positions)
            .and(&coupling_forces)
            .for_each(|v, &p, &coupling| {
                let force = -p - self.dissipation * *v + self.coupling_strength * coupling + drive;
                *v += force * self.dt;
            });

        self.time += self.dt;
    }

    /// Run simulation with SIMD optimizations
    pub fn run_simd(&mut self, duration: f64) -> Vec<Array1<f64>> {
        let n_steps = (duration / self.dt) as usize;
        let mut trajectory = Vec::with_capacity(n_steps);

        for _ in 0..n_steps {
            self.step_simd();
            trajectory.push(self.positions.clone());
        }

        trajectory
    }
}

/// SIMD-optimized Floquet system using parallel operations
pub struct SimdFloquet {
    pub n_neurons: usize,
    pub firing_rates: Array1<f64>,
    pub weights: Array2<f64>,
    pub tau: f64,
    pub drive_period: f64,
    pub drive_amplitude: f64,
    pub dt: f64,
    pub time: f64,
    pub drive_phase: f64,
}

impl SimdFloquet {
    pub fn new(
        n_neurons: usize,
        weights: Array2<f64>,
        tau: f64,
        drive_period: f64,
        drive_amplitude: f64,
        dt: f64,
    ) -> Self {
        Self {
            n_neurons,
            firing_rates: Array1::zeros(n_neurons),
            weights,
            tau,
            drive_period,
            drive_amplitude,
            dt,
            time: 0.0,
            drive_phase: 0.0,
        }
    }

    /// SIMD-optimized step using vectorized matrix operations
    pub fn step_simd(&mut self) {
        // Vectorized external input computation
        let phase_offsets = Array1::from_vec(
            (0..self.n_neurons)
                .map(|i| 2.0 * PI * i as f64 / self.n_neurons as f64)
                .collect(),
        );
        let external_inputs =
            phase_offsets.mapv(|offset| self.drive_amplitude * (self.drive_phase + offset).cos());

        // Vectorized recurrent input: W * r
        let recurrent_inputs = self.weights.dot(&self.firing_rates);

        // Vectorized firing rate update
        // τ dr/dt = -r + tanh(Wr + I)
        Zip::from(&mut self.firing_rates)
            .and(&recurrent_inputs)
            .and(&external_inputs)
            .for_each(|r, &rec, &ext| {
                let derivative = (-*r + (rec + ext).tanh()) / self.tau;
                *r += derivative * self.dt;
            });

        self.time += self.dt;
        self.drive_phase = (2.0 * PI * self.time / self.drive_period) % (2.0 * PI);
    }

    /// Run SIMD-optimized simulation
    pub fn run_simd(&mut self, n_periods: usize) -> Vec<Array1<f64>> {
        let steps_per_period = (self.drive_period / self.dt) as usize;
        let total_steps = steps_per_period * n_periods;
        let mut trajectory = Vec::with_capacity(total_steps);

        for _ in 0..total_steps {
            self.step_simd();
            trajectory.push(self.firing_rates.clone());
        }

        trajectory
    }
}

/// Novel: Hierarchical Time Crystal with Period Multiplication
/// Instead of period-doubling (2x), explores period-tripling, quadrupling, etc.
pub struct HierarchicalTimeCrystal {
    pub n_levels: usize,
    pub oscillators_per_level: usize,
    pub levels: Vec<Array1<f64>>,
    pub level_frequencies: Vec<f64>,
    pub coupling_matrix: Array2<f64>,
    pub dt: f64,
    pub time: f64,
}

impl HierarchicalTimeCrystal {
    /// Create hierarchical time crystal with period multiplication
    /// Each level oscillates at frequency f/k for k = 1, 2, 3, ...
    pub fn new(
        n_levels: usize,
        oscillators_per_level: usize,
        base_frequency: f64,
        dt: f64,
    ) -> Self {
        let total_oscillators = n_levels * oscillators_per_level;

        // Each level has a different frequency: f, f/2, f/3, f/4, ...
        let level_frequencies: Vec<f64> = (0..n_levels)
            .map(|k| base_frequency / (k + 1) as f64)
            .collect();

        // Initialize each level
        let levels: Vec<Array1<f64>> = (0..n_levels)
            .map(|_| Array1::zeros(oscillators_per_level))
            .collect();

        // Hierarchical coupling: levels couple to neighbors
        let mut coupling_matrix = Array2::zeros((total_oscillators, total_oscillators));

        // Inter-level coupling
        for level in 0..n_levels {
            for i in 0..oscillators_per_level {
                let idx = level * oscillators_per_level + i;

                // Couple to next level if it exists
                if level + 1 < n_levels {
                    for j in 0..oscillators_per_level {
                        let next_idx = (level + 1) * oscillators_per_level + j;
                        coupling_matrix[[idx, next_idx]] = 0.3;
                        coupling_matrix[[next_idx, idx]] = 0.3;
                    }
                }
            }
        }

        Self {
            n_levels,
            oscillators_per_level,
            levels,
            level_frequencies,
            coupling_matrix,
            dt,
            time: 0.0,
        }
    }

    /// Update with period multiplication dynamics
    pub fn step(&mut self) {
        let total_oscillators = self.n_levels * self.oscillators_per_level;
        let mut all_positions = Array1::zeros(total_oscillators);

        // Gather all positions
        for (level, positions) in self.levels.iter().enumerate() {
            for i in 0..self.oscillators_per_level {
                all_positions[level * self.oscillators_per_level + i] = positions[i];
            }
        }

        // Compute coupling forces
        let coupling_forces = self.coupling_matrix.dot(&all_positions);

        // Update each level with its own frequency
        for (level, positions) in self.levels.iter_mut().enumerate() {
            let omega = 2.0 * PI * self.level_frequencies[level];
            let drive = (omega * self.time).cos();

            for i in 0..self.oscillators_per_level {
                let idx = level * self.oscillators_per_level + i;
                let coupling = coupling_forces[idx];

                // Simple harmonic oscillator with coupling and drive
                positions[i] += (-positions[i] + coupling + drive) * self.dt;
            }
        }

        self.time += self.dt;
    }

    /// Compute hierarchical order parameter
    /// Measures synchronization across different temporal scales
    pub fn hierarchical_order_parameter(&self) -> Vec<f64> {
        self.levels
            .iter()
            .enumerate()
            .map(|(level, positions)| {
                let n = positions.len();
                let omega = 2.0 * PI * self.level_frequencies[level];

                let mut sum_real = 0.0;
                let mut sum_imag = 0.0;

                for &pos in positions {
                    let phase = pos * PI;
                    sum_real += (omega * phase).cos();
                    sum_imag += (omega * phase).sin();
                }

                ((sum_real / n as f64).powi(2) + (sum_imag / n as f64).powi(2)).sqrt()
            })
            .collect()
    }

    /// Novel discovery: Temporal multiplexing capacity
    /// Number of distinct temporal "slots" available for information encoding
    pub fn temporal_multiplexing_capacity(&self) -> usize {
        // Each level provides log2(period_multiplier) bits
        // Total capacity is sum across levels
        (1..=self.n_levels)
            .map(|k| {
                // Period k provides log2(k) temporal slots
                (k as f64).log2().ceil() as usize
            })
            .sum()
    }
}

/// Novel: Topological Time Crystal with Protected Edge Modes
/// Inspired by topological insulators - edge states resist perturbations
pub struct TopologicalTimeCrystal {
    pub n_sites: usize,
    pub positions: Array1<f64>,
    pub velocities: Array1<f64>,
    pub hopping_matrix: Array2<f64>,
    pub edge_protection: f64,
    pub dt: f64,
    pub time: f64,
}

impl TopologicalTimeCrystal {
    /// Create topological time crystal with protected edge modes
    pub fn new(n_sites: usize, hopping_strength: f64, edge_protection: f64, dt: f64) -> Self {
        let mut hopping_matrix = Array2::zeros((n_sites, n_sites));

        // SSH-like model: alternating hopping strengths
        for i in 0..n_sites - 1 {
            let hop = if i % 2 == 0 {
                hopping_strength * 1.5 // Strong bond
            } else {
                hopping_strength * 0.5 // Weak bond
            };
            hopping_matrix[[i, i + 1]] = hop;
            hopping_matrix[[i + 1, i]] = hop;
        }

        // Edge protection: reduce coupling at boundaries
        hopping_matrix[[0, 1]] *= edge_protection;
        hopping_matrix[[1, 0]] *= edge_protection;
        hopping_matrix[[n_sites - 2, n_sites - 1]] *= edge_protection;
        hopping_matrix[[n_sites - 1, n_sites - 2]] *= edge_protection;

        Self {
            n_sites,
            positions: Array1::zeros(n_sites),
            velocities: Array1::zeros(n_sites),
            hopping_matrix,
            edge_protection,
            dt,
            time: 0.0,
        }
    }

    /// Evolve with topological protection
    pub fn step(&mut self) {
        // Hopping forces
        let hopping_forces = self.hopping_matrix.dot(&self.positions);

        // Update positions and velocities
        Zip::from(&mut self.positions)
            .and(&self.velocities)
            .for_each(|p, &v| {
                *p += v * self.dt;
            });

        Zip::from(&mut self.velocities)
            .and(&self.positions)
            .and(&hopping_forces)
            .for_each(|v, &p, &hop| {
                *v += (-p + hop) * self.dt;
            });

        self.time += self.dt;
    }

    /// Measure edge localization (topological protection metric)
    pub fn edge_localization(&self) -> f64 {
        let edge_amplitude = self.positions[0].abs() + self.positions[self.n_sites - 1].abs();
        let bulk_amplitude: f64 = self
            .positions
            .iter()
            .skip(1)
            .take(self.n_sites - 2)
            .map(|&x| x.abs())
            .sum();

        if bulk_amplitude == 0.0 {
            1.0
        } else {
            edge_amplitude / (edge_amplitude + bulk_amplitude)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_dtc() {
        let n = 50;
        let coupling_matrix = Array2::zeros((n, n));
        let mut simd_dtc = SimdDTC::new(n, coupling_matrix, 8.0, 2.0, 0.5, 0.1, 0.001);

        let trajectory = simd_dtc.run_simd(0.5);
        assert_eq!(trajectory.len(), 500);
    }

    #[test]
    fn test_hierarchical_time_crystal() {
        let mut htc = HierarchicalTimeCrystal::new(4, 10, 8.0, 0.001);

        for _ in 0..1000 {
            htc.step();
        }

        let order_params = htc.hierarchical_order_parameter();
        assert_eq!(order_params.len(), 4);

        let capacity = htc.temporal_multiplexing_capacity();
        assert!(capacity > 0);
        println!("Temporal multiplexing capacity: {} bits", capacity);
    }

    #[test]
    fn test_topological_time_crystal() {
        let mut ttc = TopologicalTimeCrystal::new(20, 1.0, 0.3, 0.001);

        // Initialize edge mode
        ttc.positions[0] = 1.0;
        ttc.positions[19] = 1.0;

        for _ in 0..5000 {
            ttc.step();
        }

        let edge_loc = ttc.edge_localization();
        println!("Edge localization: {}", edge_loc);

        // Edge modes should remain somewhat localized
        assert!(edge_loc > 0.1);
    }
}
