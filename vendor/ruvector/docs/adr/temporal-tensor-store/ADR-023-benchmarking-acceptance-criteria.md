# ADR-023: Benchmarking, Failure Modes, and Acceptance Criteria

**Status**: Proposed
**Date**: 2026-02-08
**Parent**: ADR-017 Temporal Tensor Compression, ADR-018 Block-Based Storage Engine
**Author**: System Architecture Team

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-08 | Architecture Team | Initial proposal |

---

## Abstract

This ADR defines benchmarking methodology, acceptance thresholds, failure modes, and CI strategy for the Temporal Tensor Store. It makes ADR-017's performance targets measurable and enforceable by specifying harnesses, pass/fail criteria, and automated regression detection.

---

## 1. Context

ADR-017 and ADR-018 together form the Temporal Tensor Store but leave gaps in how targets are measured, what happens when they are missed, and how regressions are caught. This ADR closes those gaps with concrete harness designs, a primary acceptance test, five catalogued failure modes with fix paths, and CI integration rules.

---

## 2. Microbenchmark Targets

All measurements use a single 16KB block (4096 f32 values, group_len=64). Harness: Criterion.rs with 200 samples, 5s measurement, 2s warm-up.

### 2.1 Quantize and Dequantize Throughput

| Operation | Bit Width | Native Target | WASM Target |
|-----------|-----------|--------------|-------------|
| Quantize | 8-bit | < 2 us | < 20 us |
| Quantize | 7-bit | < 2 us | < 20 us |
| Quantize | 5-bit | < 2.5 us | < 25 us |
| Quantize | 3-bit | < 3 us | < 30 us |
| Dequantize | 8-bit | < 2 us | < 20 us |
| Dequantize | 7-bit | < 2.5 us | < 25 us |
| Dequantize | 5-bit | < 3 us | < 30 us |
| Dequantize | 3-bit | < 5 us | < 50 us |

### 2.2 Pack and Unpack Speed

| Operation | Bit Width | Native Target | WASM Target |
|-----------|-----------|--------------|-------------|
| Pack 16KB | 8-bit | < 0.5 us | < 5 us |
| Pack 16KB | 7-bit | < 1 us | < 10 us |
| Pack 16KB | 5-bit | < 1 us | < 10 us |
| Pack 16KB | 3-bit | < 1.5 us | < 15 us |
| Unpack 16KB | 8-bit | < 0.5 us | < 5 us |
| Unpack 16KB | 7-bit | < 1 us | < 10 us |
| Unpack 16KB | 5-bit | < 1 us | < 10 us |
| Unpack 16KB | 3-bit | < 1.5 us | < 15 us |

### 2.3 Tier Decision and Scoring

| Operation | Native Target | WASM Target |
|-----------|--------------|-------------|
| Tier decision per block | < 50 ns | < 500 ns |
| Per-block scoring | < 20 ns | < 200 ns |
| Maintenance tick (1000 candidates) | < 1 ms | < 10 ms |
| Delta apply (sparse, 10% nnz) | < 1 us | < 10 us |

### 2.4 Auxiliary Operations

| Operation | Native Target | WASM Target |
|-----------|--------------|-------------|
| f32-to-f16 / f16-to-f32 (single) | < 5 ns | < 50 ns |
| Drift check (64-group block) | < 50 ns | < 500 ns |
| CRC32 checksum (16KB) | < 1 us | < 10 us |
| Segment encode (16KB, 1 frame) | < 3 us | < 30 us |
| Segment decode (16KB, 1 frame) | < 3 us | < 30 us |

---

## 3. Macrobenchmark Targets

### 3.1 KV Cache-Like Workload with Zipf Access Pattern

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Total blocks | 1,000,000 | ~16 GB raw; representative large cache |
| Total accesses | 10,000,000 | Statistical stability |
| Distribution | Zipf (alpha=1.2) | Models real attention-pattern skew |
| Block size | 16 KB | Standard block from ADR-018 |
| Tier-1 byte cap | 2 GB | Memory-constrained deployment |

### 3.2 Measurements

Average read latency, P95 read latency, P99 read latency, bytes stored per token, MSE per tier (sampled from 1000 blocks per tier), tier churn rate (transitions/block/minute), Tier-1 occupancy (snapshotted every simulated second), and eviction count.

### 3.3 Macrobenchmark Acceptance Thresholds

| Metric | Target | Hard Fail |
|--------|--------|-----------|
| Avg read latency (native) | < 3 us | > 10 us |
| P95 read latency (native) | < 10 us | > 50 us |
| P99 read latency (native) | < 25 us | > 100 us |
| Avg read latency (WASM) | < 30 us | > 100 us |
| P95 read latency (WASM) | < 100 us | > 500 us |
| Bytes stored per token | < 2.5 bytes | > 4 bytes |
| Tier churn per block per min | < 0.1 avg | > 0.5 |
| Tier-1 byte usage | Under cap always | Any violation |

---

## 4. Acceptance Thresholds (Critical)

These gate merges to main. Any violation blocks the PR.

### 4.1 Latency

| Metric | Target |
|--------|--------|
| Tier-1 dequant latency (16KB block, native) | < 2 us |
| Tier-3 dequant latency (16KB block, native) | < 5 us |
| WASM dequant latency (16KB block, Node.js) | < 50 us |

**Derivation**: A 16KB block requires 4096 multiplies. On AVX2 at 3.5 GHz (8 f32/cycle), the theoretical floor is ~146 ns. The 2 us target provides 14x headroom for unpacking, memory access, and loop overhead while staying well under the 10 us inference-impact threshold. The WASM 50 us target reflects measured 8-12x V8 overhead plus a 2x safety margin.

### 4.2 Stability

| Metric | Target |
|--------|--------|
| Tier churn per block per min | < 0.1 avg |
| Tier-1 byte budget | Under configured cap |
| Segment boundary rate | < 1 per 100 frames (stable tensor) |

**Derivation**: At 0.1 transitions/block/min with 1M blocks, total transitions are ~1,667/sec. At ~5-10 us each, this consumes <2% CPU. At 1.0/block/min it becomes 8-17%, which is unacceptable.

### 4.3 Quality Thresholds

| Tier | Bits | Max MSE (normalized) | Max Relative Error |
|------|------|---------------------|-------------------|
| Hot (8-bit) | 8 | < 0.0001 | < 0.8% |
| Warm (7-bit) | 7 | < 0.0004 | < 1.6% |
| Warm (5-bit) | 5 | < 0.004 | < 6.5% |
| Cold (3-bit) | 3 | < 0.03 | < 30% |

MSE normalized by squared L2-norm of original block. Relative error is max element-wise error divided by block max absolute value.

---

## 5. Primary Acceptance Test

### 5.1 Configuration

```
blocks: 1,000,000    accesses: 10,000,000   distribution: Zipf(1.2)
tier1_byte_cap: 2GB  block_size: 16KB       group_len: 64
hot_min_score: 512   warm_min_score: 64     hysteresis: 32
min_residency: 60    drift_pct_q8: 26       max_delta_chain: 8
```

### 5.2 Pass Criteria

The simulation PASSES if and only if all three hold simultaneously:
1. **Budget**: Tier-1 holds under configured byte cap at every epoch snapshot.
2. **Stability**: Average tier flips per block per minute < 0.1.
3. **Latency**: P95 read latency stays within tier target on host.

### 5.3 Zipf Simulation Pseudocode

```
function run_zipf_simulation(config):
    store = BlockStore::new(config.tier1_byte_cap)
    blocks = Array[config.num_blocks]
    for i in 0..config.num_blocks:
        blocks[i] = generate_random_f32_block(config.block_size)
        store.ingest(block_id=i, data=blocks[i], initial_tier=COLD)

    zipf = ZipfDistribution::new(config.num_blocks, config.alpha)
    rng = StableRng::seed(42)

    latencies = Vec::new()
    tier_flips = Array[config.num_blocks].fill(0)
    prev_tier = Array[config.num_blocks].fill(COLD)
    epoch_snapshots = Vec::new()
    sim_clock = 0

    for access in 0..config.num_accesses:
        block_id = zipf.sample(rng)
        sim_clock += 1

        t_start = precise_now()
        tier = store.current_tier(block_id)
        data = store.read_block(block_id, sim_clock)
        t_end = precise_now()
        latencies.push(t_end - t_start)

        if tier != prev_tier[block_id]:
            tier_flips[block_id] += 1
            prev_tier[block_id] = tier

        if access % config.maintenance_interval == 0:
            store.run_maintenance_tick(sim_clock)
        if access % config.snapshot_interval == 0:
            epoch_snapshots.push(EpochSnapshot {
                sim_clock, tier1_bytes: store.tier1_bytes(),
                tier2_bytes: store.tier2_bytes(),
                tier3_bytes: store.tier3_bytes(),
            })

    sim_minutes = sim_clock / config.ticks_per_minute
    results = SimulationResults {
        avg_latency:     mean(latencies),
        p95_latency:     percentile(latencies, 0.95),
        p99_latency:     percentile(latencies, 0.99),
        avg_churn:       mean(tier_flips) / sim_minutes,
        budget_violated: any(s.tier1_bytes > config.tier1_byte_cap for s in epoch_snapshots),
    }

    // Quality sampling: 1000 blocks per tier
    for tier in [HOT, WARM, COLD]:
        for id in store.sample_block_ids(tier, 1000):
            reconstructed = store.read_block(id, sim_clock)
            results.quality[tier].push(mse(blocks[id], reconstructed))
    return results

function assert_pass(results, config):
    assert !results.budget_violated           // Criterion 1
    assert results.avg_churn < 0.1            // Criterion 2
    assert results.p95_latency < config.p95   // Criterion 3
    for tier, samples in results.quality:
        for mse in samples:
            assert mse < config.mse_threshold[tier]  // Criterion 4
```

### 5.4 Reproducibility

Fixed RNG seed (42), Zipf-Mandelbrot inverse CDF, monotonic clock (`Instant::now()`), CPU frequency scaling disabled or handled by Criterion warm-up.

---

## 6. Failure Modes and Fix Paths

### 6.1 Thrashing

- **Symptom**: Tier flips > 0.1/block/min; excessive segment boundaries
- **Root cause**: Hysteresis too small; tau too large causing score oscillation
- **Fix**: Increase hysteresis (32 to 64+), increase min_residency (60 to 120+ ticks), reduce tau

### 6.2 Delta Chain Blowup

- **Symptom**: P95 read latency > 10x tier target; growing read amplification
- **Root cause**: Delta chains not compacted; unbounded chain growth
- **Fix**: Compact when chain exceeds max_delta_chain (default 8); schedule in maintenance tick; hard cap forces sync compaction on read at 2x max

### 6.3 Scale Instability

- **Symptom**: MSE exceeds threshold on bimodal/heavy-tailed tensors
- **Root cause**: Single per-group scale insufficient for outlier distributions
- **Fix**: Enable two-level scale for 3-bit; reduce group_len to 32 for affected blocks; clamp outliers at 3-sigma with sparse correction side-channel

### 6.4 Hot Set Misprediction

- **Symptom**: Tier-1 byte usage exceeds configured cap
- **Root cause**: Scoring promotes too many blocks; hot_min_score too low
- **Fix**: Raise t1_threshold, lower w_pop, enforce per-tier byte cap with LRU eviction, add feedback loop (auto-raise threshold when eviction rate exceeds N/sec)

### 6.5 Checksum Corruption

- **Symptom**: CRC32 mismatch on read
- **Root cause**: Bit flip in storage; partial write; pack/unpack bug
- **Fix**: Rehydrate from delta chain if available; attempt factor decomposition recovery; else mark Unrecoverable and emit alert metric; enable background scrubbing on idle blocks

---

## 7. Benchmark Harness Design

### 7.1 Microbenchmarks (Criterion.rs)

```
crates/ruvector-temporal-tensor/benches/
    quantize.rs       -- per bit width
    dequantize.rs     -- per bit width
    bitpack.rs        -- pack/unpack per bit width
    tier_policy.rs    -- scoring and tier decision
    f16_conversion.rs -- f32<->f16
    segment.rs        -- encode/decode round-trip
    maintenance.rs    -- maintenance tick with N candidates
```

Input data: fixed seed (42), standard normal scaled to [-1.0, 1.0]. Median is the primary statistic. Regression detected when new CI lower bound exceeds baseline upper bound by >5%.

### 7.2 Zipf Simulation (Custom Rust)

Located at `crates/ruvector-temporal-tensor/tests/zipf_simulation.rs`. Supports `--quick` (100K blocks, 1M accesses, ~30s) for PR checks and `--full` (1M blocks, 10M accesses, ~5-10min) for nightly. Outputs JSON for CI and human-readable summary to stdout. Configurable via env vars (`ZIPF_BLOCKS`, `ZIPF_ACCESSES`, `ZIPF_ALPHA`).

### 7.3 WASM Benchmarks

Built with `wasm-pack build --release --target nodejs`. Node.js runner calls each FFI function in a 10,000-iteration loop, measured with `process.hrtime.bigint()`. Reports median, P95, P99 and computes WASM/native overhead ratio.

---

## 8. CI Integration Guidelines

### 8.1 Pipeline Stages

| Stage | Trigger | Timeout | Scope |
|-------|---------|---------|-------|
| PR check | Every PR | 10 min | Criterion quick, Zipf quick, quality |
| Nightly | 02:00 UTC | 30 min | Full Criterion, Zipf full, WASM, quality sweep |
| Release gate | Tag push | 45 min | All benchmarks, cross-platform, WASM + native |

### 8.2 Regression Detection

```yaml
benchmark-check:
  steps:
    - run: cargo bench --bench '*' -- --output-format bencher | tee output.txt
    - run: python scripts/bench_compare.py --baseline .bench_baseline.json
           --current output.txt --threshold 0.10 --fail-on-regression
    - run: cargo test --release --test zipf_simulation -- --quick
```

Baselines committed as `.bench_baseline.json` on main. Updated only on architecture-team-reviewed PRs that modify quantization or storage code. Comparison: `(new_median - baseline) / baseline`; fail at 10% for latency, 20% for throughput.

### 8.3 Alerting

| Condition | Action |
|-----------|--------|
| PR regression > 10% | Block merge; PR comment |
| Nightly regression > 15% | GitHub issue: `perf-regression` |
| Zipf simulation failure | GitHub issue: `acceptance-failure` |
| WASM overhead > 15x native | GitHub issue: `wasm-performance` |
| Quality violation | Block merge/release |

---

## 9. SOTA Integration Benchmarks

### 9.1 Reference Systems

| System | Year | Key Result |
|--------|------|-----------|
| **RIPPLE++** | 2026 | Tens of thousands of updates/sec, sub-ms latency for incremental graph computation |
| **OMEGA** | 2025 | Sub-ms GNN inference via selective recompute |
| **STAG** | 2025 | Additivity-based incremental propagation; linear scaling with delta size |

### 9.2 Comparison

| Metric | Temporal Tensor Store | RIPPLE++ | OMEGA | STAG |
|--------|----------------------|----------|-------|------|
| Single read | < 2-5 us | N/A (graph) | ~100 us | ~50 us |
| Batch update (1000) | < 1 ms | ~10 ms | ~5 ms | ~2 ms |
| Memory/element | 0.375-1.0 B | 8 B | 4-8 B | 4 B |

The store targets block-level compression rather than graph-level computation but shares the sub-millisecond incremental update goal. The maintenance tick budget (<1ms for 1000 candidates) is competitive.

---

## 10. Test Scenarios

### 10.1 Scenario Matrix

| ID | Purpose | Blocks | Accesses | Distribution |
|----|---------|--------|----------|-------------|
| S1 | Baseline: uniform access | 10K | 1M | Uniform |
| S2 | Primary acceptance (Zipf) | 1M | 10M | Zipf(1.2) |
| S3 | High skew stress | 1M | 10M | Zipf(2.0) |
| S4 | Temporal shift (rotating hot set) | 100K | 5M | Rotating Zipf |
| S5 | Burst access pattern | 100K | 2M | Burst + uniform |
| S6 | Severe memory constraint (100MB cap) | 1M | 10M | Zipf(1.2) |
| S7 | Outlier/bimodal tensors | 10K | 500K | Zipf(1.2) |
| S8 | Stable tensors (near-zero drift) | 10K | 500K | Zipf(1.2) |

### 10.2 Per-Scenario Pass Criteria

| ID | Pass Condition |
|----|---------------|
| S1 | All blocks converge to same tier within 2x access count |
| S2 | Full acceptance test (Section 5.2) |
| S3 | Tier-1 < 5% of blocks; no budget violation |
| S4 | Churn < 0.2/block/min despite rotation |
| S5 | P95 spike during burst < 2x steady-state P95 |
| S6 | Zero OOM; cap held; avg latency < 5x unconstrained |
| S7 | MSE for bimodal blocks < 2x threshold |
| S8 | Segment count per block < 1.1 |

---

## 11. Risks and Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| CI noise causes false regressions | Medium | 2% Criterion noise threshold; require 3 consecutive failures; pin CI hardware |
| Zipf simulation too slow for PR | Medium | Quick mode (~30s); full mode nightly only |
| WASM results platform-dependent | Low | Pin Node.js version; accept 20% variance |
| Baseline drift over time | Medium | Rebaseline quarterly or on hardware change |

---

## 12. Implementation Roadmap

**Phase 1 (Week 1)**: Criterion benchmarks for all Section 2 operations; initial baselines; `bench_compare.py` script; PR pipeline integration.

**Phase 2 (Week 1-2)**: Zipf simulation with quick/full modes and JSON output; nightly pipeline integration.

**Phase 3 (Week 2)**: WASM Node.js benchmark runner; WASM-specific baselines; nightly pipeline.

**Phase 4 (Week 2-3)**: Failure mode detectors (thrashing counter, delta chain monitor, quality sampler, corruption injection test); wire into simulation harness.

**Phase 5 (Week 3)**: CI hardening (pinned hardware, nightly scheduling, alerting, release-gate workflow).

---

## 13. References

1. Frantar et al. "GPTQ: Accurate Post-Training Quantization." ICLR 2023.
2. Lin et al. "AWQ: Activation-aware Weight Quantization." MLSys 2024.
3. Criterion.rs documentation. https://bheisler.github.io/criterion.rs/
4. Gray. "The Benchmark Handbook." Morgan Kaufmann, 1993.
5. Pelkonen et al. "Gorilla: In-Memory Time Series Database." VLDB 2015.
6. Li et al. "RIPPLE++: Incremental Graph Computation." SIGMOD 2026.
7. Chen et al. "OMEGA: Selective Recompute for Low-Latency GNN Serving." OSDI 2025.
8. Wang et al. "STAG: Additivity-Based Incremental Graph Propagation." VLDB 2025.
9. ADR-017: Temporal Tensor Compression. RuVector Architecture Team, 2026.
10. ADR-018: Block-Based Storage Engine. RuVector Architecture Team, 2026.
