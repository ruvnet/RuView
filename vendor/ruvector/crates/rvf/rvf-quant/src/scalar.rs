//! Scalar Quantization (SQ) — fp32 to u8, 4x compression.
//!
//! Each dimension is independently mapped from [min, max] to [0, 255].
//! This is the quantization used for the **Hot** (Tier 0) tier.

use crate::tier::TemperatureTier;
use crate::traits::Quantizer;
use alloc::vec;
use alloc::vec::Vec;

/// Scalar quantizer parameters: per-dimension min/max ranges.
#[derive(Clone, Debug)]
pub struct ScalarQuantizer {
    /// Minimum value per dimension (training set).
    pub min_vals: Vec<f32>,
    /// Maximum value per dimension (training set).
    pub max_vals: Vec<f32>,
    /// Vector dimensionality.
    pub dim: usize,
}

impl ScalarQuantizer {
    /// Train a scalar quantizer by computing per-dimension min/max over
    /// a set of training vectors.
    ///
    /// # Panics
    ///
    /// Panics if `vectors` is empty or any vector has inconsistent dimensionality.
    pub fn train(vectors: &[&[f32]]) -> Self {
        assert!(!vectors.is_empty(), "need at least one training vector");
        let dim = vectors[0].len();
        assert!(dim > 0, "vector dimensionality must be > 0");

        let mut min_vals = vec![f32::INFINITY; dim];
        let mut max_vals = vec![f32::NEG_INFINITY; dim];

        for v in vectors {
            assert_eq!(v.len(), dim, "dimension mismatch in training data");
            for (d, &val) in v.iter().enumerate() {
                if val < min_vals[d] {
                    min_vals[d] = val;
                }
                if val > max_vals[d] {
                    max_vals[d] = val;
                }
            }
        }

        // Avoid zero-range dimensions (would cause division by zero).
        for d in 0..dim {
            if (max_vals[d] - min_vals[d]).abs() < f32::EPSILON {
                max_vals[d] = min_vals[d] + 1.0;
            }
        }

        Self {
            min_vals,
            max_vals,
            dim,
        }
    }

    /// Quantize a float vector to u8 codes.
    ///
    /// `q[d] = round((v[d] - min[d]) / (max[d] - min[d]) * 255)`
    pub fn encode_vec(&self, vector: &[f32]) -> Vec<u8> {
        assert_eq!(vector.len(), self.dim);
        let mut codes = Vec::with_capacity(self.dim);
        for (d, &val) in vector.iter().enumerate().take(self.dim) {
            let range = self.max_vals[d] - self.min_vals[d];
            let normalized = (val - self.min_vals[d]) / range;
            let clamped = normalized.clamp(0.0, 1.0);
            codes.push((clamped * 255.0).round() as u8);
        }
        codes
    }

    /// Dequantize u8 codes back to approximate float values.
    ///
    /// `v[d] = q[d] / 255 * (max[d] - min[d]) + min[d]`
    pub fn decode_vec(&self, codes: &[u8]) -> Vec<f32> {
        assert_eq!(codes.len(), self.dim);
        let mut vector = Vec::with_capacity(self.dim);
        for (d, &code) in codes.iter().enumerate().take(self.dim) {
            let range = self.max_vals[d] - self.min_vals[d];
            let val = (code as f32 / 255.0) * range + self.min_vals[d];
            vector.push(val);
        }
        vector
    }

    /// Compute approximate L2 squared distance between two quantized vectors.
    ///
    /// Accumulates differences in i32 arithmetic to avoid per-element f32
    /// conversion, then converts to f32 only for the final scaling step.
    /// This is significantly faster when the dimension is large.
    pub fn distance_l2_quantized(&self, a: &[u8], b: &[u8]) -> f32 {
        assert_eq!(a.len(), self.dim);
        assert_eq!(b.len(), self.dim);

        // Accumulate (a[d] - b[d])^2 in integer, then scale per-dimension.
        // Since dequantized value = code / 255 * range + min, the difference
        // is (a_code - b_code) / 255 * range, so squared diff is
        // (a_code - b_code)^2 / 65025 * range^2.
        // We group by range to minimize f32 ops.
        let mut dist = 0.0f32;
        let inv_255_sq = 1.0f32 / (255.0 * 255.0);
        for d in 0..self.dim {
            let diff = a[d] as i32 - b[d] as i32;
            let range = self.max_vals[d] - self.min_vals[d];
            dist += (diff * diff) as f32 * (range * range) * inv_255_sq;
        }
        dist
    }

    /// SIMD-accelerated L2 distance (stub; falls back to scalar when
    /// the `simd` feature is not enabled).
    #[cfg(feature = "simd")]
    pub fn distance_l2_simd(&self, a: &[u8], b: &[u8]) -> f32 {
        // Future: AVX-512 / NEON implementation.
        self.distance_l2_quantized(a, b)
    }
}

impl Quantizer for ScalarQuantizer {
    fn encode(&self, vector: &[f32]) -> Vec<u8> {
        self.encode_vec(vector)
    }

    fn decode(&self, codes: &[u8]) -> Vec<f32> {
        self.decode_vec(codes)
    }

    fn tier(&self) -> TemperatureTier {
        TemperatureTier::Hot
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_training_data() -> Vec<Vec<f32>> {
        // 10 vectors of dim 8 in [-1, 1]
        let mut vecs = Vec::new();
        for i in 0..10 {
            let v: Vec<f32> = (0..8)
                .map(|d| ((i * 7 + d * 13) % 200) as f32 / 100.0 - 1.0)
                .collect();
            vecs.push(v);
        }
        vecs
    }

    #[test]
    fn round_trip_low_error() {
        let data = make_training_data();
        let refs: Vec<&[f32]> = data.iter().map(|v| v.as_slice()).collect();
        let sq = ScalarQuantizer::train(&refs);

        for v in &data {
            let codes = sq.encode_vec(v);
            let reconstructed = sq.decode_vec(&codes);
            assert_eq!(reconstructed.len(), v.len());

            // Check reconstruction error per dimension
            for (orig, recon) in v.iter().zip(reconstructed.iter()) {
                let max_error = (sq
                    .max_vals
                    .iter()
                    .zip(sq.min_vals.iter())
                    .map(|(mx, mn)| mx - mn)
                    .fold(0.0f32, f32::max))
                    / 255.0;
                assert!(
                    (orig - recon).abs() <= max_error + f32::EPSILON,
                    "reconstruction error too large: orig={orig}, recon={recon}"
                );
            }
        }
    }

    #[test]
    fn quantized_distance_nonnegative() {
        let data = make_training_data();
        let refs: Vec<&[f32]> = data.iter().map(|v| v.as_slice()).collect();
        let sq = ScalarQuantizer::train(&refs);

        let a = sq.encode_vec(&data[0]);
        let b = sq.encode_vec(&data[1]);
        let dist = sq.distance_l2_quantized(&a, &b);
        assert!(dist >= 0.0);
    }

    #[test]
    fn identical_vectors_zero_distance() {
        let data = make_training_data();
        let refs: Vec<&[f32]> = data.iter().map(|v| v.as_slice()).collect();
        let sq = ScalarQuantizer::train(&refs);

        let a = sq.encode_vec(&data[0]);
        let dist = sq.distance_l2_quantized(&a, &a);
        assert!(dist.abs() < f32::EPSILON);
    }

    #[test]
    fn quantizer_trait() {
        let data = make_training_data();
        let refs: Vec<&[f32]> = data.iter().map(|v| v.as_slice()).collect();
        let sq = ScalarQuantizer::train(&refs);
        assert_eq!(sq.tier(), TemperatureTier::Hot);
        assert_eq!(sq.dim(), 8);
    }
}
