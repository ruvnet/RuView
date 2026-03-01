//! Common quantization trait shared by all quantizer types.

use crate::tier::TemperatureTier;
use alloc::vec::Vec;

/// Trait for vector quantization codecs.
///
/// Every quantizer can encode a float vector into a compact byte representation
/// and decode it back to an approximate float vector.
pub trait Quantizer {
    /// Encode a float vector into compact codes.
    fn encode(&self, vector: &[f32]) -> Vec<u8>;

    /// Decode compact codes back to an approximate float vector.
    fn decode(&self, codes: &[u8]) -> Vec<f32>;

    /// The temperature tier this quantizer is designed for.
    fn tier(&self) -> TemperatureTier;

    /// The dimensionality this quantizer was trained for.
    fn dim(&self) -> usize;
}
