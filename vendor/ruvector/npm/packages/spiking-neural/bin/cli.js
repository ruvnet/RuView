#!/usr/bin/env node

/**
 * Spiking Neural Network CLI
 * Usage: npx spiking-neural <command> [options]
 */

const {
  createFeedforwardSNN,
  rateEncoding,
  temporalEncoding,
  SIMDOps,
  native,
  version
} = require('../src/index');

const args = process.argv.slice(2);
const command = args[0] || 'help';

// ANSI colors
const c = {
  reset: '\x1b[0m',
  bold: '\x1b[1m',
  dim: '\x1b[2m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  cyan: '\x1b[36m',
  red: '\x1b[31m',
  magenta: '\x1b[35m'
};

function log(msg = '') { console.log(msg); }
function header(title) { log(`\n${c.bold}${c.cyan}${title}${c.reset}\n${'='.repeat(60)}`); }
function success(msg) { log(`${c.green}${msg}${c.reset}`); }
function warn(msg) { log(`${c.yellow}${msg}${c.reset}`); }
function info(msg) { log(`${c.blue}${msg}${c.reset}`); }

// Commands
const commands = {
  help: showHelp,
  version: showVersion,
  demo: runDemo,
  benchmark: runBenchmark,
  test: runTest,
  simd: runSIMDBenchmark,
  pattern: () => runDemo(['pattern']),
  train: runTrain,
  info: showInfo
};

async function main() {
  if (commands[command]) {
    await commands[command](args.slice(1));
  } else {
    log(`${c.red}Unknown command: ${command}${c.reset}`);
    showHelp();
    process.exit(1);
  }
}

function showHelp() {
  log(`
${c.bold}${c.cyan}Spiking Neural Network CLI${c.reset}
${c.dim}High-performance SNN with SIMD optimization${c.reset}

${c.bold}USAGE:${c.reset}
  npx spiking-neural <command> [options]
  snn <command> [options]

${c.bold}COMMANDS:${c.reset}
  ${c.green}demo${c.reset} <type>     Run a demonstration
                   Types: pattern, temporal, learning, all
  ${c.green}benchmark${c.reset}       Run performance benchmarks
  ${c.green}simd${c.reset}            Run SIMD vector operation benchmarks
  ${c.green}train${c.reset} [opts]    Train a custom SNN
  ${c.green}test${c.reset}            Run validation tests
  ${c.green}info${c.reset}            Show system information
  ${c.green}version${c.reset}         Show version
  ${c.green}help${c.reset}            Show this help

${c.bold}EXAMPLES:${c.reset}
  ${c.dim}# Run pattern recognition demo${c.reset}
  npx spiking-neural demo pattern

  ${c.dim}# Run full benchmark suite${c.reset}
  npx spiking-neural benchmark

  ${c.dim}# Train custom network${c.reset}
  npx spiking-neural train --layers 25,50,10 --epochs 5

${c.bold}SDK USAGE:${c.reset}
  const { createFeedforwardSNN, rateEncoding } = require('spiking-neural');

  const snn = createFeedforwardSNN([100, 50, 10], {
    dt: 1.0,
    tau: 20.0,
    lateral_inhibition: true
  });

  const spikes = rateEncoding(inputData, snn.dt, 100);
  snn.step(spikes);
`);
}

function showVersion() {
  log(`spiking-neural v${version}`);
  log(`Native SIMD: ${native ? 'enabled' : 'JavaScript fallback'}`);
}

function showInfo() {
  header('System Information');
  log(`
${c.bold}Package:${c.reset}        spiking-neural v${version}
${c.bold}Native SIMD:${c.reset}    ${native ? c.green + 'Enabled' : c.yellow + 'JavaScript fallback'}${c.reset}
${c.bold}Node.js:${c.reset}        ${process.version}
${c.bold}Platform:${c.reset}       ${process.platform} ${process.arch}
${c.bold}Memory:${c.reset}         ${Math.round(process.memoryUsage().heapUsed / 1024 / 1024)}MB used

${c.bold}Capabilities:${c.reset}
  - LIF Neurons with configurable parameters
  - STDP Learning (unsupervised)
  - Lateral Inhibition (winner-take-all)
  - Rate & Temporal Encoding
  - SIMD-optimized vector operations
  - Multi-layer feedforward networks
`);
}

async function runDemo(demoArgs) {
  const type = demoArgs[0] || 'pattern';

  if (type === 'pattern' || type === 'all') {
    await demoPatternRecognition();
  }
  if (type === 'temporal' || type === 'all') {
    await demoTemporalDynamics();
  }
  if (type === 'learning' || type === 'all') {
    await demoSTDPLearning();
  }
}

async function demoPatternRecognition() {
  header('Pattern Recognition Demo');

  // Define 5x5 patterns
  const patterns = {
    'Cross': [0,0,1,0,0, 0,0,1,0,0, 1,1,1,1,1, 0,0,1,0,0, 0,0,1,0,0],
    'Square': [1,1,1,1,1, 1,0,0,0,1, 1,0,0,0,1, 1,0,0,0,1, 1,1,1,1,1],
    'Diagonal': [1,0,0,0,0, 0,1,0,0,0, 0,0,1,0,0, 0,0,0,1,0, 0,0,0,0,1],
    'X-Shape': [1,0,0,0,1, 0,1,0,1,0, 0,0,1,0,0, 0,1,0,1,0, 1,0,0,0,1]
  };

  // Visualize patterns
  log(`\n${c.bold}Patterns:${c.reset}\n`);
  for (const [name, pattern] of Object.entries(patterns)) {
    log(`${c.cyan}${name}:${c.reset}`);
    for (let i = 0; i < 5; i++) {
      const row = pattern.slice(i * 5, (i + 1) * 5).map(v => v ? '##' : '  ').join('');
      log(`  ${row}`);
    }
    log();
  }

  // Create SNN with parameters tuned for spiking
  const snn = createFeedforwardSNN([25, 20, 4], {
    dt: 1.0,
    tau: 2.0,             // Fast integration for quick spiking
    v_rest: 0.0,          // Simplified: rest at 0
    v_reset: 0.0,         // Reset to 0
    v_thresh: 1.0,        // Threshold at 1
    resistance: 1.0,      // Direct current-to-voltage
    a_plus: 0.02,
    a_minus: 0.02,
    init_weight: 0.2,     // Strong enough for spike propagation
    init_std: 0.02,
    lateral_inhibition: false
  });

  log(`${c.bold}Network:${c.reset} 25-20-4 (${25*20 + 20*4} synapses)`);
  log(`${c.bold}Native SIMD:${c.reset} ${native ? c.green + 'Enabled' : c.yellow + 'Fallback'}${c.reset}\n`);

  // Training - use direct pattern as current (scaled)
  log(`${c.bold}Training (5 epochs):${c.reset}\n`);
  const pattern_names = Object.keys(patterns);
  const pattern_arrays = Object.values(patterns);

  for (let epoch = 0; epoch < 5; epoch++) {
    let total_spikes = 0;
    for (let p = 0; p < pattern_names.length; p++) {
      snn.reset();
      for (let t = 0; t < 50; t++) {
        // Scale pattern to produce spikes (current * 2 to exceed threshold)
        const input = new Float32Array(pattern_arrays[p].map(v => v * 2.0));
        total_spikes += snn.step(input);
      }
    }
    log(`  Epoch ${epoch + 1}: ${total_spikes} total spikes`);
  }

  // Testing
  log(`\n${c.bold}Testing:${c.reset}\n`);
  for (let p = 0; p < pattern_names.length; p++) {
    snn.reset();
    const output_activity = new Float32Array(4);
    for (let t = 0; t < 50; t++) {
      const input = new Float32Array(pattern_arrays[p].map(v => v * 2.0));
      snn.step(input);
      const output = snn.getOutput();
      for (let i = 0; i < 4; i++) output_activity[i] += output[i];
    }
    const winner = Array.from(output_activity).indexOf(Math.max(...output_activity));
    const total = output_activity.reduce((a, b) => a + b, 0);
    const confidence = total > 0 ? (output_activity[winner] / total * 100) : 0;
    log(`  ${pattern_names[p].padEnd(10)} -> Neuron ${winner} (${confidence.toFixed(1)}%)`);
  }

  success('\nPattern recognition complete!');
}

async function demoTemporalDynamics() {
  header('Temporal Dynamics Demo');

  const snn = createFeedforwardSNN([10, 10], {
    dt: 1.0,
    tau: 10.0,
    v_rest: 0.0,
    v_reset: 0.0,
    v_thresh: 1.0,
    resistance: 1.0
  });

  log(`\nSimulating 50ms with constant input:\n`);
  log('Time (ms) | Input Sum | Output Spikes');
  log('-'.repeat(40));

  const input_pattern = new Float32Array(10).fill(1.5); // Strong enough to spike

  for (let t = 0; t < 50; t += 5) {
    snn.step(input_pattern);

    const stats = snn.getStats();
    const in_sum = input_pattern.reduce((a, b) => a + b, 0);
    const out_count = stats.layers[1].neurons.spike_count;

    log(`${t.toString().padStart(9)} | ${in_sum.toFixed(1).padStart(9)} | ${out_count.toString().padStart(13)}`);
  }

  success('\nTemporal dynamics complete!');
}

async function demoSTDPLearning() {
  header('STDP Learning Demo');

  const snn = createFeedforwardSNN([10, 5], {
    dt: 1.0,
    tau: 10.0,
    v_rest: 0.0,
    v_reset: 0.0,
    v_thresh: 1.0,
    resistance: 1.0,
    a_plus: 0.02,
    a_minus: 0.02
  });

  log('\nWeight evolution during learning:\n');

  for (let epoch = 0; epoch < 10; epoch++) {
    const pattern = new Float32Array(10).map(() => Math.random() > 0.5 ? 2.0 : 0);

    for (let t = 0; t < 50; t++) {
      snn.step(pattern);
    }

    const stats = snn.getStats();
    const w = stats.layers[0].synapses;
    log(`  Epoch ${(epoch + 1).toString().padStart(2)}: mean=${w.mean.toFixed(3)}, min=${w.min.toFixed(3)}, max=${w.max.toFixed(3)}`);
  }

  success('\nSTDP learning complete!');
}

async function runBenchmark() {
  header('Performance Benchmark');

  const sizes = [100, 500, 1000, 2000];
  const iterations = 100;

  log(`\n${c.bold}Network Size Scaling:${c.reset}\n`);
  log('Neurons | Time/Step | Spikes/Step | Ops/Sec');
  log('-'.repeat(50));

  for (const size of sizes) {
    const snn = createFeedforwardSNN([size, Math.floor(size / 2), 10], {
      dt: 1.0,
      tau: 10.0,
      v_rest: 0.0,
      v_reset: 0.0,
      v_thresh: 1.0,
      resistance: 1.0,
      lateral_inhibition: false
    });

    const input = new Float32Array(size).fill(1.5);

    // Warmup
    for (let i = 0; i < 10; i++) {
      snn.step(input);
    }

    // Benchmark
    const start = performance.now();
    let total_spikes = 0;
    for (let i = 0; i < iterations; i++) {
      total_spikes += snn.step(input);
    }
    const elapsed = performance.now() - start;

    const time_per_step = elapsed / iterations;
    const spikes_per_step = total_spikes / iterations;
    const ops_per_sec = Math.round(1000 / time_per_step);

    log(`${size.toString().padStart(7)} | ${time_per_step.toFixed(3).padStart(9)}ms | ${spikes_per_step.toFixed(1).padStart(11)} | ${ops_per_sec.toString().padStart(7)}`);
  }

  log(`\n${c.bold}Native SIMD:${c.reset} ${native ? c.green + 'Enabled (10-50x faster)' : c.yellow + 'Disabled (use npm run build:native)'}${c.reset}`);

  success('\nBenchmark complete!');
}

async function runSIMDBenchmark() {
  header('SIMD Vector Operations Benchmark');

  const dimensions = [64, 128, 256, 512];
  const iterations = 10000;

  log(`\n${c.bold}Dot Product Performance:${c.reset}\n`);
  log('Dimension | Naive (ms) | SIMD (ms) | Speedup');
  log('-'.repeat(50));

  for (const dim of dimensions) {
    const a = new Float32Array(dim).map(() => Math.random());
    const b = new Float32Array(dim).map(() => Math.random());

    // Naive
    let start = performance.now();
    for (let i = 0; i < iterations; i++) {
      let sum = 0;
      for (let j = 0; j < dim; j++) sum += a[j] * b[j];
    }
    const naiveTime = performance.now() - start;

    // SIMD
    start = performance.now();
    for (let i = 0; i < iterations; i++) {
      SIMDOps.dotProduct(a, b);
    }
    const simdTime = performance.now() - start;

    const speedup = naiveTime / simdTime;
    log(`${dim.toString().padStart(9)} | ${naiveTime.toFixed(2).padStart(10)} | ${simdTime.toFixed(2).padStart(9)} | ${speedup.toFixed(2)}x${speedup > 1.2 ? ' *' : ''}`);
  }

  log(`\n${c.bold}Euclidean Distance:${c.reset}\n`);
  log('Dimension | Naive (ms) | SIMD (ms) | Speedup');
  log('-'.repeat(50));

  for (const dim of dimensions) {
    const a = new Float32Array(dim).map(() => Math.random());
    const b = new Float32Array(dim).map(() => Math.random());

    // Naive
    let start = performance.now();
    for (let i = 0; i < iterations; i++) {
      let sum = 0;
      for (let j = 0; j < dim; j++) {
        const d = a[j] - b[j];
        sum += d * d;
      }
      Math.sqrt(sum);
    }
    const naiveTime = performance.now() - start;

    // SIMD
    start = performance.now();
    for (let i = 0; i < iterations; i++) {
      SIMDOps.distance(a, b);
    }
    const simdTime = performance.now() - start;

    const speedup = naiveTime / simdTime;
    log(`${dim.toString().padStart(9)} | ${naiveTime.toFixed(2).padStart(10)} | ${simdTime.toFixed(2).padStart(9)} | ${speedup.toFixed(2)}x${speedup > 1.5 ? ' **' : speedup > 1.2 ? ' *' : ''}`);
  }

  success('\nSIMD benchmark complete!');
}

async function runTest() {
  header('Validation Tests');

  let passed = 0;
  let failed = 0;

  function test(name, fn) {
    try {
      fn();
      log(`  ${c.green}PASS${c.reset} ${name}`);
      passed++;
    } catch (e) {
      log(`  ${c.red}FAIL${c.reset} ${name}: ${e.message}`);
      failed++;
    }
  }

  function assert(condition, msg) {
    if (!condition) throw new Error(msg);
  }

  log('\n');

  // LIF Layer tests
  test('LIFLayer creation', () => {
    const layer = require('../src/index').LIFLayer;
    const l = new layer(10);
    assert(l.n_neurons === 10, 'Wrong neuron count');
    assert(l.voltages.length === 10, 'Wrong voltage array size');
  });

  test('LIFLayer update', () => {
    const { LIFLayer } = require('../src/index');
    const l = new LIFLayer(10);
    l.currents.fill(100); // Strong input
    const spikes = l.update();
    assert(typeof spikes === 'number', 'Update should return spike count');
  });

  // Synaptic Layer tests
  test('SynapticLayer creation', () => {
    const { SynapticLayer } = require('../src/index');
    const s = new SynapticLayer(10, 5);
    assert(s.weights.length === 50, 'Wrong weight matrix size');
  });

  test('SynapticLayer forward', () => {
    const { SynapticLayer } = require('../src/index');
    const s = new SynapticLayer(10, 5);
    const pre = new Float32Array(10).fill(1);
    const post = new Float32Array(5);
    s.forward(pre, post);
    assert(post.some(v => v !== 0), 'Forward should produce output');
  });

  // Network tests
  test('createFeedforwardSNN', () => {
    const snn = createFeedforwardSNN([10, 5, 2]);
    assert(snn.layers.length === 3, 'Wrong layer count');
  });

  test('SNN step', () => {
    const snn = createFeedforwardSNN([10, 5, 2]);
    const input = new Float32Array(10).fill(1);
    const spikes = snn.step(input);
    assert(typeof spikes === 'number', 'Step should return spike count');
  });

  test('SNN getOutput', () => {
    const snn = createFeedforwardSNN([10, 5, 2]);
    snn.step(new Float32Array(10).fill(1));
    const output = snn.getOutput();
    assert(output.length === 2, 'Output should match last layer size');
  });

  test('SNN reset', () => {
    const snn = createFeedforwardSNN([10, 5, 2]);
    snn.step(new Float32Array(10).fill(1));
    snn.reset();
    assert(snn.time === 0, 'Reset should zero time');
  });

  // Encoding tests
  test('rateEncoding', () => {
    const spikes = rateEncoding([1, 0, 0.5], 1.0, 100);
    assert(spikes.length === 3, 'Output should match input length');
    assert(spikes.every(v => v === 0 || v === 1), 'Should produce binary spikes');
  });

  test('temporalEncoding', () => {
    const spikes = temporalEncoding([1, 0, 0.5], 0, 0, 50);
    assert(spikes.length === 3, 'Output should match input length');
  });

  // SIMD tests
  test('SIMDOps.dotProduct', () => {
    const a = new Float32Array([1, 2, 3, 4]);
    const b = new Float32Array([1, 1, 1, 1]);
    const result = SIMDOps.dotProduct(a, b);
    assert(Math.abs(result - 10) < 0.001, `Expected 10, got ${result}`);
  });

  test('SIMDOps.distance', () => {
    const a = new Float32Array([0, 0, 0]);
    const b = new Float32Array([3, 4, 0]);
    const result = SIMDOps.distance(a, b);
    assert(Math.abs(result - 5) < 0.001, `Expected 5, got ${result}`);
  });

  log('\n' + '-'.repeat(40));
  log(`${c.bold}Results:${c.reset} ${c.green}${passed} passed${c.reset}, ${failed > 0 ? c.red : c.dim}${failed} failed${c.reset}`);

  if (failed > 0) process.exit(1);
  success('\nAll tests passed!');
}

async function runTrain(trainArgs) {
  // Parse arguments
  let layers = [25, 20, 4];
  let epochs = 5;

  for (let i = 0; i < trainArgs.length; i++) {
    if (trainArgs[i] === '--layers' && trainArgs[i + 1]) {
      layers = trainArgs[i + 1].split(',').map(Number);
      i++;
    }
    if (trainArgs[i] === '--epochs' && trainArgs[i + 1]) {
      epochs = parseInt(trainArgs[i + 1]);
      i++;
    }
  }

  header(`Training SNN [${layers.join('-')}]`);

  const snn = createFeedforwardSNN(layers, {
    dt: 1.0,
    tau: 20.0,
    a_plus: 0.005,
    a_minus: 0.005,
    lateral_inhibition: true
  });

  const input_size = layers[0];
  log(`\n${c.bold}Configuration:${c.reset}`);
  log(`  Layers: ${layers.join(' -> ')}`);
  log(`  Epochs: ${epochs}`);
  log(`  Learning: STDP (unsupervised)`);
  log(`  Native SIMD: ${native ? 'enabled' : 'disabled'}\n`);

  log(`${c.bold}Training:${c.reset}\n`);

  for (let epoch = 0; epoch < epochs; epoch++) {
    let total_spikes = 0;

    // Generate random patterns for each epoch
    for (let p = 0; p < 10; p++) {
      const pattern = new Float32Array(input_size).map(() => Math.random() > 0.5 ? 1 : 0);
      snn.reset();

      for (let t = 0; t < 100; t++) {
        const input = rateEncoding(pattern, snn.dt, 100);
        total_spikes += snn.step(input);
      }
    }

    const stats = snn.getStats();
    const w = stats.layers[0].synapses;
    log(`  Epoch ${epoch + 1}/${epochs}: ${total_spikes} spikes, weights: mean=${w.mean.toFixed(3)}, range=[${w.min.toFixed(3)}, ${w.max.toFixed(3)}]`);
  }

  success('\nTraining complete!');
}

main().catch(err => {
  console.error(`${c.red}Error:${c.reset}`, err.message);
  process.exit(1);
});
