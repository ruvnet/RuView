//! Per-channel and per-layer dither management.
//!
//! `ChannelDither` bundles one `GoldenRatioDither` state per channel,
//! seeded from `(layer_id, channel_id)` pairs so every channel is
//! structurally decorrelated without any RNG.

use crate::{DitherSource, GoldenRatioDither};

/// Per-channel dither pool seeded from `(layer_id, channel_id)` pairs.
///
/// Allocates one `GoldenRatioDither` per channel; each is independently
/// advanced, so channels cannot constructively interfere.
pub struct ChannelDither {
    channels: Vec<GoldenRatioDither>,
    bits: u32,
    eps: f32,
}

impl ChannelDither {
    /// Build a pool of `n_channels` dithers for `layer_id` / `bits` / `eps`.
    pub fn new(layer_id: u32, n_channels: usize, bits: u32, eps: f32) -> Self {
        let channels = (0..n_channels)
            .map(|ch| GoldenRatioDither::from_ids(layer_id, ch as u32))
            .collect();
        Self {
            channels,
            bits,
            eps,
        }
    }

    /// Quantize `activations` in-place.  Each column (channel dimension) uses
    /// its own independent dither state.
    ///
    /// `activations` is a flat row-major tensor of shape `[batch, channels]`.
    /// If the slice is not a multiple of `n_channels`, the remainder is
    /// processed using channel 0.
    pub fn quantize_batch(&mut self, activations: &mut [f32]) {
        assert!(
            !self.channels.is_empty(),
            "ChannelDither must have >= 1 channel"
        );
        assert!(self.bits >= 2 && self.bits <= 31, "bits must be in [2, 31]");
        let nc = self.channels.len();
        let qmax = ((1u32 << (self.bits - 1)) - 1) as f32;
        let lsb = 1.0 / qmax;
        for (i, x) in activations.iter_mut().enumerate() {
            let ch = i % nc;
            let d = self.channels[ch].next(self.eps * lsb);
            *x = ((*x + d) * qmax).round().clamp(-qmax, qmax) / qmax;
        }
    }

    /// Number of channels in this pool.
    pub fn n_channels(&self) -> usize {
        self.channels.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_dither_correct_count() {
        let cd = ChannelDither::new(0, 16, 8, 0.5);
        assert_eq!(cd.n_channels(), 16);
    }

    #[test]
    fn channel_dither_in_bounds() {
        let mut cd = ChannelDither::new(1, 8, 5, 0.5);
        let mut acts: Vec<f32> = (0..64).map(|i| (i as f32 / 63.0) * 2.0 - 1.0).collect();
        cd.quantize_batch(&mut acts);
        for v in acts {
            assert!(v >= -1.0 && v <= 1.0, "out of bounds: {v}");
        }
    }

    #[test]
    fn different_layers_produce_different_outputs() {
        let input: Vec<f32> = vec![0.5; 16];
        let mut buf0 = input.clone();
        let mut buf1 = input.clone();
        ChannelDither::new(0, 8, 8, 0.5).quantize_batch(&mut buf0);
        ChannelDither::new(99, 8, 8, 0.5).quantize_batch(&mut buf1);
        assert_ne!(
            buf0, buf1,
            "different layer_ids must yield different dithered outputs"
        );
    }
}
