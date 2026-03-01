//! SONA online adaptation: LoRA + EWC++ for WiFi-DensePose (ADR-023 Phase 5).
//!
//! Enables rapid low-parameter adaptation to changing WiFi environments without
//! catastrophic forgetting. All arithmetic uses `f32`, no external dependencies.

use std::collections::VecDeque;

// ── LoRA Adapter ────────────────────────────────────────────────────────────

/// Low-Rank Adaptation layer storing factorised delta `scale * A * B`.
#[derive(Debug, Clone)]
pub struct LoraAdapter {
    pub a: Vec<Vec<f32>>,          // (in_features, rank)
    pub b: Vec<Vec<f32>>,          // (rank, out_features)
    pub scale: f32,                // alpha / rank
    pub in_features: usize,
    pub out_features: usize,
    pub rank: usize,
}

impl LoraAdapter {
    pub fn new(in_features: usize, out_features: usize, rank: usize, alpha: f32) -> Self {
        Self {
            a: vec![vec![0.0f32; rank]; in_features],
            b: vec![vec![0.0f32; out_features]; rank],
            scale: alpha / rank.max(1) as f32,
            in_features, out_features, rank,
        }
    }

    /// Compute `scale * input * A * B`, returning a vector of length `out_features`.
    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        assert_eq!(input.len(), self.in_features);
        let mut hidden = vec![0.0f32; self.rank];
        for (i, &x) in input.iter().enumerate() {
            for r in 0..self.rank { hidden[r] += x * self.a[i][r]; }
        }
        let mut output = vec![0.0f32; self.out_features];
        for r in 0..self.rank {
            for j in 0..self.out_features { output[j] += hidden[r] * self.b[r][j]; }
        }
        for v in output.iter_mut() { *v *= self.scale; }
        output
    }

    /// Full delta weight matrix `scale * A * B`, shape (in_features, out_features).
    pub fn delta_weights(&self) -> Vec<Vec<f32>> {
        let mut delta = vec![vec![0.0f32; self.out_features]; self.in_features];
        for i in 0..self.in_features {
            for r in 0..self.rank {
                let a_val = self.a[i][r];
                for j in 0..self.out_features { delta[i][j] += a_val * self.b[r][j]; }
            }
        }
        for row in delta.iter_mut() { for v in row.iter_mut() { *v *= self.scale; } }
        delta
    }

    /// Add LoRA delta to base weights in place.
    pub fn merge_into(&self, base_weights: &mut [Vec<f32>]) {
        let delta = self.delta_weights();
        for (rb, rd) in base_weights.iter_mut().zip(delta.iter()) {
            for (w, &d) in rb.iter_mut().zip(rd.iter()) { *w += d; }
        }
    }

    /// Subtract LoRA delta from base weights in place.
    pub fn unmerge_from(&self, base_weights: &mut [Vec<f32>]) {
        let delta = self.delta_weights();
        for (rb, rd) in base_weights.iter_mut().zip(delta.iter()) {
            for (w, &d) in rb.iter_mut().zip(rd.iter()) { *w -= d; }
        }
    }

    /// Trainable parameter count: `rank * (in_features + out_features)`.
    pub fn n_params(&self) -> usize { self.rank * (self.in_features + self.out_features) }

    /// Reset A and B to zero.
    pub fn reset(&mut self) {
        for row in self.a.iter_mut() { for v in row.iter_mut() { *v = 0.0; } }
        for row in self.b.iter_mut() { for v in row.iter_mut() { *v = 0.0; } }
    }
}

// ── EWC++ Regularizer ───────────────────────────────────────────────────────

/// Elastic Weight Consolidation++ regularizer with running Fisher average.
#[derive(Debug, Clone)]
pub struct EwcRegularizer {
    pub lambda: f32,
    pub decay: f32,
    pub fisher_diag: Vec<f32>,
    pub reference_params: Vec<f32>,
}

impl EwcRegularizer {
    pub fn new(lambda: f32, decay: f32) -> Self {
        Self { lambda, decay, fisher_diag: Vec::new(), reference_params: Vec::new() }
    }

    /// Diagonal Fisher via numerical central differences: F_i = grad_i^2.
    pub fn compute_fisher(params: &[f32], loss_fn: impl Fn(&[f32]) -> f32, n_samples: usize) -> Vec<f32> {
        let eps = 1e-4f32;
        let n = params.len();
        let mut fisher = vec![0.0f32; n];
        let samples = n_samples.max(1);
        for _ in 0..samples {
            let mut p = params.to_vec();
            for i in 0..n {
                let orig = p[i];
                p[i] = orig + eps;
                let lp = loss_fn(&p);
                p[i] = orig - eps;
                let lm = loss_fn(&p);
                p[i] = orig;
                let g = (lp - lm) / (2.0 * eps);
                fisher[i] += g * g;
            }
        }
        for f in fisher.iter_mut() { *f /= samples as f32; }
        fisher
    }

    /// Online update: `F = decay * F_old + (1-decay) * F_new`.
    pub fn update_fisher(&mut self, new_fisher: &[f32]) {
        if self.fisher_diag.is_empty() {
            self.fisher_diag = new_fisher.to_vec();
            return;
        }
        assert_eq!(self.fisher_diag.len(), new_fisher.len());
        for (old, &nv) in self.fisher_diag.iter_mut().zip(new_fisher.iter()) {
            *old = self.decay * *old + (1.0 - self.decay) * nv;
        }
    }

    /// Penalty: `0.5 * lambda * sum(F_i * (theta_i - theta_i*)^2)`.
    pub fn penalty(&self, current_params: &[f32]) -> f32 {
        if self.reference_params.is_empty() || self.fisher_diag.is_empty() { return 0.0; }
        let n = current_params.len().min(self.reference_params.len()).min(self.fisher_diag.len());
        let mut sum = 0.0f32;
        for i in 0..n {
            let d = current_params[i] - self.reference_params[i];
            sum += self.fisher_diag[i] * d * d;
        }
        0.5 * self.lambda * sum
    }

    /// Gradient of penalty: `lambda * F_i * (theta_i - theta_i*)`.
    pub fn penalty_gradient(&self, current_params: &[f32]) -> Vec<f32> {
        if self.reference_params.is_empty() || self.fisher_diag.is_empty() {
            return vec![0.0f32; current_params.len()];
        }
        let n = current_params.len().min(self.reference_params.len()).min(self.fisher_diag.len());
        let mut grad = vec![0.0f32; current_params.len()];
        for i in 0..n {
            grad[i] = self.lambda * self.fisher_diag[i] * (current_params[i] - self.reference_params[i]);
        }
        grad
    }

    /// Save current params as the new reference point.
    pub fn consolidate(&mut self, params: &[f32]) { self.reference_params = params.to_vec(); }
}

// ── Configuration & Types ───────────────────────────────────────────────────

/// SONA adaptation configuration.
#[derive(Debug, Clone)]
pub struct SonaConfig {
    pub lora_rank: usize,
    pub lora_alpha: f32,
    pub ewc_lambda: f32,
    pub ewc_decay: f32,
    pub adaptation_lr: f32,
    pub max_steps: usize,
    pub convergence_threshold: f32,
    pub temporal_consistency_weight: f32,
}

impl Default for SonaConfig {
    fn default() -> Self {
        Self {
            lora_rank: 4, lora_alpha: 8.0, ewc_lambda: 5000.0, ewc_decay: 0.99,
            adaptation_lr: 0.001, max_steps: 50, convergence_threshold: 1e-4,
            temporal_consistency_weight: 0.1,
        }
    }
}

/// Single training sample for online adaptation.
#[derive(Debug, Clone)]
pub struct AdaptationSample {
    pub csi_features: Vec<f32>,
    pub target: Vec<f32>,
}

/// Result of a SONA adaptation run.
#[derive(Debug, Clone)]
pub struct AdaptationResult {
    pub adapted_params: Vec<f32>,
    pub steps_taken: usize,
    pub final_loss: f32,
    pub converged: bool,
    pub ewc_penalty: f32,
}

/// Saved environment-specific adaptation profile.
#[derive(Debug, Clone)]
pub struct SonaProfile {
    pub name: String,
    pub lora_a: Vec<Vec<f32>>,
    pub lora_b: Vec<Vec<f32>>,
    pub fisher_diag: Vec<f32>,
    pub reference_params: Vec<f32>,
    pub adaptation_count: usize,
}

// ── SONA Adapter ────────────────────────────────────────────────────────────

/// Full SONA system: LoRA adapter + EWC++ regularizer for online adaptation.
#[derive(Debug, Clone)]
pub struct SonaAdapter {
    pub config: SonaConfig,
    pub lora: LoraAdapter,
    pub ewc: EwcRegularizer,
    pub param_count: usize,
    pub adaptation_count: usize,
}

impl SonaAdapter {
    pub fn new(config: SonaConfig, param_count: usize) -> Self {
        let lora = LoraAdapter::new(param_count, 1, config.lora_rank, config.lora_alpha);
        let ewc = EwcRegularizer::new(config.ewc_lambda, config.ewc_decay);
        Self { config, lora, ewc, param_count, adaptation_count: 0 }
    }

    /// Run gradient descent with LoRA + EWC on the given samples.
    pub fn adapt(&mut self, base_params: &[f32], samples: &[AdaptationSample]) -> AdaptationResult {
        assert_eq!(base_params.len(), self.param_count);
        if samples.is_empty() {
            return AdaptationResult {
                adapted_params: base_params.to_vec(), steps_taken: 0,
                final_loss: 0.0, converged: true, ewc_penalty: self.ewc.penalty(base_params),
            };
        }
        let lr = self.config.adaptation_lr;
        let (mut prev_loss, mut steps, mut converged) = (f32::MAX, 0usize, false);
        let out_dim = samples[0].target.len();
        let in_dim = samples[0].csi_features.len();

        for step in 0..self.config.max_steps {
            steps = step + 1;
            let df = self.lora_delta_flat();
            let eff: Vec<f32> = base_params.iter().zip(df.iter()).map(|(&b, &d)| b + d).collect();
            let (dl, dg) = Self::mse_loss_grad(&eff, samples, in_dim, out_dim);
            let ep = self.ewc.penalty(&eff);
            let eg = self.ewc.penalty_gradient(&eff);
            let total = dl + ep;
            if (prev_loss - total).abs() < self.config.convergence_threshold {
                converged = true; prev_loss = total; break;
            }
            prev_loss = total;
            let gl = df.len().min(dg.len()).min(eg.len());
            let mut tg = vec![0.0f32; gl];
            for i in 0..gl { tg[i] = dg[i] + eg[i]; }
            self.update_lora(&tg, lr);
        }
        let df = self.lora_delta_flat();
        let adapted: Vec<f32> = base_params.iter().zip(df.iter()).map(|(&b, &d)| b + d).collect();
        let ewc_penalty = self.ewc.penalty(&adapted);
        self.adaptation_count += 1;
        AdaptationResult { adapted_params: adapted, steps_taken: steps, final_loss: prev_loss, converged, ewc_penalty }
    }

    pub fn save_profile(&self, name: &str) -> SonaProfile {
        SonaProfile {
            name: name.to_string(), lora_a: self.lora.a.clone(), lora_b: self.lora.b.clone(),
            fisher_diag: self.ewc.fisher_diag.clone(), reference_params: self.ewc.reference_params.clone(),
            adaptation_count: self.adaptation_count,
        }
    }

    pub fn load_profile(&mut self, profile: &SonaProfile) {
        self.lora.a = profile.lora_a.clone();
        self.lora.b = profile.lora_b.clone();
        self.ewc.fisher_diag = profile.fisher_diag.clone();
        self.ewc.reference_params = profile.reference_params.clone();
        self.adaptation_count = profile.adaptation_count;
    }

    fn lora_delta_flat(&self) -> Vec<f32> {
        self.lora.delta_weights().into_iter().map(|r| r[0]).collect()
    }

    fn mse_loss_grad(params: &[f32], samples: &[AdaptationSample], in_dim: usize, out_dim: usize) -> (f32, Vec<f32>) {
        let n = samples.len() as f32;
        let ws = in_dim * out_dim;
        let mut grad = vec![0.0f32; params.len()];
        let mut loss = 0.0f32;
        for s in samples {
            let (inp, tgt) = (&s.csi_features, &s.target);
            let mut pred = vec![0.0f32; out_dim];
            for j in 0..out_dim {
                for i in 0..in_dim.min(inp.len()) {
                    let idx = j * in_dim + i;
                    if idx < ws && idx < params.len() { pred[j] += params[idx] * inp[i]; }
                }
            }
            for j in 0..out_dim.min(tgt.len()) {
                let e = pred[j] - tgt[j];
                loss += e * e;
                for i in 0..in_dim.min(inp.len()) {
                    let idx = j * in_dim + i;
                    if idx < ws && idx < grad.len() { grad[idx] += 2.0 * e * inp[i] / n; }
                }
            }
        }
        (loss / n, grad)
    }

    fn update_lora(&mut self, grad: &[f32], lr: f32) {
        let (scale, rank) = (self.lora.scale, self.lora.rank);
        if self.lora.b.iter().all(|r| r.iter().all(|&v| v == 0.0)) && rank > 0 {
            self.lora.b[0][0] = 1.0;
        }
        for i in 0..self.lora.in_features.min(grad.len()) {
            for r in 0..rank {
                self.lora.a[i][r] -= lr * grad[i] * scale * self.lora.b[r][0];
            }
        }
        for r in 0..rank {
            let mut g = 0.0f32;
            for i in 0..self.lora.in_features.min(grad.len()) {
                g += grad[i] * scale * self.lora.a[i][r];
            }
            self.lora.b[r][0] -= lr * g;
        }
    }
}

// ── Environment Detector ────────────────────────────────────────────────────

/// CSI baseline drift information.
#[derive(Debug, Clone)]
pub struct DriftInfo {
    pub magnitude: f32,
    pub duration_frames: usize,
    pub baseline_mean: f32,
    pub current_mean: f32,
}

/// Detects environmental drift in CSI statistics (>3 sigma from baseline).
#[derive(Debug, Clone)]
pub struct EnvironmentDetector {
    window_size: usize,
    means: VecDeque<f32>,
    variances: VecDeque<f32>,
    baseline_mean: f32,
    baseline_var: f32,
    baseline_std: f32,
    baseline_set: bool,
    drift_frames: usize,
}

impl EnvironmentDetector {
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size: window_size.max(2),
            means: VecDeque::with_capacity(window_size),
            variances: VecDeque::with_capacity(window_size),
            baseline_mean: 0.0, baseline_var: 0.0, baseline_std: 0.0,
            baseline_set: false, drift_frames: 0,
        }
    }

    pub fn update(&mut self, csi_mean: f32, csi_var: f32) {
        self.means.push_back(csi_mean);
        self.variances.push_back(csi_var);
        while self.means.len() > self.window_size { self.means.pop_front(); }
        while self.variances.len() > self.window_size { self.variances.pop_front(); }
        if !self.baseline_set && self.means.len() >= self.window_size { self.reset_baseline(); }
        if self.drift_detected() { self.drift_frames += 1; } else { self.drift_frames = 0; }
    }

    pub fn drift_detected(&self) -> bool {
        if !self.baseline_set || self.means.is_empty() { return false; }
        let dev = (self.current_mean() - self.baseline_mean).abs();
        let thr = if self.baseline_std > f32::EPSILON { 3.0 * self.baseline_std }
                  else { f32::EPSILON * 100.0 };
        dev > thr
    }

    pub fn reset_baseline(&mut self) {
        if self.means.is_empty() { return; }
        let n = self.means.len() as f32;
        self.baseline_mean = self.means.iter().sum::<f32>() / n;
        let var = self.means.iter().map(|&m| (m - self.baseline_mean).powi(2)).sum::<f32>() / n;
        self.baseline_var = var;
        self.baseline_std = var.sqrt();
        self.baseline_set = true;
        self.drift_frames = 0;
    }

    pub fn drift_info(&self) -> DriftInfo {
        let cm = self.current_mean();
        let abs_dev = (cm - self.baseline_mean).abs();
        let magnitude = if self.baseline_std > f32::EPSILON { abs_dev / self.baseline_std }
                        else if abs_dev > f32::EPSILON { abs_dev / f32::EPSILON }
                        else { 0.0 };
        DriftInfo { magnitude, duration_frames: self.drift_frames, baseline_mean: self.baseline_mean, current_mean: cm }
    }

    fn current_mean(&self) -> f32 {
        if self.means.is_empty() { 0.0 }
        else { self.means.iter().sum::<f32>() / self.means.len() as f32 }
    }
}

// ── Temporal Consistency Loss ───────────────────────────────────────────────

/// Penalises large velocity between consecutive outputs: `sum((c-p)^2) / dt`.
pub struct TemporalConsistencyLoss;

impl TemporalConsistencyLoss {
    pub fn compute(prev_output: &[f32], curr_output: &[f32], dt: f32) -> f32 {
        if dt <= 0.0 { return 0.0; }
        let n = prev_output.len().min(curr_output.len());
        let mut sq = 0.0f32;
        for i in 0..n { let d = curr_output[i] - prev_output[i]; sq += d * d; }
        sq / dt
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lora_adapter_param_count() {
        let lora = LoraAdapter::new(64, 32, 4, 8.0);
        assert_eq!(lora.n_params(), 4 * (64 + 32));
    }

    #[test]
    fn lora_adapter_forward_shape() {
        let lora = LoraAdapter::new(8, 4, 2, 4.0);
        assert_eq!(lora.forward(&vec![1.0f32; 8]).len(), 4);
    }

    #[test]
    fn lora_adapter_zero_init_produces_zero_delta() {
        let delta = LoraAdapter::new(8, 4, 2, 4.0).delta_weights();
        assert_eq!(delta.len(), 8);
        for row in &delta { assert_eq!(row.len(), 4); for &v in row { assert_eq!(v, 0.0); } }
    }

    #[test]
    fn lora_adapter_merge_unmerge_roundtrip() {
        let mut lora = LoraAdapter::new(3, 2, 1, 2.0);
        lora.a[0][0] = 1.0; lora.a[1][0] = 2.0; lora.a[2][0] = 3.0;
        lora.b[0][0] = 0.5; lora.b[0][1] = -0.5;
        let mut base = vec![vec![10.0, 20.0], vec![30.0, 40.0], vec![50.0, 60.0]];
        let orig = base.clone();
        lora.merge_into(&mut base);
        assert_ne!(base, orig);
        lora.unmerge_from(&mut base);
        for (rb, ro) in base.iter().zip(orig.iter()) {
            for (&b, &o) in rb.iter().zip(ro.iter()) {
                assert!((b - o).abs() < 1e-5, "roundtrip failed: {b} vs {o}");
            }
        }
    }

    #[test]
    fn lora_adapter_rank_1_outer_product() {
        let mut lora = LoraAdapter::new(3, 2, 1, 1.0); // scale=1
        lora.a[0][0] = 1.0; lora.a[1][0] = 2.0; lora.a[2][0] = 3.0;
        lora.b[0][0] = 4.0; lora.b[0][1] = 5.0;
        let d = lora.delta_weights();
        let expected = [[4.0, 5.0], [8.0, 10.0], [12.0, 15.0]];
        for (i, row) in expected.iter().enumerate() {
            for (j, &v) in row.iter().enumerate() { assert!((d[i][j] - v).abs() < 1e-6); }
        }
    }

    #[test]
    fn lora_scale_factor() {
        assert!((LoraAdapter::new(8, 4, 4, 16.0).scale - 4.0).abs() < 1e-6);
        assert!((LoraAdapter::new(8, 4, 2, 8.0).scale - 4.0).abs() < 1e-6);
    }

    #[test]
    fn ewc_fisher_positive() {
        let fisher = EwcRegularizer::compute_fisher(
            &[1.0f32, -2.0, 0.5],
            |p: &[f32]| p.iter().map(|&x| x * x).sum::<f32>(), 1,
        );
        assert_eq!(fisher.len(), 3);
        for &f in &fisher { assert!(f >= 0.0, "Fisher must be >= 0, got {f}"); }
    }

    #[test]
    fn ewc_penalty_zero_at_reference() {
        let mut ewc = EwcRegularizer::new(5000.0, 0.99);
        let p = vec![1.0, 2.0, 3.0];
        ewc.fisher_diag = vec![1.0; 3]; ewc.consolidate(&p);
        assert!(ewc.penalty(&p).abs() < 1e-10);
    }

    #[test]
    fn ewc_penalty_positive_away_from_reference() {
        let mut ewc = EwcRegularizer::new(5000.0, 0.99);
        ewc.fisher_diag = vec![1.0; 3]; ewc.consolidate(&[1.0, 2.0, 3.0]);
        let pen = ewc.penalty(&[2.0, 3.0, 4.0]);
        assert!(pen > 0.0); // 0.5 * 5000 * 3 = 7500
        assert!((pen - 7500.0).abs() < 1e-3, "expected ~7500, got {pen}");
    }

    #[test]
    fn ewc_penalty_gradient_direction() {
        let mut ewc = EwcRegularizer::new(100.0, 0.99);
        let r = vec![1.0, 2.0, 3.0];
        ewc.fisher_diag = vec![1.0; 3]; ewc.consolidate(&r);
        let c = vec![2.0, 4.0, 5.0];
        let grad = ewc.penalty_gradient(&c);
        for (i, &g) in grad.iter().enumerate() {
            assert!(g * (c[i] - r[i]) > 0.0, "gradient[{i}] wrong sign");
        }
    }

    #[test]
    fn ewc_online_update_decays() {
        let mut ewc = EwcRegularizer::new(1.0, 0.5);
        ewc.update_fisher(&[10.0, 20.0]);
        assert!((ewc.fisher_diag[0] - 10.0).abs() < 1e-6);
        ewc.update_fisher(&[0.0, 0.0]);
        assert!((ewc.fisher_diag[0] - 5.0).abs() < 1e-6);  // 0.5*10 + 0.5*0
        assert!((ewc.fisher_diag[1] - 10.0).abs() < 1e-6); // 0.5*20 + 0.5*0
    }

    #[test]
    fn ewc_consolidate_updates_reference() {
        let mut ewc = EwcRegularizer::new(1.0, 0.99);
        ewc.consolidate(&[1.0, 2.0]);
        assert_eq!(ewc.reference_params, vec![1.0, 2.0]);
        ewc.consolidate(&[3.0, 4.0]);
        assert_eq!(ewc.reference_params, vec![3.0, 4.0]);
    }

    #[test]
    fn sona_config_defaults() {
        let c = SonaConfig::default();
        assert_eq!(c.lora_rank, 4);
        assert!((c.lora_alpha - 8.0).abs() < 1e-6);
        assert!((c.ewc_lambda - 5000.0).abs() < 1e-3);
        assert!((c.ewc_decay - 0.99).abs() < 1e-6);
        assert!((c.adaptation_lr - 0.001).abs() < 1e-6);
        assert_eq!(c.max_steps, 50);
        assert!((c.convergence_threshold - 1e-4).abs() < 1e-8);
        assert!((c.temporal_consistency_weight - 0.1).abs() < 1e-6);
    }

    #[test]
    fn sona_adapter_converges_on_simple_task() {
        let cfg = SonaConfig {
            lora_rank: 1, lora_alpha: 1.0, ewc_lambda: 0.0, ewc_decay: 0.99,
            adaptation_lr: 0.01, max_steps: 200, convergence_threshold: 1e-6,
            temporal_consistency_weight: 0.0,
        };
        let mut adapter = SonaAdapter::new(cfg, 1);
        let samples: Vec<_> = (1..=5).map(|i| {
            let x = i as f32;
            AdaptationSample { csi_features: vec![x], target: vec![2.0 * x] }
        }).collect();
        let r = adapter.adapt(&[0.0f32], &samples);
        assert!(r.final_loss < 1.0, "loss should decrease, got {}", r.final_loss);
        assert!(r.steps_taken > 0);
    }

    #[test]
    fn sona_adapter_respects_max_steps() {
        let cfg = SonaConfig { max_steps: 5, convergence_threshold: 0.0, ..SonaConfig::default() };
        let mut a = SonaAdapter::new(cfg, 4);
        let s = vec![AdaptationSample { csi_features: vec![1.0, 0.0, 0.0, 0.0], target: vec![1.0] }];
        assert_eq!(a.adapt(&[0.0; 4], &s).steps_taken, 5);
    }

    #[test]
    fn sona_profile_save_load_roundtrip() {
        let mut a = SonaAdapter::new(SonaConfig::default(), 8);
        a.lora.a[0][0] = 1.5; a.lora.b[0][0] = -0.3;
        a.ewc.fisher_diag = vec![1.0, 2.0, 3.0];
        a.ewc.reference_params = vec![0.1, 0.2, 0.3];
        a.adaptation_count = 42;
        let p = a.save_profile("test-env");
        assert_eq!(p.name, "test-env");
        assert_eq!(p.adaptation_count, 42);
        let mut a2 = SonaAdapter::new(SonaConfig::default(), 8);
        a2.load_profile(&p);
        assert!((a2.lora.a[0][0] - 1.5).abs() < 1e-6);
        assert!((a2.lora.b[0][0] - (-0.3)).abs() < 1e-6);
        assert_eq!(a2.ewc.fisher_diag.len(), 3);
        assert!((a2.ewc.fisher_diag[2] - 3.0).abs() < 1e-6);
        assert_eq!(a2.adaptation_count, 42);
    }

    #[test]
    fn environment_detector_no_drift_initially() {
        assert!(!EnvironmentDetector::new(10).drift_detected());
    }

    #[test]
    fn environment_detector_detects_large_shift() {
        let mut d = EnvironmentDetector::new(10);
        for _ in 0..10 { d.update(10.0, 0.1); }
        assert!(!d.drift_detected());
        for _ in 0..10 { d.update(50.0, 0.1); }
        assert!(d.drift_detected());
        assert!(d.drift_info().magnitude > 3.0, "magnitude = {}", d.drift_info().magnitude);
    }

    #[test]
    fn environment_detector_reset_baseline() {
        let mut d = EnvironmentDetector::new(10);
        for _ in 0..10 { d.update(10.0, 0.1); }
        for _ in 0..10 { d.update(50.0, 0.1); }
        assert!(d.drift_detected());
        d.reset_baseline();
        assert!(!d.drift_detected());
    }

    #[test]
    fn temporal_consistency_zero_for_static() {
        let o = vec![1.0, 2.0, 3.0];
        assert!(TemporalConsistencyLoss::compute(&o, &o, 0.033).abs() < 1e-10);
    }
}
