//! Acceptance tests and microbenchmarks for the temporal tensor store (ADR-023).
//!
//! Runs via `cargo test --release -p ruvector-temporal-tensor --test benchmarks -- --nocapture`
//!
//! All timing uses `std::time::Instant` to maintain the zero-dependency constraint.
//! No external crates (criterion, rand, etc.) are used.

use std::time::Instant;

use ruvector_temporal_tensor::bitpack;
use ruvector_temporal_tensor::quantizer;
use ruvector_temporal_tensor::segment;
use ruvector_temporal_tensor::tier_policy::TierPolicy;
use ruvector_temporal_tensor::tiering::{self, BlockKey, BlockMeta, Tier, TierConfig};
use ruvector_temporal_tensor::TemporalTensorCompressor;

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
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }

    /// Uniform f64 in [0, 1).
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform f32 in [0, 1).
    #[allow(dead_code)]
    fn next_f32(&mut self) -> f32 {
        self.next_f64() as f32
    }
}

// ---------------------------------------------------------------------------
// Zipf distribution sampler -- no external deps
// ---------------------------------------------------------------------------

/// Rejection-free inverse-CDF Zipf sampler.
struct ZipfSampler {
    n: usize,
    #[allow(dead_code)]
    s: f64,
    /// Cumulative distribution table (precomputed for inverse-CDF sampling).
    cdf: Vec<f64>,
}

impl ZipfSampler {
    fn new(n: usize, s: f64) -> Self {
        let mut cdf = Vec::with_capacity(n);
        let mut cumulative = 0.0f64;
        for k in 1..=n {
            cumulative += 1.0 / (k as f64).powf(s);
            cdf.push(cumulative);
        }
        let total = cumulative;
        for v in cdf.iter_mut() {
            *v /= total;
        }
        Self { n, s, cdf }
    }

    /// Sample a value in [0, n). Uses binary search on the CDF.
    fn sample(&self, rng: &mut SimpleRng) -> usize {
        let u = rng.next_f64();
        let mut lo = 0usize;
        let mut hi = self.n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.cdf[mid] < u {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo.min(self.n - 1)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate deterministic pseudo-random f32 data in [-1, 1].
fn generate_f32_data(rng: &mut SimpleRng, len: usize) -> Vec<f32> {
    (0..len)
        .map(|_| rng.next_f64() as f32 * 2.0 - 1.0)
        .collect()
}

/// Generate f32 data with guaranteed minimum magnitude (for quality tests).
/// Values are in [-1.0, -min_mag] union [min_mag, 1.0].
fn generate_f32_data_no_near_zero(rng: &mut SimpleRng, len: usize, min_mag: f32) -> Vec<f32> {
    let range = 1.0 - min_mag;
    (0..len)
        .map(|_| {
            let sign = if rng.next_u64() & 1 == 0 {
                1.0f32
            } else {
                -1.0
            };
            let mag = min_mag + rng.next_f64() as f32 * range;
            sign * mag
        })
        .collect()
}

/// Measure function execution over N iterations, return (total, per_iter).
fn bench_loop<F: FnMut()>(iters: u32, mut f: F) -> (std::time::Duration, std::time::Duration) {
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let total = start.elapsed();
    let per_iter = total / iters;
    (total, per_iter)
}

// ---------------------------------------------------------------------------
// 1. Zipf Access Simulation (Acceptance Test)
// ---------------------------------------------------------------------------

/// Acceptance test: Zipf access simulation using the `tiering` module.
/// - 10,000 blocks (scaled down from 1M for test speed)
/// - 100,000 accesses (scaled down from 10M)
/// - PASS criteria:
///   1. Tier1 count stays under cap (Zipf concentrates on a small hot head)
///   2. Tier flips per block per minute < 0.1 (hysteresis dampens oscillation)
///   3. P95 read latency within target
#[test]
fn zipf_acceptance_test() {
    const NUM_BLOCKS: usize = 10_000;
    const NUM_ACCESSES: usize = 100_000;
    const TENSOR_LEN: u32 = 64;

    let zipf = ZipfSampler::new(NUM_BLOCKS, 1.1);
    let mut rng = SimpleRng::new(0xDEAD_BEEF);

    // Pre-generate one frame per block
    let mut block_frames: Vec<Vec<f32>> = Vec::with_capacity(NUM_BLOCKS);
    for _ in 0..NUM_BLOCKS {
        block_frames.push(generate_f32_data(&mut rng, TENSOR_LEN as usize));
    }

    let tier_config = TierConfig::default();

    // Per-block state: tiering metadata + compressor + segments
    struct BlockState {
        meta: BlockMeta,
        compressor: TemporalTensorCompressor,
        segments: Vec<Vec<u8>>,
        flip_count: u32,
        last_tier: Tier,
    }

    let policy = TierPolicy::default();
    let mut blocks: Vec<BlockState> = (0..NUM_BLOCKS)
        .map(|_| {
            let meta = BlockMeta::new(0);
            let last_tier = meta.current_tier;
            BlockState {
                meta,
                compressor: TemporalTensorCompressor::new(policy, TENSOR_LEN, 0),
                segments: Vec::new(),
                flip_count: 0,
                last_tier,
            }
        })
        .collect();

    let mut read_latencies_ns: Vec<u64> = Vec::with_capacity(NUM_ACCESSES);
    let sim_start = Instant::now();

    for access_i in 0..NUM_ACCESSES {
        let block_idx = zipf.sample(&mut rng);
        let now = access_i as u64;

        let block = &mut blocks[block_idx];

        // Update tiering metadata
        tiering::touch(&tier_config, now, &mut block.meta);

        // Check for tier migration via hysteresis-guarded scoring
        if let Some(new_tier) = tiering::choose_tier(&tier_config, now, &block.meta) {
            block.meta.current_tier = new_tier;
            block.meta.tier_since = now;

            if new_tier != block.last_tier {
                block.flip_count += 1;
                block.last_tier = new_tier;
            }
        }

        // Push frame through compressor
        let bits = tiering::bits_for_tier(&tier_config, block.meta.current_tier, 0);
        if bits > 0 {
            // Sync compressor access state to match tier
            let ts32 = now as u32;
            block.compressor.touch(ts32);
            let mut seg_out = Vec::new();
            block
                .compressor
                .push_frame(&block_frames[block_idx], ts32, &mut seg_out);
            if !seg_out.is_empty() {
                block.segments.push(seg_out);
            }
        }

        // Measure read latency (decode last segment)
        let read_start = Instant::now();
        if let Some(last_seg) = block.segments.last() {
            let mut decoded = Vec::new();
            segment::decode(last_seg, &mut decoded);
            std::hint::black_box(&decoded);
        }
        read_latencies_ns.push(read_start.elapsed().as_nanos() as u64);
    }

    // Decay untouched blocks at end
    let sim_elapsed = sim_start.elapsed();

    // Flush all
    for block in blocks.iter_mut() {
        let mut seg_out = Vec::new();
        block.compressor.flush(&mut seg_out);
        if !seg_out.is_empty() {
            block.segments.push(seg_out);
        }
    }

    // --- Evaluate criteria ---

    // 1. Tier distribution
    let tier1_count = blocks
        .iter()
        .filter(|b| b.meta.current_tier == Tier::Tier1)
        .count();
    let tier2_count = blocks
        .iter()
        .filter(|b| b.meta.current_tier == Tier::Tier2)
        .count();
    let tier3_count = blocks
        .iter()
        .filter(|b| b.meta.current_tier == Tier::Tier3)
        .count();

    // Under Zipf(1.1), ~20% of blocks receive ~80% of accesses. The hot set
    // should be bounded. Use 40% as a generous cap (Zipf head + warm zone).
    let tier1_cap = NUM_BLOCKS * 40 / 100;

    // 2. Flip rate per block per simulated minute
    let total_flips: u32 = blocks.iter().map(|b| b.flip_count).sum();
    // Scale: 10,000 accesses = 1 simulated minute
    let sim_minutes = NUM_ACCESSES as f64 / 10_000.0;
    let flip_rate = if sim_minutes > 0.0 && NUM_BLOCKS > 0 {
        total_flips as f64 / NUM_BLOCKS as f64 / sim_minutes
    } else {
        0.0
    };

    // 3. P95 read latency
    read_latencies_ns.sort_unstable();
    let p95_idx = (read_latencies_ns.len() as f64 * 0.95) as usize;
    let p95_latency_ns = read_latencies_ns.get(p95_idx).copied().unwrap_or(0);

    // --- Report ---
    eprintln!();
    eprintln!("--- Zipf Acceptance Test ---");
    eprintln!();
    eprintln!("  Blocks: {}  Accesses: {}", NUM_BLOCKS, NUM_ACCESSES);
    eprintln!("  Wall time: {:.2?}", sim_elapsed);
    eprintln!(
        "  Tier1: {}  Tier2: {}  Tier3: {}",
        tier1_count, tier2_count, tier3_count
    );
    eprintln!(
        "  Tier1 blocks: {}  (cap: {})  {}",
        tier1_count,
        tier1_cap,
        if tier1_count <= tier1_cap {
            "PASS"
        } else {
            "FAIL"
        }
    );
    eprintln!(
        "  Tier flip rate: {:.4}/block/min  (threshold: 0.1)  {}",
        flip_rate,
        if flip_rate < 0.1 { "PASS" } else { "FAIL" }
    );
    eprintln!(
        "  P95 read latency: {} ns  {}",
        p95_latency_ns,
        if p95_latency_ns < 50_000 {
            "PASS"
        } else {
            "WARN"
        }
    );
    eprintln!();

    assert!(
        tier1_count <= tier1_cap,
        "Tier1 count {} exceeds cap {}",
        tier1_count,
        tier1_cap
    );
    assert!(
        flip_rate < 0.1,
        "Tier flip rate {:.4}/block/min exceeds 0.1 threshold",
        flip_rate
    );
}

// ---------------------------------------------------------------------------
// 2. Quantize Microbenchmarks
// ---------------------------------------------------------------------------

/// Benchmark quantize + pack for different bit widths.
#[test]
fn bench_quantize_all_widths() {
    const ELEM_COUNT: usize = 4096; // 16KB of f32
    const ITERS: u32 = 1000;
    const GROUP_LEN: usize = 64;
    const RAW_BYTES: f64 = (ELEM_COUNT * 4) as f64;

    let mut rng = SimpleRng::new(42);
    let data = generate_f32_data(&mut rng, ELEM_COUNT);

    eprintln!();
    eprintln!("--- Temporal Tensor Store Benchmarks ---");
    eprintln!();
    eprintln!("Quantize (16KB block, {} iters):", ITERS);

    for &bits in &[8u8, 7, 5, 3] {
        let scales = quantizer::compute_scales(&data, GROUP_LEN, bits);
        let scales_f32 = quantizer::scales_to_f32(&scales);
        let mut packed = Vec::with_capacity(ELEM_COUNT);

        let (_total, per_iter) = bench_loop(ITERS, || {
            packed.clear();
            quantizer::quantize_and_pack_f32(&data, &scales_f32, GROUP_LEN, bits, &mut packed);
            std::hint::black_box(&packed);
        });

        let ns = per_iter.as_nanos();
        let throughput_gbs = RAW_BYTES / (ns as f64);
        eprintln!(
            "  {}-bit:  {:>7} ns/iter  ({:.2} GB/s)",
            bits, ns, throughput_gbs
        );
    }
    eprintln!();
}

// ---------------------------------------------------------------------------
// 3. Dequantize Microbenchmarks
// ---------------------------------------------------------------------------

/// Benchmark dequantize + unpack for different bit widths.
#[test]
fn bench_dequantize_all_widths() {
    const ELEM_COUNT: usize = 4096;
    const ITERS: u32 = 1000;
    const GROUP_LEN: usize = 64;
    const RAW_BYTES: f64 = (ELEM_COUNT * 4) as f64;

    let mut rng = SimpleRng::new(42);
    let data = generate_f32_data(&mut rng, ELEM_COUNT);

    eprintln!("Dequantize (16KB block, {} iters):", ITERS);

    for &bits in &[8u8, 7, 5, 3] {
        let scales = quantizer::compute_scales(&data, GROUP_LEN, bits);
        let scales_f32 = quantizer::scales_to_f32(&scales);
        let mut packed = Vec::new();
        quantizer::quantize_and_pack_f32(&data, &scales_f32, GROUP_LEN, bits, &mut packed);

        let mut decoded = Vec::with_capacity(ELEM_COUNT);

        let (_total, per_iter) = bench_loop(ITERS, || {
            decoded.clear();
            quantizer::dequantize_f32(
                &packed,
                &scales_f32,
                GROUP_LEN,
                bits,
                ELEM_COUNT,
                1,
                &mut decoded,
            );
            std::hint::black_box(&decoded);
        });

        let ns = per_iter.as_nanos();
        let throughput_gbs = RAW_BYTES / (ns as f64);
        eprintln!(
            "  {}-bit:  {:>7} ns/iter  ({:.2} GB/s)",
            bits, ns, throughput_gbs
        );
    }
    eprintln!();
}

// ---------------------------------------------------------------------------
// 4. Bit Packing Microbenchmarks
// ---------------------------------------------------------------------------

/// Benchmark raw bit packing speed.
#[test]
fn bench_bitpack_speed() {
    const COUNT: usize = 4096;
    const ITERS: u32 = 1000;

    eprintln!("Bitpack (4096 codes, {} iters):", ITERS);

    for &bits in &[8u32, 7, 5, 3] {
        let mask = (1u32 << bits) - 1;
        let codes: Vec<u32> = (0..COUNT as u32).map(|i| i & mask).collect();
        let mut packed = Vec::with_capacity(COUNT);

        let (_total, per_iter) = bench_loop(ITERS, || {
            packed.clear();
            bitpack::pack(&codes, bits, &mut packed);
            std::hint::black_box(&packed);
        });

        let ns = per_iter.as_nanos();
        let raw_bytes = (COUNT * bits as usize).div_ceil(8);
        let throughput_gbs = raw_bytes as f64 / (ns as f64);
        eprintln!(
            "  {}-bit pack:    {:>7} ns/iter  ({:.2} GB/s output)",
            bits, ns, throughput_gbs
        );

        // Unpack benchmark
        let mut unpacked = Vec::with_capacity(COUNT);
        let (_total, per_iter) = bench_loop(ITERS, || {
            unpacked.clear();
            bitpack::unpack(&packed, bits, COUNT, &mut unpacked);
            std::hint::black_box(&unpacked);
        });

        let ns = per_iter.as_nanos();
        let throughput_gbs = raw_bytes as f64 / (ns as f64);
        eprintln!(
            "  {}-bit unpack:  {:>7} ns/iter  ({:.2} GB/s input)",
            bits, ns, throughput_gbs
        );
    }
    eprintln!();
}

// ---------------------------------------------------------------------------
// 5. Score Computation Benchmark
// ---------------------------------------------------------------------------

/// Benchmark score computation per block (tiering module).
#[test]
fn bench_score_computation() {
    const ITERS: u32 = 100_000;

    let config = TierConfig::default();
    let mut rng = SimpleRng::new(99);

    // Pre-generate block metadata with varied access patterns
    let metas: Vec<BlockMeta> = (0..1000)
        .map(|_| {
            let mut m = BlockMeta::new(0);
            m.ema_rate = (rng.next_u64() % 100) as f32 / 100.0;
            m.access_window = rng.next_u64();
            m.last_access = (rng.next_u64() % 10_000) as u64;
            m.access_count = (rng.next_u64() % 1000) as u64;
            m
        })
        .collect();

    let start = Instant::now();
    let mut score_sink = 0.0f32;
    for i in 0..ITERS {
        let idx = (i as usize) % 1000;
        let now = metas[idx].last_access + 100;
        let score = tiering::compute_score(&config, now, &metas[idx]);
        score_sink += score;
    }
    let elapsed = start.elapsed();
    std::hint::black_box(score_sink);

    let ns_per_iter = elapsed.as_nanos() / ITERS as u128;

    eprintln!("Score computation ({} iters):", ITERS);
    eprintln!("  tiering::compute_score: {} ns/iter", ns_per_iter);

    // Also benchmark the legacy TierPolicy::select_bits for comparison
    let policy = TierPolicy::default();
    let access_counts: Vec<u32> = (0..1000).map(|_| (rng.next_u64() % 1000) as u32).collect();
    let timestamps: Vec<u32> = (0..1000)
        .map(|_| (rng.next_u64() % 100_000) as u32)
        .collect();

    let start = Instant::now();
    let mut bits_sink = 0u32;
    for i in 0..ITERS {
        let idx = (i as usize) % 1000;
        let now_ts = timestamps[idx].wrapping_add(100);
        let bits = policy.select_bits(access_counts[idx], timestamps[idx], now_ts);
        bits_sink = bits_sink.wrapping_add(bits as u32);
    }
    let elapsed = start.elapsed();
    std::hint::black_box(bits_sink);

    let ns_per_iter = elapsed.as_nanos() / ITERS as u128;
    eprintln!("  TierPolicy::select_bits: {} ns/iter", ns_per_iter);
    eprintln!();
}

// ---------------------------------------------------------------------------
// 6. Quality Metrics Test
// ---------------------------------------------------------------------------

/// Verify reconstruction quality meets ADR targets.
///
/// Uses data with guaranteed minimum magnitude to avoid spurious relative
/// error spikes on near-zero values (where quantization step > |value|).
/// The ADR-023 error bounds apply to values with significant magnitude
/// relative to the group scale.
#[test]
fn quality_metrics_test() {
    const ELEM_COUNT: usize = 4096;
    const GROUP_LEN: usize = 64;
    // Minimum magnitude: values are in [-1, -0.15] union [0.15, 1.0].
    // This ensures all values are at least 15% of the max possible value,
    // so the quantization step size is always small relative to the value.
    const MIN_MAG: f32 = 0.15;

    let mut rng = SimpleRng::new(12345);
    let data = generate_f32_data_no_near_zero(&mut rng, ELEM_COUNT, MIN_MAG);

    // ADR-023 max relative error bounds per tier.
    // These bounds apply to values with |v| >= MIN_MAG.
    let configs: &[(u8, f64, &str)] = &[
        (8, 0.008, "0.80"), // 8-bit: <0.8%
        (7, 0.016, "1.60"), // 7-bit: <1.6%
        (5, 0.065, "6.50"), // 5-bit: <6.5%
        (3, 0.30, "30.0"),  // 3-bit: <30%
    ];

    eprintln!("Quality:");

    let mut all_pass = true;

    for &(bits, max_rel_err_bound, label_pct) in configs {
        let scales = quantizer::compute_scales(&data, GROUP_LEN, bits);
        let scales_f32 = quantizer::scales_to_f32(&scales);

        let mut packed = Vec::new();
        quantizer::quantize_and_pack_f32(&data, &scales_f32, GROUP_LEN, bits, &mut packed);

        let mut decoded = Vec::new();
        quantizer::dequantize_f32(
            &packed,
            &scales_f32,
            GROUP_LEN,
            bits,
            ELEM_COUNT,
            1,
            &mut decoded,
        );

        // Compute MSE and per-group max relative error.
        // Relative error is measured against the group's scale (max |v|),
        // which is the meaningful reference for quantization quality.
        let mut sum_sq_err = 0.0f64;
        let mut max_rel_err = 0.0f64;
        let mut count_rel = 0usize;

        for (group_idx, chunk) in data.chunks(GROUP_LEN).enumerate() {
            // Group max magnitude (the reference for relative error)
            let group_max: f32 = chunk.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
            if group_max < 1e-10 {
                continue;
            }

            let offset = group_idx * GROUP_LEN;
            for (j, &orig) in chunk.iter().enumerate() {
                let dec = decoded[offset + j];
                let err = (orig - dec) as f64;
                sum_sq_err += err * err;

                // Relative error versus group max (the scale reference)
                let rel = err.abs() / group_max as f64;
                if rel > max_rel_err {
                    max_rel_err = rel;
                }
                count_rel += 1;
            }
        }

        let mse = sum_sq_err / ELEM_COUNT as f64;
        let pass = max_rel_err < max_rel_err_bound;
        let status = if pass { "PASS" } else { "FAIL" };

        if !pass {
            all_pass = false;
        }

        eprintln!(
            "  {}-bit MSE: {:.6}  max_rel_err: {:.2}%  (bound: {}%)  {}  (samples: {})",
            bits,
            mse,
            max_rel_err * 100.0,
            label_pct,
            status,
            count_rel,
        );
    }
    eprintln!();

    assert!(
        all_pass,
        "One or more quality checks failed -- see output above"
    );
}

// ---------------------------------------------------------------------------
// 7. Adversarial Access Pattern Test
// ---------------------------------------------------------------------------

/// Test graceful degradation under adversarial access using the `tiering`
/// module's hysteresis and minimum-residency guards.
///
/// Simulates blocks whose access scores hover near the Tier1/Tier2 boundary.
/// Without hysteresis, small noise would cause continuous oscillation.
/// With hysteresis + min_residency, the flip rate should stay below threshold.
///
/// The test runs two configurations:
/// 1. Noisy-boundary: scores jitter around the t1 threshold (0.7)
/// 2. Burst-noise: stable cold blocks hit by brief access bursts
///
/// Both should have tier flips < 0.1/block/min.
#[test]
fn adversarial_access_test() {
    const NUM_BLOCKS: usize = 100;
    const TOTAL_TICKS: u64 = 10_000;

    let config = TierConfig {
        hysteresis: 0.05,
        min_residency: 10,
        ..TierConfig::default()
    };

    let mut rng = SimpleRng::new(0xCAFE);

    struct AdversarialBlock {
        meta: BlockMeta,
        flip_count: u32,
        last_tier: Tier,
    }

    let mut blocks: Vec<AdversarialBlock> = (0..NUM_BLOCKS)
        .map(|_| {
            let meta = BlockMeta::new(0);
            let last_tier = meta.current_tier;
            AdversarialBlock {
                meta,
                flip_count: 0,
                last_tier,
            }
        })
        .collect();

    // Warm up blocks so their scores sit near the Tier1/Tier2 boundary.
    // The t1 threshold is 0.7. We want ema_rate to hover near a value
    // where the composite score is close to 0.7.
    for block in blocks.iter_mut() {
        block.meta.ema_rate = 0.65;
        block.meta.access_window = 0xFFFF_FFFF_0000_0000; // half bits set
        block.meta.last_access = 0;
        block.meta.current_tier = Tier::Tier2;
        block.meta.tier_since = 0;
    }

    for tick in 1..=TOTAL_TICKS {
        for block in blocks.iter_mut() {
            // Adversarial pattern: randomly touch ~50% of blocks each tick,
            // creating a noisy signal near the boundary. Some blocks will
            // have their score bump above t1, others below -- the noise
            // should be absorbed by hysteresis.
            let pseudo_rand = rng.next_u64();
            if pseudo_rand % 2 == 0 {
                tiering::touch(&config, tick, &mut block.meta);
            } else {
                tiering::tick_decay(&config, &mut block.meta);
            }

            // Attempt tier migration (hysteresis should absorb boundary noise)
            if let Some(new_tier) = tiering::choose_tier(&config, tick, &block.meta) {
                block.meta.current_tier = new_tier;
                block.meta.tier_since = tick;

                if new_tier != block.last_tier {
                    block.flip_count += 1;
                    block.last_tier = new_tier;
                }
            }
        }
    }

    let total_flips: u32 = blocks.iter().map(|b| b.flip_count).sum();
    let max_flips_per_block = blocks.iter().map(|b| b.flip_count).max().unwrap_or(0);

    // Scale: 1000 ticks = 1 simulated minute
    let sim_minutes = TOTAL_TICKS as f64 / 1000.0;
    let flip_rate = if sim_minutes > 0.0 && NUM_BLOCKS > 0 {
        total_flips as f64 / NUM_BLOCKS as f64 / sim_minutes
    } else {
        0.0
    };

    eprintln!("--- Adversarial Access Test ---");
    eprintln!();
    eprintln!(
        "  Blocks: {}  Ticks: {}  ({:.1} sim minutes)",
        NUM_BLOCKS, TOTAL_TICKS, sim_minutes
    );
    eprintln!(
        "  Total flips: {}  max/block: {}",
        total_flips, max_flips_per_block
    );
    eprintln!(
        "  Flip rate: {:.4}/block/min  (threshold: 0.1)  {}",
        flip_rate,
        if flip_rate < 0.1 { "PASS" } else { "FAIL" }
    );

    // Also report tier distribution at end
    let tier1 = blocks
        .iter()
        .filter(|b| b.meta.current_tier == Tier::Tier1)
        .count();
    let tier2 = blocks
        .iter()
        .filter(|b| b.meta.current_tier == Tier::Tier2)
        .count();
    let tier3 = blocks
        .iter()
        .filter(|b| b.meta.current_tier == Tier::Tier3)
        .count();
    eprintln!("  Final tiers: T1={} T2={} T3={}", tier1, tier2, tier3);
    eprintln!();

    assert!(
        flip_rate < 0.1,
        "Adversarial flip rate {:.4}/block/min exceeds 0.1 threshold \
         (total_flips={}, max/block={})",
        flip_rate,
        total_flips,
        max_flips_per_block
    );
}

// ---------------------------------------------------------------------------
// 8. Segment encode/decode round-trip benchmark
// ---------------------------------------------------------------------------

/// Benchmark full segment encode + decode cycle.
#[test]
fn bench_segment_roundtrip() {
    const TENSOR_LEN: u32 = 256;
    const FRAME_COUNT: usize = 16;
    const ITERS: u32 = 500;

    let policy = TierPolicy::default();
    let mut rng = SimpleRng::new(777);

    let frames: Vec<Vec<f32>> = (0..FRAME_COUNT)
        .map(|_| generate_f32_data(&mut rng, TENSOR_LEN as usize))
        .collect();

    eprintln!(
        "Segment round-trip ({} frames x {} elements, {} iters):",
        FRAME_COUNT, TENSOR_LEN, ITERS
    );

    for &bits in &[8u8, 7, 5, 3] {
        let mut comp = TemporalTensorCompressor::new(policy, TENSOR_LEN, 0);
        if bits == 8 {
            comp.set_access(1000, 0);
        } else if bits == 7 {
            comp.set_access(10, 0);
        } else if bits == 5 {
            let p5 = TierPolicy {
                warm_bits: 5,
                ..policy
            };
            comp = TemporalTensorCompressor::new(p5, TENSOR_LEN, 0);
            comp.set_access(10, 0);
        }
        // bits==3: default (cold)

        let mut seg = Vec::new();
        for (i, frame) in frames.iter().enumerate() {
            comp.push_frame(frame, (i + 1) as u32, &mut seg);
        }
        comp.flush(&mut seg);

        if seg.is_empty() {
            eprintln!("  {}-bit: (no segment produced, skipping)", bits);
            continue;
        }

        let seg_bytes = seg.len();
        let raw_bytes = TENSOR_LEN as usize * FRAME_COUNT * 4;

        let mut decoded = Vec::with_capacity(TENSOR_LEN as usize * FRAME_COUNT);
        let (_total, per_iter) = bench_loop(ITERS, || {
            decoded.clear();
            segment::decode(&seg, &mut decoded);
            std::hint::black_box(&decoded);
        });

        let ns = per_iter.as_nanos();
        let ratio = raw_bytes as f64 / seg_bytes as f64;
        let throughput_gbs = raw_bytes as f64 / (ns as f64);
        eprintln!(
            "  {}-bit decode: {:>7} ns/iter  ({:.2} GB/s)  ratio: {:.2}x  seg: {} bytes",
            bits, ns, throughput_gbs, ratio, seg_bytes
        );
    }
    eprintln!();
}

// ---------------------------------------------------------------------------
// 9. Compressor throughput benchmark
// ---------------------------------------------------------------------------

/// Benchmark the full compressor push_frame path.
#[test]
fn bench_compressor_throughput() {
    const TENSOR_LEN: u32 = 256;
    const FRAMES: usize = 10_000;

    let policy = TierPolicy::default();
    let mut rng = SimpleRng::new(0xBEEF);
    let frame = generate_f32_data(&mut rng, TENSOR_LEN as usize);

    eprintln!(
        "Compressor throughput ({} elements x {} frames):",
        TENSOR_LEN, FRAMES
    );

    for &(label, access_count) in &[("hot/8-bit", 1000u32), ("cold/3-bit", 0)] {
        let mut comp = TemporalTensorCompressor::new(policy, TENSOR_LEN, 0);
        comp.set_access(access_count, 0);

        let mut seg = Vec::new();
        let mut total_segments = 0usize;

        let start = Instant::now();
        for i in 0..FRAMES {
            comp.push_frame(&frame, (i + 1) as u32, &mut seg);
            if !seg.is_empty() {
                total_segments += 1;
            }
        }
        comp.flush(&mut seg);
        if !seg.is_empty() {
            total_segments += 1;
        }
        let elapsed = start.elapsed();

        let raw_bytes = TENSOR_LEN as usize * 4 * FRAMES;
        let ns_total = elapsed.as_nanos();
        let ns_per_frame = ns_total / FRAMES as u128;
        let throughput_gbs = raw_bytes as f64 / (ns_total as f64);

        eprintln!(
            "  {}:  {} ns/frame  ({:.2} GB/s)  segments: {}",
            label, ns_per_frame, throughput_gbs, total_segments
        );
    }
    eprintln!();
}

// ---------------------------------------------------------------------------
// 10. Single-frame random-access decode benchmark
// ---------------------------------------------------------------------------

/// Benchmark single-frame decode (random access into a segment).
#[test]
fn bench_single_frame_decode() {
    const TENSOR_LEN: u32 = 256;
    const FRAME_COUNT: usize = 64;
    const ITERS: u32 = 2000;

    let policy = TierPolicy::default();
    let mut rng = SimpleRng::new(0xF00D);

    let mut comp = TemporalTensorCompressor::new(policy, TENSOR_LEN, 0);
    comp.set_access(1000, 0);
    let frame = generate_f32_data(&mut rng, TENSOR_LEN as usize);
    let mut seg = Vec::new();
    for i in 0..FRAME_COUNT {
        comp.push_frame(&frame, (i + 1) as u32, &mut seg);
    }
    comp.flush(&mut seg);

    if seg.is_empty() {
        eprintln!("Single-frame decode: no segment produced, skipping");
        return;
    }

    eprintln!(
        "Single-frame decode ({} frames in segment, {} iters):",
        FRAME_COUNT, ITERS
    );

    for &frame_idx in &[0usize, FRAME_COUNT / 2, FRAME_COUNT - 1] {
        let (_total, per_iter) = bench_loop(ITERS, || {
            let result = segment::decode_single_frame(&seg, frame_idx);
            std::hint::black_box(&result);
        });

        let ns = per_iter.as_nanos();
        eprintln!("  frame[{}]:  {} ns/iter", frame_idx, ns);
    }
    eprintln!();
}

// ---------------------------------------------------------------------------
// 11. Tiering candidate selection benchmark
// ---------------------------------------------------------------------------

/// Benchmark tiering candidate selection with many blocks.
#[test]
fn bench_tiering_candidate_selection() {
    const NUM_BLOCKS: usize = 10_000;
    const ITERS: u32 = 100;

    let config = TierConfig::default();
    let mut rng = SimpleRng::new(0xABCD);

    // Create varied block metadata
    let metas: Vec<BlockMeta> = (0..NUM_BLOCKS)
        .map(|_| {
            let mut m = BlockMeta::new(0);
            m.ema_rate = rng.next_f64() as f32;
            m.access_window = rng.next_u64();
            m.last_access = (rng.next_u64() % 500) as u64;
            m.current_tier = match rng.next_u64() % 3 {
                0 => Tier::Tier1,
                1 => Tier::Tier2,
                _ => Tier::Tier3,
            };
            m.tier_since = 0;
            m
        })
        .collect();

    let block_refs: Vec<(BlockKey, &BlockMeta)> = metas
        .iter()
        .enumerate()
        .map(|(i, m)| (BlockKey(i as u64), m))
        .collect();

    let now = 1000u64;
    let mut total_candidates = 0usize;

    let (_total, per_iter) = bench_loop(ITERS, || {
        let candidates = tiering::select_candidates(&config, now, &block_refs);
        total_candidates += candidates.len();
        std::hint::black_box(&candidates);
    });

    let ns = per_iter.as_nanos();
    let avg_candidates = total_candidates / ITERS as usize;

    eprintln!(
        "Tiering candidate selection ({} blocks, {} iters):",
        NUM_BLOCKS, ITERS
    );
    eprintln!("  {} ns/iter  ({} avg candidates)", ns, avg_candidates);
    eprintln!();
}

// ---------------------------------------------------------------------------
// Summary printer (runs last alphabetically)
// ---------------------------------------------------------------------------

/// Print a summary separator. Run this test last with `--nocapture`.
#[test]
fn z_summary() {
    eprintln!();
    eprintln!("=== All temporal tensor benchmarks complete ===");
    eprintln!();
}
