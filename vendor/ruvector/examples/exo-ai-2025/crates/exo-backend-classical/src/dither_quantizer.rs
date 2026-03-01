//! DitheredQuantizer: deterministic low-bit quantization for exo activations.
//!
//! Wraps `ruvector-dither` to provide drop-in dithered quantization for
//! exo-backend-classical activation and weight tensors.
//!
//! Dithering breaks power-of-two resonances that cause idle tones / sticky
//! activations in 3/5/7-bit inference — without any RNG overhead.
//!
//! # Quick start
//!
//! ```
//! use exo_backend_classical::dither_quantizer::{DitheredQuantizer, DitherKind};
//!
//! // 8-bit, golden-ratio dither, layer 0, 16 channels, ε = 0.5 LSB
//! let mut q = DitheredQuantizer::new(DitherKind::GoldenRatio, 0, 16, 8, 0.5);
//!
//! let mut activations = vec![0.3_f32, -0.7, 0.5, 0.1];
//! q.quantize(&mut activations);
//! assert!(activations.iter().all(|&v| v >= -1.0 && v <= 1.0));
//! ```

use ruvector_dither::{channel::ChannelDither, quantize_slice_dithered, PiDither};

/// Which deterministic dither sequence to use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DitherKind {
    /// Golden-ratio quasi-random sequence (best equidistribution, no period).
    GoldenRatio,
    /// π-digit cyclic sequence (period = 256; ideal for weight pack-time use).
    Pi,
}

enum Source {
    Golden(ChannelDither),
    Pi(PiDither),
}

/// Dithered quantizer for exo activation / weight tensors.
pub struct DitheredQuantizer {
    source: Source,
    bits: u32,
    eps: f32,
}

impl DitheredQuantizer {
    /// Create a new quantizer.
    ///
    /// - `kind`       – dither sequence type
    /// - `layer_id`   – identifies this layer (seeds per-channel states)
    /// - `n_channels` – number of independent channels (ignored for Pi)
    /// - `bits`       – quantizer bit-width (3–8)
    /// - `eps`        – dither amplitude in LSB units (0.5 recommended)
    pub fn new(kind: DitherKind, layer_id: u32, n_channels: usize, bits: u32, eps: f32) -> Self {
        let source = match kind {
            DitherKind::GoldenRatio => {
                Source::Golden(ChannelDither::new(layer_id, n_channels, bits, eps))
            }
            DitherKind::Pi => Source::Pi(PiDither::from_tensor_id(layer_id)),
        };
        Self { source, bits, eps }
    }

    /// Quantize `activations` in-place.
    ///
    /// Each element is rounded to the nearest representable value in
    /// `[-1.0, 1.0]` at `bits`-bit precision with dither applied.
    pub fn quantize(&mut self, activations: &mut [f32]) {
        match &mut self.source {
            Source::Golden(cd) => cd.quantize_batch(activations),
            Source::Pi(pd) => quantize_slice_dithered(activations, self.bits, self.eps, pd),
        }
    }

    /// Reset the dither state to the initial seed (useful for reproducible tests).
    pub fn reset(&mut self, layer_id: u32, n_channels: usize) {
        match &mut self.source {
            Source::Golden(cd) => {
                *cd = ChannelDither::new(layer_id, n_channels, self.bits, self.eps);
            }
            Source::Pi(pd) => {
                *pd = PiDither::from_tensor_id(layer_id);
            }
        }
    }

    /// Bit-width used by this quantizer.
    pub fn bits(&self) -> u32 {
        self.bits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_quantizer_in_bounds() {
        let mut q = DitheredQuantizer::new(DitherKind::GoldenRatio, 0, 8, 8, 0.5);
        let mut acts: Vec<f32> = (0..64).map(|i| (i as f32 / 63.0) * 2.0 - 1.0).collect();
        q.quantize(&mut acts);
        for v in &acts {
            assert!(*v >= -1.0 && *v <= 1.0, "out of bounds: {v}");
        }
    }

    #[test]
    fn pi_quantizer_in_bounds() {
        let mut q = DitheredQuantizer::new(DitherKind::Pi, 42, 1, 5, 0.5);
        let mut acts = vec![0.3_f32, -0.7, 0.5, 0.1, -1.0, 1.0];
        q.quantize(&mut acts);
        for v in &acts {
            assert!(*v >= -1.0 && *v <= 1.0, "out of bounds: {v}");
        }
    }

    #[test]
    fn different_layers_different_output() {
        let input: Vec<f32> = vec![0.5; 16];

        let quantize = |layer: u32| {
            let mut buf = input.clone();
            let mut q = DitheredQuantizer::new(DitherKind::GoldenRatio, layer, 8, 8, 0.5);
            q.quantize(&mut buf);
            buf
        };
        assert_ne!(quantize(0), quantize(1));
    }

    #[test]
    fn deterministic_after_reset() {
        let input: Vec<f32> = vec![0.3, -0.4, 0.7, -0.1, 0.9];
        let mut q = DitheredQuantizer::new(DitherKind::GoldenRatio, 7, 4, 8, 0.5);

        let mut buf1 = input.clone();
        q.quantize(&mut buf1);

        q.reset(7, 4);
        let mut buf2 = input.clone();
        q.quantize(&mut buf2);

        assert_eq!(buf1, buf2, "reset must restore deterministic output");
    }

    #[test]
    fn three_bit_quantization() {
        let mut q = DitheredQuantizer::new(DitherKind::Pi, 0, 1, 3, 0.5);
        let mut acts = vec![-0.9_f32, -0.5, 0.0, 0.5, 0.9];
        q.quantize(&mut acts);
        for v in &acts {
            assert!(*v >= -1.0 && *v <= 1.0);
        }
        // 3-bit: qmax = 3, only multiples of 1/3 are valid
        let step = 1.0 / 3.0;
        for v in &acts {
            let rem = (v / step).round() * step - v;
            assert!(rem.abs() < 1e-5, "3-bit output should be on grid: {v}");
        }
    }
}
