//! Thermal noise sources: Gaussian (Langevin) and Poisson spike noise.

use rand::Rng;
use rand_distr::{Distribution, Normal, Poisson};

/// Draw a Gaussian noise sample with standard deviation σ = √(2/β).
///
/// This matches the fluctuation-dissipation theorem for overdamped Langevin:
/// the noise amplitude must be √(2kT) = √(2/β) in dimensionless units.
#[inline]
pub fn langevin_noise(beta: f32, rng: &mut impl Rng) -> f32 {
    if beta <= 0.0 || !beta.is_finite() {
        return 0.0;
    }
    let sigma = (2.0 / beta).sqrt();
    Normal::new(0.0_f32, sigma)
        .unwrap_or_else(|_| Normal::new(0.0_f32, 1e-6).unwrap())
        .sample(rng)
}

/// Draw `n` independent Langevin noise samples.
pub fn langevin_noise_vec(beta: f32, n: usize, rng: &mut impl Rng) -> Vec<f32> {
    if beta <= 0.0 || !beta.is_finite() {
        return vec![0.0; n];
    }
    let sigma = (2.0 / beta).sqrt();
    let dist = Normal::new(0.0_f32, sigma).unwrap_or_else(|_| Normal::new(0.0_f32, 1e-6).unwrap());
    (0..n).map(|_| dist.sample(rng)).collect()
}

/// Poisson spike noise: add a random kick of magnitude `kick` with rate λ.
///
/// Returns the kick to add to a single activation (0.0 if no spike this step).
#[inline]
pub fn poisson_spike(rate: f64, kick: f32, rng: &mut impl Rng) -> f32 {
    if rate <= 0.0 || !rate.is_finite() {
        return 0.0;
    }
    let dist = Poisson::new(rate).unwrap_or_else(|_| Poisson::new(1e-6).unwrap());
    let count = dist.sample(rng) as u64;
    if count > 0 {
        // Random sign
        let sign = if rng.gen::<bool>() { 1.0 } else { -1.0 };
        sign * kick * count as f32
    } else {
        0.0
    }
}
