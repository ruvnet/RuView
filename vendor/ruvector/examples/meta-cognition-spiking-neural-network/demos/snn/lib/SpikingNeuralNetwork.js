/**
 * Spiking Neural Network - High-Level JavaScript Interface
 *
 * Wraps the SIMD-optimized N-API native addon with an easy-to-use API.
 */

const path = require('path');

// Try to load native addon (may not be built yet)
let native;
try {
  native = require('../build/Release/snn_simd.node');
} catch (e) {
  console.warn('⚠️  Native SNN addon not found. Using JavaScript fallback.');
  console.warn('   Run: cd demos/snn && npm install && npm run build');
  native = null;
}

/**
 * Leaky Integrate-and-Fire Neuron Layer
 */
class LIFLayer {
  constructor(n_neurons, params = {}) {
    this.n_neurons = n_neurons;

    // LIF parameters
    this.tau = params.tau || 20.0;           // Membrane time constant (ms)
    this.v_rest = params.v_rest || -70.0;    // Resting potential (mV)
    this.v_reset = params.v_reset || -75.0;  // Reset potential (mV)
    this.v_thresh = params.v_thresh || -50.0; // Spike threshold (mV)
    this.resistance = params.resistance || 10.0; // Membrane resistance (MOhm)
    this.dt = params.dt || 1.0;              // Time step (ms)

    // State variables
    this.voltages = new Float32Array(n_neurons);
    this.currents = new Float32Array(n_neurons);
    this.spikes = new Float32Array(n_neurons);

    // Initialize voltages to resting potential
    this.voltages.fill(this.v_rest);
  }

  /**
   * Update neuron states for one time step
   */
  update() {
    if (native) {
      // Use native SIMD implementation
      native.lifUpdate(
        this.voltages,
        this.currents,
        this.dt,
        this.tau,
        this.v_rest,
        this.resistance
      );

      return native.detectSpikes(
        this.voltages,
        this.spikes,
        this.v_thresh,
        this.v_reset
      );
    } else {
      // JavaScript fallback
      return this._updateJS();
    }
  }

  /**
   * JavaScript fallback (slower)
   */
  _updateJS() {
    let spike_count = 0;

    for (let i = 0; i < this.n_neurons; i++) {
      // Update membrane potential
      const dv = (-(this.voltages[i] - this.v_rest) +
                  this.resistance * this.currents[i]) * this.dt / this.tau;
      this.voltages[i] += dv;

      // Check for spike
      if (this.voltages[i] >= this.v_thresh) {
        this.spikes[i] = 1.0;
        this.voltages[i] = this.v_reset;
        spike_count++;
      } else {
        this.spikes[i] = 0.0;
      }
    }

    return spike_count;
  }

  /**
   * Set input currents for next time step
   */
  setCurrents(currents) {
    this.currents.set(currents);
  }

  /**
   * Get current spikes
   */
  getSpikes() {
    return this.spikes;
  }

  /**
   * Reset all neurons to resting state
   */
  reset() {
    this.voltages.fill(this.v_rest);
    this.currents.fill(0);
    this.spikes.fill(0);
  }
}

/**
 * Synaptic Connection Layer with STDP Learning
 */
class SynapticLayer {
  constructor(n_pre, n_post, params = {}) {
    this.n_pre = n_pre;
    this.n_post = n_post;

    // STDP parameters
    this.tau_plus = params.tau_plus || 20.0;   // LTP time constant (ms)
    this.tau_minus = params.tau_minus || 20.0; // LTD time constant (ms)
    this.a_plus = params.a_plus || 0.01;       // LTP learning rate
    this.a_minus = params.a_minus || 0.01;     // LTD learning rate
    this.w_min = params.w_min || 0.0;          // Minimum weight
    this.w_max = params.w_max || 1.0;          // Maximum weight
    this.dt = params.dt || 1.0;                // Time step (ms)

    // Weight matrix [n_post x n_pre]
    this.weights = new Float32Array(n_post * n_pre);

    // Spike traces for STDP
    this.pre_trace = new Float32Array(n_pre);
    this.post_trace = new Float32Array(n_post);

    // Decay factors
    this.trace_decay = Math.exp(-this.dt / this.tau_plus);

    // Initialize weights randomly
    this.initializeWeights(params.init_weight || 0.5, params.init_std || 0.1);
  }

  /**
   * Initialize weights with Gaussian distribution
   */
  initializeWeights(mean, std) {
    for (let i = 0; i < this.weights.length; i++) {
      // Box-Muller transform for Gaussian
      const u1 = Math.random();
      const u2 = Math.random();
      const z = Math.sqrt(-2 * Math.log(u1)) * Math.cos(2 * Math.PI * u2);
      let w = mean + z * std;
      w = Math.max(this.w_min, Math.min(this.w_max, w));
      this.weights[i] = w;
    }
  }

  /**
   * Compute post-synaptic currents from pre-synaptic spikes
   */
  forward(pre_spikes, post_currents) {
    if (native) {
      native.computeCurrents(post_currents, pre_spikes, this.weights);
    } else {
      this._forwardJS(pre_spikes, post_currents);
    }
  }

  _forwardJS(pre_spikes, post_currents) {
    post_currents.fill(0);

    for (let j = 0; j < this.n_post; j++) {
      let sum = 0;
      for (let i = 0; i < this.n_pre; i++) {
        sum += pre_spikes[i] * this.weights[j * this.n_pre + i];
      }
      post_currents[j] = sum;
    }
  }

  /**
   * Update weights using STDP
   */
  learn(pre_spikes, post_spikes) {
    if (native) {
      // Update traces
      native.updateTraces(this.pre_trace, pre_spikes, this.trace_decay);
      native.updateTraces(this.post_trace, post_spikes, this.trace_decay);

      // Apply STDP
      native.stdpUpdate(
        this.weights,
        pre_spikes,
        post_spikes,
        this.pre_trace,
        this.post_trace,
        this.a_plus,
        this.a_minus,
        this.w_min,
        this.w_max
      );
    } else {
      this._learnJS(pre_spikes, post_spikes);
    }
  }

  _learnJS(pre_spikes, post_spikes) {
    // Update traces
    for (let i = 0; i < this.n_pre; i++) {
      this.pre_trace[i] = this.pre_trace[i] * this.trace_decay + pre_spikes[i];
    }
    for (let j = 0; j < this.n_post; j++) {
      this.post_trace[j] = this.post_trace[j] * this.trace_decay + post_spikes[j];
    }

    // Update weights
    for (let j = 0; j < this.n_post; j++) {
      for (let i = 0; i < this.n_pre; i++) {
        const idx = j * this.n_pre + i;

        // LTP: pre spike strengthens synapse based on post trace
        const ltp = pre_spikes[i] * this.post_trace[j] * this.a_plus;

        // LTD: post spike weakens synapse based on pre trace
        const ltd = post_spikes[j] * this.pre_trace[i] * this.a_minus;

        // Update and clamp
        this.weights[idx] += ltp - ltd;
        this.weights[idx] = Math.max(this.w_min, Math.min(this.w_max, this.weights[idx]));
      }
    }
  }

  /**
   * Get weight statistics
   */
  getWeightStats() {
    let sum = 0, min = Infinity, max = -Infinity;
    for (let i = 0; i < this.weights.length; i++) {
      sum += this.weights[i];
      min = Math.min(min, this.weights[i]);
      max = Math.max(max, this.weights[i]);
    }
    return {
      mean: sum / this.weights.length,
      min: min,
      max: max
    };
  }
}

/**
 * Complete Spiking Neural Network
 */
class SpikingNeuralNetwork {
  constructor(layers, params = {}) {
    this.layers = layers;
    this.dt = params.dt || 1.0;
    this.time = 0;

    // Lateral inhibition
    this.lateral_inhibition = params.lateral_inhibition || false;
    this.inhibition_strength = params.inhibition_strength || 10.0;

    // Statistics
    this.spike_history = [];
    this.weight_history = [];
  }

  /**
   * Process one time step
   */
  step(input_spikes = null) {
    // Set input to first layer
    if (input_spikes && this.layers.length > 0) {
      if (this.layers[0].neuron_layer) {
        this.layers[0].neuron_layer.setCurrents(input_spikes);
      }
    }

    let total_spikes = 0;

    // Update each layer
    for (let i = 0; i < this.layers.length; i++) {
      const layer = this.layers[i];

      // Update neurons
      if (layer.neuron_layer) {
        const spike_count = layer.neuron_layer.update();
        total_spikes += spike_count;

        // Apply lateral inhibition
        if (this.lateral_inhibition && native) {
          native.lateralInhibition(
            layer.neuron_layer.voltages,
            layer.neuron_layer.spikes,
            this.inhibition_strength
          );
        }

        // Forward to next layer via synapses
        if (layer.synaptic_layer && i + 1 < this.layers.length) {
          const next_layer = this.layers[i + 1].neuron_layer;
          if (next_layer) {
            layer.synaptic_layer.forward(
              layer.neuron_layer.getSpikes(),
              next_layer.currents
            );

            // STDP learning
            layer.synaptic_layer.learn(
              layer.neuron_layer.getSpikes(),
              next_layer.getSpikes()
            );
          }
        }
      }
    }

    this.time += this.dt;
    return total_spikes;
  }

  /**
   * Run network for multiple time steps
   */
  run(duration, input_generator = null) {
    const n_steps = Math.floor(duration / this.dt);
    const results = {
      spikes: [],
      times: [],
      total_spikes: 0
    };

    for (let step = 0; step < n_steps; step++) {
      const input = input_generator ? input_generator(this.time) : null;
      const spike_count = this.step(input);

      results.spikes.push(spike_count);
      results.times.push(this.time);
      results.total_spikes += spike_count;
    }

    return results;
  }

  /**
   * Get output spikes from last layer
   */
  getOutput() {
    if (this.layers.length === 0) return null;
    const last_layer = this.layers[this.layers.length - 1];
    return last_layer.neuron_layer ? last_layer.neuron_layer.getSpikes() : null;
  }

  /**
   * Reset network to initial state
   */
  reset() {
    this.time = 0;
    for (const layer of this.layers) {
      if (layer.neuron_layer) layer.neuron_layer.reset();
    }
  }

  /**
   * Get network statistics
   */
  getStats() {
    const stats = {
      time: this.time,
      layers: []
    };

    for (let i = 0; i < this.layers.length; i++) {
      const layer_stats = { index: i };

      if (this.layers[i].neuron_layer) {
        const neurons = this.layers[i].neuron_layer;
        const avg_voltage = neurons.voltages.reduce((a, b) => a + b, 0) / neurons.n_neurons;
        const spike_count = neurons.spikes.reduce((a, b) => a + b, 0);

        layer_stats.neurons = {
          count: neurons.n_neurons,
          avg_voltage: avg_voltage,
          spike_count: spike_count
        };
      }

      if (this.layers[i].synaptic_layer) {
        layer_stats.synapses = this.layers[i].synaptic_layer.getWeightStats();
      }

      stats.layers.push(layer_stats);
    }

    return stats;
  }
}

/**
 * Helper: Create a simple feedforward SNN
 */
function createFeedforwardSNN(layer_sizes, params = {}) {
  const layers = [];

  for (let i = 0; i < layer_sizes.length; i++) {
    const layer = {
      neuron_layer: new LIFLayer(layer_sizes[i], params),
      synaptic_layer: null
    };

    // Add synaptic connection to next layer
    if (i < layer_sizes.length - 1) {
      layer.synaptic_layer = new SynapticLayer(
        layer_sizes[i],
        layer_sizes[i + 1],
        params
      );
    }

    layers.push(layer);
  }

  return new SpikingNeuralNetwork(layers, params);
}

/**
 * Input encoding: Rate coding (Poisson spike train)
 */
function rateEncoding(values, dt, max_rate = 100) {
  const spikes = new Float32Array(values.length);

  for (let i = 0; i < values.length; i++) {
    // Probability of spike = rate * dt / 1000
    const rate = values[i] * max_rate;
    const p_spike = rate * dt / 1000;
    spikes[i] = Math.random() < p_spike ? 1.0 : 0.0;
  }

  return spikes;
}

/**
 * Input encoding: Temporal coding (time-to-first-spike)
 */
function temporalEncoding(values, time, t_start = 0, t_window = 50) {
  const spikes = new Float32Array(values.length);

  for (let i = 0; i < values.length; i++) {
    // Spike time = t_start + (1 - value) * t_window
    const spike_time = t_start + (1 - values[i]) * t_window;
    spikes[i] = (time >= spike_time && time < spike_time + 1) ? 1.0 : 0.0;
  }

  return spikes;
}

module.exports = {
  SpikingNeuralNetwork,
  LIFLayer,
  SynapticLayer,
  createFeedforwardSNN,
  rateEncoding,
  temporalEncoding,
  native: native !== null
};
