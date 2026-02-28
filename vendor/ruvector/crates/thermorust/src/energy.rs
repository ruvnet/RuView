//! Energy models: Ising/Hopfield Hamiltonian and the `EnergyModel` trait.

use crate::state::State;

/// Coupling weights and local fields for a fully-connected motif.
///
/// `j` is a flattened row-major `n×n` symmetric matrix; `h` is the `n`-vector
/// of local (bias) fields.
#[derive(Clone, Debug)]
pub struct Couplings {
    /// Symmetric coupling matrix J_ij (row-major, length n²).
    pub j: Vec<f32>,
    /// Local field h_i (length n).
    pub h: Vec<f32>,
}

impl Couplings {
    /// Build zero-coupling weights for `n` units.
    pub fn zeros(n: usize) -> Self {
        Self {
            j: vec![0.0; n * n],
            h: vec![0.0; n],
        }
    }

    /// Build ferromagnetic ring couplings: J_{i, i+1} = strength.
    pub fn ferromagnetic_ring(n: usize, strength: f32) -> Self {
        let mut j = vec![0.0; n * n];
        for i in 0..n {
            let next = (i + 1) % n;
            j[i * n + next] = strength;
            j[next * n + i] = strength;
        }
        Self { j, h: vec![0.0; n] }
    }

    /// Build random Hopfield memory couplings from a list of patterns.
    ///
    /// Patterns should be `±1` binary vectors of length `n`.
    pub fn hopfield_memory(n: usize, patterns: &[Vec<f32>]) -> Self {
        let mut j = vec![0.0f32; n * n];
        let scale = 1.0 / n as f32;
        for pat in patterns {
            assert_eq!(pat.len(), n, "pattern length must equal n");
            for i in 0..n {
                for k in (i + 1)..n {
                    let dj = scale * pat[i] * pat[k];
                    j[i * n + k] += dj;
                    j[k * n + i] += dj;
                }
            }
        }
        Self { j, h: vec![0.0; n] }
    }
}

/// Trait implemented by any Hamiltonian that can return a scalar energy.
pub trait EnergyModel {
    /// Compute the total energy of `state`.
    fn energy(&self, state: &State) -> f32;
}

/// Ising/Hopfield Hamiltonian:
///   H = −Σᵢ hᵢ xᵢ − Σᵢ<ⱼ Jᵢⱼ xᵢ xⱼ
#[derive(Clone, Debug)]
pub struct Ising {
    pub c: Couplings,
}

impl Ising {
    pub fn new(c: Couplings) -> Self {
        Self { c }
    }
}

impl EnergyModel for Ising {
    fn energy(&self, s: &State) -> f32 {
        let n = s.x.len();
        debug_assert_eq!(self.c.h.len(), n);
        let mut e = 0.0_f32;
        for i in 0..n {
            e -= self.c.h[i] * s.x[i];
            for j in (i + 1)..n {
                e -= self.c.j[i * n + j] * s.x[i] * s.x[j];
            }
        }
        e
    }
}

/// Soft-spin (XY-like) model with continuous activations.
///
/// Adds a quartic double-well self-energy per unit: −a·x² + b·x⁴
/// which promotes ±1 attractors.
#[derive(Clone, Debug)]
pub struct SoftSpin {
    pub c: Couplings,
    /// Well depth coefficient (>0 pushes spins toward ±1).
    pub a: f32,
    /// Quartic stiffness (>0 keeps spins bounded).
    pub b: f32,
}

impl SoftSpin {
    pub fn new(c: Couplings, a: f32, b: f32) -> Self {
        Self { c, a, b }
    }
}

impl EnergyModel for SoftSpin {
    fn energy(&self, s: &State) -> f32 {
        let n = s.x.len();
        let mut e = 0.0_f32;
        for i in 0..n {
            let xi = s.x[i];
            // Double-well self-energy
            e += -self.a * xi * xi + self.b * xi * xi * xi * xi;
            // Local field
            e -= self.c.h[i] * xi;
            for j in (i + 1)..n {
                e -= self.c.j[i * n + j] * xi * s.x[j];
            }
        }
        e
    }
}
