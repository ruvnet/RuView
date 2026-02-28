# ruvector-scipix Benchmark Suite

Comprehensive performance benchmarking for the Scipix OCR clone using Criterion.

## Overview

This benchmark suite provides detailed performance analysis across all critical components of the OCR system:

- **OCR Latency**: End-to-end OCR performance metrics
- **Preprocessing**: Image preprocessing pipeline performance
- **LaTeX Generation**: LaTeX AST generation and string building
- **Inference**: Model inference benchmarks (detection, recognition, math)
- **Cache**: Embedding cache and similarity search performance
- **API**: REST API request/response handling
- **Memory**: Memory usage, growth, and fragmentation analysis

## Performance Targets

### Primary Targets

- **Single Image OCR**: < 100ms at P95
- **Batch Processing (16 images)**: < 500ms total
- **Preprocessing Pipeline**: < 20ms
- **LaTeX Generation**: < 5ms

### Secondary Targets

- **Cache Hit Latency**: < 1ms
- **Similarity Search (1000 embeddings)**: < 10ms
- **API Request Parsing**: < 0.5ms
- **Model Warm-up**: < 200ms

## Running Benchmarks

### Run All Benchmarks

```bash
cd examples/scipix
./scripts/run_benchmarks.sh all
```

### Run Specific Benchmark Suite

```bash
# OCR latency benchmarks
./scripts/run_benchmarks.sh latency

# Preprocessing benchmarks
./scripts/run_benchmarks.sh preprocessing

# LaTeX generation benchmarks
./scripts/run_benchmarks.sh latex

# Model inference benchmarks
./scripts/run_benchmarks.sh inference

# Cache benchmarks
./scripts/run_benchmarks.sh cache

# API benchmarks
./scripts/run_benchmarks.sh api

# Memory benchmarks
./scripts/run_benchmarks.sh memory
```

### Quick Benchmark Suite

For rapid iteration during development:

```bash
./scripts/run_benchmarks.sh quick
```

### CI Benchmark Suite

Minimal samples for continuous integration:

```bash
./scripts/run_benchmarks.sh ci
```

## Baseline Tracking

### Save Current Results as Baseline

```bash
BASELINE=v1.0 ./scripts/run_benchmarks.sh all
```

### Compare with Saved Baseline

```bash
./scripts/run_benchmarks.sh compare v1.0
```

### Compare with Main Branch

```bash
BASELINE=main ./scripts/run_benchmarks.sh all
./scripts/run_benchmarks.sh compare main
```

## Benchmark Details

### 1. OCR Latency Benchmarks (`ocr_latency.rs`)

Tests end-to-end OCR performance across various scenarios:

- **Single Image OCR**: Different image sizes (224x224 to 1024x1024)
- **Batch Processing**: Batch sizes from 1 to 32 images
- **Cold vs Warm Start**: Model initialization overhead
- **Latency Percentiles**: P50, P95, P99 measurements
- **Throughput**: Images per second

**Key Metrics:**
- Mean latency
- P95/P99 latency
- Throughput (images/sec)
- Batch efficiency

### 2. Preprocessing Benchmarks (`preprocessing.rs`)

Image preprocessing pipeline performance:

- **Individual Transforms**: Grayscale, blur, threshold, edge detection
- **Full Pipeline**: Sequential preprocessing chain
- **Parallel vs Sequential**: Batch processing comparison
- **Resize Operations**: Nearest neighbor and bilinear interpolation

**Key Metrics:**
- Transform latency
- Pipeline total time
- Parallel speedup
- Memory overhead

### 3. LaTeX Generation Benchmarks (`latex_generation.rs`)

LaTeX code generation from AST:

- **Simple Expressions**: Fractions, powers, sums
- **Complex Expressions**: Matrices, integrals, summations
- **AST Traversal**: Tree depth impact on performance
- **String Building**: Optimization strategies
- **Batch Generation**: Multiple expressions

**Key Metrics:**
- Generation latency
- AST traversal time
- String concatenation efficiency

### 4. Inference Benchmarks (`inference.rs`)

Neural network model inference:

- **Text Detection Model**: Bounding box detection
- **Text Recognition Model**: OCR text extraction
- **Math Model**: Mathematical notation recognition
- **Tensor Preprocessing**: Image to tensor conversion
- **Output Postprocessing**: NMS, confidence filtering, CTC decoding
- **Batch Inference**: Multi-image processing
- **Model Warm-up**: Initialization overhead

**Key Metrics:**
- Inference latency per model
- Batch throughput
- Preprocessing overhead
- Postprocessing time

### 5. Cache Benchmarks (`cache.rs`)

Embedding cache and similarity search:

- **Embedding Generation**: Image to vector embedding
- **Similarity Search**: Linear and approximate nearest neighbor
- **Cache Hit/Miss Latency**: Lookup performance
- **Cache Insertion**: Add new entries
- **Batch Operations**: Multi-query performance
- **Cache Statistics**: Memory and efficiency metrics

**Key Metrics:**
- Embedding generation time
- Search latency (linear vs ANN)
- Hit/miss ratio impact
- Memory per embedding

### 6. API Benchmarks (`api.rs`)

REST API performance:

- **Request Parsing**: JSON deserialization
- **Response Serialization**: JSON encoding
- **Concurrent Requests**: Multi-client handling
- **Middleware Overhead**: Auth, logging, validation, rate limiting
- **Error Handling**: Error response generation
- **End-to-End Request**: Full request cycle

**Key Metrics:**
- Parse/serialize latency
- Middleware overhead
- Concurrent throughput
- Error handling time

### 7. Memory Benchmarks (`memory.rs`)

Memory usage and management:

- **Peak Memory**: Maximum usage during inference
- **Memory per Image**: Batch processing memory scaling
- **Model Loading**: Memory required for model initialization
- **Memory Growth**: Leak detection over time
- **Fragmentation**: Allocation/deallocation patterns
- **Cache Memory**: Embedding storage overhead
- **Memory Pools**: Pool vs heap allocation
- **Tensor Layouts**: HWC vs CHW memory impact

**Key Metrics:**
- Peak memory usage
- Memory growth rate
- Fragmentation level
- Pool efficiency

## HTML Reports

Criterion automatically generates detailed HTML reports with:

- Performance graphs
- Statistical analysis
- Regression detection
- Historical comparisons

### View Reports

After running benchmarks, open:

```bash
open target/criterion/report/index.html
```

Or for a specific benchmark:

```bash
open target/criterion/ocr_latency/report/index.html
```

## Interpreting Results

### Latency Metrics

- **Mean**: Average latency across all samples
- **Median (P50)**: 50th percentile - half of requests are faster
- **P95**: 95th percentile - 95% of requests are faster
- **P99**: 99th percentile - 99% of requests are faster
- **Standard Deviation**: Variance in latency

### Throughput Metrics

- **Images/Second**: Processing rate
- **Batch Efficiency**: Speedup from batching
- **Sustainable Throughput**: Max rate with <95% success

### Regression Detection

Criterion detects performance regressions automatically:

- **Green**: Performance improved
- **Yellow**: Minor change (within noise)
- **Red**: Performance regressed

### Memory Metrics

- **Peak Usage**: Maximum memory at any point
- **Growth Rate**: Memory increase over time
- **Fragmentation**: Memory layout efficiency

## Best Practices

### Running Benchmarks

1. **Consistent Environment**: Run on the same hardware
2. **Quiet System**: Close other applications
3. **Multiple Samples**: Use sufficient sample size (50-100)
4. **Warm-up**: Allow for JIT compilation and caching
5. **Baseline Tracking**: Save results for comparison

### Analyzing Results

1. **Focus on Percentiles**: P95/P99 more important than mean
2. **Check Variance**: High variance indicates instability
3. **Profile Outliers**: Investigate extreme values
4. **Memory Leaks**: Monitor growth rate
5. **Regression Limits**: Set acceptable thresholds

### Optimization Workflow

1. **Baseline**: Establish current performance
2. **Profile**: Identify bottlenecks
3. **Optimize**: Implement improvements
4. **Benchmark**: Measure impact
5. **Compare**: Verify improvement vs baseline
6. **Iterate**: Repeat until targets met

## Continuous Integration

### CI Benchmark Configuration

```yaml
# .github/workflows/benchmark.yml
name: Benchmarks

on:
  pull_request:
  push:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run benchmarks
        run: |
          cd examples/scipix
          ./scripts/run_benchmarks.sh ci

      - name: Compare with baseline
        run: |
          cd examples/scipix
          ./scripts/run_benchmarks.sh compare main
```

## Troubleshooting

### Benchmarks Running Slowly

- Reduce sample size: `cargo bench -- --sample-size 10`
- Use quick mode: `./scripts/run_benchmarks.sh quick`
- Run specific benchmarks only

### Inconsistent Results

- Ensure system is idle
- Disable CPU frequency scaling
- Run with higher sample size
- Check for thermal throttling

### Memory Issues

- Monitor system memory during benchmarks
- Use memory profiling tools (valgrind, heaptrack)
- Check for memory leaks with growth benchmarks

## Contributing

When adding new features:

1. Add corresponding benchmarks
2. Set performance targets
3. Run baseline before/after changes
4. Document any performance impact
5. Update this documentation

## Resources

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Benchmarking Best Practices](https://easyperf.net/blog/)
