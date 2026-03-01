//! Quantization accuracy tests.
//!
//! Tests rvf-quant scalar and binary quantization to verify
//! compression ratios and error bounds.

use rvf_quant::binary::{decode_binary, encode_binary, hamming_distance};
use rvf_quant::scalar::ScalarQuantizer;
use rvf_quant::traits::Quantizer;

/// Generate pseudo-random unit vectors using a simple LCG.
fn random_unit_vectors(n: usize, dim: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            let v: Vec<f32> = (0..dim)
                .map(|_| {
                    s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
                    (s >> 33) as f32 / (1u64 << 31) as f32 - 0.5
                })
                .collect();
            let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                v.iter().map(|x| x / norm).collect()
            } else {
                v
            }
        })
        .collect()
}

#[test]
fn scalar_quantize_round_trip() {
    let vectors = random_unit_vectors(100, 64, 42);
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let quantizer = ScalarQuantizer::train(&refs);

    for v in &vectors {
        let encoded = quantizer.encode(v);
        let decoded = quantizer.decode(&encoded);

        assert_eq!(decoded.len(), v.len());

        let mse: f32 = v
            .iter()
            .zip(decoded.iter())
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f32>()
            / v.len() as f32;

        assert!(mse < 0.01, "scalar quantization MSE too high: {mse:.6}");
    }
}

#[test]
fn scalar_quantizer_compresses_4x() {
    let vectors = random_unit_vectors(10, 128, 42);
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let quantizer = ScalarQuantizer::train(&refs);

    let original_bytes = 128 * 4; // f32 = 4 bytes
    let encoded = quantizer.encode(&vectors[0]);
    let encoded_bytes = encoded.len();

    // Scalar quantization (int8) should achieve ~4x compression.
    let ratio = original_bytes as f64 / encoded_bytes as f64;
    assert!(
        ratio >= 3.0,
        "compression ratio {ratio:.1}x, expected >= 3.0x"
    );
}

#[test]
fn binary_quantize_round_trip() {
    let vectors = random_unit_vectors(50, 128, 42);

    for v in &vectors {
        let encoded = encode_binary(v);
        let decoded = decode_binary(&encoded, v.len());

        assert_eq!(decoded.len(), v.len());
        for &d in &decoded {
            assert!(
                d == 1.0 || d == -1.0,
                "binary decode should be +/-1, got {d}"
            );
        }

        // Sign should match for most components.
        let sign_matches = v
            .iter()
            .zip(decoded.iter())
            .filter(|(&a, &b)| (a >= 0.0 && b > 0.0) || (a < 0.0 && b < 0.0))
            .count();
        let match_rate = sign_matches as f64 / v.len() as f64;
        assert!(
            match_rate >= 0.5,
            "binary quantization sign match rate {match_rate:.2}, expected >= 0.5"
        );
    }
}

#[test]
fn binary_compression_ratio_32x() {
    let dim = 256;
    let original_bytes = dim * 4; // f32
    let encoded = encode_binary(&vec![0.5f32; dim]);
    let encoded_bytes = encoded.len();

    let ratio = original_bytes as f64 / encoded_bytes as f64;
    assert!(
        ratio >= 25.0,
        "binary compression ratio {ratio:.1}x, expected >= 25.0x"
    );
}

#[test]
fn hamming_distance_properties() {
    let a = vec![1.0f32; 64];
    let b = vec![-1.0f32; 64];
    let c = vec![1.0f32; 64];

    let enc_a = encode_binary(&a);
    let enc_b = encode_binary(&b);
    let enc_c = encode_binary(&c);

    // Distance to self is 0.
    assert_eq!(hamming_distance(&enc_a, &enc_a), 0);

    // Opposite vectors have maximum distance.
    let max_dist = hamming_distance(&enc_a, &enc_b);
    assert_eq!(
        max_dist, 64,
        "opposite vectors should have hamming distance = dim"
    );

    // Identical vectors have distance 0.
    assert_eq!(hamming_distance(&enc_a, &enc_c), 0);

    // Triangle inequality.
    let d_ab = hamming_distance(&enc_a, &enc_b);
    let d_bc = hamming_distance(&enc_b, &enc_c);
    let d_ac = hamming_distance(&enc_a, &enc_c);
    assert!(d_ac <= d_ab + d_bc, "triangle inequality violated");
}

#[test]
fn scalar_quantizer_preserves_nearest_neighbor_ordering() {
    let vectors = random_unit_vectors(100, 32, 42);
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let quantizer = ScalarQuantizer::train(&refs);

    let query = &vectors[0];
    let encoded_query = quantizer.encode_vec(query);

    // Compute distances in original and quantized space.
    let mut original_dists: Vec<(usize, f32)> = vectors
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, v)| {
            let d: f32 = query
                .iter()
                .zip(v.iter())
                .map(|(a, b)| (a - b) * (a - b))
                .sum();
            (i, d)
        })
        .collect();
    original_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    let mut quant_dists: Vec<(usize, f32)> = vectors
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, v)| {
            let encoded = quantizer.encode_vec(v);
            let d = quantizer.distance_l2_quantized(&encoded_query, &encoded);
            (i, d)
        })
        .collect();
    quant_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    // The top-5 nearest neighbors should overlap significantly.
    let top_k = 5;
    let original_top: std::collections::HashSet<usize> =
        original_dists.iter().take(top_k).map(|(i, _)| *i).collect();
    let quant_top: std::collections::HashSet<usize> =
        quant_dists.iter().take(top_k).map(|(i, _)| *i).collect();

    let overlap = original_top.intersection(&quant_top).count();
    assert!(
        overlap >= 3,
        "top-{top_k} overlap = {overlap}, expected >= 3"
    );
}
