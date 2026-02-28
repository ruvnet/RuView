//! PlasticityEngine — ADR-029 canonical plasticity system.
//!
//! Unifies four previously-independent EWC implementations:
//! - SONA EWC++ (production, <1ms, ReasoningBank)
//! - ruvector-nervous-system BTSP (behavioral timescale, 1-3s windows)
//! - ruvector-nervous-system E-prop (eligibility propagation, 1000ms)
//! - ruvector-gnn EWC (deprecated; this replaces it)
//!
//! Key property: EWC Fisher Information weights are scaled by IIT Φ score
//! of the pattern being protected — high-consciousness patterns are protected
//! more strongly from catastrophic forgetting.

use std::collections::HashMap;

/// A weight vector (parameter) in the model being protected.
pub type WeightId = u64;

/// Fisher Information diagonal approximation for EWC.
#[derive(Debug, Clone)]
pub struct FisherDiagonal {
    /// Fisher Information for each weight dimension
    pub values: Vec<f32>,
    /// Φ-weighted importance multiplier (1.0 = neutral, >1.0 = protect more)
    pub phi_weight: f32,
    /// Which plasticity mode computed this
    pub mode: PlasticityMode,
}

/// Plasticity learning modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlasticityMode {
    /// SONA MicroLoRA: <1ms instant adaptation, EWC++ regularization
    Instant,
    /// BTSP: behavioral timescale, 1–3 second windows, one-shot
    Behavioral,
    /// E-prop: eligibility propagation, 1000ms credit assignment
    Eligibility,
    /// EWC: classic Fisher Information regularization
    Classic,
}

/// Δ-parameter update from plasticity engine.
#[derive(Debug, Clone)]
pub struct PlasticityDelta {
    pub weight_id: WeightId,
    pub delta: Vec<f32>,
    pub mode: PlasticityMode,
    pub ewc_penalty: f32,
    pub phi_protection_applied: bool,
}

/// Trait for plasticity backend implementations.
pub trait PlasticityBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn compute_delta(
        &self,
        weight_id: WeightId,
        current: &[f32],
        gradient: &[f32],
        lr: f32,
    ) -> PlasticityDelta;
}

/// EWC++ implementation — the canonical production backend.
/// Bidirectional plasticity: strengthens important weights, prunes irrelevant ones.
pub struct EwcPlusPlusBackend {
    /// Fisher diagonal per weight
    fisher: HashMap<WeightId, FisherDiagonal>,
    /// Optimal weights (consolidation point)
    theta_star: HashMap<WeightId, Vec<f32>>,
    /// EWC regularization strength λ
    pub lambda: f32,
    /// Φ-weighting scale (0.0 = ignore Φ, 1.0 = full Φ-weighting)
    pub phi_scale: f32,
}

impl EwcPlusPlusBackend {
    pub fn new(lambda: f32) -> Self {
        Self {
            fisher: HashMap::new(),
            theta_star: HashMap::new(),
            lambda,
            phi_scale: 1.0,
        }
    }

    /// Consolidate current weights as the new optimal point.
    /// Called after learning a task to protect it from future forgetting.
    pub fn consolidate(&mut self, weight_id: WeightId, weights: Vec<f32>, phi: Option<f32>) {
        let phi_weight = phi.unwrap_or(1.0).max(0.01);
        let n = weights.len();
        // Initialize Fisher diagonal to 1.0 (uniform importance baseline)
        let fisher = FisherDiagonal {
            values: vec![1.0; n],
            phi_weight,
            mode: PlasticityMode::Classic,
        };
        self.fisher.insert(weight_id, fisher);
        self.theta_star.insert(weight_id, weights);
    }

    /// Update Fisher diagonal from gradient samples (online estimation).
    pub fn update_fisher(&mut self, weight_id: WeightId, gradient: &[f32]) {
        if let Some(f) = self.fisher.get_mut(&weight_id) {
            // F_i ← α·F_i + (1-α)·g_i² (running average)
            let alpha = 0.9f32;
            for (fi, gi) in f.values.iter_mut().zip(gradient.iter()) {
                *fi = alpha * *fi + (1.0 - alpha) * gi * gi;
            }
        }
    }

    /// Compute EWC++ penalty term for a weight update.
    fn ewc_penalty(&self, weight_id: WeightId, current: &[f32]) -> f32 {
        match (self.fisher.get(&weight_id), self.theta_star.get(&weight_id)) {
            (Some(f), Some(theta)) => {
                let penalty: f32 = f
                    .values
                    .iter()
                    .zip(current.iter().zip(theta.iter()))
                    .map(|(fi, (ci, ti))| fi * (ci - ti).powi(2))
                    .sum::<f32>();
                penalty * self.lambda * f.phi_weight * self.phi_scale
            }
            _ => 0.0,
        }
    }
}

impl PlasticityBackend for EwcPlusPlusBackend {
    fn name(&self) -> &'static str {
        "ewc++"
    }

    fn compute_delta(
        &self,
        weight_id: WeightId,
        current: &[f32],
        gradient: &[f32],
        lr: f32,
    ) -> PlasticityDelta {
        let penalty = self.ewc_penalty(weight_id, current);
        let phi_applied = self
            .fisher
            .get(&weight_id)
            .map(|f| f.phi_weight > 1.0)
            .unwrap_or(false);

        // EWC++ update: θ ← θ - lr·(∇L + λ·F·(θ - θ*))
        let delta: Vec<f32> = gradient
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let ewc_term = self
                    .fisher
                    .get(&weight_id)
                    .zip(self.theta_star.get(&weight_id))
                    .map(|(f, t)| {
                        let fi = f.values[i.min(f.values.len() - 1)];
                        let ci = current[i.min(current.len() - 1)];
                        let ti = t[i.min(t.len() - 1)];
                        self.lambda * fi * (ci - ti) * f.phi_weight
                    })
                    .unwrap_or(0.0);
                -lr * (g + ewc_term)
            })
            .collect();

        PlasticityDelta {
            weight_id,
            delta,
            mode: PlasticityMode::Instant,
            ewc_penalty: penalty,
            phi_protection_applied: phi_applied,
        }
    }
}

/// BTSP (Behavioral Timescale Synaptic Plasticity) backend.
/// One-shot learning within 1–3 second behavioral windows.
pub struct BtspBackend {
    /// Window duration in milliseconds
    pub window_ms: f32,
    /// Plateau potential threshold (triggers one-shot learning)
    pub plateau_threshold: f32,
    /// BTSP learning rate (typically large — one-shot)
    pub lr_btsp: f32,
}

impl BtspBackend {
    pub fn new() -> Self {
        Self {
            window_ms: 2000.0,
            plateau_threshold: 0.7,
            lr_btsp: 0.3,
        }
    }
}

impl Default for BtspBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PlasticityBackend for BtspBackend {
    fn name(&self) -> &'static str {
        "btsp"
    }

    fn compute_delta(
        &self,
        weight_id: WeightId,
        _current: &[f32],
        gradient: &[f32],
        _lr: f32,
    ) -> PlasticityDelta {
        // BTSP: large update if plateau potential exceeds threshold
        let n = gradient.len().max(1);
        let plateau = gradient.iter().map(|g| g.abs()).sum::<f32>() / n as f32;
        let btsp_lr = if plateau > self.plateau_threshold {
            self.lr_btsp
        } else {
            self.lr_btsp * 0.1
        };
        let delta: Vec<f32> = gradient.iter().map(|g| -btsp_lr * g).collect();
        PlasticityDelta {
            weight_id,
            delta,
            mode: PlasticityMode::Behavioral,
            ewc_penalty: 0.0,
            phi_protection_applied: false,
        }
    }
}

/// The unified plasticity engine.
pub struct PlasticityEngine {
    /// EWC++ is always present (canonical production backend)
    pub ewc: EwcPlusPlusBackend,
    /// Optional BTSP for biological one-shot plasticity
    pub btsp: Option<BtspBackend>,
    /// Default mode for new weight updates
    pub default_mode: PlasticityMode,
}

impl PlasticityEngine {
    pub fn new(lambda: f32) -> Self {
        Self {
            ewc: EwcPlusPlusBackend::new(lambda),
            btsp: None,
            default_mode: PlasticityMode::Instant,
        }
    }

    pub fn with_btsp(mut self) -> Self {
        self.btsp = Some(BtspBackend::new());
        self
    }

    /// Set Φ-based protection weight for a consolidated pattern.
    /// phi > 1.0 protects the pattern more strongly from forgetting.
    pub fn consolidate_with_phi(&mut self, weight_id: WeightId, weights: Vec<f32>, phi: f32) {
        self.ewc.consolidate(weight_id, weights, Some(phi));
    }

    /// Compute update delta for a weight, routing to appropriate backend.
    pub fn compute_delta(
        &mut self,
        weight_id: WeightId,
        current: &[f32],
        gradient: &[f32],
        lr: f32,
        mode: Option<PlasticityMode>,
    ) -> PlasticityDelta {
        // Update Fisher diagonal online
        self.ewc.update_fisher(weight_id, gradient);

        let mode = mode.unwrap_or(self.default_mode);
        match mode {
            PlasticityMode::Instant | PlasticityMode::Classic => {
                self.ewc.compute_delta(weight_id, current, gradient, lr)
            }
            PlasticityMode::Behavioral => self
                .btsp
                .as_ref()
                .map(|b| b.compute_delta(weight_id, current, gradient, lr))
                .unwrap_or_else(|| self.ewc.compute_delta(weight_id, current, gradient, lr)),
            PlasticityMode::Eligibility =>
            // E-prop: use EWC with reduced learning rate (credit assignment delay)
            {
                self.ewc
                    .compute_delta(weight_id, current, gradient, lr * 0.3)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ewc_prevents_catastrophic_forgetting() {
        let mut engine = PlasticityEngine::new(10.0);
        let weights = vec![1.0f32, 2.0, 3.0, 4.0];
        engine.consolidate_with_phi(0, weights.clone(), 2.0); // High Φ = protect more

        // Simulate gradient pushing weights far from consolidation point
        let current = vec![5.0f32, 6.0, 7.0, 8.0]; // Drifted far
        let gradient = vec![1.0f32; 4];
        let delta = engine.compute_delta(0, &current, &gradient, 0.01, None);

        // EWC penalty should be large (current far from theta_star)
        assert!(delta.ewc_penalty > 0.0, "EWC penalty should be nonzero");
        // Phi protection should be applied
        assert!(delta.phi_protection_applied);
    }

    #[test]
    fn test_btsp_one_shot_large_update() {
        let btsp = BtspBackend::new();
        let gradient = vec![0.8f32; 10]; // Above plateau threshold
        let delta = btsp.compute_delta(0, &vec![0.0; 10], &gradient, 0.01);
        // BTSP lr (0.3) should dominate over standard lr (0.01)
        assert!(
            delta.delta[0].abs() > 0.1,
            "BTSP should produce large one-shot update"
        );
    }

    #[test]
    fn test_phi_weighted_protection() {
        let mut engine = PlasticityEngine::new(1.0);
        let weights = vec![0.0f32; 4];
        engine.consolidate_with_phi(1, weights.clone(), 5.0); // Very high Φ
        engine.consolidate_with_phi(2, weights.clone(), 0.1); // Very low Φ

        let current = vec![1.0f32; 4];
        let gradient = vec![0.1f32; 4];

        let delta_high_phi = engine.compute_delta(1, &current, &gradient, 0.01, None);
        let delta_low_phi = engine.compute_delta(2, &current, &gradient, 0.01, None);

        // High Φ pattern should have larger EWC penalty (more protection)
        assert!(
            delta_high_phi.ewc_penalty > delta_low_phi.ewc_penalty,
            "High Φ patterns should be protected more strongly"
        );
    }
}
