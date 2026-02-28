//! Property-based roundtrip tests for temporal tensor compression.
//!
//! Verifies quantization roundtrip correctness across many random inputs
//! using a deterministic PRNG. No external dependencies.
//!
//! Run with:
//! ```sh
//! cargo test --release -p ruvector-temporal-tensor --test property_tests -- --nocapture
//! ```

use ruvector_temporal_tensor::bitpack;
use ruvector_temporal_tensor::delta;
use ruvector_temporal_tensor::f16;
use ruvector_temporal_tensor::quantizer;
use ruvector_temporal_tensor::segment;
use ruvector_temporal_tensor::tiering::{self, BlockMeta, TierConfig};

// ---------------------------------------------------------------------------
// Deterministic PRNG (LCG) -- no external deps
// ---------------------------------------------------------------------------

/// Simple linear congruential generator. Constants from Knuth MMIX.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn next_f32_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.next_f32() * (hi - lo)
    }

    fn next_usize_range(&mut self, lo: usize, hi: usize) -> usize {
        let range = (hi - lo) as u64;
        if range == 0 {
            return lo;
        }
        lo + (self.next_u64() % range) as usize
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const GROUP_LEN: usize = 64;

/// Generate a random f32 vector of the given length with values in [lo, hi].
fn random_vec(rng: &mut SimpleRng, len: usize, lo: f32, hi: f32) -> Vec<f32> {
    (0..len).map(|_| rng.next_f32_range(lo, hi)).collect()
}

/// Compute group-level maximum absolute values for error bounding.
fn group_max_abs(frame: &[f32], group_len: usize) -> Vec<f32> {
    frame
        .chunks(group_len)
        .map(|chunk| {
            chunk
                .iter()
                .filter(|v| v.is_finite())
                .map(|v| v.abs())
                .fold(0.0f32, f32::max)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 1. Quantize/Dequant Roundtrip Property
// ---------------------------------------------------------------------------

#[test]
fn prop_roundtrip_error_bounded() {
    let mut rng = SimpleRng::new(0xDEAD_BEEF_CAFE_BABE);

    // Error bounds as fraction of each group's max absolute value.
    // The absolute error per element is bounded by:
    //   scale * 1 (one quantization step) + f16 rounding (~0.1% of scale)
    // where scale = group_max_abs / qmax. So the error fraction of group_max is
    // approximately 1/qmax + small f16 term.
    //   8-bit: qmax=127, ~0.8% + margin -> 1%
    //   7-bit: qmax=63,  ~1.6% + margin -> 2%
    //   5-bit: qmax=15,  ~6.7% + margin -> 7%
    //   3-bit: qmax=3,  ~33%  + margin -> 35%
    let bit_configs: &[(u8, f32)] = &[
        (8, 0.01), // 8-bit: < 1% of group max
        (7, 0.02), // 7-bit: < 2% of group max
        (5, 0.07), // 5-bit: < 7% of group max
        (3, 0.35), // 3-bit: < 35% of group max
    ];

    for trial in 0..1000 {
        let len = rng.next_usize_range(64, 513); // 64..512 inclusive
        let frame = random_vec(&mut rng, len, -10.0, 10.0);

        for &(bits, max_err_frac) in bit_configs {
            let scales = quantizer::compute_scales(&frame, GROUP_LEN, bits);
            let scales_f32 = quantizer::scales_to_f32(&scales);

            let mut packed = Vec::new();
            quantizer::quantize_and_pack_f32(&frame, &scales_f32, GROUP_LEN, bits, &mut packed);

            let mut decoded = Vec::new();
            quantizer::dequantize_f32(
                &packed,
                &scales_f32,
                GROUP_LEN,
                bits,
                frame.len(),
                1,
                &mut decoded,
            );

            assert_eq!(
                decoded.len(),
                frame.len(),
                "trial={trial}, bits={bits}: length mismatch"
            );

            // Compute per-group max absolute value for error bounding.
            let gmax = group_max_abs(&frame, GROUP_LEN);

            for (i, (&orig, &dec)) in frame.iter().zip(decoded.iter()).enumerate() {
                let abs_err = (orig - dec).abs();
                let group_idx = i / GROUP_LEN;
                let group_m = if group_idx < gmax.len() {
                    gmax[group_idx]
                } else {
                    1.0
                };
                // Bound: max_err_frac * group_max + small absolute floor for near-zero groups.
                let bound = max_err_frac * group_m + 1e-6;
                assert!(
                    abs_err <= bound,
                    "trial={trial}, bits={bits}, i={i}: orig={orig}, dec={dec}, \
                     abs_err={abs_err}, bound={bound}, group_max={group_m}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 2. Bit Packing Roundtrip Property
// ---------------------------------------------------------------------------

#[test]
fn prop_bitpack_roundtrip() {
    let mut rng = SimpleRng::new(0x1234_5678_9ABC_DEF0);

    let bit_widths: &[u32] = &[3, 5, 7, 8];

    for _trial in 0..1000 {
        let count = rng.next_usize_range(1, 513);

        for &bits in bit_widths {
            let max_val = (1u32 << bits) - 1;
            let codes: Vec<u32> = (0..count)
                .map(|_| (rng.next_u64() as u32) % (max_val + 1))
                .collect();

            let mut packed = Vec::new();
            bitpack::pack(&codes, bits, &mut packed);

            let mut unpacked = Vec::new();
            bitpack::unpack(&packed, bits, count, &mut unpacked);

            assert_eq!(
                codes, unpacked,
                "bits={bits}, count={count}: pack/unpack mismatch"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3. Segment Encode/Decode Property
// ---------------------------------------------------------------------------

#[test]
fn prop_segment_roundtrip() {
    let mut rng = SimpleRng::new(0xFEED_FACE_DEAD_C0DE);

    let tensor_lens: &[usize] = &[32, 64, 128, 256, 512];
    let frame_counts: &[usize] = &[1, 2, 5, 10, 20];
    let bit_widths: &[u8] = &[3, 5, 7, 8];

    for _trial in 0..200 {
        let tensor_len = tensor_lens[rng.next_usize_range(0, tensor_lens.len())];
        let frame_count = frame_counts[rng.next_usize_range(0, frame_counts.len())];
        let bits = bit_widths[rng.next_usize_range(0, bit_widths.len())];

        // Generate the first frame and compute scales from it (shared across frames).
        let first_frame = random_vec(&mut rng, tensor_len, -5.0, 5.0);
        let scales = quantizer::compute_scales(&first_frame, GROUP_LEN, bits);
        let scales_f32 = quantizer::scales_to_f32(&scales);

        // Quantize all frames with the same scales.
        let mut packed = Vec::new();
        quantizer::quantize_and_pack_f32(&first_frame, &scales_f32, GROUP_LEN, bits, &mut packed);
        for _ in 1..frame_count {
            // Subsequent frames use values within the first frame's range to fit scales.
            let frame = random_vec(&mut rng, tensor_len, -4.0, 4.0);
            quantizer::quantize_and_pack_f32(&frame, &scales_f32, GROUP_LEN, bits, &mut packed);
        }

        // Encode into segment format.
        let mut seg = Vec::new();
        segment::encode(
            bits,
            GROUP_LEN as u32,
            tensor_len as u32,
            frame_count as u32,
            &scales,
            &packed,
            &mut seg,
        );

        // Decode the segment.
        let mut decoded = Vec::new();
        segment::decode(&seg, &mut decoded);

        assert_eq!(
            decoded.len(),
            tensor_len * frame_count,
            "trial={_trial}, bits={bits}, tensor_len={tensor_len}, frames={frame_count}: \
             decoded length mismatch"
        );

        // Parse the header and verify metadata.
        let header = segment::parse_header(&seg).expect("header should parse");
        assert_eq!(header.bits, bits);
        assert_eq!(header.tensor_len, tensor_len as u32);
        assert_eq!(header.frame_count, frame_count as u32);
        assert_eq!(header.group_len, GROUP_LEN as u32);
    }
}

// ---------------------------------------------------------------------------
// 4. f16 Roundtrip Property
// ---------------------------------------------------------------------------

#[test]
fn prop_f16_roundtrip() {
    let mut rng = SimpleRng::new(0xAAAA_BBBB_CCCC_DDDD);

    for _trial in 0..10_000 {
        // Generate value in scale-relevant range [1e-4, 1e4].
        let v = rng.next_f32_range(1e-4, 1e4);
        // Randomly negate half the values.
        let v = if rng.next_u64() & 1 == 0 { v } else { -v };

        let h = f16::f32_to_f16_bits(v);
        let back = f16::f16_bits_to_f32(h);

        // f16 has ~0.1% relative error for normal values in this range.
        let rel_err = ((back - v) / v).abs();
        assert!(
            rel_err < 0.002,
            "trial={_trial}: v={v}, back={back}, rel_err={rel_err}"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Delta Compute/Apply Property
// ---------------------------------------------------------------------------

#[test]
fn prop_delta_apply_recovers_new() {
    let mut rng = SimpleRng::new(0x0123_4567_89AB_CDEF);

    for trial in 0..500 {
        let len = rng.next_usize_range(8, 257);
        let old = random_vec(&mut rng, len, -5.0, 5.0);

        // Create "new" as old with a small number of perturbations.
        let mut new = old.clone();
        let num_changes = rng.next_usize_range(1, (len / 4).max(2));
        for _ in 0..num_changes {
            let idx = rng.next_usize_range(0, len);
            new[idx] += rng.next_f32_range(-1.0, 1.0);
        }

        let threshold = 0.001;
        let max_change_frac = 0.8;
        let result =
            delta::compute_delta(&old, &new, trial as u128, 0, 0, threshold, max_change_frac);

        match result {
            Some(d) => {
                // Apply delta to old, verify it approximates new.
                let mut reconstructed = old.clone();
                delta::apply_delta(&mut reconstructed, &d);

                for i in 0..len {
                    let err = (reconstructed[i] - new[i]).abs();
                    // Two sources of error:
                    //  1. Entries below threshold are not captured in the delta,
                    //     so the reconstruction error for those is up to `threshold`.
                    //  2. Captured entries have i16 quantization error of at most
                    //     delta_scale / 2 (half a quantization step).
                    let tolerance = threshold + d.delta_scale * 1.5 + 1e-6;
                    assert!(
                        err <= tolerance,
                        "trial={trial}, i={i}: recon={}, new={}, err={err}, tol={tolerance}",
                        reconstructed[i],
                        new[i]
                    );
                }
            }
            None => {
                // Delta was too large (>= max_change_fraction).
                // Verify that indeed many values changed.
                let changed = old
                    .iter()
                    .zip(new.iter())
                    .filter(|(&o, &n)| (o - n).abs() >= threshold)
                    .count();
                let fraction = changed as f32 / len as f32;
                assert!(
                    fraction >= max_change_frac,
                    "trial={trial}: delta was None but change fraction={fraction} < {max_change_frac}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 6. Compression Ratio Property
// ---------------------------------------------------------------------------

#[test]
fn prop_compression_ratio_matches_theory() {
    let mut rng = SimpleRng::new(0xCAFE_D00D_BEEF_FEED);

    let expected: &[(u8, f32)] = &[(8, 3.5), (7, 4.0), (5, 5.5), (3, 8.5)];

    for &(bits, min_ratio) in expected {
        // Use a 512-element tensor with group_len=64 for consistent measurement.
        let frame = random_vec(&mut rng, 512, -1.0, 1.0);
        let scales = quantizer::compute_scales(&frame, GROUP_LEN, bits);
        let mut packed = Vec::new();
        quantizer::quantize_and_pack(&frame, &scales, GROUP_LEN, bits, &mut packed);

        let raw_bytes = frame.len() * 4; // f32 = 4 bytes
        let compressed = packed.len() + scales.len() * 2; // packed data + f16 scales
        let ratio = raw_bytes as f32 / compressed as f32;

        assert!(
            ratio >= min_ratio,
            "bits={bits}: ratio={ratio:.2}x < expected={min_ratio}x \
             (raw={raw_bytes}, compressed={compressed})"
        );
    }
}

// ---------------------------------------------------------------------------
// 7. Score Monotonicity Property
// ---------------------------------------------------------------------------

#[test]
fn prop_score_monotonic_with_access() {
    let mut rng = SimpleRng::new(0x7777_8888_9999_AAAA);
    let config = TierConfig::default();

    for _trial in 0..100 {
        let start_tick = rng.next_u64() % 1000;
        let mut meta = BlockMeta::new(start_tick);

        // Score before any touch.
        let score_before = tiering::compute_score(&config, start_tick, &meta);

        // Touch the block.
        tiering::touch(&config, start_tick + 1, &mut meta);
        let score_after_touch = tiering::compute_score(&config, start_tick + 1, &meta);

        // Touching should increase (or at minimum maintain) the score.
        assert!(
            score_after_touch >= score_before - 1e-6,
            "trial={_trial}: score decreased after touch: \
             before={score_before}, after={score_after_touch}"
        );

        // Now let time pass without access -- score should decrease.
        let score_at_touch = tiering::compute_score(&config, start_tick + 1, &meta);
        let score_later = tiering::compute_score(&config, start_tick + 1000, &meta);

        assert!(
            score_later <= score_at_touch + 1e-6,
            "trial={_trial}: score increased without access: \
             at_touch={score_at_touch}, later={score_later}"
        );
    }
}

// ---------------------------------------------------------------------------
// 8. Zero Vector Property
// ---------------------------------------------------------------------------

#[test]
fn prop_zero_vector_roundtrip() {
    let bit_widths: &[u8] = &[3, 5, 7, 8];

    for &len in &[64, 128, 256, 512] {
        let frame = vec![0.0f32; len];

        for &bits in bit_widths {
            let scales = quantizer::compute_scales(&frame, GROUP_LEN, bits);
            let scales_f32 = quantizer::scales_to_f32(&scales);

            // All scales should be zero for a zero vector.
            for (i, &s) in scales_f32.iter().enumerate() {
                assert_eq!(
                    s, 0.0,
                    "len={len}, bits={bits}, group={i}: scale should be 0.0, got {s}"
                );
            }

            let mut packed = Vec::new();
            quantizer::quantize_and_pack_f32(&frame, &scales_f32, GROUP_LEN, bits, &mut packed);

            let mut decoded = Vec::new();
            quantizer::dequantize_f32(&packed, &scales_f32, GROUP_LEN, bits, len, 1, &mut decoded);

            assert_eq!(decoded.len(), len);
            for (i, &v) in decoded.iter().enumerate() {
                assert_eq!(
                    v, 0.0,
                    "len={len}, bits={bits}, i={i}: expected 0.0, got {v}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 9. Single-Value (Uniform) Vector Property
// ---------------------------------------------------------------------------

#[test]
fn prop_uniform_vector_roundtrip() {
    let mut rng = SimpleRng::new(0xBBBB_CCCC_DDDD_EEEE);
    let bit_widths: &[u8] = &[3, 5, 7, 8];

    for _trial in 0..200 {
        let len = rng.next_usize_range(64, 513);
        let value = rng.next_f32_range(-10.0, 10.0);
        let frame = vec![value; len];

        for &bits in bit_widths {
            let qmax = bitpack::qmax_from_bits(bits);
            if qmax == 0 {
                continue;
            }

            let scales = quantizer::compute_scales(&frame, GROUP_LEN, bits);
            let scales_f32 = quantizer::scales_to_f32(&scales);

            let mut packed = Vec::new();
            quantizer::quantize_and_pack_f32(&frame, &scales_f32, GROUP_LEN, bits, &mut packed);

            let mut decoded = Vec::new();
            quantizer::dequantize_f32(&packed, &scales_f32, GROUP_LEN, bits, len, 1, &mut decoded);

            assert_eq!(decoded.len(), len);

            // For a uniform vector, the quantization step is value.abs() / qmax.
            // Max error should be at most half a step (rounding) plus f16 scale error.
            let step = if value.abs() > 0.0 {
                value.abs() / qmax as f32
            } else {
                0.0
            };
            // Allow step/2 plus a small f16 rounding margin.
            let max_err = step * 0.5 + value.abs() * 0.002 + 1e-6;

            for (i, &dec) in decoded.iter().enumerate() {
                let err = (dec - value).abs();
                assert!(
                    err <= max_err,
                    "trial={_trial}, bits={bits}, i={i}: value={value}, dec={dec}, \
                     err={err}, max_err={max_err}, step={step}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 10. Extreme Value Property
// ---------------------------------------------------------------------------

#[test]
fn prop_extreme_values_dont_panic() {
    let bit_widths: &[u8] = &[3, 5, 7, 8];

    // Frames where scales stay within f16 representable range -- decoded values
    // must be finite.
    let finite_frames: Vec<Vec<f32>> = vec![
        // Very small positive values
        vec![f32::MIN_POSITIVE; 128],
        // Contains infinities and NaN (quantizer maps non-finite to 0)
        {
            let mut v = vec![1.0f32; 128];
            v[0] = f32::INFINITY;
            v[1] = f32::NEG_INFINITY;
            v[2] = f32::NAN;
            v[3] = -0.0;
            v
        },
        // All subnormal
        vec![1e-40f32; 128],
        // Alternating zero and large (within f16 scale range)
        (0..128)
            .map(|i| if i % 2 == 0 { 0.0 } else { 1e4 })
            .collect(),
    ];

    // Frames with magnitudes that overflow f16 scales -- we only assert
    // no panics and correct output length. The decoded values may be NaN/Inf
    // because scale overflows to f16 infinity.
    let overflow_frames: Vec<Vec<f32>> = vec![
        // All f32::MAX
        vec![f32::MAX; 128],
        // All f32::MIN (most negative finite)
        vec![f32::MIN; 128],
        // Mixed signs of large magnitude
        (0..128)
            .map(|i| if i % 2 == 0 { f32::MAX } else { f32::MIN })
            .collect(),
        // Mix of tiny and huge
        (0..128)
            .map(|i| {
                if i % 3 == 0 {
                    f32::MIN_POSITIVE
                } else if i % 3 == 1 {
                    1e30
                } else {
                    -1e30
                }
            })
            .collect(),
    ];

    // Test finite-output frames: no panics, correct length, all decoded finite.
    for (frame_idx, frame) in finite_frames.iter().enumerate() {
        for &bits in bit_widths {
            let scales = quantizer::compute_scales(frame, GROUP_LEN, bits);
            let scales_f32 = quantizer::scales_to_f32(&scales);

            let mut packed = Vec::new();
            quantizer::quantize_and_pack_f32(frame, &scales_f32, GROUP_LEN, bits, &mut packed);

            let mut decoded = Vec::new();
            quantizer::dequantize_f32(
                &packed,
                &scales_f32,
                GROUP_LEN,
                bits,
                frame.len(),
                1,
                &mut decoded,
            );

            assert_eq!(
                decoded.len(),
                frame.len(),
                "finite frame_idx={frame_idx}, bits={bits}: length mismatch"
            );

            for (i, &d) in decoded.iter().enumerate() {
                assert!(
                    d.is_finite(),
                    "finite frame_idx={frame_idx}, bits={bits}, i={i}: \
                     decoded value is not finite: {d}"
                );
            }
        }
    }

    // Test overflow frames: no panics, correct length (decoded may contain NaN/Inf).
    for (frame_idx, frame) in overflow_frames.iter().enumerate() {
        for &bits in bit_widths {
            let scales = quantizer::compute_scales(frame, GROUP_LEN, bits);
            let scales_f32 = quantizer::scales_to_f32(&scales);

            let mut packed = Vec::new();
            quantizer::quantize_and_pack_f32(frame, &scales_f32, GROUP_LEN, bits, &mut packed);

            let mut decoded = Vec::new();
            quantizer::dequantize_f32(
                &packed,
                &scales_f32,
                GROUP_LEN,
                bits,
                frame.len(),
                1,
                &mut decoded,
            );

            assert_eq!(
                decoded.len(),
                frame.len(),
                "overflow frame_idx={frame_idx}, bits={bits}: length mismatch"
            );
        }
    }

    // Bitpack roundtrip with boundary codes -- must not panic and must be exact.
    for &bits in bit_widths {
        let qmax = bitpack::qmax_from_bits(bits) as u32;
        if qmax > 0 {
            let max_code = qmax * 2;
            let codes: Vec<u32> = (0..128).map(|i| i as u32 % (max_code + 1)).collect();
            let mut bp = Vec::new();
            bitpack::pack(&codes, bits as u32, &mut bp);
            let mut unpacked = Vec::new();
            bitpack::unpack(&bp, bits as u32, codes.len(), &mut unpacked);
            assert_eq!(codes, unpacked);
        }
    }
}

// ---------------------------------------------------------------------------
// 11. Segment Compression Ratio is Positive
// ---------------------------------------------------------------------------

#[test]
fn prop_segment_compression_ratio_positive() {
    let mut rng = SimpleRng::new(0x1111_2222_3333_4444);

    for _trial in 0..100 {
        let tensor_len = 128;
        let bits = [3u8, 5, 7, 8][rng.next_usize_range(0, 4)];
        let frame = random_vec(&mut rng, tensor_len, -1.0, 1.0);

        let scales = quantizer::compute_scales(&frame, GROUP_LEN, bits);
        let mut packed = Vec::new();
        quantizer::quantize_and_pack(&frame, &scales, GROUP_LEN, bits, &mut packed);

        let mut seg = Vec::new();
        segment::encode(
            bits,
            GROUP_LEN as u32,
            tensor_len as u32,
            1,
            &scales,
            &packed,
            &mut seg,
        );

        let ratio = segment::compression_ratio(&seg);
        assert!(
            ratio > 1.0,
            "trial={_trial}, bits={bits}: compression ratio {ratio} should be > 1.0"
        );
    }
}

// ---------------------------------------------------------------------------
// 12. Single-Frame Decode Matches Full Decode
// ---------------------------------------------------------------------------

#[test]
fn prop_single_frame_decode_consistency() {
    let mut rng = SimpleRng::new(0x5555_6666_7777_8888);

    for _trial in 0..100 {
        let tensor_len = 64;
        let frame_count = rng.next_usize_range(1, 6);
        let bits = [3u8, 5, 7, 8][rng.next_usize_range(0, 4)];

        let first_frame = random_vec(&mut rng, tensor_len, -3.0, 3.0);
        let scales = quantizer::compute_scales(&first_frame, GROUP_LEN, bits);
        let scales_f32 = quantizer::scales_to_f32(&scales);

        let mut packed = Vec::new();
        quantizer::quantize_and_pack_f32(&first_frame, &scales_f32, GROUP_LEN, bits, &mut packed);
        for _ in 1..frame_count {
            let frame = random_vec(&mut rng, tensor_len, -2.5, 2.5);
            quantizer::quantize_and_pack_f32(&frame, &scales_f32, GROUP_LEN, bits, &mut packed);
        }

        let mut seg = Vec::new();
        segment::encode(
            bits,
            GROUP_LEN as u32,
            tensor_len as u32,
            frame_count as u32,
            &scales,
            &packed,
            &mut seg,
        );

        // Full decode.
        let mut all_decoded = Vec::new();
        segment::decode(&seg, &mut all_decoded);
        assert_eq!(all_decoded.len(), tensor_len * frame_count);

        // Single-frame decode should match the corresponding slice.
        for f in 0..frame_count {
            let single = segment::decode_single_frame(&seg, f);
            assert!(
                single.is_some(),
                "trial={_trial}, frame={f}: single-frame decode returned None"
            );
            let single = single.unwrap();
            let expected = &all_decoded[f * tensor_len..(f + 1) * tensor_len];
            assert_eq!(
                single.len(),
                expected.len(),
                "trial={_trial}, frame={f}: length mismatch"
            );
            for (i, (&s, &e)) in single.iter().zip(expected.iter()).enumerate() {
                assert!(
                    (s - e).abs() < 1e-6,
                    "trial={_trial}, frame={f}, i={i}: single={s}, full={e}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 13. Delta Encode/Decode Binary Roundtrip
// ---------------------------------------------------------------------------

#[test]
fn prop_delta_encode_decode_binary() {
    let mut rng = SimpleRng::new(0x9999_0000_1111_2222);

    for trial in 0..500 {
        let nnz = rng.next_usize_range(0, 100);
        let entries: Vec<delta::SparseEntry> = (0..nnz)
            .map(|_| delta::SparseEntry {
                index: (rng.next_u64() % 65536) as u16,
                value: (rng.next_u64() % 65536) as i16,
            })
            .collect();
        let scale = rng.next_f32_range(1e-6, 100.0);

        let record = delta::DeltaRecord {
            header: delta::DeltaHeader {
                tensor_id: rng.next_u64() as u128 | ((rng.next_u64() as u128) << 64),
                block_index: rng.next_u64() as u32,
                base_epoch: rng.next_u64(),
                nnz: nnz as u16,
            },
            delta_scale: scale,
            entries,
        };

        let bytes = delta::encode_delta(&record);
        let decoded = delta::decode_delta(&bytes)
            .unwrap_or_else(|e| panic!("trial={trial}: decode failed: {e:?}"));

        assert_eq!(decoded.header.tensor_id, record.header.tensor_id);
        assert_eq!(decoded.header.block_index, record.header.block_index);
        assert_eq!(decoded.header.base_epoch, record.header.base_epoch);
        assert_eq!(decoded.header.nnz, record.header.nnz);
        assert!(
            (decoded.delta_scale - record.delta_scale).abs() < 1e-10,
            "trial={trial}: scale mismatch"
        );
        assert_eq!(decoded.entries.len(), record.entries.len());
        for (i, (a, b)) in decoded
            .entries
            .iter()
            .zip(record.entries.iter())
            .enumerate()
        {
            assert_eq!(a.index, b.index, "trial={trial}, entry={i}: index mismatch");
            assert_eq!(a.value, b.value, "trial={trial}, entry={i}: value mismatch");
        }
    }
}

// ---------------------------------------------------------------------------
// 14. Quantization is Deterministic
// ---------------------------------------------------------------------------

#[test]
fn prop_quantization_deterministic() {
    let mut rng = SimpleRng::new(0xABCD_EF01_2345_6789);

    for _trial in 0..200 {
        let len = rng.next_usize_range(64, 257);
        let frame = random_vec(&mut rng, len, -5.0, 5.0);
        let bits = [3u8, 5, 7, 8][rng.next_usize_range(0, 4)];

        let scales = quantizer::compute_scales(&frame, GROUP_LEN, bits);
        let scales_f32 = quantizer::scales_to_f32(&scales);

        let mut packed1 = Vec::new();
        quantizer::quantize_and_pack_f32(&frame, &scales_f32, GROUP_LEN, bits, &mut packed1);

        let mut packed2 = Vec::new();
        quantizer::quantize_and_pack_f32(&frame, &scales_f32, GROUP_LEN, bits, &mut packed2);

        assert_eq!(
            packed1, packed2,
            "trial={_trial}, bits={bits}: quantization is not deterministic"
        );
    }
}
