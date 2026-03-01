/**
 * Benchmark usage example
 */

import { BenchmarkRunner } from '../src/benchmarks/runner.js';
import { ThroughputBenchmark } from '../src/benchmarks/throughput.js';
import { LatencyBenchmark } from '../src/benchmarks/latency.js';
import { MemoryBenchmark } from '../src/benchmarks/memory.js';
import { CacheBenchmark } from '../src/benchmarks/cache.js';
import { BenchmarkAnalyzer } from '../src/benchmarks/analyzer.js';
import { BenchmarkReporter } from '../src/benchmarks/reporter.js';
import { AgenticSynth } from '../src/index.js';

async function main() {
  console.log('🔥 Running Performance Benchmarks\n');

  // Initialize
  const synth = new AgenticSynth({
    enableCache: true,
    cacheSize: 1000,
    maxConcurrency: 100,
  });

  const runner = new BenchmarkRunner();
  const analyzer = new BenchmarkAnalyzer();
  const reporter = new BenchmarkReporter();

  // Register benchmark suites
  runner.registerSuite(new ThroughputBenchmark(synth));
  runner.registerSuite(new LatencyBenchmark(synth));
  runner.registerSuite(new MemoryBenchmark(synth));
  runner.registerSuite(new CacheBenchmark(synth));

  // Run benchmarks
  const result = await runner.runAll({
    name: 'Performance Test',
    iterations: 5,
    concurrency: 50,
    warmupIterations: 1,
    timeout: 300000,
  });

  // Analyze results
  analyzer.analyze(result);

  // Generate reports
  await reporter.generateMarkdown([result], 'benchmark-report.md');
  await reporter.generateJSON([result], 'benchmark-data.json');

  console.log('\n✅ Benchmarks complete!');
  console.log('📄 Reports saved to benchmark-report.md and benchmark-data.json');
}

main().catch(console.error);
