//! # ruvector-dither
//!
//! Deterministic, low-discrepancy **pre-quantization dithering** for low-bit
//! inference on tiny devices (WASM, Seed, STM32).
//!
//! ## Why dither?
//!
//! Quantizers at 3 / 5 / 7 bits can align with power-of-two boundaries and
//! produce idle tones / limit cycles — sticky activations and periodic errors
//! that degrade accuracy.  A sub-LSB pre-quantization offset:
//!
//! - Decorrelates the signal from grid boundaries.
//! - Pushes quantization error toward high frequencies (blue-noise-like),
//!   which average out downstream.
//! - Uses **no RNG** — outputs are deterministic, reproducible across
//!   platforms (WASM / x86 / ARM), and cache-friendly.
//!
//! ## Sequences
//!
//! | Type | State update | Properties |
//! |------|-------------|------------|
//! | [`GoldenRatioDither`] | frac(state + φ) | Best 1-D equidistribution |
//! | [`PiDither`] | table of π bytes | Reproducible, period = 256 |
//!
//! ## Quick start
//!
//! ```
//! use ruvector_dither::{GoldenRatioDither, PiDither, quantize_dithered};
//!
//! // Quantize with golden-ratio dither, 8-bit, ε = 0.5 LSB
//! let mut gr = GoldenRatioDither::new(0.0);
//! let q = quantize_dithered(0.314, 8, 0.5, &mut gr);
//! assert!(q >= -1.0 && q <= 1.0);
//!
//! // Quantize with π-digit dither
//! let mut pi = PiDither::new(0);
//! let q2 = quantize_dithered(0.271, 5, 0.5, &mut pi);
//! assert!(q2 >= -1.0 && q2 <= 1.0);
//! ```

#![cfg_attr(feature = "no_std", no_std)]

pub mod channel;
pub mod golden;
pub mod pi;
pub mod quantize;

pub use channel::ChannelDither;
pub use golden::GoldenRatioDither;
pub use pi::PiDither;
pub use quantize::{quantize_dithered, quantize_slice_dithered};

/// Trait implemented by any deterministic dither source.
pub trait DitherSource {
    /// Advance the sequence and return the next zero-mean offset in `[-0.5, +0.5]`.
    fn next_unit(&mut self) -> f32;

    /// Scale output to ε × LSB amplitude.
    #[inline]
    fn next(&mut self, eps_lsb: f32) -> f32 {
        self.next_unit() * eps_lsb
    }
}
