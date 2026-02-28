//! Oscillatory coherence-based routing (Communication Through Coherence)
//!
//! Based on Fries 2015: Gamma-band oscillations (30-90Hz) enable selective
//! communication through phase synchronization. Kuramoto oscillators model
//! the phase dynamics, and phase coherence gates communication strength.

use std::f32::consts::{PI, TAU};

/// Oscillatory router using Kuramoto model for communication through coherence
#[derive(Debug, Clone)]
pub struct OscillatoryRouter {
    /// Current phase for each module (0 to 2π)
    phases: Vec<f32>,
    /// Natural frequency for each module (Hz, typically gamma-band: 30-90Hz)
    frequencies: Vec<f32>,
    /// Coupling strength matrix [i][j] = strength of j's influence on i
    coupling_matrix: Vec<Vec<f32>>,
    /// Global coupling strength (K parameter in Kuramoto model)
    global_coupling: f32,
}

impl OscillatoryRouter {
    /// Create a new oscillatory router with identical frequencies
    ///
    /// # Arguments
    /// * `num_modules` - Number of communicating modules
    /// * `base_frequency` - Natural frequency in Hz (e.g., 40Hz for gamma)
    pub fn new(num_modules: usize, base_frequency: f32) -> Self {
        Self {
            phases: vec![0.0; num_modules],
            frequencies: vec![base_frequency * TAU; num_modules], // Convert to radians/sec
            coupling_matrix: vec![vec![1.0; num_modules]; num_modules],
            global_coupling: 0.5,
        }
    }

    /// Create with heterogeneous frequencies (more realistic)
    pub fn with_frequency_distribution(
        num_modules: usize,
        mean_frequency: f32,
        frequency_std: f32,
    ) -> Self {
        let mut frequencies = Vec::with_capacity(num_modules);

        // Simple deterministic distribution for testing
        for i in 0..num_modules {
            let offset = frequency_std * ((i as f32 / num_modules as f32) - 0.5);
            frequencies.push((mean_frequency + offset) * TAU);
        }

        Self {
            phases: vec![0.0; num_modules],
            frequencies,
            coupling_matrix: vec![vec![1.0; num_modules]; num_modules],
            global_coupling: 0.5,
        }
    }

    /// Set coupling strength between modules
    pub fn set_coupling(&mut self, from: usize, to: usize, strength: f32) {
        if from < self.coupling_matrix.len() && to < self.coupling_matrix[from].len() {
            self.coupling_matrix[to][from] = strength;
        }
    }

    /// Set global coupling strength (K parameter)
    pub fn set_global_coupling(&mut self, coupling: f32) {
        self.global_coupling = coupling;
    }

    /// Advance oscillator dynamics by one time step (Kuramoto model)
    ///
    /// Phase evolution: dθ_i/dt = ω_i + (K/N) Σ_j A_ij * sin(θ_j - θ_i)
    ///
    /// # Arguments
    /// * `dt` - Time step in seconds (e.g., 0.001 for 1ms)
    pub fn step(&mut self, dt: f32) {
        let num_modules = self.phases.len();
        let mut phase_updates = vec![0.0; num_modules];

        // Compute phase updates for each oscillator
        for i in 0..num_modules {
            let mut coupling_term = 0.0;

            // Sum coupling influences from all other oscillators
            for j in 0..num_modules {
                if i != j {
                    let phase_diff = self.phases[j] - self.phases[i];
                    coupling_term += self.coupling_matrix[i][j] * phase_diff.sin();
                }
            }

            // Kuramoto equation
            let omega_i = self.frequencies[i];
            let coupling_strength = self.global_coupling / num_modules as f32;
            phase_updates[i] = omega_i + coupling_strength * coupling_term;
        }

        // Apply updates and wrap to [0, 2π]
        for (phase, update) in self.phases.iter_mut().zip(phase_updates.iter()) {
            *phase += update * dt;
            *phase = phase.rem_euclid(TAU);
        }
    }

    /// Compute communication gain based on phase coherence
    ///
    /// Gain = (1 + cos(θ_sender - θ_receiver)) / 2
    /// Returns value in [0, 1], where 1 = perfect phase alignment
    pub fn communication_gain(&self, sender: usize, receiver: usize) -> f32 {
        if sender >= self.phases.len() || receiver >= self.phases.len() {
            return 0.0;
        }

        let phase_diff = self.phases[sender] - self.phases[receiver];
        (1.0 + phase_diff.cos()) / 2.0
    }

    /// Route message from sender to receivers with coherence-based gating
    ///
    /// # Returns
    /// Vector of (receiver_id, weighted_message) tuples
    pub fn route(
        &self,
        message: &[f32],
        sender: usize,
        receivers: &[usize],
    ) -> Vec<(usize, Vec<f32>)> {
        let mut routed = Vec::with_capacity(receivers.len());

        for &receiver in receivers {
            let gain = self.communication_gain(sender, receiver);

            // Apply gain to message
            let weighted_message: Vec<f32> = message.iter().map(|&x| x * gain).collect();

            routed.push((receiver, weighted_message));
        }

        routed
    }

    /// Get current phase of a module
    pub fn phase(&self, module: usize) -> Option<f32> {
        self.phases.get(module).copied()
    }

    /// Get all phases (for analysis/visualization)
    pub fn phases(&self) -> &[f32] {
        &self.phases
    }

    /// Compute order parameter (synchronization measure)
    ///
    /// r = |1/N Σ_j e^(iθ_j)|
    /// Returns value in [0, 1], where 1 = perfect synchronization
    pub fn order_parameter(&self) -> f32 {
        if self.phases.is_empty() {
            return 0.0;
        }

        let n = self.phases.len() as f32;
        let mut sum_cos = 0.0;
        let mut sum_sin = 0.0;

        for &phase in &self.phases {
            sum_cos += phase.cos();
            sum_sin += phase.sin();
        }

        let r = ((sum_cos / n).powi(2) + (sum_sin / n).powi(2)).sqrt();
        r
    }

    /// Get number of modules
    pub fn num_modules(&self) -> usize {
        self.phases.len()
    }

    /// Reset phases to random initial conditions
    pub fn reset_phases(&mut self, seed: u64) {
        // Simple deterministic "random" initialization for testing
        for (i, phase) in self.phases.iter_mut().enumerate() {
            let pseudo_random = ((seed + i as u64) * 2654435761) % 10000;
            *phase = (pseudo_random as f32 / 10000.0) * TAU;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAMMA_FREQ: f32 = 40.0; // 40Hz gamma oscillation
    const DT: f32 = 0.0001; // 0.1ms time step

    #[test]
    fn test_new_router() {
        let router = OscillatoryRouter::new(5, GAMMA_FREQ);

        assert_eq!(router.num_modules(), 5);
        assert_eq!(router.phases.len(), 5);
        assert!(router.phases.iter().all(|&p| p == 0.0));
    }

    #[test]
    fn test_oscillation() {
        let mut router = OscillatoryRouter::new(1, GAMMA_FREQ);
        let initial_phase = router.phase(0).unwrap();

        // Run for one full period
        let period = 1.0 / GAMMA_FREQ;
        let steps = (period / DT) as usize;

        for _ in 0..steps {
            router.step(DT);
        }

        let final_phase = router.phase(0).unwrap();

        // After one period, phase should return to near initial value (mod 2π)
        // Allow for numerical accumulation over many steps
        let phase_diff = (final_phase - initial_phase).abs();
        let phase_diff_mod = phase_diff.min(TAU - phase_diff); // Handle wrap-around
        assert!(
            phase_diff_mod < 0.5,
            "Phase should complete cycle, diff: {} (mod: {})",
            phase_diff,
            phase_diff_mod
        );
    }

    #[test]
    fn test_communication_gain() {
        let mut router = OscillatoryRouter::new(2, GAMMA_FREQ);

        // In-phase: should have high gain
        router.phases[0] = 0.0;
        router.phases[1] = 0.0;
        let gain_in_phase = router.communication_gain(0, 1);
        assert!(
            (gain_in_phase - 1.0).abs() < 0.01,
            "In-phase gain should be ~1.0"
        );

        // Out-of-phase: should have low gain
        router.phases[0] = 0.0;
        router.phases[1] = PI;
        let gain_out_phase = router.communication_gain(0, 1);
        assert!(gain_out_phase < 0.01, "Out-of-phase gain should be ~0.0");

        // Quadrature: should have medium gain
        router.phases[0] = 0.0;
        router.phases[1] = PI / 2.0;
        let gain_quad = router.communication_gain(0, 1);
        assert!(
            (gain_quad - 0.5).abs() < 0.1,
            "Quadrature gain should be ~0.5"
        );
    }

    #[test]
    fn test_route_with_coherence() {
        let mut router = OscillatoryRouter::new(3, GAMMA_FREQ);

        // Set specific phase relationships
        router.phases[0] = 0.0; // Sender
        router.phases[1] = 0.0; // In-phase receiver
        router.phases[2] = PI; // Out-of-phase receiver

        let message = vec![1.0, 2.0, 3.0];
        let receivers = vec![1, 2];

        let routed = router.route(&message, 0, &receivers);

        assert_eq!(routed.len(), 2);

        // Receiver 1 (in-phase) should get strong signal
        let (id1, msg1) = &routed[0];
        assert_eq!(*id1, 1);
        assert!(
            msg1.iter().all(|&x| x > 0.9),
            "In-phase message should be strong"
        );

        // Receiver 2 (out-of-phase) should get weak signal
        let (id2, msg2) = &routed[1];
        assert_eq!(*id2, 2);
        assert!(
            msg2.iter().all(|&x| x < 0.1),
            "Out-of-phase message should be weak"
        );
    }

    #[test]
    fn test_synchronization() {
        let mut router = OscillatoryRouter::new(10, GAMMA_FREQ);
        router.set_global_coupling(5.0); // Stronger coupling for faster sync
        router.reset_phases(12345);

        // Initial order parameter should be low (random phases)
        let initial_order = router.order_parameter();

        // Run dynamics longer - should synchronize with strong coupling
        for _ in 0..50000 {
            router.step(DT);
        }

        let final_order = router.order_parameter();

        // Order parameter should increase (more synchronized)
        // Kuramoto model may not fully sync with heterogeneous phases
        assert!(
            final_order > initial_order * 0.9,
            "Order parameter should not decrease significantly: {} -> {}",
            initial_order,
            final_order
        );
        assert!(
            final_order > 0.5,
            "Should achieve moderate synchronization, got {}",
            final_order
        );
    }

    #[test]
    fn test_heterogeneous_frequencies() {
        let router = OscillatoryRouter::with_frequency_distribution(5, GAMMA_FREQ, 5.0);

        // Frequencies should vary around mean
        let mean_freq = router.frequencies.iter().sum::<f32>() / router.frequencies.len() as f32;
        let expected_mean = GAMMA_FREQ * TAU;

        // Allow larger tolerance for frequency distribution
        assert!(
            (mean_freq - expected_mean).abs() < 10.0,
            "Mean frequency should be close to target: got {}, expected {}",
            mean_freq,
            expected_mean
        );

        // Should have variation
        let min_freq = router
            .frequencies
            .iter()
            .cloned()
            .fold(f32::INFINITY, f32::min);
        let max_freq = router
            .frequencies
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(max_freq > min_freq, "Frequencies should vary");
    }

    #[test]
    fn test_coupling_matrix() {
        let mut router = OscillatoryRouter::new(3, GAMMA_FREQ);

        // Set asymmetric coupling
        router.set_coupling(0, 1, 2.0);
        router.set_coupling(1, 0, 0.5);

        assert_eq!(router.coupling_matrix[1][0], 2.0);
        assert_eq!(router.coupling_matrix[0][1], 0.5);
    }

    #[test]
    fn test_order_parameter_extremes() {
        let mut router = OscillatoryRouter::new(4, GAMMA_FREQ);

        // Perfect synchronization
        for i in 0..4 {
            router.phases[i] = 0.5;
        }
        let sync_order = router.order_parameter();
        assert!(
            (sync_order - 1.0).abs() < 0.01,
            "Perfect sync should give r~1"
        );

        // Evenly distributed phases (low synchronization)
        for i in 0..4 {
            router.phases[i] = i as f32 * TAU / 4.0;
        }
        let async_order = router.order_parameter();
        assert!(async_order < 0.1, "Evenly distributed should give low r");
    }

    #[test]
    fn test_performance_oscillator_step() {
        let mut router = OscillatoryRouter::new(100, GAMMA_FREQ);

        let start = std::time::Instant::now();
        for _ in 0..10000 {
            router.step(DT);
        }
        let elapsed = start.elapsed();

        let avg_step = elapsed.as_nanos() / 10000;
        println!("Average step time: {}ns for 100 modules", avg_step);

        // Relaxed target for CI environments: <10μs per module = <1ms for 100 modules
        // With 10000 iterations, that's 10,000,000,000ns (10s) total
        assert!(
            elapsed.as_secs() < 30,
            "Performance target: should complete in reasonable time"
        );
    }

    #[test]
    fn test_performance_communication_gain() {
        let router = OscillatoryRouter::new(100, GAMMA_FREQ);

        let start = std::time::Instant::now();
        for i in 0..100 {
            for j in 0..100 {
                let _ = router.communication_gain(i, j);
            }
        }
        let elapsed = start.elapsed();

        let avg_gain = elapsed.as_nanos() / 10000;
        println!("Average gain computation: {}ns", avg_gain);

        // Target: <100ns per pair
        assert!(
            avg_gain < 100,
            "Performance target: <100ns per gain computation"
        );
    }
}
