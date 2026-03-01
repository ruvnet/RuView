//! Virtual Domain Augmentation for cross-environment generalization (ADR-027 Phase 4).
//!
//! Generates synthetic "virtual domains" simulating different physical environments
//! and applies domain-specific transformations to CSI amplitude frames for the
//! MERIDIAN adversarial training loop.
//!
//! ```rust
//! use wifi_densepose_train::virtual_aug::{VirtualDomainAugmentor, Xorshift64};
//!
//! let mut aug = VirtualDomainAugmentor::default();
//! let mut rng = Xorshift64::new(42);
//! let frame = vec![0.5_f32; 56];
//! let domain = aug.generate_domain(&mut rng);
//! let out = aug.augment_frame(&frame, &domain);
//! assert_eq!(out.len(), frame.len());
//! ```

use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Xorshift64 PRNG (matches dataset.rs pattern)
// ---------------------------------------------------------------------------

/// Lightweight 64-bit Xorshift PRNG for deterministic augmentation.
pub struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    /// Create a new PRNG. Seed `0` is replaced with a fixed non-zero value.
    pub fn new(seed: u64) -> Self {
        Self { state: if seed == 0 { 0x853c49e6748fea9b } else { seed } }
    }

    /// Advance the state and return the next `u64`.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    /// Return a uniformly distributed `f32` in `[0, 1)`.
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Return a uniformly distributed `f32` in `[lo, hi)`.
    #[inline]
    pub fn next_f32_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.next_f32() * (hi - lo)
    }

    /// Return a uniformly distributed `usize` in `[lo, hi]` (inclusive).
    #[inline]
    pub fn next_usize_range(&mut self, lo: usize, hi: usize) -> usize {
        if lo >= hi { return lo; }
        lo + (self.next_u64() % (hi - lo + 1) as u64) as usize
    }

    /// Sample an approximate Gaussian (mean=0, std=1) via Box-Muller.
    #[inline]
    pub fn next_gaussian(&mut self) -> f32 {
        let u1 = self.next_f32().max(1e-10);
        let u2 = self.next_f32();
        (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
    }
}

// ---------------------------------------------------------------------------
// VirtualDomain
// ---------------------------------------------------------------------------

/// Describes a single synthetic WiFi environment for domain augmentation.
#[derive(Debug, Clone)]
pub struct VirtualDomain {
    /// Path-loss factor simulating room size (< 1 smaller, > 1 larger room).
    pub room_scale: f32,
    /// Wall reflection coefficient in `[0, 1]` (low = absorptive, high = reflective).
    pub reflection_coeff: f32,
    /// Number of virtual scatterers (furniture / obstacles).
    pub n_scatterers: usize,
    /// Standard deviation of additive hardware noise.
    pub noise_std: f32,
    /// Unique label for the domain classifier in adversarial training.
    pub domain_id: u32,
}

// ---------------------------------------------------------------------------
// VirtualDomainAugmentor
// ---------------------------------------------------------------------------

/// Samples virtual WiFi domains and transforms CSI frames to simulate them.
///
/// Applies four transformations: room-scale amplitude scaling, per-subcarrier
/// reflection modulation, virtual scatterer sinusoidal interference, and
/// Gaussian noise injection.
#[derive(Debug, Clone)]
pub struct VirtualDomainAugmentor {
    /// Range for room scale factor `(min, max)`.
    pub room_scale_range: (f32, f32),
    /// Range for reflection coefficient `(min, max)`.
    pub reflection_coeff_range: (f32, f32),
    /// Range for number of virtual scatterers `(min, max)`.
    pub n_virtual_scatterers: (usize, usize),
    /// Range for noise standard deviation `(min, max)`.
    pub noise_std_range: (f32, f32),
    next_domain_id: u32,
}

impl Default for VirtualDomainAugmentor {
    fn default() -> Self {
        Self {
            room_scale_range: (0.5, 2.0),
            reflection_coeff_range: (0.3, 0.9),
            n_virtual_scatterers: (0, 5),
            noise_std_range: (0.01, 0.1),
            next_domain_id: 0,
        }
    }
}

impl VirtualDomainAugmentor {
    /// Randomly sample a new [`VirtualDomain`] from the configured ranges.
    pub fn generate_domain(&mut self, rng: &mut Xorshift64) -> VirtualDomain {
        let id = self.next_domain_id;
        self.next_domain_id = self.next_domain_id.wrapping_add(1);
        VirtualDomain {
            room_scale: rng.next_f32_range(self.room_scale_range.0, self.room_scale_range.1),
            reflection_coeff: rng.next_f32_range(self.reflection_coeff_range.0, self.reflection_coeff_range.1),
            n_scatterers: rng.next_usize_range(self.n_virtual_scatterers.0, self.n_virtual_scatterers.1),
            noise_std: rng.next_f32_range(self.noise_std_range.0, self.noise_std_range.1),
            domain_id: id,
        }
    }

    /// Transform a single CSI amplitude frame to simulate `domain`.
    ///
    /// Pipeline: (1) scale by `1/room_scale`, (2) per-subcarrier reflection
    /// modulation, (3) scatterer sinusoidal perturbation, (4) Gaussian noise.
    pub fn augment_frame(&self, frame: &[f32], domain: &VirtualDomain) -> Vec<f32> {
        let n = frame.len();
        let n_f = n as f32;
        let mut noise_rng = Xorshift64::new(
            (domain.domain_id as u64).wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(1),
        );
        let mut out = Vec::with_capacity(n);
        for (k, &val) in frame.iter().enumerate() {
            let k_f = k as f32;
            // 1. Room-scale amplitude attenuation (guard against zero scale)
            let scaled = if domain.room_scale.abs() < 1e-10 { val } else { val / domain.room_scale };
            // 2. Reflection coefficient modulation (per-subcarrier)
            let refl = domain.reflection_coeff
                + (1.0 - domain.reflection_coeff) * (PI * k_f / n_f).cos();
            let modulated = scaled * refl;
            // 3. Virtual scatterer sinusoidal interference
            let mut scatter = 0.0_f32;
            for s in 0..domain.n_scatterers {
                scatter += 0.05 * (2.0 * PI * (s as f32 + 1.0) * k_f / n_f).sin();
            }
            // 4. Additive Gaussian noise
            out.push(modulated + scatter + noise_rng.next_gaussian() * domain.noise_std);
        }
        out
    }

    /// Augment a batch, producing `k` virtual-domain variants per input frame.
    ///
    /// Returns `(augmented_frame, domain_id)` pairs; total = `batch.len() * k`.
    pub fn augment_batch(
        &mut self, batch: &[Vec<f32>], k: usize, rng: &mut Xorshift64,
    ) -> Vec<(Vec<f32>, u32)> {
        let mut results = Vec::with_capacity(batch.len() * k);
        for frame in batch {
            for _ in 0..k {
                let domain = self.generate_domain(rng);
                let augmented = self.augment_frame(frame, &domain);
                results.push((augmented, domain.domain_id));
            }
        }
        results
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_domain(scale: f32, coeff: f32, scatter: usize, noise: f32, id: u32) -> VirtualDomain {
        VirtualDomain { room_scale: scale, reflection_coeff: coeff, n_scatterers: scatter, noise_std: noise, domain_id: id }
    }

    #[test]
    fn domain_within_configured_ranges() {
        let mut aug = VirtualDomainAugmentor::default();
        let mut rng = Xorshift64::new(12345);
        for _ in 0..100 {
            let d = aug.generate_domain(&mut rng);
            assert!(d.room_scale >= 0.5 && d.room_scale <= 2.0);
            assert!(d.reflection_coeff >= 0.3 && d.reflection_coeff <= 0.9);
            assert!(d.n_scatterers <= 5);
            assert!(d.noise_std >= 0.01 && d.noise_std <= 0.1);
        }
    }

    #[test]
    fn augment_frame_preserves_length() {
        let aug = VirtualDomainAugmentor::default();
        let out = aug.augment_frame(&vec![0.5; 56], &make_domain(1.0, 0.5, 3, 0.05, 0));
        assert_eq!(out.len(), 56);
    }

    #[test]
    fn augment_frame_identity_domain_approx_input() {
        let aug = VirtualDomainAugmentor::default();
        let frame: Vec<f32> = (0..56).map(|i| 0.3 + 0.01 * i as f32).collect();
        let out = aug.augment_frame(&frame, &make_domain(1.0, 1.0, 0, 0.0, 0));
        for (a, b) in out.iter().zip(frame.iter()) {
            assert!((a - b).abs() < 1e-5, "identity domain: got {a}, expected {b}");
        }
    }

    #[test]
    fn augment_batch_produces_correct_count() {
        let mut aug = VirtualDomainAugmentor::default();
        let mut rng = Xorshift64::new(99);
        let batch: Vec<Vec<f32>> = (0..4).map(|_| vec![0.5; 56]).collect();
        let results = aug.augment_batch(&batch, 3, &mut rng);
        assert_eq!(results.len(), 12);
        for (f, _) in &results { assert_eq!(f.len(), 56); }
    }

    #[test]
    fn different_seeds_produce_different_augmentations() {
        let mut aug1 = VirtualDomainAugmentor::default();
        let mut aug2 = VirtualDomainAugmentor::default();
        let frame = vec![0.5_f32; 56];
        let d1 = aug1.generate_domain(&mut Xorshift64::new(1));
        let d2 = aug2.generate_domain(&mut Xorshift64::new(2));
        let out1 = aug1.augment_frame(&frame, &d1);
        let out2 = aug2.augment_frame(&frame, &d2);
        assert!(out1.iter().zip(out2.iter()).any(|(a, b)| (a - b).abs() > 1e-6));
    }

    #[test]
    fn deterministic_same_seed_same_output() {
        let batch: Vec<Vec<f32>> = (0..3).map(|i| vec![0.1 * i as f32; 56]).collect();
        let mut aug1 = VirtualDomainAugmentor::default();
        let mut aug2 = VirtualDomainAugmentor::default();
        let res1 = aug1.augment_batch(&batch, 2, &mut Xorshift64::new(42));
        let res2 = aug2.augment_batch(&batch, 2, &mut Xorshift64::new(42));
        assert_eq!(res1.len(), res2.len());
        for ((f1, id1), (f2, id2)) in res1.iter().zip(res2.iter()) {
            assert_eq!(id1, id2);
            for (a, b) in f1.iter().zip(f2.iter()) {
                assert!((a - b).abs() < 1e-7, "same seed must produce identical output");
            }
        }
    }

    #[test]
    fn domain_ids_are_sequential() {
        let mut aug = VirtualDomainAugmentor::default();
        let mut rng = Xorshift64::new(7);
        for i in 0..10_u32 { assert_eq!(aug.generate_domain(&mut rng).domain_id, i); }
    }

    #[test]
    fn xorshift64_deterministic() {
        let mut a = Xorshift64::new(999);
        let mut b = Xorshift64::new(999);
        for _ in 0..100 { assert_eq!(a.next_u64(), b.next_u64()); }
    }

    #[test]
    fn xorshift64_f32_in_unit_interval() {
        let mut rng = Xorshift64::new(42);
        for _ in 0..1000 {
            let v = rng.next_f32();
            assert!(v >= 0.0 && v < 1.0, "f32 sample {v} not in [0, 1)");
        }
    }

    #[test]
    fn augment_frame_empty_and_batch_k_zero() {
        let aug = VirtualDomainAugmentor::default();
        assert!(aug.augment_frame(&[], &make_domain(1.5, 0.5, 2, 0.05, 0)).is_empty());
        let mut aug2 = VirtualDomainAugmentor::default();
        assert!(aug2.augment_batch(&[vec![0.5; 56]], 0, &mut Xorshift64::new(1)).is_empty());
    }
}
