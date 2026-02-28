# Performance Optimization Guide

## Overview

Agentic-Synth is optimized for high-performance synthetic data generation with the following targets:
- **Sub-second response times** for cached requests
- **100+ concurrent generations** supported
- **Memory efficient** data handling (< 400MB)
- **50%+ cache hit rate** for typical workloads

## Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| P99 Latency | < 1000ms | For cached requests < 100ms |
| Throughput | > 10 req/s | Scales with concurrency |
| Memory Usage | < 400MB | With 1000-item cache |
| Cache Hit Rate | > 50% | Depends on workload patterns |
| Error Rate | < 1% | With retry logic |

## Optimization Strategies

### 1. Context Caching

**Configuration:**
```typescript
const synth = new AgenticSynth({
  enableCache: true,
  cacheSize: 1000,      // Adjust based on memory
  cacheTTL: 3600000,    // 1 hour in milliseconds
});
```

**Benefits:**
- Reduces API calls by 50-80%
- Sub-100ms latency for cache hits
- Automatic LRU eviction

**Best Practices:**
- Use consistent prompts for better cache hits
- Increase cache size for repetitive workloads
- Monitor cache hit rate with `synth.getMetrics()`

### 2. Model Routing

**Configuration:**
```typescript
const synth = new AgenticSynth({
  modelPreference: [
    'claude-sonnet-4-5-20250929',
    'claude-3-5-sonnet-20241022'
  ],
});
```

**Features:**
- Automatic load balancing
- Performance-based routing
- Error handling and fallback

### 3. Concurrent Generation

**Configuration:**
```typescript
const synth = new AgenticSynth({
  maxConcurrency: 100,  // Adjust based on API limits
});
```

**Usage:**
```typescript
const prompts = [...]; // 100+ prompts
const results = await synth.generateBatch(prompts, {
  maxTokens: 500
});
```

**Performance:**
- 2-3x faster than sequential
- Respects concurrency limits
- Automatic batching

### 4. Memory Management

**Configuration:**
```typescript
const synth = new AgenticSynth({
  memoryLimit: 512 * 1024 * 1024,  // 512MB
});
```

**Features:**
- Automatic memory tracking
- LRU eviction when over limit
- Periodic cleanup with `synth.optimize()`

### 5. Streaming for Large Outputs

**Usage:**
```typescript
const stream = synth.generateStream(prompt, {
  maxTokens: 4096
});

for await (const chunk of stream) {
  // Process chunk immediately
  processChunk(chunk);
}
```

**Benefits:**
- Lower time-to-first-byte
- Reduced memory usage
- Better user experience

## Benchmarking

### Running Benchmarks

```bash
# Run all benchmarks
npm run benchmark

# Run specific suite
npm run benchmark -- --suite "Throughput Test"

# With custom settings
npm run benchmark -- --iterations 20 --concurrency 200

# Generate report
npm run benchmark -- --output benchmarks/report.md
```

### Benchmark Suites

1. **Throughput Test**: Measures requests per second
2. **Latency Test**: Measures P50/P95/P99 latencies
3. **Memory Test**: Measures memory usage and leaks
4. **Cache Test**: Measures cache effectiveness
5. **Concurrency Test**: Tests concurrent request handling
6. **Streaming Test**: Measures streaming performance

### Analyzing Results

```bash
# Analyze performance
npm run perf:analyze

# Generate detailed report
npm run perf:report
```

## Bottleneck Detection

The built-in bottleneck analyzer automatically detects:

### 1. Latency Bottlenecks
- **Cause**: Slow API responses, network issues
- **Solution**: Increase cache size, optimize prompts
- **Impact**: 30-50% latency reduction

### 2. Throughput Bottlenecks
- **Cause**: Low concurrency, sequential processing
- **Solution**: Increase maxConcurrency, use batch API
- **Impact**: 2-3x throughput increase

### 3. Memory Bottlenecks
- **Cause**: Large cache, memory leaks
- **Solution**: Reduce cache size, call optimize()
- **Impact**: 40-60% memory reduction

### 4. Cache Bottlenecks
- **Cause**: Low hit rate, small cache
- **Solution**: Increase cache size, optimize keys
- **Impact**: 20-40% cache improvement

## CI/CD Integration

### Performance Regression Detection

```bash
# Run in CI
npm run benchmark:ci
```

**Features:**
- Automatic threshold checking
- Fails build on regression
- Generates reports for artifacts

### GitHub Actions Example

```yaml
- name: Performance Benchmarks
  run: npm run benchmark:ci

- name: Upload Report
  uses: actions/upload-artifact@v3
  with:
    name: performance-report
    path: benchmarks/performance-report.md
```

## Profiling

### CPU Profiling

```bash
npm run benchmark:profile
node --prof-process isolate-*.log > profile.txt
```

### Memory Profiling

```bash
node --expose-gc --max-old-space-size=512 dist/benchmarks/runner.js
```

### Chrome DevTools

```bash
node --inspect-brk dist/benchmarks/runner.js
# Open chrome://inspect
```

## Optimization Checklist

- [ ] Enable caching for repetitive workloads
- [ ] Set appropriate cache size (1000+ items)
- [ ] Configure concurrency based on API limits
- [ ] Use batch API for multiple generations
- [ ] Implement streaming for large outputs
- [ ] Monitor memory usage regularly
- [ ] Run benchmarks before releases
- [ ] Set up CI/CD performance tests
- [ ] Profile bottlenecks periodically
- [ ] Optimize prompt patterns for cache hits

## Performance Monitoring

### Runtime Metrics

```typescript
// Get current metrics
const metrics = synth.getMetrics();
console.log('Cache:', metrics.cache);
console.log('Memory:', metrics.memory);
console.log('Router:', metrics.router);
```

### Performance Monitor

```typescript
import { PerformanceMonitor } from '@ruvector/agentic-synth';

const monitor = new PerformanceMonitor();
monitor.start();

// ... run workload ...

const metrics = monitor.getMetrics();
console.log('Throughput:', metrics.throughput);
console.log('P99 Latency:', metrics.p99LatencyMs);
```

### Bottleneck Analysis

```typescript
import { BottleneckAnalyzer } from '@ruvector/agentic-synth';

const analyzer = new BottleneckAnalyzer();
const report = analyzer.analyze(metrics);

if (report.detected) {
  console.log('Bottlenecks:', report.bottlenecks);
  console.log('Recommendations:', report.recommendations);
}
```

## Best Practices

1. **Cache Strategy**: Use prompts as cache keys, normalize formatting
2. **Concurrency**: Start with 100, increase based on API limits
3. **Memory**: Monitor with getMetrics(), call optimize() periodically
4. **Streaming**: Use for outputs > 1000 tokens
5. **Benchmarking**: Run before releases, track trends
6. **Monitoring**: Enable in production, set up alerts
7. **Optimization**: Profile first, optimize bottlenecks
8. **Testing**: Include performance tests in CI/CD

## Troubleshooting

### High Latency
- Check cache hit rate
- Increase cache size
- Optimize prompt patterns
- Check network connectivity

### Low Throughput
- Increase maxConcurrency
- Use batch API
- Reduce maxTokens
- Check API rate limits

### High Memory Usage
- Reduce cache size
- Call optimize() regularly
- Use streaming for large outputs
- Check for memory leaks

### Low Cache Hit Rate
- Normalize prompt formatting
- Increase cache size
- Increase TTL
- Review workload patterns

## Additional Resources

- [API Documentation](./API.md)
- [Examples](../examples/)
- [Benchmark Source](../src/benchmarks/)
- [GitHub Issues](https://github.com/ruvnet/ruvector/issues)
