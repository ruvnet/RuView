//! Stochastic dynamics: Metropolis-Hastings (discrete) and overdamped Langevin (continuous).

use crate::energy::EnergyModel;
use crate::noise::{langevin_noise, poisson_spike};
use crate::state::State;
use rand::Rng;

/// Parameters governing thermal dynamics and Landauer dissipation accounting.
#[derive(Clone, Debug)]
pub struct Params {
    /// Inverse temperature β = 1/(kT).  Higher β → colder, less noise.
    pub beta: f32,
    /// Step size η for continuous (Langevin) updates.
    pub eta: f32,
    /// Joules of heat attributed to each accepted irreversible transition.
    /// Landauer's limit: kT ln2 ≈ 2.87 × 10⁻²¹ J at 300 K.
    pub irreversible_cost: f64,
    /// Which unit indices are clamped (fixed inputs).
    pub clamp_mask: Vec<bool>,
}

impl Params {
    /// Sensible defaults: room-temperature Landauer limit, no clamping.
    pub fn default_n(n: usize) -> Self {
        Self {
            beta: 2.0,
            eta: 0.05,
            irreversible_cost: 2.87e-21, // kT ln2 at 300 K in Joules
            clamp_mask: vec![false; n],
        }
    }

    #[inline]
    fn is_clamped(&self, i: usize) -> bool {
        self.clamp_mask.get(i).copied().unwrap_or(false)
    }
}

/// **Metropolis-Hastings** single spin-flip update for *discrete* Ising states.
///
/// Proposes flipping spin `i` (chosen uniformly at random), accepts with the
/// Boltzmann probability, and charges `p.irreversible_cost` on each accepted
/// non-zero-ΔE transition.
pub fn step_discrete<M: EnergyModel>(model: &M, s: &mut State, p: &Params, rng: &mut impl Rng) {
    let n = s.x.len();
    if n == 0 {
        return;
    }
    let i: usize = rng.gen_range(0..n);
    if p.is_clamped(i) {
        return;
    }

    let old_e = model.energy(s);
    let old_si = s.x[i];
    s.x[i] = -old_si;
    let new_e = model.energy(s);
    let d_e = (new_e - old_e) as f64;

    let accept = d_e <= 0.0 || {
        let prob = (-p.beta as f64 * d_e).exp();
        rng.gen::<f64>() < prob
    };

    if accept {
        if d_e != 0.0 {
            s.dissipated_j += p.irreversible_cost;
        }
    } else {
        s.x[i] = old_si;
    }
}

/// **Overdamped Langevin** update for *continuous* activations.
///
/// For each unclamped unit `i`:
///   xᵢ ← xᵢ − η · ∂H/∂xᵢ + √(2/β) · ξ
/// where ξ ~ N(0,1).  The gradient is estimated by central differences.
///
/// Optionally clips activations to `[-1, 1]` after the update.
pub fn step_continuous<M: EnergyModel>(model: &M, s: &mut State, p: &Params, rng: &mut impl Rng) {
    let n = s.x.len();
    let eps = 1e-3_f32;

    for i in 0..n {
        if p.is_clamped(i) {
            continue;
        }
        let old = s.x[i];

        // Central-difference gradient ∂H/∂xᵢ
        s.x[i] = old + eps;
        let e_plus = model.energy(s);
        s.x[i] = old - eps;
        let e_minus = model.energy(s);
        s.x[i] = old;

        let grad = (e_plus - e_minus) / (2.0 * eps);
        let noise = langevin_noise(p.beta, rng);
        let dx = -p.eta * grad + noise;

        let old_e = model.energy(s);
        s.x[i] = (old + dx).clamp(-1.0, 1.0);
        let new_e = model.energy(s);

        if (new_e as f64) < (old_e as f64) {
            s.dissipated_j += p.irreversible_cost;
        }
    }
}

/// Run `steps` discrete Metropolis updates, recording every `record_every`th
/// step into the optional `trace`.
pub fn anneal_discrete<M: EnergyModel>(
    model: &M,
    s: &mut State,
    p: &Params,
    steps: usize,
    record_every: usize,
    rng: &mut impl Rng,
) -> crate::metrics::Trace {
    let mut trace = crate::metrics::Trace::new();
    for step in 0..steps {
        step_discrete(model, s, p, rng);
        if record_every > 0 && step % record_every == 0 {
            trace.push(model.energy(s), s.dissipated_j);
        }
    }
    trace
}

/// Run `steps` Langevin updates, recording every `record_every`th step.
pub fn anneal_continuous<M: EnergyModel>(
    model: &M,
    s: &mut State,
    p: &Params,
    steps: usize,
    record_every: usize,
    rng: &mut impl Rng,
) -> crate::metrics::Trace {
    let mut trace = crate::metrics::Trace::new();
    for step in 0..steps {
        step_continuous(model, s, p, rng);
        if record_every > 0 && step % record_every == 0 {
            trace.push(model.energy(s), s.dissipated_j);
        }
    }
    trace
}

/// Inject Poisson spike noise into `s`, bypassing thermal Boltzmann acceptance.
///
/// Each unit has an independent probability `rate` (per step) of receiving a
/// kick of magnitude `kick`, with a random sign.
pub fn inject_spikes(s: &mut State, p: &Params, rate: f64, kick: f32, rng: &mut impl Rng) {
    for (i, xi) in s.x.iter_mut().enumerate() {
        if p.is_clamped(i) {
            continue;
        }
        let dk = poisson_spike(rate, kick, rng);
        *xi = (*xi + dk).clamp(-1.0, 1.0);
    }
}
