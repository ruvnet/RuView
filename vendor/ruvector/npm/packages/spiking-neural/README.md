# spiking-neural

A high-performance Spiking Neural Network (SNN) library for Node.js with biologically-inspired neuron models and unsupervised learning. SNNs process information through discrete spike events rather than continuous values, making them more energy-efficient and better suited for temporal pattern recognition.

[![npm version](https://badge.fury.io/js/spiking-neural.svg)](https://www.npmjs.com/package/spiking-neural)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## What Are Spiking Neural Networks?

Unlike traditional neural networks that use continuous activation values, SNNs communicate through discrete spike events timed in milliseconds. This mirrors how biological neurons work, enabling:

- **Temporal pattern recognition** - naturally process time-series data
- **Energy efficiency** - event-driven computation (10-100x lower power)
- **Online learning** - adapt in real-time with STDP (no batches needed)
- **Neuromorphic deployment** - run on specialized hardware like Intel Loihi

## Key Features

| Feature | Description |
|---------|-------------|
| **LIF Neurons** | Leaky Integrate-and-Fire model with configurable membrane dynamics |
| **STDP Learning** | Spike-Timing-Dependent Plasticity for unsupervised pattern learning |
| **Lateral Inhibition** | Winner-take-all competition for sparse representations |
| **SIMD Optimization** | Loop-unrolled vector math for 5-54x speedup |
| **Multiple Encodings** | Rate coding and temporal coding for flexible input handling |
| **Zero Dependencies** | Pure JavaScript SDK works everywhere Node.js runs |

## Installation

```bash
npm install spiking-neural
```

**Note**: This package is pure JavaScript and works out of the box. No native compilation required.

## Quick Start

### CLI Usage

```bash
# Run pattern recognition demo
npx spiking-neural demo pattern

# Run performance benchmarks
npx spiking-neural benchmark

# Run SIMD vector operation benchmarks
npx spiking-neural simd

# Run validation tests
npx spiking-neural test

# Show help
npx spiking-neural help
```

### SDK Usage

```javascript
const {
  createFeedforwardSNN,
  rateEncoding,
  native
} = require('spiking-neural');

// Create a 3-layer feedforward SNN
const snn = createFeedforwardSNN([100, 50, 10], {
  dt: 1.0,               // 1ms time step
  tau: 20.0,             // 20ms membrane time constant
  a_plus: 0.005,         // STDP LTP rate
  a_minus: 0.005,        // STDP LTD rate
  lateral_inhibition: true,
  inhibition_strength: 10.0
});

// Create input pattern
const input = new Float32Array(100).fill(0.5);

// Run simulation
for (let t = 0; t < 100; t++) {
  const spikes = rateEncoding(input, snn.dt, 100);
  snn.step(spikes);
}

// Get output
const output = snn.getOutput();
console.log('Output:', output);
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `demo <type>` | Run demonstration (pattern, temporal, learning, all) |
| `benchmark` | Run SNN performance benchmarks |
| `simd` | Run SIMD vector operation benchmarks |
| `train` | Train a custom SNN |
| `test` | Run validation tests |
| `info` | Show system information |
| `version` | Show version |
| `help` | Show help |

### Examples

```bash
# Pattern recognition with 5x5 pixel patterns
npx spiking-neural demo pattern

# Train custom network
npx spiking-neural train --layers 25,50,10 --epochs 10

# All demos
npx spiking-neural demo all
```

## API Reference

### `createFeedforwardSNN(layer_sizes, params)`

Create a feedforward spiking neural network.

```javascript
const snn = createFeedforwardSNN([100, 50, 10], {
  dt: 1.0,                    // Time step (ms)
  tau: 20.0,                  // Membrane time constant (ms)
  v_rest: -70.0,              // Resting potential (mV)
  v_reset: -75.0,             // Reset potential (mV)
  v_thresh: -50.0,            // Spike threshold (mV)
  resistance: 10.0,           // Membrane resistance (MOhm)
  a_plus: 0.01,               // STDP LTP learning rate
  a_minus: 0.01,              // STDP LTD learning rate
  w_min: 0.0,                 // Minimum weight
  w_max: 1.0,                 // Maximum weight
  init_weight: 0.5,           // Initial weight mean
  init_std: 0.1,              // Initial weight std
  lateral_inhibition: false,  // Enable winner-take-all
  inhibition_strength: 10.0   // Inhibition strength
});
```

### `SpikingNeuralNetwork`

```javascript
// Run one time step
const spike_count = snn.step(input_spikes);

// Run for duration
const results = snn.run(100, (time) => inputGenerator(time));

// Get output spikes
const output = snn.getOutput();

// Get network statistics
const stats = snn.getStats();

// Reset network
snn.reset();
```

### `rateEncoding(values, dt, max_rate)`

Encode values as Poisson spike trains.

```javascript
const input = new Float32Array([0.5, 0.8, 0.2]);
const spikes = rateEncoding(input, 1.0, 100); // 100 Hz max rate
```

### `temporalEncoding(values, time, t_start, t_window)`

Encode values as time-to-first-spike.

```javascript
const input = new Float32Array([0.5, 0.8, 0.2]);
const spikes = temporalEncoding(input, currentTime, 0, 50);
```

### `SIMDOps`

SIMD-optimized vector operations.

```javascript
const { SIMDOps } = require('spiking-neural');

const a = new Float32Array([1, 2, 3, 4]);
const b = new Float32Array([4, 3, 2, 1]);

SIMDOps.dotProduct(a, b);        // Dot product
SIMDOps.distance(a, b);          // Euclidean distance
SIMDOps.cosineSimilarity(a, b);  // Cosine similarity
```

### `LIFLayer`

Low-level LIF neuron layer.

```javascript
const { LIFLayer } = require('spiking-neural');

const layer = new LIFLayer(100, {
  tau: 20.0,
  v_thresh: -50.0,
  dt: 1.0
});

layer.setCurrents(inputCurrents);
const spike_count = layer.update();
const spikes = layer.getSpikes();
```

### `SynapticLayer`

Low-level synaptic connection layer with STDP.

```javascript
const { SynapticLayer } = require('spiking-neural');

const synapses = new SynapticLayer(100, 50, {
  a_plus: 0.01,
  a_minus: 0.01,
  w_min: 0.0,
  w_max: 1.0
});

synapses.forward(pre_spikes, post_currents);
synapses.learn(pre_spikes, post_spikes);
const stats = synapses.getWeightStats();
```

## Performance

### JavaScript (Auto-vectorization)

| Operation | 64d | 128d | 256d | 512d |
|-----------|-----|------|------|------|
| Dot Product | 1.1x | 1.2x | 1.6x | 1.5x |
| Distance | 5x | **54x** | 13x | 9x |
| Cosine | **2.7x** | 1.0x | 0.9x | 0.9x |

### Benchmark Results

| Network Size | Updates/sec | Latency |
|--------------|-------------|---------|
| 100 neurons | 16,000+ | 0.06ms |
| 1,000 neurons | 1,500+ | 0.67ms |
| 10,000 neurons | 150+ | 6.7ms |

## Examples

### Pattern Recognition

```javascript
const { createFeedforwardSNN, rateEncoding } = require('spiking-neural');

// 5x5 pattern -> 4 classes
const snn = createFeedforwardSNN([25, 20, 4], {
  a_plus: 0.005,
  lateral_inhibition: true
});

// Define patterns
const cross = [
  0,0,1,0,0,
  0,0,1,0,0,
  1,1,1,1,1,
  0,0,1,0,0,
  0,0,1,0,0
];

// Train
for (let epoch = 0; epoch < 5; epoch++) {
  snn.reset();
  for (let t = 0; t < 100; t++) {
    snn.step(rateEncoding(cross, 1.0, 100));
  }
}

// Test
snn.reset();
for (let t = 0; t < 100; t++) {
  snn.step(rateEncoding(cross, 1.0, 100));
}
const output = snn.getOutput();
const winner = Array.from(output).indexOf(Math.max(...output));
console.log(`Pattern classified as neuron ${winner}`);
```

### Custom Network Architecture

```javascript
const { LIFLayer, SynapticLayer, SpikingNeuralNetwork } = require('spiking-neural');

// Build custom architecture
const input_layer = new LIFLayer(100, { tau: 15.0 });
const hidden_layer = new LIFLayer(50, { tau: 20.0 });
const output_layer = new LIFLayer(10, { tau: 25.0 });

const input_hidden = new SynapticLayer(100, 50, { a_plus: 0.01 });
const hidden_output = new SynapticLayer(50, 10, { a_plus: 0.005 });

const layers = [
  { neuron_layer: input_layer, synaptic_layer: input_hidden },
  { neuron_layer: hidden_layer, synaptic_layer: hidden_output },
  { neuron_layer: output_layer, synaptic_layer: null }
];

const snn = new SpikingNeuralNetwork(layers, {
  lateral_inhibition: true
});
```

## Related Demos

This package is part of the [ruvector](https://github.com/ruvnet/ruvector) meta-cognition examples:

| Demo | Description | Command |
|------|-------------|---------|
| Pattern Recognition | 5x5 pixel classification | `npx spiking-neural demo pattern` |
| SIMD Benchmarks | Vector operation performance | `npx spiking-neural simd` |
| Attention Mechanisms | 5 attention types | See [meta-cognition examples](../examples/meta-cognition-spiking-neural-network) |
| Hyperbolic Attention | Poincaré ball model | See [meta-cognition examples](../examples/meta-cognition-spiking-neural-network) |
| Self-Discovery | Meta-cognitive systems | See [meta-cognition examples](../examples/meta-cognition-spiking-neural-network) |

## What are Spiking Neural Networks?

SNNs are **third-generation neural networks** that model biological neurons:

| Feature | Traditional ANN | Spiking NN |
|---------|-----------------|------------|
| Activation | Continuous values | Discrete spikes |
| Time | Ignored | Integral to computation |
| Learning | Backpropagation | STDP (Hebbian) |
| Energy | High | 10-100x lower |
| Hardware | GPU/TPU | Neuromorphic chips |

**Advantages:**
- More biologically realistic
- Energy efficient (event-driven)
- Natural for temporal data
- Online learning without batches

## License

MIT © [rUv](https://ruv.io)

## Links

- **Homepage**: [ruv.io](https://ruv.io)
- **GitHub**: [github.com/ruvnet/ruvector](https://github.com/ruvnet/ruvector)
- **npm**: [npmjs.com/package/spiking-neural](https://www.npmjs.com/package/spiking-neural)
- **SNN Guide**: [Meta-Cognition Examples](https://github.com/ruvnet/ruvector/tree/main/examples/meta-cognition-spiking-neural-network)
