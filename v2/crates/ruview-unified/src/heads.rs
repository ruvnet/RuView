//! Task adapters ("heads") on the frozen encoder representation `z`
//! (ADR-274 §4).
//!
//! The ADR-273 acceptance test allows adapters **smaller than 1 % of the
//! backbone parameters** — enforced here by [`within_adapter_budget`] and by
//! a unit test over every head at the deployment config. Multi-class heads
//! use a low-rank (LoRA-style) factorization to stay inside the budget.

use crate::math::{sigmoid, softmax};

/// True iff the adapter fits the ADR-273 budget: `params(adapter) <
/// 1 % · params(backbone)`.
#[must_use]
pub fn within_adapter_budget(backbone_params: usize, adapter_params: usize) -> bool {
    adapter_params * 100 < backbone_params
}

/// Binary presence head: logistic regression on `z`.
#[derive(Debug, Clone)]
pub struct PresenceHead {
    /// Weights, length `d_model`.
    pub w: Vec<f64>,
    /// Bias.
    pub b: f64,
}

impl PresenceHead {
    /// Zero-initialized head (logistic regression is convex; zero init is
    /// the standard, deterministic choice).
    #[must_use]
    pub fn new(d_model: usize) -> Self {
        Self { w: vec![0.0; d_model], b: 0.0 }
    }

    /// Parameter count.
    #[must_use]
    pub fn param_count(&self) -> usize {
        self.w.len() + 1
    }

    /// P(present | z).
    #[must_use]
    pub fn predict_prob(&self, z: &[f64]) -> f64 {
        sigmoid(self.w.iter().zip(z).map(|(w, x)| w * x).sum::<f64>() + self.b)
    }

    /// Full-batch gradient-descent training (convex ⇒ deterministic
    /// convergence). Returns the final mean cross-entropy.
    pub fn train(&mut self, zs: &[Vec<f64>], labels: &[bool], lr: f64, epochs: usize) -> f64 {
        assert_eq!(zs.len(), labels.len());
        let n = zs.len() as f64;
        let mut final_ce = f64::INFINITY;
        for _ in 0..epochs {
            let mut gw = vec![0.0; self.w.len()];
            let mut gb = 0.0;
            let mut ce = 0.0;
            for (z, &y) in zs.iter().zip(labels) {
                let p = self.predict_prob(z);
                let t = if y { 1.0 } else { 0.0 };
                ce -= t * p.max(1e-12).ln() + (1.0 - t) * (1.0 - p).max(1e-12).ln();
                let err = (p - t) / n;
                for (g, x) in gw.iter_mut().zip(z) {
                    *g += err * x;
                }
                gb += err;
            }
            for (w, g) in self.w.iter_mut().zip(&gw) {
                *w -= lr * g;
            }
            self.b -= lr * gb;
            final_ce = ce / n;
        }
        final_ce
    }
}

/// Low-rank multi-class head: `logits = U·(V·z) + b` with rank `r`
/// (LoRA-style factorization keeps `C`-class heads inside the 1 % budget).
#[derive(Debug, Clone)]
pub struct ActivityHead {
    /// Classes.
    pub classes: usize,
    /// Rank.
    pub rank: usize,
    /// `classes × rank`, row-major.
    pub u: Vec<f64>,
    /// `rank × d_model`, row-major.
    pub v: Vec<f64>,
    /// Per-class bias.
    pub b: Vec<f64>,
    d_model: usize,
}

impl ActivityHead {
    /// Deterministic small-value init (breaks the U/V symmetry without RNG).
    #[must_use]
    pub fn new(d_model: usize, classes: usize, rank: usize) -> Self {
        let u = (0..classes * rank)
            .map(|i| 0.01 * ((i % 7) as f64 - 3.0))
            .collect();
        let v = (0..rank * d_model)
            .map(|i| 0.01 * ((i % 5) as f64 - 2.0))
            .collect();
        Self { classes, rank, u, v, b: vec![0.0; classes], d_model }
    }

    /// Parameter count.
    #[must_use]
    pub fn param_count(&self) -> usize {
        self.u.len() + self.v.len() + self.b.len()
    }

    /// Class probabilities.
    #[must_use]
    pub fn predict(&self, z: &[f64]) -> Vec<f64> {
        let mut probs = self.logits(z);
        softmax(&mut probs);
        probs
    }

    fn logits(&self, z: &[f64]) -> Vec<f64> {
        let mut vz = vec![0.0; self.rank];
        for r in 0..self.rank {
            let row = &self.v[r * self.d_model..(r + 1) * self.d_model];
            vz[r] = row.iter().zip(z).map(|(a, b)| a * b).sum();
        }
        let mut logits = self.b.clone();
        for c in 0..self.classes {
            let row = &self.u[c * self.rank..(c + 1) * self.rank];
            logits[c] += row.iter().zip(&vz).map(|(a, b)| a * b).sum::<f64>();
        }
        logits
    }

    /// Full-batch softmax cross-entropy training. Returns final mean CE.
    pub fn train(&mut self, zs: &[Vec<f64>], labels: &[usize], lr: f64, epochs: usize) -> f64 {
        assert_eq!(zs.len(), labels.len());
        let n = zs.len() as f64;
        let mut final_ce = f64::INFINITY;
        for _ in 0..epochs {
            let mut gu = vec![0.0; self.u.len()];
            let mut gv = vec![0.0; self.v.len()];
            let mut gb = vec![0.0; self.b.len()];
            let mut ce = 0.0;
            for (z, &y) in zs.iter().zip(labels) {
                let mut vz = vec![0.0; self.rank];
                for r in 0..self.rank {
                    let row = &self.v[r * self.d_model..(r + 1) * self.d_model];
                    vz[r] = row.iter().zip(z).map(|(a, b)| a * b).sum();
                }
                let mut p = self.b.clone();
                for c in 0..self.classes {
                    let row = &self.u[c * self.rank..(c + 1) * self.rank];
                    p[c] += row.iter().zip(&vz).map(|(a, b)| a * b).sum::<f64>();
                }
                softmax(&mut p);
                ce -= p[y].max(1e-12).ln();
                // dlogits = p − one_hot(y), scaled by 1/n.
                let mut dvz = vec![0.0; self.rank];
                for c in 0..self.classes {
                    let dl = (p[c] - if c == y { 1.0 } else { 0.0 }) / n;
                    gb[c] += dl;
                    for r in 0..self.rank {
                        gu[c * self.rank + r] += dl * vz[r];
                        dvz[r] += dl * self.u[c * self.rank + r];
                    }
                }
                for r in 0..self.rank {
                    for (d, zv) in z.iter().enumerate() {
                        gv[r * self.d_model + d] += dvz[r] * zv;
                    }
                }
            }
            for (w, g) in self.u.iter_mut().zip(&gu) {
                *w -= lr * g;
            }
            for (w, g) in self.v.iter_mut().zip(&gv) {
                *w -= lr * g;
            }
            for (w, g) in self.b.iter_mut().zip(&gb) {
                *w -= lr * g;
            }
            final_ce = ce / n;
        }
        final_ce
    }
}

/// Linear localization head: `xyz = W·z + b` (metres).
#[derive(Debug, Clone)]
pub struct LocalizationHead {
    /// `3 × d_model` weights.
    pub w: Vec<f64>,
    /// Bias.
    pub b: [f64; 3],
    d_model: usize,
}

impl LocalizationHead {
    /// Zero-initialized (linear least squares; convex).
    #[must_use]
    pub fn new(d_model: usize) -> Self {
        Self { w: vec![0.0; 3 * d_model], b: [0.0; 3], d_model }
    }

    /// Parameter count.
    #[must_use]
    pub fn param_count(&self) -> usize {
        self.w.len() + 3
    }

    /// Predicted position.
    #[must_use]
    pub fn predict(&self, z: &[f64]) -> [f64; 3] {
        let mut out = self.b;
        for (axis, o) in out.iter_mut().enumerate() {
            let row = &self.w[axis * self.d_model..(axis + 1) * self.d_model];
            *o += row.iter().zip(z).map(|(a, b)| a * b).sum::<f64>();
        }
        out
    }

    /// Full-batch MSE training. Returns final mean squared error (m²).
    pub fn train(&mut self, zs: &[Vec<f64>], targets: &[[f64; 3]], lr: f64, epochs: usize) -> f64 {
        assert_eq!(zs.len(), targets.len());
        let n = zs.len() as f64;
        let mut final_mse = f64::INFINITY;
        for _ in 0..epochs {
            let mut gw = vec![0.0; self.w.len()];
            let mut gb = [0.0; 3];
            let mut mse = 0.0;
            for (z, t) in zs.iter().zip(targets) {
                let pred = self.predict(z);
                for axis in 0..3 {
                    let err = pred[axis] - t[axis];
                    mse += err * err;
                    let scale = 2.0 * err / (3.0 * n);
                    gb[axis] += scale;
                    for (d, zv) in z.iter().enumerate() {
                        gw[axis * self.d_model + d] += scale * zv;
                    }
                }
            }
            for (w, g) in self.w.iter_mut().zip(&gw) {
                *w -= lr * g;
            }
            for (axis, g) in gb.iter().enumerate() {
                self.b[axis] -= lr * g;
            }
            final_mse = mse / (3.0 * n);
        }
        final_mse
    }
}

/// Number of skeleton joints (COCO-17 convention, matching the ruvsense
/// pose tracker).
pub const NUM_JOINTS: usize = 17;

/// Generic low-rank linear regressor `y = U·(V·x) + b` — the shared
/// building block for structured heads that must stay inside the adapter
/// budget.
#[derive(Debug, Clone)]
pub struct LowRankLinear {
    /// Output dimension.
    pub out: usize,
    /// Rank.
    pub rank: usize,
    d_in: usize,
    /// `out × rank`, row-major.
    pub u: Vec<f64>,
    /// `rank × d_in`, row-major.
    pub v: Vec<f64>,
    /// Bias, length `out`.
    pub b: Vec<f64>,
}

impl LowRankLinear {
    /// Deterministic small-value init (breaks U/V symmetry without RNG).
    #[must_use]
    pub fn new(d_in: usize, out: usize, rank: usize) -> Self {
        let u = (0..out * rank).map(|i| 0.01 * ((i % 7) as f64 - 3.0)).collect();
        let v = (0..rank * d_in).map(|i| 0.01 * ((i % 5) as f64 - 2.0)).collect();
        Self { out, rank, d_in, u, v, b: vec![0.0; out] }
    }

    /// Parameter count.
    #[must_use]
    pub fn param_count(&self) -> usize {
        self.u.len() + self.v.len() + self.b.len()
    }

    /// Prediction.
    #[must_use]
    pub fn predict(&self, x: &[f64]) -> Vec<f64> {
        let mut vx = vec![0.0; self.rank];
        for r in 0..self.rank {
            let row = &self.v[r * self.d_in..(r + 1) * self.d_in];
            vx[r] = row.iter().zip(x).map(|(a, b)| a * b).sum();
        }
        let mut y = self.b.clone();
        for o in 0..self.out {
            let row = &self.u[o * self.rank..(o + 1) * self.rank];
            y[o] += row.iter().zip(&vx).map(|(a, b)| a * b).sum::<f64>();
        }
        y
    }

    /// Full-batch MSE training; returns the final mean squared error.
    pub fn train(&mut self, xs: &[Vec<f64>], ys: &[Vec<f64>], lr: f64, epochs: usize) -> f64 {
        assert_eq!(xs.len(), ys.len());
        let n = xs.len() as f64;
        let mut final_mse = f64::INFINITY;
        for _ in 0..epochs {
            let mut gu = vec![0.0; self.u.len()];
            let mut gv = vec![0.0; self.v.len()];
            let mut gb = vec![0.0; self.b.len()];
            let mut mse = 0.0;
            for (x, y) in xs.iter().zip(ys) {
                let mut vx = vec![0.0; self.rank];
                for r in 0..self.rank {
                    let row = &self.v[r * self.d_in..(r + 1) * self.d_in];
                    vx[r] = row.iter().zip(x).map(|(a, b)| a * b).sum();
                }
                let mut dvx = vec![0.0; self.rank];
                for o in 0..self.out {
                    let row = &self.u[o * self.rank..(o + 1) * self.rank];
                    let pred = self.b[o]
                        + row.iter().zip(&vx).map(|(a, b)| a * b).sum::<f64>();
                    let err = pred - y[o];
                    mse += err * err;
                    let scale = 2.0 * err / (self.out as f64 * n);
                    gb[o] += scale;
                    for r in 0..self.rank {
                        gu[o * self.rank + r] += scale * vx[r];
                        dvx[r] += scale * self.u[o * self.rank + r];
                    }
                }
                for r in 0..self.rank {
                    for (d, xv) in x.iter().enumerate() {
                        gv[r * self.d_in + d] += dvx[r] * xv;
                    }
                }
            }
            for (w, g) in self.u.iter_mut().zip(&gu) {
                *w -= lr * g;
            }
            for (w, g) in self.v.iter_mut().zip(&gv) {
                *w -= lr * g;
            }
            for (w, g) in self.b.iter_mut().zip(&gb) {
                *w -= lr * g;
            }
            final_mse = mse / (self.out as f64 * n);
        }
        final_mse
    }
}

/// Factorized pose estimate (ADR-281 §4, the RePos lesson): root-relative
/// skeleton and absolute root are separate quantities with separate
/// uncertainties.
#[derive(Debug, Clone)]
pub struct PoseOutput {
    /// Root-relative joint positions, metres.
    pub relative_joints_m: [[f64; 3]; NUM_JOINTS],
    /// Absolute root position, metres, building frame.
    pub root_position_m: [f64; 3],
    /// Per-joint 1σ uncertainty, metres (calibrated from training residuals).
    pub joint_uncertainty_m: [f64; NUM_JOINTS],
    /// Root 1σ uncertainty, metres.
    pub root_uncertainty_m: f64,
}

impl PoseOutput {
    /// Absolute joints: `root + relative` (the RePos composition).
    #[must_use]
    pub fn absolute_joints_m(&self) -> [[f64; 3]; NUM_JOINTS] {
        let mut out = self.relative_joints_m;
        for j in out.iter_mut() {
            for k in 0..3 {
                j[k] += self.root_position_m[k];
            }
        }
        out
    }
}

/// Factorized pose head: the **relative skeleton** branch reads the
/// environment-invariant content representation (so it cannot learn
/// room-specific position shortcuts), while the **root localization**
/// branch reads the geometry-conditioned representation (where sensor
/// pose is signal). This is the anti-leakage factorization measured in
/// `tests::factorized_pose_resists_room_shortcut_leakage`.
#[derive(Debug, Clone)]
pub struct FactorizedPoseHead {
    /// Relative-skeleton regressor on the content representation.
    pub relative: LowRankLinear,
    /// Root regressor on the geometry-conditioned representation.
    pub root: LowRankLinear,
    /// Calibrated per-joint residual σ, metres.
    pub joint_residual_std_m: [f64; NUM_JOINTS],
    /// Calibrated root residual σ, metres.
    pub root_residual_std_m: f64,
}

impl FactorizedPoseHead {
    /// New head for the given representation dims and rank.
    #[must_use]
    pub fn new(d_content: usize, d_full: usize, rank: usize) -> Self {
        Self {
            relative: LowRankLinear::new(d_content, NUM_JOINTS * 3, rank),
            root: LowRankLinear::new(d_full, 3, rank),
            joint_residual_std_m: [f64::INFINITY; NUM_JOINTS],
            root_residual_std_m: f64::INFINITY,
        }
    }

    /// Parameter count (both branches + calibration statistics).
    #[must_use]
    pub fn param_count(&self) -> usize {
        self.relative.param_count() + self.root.param_count() + NUM_JOINTS + 1
    }

    /// Trains both branches and calibrates residual uncertainties.
    /// Returns `(relative_mse, root_mse)` in m².
    pub fn train(
        &mut self,
        content_zs: &[Vec<f64>],
        full_zs: &[Vec<f64>],
        relative_targets: &[[[f64; 3]; NUM_JOINTS]],
        root_targets: &[[f64; 3]],
        lr: f64,
        epochs: usize,
    ) -> (f64, f64) {
        let rel_flat: Vec<Vec<f64>> = relative_targets
            .iter()
            .map(|j| j.iter().flatten().copied().collect())
            .collect();
        let root_flat: Vec<Vec<f64>> = root_targets.iter().map(|r| r.to_vec()).collect();
        let rel_mse = self.relative.train(content_zs, &rel_flat, lr, epochs);
        let root_mse = self.root.train(full_zs, &root_flat, lr, epochs);

        // Calibrate per-joint residual σ on the training set.
        let n = content_zs.len().max(1) as f64;
        let mut joint_sq = [0.0f64; NUM_JOINTS];
        let mut root_sq = 0.0;
        for i in 0..content_zs.len() {
            let rel = self.relative.predict(&content_zs[i]);
            for j in 0..NUM_JOINTS {
                let mut d2 = 0.0;
                for k in 0..3 {
                    d2 += (rel[j * 3 + k] - relative_targets[i][j][k]).powi(2);
                }
                joint_sq[j] += d2;
            }
            let root = self.root.predict(&full_zs[i]);
            root_sq += (0..3).map(|k| (root[k] - root_targets[i][k]).powi(2)).sum::<f64>();
        }
        for j in 0..NUM_JOINTS {
            self.joint_residual_std_m[j] = (joint_sq[j] / n).sqrt();
        }
        self.root_residual_std_m = (root_sq / n).sqrt();
        (rel_mse, root_mse)
    }

    /// Predicts a factorized pose with calibrated uncertainties.
    #[must_use]
    pub fn predict(&self, content_z: &[f64], full_z: &[f64]) -> PoseOutput {
        let rel = self.relative.predict(content_z);
        let root = self.root.predict(full_z);
        let mut relative_joints_m = [[0.0; 3]; NUM_JOINTS];
        for j in 0..NUM_JOINTS {
            for k in 0..3 {
                relative_joints_m[j][k] = rel[j * 3 + k];
            }
        }
        PoseOutput {
            relative_joints_m,
            root_position_m: [root[0], root[1], root[2]],
            joint_uncertainty_m: self.joint_residual_std_m,
            root_uncertainty_m: self.root_residual_std_m,
        }
    }
}

/// Anomaly head: z-scores the encoder's masked-reconstruction error against
/// a calibration distribution of *normal* windows. No learned weights —
/// two calibration statistics.
#[derive(Debug, Clone)]
pub struct AnomalyHead {
    /// Calibration mean of reconstruction error.
    pub mean: f64,
    /// Calibration standard deviation.
    pub std: f64,
}

impl AnomalyHead {
    /// Calibrates from reconstruction errors of known-normal windows.
    ///
    /// # Panics
    /// If fewer than 2 calibration samples are provided.
    #[must_use]
    pub fn calibrate(normal_errors: &[f64]) -> Self {
        assert!(normal_errors.len() >= 2, "need >= 2 calibration errors");
        let n = normal_errors.len() as f64;
        let mean = normal_errors.iter().sum::<f64>() / n;
        let var = normal_errors.iter().map(|e| (e - mean).powi(2)).sum::<f64>() / (n - 1.0);
        Self { mean, std: var.sqrt().max(1e-12) }
    }

    /// Parameter count (two statistics).
    #[must_use]
    pub fn param_count(&self) -> usize {
        2
    }

    /// Anomaly z-score for a window's reconstruction error.
    #[must_use]
    pub fn score(&self, recon_error: f64) -> f64 {
        (recon_error - self.mean) / self.std
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::{EncoderConfig, RfEncoder};

    #[test]
    fn every_head_fits_the_one_percent_budget_at_deployment_config() {
        let enc = RfEncoder::new(EncoderConfig::default(), 1);
        let backbone = enc.param_count();
        let d = EncoderConfig::default().d_model;

        let presence = PresenceHead::new(d).param_count();
        let activity = ActivityHead::new(d, 4, 2).param_count();
        let localization = LocalizationHead::new(d).param_count();
        let anomaly = AnomalyHead { mean: 0.0, std: 1.0 }.param_count();

        for (name, p) in [
            ("presence", presence),
            ("activity", activity),
            ("localization", localization),
            ("anomaly", anomaly),
        ] {
            assert!(
                within_adapter_budget(backbone, p),
                "{name} head has {p} params, budget is <{}",
                backbone / 100
            );
        }
        // The structured pose head is the largest adapter; its documented
        // budget is < 2 % of the backbone (ADR-281 §4).
        let pose = FactorizedPoseHead::new(enc.content_dim(), d, 2).param_count();
        assert!(pose * 100 < backbone * 2, "pose head {pose} params vs 2 % of {backbone}");
        println!(
            "backbone {backbone} params; heads: presence {presence}, activity {activity}, \
             localization {localization}, anomaly {anomaly} (budget < {}), pose {pose} (< 2 %)",
            backbone / 100
        );
    }

    /// The RePos leakage experiment, on synthetic latents: in the training
    /// rooms, room position correlates with body scale (small people in room
    /// A, tall in room B). A monolithic absolute-pose head exploits the room
    /// feature as a shortcut and collapses in an unseen room that breaks the
    /// correlation; the factorized head's skeleton branch is given a latent
    /// without the room column and generalizes.
    ///
    /// Scope: the content and full latents here are hand-built
    /// (`content = [scale, 1]`, `full = [scale, room_x/3, 1]`), so the
    /// skeleton branch's room-independence is true by construction. What this
    /// verifies is the head's algebra given such a factorization, not that the
    /// encoder produces one. `RfEncoder`'s side of the contract is covered by
    /// `encoder::tests::content_representation_excludes_the_geometry_term`.
    #[test]
    fn factorized_pose_resists_room_shortcut_leakage() {
        use crate::eval::mpjpe;

        // 17 fixed joint directions (deterministic).
        let dirs: Vec<[f64; 3]> = (0..NUM_JOINTS)
            .map(|j| {
                let a = j as f64 * 0.37;
                [a.cos() * 0.3, a.sin() * 0.3, 0.1 * ((j % 5) as f64 - 2.0)]
            })
            .collect();
        let skeleton = |scale: f64| {
            let mut joints = [[0.0; 3]; NUM_JOINTS];
            for (j, d) in dirs.iter().enumerate() {
                for k in 0..3 {
                    joints[j][k] = d[k] * (1.0 + 0.5 * scale);
                }
            }
            joints
        };
        // content z = [scale, 1]; full z = [scale, room_x/3, 1].
        let sample = |scale: f64, room_x: f64| {
            let content = vec![scale, 1.0];
            let full = vec![scale, room_x / 3.0, 1.0];
            let root = [room_x + 0.5, 2.0, 1.0];
            (content, full, skeleton(scale), root)
        };

        // Training: room A (x=0) only small scales, room B (x=3) only large —
        // the leakage trap.
        let mut content_zs = Vec::new();
        let mut full_zs = Vec::new();
        let mut rels = Vec::new();
        let mut roots = Vec::new();
        for i in 0..30 {
            let s = -1.0 + i as f64 / 30.0; // [-1, 0)
            let (c, f, r, ro) = sample(s, 0.0);
            content_zs.push(c);
            full_zs.push(f);
            rels.push(r);
            roots.push(ro);
            let s = i as f64 / 30.0; // [0, 1)
            let (c, f, r, ro) = sample(s, 3.0);
            content_zs.push(c);
            full_zs.push(f);
            rels.push(r);
            roots.push(ro);
        }

        let mut head = FactorizedPoseHead::new(2, 3, 2);
        let (rel_mse, root_mse) = head.train(&content_zs, &full_zs, &rels, &roots, 0.3, 3000);
        assert!(rel_mse < 1e-3, "relative branch must fit, mse {rel_mse}");
        assert!(root_mse < 1e-3, "root branch must fit, mse {root_mse}");

        // Monolithic baseline: absolute joints regressed from the full
        // (room-conditioned) representation.
        let abs_targets: Vec<Vec<f64>> = rels
            .iter()
            .zip(&roots)
            .map(|(rel, root)| {
                rel.iter().flat_map(|j| (0..3).map(move |k| j[k] + root[k])).collect()
            })
            .collect();
        let mut monolithic = LowRankLinear::new(3, NUM_JOINTS * 3, 2);
        monolithic.train(&full_zs, &abs_targets, 0.3, 3000);

        // Held-out room (x=6) with the correlation broken: both scales.
        let mut fact_err = 0.0;
        let mut mono_err = 0.0;
        let mut count = 0.0;
        for i in 0..20 {
            let s = -1.0 + i as f64 / 10.0; // [-1, 1)
            let (c, f, rel, root) = sample(s, 6.0);
            let truth: Vec<[f64; 3]> =
                rel.iter().zip(std::iter::repeat(root)).map(|(j, r)| {
                    [j[0] + r[0], j[1] + r[1], j[2] + r[2]]
                }).collect();

            let pose = head.predict(&c, &f);
            let fact_abs = pose.absolute_joints_m();
            fact_err += mpjpe(&fact_abs, &truth);

            let mono = monolithic.predict(&f);
            let mono_abs: Vec<[f64; 3]> = (0..NUM_JOINTS)
                .map(|j| [mono[j * 3], mono[j * 3 + 1], mono[j * 3 + 2]])
                .collect();
            mono_err += mpjpe(&mono_abs, &truth);
            count += 1.0;

            // ADR-273 acceptance item: every output carries uncertainty.
            assert!(pose.root_uncertainty_m.is_finite());
            assert!(pose.joint_uncertainty_m.iter().all(|u| u.is_finite() && *u >= 0.0));
        }
        fact_err /= count;
        mono_err /= count;
        println!(
            "held-out room MPJPE: factorized {fact_err:.4} m vs monolithic {mono_err:.4} m"
        );
        assert!(fact_err < 0.15, "factorized head must generalize, MPJPE {fact_err}");
        assert!(
            mono_err > 1.5 * fact_err,
            "monolithic head must show the shortcut collapse: {mono_err} vs {fact_err}"
        );
    }

    fn separable_data(n: usize, d: usize) -> (Vec<Vec<f64>>, Vec<bool>) {
        let mut zs = Vec::new();
        let mut ys = Vec::new();
        for i in 0..n {
            let y = i % 2 == 0;
            let offset = if y { 1.0 } else { -1.0 };
            let z: Vec<f64> =
                (0..d).map(|k| offset * (0.5 + (k as f64 / d as f64)) + 0.1 * ((i * k) % 3) as f64).collect();
            zs.push(z);
            ys.push(y);
        }
        (zs, ys)
    }

    #[test]
    fn presence_head_learns_separable_data() {
        let (zs, ys) = separable_data(60, 16);
        let mut head = PresenceHead::new(16);
        let ce = head.train(&zs, &ys, 0.5, 200);
        assert!(ce < 0.1, "cross-entropy should collapse on separable data, got {ce}");
        let correct = zs
            .iter()
            .zip(&ys)
            .filter(|(z, &y)| (head.predict_prob(z) > 0.5) == y)
            .count();
        assert_eq!(correct, zs.len());
    }

    #[test]
    fn activity_head_learns_multiclass_toy() {
        let d = 16;
        let mut zs = Vec::new();
        let mut ys = Vec::new();
        for i in 0..90 {
            let c = i % 3;
            let z: Vec<f64> = (0..d)
                .map(|k| if k % 3 == c { 1.0 } else { 0.0 } + 0.05 * ((i + k) % 5) as f64)
                .collect();
            zs.push(z);
            ys.push(c);
        }
        let mut head = ActivityHead::new(d, 3, 2);
        let ce = head.train(&zs, &ys, 0.5, 400);
        assert!(ce < 0.3, "multiclass CE should drop, got {ce}");
        let acc = zs
            .iter()
            .zip(&ys)
            .filter(|(z, &y)| {
                let p = head.predict(z);
                p.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap().0 == y
            })
            .count() as f64
            / zs.len() as f64;
        assert!(acc > 0.95, "accuracy {acc}");
    }

    #[test]
    fn anomaly_head_zscores_against_calibration() {
        let normal: Vec<f64> = (0..50).map(|i| 0.10 + 0.001 * (i % 7) as f64).collect();
        let head = AnomalyHead::calibrate(&normal);
        assert!(head.score(0.103).abs() < 3.0, "in-distribution error must not alarm");
        assert!(head.score(0.5) > 10.0, "gross error must alarm loudly");
    }
}
