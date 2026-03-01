//! Golden-ratio quasi-random dither sequence.
//!
//! State update: `state = frac(state + φ)` where φ = (√5−1)/2 ≈ 0.618…
//!
//! This is the 1-D Halton sequence in base φ — it has the best possible
//! equidistribution for a 1-D low-discrepancy sequence.

use crate::DitherSource;

/// Additive golden-ratio dither with zero-mean output in `[-0.5, 0.5]`.
///
/// The sequence has period 1 (irrational) so it never exactly repeats.
/// Two instances with different seeds stay decorrelated.
#[derive(Clone, Debug)]
pub struct GoldenRatioDither {
    state: f32,
}

/// φ = (√5 − 1) / 2
const PHI: f32 = 0.618_033_98_f32;

impl GoldenRatioDither {
    /// Create a new sequence seeded at `initial_state` ∈ [0, 1).
    ///
    /// For per-layer / per-channel decorrelation, seed with
    /// `frac(layer_id × φ + channel_id × φ²)`.
    #[inline]
    pub fn new(initial_state: f32) -> Self {
        Self {
            state: initial_state.abs().fract(),
        }
    }

    /// Construct from a `(layer_id, channel_id)` pair for structural decorrelation.
    #[inline]
    pub fn from_ids(layer_id: u32, channel_id: u32) -> Self {
        let s = ((layer_id as f32) * PHI + (channel_id as f32) * PHI * PHI).fract();
        Self { state: s }
    }

    /// Current state (useful for serialisation / checkpointing).
    #[inline]
    pub fn state(&self) -> f32 {
        self.state
    }
}

impl DitherSource for GoldenRatioDither {
    /// Advance and return next value in `[-0.5, 0.5]`.
    #[inline]
    fn next_unit(&mut self) -> f32 {
        self.state = (self.state + PHI).fract();
        self.state - 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DitherSource;

    #[test]
    fn output_is_in_range() {
        let mut d = GoldenRatioDither::new(0.0);
        for _ in 0..10_000 {
            let v = d.next_unit();
            assert!(v >= -0.5 && v <= 0.5, "out of range: {v}");
        }
    }

    #[test]
    fn mean_is_near_zero() {
        let mut d = GoldenRatioDither::new(0.0);
        let n = 100_000;
        let mean: f32 = (0..n).map(|_| d.next_unit()).sum::<f32>() / n as f32;
        assert!(mean.abs() < 0.01, "mean too large: {mean}");
    }

    #[test]
    fn from_ids_decorrelates() {
        let mut d0 = GoldenRatioDither::from_ids(0, 0);
        let mut d1 = GoldenRatioDither::from_ids(1, 7);
        // Confirm they start at different states
        let v0 = d0.next_unit();
        let v1 = d1.next_unit();
        assert!(
            (v0 - v1).abs() > 1e-4,
            "distinct seeds should produce distinct first values"
        );
    }

    #[test]
    fn deterministic_across_calls() {
        let mut d1 = GoldenRatioDither::new(0.123);
        let mut d2 = GoldenRatioDither::new(0.123);
        for _ in 0..1000 {
            assert_eq!(d1.next_unit(), d2.next_unit());
        }
    }
}
