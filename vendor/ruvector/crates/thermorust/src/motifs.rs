//! Pre-wired motif factories: ring, fully-connected, and Hopfield memory nets.

use crate::energy::{Couplings, Ising, SoftSpin};
use crate::state::State;

/// A self-contained motif: initial state + Ising Hamiltonian + default params.
pub struct IsingMotif {
    pub state: State,
    pub model: Ising,
}

impl IsingMotif {
    /// Ferromagnetic ring of `n` spins.  J_{i,i+1} = `strength`.
    pub fn ring(n: usize, strength: f32) -> Self {
        Self {
            state: State::ones(n),
            model: Ising::new(Couplings::ferromagnetic_ring(n, strength)),
        }
    }

    /// Fully connected ferromagnet: J_ij = strength for all i≠j.
    pub fn fully_connected(n: usize, strength: f32) -> Self {
        let mut j = vec![0.0_f32; n * n];
        for i in 0..n {
            for k in (i + 1)..n {
                j[i * n + k] = strength;
                j[k * n + i] = strength;
            }
        }
        Self {
            state: State::ones(n),
            model: Ising::new(Couplings { j, h: vec![0.0; n] }),
        }
    }

    /// Hopfield associative memory loaded with `patterns` (±1 binary vectors).
    pub fn hopfield(n: usize, patterns: &[Vec<f32>]) -> Self {
        Self {
            state: State::ones(n),
            model: Ising::new(Couplings::hopfield_memory(n, patterns)),
        }
    }
}

/// Soft-spin motif with double-well on-site potential for continuous activations.
pub struct SoftSpinMotif {
    pub state: State,
    pub model: SoftSpin,
}

impl SoftSpinMotif {
    /// Random-coupling soft-spin motif seeded with `seed`.
    pub fn random(n: usize, a: f32, b: f32, seed: u64) -> Self {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
        let j: Vec<f32> = (0..n * n).map(|_| rng.gen_range(-0.5_f32..0.5)).collect();
        // Symmetrise
        let mut j_sym = vec![0.0_f32; n * n];
        for i in 0..n {
            for k in 0..n {
                j_sym[i * n + k] = (j[i * n + k] + j[k * n + i]) * 0.5;
            }
        }
        let x: Vec<f32> = (0..n).map(|_| rng.gen_range(-0.1_f32..0.1)).collect();
        Self {
            state: State::from_vec(x),
            model: SoftSpin::new(
                Couplings {
                    j: j_sym,
                    h: vec![0.0; n],
                },
                a,
                b,
            ),
        }
    }
}
