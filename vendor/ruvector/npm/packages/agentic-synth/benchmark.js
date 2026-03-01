#!/usr/bin/env node

/**
 * Comprehensive Benchmark Suite for agentic-synth
 * Tests: Cache performance, generation speed, memory usage, throughput
 */

import { performance } from 'perf_hooks';
import { AgenticSynth } from './dist/index.js';
import { CacheManager } from './dist/cache/index.js';

// Color codes for terminal output
const colors = {
  reset: '\x1b[0m',
  bright: '\x1b[1m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  cyan: '\x1b[36m'
};

const c = (color, text) => `${colors[color]}${text}${colors.reset}`;

console.log(c('cyan', '\n═══════════════════════════════════════════════════'));
console.log(c('bright', '   Agentic-Synth Benchmark Suite'));
console.log(c('cyan', '═══════════════════════════════════════════════════\n'));

// Benchmark utilities
class BenchmarkRunner {
  constructor() {
    this.results = [];
  }

  async run(name, fn, iterations = 100) {
    console.log(c('blue', `\n📊 Running: ${name}`));
    console.log(c('yellow', `   Iterations: ${iterations}`));

    const times = [];
    const memoryBefore = process.memoryUsage();

    for (let i = 0; i < iterations; i++) {
      const start = performance.now();
      await fn();
      const end = performance.now();
      times.push(end - start);
    }

    const memoryAfter = process.memoryUsage();

    const sorted = times.sort((a, b) => a - b);
    const stats = {
      name,
      iterations,
      min: sorted[0],
      max: sorted[sorted.length - 1],
      mean: times.reduce((a, b) => a + b, 0) / times.length,
      median: sorted[Math.floor(sorted.length / 2)],
      p95: sorted[Math.floor(sorted.length * 0.95)],
      p99: sorted[Math.floor(sorted.length * 0.99)],
      memoryDelta: {
        heapUsed: (memoryAfter.heapUsed - memoryBefore.heapUsed) / 1024 / 1024,
        rss: (memoryAfter.rss - memoryBefore.rss) / 1024 / 1024
      }
    };

    this.results.push(stats);
    this.printStats(stats);

    return stats;
  }

  printStats(stats) {
    console.log(c('green', '   ✓ Complete'));
    console.log(`   Min: ${c('cyan', stats.min.toFixed(2))}ms`);
    console.log(`   Mean: ${c('cyan', stats.mean.toFixed(2))}ms`);
    console.log(`   Median: ${c('cyan', stats.median.toFixed(2))}ms`);
    console.log(`   P95: ${c('cyan', stats.p95.toFixed(2))}ms`);
    console.log(`   P99: ${c('cyan', stats.p99.toFixed(2))}ms`);
    console.log(`   Max: ${c('cyan', stats.max.toFixed(2))}ms`);
    console.log(`   Memory Δ: ${c('yellow', stats.memoryDelta.heapUsed.toFixed(2))}MB heap`);
  }

  summary() {
    console.log(c('cyan', '\n═══════════════════════════════════════════════════'));
    console.log(c('bright', '   Benchmark Summary'));
    console.log(c('cyan', '═══════════════════════════════════════════════════\n'));

    console.log(c('bright', 'Performance Results:\n'));

    const table = this.results.map(r => ({
      'Test': r.name.substring(0, 40),
      'Mean': `${r.mean.toFixed(2)}ms`,
      'P95': `${r.p95.toFixed(2)}ms`,
      'P99': `${r.p99.toFixed(2)}ms`,
      'Memory': `${r.memoryDelta.heapUsed.toFixed(2)}MB`
    }));

    console.table(table);

    // Performance ratings
    console.log(c('bright', '\nPerformance Ratings:\n'));

    this.results.forEach(r => {
      let rating = '⭐⭐⭐⭐⭐';
      let status = c('green', 'EXCELLENT');

      if (r.p99 > 1000) {
        rating = '⭐⭐⭐';
        status = c('yellow', 'ACCEPTABLE');
      }
      if (r.p99 > 2000) {
        rating = '⭐⭐';
        status = c('red', 'NEEDS OPTIMIZATION');
      }

      console.log(`   ${rating} ${r.name.substring(0, 35).padEnd(35)} - ${status}`);
    });

    // Recommendations
    console.log(c('bright', '\n\nOptimization Recommendations:\n'));

    const slowTests = this.results.filter(r => r.p99 > 100);
    if (slowTests.length === 0) {
      console.log(c('green', '   ✓ All benchmarks performing excellently!'));
    } else {
      slowTests.forEach(r => {
        console.log(c('yellow', `   ⚠ ${r.name}:`));
        if (r.p99 > 1000) {
          console.log('     - Consider adding caching');
          console.log('     - Optimize algorithm complexity');
        }
        if (r.memoryDelta.heapUsed > 50) {
          console.log('     - High memory usage detected');
          console.log('     - Consider memory pooling');
        }
      });
    }

    console.log(c('cyan', '\n═══════════════════════════════════════════════════\n'));
  }
}

// Benchmark tests
async function runBenchmarks() {
  const runner = new BenchmarkRunner();

  console.log(c('yellow', 'Preparing benchmark environment...\n'));

  // 1. Cache performance benchmarks
  console.log(c('bright', '1️⃣ CACHE PERFORMANCE'));

  const cache = new CacheManager({
    strategy: 'memory',
    ttl: 3600,
    maxSize: 1000
  });

  await runner.run('Cache: Set operation', async () => {
    await cache.set(`key-${Math.random()}`, { data: 'test-value' });
  }, 1000);

  // Pre-populate cache
  for (let i = 0; i < 100; i++) {
    await cache.set(`test-key-${i}`, { data: `value-${i}` });
  }

  await runner.run('Cache: Get operation (hit)', async () => {
    await cache.get(`test-key-${Math.floor(Math.random() * 100)}`);
  }, 1000);

  await runner.run('Cache: Get operation (miss)', async () => {
    await cache.get(`missing-key-${Math.random()}`);
  }, 1000);

  await runner.run('Cache: Has operation', async () => {
    await cache.has(`test-key-${Math.floor(Math.random() * 100)}`);
  }, 1000);

  // 2. Configuration benchmarks
  console.log(c('bright', '\n2️⃣ CONFIGURATION & INITIALIZATION'));

  await runner.run('AgenticSynth: Initialization', async () => {
    const synth = new AgenticSynth({
      provider: 'gemini',
      apiKey: 'test-key',
      cacheStrategy: 'memory'
    });
  }, 100);

  const synth = new AgenticSynth({
    provider: 'gemini',
    apiKey: 'test-key',
    cacheStrategy: 'memory'
  });

  await runner.run('AgenticSynth: Get config', async () => {
    synth.getConfig();
  }, 1000);

  await runner.run('AgenticSynth: Update config', async () => {
    synth.configure({ cacheTTL: Math.floor(Math.random() * 10000) });
  }, 100);

  // 3. Type validation benchmarks
  console.log(c('bright', '\n3️⃣ TYPE VALIDATION'));

  const { SynthConfigSchema } = await import('./dist/index.js');

  await runner.run('Zod: Config validation (valid)', async () => {
    SynthConfigSchema.parse({
      provider: 'gemini',
      apiKey: 'test',
      cacheStrategy: 'memory'
    });
  }, 1000);

  await runner.run('Zod: Config validation (with defaults)', async () => {
    SynthConfigSchema.parse({
      provider: 'gemini'
    });
  }, 1000);

  // 4. Data structure operations
  console.log(c('bright', '\n4️⃣ DATA STRUCTURE OPERATIONS'));

  const testData = Array.from({ length: 100 }, (_, i) => ({
    id: i,
    name: `user-${i}`,
    email: `user${i}@example.com`,
    age: 20 + (i % 50)
  }));

  await runner.run('JSON: Stringify large object', async () => {
    JSON.stringify(testData);
  }, 1000);

  await runner.run('JSON: Parse large object', async () => {
    JSON.parse(JSON.stringify(testData));
  }, 1000);

  // 5. Cache key generation
  console.log(c('bright', '\n5️⃣ CACHE KEY GENERATION'));

  await runner.run('CacheManager: Generate key (simple)', async () => {
    CacheManager.generateKey('test', { id: 1, type: 'simple' });
  }, 1000);

  await runner.run('CacheManager: Generate key (complex)', async () => {
    CacheManager.generateKey('test', {
      id: 1,
      type: 'complex',
      schema: { name: 'string', age: 'number' },
      options: { count: 10, format: 'json' }
    });
  }, 1000);

  // 6. Memory stress test
  console.log(c('bright', '\n6️⃣ MEMORY STRESS TEST'));

  await runner.run('Memory: Large cache operations', async () => {
    const tempCache = new CacheManager({
      strategy: 'memory',
      ttl: 3600,
      maxSize: 1000
    });

    for (let i = 0; i < 100; i++) {
      await tempCache.set(`key-${i}`, { data: new Array(100).fill(i) });
    }
  }, 10);

  // 7. Concurrent operations
  console.log(c('bright', '\n7️⃣ CONCURRENT OPERATIONS'));

  await runner.run('Concurrency: Parallel cache reads', async () => {
    await Promise.all(
      Array.from({ length: 10 }, (_, i) =>
        cache.get(`test-key-${i}`)
      )
    );
  }, 100);

  await runner.run('Concurrency: Parallel cache writes', async () => {
    await Promise.all(
      Array.from({ length: 10 }, (_, i) =>
        cache.set(`concurrent-${i}`, { value: i })
      )
    );
  }, 100);

  // Print summary
  runner.summary();

  // Export results
  const results = {
    timestamp: new Date().toISOString(),
    benchmarks: runner.results,
    environment: {
      nodeVersion: process.version,
      platform: process.platform,
      arch: process.arch,
      memory: process.memoryUsage()
    }
  };

  return results;
}

// Run benchmarks
runBenchmarks()
  .then(results => {
    // Save results to file
    import('fs').then(fs => {
      fs.default.writeFileSync(
        'benchmark-results.json',
        JSON.stringify(results, null, 2)
      );
      console.log(c('green', '✅ Results saved to benchmark-results.json\n'));
    });

    process.exit(0);
  })
  .catch(error => {
    console.error(c('red', '\n❌ Benchmark failed:'), error);
    process.exit(1);
  });
