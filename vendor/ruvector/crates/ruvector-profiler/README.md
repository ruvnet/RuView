# ruvector-profiler

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**Memory, power, and latency profiling hooks with CSV emitters — the observability layer for attention benchmarking.**

| Dimension | What It Measures | Output |
|-----------|-----------------|--------|
| **Memory** | RSS, KV-cache, activations, temp buffers | `MemoryReport` + CSV |
| **Power** | Wattage samples, trapezoidal energy integration | `EnergyResult` + CSV |
| **Latency** | p50/p95/p99, mean, std | `LatencyStats` + CSV |
| **Config** | SHA-256 fingerprint of all parameters | Reproducibility hash |

## Overview

This crate instruments benchmark runs with three profiling dimensions -- memory
pressure, energy consumption, and latency distribution -- and exports results to
CSV files for downstream analysis. It is the observability layer in the ruvector
attention benchmarking pipeline, sitting between the attention operators
(`ruvector-attn-mincut`) and the analysis/plotting stage.

Every benchmark run is tagged with a SHA-256 config fingerprint so that results
are reproducible and auditable across machines.

## Modules

| Module | Purpose |
|--------|---------|
| `memory` | `MemoryTracker` with RSS snapshots and peak tracking |
| `power` | `PowerTracker` with `PowerSource` trait and trapezoidal integration |
| `latency` | `LatencyStats` computing p50/p95/p99 from `LatencyRecord` samples |
| `csv_emitter` | `write_results_csv`, `write_latency_csv`, `write_memory_csv` |
| `config_hash` | `BenchConfig` with SHA-256 fingerprinting for reproducibility |

## Usage Example: Full Benchmark Loop

```rust
use ruvector_profiler::*;

// Tag this run with a reproducible fingerprint
let config = BenchConfig {
    model_commit: "abc1234".into(),
    weights_hash: "def5678".into(),
    lambda: 0.5, tau: 2, eps: 0.01,
    compiler_flags: "-O3".into(),
};
println!("Run fingerprint: {}", config_hash(&config));

// Set up trackers
let mut mem = MemoryTracker::new("mincut_l0.5_t2");
let source = MockPowerSource { watts: 75.0 };
let mut pwr = PowerTracker::new("gpu");
let mut latencies = Vec::new();

for i in 0..1000 {
    mem.snapshot();
    pwr.sample(&source);
    let start = std::time::Instant::now();

    // ... run attention operator ...

    let elapsed = start.elapsed().as_micros() as u64;
    latencies.push(LatencyRecord {
        sample_id: i, wall_time_us: elapsed,
        kernel_time_us: elapsed, seq_len: 128,
    });
}

// Aggregate
let stats = compute_latency_stats(&latencies);
let report = mem.report();
let energy = pwr.energy();

println!("Peak RSS: {} bytes | p95: {} us | Energy: {:.3} J",
    report.peak_rss, stats.p95_us, energy.total_joules);

// Export to CSV
write_latency_csv("results/latency.csv", &latencies).unwrap();
write_memory_csv("results/memory.csv", &mem.snapshots).unwrap();
```

## Memory Profiling

`MemoryTracker` captures RSS snapshots via `/proc/self/status` on Linux (zero
fallback on other platforms). Each `MemorySnapshot` records:

| Field | Description |
|-------|-------------|
| `peak_rss_bytes` | Resident set size at capture time |
| `kv_cache_bytes` | Estimated KV-cache allocation |
| `activation_bytes` | Activation tensor memory |
| `temp_buffer_bytes` | Temporary working buffers |
| `timestamp_us` | Microsecond UNIX timestamp |

`MemoryTracker::report()` aggregates snapshots into a `MemoryReport` with
`peak_rss`, `mean_rss`, `kv_cache_total`, and `activation_total`.

## Power Profiling

`PowerTracker` collects wattage readings from any `PowerSource` implementation.
Energy is computed via trapezoidal integration over the sample timeline, yielding
an `EnergyResult` with `total_joules`, `mean_watts`, `peak_watts`, and
`duration_s`. A `MockPowerSource` is provided for deterministic tests.

```rust
use ruvector_profiler::PowerSource;

struct NvmlPowerSource { /* device handle */ }
impl PowerSource for NvmlPowerSource {
    fn read_watts(&self) -> f64 { todo!("read from NVML/RAPL") }
}
```

## Latency Profiling

`compute_latency_stats` takes a slice of `LatencyRecord` and returns
`LatencyStats` with `p50_us`, `p95_us`, `p99_us`, `mean_us`, `std_us`, and
sample count `n`. Records need not be pre-sorted.

## CSV Output Formats

### write_results_csv -- Aggregate summary

```csv
setting,coherence_delta,kv_cache_reduction,peak_mem_reduction,energy_reduction,p95_latency_us,accuracy
mincut_l0.5_t2,-0.003,0.25,0.18,0.12,1150,0.994
```

### write_latency_csv -- Per-sample latency

```csv
sample_id,wall_time_us,kernel_time_us,seq_len
0,850,780,128
```

### write_memory_csv -- Per-snapshot memory

```csv
timestamp_us,peak_rss_bytes,kv_cache_bytes,activation_bytes,temp_buffer_bytes
1700000000,4194304,1048576,2097152,524288
```

## Config Fingerprinting

`BenchConfig` captures all parameters defining a benchmark run. `config_hash`
produces a 64-character SHA-256 hex digest of the JSON-serialized config.

```rust
use ruvector_profiler::{BenchConfig, config_hash};

let config = BenchConfig {
    model_commit: "abc1234".into(), weights_hash: "def5678".into(),
    lambda: 0.5, tau: 2, eps: 0.01, compiler_flags: "-O3".into(),
};
assert_eq!(config_hash(&config).len(), 64);
```

## Integration with run_mincut_bench.sh

The `scripts/run_mincut_bench.sh` script orchestrates a full benchmark run:

```text
run_mincut_bench.sh
  +-- cargo build --release (-p attn-mincut, coherence, profiler)
  +-- Baseline softmax run --> baseline.csv
  +-- Grid search (lambda x tau) --> per-setting CSV + witness JSONL
  +-- Aggregate metrics --> results.csv
  +-- Pack witness bundle --> witness.rvf
```

CSV files follow the schemas above. Use `config_hash` to link results back to
their exact configuration.

<details>
<summary><strong>Tutorial: Running a Complete Min-Cut Benchmark</strong></summary>

### Step 1: Set up config and trackers

```rust
use ruvector_profiler::*;

let config = BenchConfig {
    model_commit: "abc1234".into(),
    weights_hash: "def5678".into(),
    lambda: 0.5, tau: 2, eps: 0.01,
    compiler_flags: "-O3 -mavx2".into(),
};
println!("Config fingerprint: {}", config_hash(&config));

let mut mem_tracker = MemoryTracker::new("mincut_l0.5_t2");
let power_source = MockPowerSource { watts: 75.0 };
let mut power_tracker = PowerTracker::new("gpu");
```

### Step 2: Run benchmark loop

```rust
let mut latencies = Vec::new();
for i in 0..1000 {
    mem_tracker.snapshot();
    power_tracker.sample(&power_source);
    let start = std::time::Instant::now();
    // ... run attn_mincut() ...
    latencies.push(LatencyRecord {
        sample_id: i,
        wall_time_us: start.elapsed().as_micros() as u64,
        kernel_time_us: start.elapsed().as_micros() as u64,
        seq_len: 128,
    });
}
```

### Step 3: Export results

```rust
let stats = compute_latency_stats(&latencies);
let report = mem_tracker.report();
let energy = power_tracker.energy();

write_latency_csv("results/latency.csv", &latencies).unwrap();
write_memory_csv("results/memory.csv", &mem_tracker.snapshots).unwrap();

println!("Peak RSS: {} | p95: {}us | Energy: {:.3}J",
    report.peak_rss, stats.p95_us, energy.total_joules);
```

### Step 4: Use the benchmark script

```bash
# Full grid search: 1000 samples x 6 settings
./scripts/run_mincut_bench.sh --samples 1000

# Custom grid
./scripts/run_mincut_bench.sh --lambda "0.3 0.5 0.7" --tau "0 2" --seed 42
```

### Expected output structure

```
results/mincut-bench/
  csv/
    baseline.csv           # Softmax reference
    mincut_l0.3_t0.csv     # Per-setting results
    mincut_l0.3_t2.csv
    ...
    results.csv            # Aggregate comparison
  witness/
    mincut_l0.3_t0.jsonl   # SHA-256 witness chains
    witness.rvf            # RVF-packed bundle
  figs/                    # Generated plots
```

</details>

## Related Crates

| Crate | Role |
|-------|------|
| [`ruvector-attn-mincut`](../ruvector-attn-mincut/README.md) | Attention operators being profiled |
| [`ruvector-coherence`](../ruvector-coherence/README.md) | Quality metrics fed into `ResultRow` |
| [`ruvector-solver`](../ruvector-solver/README.md) | Sublinear solvers for graph analytics |

## License

Licensed under the [MIT License](../../LICENSE).
