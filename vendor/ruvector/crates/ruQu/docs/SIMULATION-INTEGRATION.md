# ruQu Simulation Integration Guide

**Status**: Proposed
**Date**: 2026-01-17
**Authors**: ruv.io, RuVector Team

---

## Overview

This guide documents how to build and prove the RuVector + dynamic mincut control system against real quantum error correction workloads using Rust-native simulation engines before moving to cloud hardware.

---

## Available Simulation Engines

### 1. Stim with Rust Bindings (Recommended)

**Stim** is a high-performance stabilizer circuit simulator designed for quantum error correction workloads. It can sample syndrome data at kilohertz rates and handle QEC circuits with thousands of qubits.

**Rust Bindings**: `stim-rs` provides direct embedding of Stim's high-performance logic into Rust workflows.

```toml
[dependencies]
stim-rs = "0.x"  # Rust bindings to Stim
```

**Use Case**: Feed Stim circuits into your Rust pipeline and generate high-throughput syndrome streams for processing with the dynamic mincut engine.

### 2. Pure Rust Quantum Simulators

| Crate | Description | Best For |
|-------|-------------|----------|
| `quantsim_core` | Rust quantum circuit simulator engine | Small to moderate circuits, portable |
| `onq` | Experimental Rust quantum engine | Trying out control loops |
| `LogosQ` | High-performance state-vector simulation | Dense circuits, comparing strategies |

```toml
[dependencies]
quantsim_core = "0.x"
onq = "0.4"
```

### 3. Emerging High-Performance Libraries

**LogosQ** offers dramatic speedups over Python frameworks for state-vector and circuit simulation. Good for:
- Dense circuit simulation
- Testing control loops on simulated quantum state data
- Comparing performance impacts of different classical gating strategies

---

## Latency-Oriented Test Workflow

### Step 1: Build a Syndrome Generator

Use Stim via `stim-rs` with a Rust harness that:

1. Defines a surface code QEC circuit
2. Produces syndrome streams in a loop
3. Exposes streams via async channels or memory buffers to the dynamic mincut kernel

```rust
use stim_rs::{Circuit, Detector, Sampler};
use tokio::sync::mpsc;

pub struct SyndromeGenerator {
    circuit: Circuit,
    sampler: Sampler,
}

impl SyndromeGenerator {
    pub fn new(distance: usize, noise_rate: f64) -> Self {
        let circuit = Circuit::surface_code(distance, noise_rate);
        let sampler = circuit.compile_sampler();
        Self { circuit, sampler }
    }

    pub async fn stream(&self, tx: mpsc::Sender<SyndromeRound>) {
        loop {
            let detection_events = self.sampler.sample();
            let round = SyndromeRound::from_stim(detection_events);
            if tx.send(round).await.is_err() {
                break;
            }
        }
    }
}
```

### Step 2: Integrate RuVector Kernel

Embed RuVector + dynamic mincut implementation in Rust:

```rust
use ruvector_mincut::SubpolynomialMinCut;
use ruqu::coherence_gate::CoherenceGate;

pub struct QuantumController {
    gate: CoherenceGate,
    mincut: SubpolynomialMinCut,
}

impl QuantumController {
    pub async fn process_syndrome(&mut self, round: SyndromeRound) -> GateDecision {
        // Update patch graphs
        self.mincut.apply_delta(round.to_graph_delta());

        // Compute cut value and risk score
        let cut_value = self.mincut.current_cut();
        let risk_score = self.evaluate_risk(cut_value);

        // Output permission-to-act signal with region mask
        self.gate.decide(risk_score).await
    }
}
```

### Step 3: Profile Latency

Measure critical performance metrics:

| Metric | Target | Measurement Tool |
|--------|--------|------------------|
| Worst-case latency per cycle | < 4μs | `criterion.rs` |
| Tail latency (p99) | < 10μs | Custom histogram |
| Tail latency (p999) | < 50μs | Custom histogram |
| Scaling with code distance | Sublinear | Parametric benchmark |

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn latency_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_latency");

    for distance in [5, 9, 13, 17, 21] {
        group.bench_with_input(
            BenchmarkId::new("decide", distance),
            &distance,
            |b, &d| {
                let controller = QuantumController::new(d);
                let syndrome = generate_test_syndrome(d);
                b.iter(|| controller.process_syndrome(syndrome.clone()));
            },
        );
    }

    group.finish();
}
```

### Step 4: Benchmark Against Standard Decoders

Compare configurations:

| Configuration | Description |
|---------------|-------------|
| Kernel only | Fast gating without decoder |
| Gated decoder | Baseline decoder with ruQu gating |
| Baseline only | Standard decoder without gating |

**Metrics to Compare**:

```rust
struct BenchmarkResults {
    run_success_rate: f64,
    logical_error_rate: f64,
    overhead_cycles: u64,
    cpu_utilization: f64,
}

fn compare_configurations(distance: usize, noise: f64) -> ComparisonReport {
    let kernel_only = benchmark_kernel_only(distance, noise);
    let gated_decoder = benchmark_gated_decoder(distance, noise);
    let baseline_only = benchmark_baseline_only(distance, noise);

    ComparisonReport {
        kernel_only,
        gated_decoder,
        baseline_only,
        improvement_factor: calculate_improvement(gated_decoder, baseline_only),
    }
}
```

---

## Why Rust is Optimal for This

| Advantage | Benefit |
|-----------|---------|
| **Systems performance** | Control over memory layout, cache-friendly structures |
| **Async support** | Excellent async/await for real-time data paths |
| **Safe parallelism** | Multi-tile and patch processing without data races |
| **Growing ecosystem** | Quantum libraries like `stim-rs`, `quantsim_core` |
| **Type safety** | Catch bugs at compile time, not in production |

---

## Project Template

### Cargo.toml

```toml
[package]
name = "ruqu-simulation"
version = "0.1.0"
edition = "2021"

[dependencies]
# Quantum simulation
stim-rs = "0.x"
quantsim_core = "0.x"
onq = "0.4"

# RuVector integration
ruvector-mincut = { path = "../ruvector-mincut" }
cognitum-gate-tilezero = { path = "../cognitum-gate-tilezero" }

# Async runtime
tokio = { version = "1.0", features = ["full"] }

# Benchmarking
criterion = { version = "0.5", features = ["async_tokio"] }

# Metrics and profiling
metrics = "0.21"
tracing = "0.1"
```

### Main Entry Point

```rust
use tokio::sync::mpsc;
use tracing::{info, instrument};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::init();

    // Create syndrome generator
    let generator = SyndromeGenerator::new(
        distance: 17,
        noise_rate: 0.001,
    );

    // Create controller with mincut engine
    let mut controller = QuantumController::new(17);

    // Channel for syndrome streaming
    let (tx, mut rx) = mpsc::channel(1024);

    // Spawn generator task
    tokio::spawn(async move {
        generator.stream(tx).await;
    });

    // Process syndromes
    let mut cycle = 0u64;
    while let Some(syndrome) = rx.recv().await {
        let decision = controller.process_syndrome(syndrome).await;

        if cycle % 10000 == 0 {
            info!(
                cycle,
                decision = ?decision,
                cut_value = controller.current_cut(),
                "Gate decision"
            );
        }

        cycle += 1;
    }

    Ok(())
}
```

---

## Runtime Model Options

### Synchronous (Simple)

Best for: Initial prototyping, single-threaded testing

```rust
fn main() {
    let mut controller = QuantumController::new(17);
    let generator = SyndromeGenerator::new(17, 0.001);

    for _ in 0..1_000_000 {
        let syndrome = generator.sample();
        let decision = controller.process_syndrome_sync(syndrome);
    }
}
```

### Async Tokio (Recommended)

Best for: Production workloads, multi-tile parallelism

```rust
#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let controller = Arc::new(Mutex::new(QuantumController::new(17)));

    // Process multiple tiles in parallel
    let handles: Vec<_> = (0..255)
        .map(|tile_id| {
            let controller = controller.clone();
            tokio::spawn(async move {
                process_tile(tile_id, controller).await;
            })
        })
        .collect();

    futures::future::join_all(handles).await;
}
```

### No Async (Bare Metal)

Best for: FPGA/ASIC deployment prep, minimal overhead

```rust
#![no_std]

fn process_cycle(syndrome: &[u8], state: &mut GateState) -> GateDecision {
    // Pure computation, no allocation, no runtime
    state.update(syndrome);
    state.decide()
}
```

---

## Performance Targets

| Code Distance | Qubits | Target Latency | Memory |
|---------------|--------|----------------|--------|
| 5 | 41 | < 1μs | < 4 KB |
| 9 | 145 | < 2μs | < 16 KB |
| 13 | 313 | < 3μs | < 32 KB |
| 17 | 545 | < 4μs | < 64 KB |
| 21 | 841 | < 5μs | < 128 KB |

---

## Next Steps

1. **Set up Stim integration**: Install `stim-rs` and generate first syndrome streams
2. **Port mincut kernel**: Adapt `ruvector-mincut` for syndrome-driven updates
3. **Profile baseline**: Establish latency baseline with trivial gate logic
4. **Add three-filter pipeline**: Implement structural, shift, and evidence filters
5. **Compare with decoders**: Benchmark against PyMatching, fusion blossom
6. **Scale testing**: Test with larger code distances and higher noise rates

---

## References

- [Stim GitHub](https://github.com/quantumlib/Stim) - High-performance QEC simulator
- [stim-rs](https://crates.io/crates/stim-rs) - Rust bindings for Stim
- [quantsim_core](https://crates.io/crates/quantsim_core) - Rust quantum simulator
- [onq](https://crates.io/crates/onq) - Experimental Rust quantum engine
- [Criterion.rs](https://bheisler.github.io/criterion.rs/book/) - Rust benchmarking
