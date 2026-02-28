#!/usr/bin/env node

/**
 * Basic Spiking Neural Network Example
 *
 * Demonstrates the fundamental usage of the spiking-neural SDK.
 */

const {
  createFeedforwardSNN,
  rateEncoding,
  native,
  version
} = require('spiking-neural');

console.log(`\nSpiking Neural Network SDK v${version}`);
console.log(`Native SIMD: ${native ? 'Enabled' : 'JavaScript fallback'}\n`);
console.log('='.repeat(50));

// Create a 3-layer feedforward SNN
const snn = createFeedforwardSNN([100, 50, 10], {
  dt: 1.0,               // 1ms time step
  tau: 20.0,             // 20ms membrane time constant
  a_plus: 0.005,         // STDP LTP rate
  a_minus: 0.005,        // STDP LTD rate
  lateral_inhibition: true,
  inhibition_strength: 10.0
});

console.log('\nNetwork created: 100 -> 50 -> 10 neurons');
console.log(`Total synapses: ${100 * 50 + 50 * 10}`);

// Create input pattern (random)
const input_pattern = new Float32Array(100).map(() => Math.random());

console.log('\nRunning 100ms simulation...\n');

// Run for 100ms
let total_spikes = 0;
for (let t = 0; t < 100; t++) {
  // Encode input as spike train
  const spikes = rateEncoding(input_pattern, snn.dt, 100);
  total_spikes += snn.step(spikes);
}

// Get network statistics
const stats = snn.getStats();

console.log('Results:');
console.log(`  Simulation time: ${stats.time}ms`);
console.log(`  Total spikes: ${total_spikes}`);
console.log(`  Avg spikes/ms: ${(total_spikes / stats.time).toFixed(2)}`);

// Layer statistics
console.log('\nLayer Statistics:');
for (const layer of stats.layers) {
  if (layer.neurons) {
    console.log(`  Layer ${layer.index}: ${layer.neurons.count} neurons, ${layer.neurons.spike_count} current spikes`);
  }
  if (layer.synapses) {
    console.log(`    Weights: mean=${layer.synapses.mean.toFixed(3)}, range=[${layer.synapses.min.toFixed(3)}, ${layer.synapses.max.toFixed(3)}]`);
  }
}

// Get final output
const output = snn.getOutput();
console.log('\nOutput layer activity:', Array.from(output).map(v => v.toFixed(2)).join(', '));

console.log('\nDone!\n');
