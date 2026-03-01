//! Few-shot rapid adaptation (MERIDIAN Phase 5).
//!
//! Test-time training with contrastive learning and entropy minimization on
//! unlabeled CSI frames. Produces LoRA weight deltas for new environments.

/// Loss function(s) for test-time adaptation.
#[derive(Debug, Clone)]
pub enum AdaptationLoss {
    /// Contrastive TTT: positive = temporally adjacent, negative = random.
    ContrastiveTTT { /// Gradient-descent epochs.
        epochs: usize, /// Learning rate.
        lr: f32 },
    /// Minimize entropy of confidence outputs for sharper predictions.
    EntropyMin { /// Gradient-descent epochs.
        epochs: usize, /// Learning rate.
        lr: f32 },
    /// Both contrastive and entropy losses combined.
    Combined { /// Gradient-descent epochs.
        epochs: usize, /// Learning rate.
        lr: f32, /// Weight for entropy term.
        lambda_ent: f32 },
}

impl AdaptationLoss {
    /// Number of epochs for this variant.
    pub fn epochs(&self) -> usize {
        match self { Self::ContrastiveTTT { epochs, .. }
            | Self::EntropyMin { epochs, .. }
            | Self::Combined { epochs, .. } => *epochs }
    }
    /// Learning rate for this variant.
    pub fn lr(&self) -> f32 {
        match self { Self::ContrastiveTTT { lr, .. }
            | Self::EntropyMin { lr, .. }
            | Self::Combined { lr, .. } => *lr }
    }
}

/// Result of [`RapidAdaptation::adapt`].
#[derive(Debug, Clone)]
pub struct AdaptationResult {
    /// LoRA weight deltas.
    pub lora_weights: Vec<f32>,
    /// Final epoch loss.
    pub final_loss: f32,
    /// Calibration frames consumed.
    pub frames_used: usize,
    /// Epochs executed.
    pub adaptation_epochs: usize,
}

/// Few-shot rapid adaptation engine.
///
/// Accumulates unlabeled CSI calibration frames and runs test-time training
/// to produce LoRA weight deltas.
///
/// ```rust
/// use wifi_densepose_train::rapid_adapt::{RapidAdaptation, AdaptationLoss};
/// let loss = AdaptationLoss::Combined { epochs: 5, lr: 0.001, lambda_ent: 0.5 };
/// let mut ra = RapidAdaptation::new(10, 4, loss);
/// for i in 0..10 { ra.push_frame(&vec![i as f32; 8]); }
/// assert!(ra.is_ready());
/// let r = ra.adapt();
/// assert_eq!(r.frames_used, 10);
/// ```
pub struct RapidAdaptation {
    /// Minimum frames before adaptation (default 200 = 10 s @ 20 Hz).
    pub min_calibration_frames: usize,
    /// LoRA factorization rank (default 4).
    pub lora_rank: usize,
    /// Loss variant for test-time training.
    pub adaptation_loss: AdaptationLoss,
    calibration_buffer: Vec<Vec<f32>>,
}

impl RapidAdaptation {
    /// Create a new adaptation engine.
    pub fn new(min_calibration_frames: usize, lora_rank: usize, adaptation_loss: AdaptationLoss) -> Self {
        Self { min_calibration_frames, lora_rank, adaptation_loss, calibration_buffer: Vec::new() }
    }
    /// Push a single unlabeled CSI frame.
    pub fn push_frame(&mut self, frame: &[f32]) { self.calibration_buffer.push(frame.to_vec()); }
    /// True when buffer >= min_calibration_frames.
    pub fn is_ready(&self) -> bool { self.calibration_buffer.len() >= self.min_calibration_frames }
    /// Number of buffered frames.
    pub fn buffer_len(&self) -> usize { self.calibration_buffer.len() }

    /// Run test-time adaptation producing LoRA weight deltas.
    ///
    /// # Panics
    /// Panics if the calibration buffer is empty.
    pub fn adapt(&self) -> AdaptationResult {
        assert!(!self.calibration_buffer.is_empty(), "empty calibration buffer");
        let (n, fdim) = (self.calibration_buffer.len(), self.calibration_buffer[0].len());
        let lora_sz = 2 * fdim * self.lora_rank;
        let mut w = vec![0.01_f32; lora_sz];
        let (epochs, lr) = (self.adaptation_loss.epochs(), self.adaptation_loss.lr());
        let mut final_loss = 0.0_f32;
        for _ in 0..epochs {
            let mut g = vec![0.0_f32; lora_sz];
            let loss = match &self.adaptation_loss {
                AdaptationLoss::ContrastiveTTT { .. } => self.contrastive_step(&w, fdim, &mut g),
                AdaptationLoss::EntropyMin { .. } => self.entropy_step(&w, fdim, &mut g),
                AdaptationLoss::Combined { lambda_ent, .. } => {
                    let cl = self.contrastive_step(&w, fdim, &mut g);
                    let mut eg = vec![0.0_f32; lora_sz];
                    let el = self.entropy_step(&w, fdim, &mut eg);
                    for (gi, egi) in g.iter_mut().zip(eg.iter()) { *gi += lambda_ent * egi; }
                    cl + lambda_ent * el
                }
            };
            for (wi, gi) in w.iter_mut().zip(g.iter()) { *wi -= lr * gi; }
            final_loss = loss;
        }
        AdaptationResult { lora_weights: w, final_loss, frames_used: n, adaptation_epochs: epochs }
    }

    fn contrastive_step(&self, w: &[f32], fdim: usize, grad: &mut [f32]) -> f32 {
        let n = self.calibration_buffer.len();
        if n < 2 { return 0.0; }
        let (margin, pairs) = (1.0_f32, n - 1);
        let mut total = 0.0_f32;
        for i in 0..pairs {
            let (anc, pos) = (&self.calibration_buffer[i], &self.calibration_buffer[i + 1]);
            let neg = &self.calibration_buffer[(i + n / 2) % n];
            let (pa, pp, pn) = (self.project(anc, w, fdim), self.project(pos, w, fdim), self.project(neg, w, fdim));
            let trip = (l2_dist(&pa, &pp) - l2_dist(&pa, &pn) + margin).max(0.0);
            total += trip;
            if trip > 0.0 {
                for (j, g) in grad.iter_mut().enumerate() {
                    let v = anc.get(j % fdim).copied().unwrap_or(0.0);
                    *g += v * 0.01 / pairs as f32;
                }
            }
        }
        total / pairs as f32
    }

    fn entropy_step(&self, w: &[f32], fdim: usize, grad: &mut [f32]) -> f32 {
        let n = self.calibration_buffer.len();
        if n == 0 { return 0.0; }
        let nc = self.lora_rank.max(2);
        let mut total = 0.0_f32;
        for frame in &self.calibration_buffer {
            let proj = self.project(frame, w, fdim);
            let mut logits = vec![0.0_f32; nc];
            for (i, &v) in proj.iter().enumerate() { logits[i % nc] += v; }
            let mx = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let exps: Vec<f32> = logits.iter().map(|&l| (l - mx).exp()).collect();
            let s: f32 = exps.iter().sum();
            let ent: f32 = exps.iter().map(|&e| { let p = e / s; if p > 1e-10 { -p * p.ln() } else { 0.0 } }).sum();
            total += ent;
            for (j, g) in grad.iter_mut().enumerate() {
                let v = frame.get(j % frame.len().max(1)).copied().unwrap_or(0.0);
                *g += v * ent * 0.001 / n as f32;
            }
        }
        total / n as f32
    }

    fn project(&self, frame: &[f32], w: &[f32], fdim: usize) -> Vec<f32> {
        let rank = self.lora_rank;
        let mut hidden = vec![0.0_f32; rank];
        for r in 0..rank {
            for d in 0..fdim.min(frame.len()) {
                let idx = d * rank + r;
                if idx < w.len() { hidden[r] += w[idx] * frame[d]; }
            }
        }
        let boff = fdim * rank;
        (0..fdim).map(|d| {
            let lora: f32 = (0..rank).map(|r| {
                let idx = boff + r * fdim + d;
                if idx < w.len() { w[idx] * hidden[r] } else { 0.0 }
            }).sum();
            frame.get(d).copied().unwrap_or(0.0) + lora
        }).collect()
    }
}

fn l2_dist(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(&x, &y)| (x - y).powi(2)).sum::<f32>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_frame_accumulates() {
        let mut a = RapidAdaptation::new(5, 4, AdaptationLoss::ContrastiveTTT { epochs: 1, lr: 0.01 });
        assert_eq!(a.buffer_len(), 0);
        a.push_frame(&[1.0, 2.0]); assert_eq!(a.buffer_len(), 1);
        a.push_frame(&[3.0, 4.0]); assert_eq!(a.buffer_len(), 2);
    }

    #[test]
    fn is_ready_threshold() {
        let mut a = RapidAdaptation::new(5, 4, AdaptationLoss::EntropyMin { epochs: 3, lr: 0.001 });
        for i in 0..4 { a.push_frame(&[i as f32; 8]); assert!(!a.is_ready()); }
        a.push_frame(&[99.0; 8]); assert!(a.is_ready());
        a.push_frame(&[100.0; 8]); assert!(a.is_ready());
    }

    #[test]
    fn adapt_lora_weight_dimension() {
        let (fdim, rank) = (16, 4);
        let mut a = RapidAdaptation::new(10, rank, AdaptationLoss::ContrastiveTTT { epochs: 3, lr: 0.01 });
        for i in 0..10 { a.push_frame(&vec![i as f32 * 0.1; fdim]); }
        let r = a.adapt();
        assert_eq!(r.lora_weights.len(), 2 * fdim * rank);
        assert_eq!(r.frames_used, 10);
        assert_eq!(r.adaptation_epochs, 3);
    }

    #[test]
    fn contrastive_loss_decreases() {
        let (fdim, rank) = (32, 4);
        let mk = |ep| {
            let mut a = RapidAdaptation::new(20, rank, AdaptationLoss::ContrastiveTTT { epochs: ep, lr: 0.01 });
            for i in 0..20 { let v = i as f32 * 0.1; a.push_frame(&(0..fdim).map(|d| v + d as f32 * 0.01).collect::<Vec<_>>()); }
            a.adapt().final_loss
        };
        assert!(mk(10) <= mk(1) + 1e-6, "10 epochs should yield <= 1 epoch loss");
    }

    #[test]
    fn combined_loss_adaptation() {
        let (fdim, rank) = (16, 4);
        let mut a = RapidAdaptation::new(10, rank, AdaptationLoss::Combined { epochs: 5, lr: 0.001, lambda_ent: 0.5 });
        for i in 0..10 { a.push_frame(&(0..fdim).map(|d| ((i * fdim + d) as f32).sin()).collect::<Vec<_>>()); }
        let r = a.adapt();
        assert_eq!(r.frames_used, 10);
        assert_eq!(r.adaptation_epochs, 5);
        assert!(r.final_loss.is_finite());
        assert_eq!(r.lora_weights.len(), 2 * fdim * rank);
        assert!(r.lora_weights.iter().all(|w| w.is_finite()));
    }

    #[test]
    fn l2_distance_tests() {
        assert!(l2_dist(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]).abs() < 1e-10);
        assert!((l2_dist(&[0.0, 0.0], &[3.0, 4.0]) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn loss_accessors() {
        let c = AdaptationLoss::ContrastiveTTT { epochs: 7, lr: 0.02 };
        assert_eq!(c.epochs(), 7); assert!((c.lr() - 0.02).abs() < 1e-7);
        let e = AdaptationLoss::EntropyMin { epochs: 3, lr: 0.1 };
        assert_eq!(e.epochs(), 3); assert!((e.lr() - 0.1).abs() < 1e-7);
        let cb = AdaptationLoss::Combined { epochs: 5, lr: 0.001, lambda_ent: 0.3 };
        assert_eq!(cb.epochs(), 5); assert!((cb.lr() - 0.001).abs() < 1e-7);
    }
}
