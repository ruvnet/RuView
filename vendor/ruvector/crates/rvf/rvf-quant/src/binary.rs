//! Binary Quantization — 32x compression (1 bit per dimension).
//!
//! Used for the **Cold** (Tier 2) tier. Encodes only the sign of each
//! dimension and uses Hamming distance for comparison.

use alloc::vec;
use alloc::vec::Vec;

/// Encode a float vector to binary: 1 bit per dimension (sign bit).
///
/// Bit layout: dimension `d` maps to bit `d % 8` of byte `d / 8`.
/// A positive value (>= 0) is encoded as 1, negative as 0.
pub fn encode_binary(vector: &[f32]) -> Vec<u8> {
    let num_bytes = vector.len().div_ceil(8);
    let mut bits = vec![0u8; num_bytes];
    for (d, &val) in vector.iter().enumerate() {
        if val >= 0.0 {
            bits[d / 8] |= 1 << (d % 8);
        }
    }
    bits
}

/// Decode binary codes back to an approximate float vector.
///
/// Each bit is decoded to +1.0 (set) or -1.0 (unset).
pub fn decode_binary(bits: &[u8], dim: usize) -> Vec<f32> {
    let mut vector = Vec::with_capacity(dim);
    for d in 0..dim {
        let byte_idx = d / 8;
        let bit_idx = d % 8;
        if byte_idx < bits.len() && (bits[byte_idx] >> bit_idx) & 1 == 1 {
            vector.push(1.0);
        } else {
            vector.push(-1.0);
        }
    }
    vector
}

/// Compute the Hamming distance between two binary-encoded vectors.
///
/// Processes data in u64 chunks (8 bytes at a time) using `count_ones()`
/// which maps to hardware POPCNT on supported platforms. Falls back to
/// byte-by-byte processing for the remainder.
pub fn hamming_distance(a: &[u8], b: &[u8]) -> u32 {
    assert_eq!(a.len(), b.len(), "binary vectors must have equal length");
    let n = a.len();
    let chunks = n / 8;
    let remainder = n % 8;
    let mut dist = 0u32;

    // Process 8 bytes at a time using u64 popcount.
    for i in 0..chunks {
        let offset = i * 8;
        let xa = u64::from_le_bytes([
            a[offset],
            a[offset + 1],
            a[offset + 2],
            a[offset + 3],
            a[offset + 4],
            a[offset + 5],
            a[offset + 6],
            a[offset + 7],
        ]);
        let xb = u64::from_le_bytes([
            b[offset],
            b[offset + 1],
            b[offset + 2],
            b[offset + 3],
            b[offset + 4],
            b[offset + 5],
            b[offset + 6],
            b[offset + 7],
        ]);
        dist += (xa ^ xb).count_ones();
    }

    // Handle remainder bytes.
    let base = chunks * 8;
    for i in 0..remainder {
        dist += (a[base + i] ^ b[base + i]).count_ones();
    }
    dist
}

/// SIMD-accelerated Hamming distance (stub; falls back to scalar
/// when the `simd` feature is not enabled or unavailable).
#[cfg(feature = "simd")]
pub fn hamming_distance_simd(a: &[u8], b: &[u8]) -> u32 {
    // Future: VPOPCNTDQ / CNT implementation.
    hamming_distance(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_round_trip() {
        let v = vec![1.0, -0.5, 0.3, -2.0, 0.0, 0.1, -0.1, 0.9];
        let bits = encode_binary(&v);
        let decoded = decode_binary(&bits, v.len());

        // Check sign preservation
        for (d, (&orig, &dec)) in v.iter().zip(decoded.iter()).enumerate() {
            if orig >= 0.0 {
                assert_eq!(dec, 1.0, "dim {d}: expected +1 for val {orig}");
            } else {
                assert_eq!(dec, -1.0, "dim {d}: expected -1 for val {orig}");
            }
        }
    }

    #[test]
    fn hamming_self_is_zero() {
        let v = vec![1.0, -1.0, 0.5, -0.5, 0.0, 1.0, -1.0, 0.5];
        let bits = encode_binary(&v);
        assert_eq!(hamming_distance(&bits, &bits), 0);
    }

    #[test]
    fn hamming_opposite_is_max() {
        let v1 = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let v2 = vec![-1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0];
        let b1 = encode_binary(&v1);
        let b2 = encode_binary(&v2);
        assert_eq!(hamming_distance(&b1, &b2), 8);
    }

    #[test]
    fn hamming_matches_naive() {
        let v1 = vec![
            1.0, -1.0, 0.5, -0.5, 0.1, -0.1, 0.9, -0.9, 0.3, -0.3, 0.7, -0.7, 0.2, -0.2, 0.8, -0.8,
        ];
        let v2 = vec![
            -1.0, 1.0, -0.5, 0.5, -0.1, 0.1, -0.9, 0.9, -0.3, 0.3, -0.7, 0.7, -0.2, 0.2, -0.8, 0.8,
        ];
        let b1 = encode_binary(&v1);
        let b2 = encode_binary(&v2);

        // All signs are flipped -> hamming distance = 16
        assert_eq!(hamming_distance(&b1, &b2), 16);

        // Naive computation for verification
        let mut naive_dist = 0u32;
        for d in 0..16 {
            let s1 = if v1[d] >= 0.0 { 1 } else { 0 };
            let s2 = if v2[d] >= 0.0 { 1 } else { 0 };
            if s1 != s2 {
                naive_dist += 1;
            }
        }
        assert_eq!(hamming_distance(&b1, &b2), naive_dist);
    }

    #[test]
    fn non_multiple_of_8_dimensions() {
        let v = vec![1.0, -1.0, 0.5, -0.5, 0.1]; // 5 dims
        let bits = encode_binary(&v);
        assert_eq!(bits.len(), 1); // ceil(5/8) = 1
        let decoded = decode_binary(&bits, 5);
        assert_eq!(decoded.len(), 5);
        assert_eq!(decoded[0], 1.0);
        assert_eq!(decoded[1], -1.0);
        assert_eq!(decoded[4], 1.0); // 0.1 >= 0
    }
}
