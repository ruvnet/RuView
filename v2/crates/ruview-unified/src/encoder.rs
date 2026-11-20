//! Universal RF foundation encoder (ADR-274 §3).
//!
//! A deliberately small, pure-Rust, exactly-differentiable network. The
//! representation contract is the one ADR-273 fixes:
//!
//! ```text
//! z = Encoder(tokens) ⊙ σ(AgeEncoder(age)) + GeometryEncoder(sensor_pose)
//! ```
//!
//! Architecture (all f64, weights row-major):
//!
//! ```text
//! h_i  = tanh(W1·x_i + b1)          token embedding (unmasked tokens)
//! c    = mean_i h_i                  permutation-invariant context pool
//! m    = tanh(W2·c + b2)             context mixing 1
//! g    = tanh(W2b·m + b2b)           context mixing 2
//! gate = σ(age_w·age + age_b)        multiplicative freshness gate
//! z    = g ⊙ gate + Wg·geo + bg      fused window representation
//! ```
//!
//! Pretraining (see [`crate::pretrain`]) reconstructs *masked* tokens from
//! `[z ; position_encoding(j)]` through a linear head `W3, b3` that is
//! discarded at deployment. The backward pass is hand-derived and verified
//! against central finite differences in `pretrain::tests` — the gradient
//! check is the crate's proof that this module computes what it claims.

use rand_chacha::ChaCha20Rng;

use crate::math::{sigmoid, xavier_init};

/// Age input transform for the freshness gate (ADR-281 §4, the age-aware
/// CSI recipe): `log(1 + sample_age_ms)` — log-scaling keeps millisecond
/// and multi-second staleness on comparable input scales.
#[must_use]
pub fn age_feature(age_s: f64) -> f64 {
    (1.0 + age_s * 1000.0).ln()
}
use crate::tokenizer::{position_encoding, RfToken, TokenizedWindow, D_IN, D_POS};

/// Dense row-major matrix with a bias vector (one linear layer).
#[derive(Debug, Clone)]
pub struct Linear {
    /// Output rows.
    pub rows: usize,
    /// Input columns.
    pub cols: usize,
    /// Row-major weights, `rows × cols`.
    pub w: Vec<f64>,
    /// Bias, length `rows`.
    pub b: Vec<f64>,
}

impl Linear {
    /// Xavier-initialized layer.
    #[must_use]
    pub fn new(rng: &mut ChaCha20Rng, rows: usize, cols: usize) -> Self {
        Self { rows, cols, w: xavier_init(rng, rows, cols), b: vec![0.0; rows] }
    }

    /// Zeroed layer with the same shape (gradient accumulator).
    #[must_use]
    pub fn zeros_like(&self) -> Self {
        Self { rows: self.rows, cols: self.cols, w: vec![0.0; self.w.len()], b: vec![0.0; self.rows] }
    }

    /// `y = W·x + b`.
    #[must_use]
    pub fn forward(&self, x: &[f64]) -> Vec<f64> {
        debug_assert_eq!(x.len(), self.cols);
        let mut y = self.b.clone();
        for r in 0..self.rows {
            let row = &self.w[r * self.cols..(r + 1) * self.cols];
            let mut acc = 0.0;
            for (wv, xv) in row.iter().zip(x) {
                acc += wv * xv;
            }
            y[r] += acc;
        }
        y
    }

    /// `x_grad = Wᵀ·dy` (input gradient).
    #[must_use]
    pub fn backward_input(&self, dy: &[f64]) -> Vec<f64> {
        let mut dx = vec![0.0; self.cols];
        for r in 0..self.rows {
            let row = &self.w[r * self.cols..(r + 1) * self.cols];
            for (c, wv) in row.iter().enumerate() {
                dx[c] += wv * dy[r];
            }
        }
        dx
    }

    /// Accumulates `dW += dy ⊗ x`, `db += dy` into `grad`.
    pub fn accumulate_grad(&self, grad: &mut Linear, dy: &[f64], x: &[f64]) {
        for r in 0..self.rows {
            grad.b[r] += dy[r];
            let row = &mut grad.w[r * self.cols..(r + 1) * self.cols];
            for (c, xv) in x.iter().enumerate() {
                row[c] += dy[r] * xv;
            }
        }
    }

    /// SGD step: `p -= lr·g`.
    pub fn sgd(&mut self, grad: &Linear, lr: f64) {
        for (p, g) in self.w.iter_mut().zip(&grad.w) {
            *p -= lr * g;
        }
        for (p, g) in self.b.iter_mut().zip(&grad.b) {
            *p -= lr * g;
        }
    }

    /// Number of parameters (weights + biases).
    #[must_use]
    pub fn param_count(&self) -> usize {
        self.w.len() + self.b.len()
    }
}

/// Encoder hyper-parameters. The default is the deployment config; tests use
/// tiny configs for the finite-difference gradient check.
#[derive(Debug, Clone, Copy)]
pub struct EncoderConfig {
    /// Token feature dimension.
    pub d_in: usize,
    /// Position-encoding dimension (pretraining decoder only).
    pub d_pos: usize,
    /// Model (representation) dimension.
    pub d_model: usize,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self { d_in: D_IN, d_pos: D_POS, d_model: 128 }
    }
}

/// Window-level context consumed by the fusion stage.
#[derive(Debug, Clone, Copy)]
pub struct WindowContext {
    /// Sample age in seconds.
    pub age_s: f64,
    /// Geometry summary (mean TX xyz, mean RX xyz, decametres).
    pub geometry: [f64; 6],
}

impl From<&TokenizedWindow> for WindowContext {
    fn from(w: &TokenizedWindow) -> Self {
        Self { age_s: w.age_s, geometry: w.geometry }
    }
}

/// Intermediate activations kept for the backward pass.
#[derive(Debug, Clone)]
pub struct ForwardCache {
    /// Indices of unmasked tokens (context providers).
    pub unmasked: Vec<usize>,
    /// Embeddings `h_i` for unmasked tokens (parallel to `unmasked`).
    pub h: Vec<Vec<f64>>,
    /// Pooled context `c`.
    pub c: Vec<f64>,
    /// Mixing activations `m`, `g`.
    pub m: Vec<f64>,
    /// Second mixing output.
    pub g: Vec<f64>,
    /// Freshness gate `σ(age_w·age + age_b)`.
    pub gate: Vec<f64>,
    /// Fused representation `z`.
    pub z: Vec<f64>,
    /// Window context used.
    pub ctx: WindowContext,
}

/// The universal RF foundation encoder.
#[derive(Debug, Clone)]
pub struct RfEncoder {
    /// Config.
    pub cfg: EncoderConfig,
    /// Token embedding.
    pub w1: Linear,
    /// Context mixing 1.
    pub w2: Linear,
    /// Context mixing 2.
    pub w2b: Linear,
    /// Age gate weight (elementwise on the scalar age).
    pub age_w: Vec<f64>,
    /// Age gate bias.
    pub age_b: Vec<f64>,
    /// Geometry encoder.
    pub wg: Linear,
    /// Masked-token reconstruction head (pretraining only; not counted as a
    /// deployment adapter).
    pub w3: Linear,
}

impl RfEncoder {
    /// Deterministically initialized encoder.
    #[must_use]
    pub fn new(cfg: EncoderConfig, seed: u64) -> Self {
        let mut rng = crate::math::seeded_rng(seed);
        let h = cfg.d_model;
        Self {
            cfg,
            w1: Linear::new(&mut rng, h, cfg.d_in),
            w2: Linear::new(&mut rng, h, h),
            w2b: Linear::new(&mut rng, h, h),
            age_w: xavier_init(&mut rng, h, 1),
            age_b: vec![0.0; h],
            wg: Linear::new(&mut rng, h, 6),
            w3: Linear::new(&mut rng, cfg.d_in, h + cfg.d_pos),
        }
    }

    /// Total trainable parameters (the "backbone" for the ≤1 % adapter
    /// budget of ADR-273's acceptance test).
    #[must_use]
    pub fn param_count(&self) -> usize {
        self.w1.param_count()
            + self.w2.param_count()
            + self.w2b.param_count()
            + self.age_w.len()
            + self.age_b.len()
            + self.wg.param_count()
            + self.w3.param_count()
    }

    /// Forward pass over the unmasked token set, producing the fused window
    /// representation `z` and the cache needed for backprop.
    ///
    /// `masked` lists token indices excluded from the context pool (empty at
    /// inference time).
    #[must_use]
    pub fn forward(&self, tokens: &[RfToken], masked: &[usize], ctx: WindowContext) -> ForwardCache {
        let h_dim = self.cfg.d_model;
        let unmasked: Vec<usize> =
            (0..tokens.len()).filter(|i| !masked.contains(i)).collect();
        assert!(!unmasked.is_empty(), "cannot encode a fully masked window");

        let mut h = Vec::with_capacity(unmasked.len());
        let mut c = vec![0.0; h_dim];
        for &i in &unmasked {
            let mut hi = self.w1.forward(&tokens[i].features);
            for v in &mut hi {
                *v = v.tanh();
            }
            for (cv, hv) in c.iter_mut().zip(&hi) {
                *cv += hv;
            }
            h.push(hi);
        }
        for cv in &mut c {
            *cv /= unmasked.len() as f64;
        }

        let mut m = self.w2.forward(&c);
        for v in &mut m {
            *v = v.tanh();
        }
        let mut g = self.w2b.forward(&m);
        for v in &mut g {
            *v = v.tanh();
        }

        let age_feat = age_feature(ctx.age_s);
        let gate: Vec<f64> = self
            .age_w
            .iter()
            .zip(&self.age_b)
            .map(|(w, b)| sigmoid(w * age_feat + b))
            .collect();

        let geo = self.wg.forward(&ctx.geometry);
        let z: Vec<f64> =
            (0..h_dim).map(|k| g[k] * gate[k] + geo[k]).collect();

        ForwardCache { unmasked, h, c, m, g, gate, z, ctx }
    }

    /// Inference entry point: encode a full window (nothing masked) into the
    /// fused representation `z = g ⊙ gate + Wg·geo + bg`.
    ///
    /// Use `z` for geometry-conditioned tasks (localization, channel
    /// prediction) where sensor pose is signal, not nuisance.
    #[must_use]
    pub fn encode(&self, window: &TokenizedWindow) -> Vec<f64> {
        self.forward(&window.tokens, &[], WindowContext::from(window)).z
    }

    /// Environment-invariant content representation
    /// `[g ⊙ gate ; mean_i(x_i)]` — the fused `z` *without* the additive
    /// geometry term, concatenated with the window-mean token features
    /// (dimension `d_model + d_in`).
    ///
    /// Two deliberate choices, both anti-leakage (ADR-273 §5):
    /// - The PerceptAlign lesson: geometry should *condition* spatial tasks,
    ///   but for environment-invariant heads (presence, activity, anomaly)
    ///   the additive `Wg·geo` term is a room-specific offset a linear
    ///   adapter would memorize.
    /// - The skip connection exposes pooled token statistics (Doppler
    ///   energy, temporal variance, freshness) whose *semantics are
    ///   identical in every room*, so a small head can generalize across
    ///   environments instead of re-deriving them through mixing layers that
    ///   entangle them with room-specific fading structure.
    #[must_use]
    pub fn encode_content(&self, window: &TokenizedWindow) -> Vec<f64> {
        let cache = self.forward(&window.tokens, &[], WindowContext::from(window));
        let mut out: Vec<f64> =
            (0..self.cfg.d_model).map(|k| cache.g[k] * cache.gate[k]).collect();
        let n = window.tokens.len().max(1) as f64;
        let mut mean = vec![0.0; self.cfg.d_in];
        for t in &window.tokens {
            for (m, v) in mean.iter_mut().zip(&t.features) {
                *m += v / n;
            }
        }
        out.extend_from_slice(&mean);
        out
    }

    /// Dimension of the [`Self::encode_content`] representation.
    #[must_use]
    pub fn content_dim(&self) -> usize {
        self.cfg.d_model + self.cfg.d_in
    }

    /// Reconstruction of masked token `j` from the cache: `W3·[z ; pos(j)]`.
    #[must_use]
    pub fn reconstruct(&self, cache: &ForwardCache, token_idx: usize) -> Vec<f64> {
        let mut u = cache.z.clone();
        u.extend_from_slice(&position_encoding(token_idx)[..self.cfg.d_pos]);
        self.w3.forward(&u)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toy_tokens(n: usize) -> Vec<RfToken> {
        (0..n)
            .map(|i| {
                let mut f = [0.0f64; D_IN];
                for (k, v) in f.iter_mut().enumerate() {
                    *v = ((i * 31 + k * 7) % 13) as f64 / 13.0 - 0.5;
                }
                RfToken { features: f, link: 0, group: i }
            })
            .collect()
    }

    #[test]
    fn encoder_is_deterministic_given_seed() {
        let a = RfEncoder::new(EncoderConfig::default(), 42);
        let b = RfEncoder::new(EncoderConfig::default(), 42);
        assert_eq!(a.w1.w, b.w1.w);
        let tokens = toy_tokens(6);
        let ctx = WindowContext { age_s: 0.1, geometry: [0.1; 6] };
        assert_eq!(a.forward(&tokens, &[], ctx).z, b.forward(&tokens, &[], ctx).z);
    }

    #[test]
    fn param_count_matches_hand_computation() {
        let e = RfEncoder::new(EncoderConfig::default(), 1);
        let h = 128;
        let expected = (h * D_IN + h)      // w1
            + (h * h + h)                  // w2
            + (h * h + h)                  // w2b
            + h + h                        // age_w, age_b
            + (h * 6 + h)                  // wg
            + (D_IN * (h + D_POS) + D_IN); // w3
        assert_eq!(e.param_count(), expected);
    }

    #[test]
    fn stale_windows_are_gated_toward_geometry_prior() {
        // As age → ∞ with negative gate logits, σ → 0 or 1 per unit; what we
        // verify is the *contract*: z depends on age only through the gate,
        // so two ages produce different z while geometry contribution stays.
        let e = RfEncoder::new(EncoderConfig::default(), 3);
        let tokens = toy_tokens(8);
        let fresh = e.forward(&tokens, &[], WindowContext { age_s: 0.0, geometry: [0.2; 6] });
        let stale = e.forward(&tokens, &[], WindowContext { age_s: 9.0, geometry: [0.2; 6] });
        assert_ne!(fresh.z, stale.z);
        // Same age, different geometry ⇒ additive path shifts z.
        let moved = e.forward(&tokens, &[], WindowContext { age_s: 0.0, geometry: [0.4; 6] });
        assert_ne!(fresh.z, moved.z);
    }

    #[test]
    fn content_representation_excludes_the_geometry_term() {
        // z = g ⊙ gate + Wg·geo + bg; encode_content returns g ⊙ gate with the
        // window-mean token features and no additive room offset. This is the
        // complement of stale_windows_are_gated_toward_geometry_prior, which
        // pins that z *does* move with geometry: together they are the
        // factorization the pose head relies on. Exercised against the real
        // encoder, so wiring geometry into the content path fails here.
        let e = RfEncoder::new(EncoderConfig::default(), 7);
        let tokens = toy_tokens(8);
        let room_a =
            TokenizedWindow { tokens: tokens.clone(), age_s: 0.25, geometry: [0.2; 6] };
        let room_b = TokenizedWindow { tokens, age_s: 0.25, geometry: [0.9; 6] };

        // The fused representation separates the two rooms.
        assert_ne!(
            e.forward(&room_a.tokens, &[], WindowContext::from(&room_a)).z,
            e.forward(&room_b.tokens, &[], WindowContext::from(&room_b)).z
        );
        // The content representation cannot tell them apart.
        assert_eq!(e.encode_content(&room_a), e.encode_content(&room_b));
        assert_eq!(e.encode_content(&room_a).len(), e.content_dim());
    }

    #[test]
    fn masking_excludes_tokens_from_context() {
        let e = RfEncoder::new(EncoderConfig::default(), 5);
        let tokens = toy_tokens(8);
        let ctx = WindowContext { age_s: 0.1, geometry: [0.0; 6] };
        let full = e.forward(&tokens, &[], ctx);
        let masked = e.forward(&tokens, &[2, 5], ctx);
        assert_eq!(masked.unmasked.len(), 6);
        assert_ne!(full.z, masked.z);
    }
}
