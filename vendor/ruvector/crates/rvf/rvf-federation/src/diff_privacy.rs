//! Differential privacy primitives for federated learning.
//!
//! Provides calibrated noise injection, gradient clipping, and a Renyi
//! Differential Privacy (RDP) accountant for tracking cumulative privacy loss.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rand_distr::{Distribution, Normal};

use crate::error::FederationError;
use crate::types::{DiffPrivacyProof, NoiseMechanism};

/// Differential privacy engine for adding calibrated noise.
pub struct DiffPrivacyEngine {
    /// Target epsilon (privacy loss bound).
    epsilon: f64,
    /// Target delta (probability of exceeding epsilon).
    delta: f64,
    /// L2 sensitivity bound.
    sensitivity: f64,
    /// Gradient clipping norm.
    clipping_norm: f64,
    /// Noise mechanism.
    mechanism: NoiseMechanism,
    /// Random number generator.
    rng: StdRng,
}

impl DiffPrivacyEngine {
    /// Create a new DP engine with Gaussian mechanism.
    ///
    /// Default: epsilon=1.0, delta=1e-5 (strong privacy).
    pub fn gaussian(
        epsilon: f64,
        delta: f64,
        sensitivity: f64,
        clipping_norm: f64,
    ) -> Result<Self, FederationError> {
        if epsilon <= 0.0 {
            return Err(FederationError::InvalidEpsilon(epsilon));
        }
        if delta <= 0.0 || delta >= 1.0 {
            return Err(FederationError::InvalidDelta(delta));
        }
        Ok(Self {
            epsilon,
            delta,
            sensitivity,
            clipping_norm,
            mechanism: NoiseMechanism::Gaussian,
            rng: StdRng::from_rng(rand::thread_rng()).unwrap(),
        })
    }

    /// Create a new DP engine with Laplace mechanism.
    pub fn laplace(
        epsilon: f64,
        sensitivity: f64,
        clipping_norm: f64,
    ) -> Result<Self, FederationError> {
        if epsilon <= 0.0 {
            return Err(FederationError::InvalidEpsilon(epsilon));
        }
        Ok(Self {
            epsilon,
            delta: 0.0,
            sensitivity,
            clipping_norm,
            mechanism: NoiseMechanism::Laplace,
            rng: StdRng::from_rng(rand::thread_rng()).unwrap(),
        })
    }

    /// Create with a deterministic seed (for testing).
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.rng = StdRng::seed_from_u64(seed);
        self
    }

    /// Compute the Gaussian noise standard deviation (sigma).
    fn gaussian_sigma(&self) -> f64 {
        self.sensitivity * (2.0_f64 * (1.25_f64 / self.delta).ln()).sqrt() / self.epsilon
    }

    /// Compute the Laplace noise scale (b).
    fn laplace_scale(&self) -> f64 {
        self.sensitivity / self.epsilon
    }

    /// Clip a gradient vector to the configured L2 norm bound.
    pub fn clip_gradients(&self, gradients: &mut [f64]) {
        let norm: f64 = gradients.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm > self.clipping_norm {
            let scale = self.clipping_norm / norm;
            for g in gradients.iter_mut() {
                *g *= scale;
            }
        }
    }

    /// Add calibrated noise to a vector of parameters.
    ///
    /// Clips gradients first, then adds noise per the configured mechanism.
    pub fn add_noise(&mut self, params: &mut [f64]) -> DiffPrivacyProof {
        self.clip_gradients(params);

        match self.mechanism {
            NoiseMechanism::Gaussian => {
                let sigma = self.gaussian_sigma();
                let normal = Normal::new(0.0, sigma).unwrap();
                for p in params.iter_mut() {
                    *p += normal.sample(&mut self.rng);
                }
                DiffPrivacyProof {
                    epsilon: self.epsilon,
                    delta: self.delta,
                    mechanism: NoiseMechanism::Gaussian,
                    sensitivity: self.sensitivity,
                    clipping_norm: self.clipping_norm,
                    noise_scale: sigma,
                    noised_parameter_count: params.len() as u64,
                }
            }
            NoiseMechanism::Laplace => {
                let b = self.laplace_scale();
                for p in params.iter_mut() {
                    // Laplace noise via inverse CDF: b * sign(u-0.5) * ln(1 - 2|u-0.5|)
                    let u: f64 = self.rng.gen::<f64>() - 0.5;
                    let noise = -b * u.signum() * (1.0 - 2.0 * u.abs()).ln();
                    *p += noise;
                }
                DiffPrivacyProof {
                    epsilon: self.epsilon,
                    delta: 0.0,
                    mechanism: NoiseMechanism::Laplace,
                    sensitivity: self.sensitivity,
                    clipping_norm: self.clipping_norm,
                    noise_scale: b,
                    noised_parameter_count: params.len() as u64,
                }
            }
        }
    }

    /// Add noise to a single scalar value.
    pub fn add_noise_scalar(&mut self, value: &mut f64) -> f64 {
        let mut v = [*value];
        self.add_noise(&mut v);
        *value = v[0];
        v[0]
    }

    /// Current epsilon setting.
    pub fn epsilon(&self) -> f64 {
        self.epsilon
    }

    /// Current delta setting.
    pub fn delta(&self) -> f64 {
        self.delta
    }
}

// -- Privacy Accountant (RDP) ------------------------------------------------

/// Renyi Differential Privacy (RDP) accountant for tracking cumulative privacy loss.
///
/// Tracks privacy budget across multiple export rounds using RDP composition,
/// which provides tighter bounds than naive (epsilon, delta)-DP composition.
pub struct PrivacyAccountant {
    /// Maximum allowed cumulative epsilon.
    epsilon_limit: f64,
    /// Target delta for conversion from RDP to (epsilon, delta)-DP.
    target_delta: f64,
    /// Accumulated RDP values at various alpha orders.
    /// Each entry: (alpha_order, accumulated_rdp_epsilon)
    rdp_alphas: Vec<(f64, f64)>,
    /// History of exports: (timestamp, epsilon_spent, mechanism).
    history: Vec<ExportRecord>,
}

/// Record of a single privacy-consuming export.
#[derive(Clone, Debug)]
pub struct ExportRecord {
    /// UNIX timestamp of the export.
    pub timestamp_s: u64,
    /// Epsilon consumed by this export.
    pub epsilon: f64,
    /// Delta for this export (0 for pure epsilon-DP).
    pub delta: f64,
    /// Mechanism used.
    pub mechanism: NoiseMechanism,
    /// Number of parameters.
    pub parameter_count: u64,
}

impl PrivacyAccountant {
    /// Create a new accountant with the given budget.
    pub fn new(epsilon_limit: f64, target_delta: f64) -> Self {
        // Standard RDP alpha orders for accounting
        let alphas: Vec<f64> = vec![
            1.5, 1.75, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0, 512.0,
            1024.0,
        ];
        let rdp_alphas = alphas.into_iter().map(|a| (a, 0.0)).collect();
        Self {
            epsilon_limit,
            target_delta,
            rdp_alphas,
            history: Vec::new(),
        }
    }

    /// Compute RDP epsilon for the Gaussian mechanism at a given alpha order.
    fn gaussian_rdp(alpha: f64, sigma: f64) -> f64 {
        alpha / (2.0 * sigma * sigma)
    }

    /// Convert RDP to (epsilon, delta)-DP for a given alpha order.
    fn rdp_to_dp(alpha: f64, rdp_epsilon: f64, delta: f64) -> f64 {
        rdp_epsilon - (delta.ln()) / (alpha - 1.0)
    }

    /// Record a Gaussian mechanism query.
    pub fn record_gaussian(&mut self, sigma: f64, epsilon: f64, delta: f64, parameter_count: u64) {
        // Accumulate RDP at each alpha order
        for (alpha, rdp_eps) in &mut self.rdp_alphas {
            *rdp_eps += Self::gaussian_rdp(*alpha, sigma);
        }
        self.history.push(ExportRecord {
            timestamp_s: 0,
            epsilon,
            delta,
            mechanism: NoiseMechanism::Gaussian,
            parameter_count,
        });
    }

    /// Record a Laplace mechanism query.
    pub fn record_laplace(&mut self, epsilon: f64, parameter_count: u64) {
        // For Laplace, RDP epsilon at order alpha is: alpha * eps / (alpha - 1)
        // when alpha > 1
        for (alpha, rdp_eps) in &mut self.rdp_alphas {
            if *alpha > 1.0 {
                *rdp_eps += *alpha * epsilon / (*alpha - 1.0);
            }
        }
        self.history.push(ExportRecord {
            timestamp_s: 0,
            epsilon,
            delta: 0.0,
            mechanism: NoiseMechanism::Laplace,
            parameter_count,
        });
    }

    /// Get the current best (tightest) epsilon estimate.
    pub fn current_epsilon(&self) -> f64 {
        self.rdp_alphas
            .iter()
            .map(|(alpha, rdp_eps)| Self::rdp_to_dp(*alpha, *rdp_eps, self.target_delta))
            .fold(f64::INFINITY, f64::min)
    }

    /// Remaining privacy budget.
    pub fn remaining_budget(&self) -> f64 {
        (self.epsilon_limit - self.current_epsilon()).max(0.0)
    }

    /// Check if we can afford another export with the given epsilon.
    pub fn can_afford(&self, additional_epsilon: f64) -> bool {
        self.current_epsilon() + additional_epsilon <= self.epsilon_limit
    }

    /// Check if budget is exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.current_epsilon() >= self.epsilon_limit
    }

    /// Fraction of budget consumed (0.0 to 1.0+).
    pub fn budget_fraction_used(&self) -> f64 {
        self.current_epsilon() / self.epsilon_limit
    }

    /// Number of exports recorded.
    pub fn export_count(&self) -> usize {
        self.history.len()
    }

    /// Export history.
    pub fn history(&self) -> &[ExportRecord] {
        &self.history
    }

    /// Epsilon limit.
    pub fn epsilon_limit(&self) -> f64 {
        self.epsilon_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaussian_engine_creates() {
        let engine = DiffPrivacyEngine::gaussian(1.0, 1e-5, 1.0, 1.0);
        assert!(engine.is_ok());
    }

    #[test]
    fn invalid_epsilon_rejected() {
        let engine = DiffPrivacyEngine::gaussian(0.0, 1e-5, 1.0, 1.0);
        assert!(engine.is_err());
        let engine = DiffPrivacyEngine::gaussian(-1.0, 1e-5, 1.0, 1.0);
        assert!(engine.is_err());
    }

    #[test]
    fn invalid_delta_rejected() {
        let engine = DiffPrivacyEngine::gaussian(1.0, 0.0, 1.0, 1.0);
        assert!(engine.is_err());
        let engine = DiffPrivacyEngine::gaussian(1.0, 1.0, 1.0, 1.0);
        assert!(engine.is_err());
    }

    #[test]
    fn gradient_clipping() {
        let engine = DiffPrivacyEngine::gaussian(1.0, 1e-5, 1.0, 1.0).unwrap();
        let mut grads = vec![3.0, 4.0]; // norm = 5.0
        engine.clip_gradients(&mut grads);
        let norm: f64 = grads.iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6); // clipped to norm 1.0
    }

    #[test]
    fn gradient_no_clip_when_small() {
        let engine = DiffPrivacyEngine::gaussian(1.0, 1e-5, 1.0, 10.0).unwrap();
        let mut grads = vec![3.0, 4.0]; // norm = 5.0, clip = 10.0
        engine.clip_gradients(&mut grads);
        assert!((grads[0] - 3.0).abs() < 1e-10);
        assert!((grads[1] - 4.0).abs() < 1e-10);
    }

    #[test]
    fn add_noise_gaussian_deterministic() {
        let mut engine = DiffPrivacyEngine::gaussian(1.0, 1e-5, 1.0, 100.0)
            .unwrap()
            .with_seed(42);
        let mut params = vec![1.0, 2.0, 3.0];
        let original = params.clone();
        let proof = engine.add_noise(&mut params);
        assert_eq!(proof.mechanism, NoiseMechanism::Gaussian);
        assert_eq!(proof.noised_parameter_count, 3);
        // Params should be different from original (noise added)
        assert!(params
            .iter()
            .zip(original.iter())
            .any(|(a, b)| (a - b).abs() > 1e-10));
    }

    #[test]
    fn add_noise_laplace_deterministic() {
        let mut engine = DiffPrivacyEngine::laplace(1.0, 1.0, 100.0)
            .unwrap()
            .with_seed(42);
        let mut params = vec![1.0, 2.0, 3.0];
        let proof = engine.add_noise(&mut params);
        assert_eq!(proof.mechanism, NoiseMechanism::Laplace);
        assert_eq!(proof.noised_parameter_count, 3);
    }

    #[test]
    fn privacy_accountant_initial_state() {
        let acc = PrivacyAccountant::new(10.0, 1e-5);
        assert_eq!(acc.export_count(), 0);
        assert!(!acc.is_exhausted());
        assert!(acc.can_afford(1.0));
        assert!(acc.remaining_budget() > 9.9);
    }

    #[test]
    fn privacy_accountant_tracks_gaussian() {
        let mut acc = PrivacyAccountant::new(10.0, 1e-5);
        // sigma=1.0 with epsilon=1.0 per query
        acc.record_gaussian(1.0, 1.0, 1e-5, 100);
        assert_eq!(acc.export_count(), 1);
        let eps = acc.current_epsilon();
        assert!(eps > 0.0);
        assert!(eps < 10.0);
    }

    #[test]
    fn privacy_accountant_composition() {
        let mut acc = PrivacyAccountant::new(10.0, 1e-5);
        let eps_after_1 = {
            acc.record_gaussian(1.0, 1.0, 1e-5, 100);
            acc.current_epsilon()
        };
        acc.record_gaussian(1.0, 1.0, 1e-5, 100);
        let eps_after_2 = acc.current_epsilon();
        // After 2 queries, epsilon should be larger
        assert!(eps_after_2 > eps_after_1);
    }

    #[test]
    fn privacy_accountant_exhaustion() {
        let mut acc = PrivacyAccountant::new(1.0, 1e-5);
        // Use a very small sigma to burn budget fast
        for _ in 0..100 {
            acc.record_gaussian(0.1, 10.0, 1e-5, 10);
        }
        assert!(acc.is_exhausted());
        assert!(!acc.can_afford(0.1));
    }
}
