//! ThermoLayer: thermodynamic coherence gate for exo-backend-classical.
//!
//! Wraps a `thermorust` Ising motif and treats the energy drop ΔE as a
//! **coherence λ-signal**: a large negative ΔE means the activation pattern
//! is "settling" (becoming coherent); a near-zero ΔE means it is already
//! at a local minimum or chaotically fluctuating at high temperature.
//!
//! The λ-signal can be used to gate min-cut operations or to weight
//! confidence scores in the ruvector-attn-mincut pipeline.
//!
//! # Integration sketch
//! ```no_run
//! use exo_backend_classical::thermo_layer::{ThermoLayer, ThermoConfig};
//!
//! let cfg = ThermoConfig { n: 16, beta: 3.0, steps_per_call: 20, ..Default::default() };
//! let mut layer = ThermoLayer::new(cfg);
//!
//! // Activations from an attention layer (length must equal `n`).
//! let mut acts = vec![0.5_f32; 16];
//! let signal = layer.run(&mut acts, 20);
//! println!("λ = {:.4}, dissipation = {:.3e} J", signal.lambda, signal.dissipation_j);
//! ```

use rand::SeedableRng;
use thermorust::{
    dynamics::{step_discrete, Params},
    energy::{Couplings, EnergyModel, Ising},
    metrics::magnetisation,
    State,
};

/// Configuration for a `ThermoLayer`.
#[derive(Clone, Debug)]
pub struct ThermoConfig {
    /// Number of units in the Ising motif (must match activation vector length).
    pub n: usize,
    /// Inverse temperature β = 1/(kT).  Higher = colder, more deterministic.
    pub beta: f32,
    /// Ferromagnetic coupling strength J for ring topology.
    pub coupling: f32,
    /// Metropolis steps executed per `run()` call.
    pub steps_per_call: usize,
    /// Landauer cost in Joules per accepted irreversible flip.
    pub irreversible_cost: f64,
    /// RNG seed (fixed → fully deterministic).
    pub seed: u64,
}

impl Default for ThermoConfig {
    fn default() -> Self {
        Self {
            n: 16,
            beta: 3.0,
            coupling: 0.2,
            steps_per_call: 20,
            irreversible_cost: 2.87e-21, // kT ln2 at 300 K
            seed: 0,
        }
    }
}

/// Thermodynamic coherence signal returned by `ThermoLayer::run`.
#[derive(Clone, Debug)]
pub struct ThermoSignal {
    /// λ-signal: −ΔE / |E_initial|  (positive = energy decreased = more coherent).
    pub lambda: f32,
    /// Magnetisation m ∈ [−1, 1] after update.
    pub magnetisation: f32,
    /// Cumulative Joules dissipated since layer creation.
    pub dissipation_j: f64,
    /// Energy after the update step.
    pub energy_after: f32,
}

/// Ising-motif thermodynamic gate.
pub struct ThermoLayer {
    model: Ising,
    state: State,
    params: Params,
    rng: rand::rngs::SmallRng,
}

impl ThermoLayer {
    /// Create a new `ThermoLayer` from `cfg`.
    pub fn new(cfg: ThermoConfig) -> Self {
        let couplings = Couplings::ferromagnetic_ring(cfg.n, cfg.coupling);
        let model = Ising::new(couplings);
        let state = State::ones(cfg.n);
        let params = Params {
            beta: cfg.beta,
            eta: 0.05,
            irreversible_cost: cfg.irreversible_cost,
            clamp_mask: vec![false; cfg.n],
        };
        let rng = rand::rngs::SmallRng::seed_from_u64(cfg.seed);
        Self {
            model,
            state,
            params,
            rng,
        }
    }

    /// Apply activations as external fields, run MH steps, return coherence signal.
    ///
    /// The activation vector is **modified in place** by the thermodynamic
    /// relaxation: each element is replaced by the Ising spin value after
    /// `steps_per_call` Metropolis updates.  Values are clamped to {-1, +1}.
    pub fn run(&mut self, activations: &mut [f32], steps: usize) -> ThermoSignal {
        let n = self.state.len().min(activations.len());

        // Clamp inputs to ±1 and load as spin state.
        for i in 0..n {
            self.state.x[i] = activations[i].clamp(-1.0, 1.0).signum();
        }

        let e_before = self.model.energy(&self.state);

        // Run Metropolis steps.
        for _ in 0..steps {
            step_discrete(&self.model, &mut self.state, &self.params, &mut self.rng);
        }

        let e_after = self.model.energy(&self.state);
        let d_e = e_after - e_before;
        let lambda = if e_before.abs() > 1e-9 {
            -d_e / e_before.abs()
        } else {
            0.0
        };

        // Write relaxed spins back to the caller's buffer.
        for i in 0..n {
            activations[i] = self.state.x[i];
        }

        ThermoSignal {
            lambda,
            magnetisation: magnetisation(&self.state),
            dissipation_j: self.state.dissipated_j,
            energy_after: e_after,
        }
    }

    /// Reset the spin state to all +1.
    pub fn reset(&mut self) {
        for xi in &mut self.state.x {
            *xi = 1.0;
        }
        self.state.dissipated_j = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thermo_layer_runs_without_panic() {
        let cfg = ThermoConfig {
            n: 8,
            steps_per_call: 10,
            ..Default::default()
        };
        let mut layer = ThermoLayer::new(cfg);
        let mut acts = vec![1.0_f32; 8];
        let sig = layer.run(&mut acts, 10);
        assert!(sig.lambda.is_finite());
        assert!(sig.magnetisation >= -1.0 && sig.magnetisation <= 1.0);
        assert!(sig.dissipation_j >= 0.0);
    }

    #[test]
    fn activations_are_binarised() {
        let cfg = ThermoConfig {
            n: 4,
            steps_per_call: 0,
            ..Default::default()
        };
        let mut layer = ThermoLayer::new(cfg);
        let mut acts = vec![0.7_f32, -0.3, 0.1, -0.9];
        layer.run(&mut acts, 0);
        for a in &acts {
            assert!(
                (*a - 1.0).abs() < 1e-6 || (*a + 1.0).abs() < 1e-6,
                "not ±1: {a}"
            );
        }
    }

    #[test]
    fn lambda_finite_after_many_steps() {
        let cfg = ThermoConfig {
            n: 16,
            beta: 5.0,
            ..Default::default()
        };
        let mut layer = ThermoLayer::new(cfg);
        for _ in 0..10 {
            let mut acts = vec![1.0_f32; 16];
            let sig = layer.run(&mut acts, 50);
            assert!(sig.lambda.is_finite());
        }
    }
}
