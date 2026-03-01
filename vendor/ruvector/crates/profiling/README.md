# Ruvector Performance Profiling

This directory contains profiling scripts, reports, and analysis for Ruvector performance optimization.

## Directory Structure

```
profiling/
├── scripts/          # Profiling and benchmarking scripts
├── reports/          # Generated profiling reports
├── flamegraphs/      # CPU flamegraphs
├── memory/           # Memory profiling data
└── benchmarks/       # Benchmark results
```

## Profiling Tools

### CPU Profiling
- **perf**: Linux performance counters
- **flamegraph**: Visualization of CPU hotspots
- **cargo-flamegraph**: Integrated Rust profiling

### Memory Profiling
- **valgrind**: Memory leak detection and profiling
- **heaptrack**: Heap memory profiling
- **massif**: Heap profiler

### Cache Profiling
- **perf-cache**: Cache miss analysis
- **cachegrind**: Cache simulation

## Quick Start

```bash
# Install profiling tools
./scripts/install_tools.sh

# Run CPU profiling
./scripts/cpu_profile.sh

# Run memory profiling
./scripts/memory_profile.sh

# Generate flamegraph
./scripts/generate_flamegraph.sh

# Run comprehensive benchmark suite
./scripts/benchmark_all.sh
```

## Performance Targets

- **Throughput**: 50,000+ queries per second (QPS)
- **Latency**: Sub-millisecond p50 latency (<1ms)
- **Recall**: 95% recall at high QPS
- **Memory**: Efficient memory usage with minimal allocations in hot paths
- **Scalability**: Linear scaling from 1-128 threads

## Optimization Areas

1. **CPU Optimization**
   - SIMD intrinsics (AVX2/AVX-512)
   - Target-specific compilation
   - Hot path optimization

2. **Memory Optimization**
   - Arena allocation
   - Object pooling
   - Zero-copy operations

3. **Cache Optimization**
   - Structure-of-Arrays layout
   - Cache-line alignment
   - Prefetching

4. **Concurrency Optimization**
   - Lock-free data structures
   - RwLock optimization
   - Rayon tuning

5. **Compile-Time Optimization**
   - Profile-Guided Optimization (PGO)
   - Link-Time Optimization (LTO)
   - Target CPU features
