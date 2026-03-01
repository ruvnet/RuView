#!/usr/bin/env node
/**
 * RuVector Hooks Performance Benchmark
 *
 * Measures performance of all hook operations to identify bottlenecks
 */

const { execSync } = require('child_process');
const path = require('path');

const CLI = path.join(__dirname, '../bin/cli.js');

// Benchmark configuration
const ITERATIONS = 10;
const WARMUP = 2;

// Results storage
const results = {};

function runCommand(cmd, silent = true) {
  const start = performance.now();
  try {
    execSync(`node ${CLI} ${cmd}`, {
      stdio: silent ? 'pipe' : 'inherit',
      timeout: 30000
    });
    return performance.now() - start;
  } catch (e) {
    return performance.now() - start;
  }
}

function benchmark(name, cmd, iterations = ITERATIONS) {
  console.log(`\nBenchmarking: ${name}`);

  // Warmup
  for (let i = 0; i < WARMUP; i++) {
    runCommand(cmd);
  }

  // Actual benchmark
  const times = [];
  for (let i = 0; i < iterations; i++) {
    const time = runCommand(cmd);
    times.push(time);
    process.stdout.write(`  Run ${i + 1}/${iterations}: ${time.toFixed(1)}ms\r`);
  }

  const avg = times.reduce((a, b) => a + b, 0) / times.length;
  const min = Math.min(...times);
  const max = Math.max(...times);
  const p95 = times.sort((a, b) => a - b)[Math.floor(times.length * 0.95)];

  results[name] = { avg, min, max, p95, times };
  console.log(`  Avg: ${avg.toFixed(1)}ms | Min: ${min.toFixed(1)}ms | Max: ${max.toFixed(1)}ms | P95: ${p95.toFixed(1)}ms`);

  return avg;
}

async function main() {
  console.log('='.repeat(60));
  console.log('RuVector Hooks Performance Benchmark');
  console.log('='.repeat(60));
  console.log(`Iterations: ${ITERATIONS} | Warmup: ${WARMUP}`);

  // Session operations
  console.log('\n--- Session Operations ---');
  benchmark('session-start', 'hooks session-start');
  benchmark('session-end', 'hooks session-end');

  // Memory operations (non-semantic)
  console.log('\n--- Memory Operations (Hash Embeddings) ---');
  benchmark('remember (hash)', 'hooks remember "Test memory content for benchmarking" -t benchmark');
  benchmark('recall (hash)', 'hooks recall "test benchmark" -k 5');

  // Stats and routing
  console.log('\n--- Stats & Routing ---');
  benchmark('stats', 'hooks stats');
  benchmark('route', 'hooks route "implement feature" --file src/test.ts');
  benchmark('suggest-context', 'hooks suggest-context');

  // Pre/Post hooks
  console.log('\n--- Pre/Post Edit Hooks ---');
  benchmark('pre-edit', 'hooks pre-edit /tmp/test.js');
  benchmark('post-edit (success)', 'hooks post-edit /tmp/test.js --success');
  benchmark('post-edit (failure)', 'hooks post-edit /tmp/test.js');

  // Pre/Post command hooks
  console.log('\n--- Pre/Post Command Hooks ---');
  benchmark('pre-command', 'hooks pre-command "npm test"');
  benchmark('post-command (success)', 'hooks post-command "npm test" --success');

  // Trajectory operations (using correct CLI args)
  console.log('\n--- Trajectory Operations ---');
  benchmark('trajectory-begin', 'hooks trajectory-begin -c "benchmark task" -a tester');
  benchmark('trajectory-step', 'hooks trajectory-step -a "step action" -r "step result"');
  benchmark('trajectory-end (success)', 'hooks trajectory-end --success --quality 1.0');

  // Co-edit operations (using correct CLI args)
  console.log('\n--- Co-Edit Operations ---');
  benchmark('coedit-record', 'hooks coedit-record -p file1.js -r file2.js');
  benchmark('coedit-suggest', 'hooks coedit-suggest -f src/index.ts');

  // Error operations (using correct CLI args)
  console.log('\n--- Error Operations ---');
  benchmark('error-record', 'hooks error-record -e "TypeError: undefined" -x "Add null check"');
  benchmark('error-suggest', 'hooks error-suggest -e "Cannot read property"');

  // Learning operations
  console.log('\n--- Learning Operations ---');
  benchmark('force-learn', 'hooks force-learn');

  // Summary
  console.log('\n' + '='.repeat(60));
  console.log('PERFORMANCE SUMMARY');
  console.log('='.repeat(60));

  const sorted = Object.entries(results)
    .sort((a, b) => b[1].avg - a[1].avg);

  console.log('\nSlowest operations:');
  sorted.slice(0, 5).forEach(([name, r], i) => {
    console.log(`  ${i + 1}. ${name}: ${r.avg.toFixed(1)}ms avg`);
  });

  console.log('\nFastest operations:');
  sorted.slice(-5).reverse().forEach(([name, r], i) => {
    console.log(`  ${i + 1}. ${name}: ${r.avg.toFixed(1)}ms avg`);
  });

  // Identify bottlenecks (>100ms)
  const bottlenecks = sorted.filter(([_, r]) => r.avg > 100);
  if (bottlenecks.length > 0) {
    console.log('\n⚠️  BOTTLENECKS (>100ms):');
    bottlenecks.forEach(([name, r]) => {
      console.log(`  - ${name}: ${r.avg.toFixed(1)}ms`);
    });
  }

  // Total time for all operations
  const total = Object.values(results).reduce((sum, r) => sum + r.avg, 0);
  console.log(`\nTotal benchmark time: ${total.toFixed(1)}ms`);

  // Output JSON for further analysis
  console.log('\n--- JSON Results ---');
  console.log(JSON.stringify(results, null, 2));
}

main().catch(console.error);
