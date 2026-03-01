# Benchmark Suite Documentation

## Overview

The agentic-synth benchmark suite provides comprehensive performance testing across multiple dimensions:
- Data generation throughput
- API latency and percentiles
- Memory usage profiling
- Cache effectiveness
- Streaming performance
- Concurrent generation scenarios

## Quick Start

```bash
# Install dependencies
npm install

# Build project
npm run build

# Run all benchmarks
npm run benchmark

# Run specific benchmark
npm run benchmark -- --suite "Throughput Test"

# Run with custom configuration
npm run benchmark -- --iterations 20 --concurrency 200

# Generate report
npm run benchmark -- --output benchmarks/report.md
```

## Benchmark Suites

### 1. Throughput Benchmark

**Measures**: Requests per second at various concurrency levels

**Configuration**:
```typescript
{
  iterations: 10,
  concurrency: 100,
  maxTokens: 100
}
```

**Targets**:
- Minimum: 10 req/s
- Target: 50+ req/s
- Optimal: 100+ req/s

### 2. Latency Benchmark

**Measures**: Response time percentiles (P50, P95, P99)

**Configuration**:
```typescript
{
  iterations: 50,
  maxTokens: 50
}
```

**Targets**:
- P50: < 500ms
- P95: < 800ms
- P99: < 1000ms
- Cached: < 100ms

### 3. Memory Benchmark

**Measures**: Memory usage patterns and leak detection

**Configuration**:
```typescript
{
  iterations: 100,
  maxTokens: 100,
  enableGC: true
}
```

**Targets**:
- Peak: < 400MB
- Final (after GC): < 200MB
- No memory leaks

### 4. Cache Benchmark

**Measures**: Cache hit rates and effectiveness

**Configuration**:
```typescript
{
  cacheSize: 1000,
  ttl: 3600000,
  repeatRatio: 0.5
}
```

**Targets**:
- Hit rate: > 50%
- Optimal: > 80%

### 5. Concurrency Benchmark

**Measures**: Performance at various concurrency levels

**Tests**: 10, 50, 100, 200 concurrent requests

**Targets**:
- 10 concurrent: < 2s total
- 50 concurrent: < 5s total
- 100 concurrent: < 10s total
- 200 concurrent: < 20s total

### 6. Streaming Benchmark

**Measures**: Streaming performance and time-to-first-byte

**Configuration**:
```typescript
{
  maxTokens: 500,
  measureFirstChunk: true
}
```

**Targets**:
- First chunk: < 200ms
- Total duration: < 5s
- Chunks: 50-100

## CLI Usage

### Basic Commands

```bash
# Run all benchmarks
agentic-synth benchmark

# Run specific suite
agentic-synth benchmark --suite "Latency Test"

# Custom iterations
agentic-synth benchmark --iterations 20

# Custom concurrency
agentic-synth benchmark --concurrency 200

# Output report
agentic-synth benchmark --output report.md
```

### Advanced Options

```bash
# Full configuration
agentic-synth benchmark \
  --suite "All" \
  --iterations 20 \
  --concurrency 100 \
  --warmup 5 \
  --output benchmarks/detailed-report.md
```

## Programmatic Usage

### Running Benchmarks

```typescript
import {
  BenchmarkRunner,
  ThroughputBenchmark,
  LatencyBenchmark,
  BenchmarkAnalyzer,
  BenchmarkReporter
} from '@ruvector/agentic-synth/benchmarks';
import { AgenticSynth } from '@ruvector/agentic-synth';

const synth = new AgenticSynth({
  enableCache: true,
  maxConcurrency: 100
});

const runner = new BenchmarkRunner();
runner.registerSuite(new ThroughputBenchmark(synth));
runner.registerSuite(new LatencyBenchmark(synth));

const result = await runner.runAll({
  name: 'My Benchmark',
  iterations: 10,
  concurrency: 100,
  warmupIterations: 2,
  timeout: 300000
});

console.log('Throughput:', result.metrics.throughput);
console.log('P99 Latency:', result.metrics.p99LatencyMs);
```

### Analyzing Results

```typescript
import { BenchmarkAnalyzer } from '@ruvector/agentic-synth/benchmarks';

const analyzer = new BenchmarkAnalyzer();
analyzer.analyze(result);

// Automatic bottleneck detection
// Optimization recommendations
// Performance comparison
```

### Generating Reports

```typescript
import { BenchmarkReporter } from '@ruvector/agentic-synth/benchmarks';

const reporter = new BenchmarkReporter();

// Markdown report
await reporter.generateMarkdown([result], 'report.md');

// JSON data export
await reporter.generateJSON([result], 'data.json');
```

## CI/CD Integration

### GitHub Actions

```yaml
name: Performance Benchmarks

on: [push, pull_request]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: '18'

      - name: Install Dependencies
        run: npm ci

      - name: Build
        run: npm run build

      - name: Run Benchmarks
        run: npm run benchmark:ci
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}

      - name: Upload Report
        uses: actions/upload-artifact@v3
        with:
          name: performance-report
          path: benchmarks/performance-report.md

      - name: Check Regression
        run: |
          if [ $? -ne 0 ]; then
            echo "Performance regression detected!"
            exit 1
          fi
```

### GitLab CI

```yaml
benchmark:
  stage: test
  script:
    - npm ci
    - npm run build
    - npm run benchmark:ci
  artifacts:
    paths:
      - benchmarks/performance-report.md
    when: always
  only:
    - main
    - merge_requests
```

## Performance Regression Detection

The CI runner automatically checks for regressions:

```typescript
{
  maxP99Latency: 1000,      // 1 second
  minThroughput: 10,        // 10 req/s
  maxMemoryMB: 400,         // 400MB
  minCacheHitRate: 0.5,     // 50%
  maxErrorRate: 0.01        // 1%
}
```

**Exit Codes**:
- 0: All tests passed
- 1: Performance regression detected

## Report Formats

### Markdown Report

Includes:
- Performance metrics table
- Latency distribution
- Optimization recommendations
- Historical trends
- Pass/fail status

### JSON Report

Includes:
- Raw metrics data
- Timestamp
- Configuration
- Recommendations
- Full result objects

## Performance Metrics

### Collected Metrics

| Metric | Description | Unit |
|--------|-------------|------|
| throughput | Requests per second | req/s |
| p50LatencyMs | 50th percentile latency | ms |
| p95LatencyMs | 95th percentile latency | ms |
| p99LatencyMs | 99th percentile latency | ms |
| avgLatencyMs | Average latency | ms |
| cacheHitRate | Cache hit ratio | 0-1 |
| memoryUsageMB | Memory usage | MB |
| cpuUsagePercent | CPU usage | % |
| concurrentRequests | Active requests | count |
| errorRate | Error ratio | 0-1 |

### Performance Targets

| Category | Metric | Target | Optimal |
|----------|--------|--------|---------|
| Speed | P99 Latency | < 1000ms | < 500ms |
| Speed | Throughput | > 10 req/s | > 50 req/s |
| Cache | Hit Rate | > 50% | > 80% |
| Memory | Usage | < 400MB | < 200MB |
| Reliability | Error Rate | < 1% | < 0.1% |

## Bottleneck Analysis

### Automatic Detection

The analyzer automatically detects:

1. **Latency Bottlenecks**
   - Slow API responses
   - Network issues
   - Cache misses

2. **Throughput Bottlenecks**
   - Low concurrency
   - Sequential processing
   - API rate limits

3. **Memory Bottlenecks**
   - Large cache size
   - Memory leaks
   - Excessive buffering

4. **Cache Bottlenecks**
   - Low hit rate
   - Small cache size
   - Poor key strategy

### Recommendations

Each bottleneck includes:
- Category (cache, routing, memory, etc.)
- Severity (low, medium, high, critical)
- Issue description
- Optimization recommendation
- Estimated improvement
- Implementation effort

## Best Practices

### Running Benchmarks

1. **Warmup**: Always use warmup iterations (2-5)
2. **Iterations**: Use 10+ for statistical significance
3. **Concurrency**: Test at expected load levels
4. **Environment**: Run in consistent environment
5. **Monitoring**: Watch system resources

### Analyzing Results

1. **Trends**: Compare across multiple runs
2. **Baselines**: Establish performance baselines
3. **Regressions**: Set up automated checks
4. **Profiling**: Profile bottlenecks before optimizing
5. **Documentation**: Document optimization changes

### CI/CD Integration

1. **Automation**: Run on every PR/commit
2. **Thresholds**: Set realistic regression thresholds
3. **Artifacts**: Save reports and data
4. **Notifications**: Alert on regressions
5. **History**: Track performance over time

## Troubleshooting

### Common Issues

**High Variance**:
- Increase warmup iterations
- Run more iterations
- Check system load

**API Errors**:
- Verify API key
- Check rate limits
- Review network connectivity

**Out of Memory**:
- Reduce concurrency
- Decrease cache size
- Enable GC

**Slow Benchmarks**:
- Reduce iterations
- Decrease concurrency
- Use smaller maxTokens

## Advanced Features

### Custom Benchmarks

```typescript
import { BenchmarkSuite } from '@ruvector/agentic-synth/benchmarks';

class CustomBenchmark implements BenchmarkSuite {
  name = 'Custom Test';

  async run(): Promise<void> {
    // Your benchmark logic
  }
}

runner.registerSuite(new CustomBenchmark());
```

### Custom Thresholds

```typescript
import { BottleneckAnalyzer } from '@ruvector/agentic-synth/benchmarks';

const analyzer = new BottleneckAnalyzer();
analyzer.setThresholds({
  maxP99LatencyMs: 500,    // Stricter than default
  minThroughput: 50,       // Higher than default
  maxMemoryMB: 300         // Lower than default
});
```

### Performance Hooks

```bash
# Pre-benchmark hook
npx claude-flow@alpha hooks pre-task --description "Benchmarking"

# Post-benchmark hook
npx claude-flow@alpha hooks post-task --task-id "bench-123"
```

## Resources

- [Performance Optimization Guide](./PERFORMANCE.md)
- [API Documentation](./API.md)
- [Examples](../examples/)
- [Source Code](../src/benchmarks/)
