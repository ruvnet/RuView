//! Thermodynamic observables: magnetisation, entropy, free energy, overlap.

use crate::state::State;

/// Mean magnetisation: m = (1/n) Σᵢ xᵢ ∈ [−1, 1].
pub fn magnetisation(s: &State) -> f32 {
    if s.x.is_empty() {
        return 0.0;
    }
    s.x.iter().sum::<f32>() / s.x.len() as f32
}

/// Mean-squared activation: ⟨x²⟩.
pub fn mean_sq(s: &State) -> f32 {
    if s.x.is_empty() {
        return 0.0;
    }
    s.x.iter().map(|xi| xi * xi).sum::<f32>() / s.x.len() as f32
}

/// Pattern overlap (Hopfield order parameter):
///   m_μ = (1/n) Σᵢ ξᵢ^μ xᵢ
///
/// Returns `None` if lengths differ.
pub fn overlap(s: &State, pattern: &[f32]) -> Option<f32> {
    let n = s.x.len();
    if pattern.len() != n || n == 0 {
        return None;
    }
    let sum: f32 = s.x.iter().zip(pattern.iter()).map(|(xi, pi)| xi * pi).sum();
    Some(sum / n as f32)
}

/// Approximate configurational entropy (binary case) via:
///   S ≈ −n [ p ln p + (1−p) ln(1−p) ]
/// where p = fraction of spins at +1.
///
/// Returns 0 for edge cases (all ±1).
pub fn binary_entropy(s: &State) -> f32 {
    let n = s.x.len();
    if n == 0 {
        return 0.0;
    }
    let p_up = s.x.iter().filter(|&&xi| xi > 0.0).count() as f32 / n as f32;
    let p_dn = 1.0 - p_up;
    let h = |p: f32| {
        if p <= 0.0 || p >= 1.0 {
            0.0
        } else {
            -p * p.ln() - (1.0 - p) * (1.0 - p).ln()
        }
    };
    n as f32 * h(p_up) * 0.5 + n as f32 * h(p_dn) * 0.5
}

/// Estimate free energy: F ≈ E − T·S = E − S/β.
///
/// `energy` should be `model.energy(s)`.
pub fn free_energy(energy: f32, entropy: f32, beta: f32) -> f32 {
    energy - entropy / beta
}

/// Running statistics accumulator for energy / dissipation traces.
#[derive(Default, Debug, Clone)]
pub struct Trace {
    /// Energy samples (one per recorded step).
    pub energies: Vec<f32>,
    /// Cumulative dissipation at each recorded step.
    pub dissipation: Vec<f64>,
}

impl Trace {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one observation.
    pub fn push(&mut self, energy: f32, dissipated_j: f64) {
        self.energies.push(energy);
        self.dissipation.push(dissipated_j);
    }

    /// Mean energy over all recorded steps.
    pub fn mean_energy(&self) -> f32 {
        if self.energies.is_empty() {
            return 0.0;
        }
        self.energies.iter().sum::<f32>() / self.energies.len() as f32
    }

    /// Total heat shed over all steps.
    pub fn total_dissipation(&self) -> f64 {
        self.dissipation.last().copied().unwrap_or(0.0)
    }
}
