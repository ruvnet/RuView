# SONA Performance Benchmarks

## Overview

This document defines performance targets, benchmark methodology, and expected results for SONA components. All benchmarks are designed to be reproducible and measurable.

## Performance Targets Summary

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      SONA Performance Targets                           │
├─────────────────────────────────────────────────────────────────────────┤
│  Component              │ Target         │ Stretch Goal  │ Unit        │
├─────────────────────────┼────────────────┼───────────────┼─────────────┤
│  Micro-LoRA forward     │ <50μs          │ <20μs         │ per request │
│  Micro-LoRA update      │ <100μs         │ <50μs         │ per signal  │
│  Base LoRA forward      │ <200μs         │ <100μs        │ per layer   │
│  Pattern extraction     │ <1s            │ <500ms        │ per 1000    │
│  Trajectory recording   │ <10μs          │ <5μs          │ per step    │
│  Background cycle       │ <30s           │ <15s          │ per cycle   │
│  Deep cycle             │ <10min         │ <5min         │ per cycle   │
│  Memory overhead        │ <100MB         │ <50MB         │ total       │
│  Pattern search         │ <1ms           │ <100μs        │ per query   │
│  Dream generation       │ <100ms         │ <50ms         │ per dream   │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Micro-LoRA Benchmarks

### Forward Pass Latency

**Target**: <50μs average, <100μs p99

```rust
// benches/micro_lora.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_micro_lora_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro_lora_forward");

    for rank in [1, 2] {
        for hidden_dim in [256, 512, 1024, 2048] {
            let lora = MicroLoRA::new(hidden_dim, rank);
            let input = vec![0.1f32; hidden_dim];
            let mut output = vec![0.0f32; hidden_dim];

            group.bench_with_input(
                BenchmarkId::new(format!("rank{}", rank), hidden_dim),
                &hidden_dim,
                |b, _| {
                    b.iter(|| {
                        output.fill(0.0);
                        unsafe { lora.forward_simd(&input, &mut output) };
                    });
                },
            );
        }
    }

    group.finish();
}
```

**Expected Results**:

| Rank | Hidden Dim | AVX2 (μs) | Scalar (μs) | Speedup |
|------|------------|-----------|-------------|---------|
| 1    | 256        | 3.2       | 12.5        | 3.9x    |
| 1    | 512        | 5.8       | 24.1        | 4.2x    |
| 1    | 1024       | 10.4      | 47.3        | 4.5x    |
| 1    | 2048       | 19.7      | 93.8        | 4.8x    |
| 2    | 256        | 5.1       | 23.4        | 4.6x    |
| 2    | 512        | 9.3       | 46.2        | 5.0x    |
| 2    | 1024       | 17.2      | 91.5        | 5.3x    |
| 2    | 2048       | 33.1      | 182.4       | 5.5x    |

### Gradient Accumulation

**Target**: <100μs per signal

```rust
fn bench_gradient_accumulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("gradient_accumulation");

    for hidden_dim in [256, 512, 1024] {
        let mut lora = MicroLoRA::new(hidden_dim, 1);
        let signal = LearningSignal {
            query_embedding: vec![0.1; hidden_dim],
            gradient_estimate: vec![0.01; hidden_dim],
            quality_score: 0.8,
            timestamp: Instant::now(),
            metadata: SignalMetadata::default(),
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(hidden_dim),
            &hidden_dim,
            |b, _| {
                b.iter(|| {
                    lora.accumulate_gradient(&signal);
                });
            },
        );
    }

    group.finish();
}
```

**Expected Results**:

| Hidden Dim | Time (μs) | Throughput (signals/s) |
|------------|-----------|------------------------|
| 256        | 8.3       | 120,481                |
| 512        | 15.7      | 63,694                 |
| 1024       | 30.2      | 33,112                 |

---

## Base LoRA Benchmarks

### Forward Pass (Per Layer)

**Target**: <200μs per layer

```rust
fn bench_base_lora_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("base_lora_forward");

    for rank in [4, 8, 16] {
        for hidden_dim in [512, 1024, 2048] {
            let lora = BaseLoRA::new(hidden_dim, rank, 1);
            let input = vec![0.1f32; hidden_dim];
            let mut output = vec![0.0f32; hidden_dim];

            group.bench_with_input(
                BenchmarkId::new(format!("rank{}", rank), hidden_dim),
                &hidden_dim,
                |b, _| {
                    b.iter(|| {
                        lora.forward_layer(0, &input, &mut output);
                    });
                },
            );
        }
    }

    group.finish();
}
```

**Expected Results**:

| Rank | Hidden Dim | Time (μs) | FLOPs    | GFLOPS |
|------|------------|-----------|----------|--------|
| 4    | 512        | 45        | 4.2M     | 93     |
| 4    | 1024       | 85        | 8.4M     | 99     |
| 4    | 2048       | 162       | 16.8M    | 104    |
| 8    | 512        | 82        | 8.4M     | 102    |
| 8    | 1024       | 158       | 16.8M    | 106    |
| 8    | 2048       | 305       | 33.5M    | 110    |
| 16   | 512        | 155       | 16.8M    | 108    |
| 16   | 1024       | 298       | 33.5M    | 112    |
| 16   | 2048       | 582       | 67.1M    | 115    |

---

## Trajectory Recording Benchmarks

### Step Recording Latency

**Target**: <10μs per step

```rust
fn bench_trajectory_recording(c: &mut Criterion) {
    let mut group = c.benchmark_group("trajectory_recording");

    for hidden_dim in [256, 512] {
        for num_heads in [4, 8] {
            let mut builder = TrajectoryBuilder::new(1, vec![0.1; hidden_dim]);

            group.bench_with_input(
                BenchmarkId::new(format!("h{}_heads{}", hidden_dim, num_heads), hidden_dim),
                &(hidden_dim, num_heads),
                |b, &(hd, nh)| {
                    b.iter(|| {
                        builder.add_step(
                            vec![0.5; hd],
                            vec![0.1; hd * nh],
                            0.8,
                        );
                    });
                },
            );
        }
    }

    group.finish();
}
```

**Expected Results**:

| Hidden Dim | Heads | Time (μs) | Memory (bytes) |
|------------|-------|-----------|----------------|
| 256        | 4     | 2.1       | 5,120          |
| 256        | 8     | 3.8       | 9,216          |
| 512        | 4     | 3.7       | 10,240         |
| 512        | 8     | 6.9       | 18,432         |

### Buffer Operations

**Target**: Lock-free with <1% contention

```rust
fn bench_trajectory_buffer(c: &mut Criterion) {
    let buffer = Arc::new(TrajectoryBuffer::new(10000));

    c.bench_function("trajectory_buffer_record", |b| {
        let trajectory = QueryTrajectory {
            id: 1,
            query_embedding: vec![0.1; 256],
            steps: vec![],
            final_quality: 0.8,
            latency_us: 1000,
        };

        b.iter(|| {
            buffer.record(trajectory.clone());
        });
    });

    c.bench_function("trajectory_buffer_drain", |b| {
        // Pre-fill buffer
        for i in 0..1000 {
            buffer.record(QueryTrajectory {
                id: i,
                query_embedding: vec![0.1; 256],
                steps: vec![],
                final_quality: 0.8,
                latency_us: 1000,
            });
        }

        b.iter(|| {
            buffer.drain()
        });
    });
}
```

---

## Pattern Learning Benchmarks

### K-means++ Extraction

**Target**: <1s for 1000 trajectories

```rust
fn bench_pattern_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_extraction");

    for n_trajectories in [100, 500, 1000, 5000] {
        let mut bank = ReasoningBank::new(PatternConfig {
            k_clusters: 50,
            embedding_dim: 256,
            ..Default::default()
        });

        // Pre-populate
        for i in 0..n_trajectories {
            bank.add_trajectory(&generate_random_trajectory(i, 256));
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(n_trajectories),
            &n_trajectories,
            |b, _| {
                b.iter(|| {
                    bank.extract_patterns()
                });
            },
        );
    }

    group.finish();
}
```

**Expected Results**:

| Trajectories | Clusters | Time (ms) | Iterations |
|--------------|----------|-----------|------------|
| 100          | 10       | 12        | 8          |
| 500          | 25       | 95        | 12         |
| 1000         | 50       | 380       | 15         |
| 5000         | 100      | 2,450     | 20         |

### Pattern Search

**Target**: <1ms per query

```rust
fn bench_pattern_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_search");

    for n_patterns in [1000, 10000, 100000] {
        let mut index = PatternIndex::new(256, n_patterns);

        // Pre-populate
        for i in 0..n_patterns {
            let embedding: Vec<f32> = (0..256).map(|_| rand::random()).collect();
            index.add_pattern(i as u64, &embedding).unwrap();
        }

        let query: Vec<f32> = (0..256).map(|_| rand::random()).collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(n_patterns),
            &n_patterns,
            |b, _| {
                b.iter(|| {
                    index.find_similar(&query, 10)
                });
            },
        );
    }

    group.finish();
}
```

**Expected Results** (HNSW with ef=50):

| Patterns | Search Time (μs) | Recall@10 |
|----------|------------------|-----------|
| 1,000    | 45               | 0.98      |
| 10,000   | 120              | 0.96      |
| 100,000  | 350              | 0.94      |
| 1,000,000| 850              | 0.92      |

---

## EWC++ Benchmarks

### Fisher Information Update

**Target**: <1ms per update

```rust
fn bench_fisher_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("fisher_update");

    for param_count in [1000, 10000, 100000] {
        let mut ewc = EwcPlusPlus::new(EwcConfig {
            param_count,
            ..Default::default()
        });

        let gradients: Vec<f32> = (0..param_count).map(|_| rand::random::<f32>() * 0.01).collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(param_count),
            &param_count,
            |b, _| {
                b.iter(|| {
                    ewc.update_fisher(&gradients);
                });
            },
        );
    }

    group.finish();
}
```

**Expected Results**:

| Parameters | Update Time (μs) | Memory (KB) |
|------------|------------------|-------------|
| 1,000      | 15               | 8           |
| 10,000     | 120              | 80          |
| 100,000    | 1,150            | 800         |

### Constraint Application

**Target**: <500μs per gradient vector

```rust
fn bench_constraint_application(c: &mut Criterion) {
    let mut group = c.benchmark_group("ewc_constraints");

    for param_count in [1000, 10000, 100000] {
        let ewc = EwcPlusPlus::new(EwcConfig {
            param_count,
            num_tasks: 5,
            ..Default::default()
        });

        // Pre-train Fisher
        for _ in 0..100 {
            let grads: Vec<f32> = (0..param_count).map(|_| rand::random::<f32>() * 0.01).collect();
            ewc.update_fisher(&grads);
        }

        let gradients: Vec<f32> = (0..param_count).map(|_| rand::random::<f32>() * 0.01).collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(param_count),
            &param_count,
            |b, _| {
                b.iter(|| {
                    ewc.apply_constraints(&gradients)
                });
            },
        );
    }

    group.finish();
}
```

---

## Dream Engine Benchmarks

### Dream Generation

**Target**: <100ms per dream

```rust
fn bench_dream_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("dream_generation");

    for memory_size in [1000, 10000, 50000] {
        let mut engine = DreamEngine::new(DreamConfig::default());

        // Pre-populate memory
        for i in 0..memory_size {
            engine.add_memory_node(MemoryNode {
                id: i as u64,
                embedding: (0..256).map(|_| rand::random()).collect(),
                timestamp: Instant::now(),
                access_count: rand::random::<u32>() % 100,
                importance: rand::random(),
            });
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(memory_size),
            &memory_size,
            |b, _| {
                b.iter(|| {
                    engine.generate_dream()
                });
            },
        );
    }

    group.finish();
}
```

**Expected Results**:

| Memory Nodes | Dream Time (ms) | Avg Path Length |
|--------------|-----------------|-----------------|
| 1,000        | 12              | 8               |
| 10,000       | 45              | 12              |
| 50,000       | 85              | 15              |

### Dream Quality Evaluation

**Target**: <50ms per evaluation

```rust
fn bench_dream_evaluation(c: &mut Criterion) {
    let evaluator = DreamEvaluator::new(EvaluatorConfig::default());

    let dream = Dream {
        id: 1,
        path: (0..15).map(|i| MemoryNode {
            id: i,
            embedding: (0..256).map(|_| rand::random()).collect(),
            timestamp: Instant::now(),
            access_count: 10,
            importance: 0.5,
        }).collect(),
        creative_jumps: 3,
        total_novelty: 0.0,
    };

    c.bench_function("dream_evaluation", |b| {
        b.iter(|| {
            evaluator.evaluate(&dream)
        });
    });
}
```

---

## Learning Loop Benchmarks

### Loop A (Instant) - Per Request

**Target**: <1ms total overhead

```rust
fn bench_loop_a(c: &mut Criterion) {
    let loop_a = InstantLoop::new(256, InstantLoopConfig::default());

    let trajectory = QueryTrajectory {
        id: 1,
        query_embedding: vec![0.1; 256],
        steps: (0..10).map(|_| TrajectoryStep {
            activations: vec![0.5; 256],
            attention_weights: vec![0.1; 2048],
            reward: 0.8,
            timestamp: Instant::now(),
        }).collect(),
        final_quality: 0.8,
        latency_us: 50000,
    };

    c.bench_function("loop_a_on_inference", |b| {
        b.iter(|| {
            loop_a.on_inference(trajectory.clone());
        });
    });

    c.bench_function("loop_a_flush", |b| {
        // Pre-fill with signals
        for _ in 0..100 {
            loop_a.on_inference(trajectory.clone());
        }

        b.iter(|| {
            loop_a.flush_updates();
        });
    });
}
```

**Expected Results**:

| Operation     | Time (μs) | Notes                    |
|---------------|-----------|--------------------------|
| on_inference  | 650       | Recording + accumulation |
| flush_updates | 120       | LoRA + edge commit       |
| Total         | 770       | Per request overhead     |

### Loop B (Background) - Hourly

**Target**: <30s per cycle

```rust
fn bench_loop_b(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let loop_b = BackgroundLoop::new(BackgroundLoopConfig::default(), 256);

    // Generate trajectories
    let trajectories: Vec<_> = (0..1000)
        .map(|i| generate_random_trajectory(i, 256))
        .collect();

    c.bench_function("loop_b_cycle", |b| {
        b.to_async(&runtime).iter(|| async {
            loop_b.run_cycle(trajectories.clone()).await
        });
    });
}
```

**Breakdown**:

| Phase                  | Time (s) | % of Total |
|------------------------|----------|------------|
| Trajectory ingestion   | 0.5      | 2%         |
| Pattern extraction     | 8.0      | 32%        |
| Gradient computation   | 5.0      | 20%        |
| EWC++ constraints      | 3.0      | 12%        |
| LoRA update            | 2.0      | 8%         |
| Fisher update          | 4.0      | 16%        |
| Metrics/logging        | 2.5      | 10%        |
| **Total**              | **25.0** | 100%       |

### Loop C (Deep) - Weekly

**Target**: <10min per cycle

```rust
fn bench_loop_c(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let loop_c = DeepLoop::new(DeepLoopConfig::default());

    // This is a longer benchmark, run fewer iterations
    c.bench_function("loop_c_cycle", |b| {
        b.to_async(&runtime).iter(|| async {
            loop_c.run_cycle().await
        });
    });
}
```

**Breakdown**:

| Phase                  | Time (min) | % of Total |
|------------------------|------------|------------|
| Dream generation (50)  | 1.5        | 15%        |
| Φ evaluation           | 2.0        | 20%        |
| Dream integration      | 1.0        | 10%        |
| Memory consolidation   | 3.0        | 30%        |
| EWC++ consolidation    | 2.0        | 20%        |
| Metrics/persistence    | 0.5        | 5%         |
| **Total**              | **10.0**   | 100%       |

---

## Memory Benchmarks

### Memory Usage by Component

```rust
fn measure_memory_usage() -> MemoryReport {
    let mut report = MemoryReport::default();

    // Micro-LoRA (rank=1, hidden=256)
    let micro_lora = MicroLoRA::new(256, 1);
    report.micro_lora = std::mem::size_of_val(&micro_lora)
        + micro_lora.down_proj.len() * 4
        + micro_lora.up_proj.len() * 4
        + micro_lora.gradient_buffer.len() * 4;

    // Base LoRA (rank=8, hidden=256, layers=12)
    let base_lora = BaseLoRA::new(256, 8, 12);
    report.base_lora = std::mem::size_of_val(&base_lora)
        + base_lora.layers.iter().map(|l|
            l.down_proj.len() * 4 + l.up_proj.len() * 4
        ).sum::<usize>();

    // Trajectory buffer (capacity=10000)
    report.trajectory_buffer = 10000 * (
        256 * 4  // query embedding
        + 10 * (256 * 4 + 2048 * 4 + 4 + 8)  // 10 steps
    );

    // Pattern index (100k patterns)
    report.pattern_index = 100000 * (256 * 4 + 64);  // embedding + metadata

    // EWC++ (100k params, 5 tasks)
    report.ewc = 100000 * 4 * 5;  // Fisher per task

    report
}
```

**Expected Memory Usage**:

| Component        | Size (MB) | Notes                    |
|------------------|-----------|--------------------------|
| Micro-LoRA       | 0.004     | Minimal overhead         |
| Base LoRA        | 0.6       | 12 layers                |
| Trajectory Buffer| 82.0      | 10k capacity             |
| Pattern Index    | 102.4     | 100k patterns            |
| EWC++ Fisher     | 2.0       | 100k params × 5 tasks    |
| Dream Engine     | 12.8      | 50k memory nodes         |
| **Total**        | **199.8** | Peak usage               |

---

## Throughput Benchmarks

### End-to-End Query Throughput

```rust
fn bench_query_throughput(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let sona = runtime.block_on(async {
        SonaEngine::new(SonaConfig::default()).await.unwrap()
    });

    c.bench_function("query_throughput", |b| {
        b.to_async(&runtime).iter(|| async {
            sona.process("test query", &Context::default()).await
        });
    });
}
```

**Expected Throughput**:

| Scenario           | QPS     | Latency p50 | Latency p99 |
|--------------------|---------|-------------|-------------|
| Baseline (no SONA) | 850     | 1.1ms       | 2.5ms       |
| With Micro-LoRA    | 780     | 1.2ms       | 2.8ms       |
| Full SONA          | 720     | 1.3ms       | 3.2ms       |

**Overhead**: ~15% throughput reduction for full self-learning capability.

---

## Hardware-Specific Benchmarks

### CPU Feature Detection

```rust
fn check_cpu_features() -> CpuFeatures {
    CpuFeatures {
        avx2: is_x86_feature_detected!("avx2"),
        avx512f: is_x86_feature_detected!("avx512f"),
        fma: is_x86_feature_detected!("fma"),
        sse4_1: is_x86_feature_detected!("sse4.1"),
        sse4_2: is_x86_feature_detected!("sse4.2"),
    }
}
```

### Performance by CPU

| CPU                    | Micro-LoRA (μs) | Pattern Search (μs) | Overall Speedup |
|------------------------|-----------------|---------------------|-----------------|
| Intel i9-13900K (AVX2) | 3.2             | 45                  | 4.8x            |
| AMD Ryzen 9 7950X      | 3.5             | 48                  | 4.5x            |
| Apple M2 Pro (NEON)    | 4.1             | 52                  | 3.9x            |
| Intel Xeon Platinum    | 2.8             | 38                  | 5.2x            |

---

## Benchmark Commands

```bash
# Run all benchmarks
cargo bench --package ruvllm --features sona

# Run specific benchmark group
cargo bench --package ruvllm --bench micro_lora

# Run with specific features
cargo bench --package ruvllm --features "sona,avx2"

# Profile memory
cargo bench --package ruvllm --bench memory -- --profile-time 60

# Generate flamegraph
cargo flamegraph --bench micro_lora -- --bench
```

---

## Continuous Benchmarking

### CI Integration

```yaml
# .github/workflows/bench.yml
name: Benchmarks

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Run benchmarks
        run: cargo bench --package ruvllm --features sona -- --save-baseline main

      - name: Compare with baseline
        run: cargo bench --package ruvllm --features sona -- --baseline main

      - name: Upload results
        uses: actions/upload-artifact@v4
        with:
          name: benchmark-results
          path: target/criterion
```

### Regression Detection

```rust
// Fail CI if performance regresses by more than 10%
const MAX_REGRESSION_PERCENT: f64 = 10.0;

fn check_regression(baseline: Duration, current: Duration) -> Result<(), String> {
    let regression = (current.as_nanos() as f64 / baseline.as_nanos() as f64 - 1.0) * 100.0;

    if regression > MAX_REGRESSION_PERCENT {
        Err(format!(
            "Performance regression of {:.1}% exceeds threshold of {}%",
            regression, MAX_REGRESSION_PERCENT
        ))
    } else {
        Ok(())
    }
}
```

---

## Next Steps

1. **09-API-REFERENCE.md** - Complete API documentation
