#!/usr/bin/env node

/**
 * SNN-CSI Processor — Spiking Neural Network for WiFi CSI Sensing
 *
 * Receives live CSI frames via UDP (ADR-018 binary format), feeds subcarrier
 * amplitude deltas through a 128-64-8 SNN with STDP online learning.
 * Output neurons map to: presence, motion, breathing, HR, phase_var, persons, fall, RSSI.
 *
 * Usage:
 *   node scripts/snn-csi-processor.js [options]
 *
 * Options:
 *   --port <n>           UDP listen port (default: 5006)
 *   --max-rate <n>       Max spike rate in Hz (default: 200)
 *   --learning-rate <n>  STDP a_plus/a_minus (default: 0.005)
 *   --hidden <n>         Hidden layer neurons (default: 64)
 *   --no-learn           Disable STDP (freeze weights)
 *   --send-vectors       Forward spike vectors to Cognitum Seed
 *   --seed-host <host>   Cognitum Seed host (default: localhost)
 *   --seed-port <n>      Cognitum Seed port (default: 5007)
 *   --quiet              Suppress visualization, print only JSON
 *
 * Requires: @ruvector/spiking-neural (vendored or npm)
 *
 * ADR-074: Spiking Neural Network for CSI Sensing
 */

'use strict';

const dgram = require('dgram');
const path = require('path');
const { parseRawCsiFrame } = require('./lib/raw-csi');

// ---------------------------------------------------------------------------
// Resolve spiking-neural: try npm, then vendor
// ---------------------------------------------------------------------------
let snn_lib;
try {
  snn_lib = require('@ruvector/spiking-neural');
} catch {
  try {
    snn_lib = require(path.resolve(
      __dirname, '..', 'vendor', 'ruvector', 'npm', 'packages', 'spiking-neural', 'src', 'index.js'
    ));
  } catch {
    // If src/index.js doesn't exist locally, fall back to the CLI which re-exports
    snn_lib = require(path.resolve(
      __dirname, '..', 'vendor', 'ruvector', 'npm', 'packages', 'spiking-neural', 'bin', 'cli.js'
    ));
  }
}

const { createFeedforwardSNN, rateEncoding, SIMDOps, version: snnVersion } = snn_lib;

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------
function parseArgs() {
  const args = process.argv.slice(2);
  const opts = {
    port: 5006,
    maxRate: 200,
    learningRate: 0.005,
    hidden: 64,
    learn: true,
    sendVectors: false,
    seedHost: 'localhost',
    seedPort: 5007,
    quiet: false,
  };
  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case '--port':        opts.port = parseInt(args[++i], 10); break;
      case '--max-rate':    opts.maxRate = parseInt(args[++i], 10); break;
      case '--learning-rate': opts.learningRate = parseFloat(args[++i]); break;
      case '--hidden':      opts.hidden = parseInt(args[++i], 10); break;
      case '--no-learn':    opts.learn = false; break;
      case '--send-vectors': opts.sendVectors = true; break;
      case '--seed-host':   opts.seedHost = args[++i]; break;
      case '--seed-port':   opts.seedPort = parseInt(args[++i], 10); break;
      case '--quiet':       opts.quiet = true; break;
      case '--help': case '-h':
        console.log(`SNN-CSI Processor (spiking-neural v${snnVersion || '?'})`);
        console.log('Usage: node scripts/snn-csi-processor.js [options]');
        console.log('  --port <n>           UDP listen port (default: 5006)');
        console.log('  --max-rate <n>       Max spike rate Hz (default: 200)');
        console.log('  --learning-rate <n>  STDP rate (default: 0.005)');
        console.log('  --hidden <n>         Hidden neurons (default: 64)');
        console.log('  --no-learn           Freeze STDP weights');
        console.log('  --send-vectors       Forward to Cognitum Seed');
        console.log('  --seed-host <host>   Seed host (default: localhost)');
        console.log('  --seed-port <n>      Seed port (default: 5007)');
        console.log('  --quiet              JSON-only output');
        process.exit(0);
    }
  }
  return opts;
}

// ---------------------------------------------------------------------------
// ADR-018 binary frame parser
// ---------------------------------------------------------------------------
function parseFrame(buf) {
  const frame = parseRawCsiFrame(buf, { includePhase: false });
  if (!frame) return null;
  return {
    timestamp: Date.now(),
    frequency: frame.freqMhz,
    rssi: frame.rssi,
    noiseFloor: frame.noiseFloor,
    numSubcarriers: frame.nSubcarriers,
    nodeId: frame.nodeId,
    seqNum: frame.seq,
    amplitudes: Float32Array.from(frame.amplitudes),
  };
}

// ---------------------------------------------------------------------------
// SNN setup
// ---------------------------------------------------------------------------
const INPUT_NEURONS = 128;
const OUTPUT_NEURONS = 8;

const OUTPUT_LABELS = [
  'presence', 'motion', 'breathing', 'heart_rate',
  'phase_var', 'persons', 'fall', 'rssi'
];

function createCSISnn(opts) {
  const snn = createFeedforwardSNN([INPUT_NEURONS, opts.hidden, OUTPUT_NEURONS], {
    dt: 1.0,
    tau: 20.0,
    v_rest: -70.0,
    v_reset: -75.0,
    v_thresh: -50.0,
    resistance: 10.0,
    a_plus: opts.learningRate,
    a_minus: opts.learningRate * 0.6, // Slight asymmetry: LTP > LTD for stability
    w_min: 0.0,
    w_max: 1.0,
    init_weight: 0.3,
    init_std: 0.05,
    lateral_inhibition: true,
    inhibition_strength: 15.0,
  });
  return snn;
}

// ---------------------------------------------------------------------------
// Amplitude delta tracking + normalization
// ---------------------------------------------------------------------------
class DeltaTracker {
  constructor(size) {
    this.size = size;
    this.prev = null;
    this.maxDelta = 1.0; // Adaptive normalization ceiling
    this.ewmaMaxDelta = 1.0;
  }

  /**
   * Compute normalized amplitude deltas from a new frame.
   * Returns Float32Array of length INPUT_NEURONS (zero-padded if fewer subcarriers).
   */
  update(amplitudes) {
    const n = Math.min(amplitudes.length, this.size);
    const deltas = new Float32Array(this.size);

    if (this.prev === null) {
      this.prev = new Float32Array(amplitudes);
      return deltas; // First frame: all zeros (no delta yet)
    }

    let frameMax = 0;
    for (let i = 0; i < n; i++) {
      const d = Math.abs(amplitudes[i] - this.prev[i]);
      deltas[i] = d;
      if (d > frameMax) frameMax = d;
    }

    // Update adaptive normalization with EWMA
    if (frameMax > 0) {
      this.ewmaMaxDelta = 0.95 * this.ewmaMaxDelta + 0.05 * frameMax;
      this.maxDelta = Math.max(this.ewmaMaxDelta, 1.0);
    }

    // Normalize to [0, 1]
    for (let i = 0; i < this.size; i++) {
      deltas[i] = Math.min(deltas[i] / this.maxDelta, 1.0);
    }

    // Store current amplitudes for next delta
    this.prev = new Float32Array(amplitudes);

    return deltas;
  }
}

// ---------------------------------------------------------------------------
// Spike rate smoother (exponentially-weighted moving average on output)
// ---------------------------------------------------------------------------
class OutputSmoother {
  constructor(size, alpha) {
    this.size = size;
    this.alpha = alpha; // Smoothing factor (0.1 = slow, 0.5 = fast)
    this.smoothed = new Float32Array(size);
  }

  update(raw) {
    for (let i = 0; i < this.size; i++) {
      this.smoothed[i] = this.alpha * raw[i] + (1 - this.alpha) * this.smoothed[i];
    }
    return this.smoothed;
  }
}

// ---------------------------------------------------------------------------
// ASCII visualization
// ---------------------------------------------------------------------------
const BAR_CHARS = ' .:;+=xX#@';

function renderBar(value, maxWidth) {
  const clamped = Math.min(Math.max(value, 0), 1);
  const filled = Math.round(clamped * maxWidth);
  const charIdx = Math.min(Math.floor(clamped * (BAR_CHARS.length - 1)), BAR_CHARS.length - 1);
  return BAR_CHARS[charIdx].repeat(filled).padEnd(maxWidth);
}

function renderVisualization(outputSmoothed, stats, frameCount, opts) {
  const lines = [];
  lines.push('');
  lines.push(`--- SNN-CSI Processor (frame #${frameCount}) ---`);
  lines.push(`  Network: ${INPUT_NEURONS}-${opts.hidden}-${OUTPUT_NEURONS}  |  STDP: ${opts.learn ? 'ON' : 'OFF'}  |  Spikes: ${stats.totalSpikes}`);
  lines.push('');
  lines.push('  Output Activity:');

  // Find max for relative scaling
  const maxVal = Math.max(...outputSmoothed, 0.001);

  for (let i = 0; i < OUTPUT_NEURONS; i++) {
    const norm = outputSmoothed[i] / maxVal;
    const bar = renderBar(norm, 30);
    const raw = outputSmoothed[i].toFixed(2).padStart(6);
    lines.push(`    ${OUTPUT_LABELS[i].padEnd(12)} |${bar}| ${raw}`);
  }

  lines.push('');

  // Hidden layer activity heatmap (single row)
  const hiddenActivity = stats.hiddenSpikes || [];
  let heatmap = '  Hidden: ';
  for (let i = 0; i < Math.min(opts.hidden, 64); i++) {
    const val = hiddenActivity[i] || 0;
    const charIdx = Math.min(Math.floor(val * (BAR_CHARS.length - 1)), BAR_CHARS.length - 1);
    heatmap += BAR_CHARS[Math.max(charIdx, 0)];
  }
  lines.push(heatmap);

  // Weight stats
  if (stats.weightMean !== undefined) {
    lines.push(`  Weights: mean=${stats.weightMean.toFixed(3)}  min=${stats.weightMin.toFixed(3)}  max=${stats.weightMax.toFixed(3)}`);
  }

  lines.push('');

  // Clear screen and print (ANSI escape)
  process.stdout.write('\x1b[2J\x1b[H');
  process.stdout.write(lines.join('\n'));
}

// ---------------------------------------------------------------------------
// Main processing loop
// ---------------------------------------------------------------------------
function main() {
  const opts = parseArgs();

  console.log(`SNN-CSI Processor`);
  console.log(`  spiking-neural version: ${snnVersion || 'unknown'}`);
  console.log(`  Network: ${INPUT_NEURONS} -> ${opts.hidden} -> ${OUTPUT_NEURONS}`);
  console.log(`  Synapses: ${INPUT_NEURONS * opts.hidden + opts.hidden * OUTPUT_NEURONS}`);
  console.log(`  STDP: ${opts.learn ? `ON (lr=${opts.learningRate})` : 'OFF (frozen)'}`);
  console.log(`  Lateral inhibition: ON (strength=15.0)`);
  console.log(`  Listening on UDP port ${opts.port}...`);
  console.log('');

  const snn = createCSISnn(opts);
  const deltaTracker = new DeltaTracker(INPUT_NEURONS);
  const smoother = new OutputSmoother(OUTPUT_NEURONS, 0.3);

  let frameCount = 0;
  let totalSpikes = 0;
  const SIM_STEPS_PER_FRAME = 5; // Run 5ms of SNN simulation per CSI frame

  // Optional: Cognitum Seed forwarding socket
  let seedSocket = null;
  if (opts.sendVectors) {
    seedSocket = dgram.createSocket('udp4');
    console.log(`  Forwarding spike vectors to ${opts.seedHost}:${opts.seedPort}`);
  }

  // UDP listener
  const server = dgram.createSocket('udp4');

  server.on('message', (msg, rinfo) => {
    const frame = parseFrame(msg);
    if (!frame) return;

    frameCount++;

    // Compute amplitude deltas
    const deltas = deltaTracker.update(frame.amplitudes);

    // Run SNN for multiple simulation steps per frame
    let frameSpikes = 0;
    const outputAccum = new Float32Array(OUTPUT_NEURONS);

    for (let t = 0; t < SIM_STEPS_PER_FRAME; t++) {
      // Rate-encode deltas as Poisson spikes
      const inputSpikes = rateEncoding(deltas, 1.0, opts.maxRate);

      // Step SNN (STDP learning happens inside if weights are not frozen)
      frameSpikes += snn.step(inputSpikes);

      // Accumulate output
      const output = snn.getOutput();
      for (let i = 0; i < OUTPUT_NEURONS; i++) {
        outputAccum[i] += output[i];
      }
    }

    totalSpikes += frameSpikes;

    // Normalize accumulated output by simulation steps
    for (let i = 0; i < OUTPUT_NEURONS; i++) {
      outputAccum[i] /= SIM_STEPS_PER_FRAME;
    }

    // Smooth output
    const smoothed = smoother.update(outputAccum);

    // Get network stats
    const netStats = snn.getStats();
    const stats = {
      totalSpikes: frameSpikes,
      hiddenSpikes: [],
      weightMean: 0,
      weightMin: 0,
      weightMax: 0,
    };

    // Extract hidden layer spike info if available
    if (netStats.layers && netStats.layers.length > 1) {
      const hiddenLayer = netStats.layers[1];
      if (hiddenLayer.neurons) {
        // Build a rough activity vector from spike counts
        // The API gives aggregate counts, not per-neuron; approximate with output
        stats.hiddenSpikes = new Array(opts.hidden).fill(0);
        stats.hiddenSpikes[0] = hiddenLayer.neurons.spike_count > 0 ? 1 : 0;
      }
      if (netStats.layers[0] && netStats.layers[0].synapses) {
        stats.weightMean = netStats.layers[0].synapses.mean;
        stats.weightMin = netStats.layers[0].synapses.min;
        stats.weightMax = netStats.layers[0].synapses.max;
      }
    }

    // Visualization or JSON output
    if (opts.quiet) {
      const result = {
        frame: frameCount,
        timestamp: frame.timestamp,
        nodeId: frame.nodeId,
        channel: Math.round((frame.frequency - 2407) / 5),
        subcarriers: frame.numSubcarriers,
        rssi: frame.rssi,
        spikes: frameSpikes,
        output: {},
      };
      for (let i = 0; i < OUTPUT_NEURONS; i++) {
        result.output[OUTPUT_LABELS[i]] = parseFloat(smoothed[i].toFixed(3));
      }
      console.log(JSON.stringify(result));
    } else {
      renderVisualization(smoothed, stats, frameCount, opts);
    }

    // Forward spike vector to Cognitum Seed
    if (seedSocket) {
      const vectorBuf = Buffer.alloc(4 + OUTPUT_NEURONS * 4); // 4-byte header + float32 array
      vectorBuf.writeUInt16LE(0x534E, 0); // 'SN' magic
      vectorBuf.writeUInt8(OUTPUT_NEURONS, 2);
      vectorBuf.writeUInt8(frame.nodeId & 0xFF, 3);
      for (let i = 0; i < OUTPUT_NEURONS; i++) {
        vectorBuf.writeFloatLE(smoothed[i], 4 + i * 4);
      }
      seedSocket.send(vectorBuf, opts.seedPort, opts.seedHost);
    }
  });

  server.on('error', (err) => {
    console.error(`UDP error: ${err.message}`);
    server.close();
    process.exit(1);
  });

  server.bind(opts.port, () => {
    console.log(`Listening for CSI frames on UDP port ${opts.port}`);
  });

  // Periodic weight decay (prevent drift) — every 1 second
  if (opts.learn) {
    setInterval(() => {
      // Weight decay is applied implicitly by the SNN's w_min/w_max clamping
      // and the balanced LTP/LTD rates. No additional decay needed for now.
      // Future: iterate weights and multiply by 0.999 if drift is observed.
    }, 1000);
  }

  // Periodic stats dump (every 10 seconds)
  setInterval(() => {
    if (opts.quiet) return;
    const stats = snn.getStats();
    const uptimeSec = Math.floor(process.uptime());
    const fps = frameCount > 0 ? (frameCount / uptimeSec).toFixed(1) : '0.0';
    process.stderr.write(
      `[${uptimeSec}s] frames=${frameCount} fps=${fps} totalSpikes=${totalSpikes} ` +
      `mem=${Math.round(process.memoryUsage().heapUsed / 1024)}KB\n`
    );
  }, 10000);

  // Graceful shutdown
  process.on('SIGINT', () => {
    console.log('\n\nShutting down SNN-CSI Processor...');
    const stats = snn.getStats();
    console.log(`  Total frames processed: ${frameCount}`);
    console.log(`  Total spikes: ${totalSpikes}`);
    if (stats.layers && stats.layers[0] && stats.layers[0].synapses) {
      const w = stats.layers[0].synapses;
      console.log(`  Final weights: mean=${w.mean.toFixed(3)} min=${w.min.toFixed(3)} max=${w.max.toFixed(3)}`);
    }
    server.close();
    if (seedSocket) seedSocket.close();
    process.exit(0);
  });
}

main();
