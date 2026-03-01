#!/usr/bin/env node

/**
 * Pattern Recognition with Spiking Neural Networks
 *
 * This example demonstrates:
 * - Rate-coded input encoding
 * - STDP learning (unsupervised)
 * - Pattern classification
 * - Lateral inhibition for winner-take-all
 */

const {
  createFeedforwardSNN,
  rateEncoding,
  native,
  version
} = require('spiking-neural');

console.log(`\nPattern Recognition with SNNs v${version}`);
console.log(`Native SIMD: ${native ? 'Enabled' : 'JavaScript fallback'}\n`);
console.log('='.repeat(60));

// Define 5x5 patterns
const patterns = {
  'Cross': [
    0, 0, 1, 0, 0,
    0, 0, 1, 0, 0,
    1, 1, 1, 1, 1,
    0, 0, 1, 0, 0,
    0, 0, 1, 0, 0
  ],
  'Square': [
    1, 1, 1, 1, 1,
    1, 0, 0, 0, 1,
    1, 0, 0, 0, 1,
    1, 0, 0, 0, 1,
    1, 1, 1, 1, 1
  ],
  'Diagonal': [
    1, 0, 0, 0, 0,
    0, 1, 0, 0, 0,
    0, 0, 1, 0, 0,
    0, 0, 0, 1, 0,
    0, 0, 0, 0, 1
  ],
  'X-Shape': [
    1, 0, 0, 0, 1,
    0, 1, 0, 1, 0,
    0, 0, 1, 0, 0,
    0, 1, 0, 1, 0,
    1, 0, 0, 0, 1
  ]
};

// Visualize patterns
console.log('\nPatterns:\n');
for (const [name, pattern] of Object.entries(patterns)) {
  console.log(`${name}:`);
  for (let i = 0; i < 5; i++) {
    const row = pattern.slice(i * 5, (i + 1) * 5).map(v => v ? '##' : '  ').join('');
    console.log(`  ${row}`);
  }
  console.log();
}

// Create SNN
const n_input = 25;   // 5x5 pixels
const n_hidden = 20;  // Hidden layer
const n_output = 4;   // 4 pattern classes

const snn = createFeedforwardSNN([n_input, n_hidden, n_output], {
  dt: 1.0,
  tau: 20.0,
  v_thresh: -50.0,
  v_reset: -70.0,
  a_plus: 0.005,
  a_minus: 0.005,
  init_weight: 0.3,
  init_std: 0.1,
  lateral_inhibition: true,
  inhibition_strength: 15.0
});

console.log(`Network: ${n_input}-${n_hidden}-${n_output} (${n_input * n_hidden + n_hidden * n_output} synapses)`);
console.log(`Learning: STDP (unsupervised)`);

// Training
console.log('\n--- TRAINING ---\n');

const n_epochs = 5;
const presentation_time = 100;
const pattern_names = Object.keys(patterns);
const pattern_arrays = Object.values(patterns);

for (let epoch = 0; epoch < n_epochs; epoch++) {
  let total_spikes = 0;

  for (let p = 0; p < pattern_names.length; p++) {
    const pattern = pattern_arrays[p];
    snn.reset();

    for (let t = 0; t < presentation_time; t++) {
      const input_spikes = rateEncoding(pattern, snn.dt, 100);
      total_spikes += snn.step(input_spikes);
    }
  }

  const stats = snn.getStats();
  const w = stats.layers[0].synapses;
  console.log(`Epoch ${epoch + 1}/${n_epochs}: ${total_spikes} spikes, weights: mean=${w.mean.toFixed(3)}`);
}

// Testing
console.log('\n--- TESTING ---\n');

const results = [];
for (let p = 0; p < pattern_names.length; p++) {
  const pattern = pattern_arrays[p];
  snn.reset();

  const output_activity = new Float32Array(n_output);

  for (let t = 0; t < presentation_time; t++) {
    const input_spikes = rateEncoding(pattern, snn.dt, 100);
    snn.step(input_spikes);

    const output = snn.getOutput();
    for (let i = 0; i < n_output; i++) {
      output_activity[i] += output[i];
    }
  }

  const winner = Array.from(output_activity).indexOf(Math.max(...output_activity));
  const total = output_activity.reduce((a, b) => a + b, 0);
  const confidence = total > 0 ? (output_activity[winner] / total * 100) : 0;

  results.push({ pattern: pattern_names[p], winner, confidence });
  console.log(`${pattern_names[p].padEnd(10)} -> Neuron ${winner} (${confidence.toFixed(1)}% confidence)`);
}

// Noise test
console.log('\n--- ROBUSTNESS (20% noise) ---\n');

function addNoise(pattern, noise_level = 0.2) {
  return pattern.map(v => Math.random() < noise_level ? 1 - v : v);
}

for (let p = 0; p < pattern_names.length; p++) {
  const noisy_pattern = addNoise(pattern_arrays[p], 0.2);
  snn.reset();

  const output_activity = new Float32Array(n_output);

  for (let t = 0; t < presentation_time; t++) {
    const input_spikes = rateEncoding(noisy_pattern, snn.dt, 100);
    snn.step(input_spikes);

    const output = snn.getOutput();
    for (let i = 0; i < n_output; i++) {
      output_activity[i] += output[i];
    }
  }

  const winner = Array.from(output_activity).indexOf(Math.max(...output_activity));
  const correct = winner === results[p].winner;

  console.log(`${pattern_names[p].padEnd(10)} -> Neuron ${winner} ${correct ? '✓' : '✗'}`);
}

console.log('\nDone!\n');
