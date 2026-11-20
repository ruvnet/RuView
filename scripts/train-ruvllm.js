#!/usr/bin/env node
/**
 * WiFi-DensePose CSI Training Pipeline using ruvllm
 *
 * Complete training, refinement, and quantization pipeline for CSI sensing models.
 * Uses ruvllm's ContrastiveTrainer, TrainingPipeline, LoRA, EWC, and SafeTensors export.
 *
 * Usage:
 *   node scripts/train-ruvllm.js --data data/recordings/pretrain-*.csi.jsonl
 *   node scripts/train-ruvllm.js --data data/recordings/pretrain-1775182186.csi.jsonl --benchmark
 *   node scripts/train-ruvllm.js --data data/recordings/*.csi.jsonl --output models/csi-v1
 *
 * ADR: docs/adr/ADR-071-ruvllm-training-pipeline.md
 */

'use strict';

const fs = require('fs');
const path = require('path');
const { parseArgs } = require('util');

// ---------------------------------------------------------------------------
// Resolve ruvllm from vendor tree — use compiled JS output
// ---------------------------------------------------------------------------
const RUVLLM_PATH = path.resolve(__dirname, '..', 'vendor', 'ruvector', 'npm', 'packages', 'ruvllm', 'src');

const {
  ContrastiveTrainer,
  cosineSimilarity,
  tripletLoss,
  infoNCELoss,
  computeGradient,
} = require(path.join(RUVLLM_PATH, 'contrastive.js'));

const {
  TrainingPipeline,
} = require(path.join(RUVLLM_PATH, 'training.js'));

const {
  LoraAdapter,
  LoraManager,
} = require(path.join(RUVLLM_PATH, 'lora.js'));

const {
  EwcManager,
  ReasoningBank,
  SonaCoordinator,
} = require(path.join(RUVLLM_PATH, 'sona.js'));

const {
  SafeTensorsWriter,
  ModelExporter,
  DatasetExporter,
} = require(path.join(RUVLLM_PATH, 'export.js'));

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------
const { values: args } = parseArgs({
  options: {
    data: { type: 'string', short: 'd' },
    output: { type: 'string', short: 'o', default: 'models/csi-ruvllm' },
    benchmark: { type: 'boolean', short: 'b', default: false },
    epochs: { type: 'string', short: 'e', default: '20' },
    'batch-size': { type: 'string', default: '32' },
    'lora-rank': { type: 'string', default: '4' },
    'quantize-bits': { type: 'string', default: '4' },
    verbose: { type: 'boolean', short: 'v', default: false },
  },
  strict: true,
});

if (!args.data) {
  console.error('Usage: node scripts/train-ruvllm.js --data <path-to-csi-jsonl> [--output dir] [--benchmark]');
  process.exit(1);
}

const CONFIG = {
  dataGlob: args.data,
  outputDir: args.output,
  benchmark: args.benchmark,
  epochs: parseInt(args.epochs, 10),
  batchSize: parseInt(args['batch-size'], 10),
  loraRank: parseInt(args['lora-rank'], 10),
  quantizeBits: parseInt(args['quantize-bits'], 10),
  verbose: args.verbose,

  // Contrastive training hyperparameters
  margin: 0.3,
  temperature: 0.07,
  hardNegativeRatio: 0.7,
  learningRate: 0.001,

  // Temporal window thresholds (seconds)
  positiveWindowSec: 1.0,
  negativeWindowSec: 10.0,   // Reduced from 30s — 120s recording needs tighter threshold

  // Data augmentation
  augmentMultiplier: 10,     // Expand dataset 10x via augmentation

  // Feature dimensions
  inputDim: 8,        // 8-dim CSI feature vector
  hiddenDim: 64,      // intermediate
  embeddingDim: 128,   // output embedding
};

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

/**
 * Parse CSI JSONL file into typed frames.
 * Returns arrays of feature frames, vitals frames, and raw CSI frames.
 */
function loadCsiData(filePath) {
  const features = [];
  const vitals = [];
  const rawCsi = [];

  const content = fs.readFileSync(filePath, 'utf-8');
  const lines = content.split('\n').filter(l => l.trim());

  for (const line of lines) {
    try {
      const frame = JSON.parse(line);
      switch (frame.type) {
        case 'feature':
          features.push({
            timestamp: frame.timestamp,
            nodeId: frame.node_id,
            features: frame.features,  // 8-dim float array
            rssi: frame.rssi,
            seq: frame.seq,
          });
          break;
        case 'vitals':
          vitals.push({
            timestamp: frame.timestamp,
            nodeId: frame.node_id,
            breathingBpm: frame.breathing_bpm,
            heartrateBpm: frame.heartrate_bpm,
            nPersons: frame.n_persons,
            motionEnergy: frame.motion_energy,
            presenceScore: frame.presence_score,
            rssi: frame.rssi,
          });
          break;
        case 'raw_csi':
          rawCsi.push({
            timestamp: frame.timestamp,
            nodeId: frame.node_id,
            subcarriers: frame.subcarriers,
            iqHex: frame.iq_hex,
            rssi: frame.rssi,
          });
          break;
      }
    } catch (e) {
      // Skip malformed lines
    }
  }

  return { features, vitals, rawCsi };
}

/**
 * Resolve glob pattern to file list. Handles simple * patterns on both
 * Unix and Windows without requiring a glob library.
 */
function resolveGlob(pattern) {
  if (!pattern.includes('*')) {
    return fs.existsSync(pattern) ? [pattern] : [];
  }
  const dir = path.dirname(pattern);
  const base = path.basename(pattern);
  const regex = new RegExp('^' + base.replace(/\*/g, '.*') + '$');
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir)
    .filter(f => regex.test(f))
    .map(f => path.join(dir, f));
}

// ---------------------------------------------------------------------------
// Embedding encoder (simulates 8 -> 64 -> 128 FC network)
// ---------------------------------------------------------------------------

/**
 * Two-layer FC encoder with batch normalization: 8 -> 64 (BN, ReLU) -> 128 (BN) -> L2 norm
 * Uses Xavier/Glorot initialization for better gradient flow.
 */
class CsiEncoder {
  constructor(inputDim, hiddenDim, outputDim, seed = 42) {
    this.inputDim = inputDim;
    this.hiddenDim = hiddenDim;
    this.outputDim = outputDim;

    // Xavier/Glorot initialization (better for sigmoid/tanh and general use)
    const rng = this._createRng(seed);
    this.w1 = this._initXavier(inputDim, hiddenDim, rng);
    this.b1 = new Float64Array(hiddenDim);
    this.w2 = this._initXavier(hiddenDim, outputDim, rng);
    this.b2 = new Float64Array(outputDim);

    // Batch norm parameters (gamma=1, beta=0 initially)
    this.bn1_gamma = new Float64Array(hiddenDim).fill(1.0);
    this.bn1_beta = new Float64Array(hiddenDim);
    this.bn2_gamma = new Float64Array(outputDim).fill(1.0);
    this.bn2_beta = new Float64Array(outputDim);

    // Running statistics for batch norm (updated during encoding batches)
    this.bn1_runMean = new Float64Array(hiddenDim);
    this.bn1_runVar = new Float64Array(hiddenDim).fill(1.0);
    this.bn2_runMean = new Float64Array(outputDim);
    this.bn2_runVar = new Float64Array(outputDim).fill(1.0);
    this._bnMomentum = 0.1;
    this._bnEps = 1e-5;
    this._bnInitialized = false;
  }

  /**
   * Forward pass: input (8-dim) -> embedding (128-dim)
   */
  encode(input) {
    // Layer 1: input @ w1 + b1
    const hidden = new Float64Array(this.hiddenDim);
    for (let j = 0; j < this.hiddenDim; j++) {
      let sum = this.b1[j];
      for (let i = 0; i < this.inputDim; i++) {
        sum += (input[i] || 0) * this.w1[i * this.hiddenDim + j];
      }
      hidden[j] = sum;
    }

    // Batch norm layer 1 (use running stats for single-sample inference)
    for (let j = 0; j < this.hiddenDim; j++) {
      const normed = (hidden[j] - this.bn1_runMean[j]) / Math.sqrt(this.bn1_runVar[j] + this._bnEps);
      hidden[j] = Math.max(0, this.bn1_gamma[j] * normed + this.bn1_beta[j]); // BN + ReLU
    }

    // Layer 2: hidden @ w2 + b2
    const output = new Float64Array(this.outputDim);
    for (let j = 0; j < this.outputDim; j++) {
      let sum = this.b2[j];
      for (let i = 0; i < this.hiddenDim; i++) {
        sum += hidden[i] * this.w2[i * this.outputDim + j];
      }
      output[j] = sum;
    }

    // Batch norm layer 2
    for (let j = 0; j < this.outputDim; j++) {
      const normed = (output[j] - this.bn2_runMean[j]) / Math.sqrt(this.bn2_runVar[j] + this._bnEps);
      output[j] = this.bn2_gamma[j] * normed + this.bn2_beta[j];
    }

    // L2 normalize
    let norm = 0;
    for (let i = 0; i < output.length; i++) norm += output[i] * output[i];
    norm = Math.sqrt(norm) || 1;
    const result = new Array(this.outputDim);
    for (let i = 0; i < this.outputDim; i++) result[i] = output[i] / norm;
    return result;
  }

  /**
   * Forward pass without L2 normalization (for gradient updates).
   * Returns raw hidden and pre-norm output for backprop.
   */
  encodeRaw(input) {
    const hidden = new Float64Array(this.hiddenDim);
    for (let j = 0; j < this.hiddenDim; j++) {
      let sum = this.b1[j];
      for (let i = 0; i < this.inputDim; i++) {
        sum += (input[i] || 0) * this.w1[i * this.hiddenDim + j];
      }
      hidden[j] = sum;
    }
    for (let j = 0; j < this.hiddenDim; j++) {
      const normed = (hidden[j] - this.bn1_runMean[j]) / Math.sqrt(this.bn1_runVar[j] + this._bnEps);
      hidden[j] = Math.max(0, this.bn1_gamma[j] * normed + this.bn1_beta[j]);
    }
    const output = new Float64Array(this.outputDim);
    for (let j = 0; j < this.outputDim; j++) {
      let sum = this.b2[j];
      for (let i = 0; i < this.hiddenDim; i++) {
        sum += hidden[i] * this.w2[i * this.outputDim + j];
      }
      output[j] = sum;
    }
    return { hidden, output };
  }

  /**
   * Encode a batch and update running batch norm statistics.
   */
  encodeBatch(inputs) {
    if (inputs.length === 0) return [];

    // Compute batch statistics for BN layer 1
    const batchHidden = [];
    for (const input of inputs) {
      const h = new Float64Array(this.hiddenDim);
      for (let j = 0; j < this.hiddenDim; j++) {
        let sum = this.b1[j];
        for (let i = 0; i < this.inputDim; i++) sum += (input[i] || 0) * this.w1[i * this.hiddenDim + j];
        h[j] = sum;
      }
      batchHidden.push(h);
    }

    // Update BN1 running stats from batch
    const n = inputs.length;
    for (let j = 0; j < this.hiddenDim; j++) {
      let bMean = 0, bVar = 0;
      for (let b = 0; b < n; b++) bMean += batchHidden[b][j];
      bMean /= n;
      for (let b = 0; b < n; b++) bVar += (batchHidden[b][j] - bMean) ** 2;
      bVar /= n;
      if (this._bnInitialized) {
        this.bn1_runMean[j] = (1 - this._bnMomentum) * this.bn1_runMean[j] + this._bnMomentum * bMean;
        this.bn1_runVar[j] = (1 - this._bnMomentum) * this.bn1_runVar[j] + this._bnMomentum * bVar;
      } else {
        this.bn1_runMean[j] = bMean;
        this.bn1_runVar[j] = bVar;
      }
    }

    // Apply BN1 + ReLU, then compute layer 2
    const batchOutput = [];
    for (let b = 0; b < n; b++) {
      for (let j = 0; j < this.hiddenDim; j++) {
        const normed = (batchHidden[b][j] - this.bn1_runMean[j]) / Math.sqrt(this.bn1_runVar[j] + this._bnEps);
        batchHidden[b][j] = Math.max(0, this.bn1_gamma[j] * normed + this.bn1_beta[j]);
      }
      const out = new Float64Array(this.outputDim);
      for (let j = 0; j < this.outputDim; j++) {
        let sum = this.b2[j];
        for (let i = 0; i < this.hiddenDim; i++) sum += batchHidden[b][i] * this.w2[i * this.outputDim + j];
        out[j] = sum;
      }
      batchOutput.push(out);
    }

    // Update BN2 running stats from batch
    for (let j = 0; j < this.outputDim; j++) {
      let bMean = 0, bVar = 0;
      for (let b = 0; b < n; b++) bMean += batchOutput[b][j];
      bMean /= n;
      for (let b = 0; b < n; b++) bVar += (batchOutput[b][j] - bMean) ** 2;
      bVar /= n;
      if (this._bnInitialized) {
        this.bn2_runMean[j] = (1 - this._bnMomentum) * this.bn2_runMean[j] + this._bnMomentum * bMean;
        this.bn2_runVar[j] = (1 - this._bnMomentum) * this.bn2_runVar[j] + this._bnMomentum * bVar;
      } else {
        this.bn2_runMean[j] = bMean;
        this.bn2_runVar[j] = bVar;
      }
    }
    this._bnInitialized = true;

    // Apply BN2 + L2 normalize
    const results = [];
    for (let b = 0; b < n; b++) {
      for (let j = 0; j < this.outputDim; j++) {
        const normed = (batchOutput[b][j] - this.bn2_runMean[j]) / Math.sqrt(this.bn2_runVar[j] + this._bnEps);
        batchOutput[b][j] = this.bn2_gamma[j] * normed + this.bn2_beta[j];
      }
      let norm = 0;
      for (let i = 0; i < this.outputDim; i++) norm += batchOutput[b][i] ** 2;
      norm = Math.sqrt(norm) || 1;
      const result = new Array(this.outputDim);
      for (let i = 0; i < this.outputDim; i++) result[i] = batchOutput[b][i] / norm;
      results.push(result);
    }
    return results;
  }

  _createRng(seed) {
    let s = seed;
    return () => {
      s ^= s << 13;
      s ^= s >> 17;
      s ^= s << 5;
      return ((s >>> 0) / 4294967296) - 0.5;
    };
  }

  /** Xavier/Glorot initialization: scale = sqrt(2 / (fanIn + fanOut)) */
  _initXavier(rows, cols, rng) {
    const scale = Math.sqrt(2.0 / (rows + cols));
    const arr = new Float64Array(rows * cols);
    for (let i = 0; i < arr.length; i++) arr[i] = rng() * 2 * scale;
    return arr;
  }
}

// ---------------------------------------------------------------------------
// Presence head: 128 -> 1 (sigmoid) for presence detection
// ---------------------------------------------------------------------------

/**
 * Simple linear head for presence prediction: embedding (128) -> score (0-1).
 * Trained with binary cross-entropy on presence labels.
 */
class PresenceHead {
  constructor(inputDim, seed = 123) {
    this.inputDim = inputDim;
    const rng = CsiEncoder.prototype._createRng.call(null, seed);
    // Xavier init for 128->1
    const scale = Math.sqrt(2.0 / (inputDim + 1));
    this.weights = new Float64Array(inputDim);
    // Use a simple seeded init
    let s = seed;
    const nextRng = () => { s ^= s << 13; s ^= s >> 17; s ^= s << 5; return ((s >>> 0) / 4294967296) - 0.5; };
    for (let i = 0; i < inputDim; i++) this.weights[i] = nextRng() * 2 * scale;
    this.bias = 0;
  }

  /** Forward: sigmoid(w . x + b) */
  forward(embedding) {
    let z = this.bias;
    for (let i = 0; i < this.inputDim; i++) z += this.weights[i] * (embedding[i] || 0);
    return 1.0 / (1.0 + Math.exp(-z)); // sigmoid
  }

  /** Train one step with binary cross-entropy gradient */
  trainStep(embedding, target, lr) {
    const pred = this.forward(embedding);
    // BCE gradient: dL/dz = pred - target
    const dz = pred - target;
    // Update weights
    for (let i = 0; i < this.inputDim; i++) {
      this.weights[i] -= lr * dz * (embedding[i] || 0);
    }
    this.bias -= lr * dz;
    // Return BCE loss
    const eps = 1e-7;
    return -(target * Math.log(pred + eps) + (1 - target) * Math.log(1 - pred + eps));
  }

  /** Export weights for model saving */
  getWeights() {
    return { weights: Array.from(this.weights), bias: this.bias };
  }

  /** Load weights from saved model */
  loadWeights(saved) {
    if (saved.weights) this.weights = new Float64Array(saved.weights);
    if (typeof saved.bias === 'number') this.bias = saved.bias;
  }
}

// ---------------------------------------------------------------------------
// Triplet generation
// ---------------------------------------------------------------------------

/**
 * Generate contrastive triplets from feature frames.
 *
 * Strategies:
 * 1. Temporal positive: frames within 1s = similar environment state
 * 2. Temporal negative: frames >30s apart = different state
 * 3. Cross-node positive: same timestamp from node 1 and node 2 = same person
 * 4. Cross-node negative: different timestamp, different node = different state
 * 5. Hard negatives: frames near transition boundaries
 */
function generateTriplets(features, vitals, config) {
  const triplets = [];

  // Index features by node
  const byNode = {};
  for (const f of features) {
    if (!byNode[f.nodeId]) byNode[f.nodeId] = [];
    byNode[f.nodeId].push(f);
  }
  const nodeIds = Object.keys(byNode).map(Number);

  // Sort each node's features by timestamp
  for (const nid of nodeIds) {
    byNode[nid].sort((a, b) => a.timestamp - b.timestamp);
  }

  // Build a timestamp -> vitals map for labeling
  const vitalsMap = new Map();
  for (const v of vitals) {
    const key = `${v.nodeId}-${Math.round(v.timestamp * 10)}`;
    vitalsMap.set(key, v);
  }

  function findNearestVitals(nodeId, timestamp) {
    // Simple nearest-neighbor lookup in vitals
    let best = null;
    let bestDist = Infinity;
    for (const v of vitals) {
      if (v.nodeId !== nodeId) continue;
      const dist = Math.abs(v.timestamp - timestamp);
      if (dist < bestDist) {
        bestDist = dist;
        best = v;
      }
    }
    return best;
  }

  // Strategy 1 + 2: Temporal positive/negative within same node
  for (const nid of nodeIds) {
    const frames = byNode[nid];
    for (let i = 0; i < frames.length; i++) {
      const anchor = frames[i];

      // Find temporal positive (within 1 second)
      for (let j = i + 1; j < frames.length && j < i + 20; j++) {
        const candidate = frames[j];
        const timeDiff = Math.abs(candidate.timestamp - anchor.timestamp);

        if (timeDiff <= config.positiveWindowSec) {
          // Find a temporal negative (>30 seconds away)
          for (let k = 0; k < frames.length; k++) {
            const neg = frames[k];
            const negTimeDiff = Math.abs(neg.timestamp - anchor.timestamp);

            if (negTimeDiff >= config.negativeWindowSec) {
              const isHard = negTimeDiff < config.negativeWindowSec * 2;
              triplets.push({
                anchor: anchor.features,
                positive: candidate.features,
                negative: neg.features,
                isHard,
                type: 'temporal',
                anchorLabel: `node${nid}-t${anchor.timestamp.toFixed(2)}`,
                posLabel: `node${nid}-t${candidate.timestamp.toFixed(2)}`,
                negLabel: `node${nid}-t${neg.timestamp.toFixed(2)}`,
              });
              break; // One negative per positive
            }
          }
        }
      }
    }
  }

  // Strategy 3: Cross-node positive (same timestamp, different nodes)
  if (nodeIds.length >= 2) {
    const node1Frames = byNode[nodeIds[0]] || [];
    const node2Frames = byNode[nodeIds[1]] || [];

    for (const f1 of node1Frames) {
      // Find node2 frame closest in time
      let bestMatch = null;
      let bestDist = Infinity;
      for (const f2 of node2Frames) {
        const dist = Math.abs(f2.timestamp - f1.timestamp);
        if (dist < bestDist) {
          bestDist = dist;
          bestMatch = f2;
        }
      }

      if (bestMatch && bestDist < config.positiveWindowSec) {
        // Find a cross-node negative (different time from different node)
        for (const f2neg of node2Frames) {
          const negDist = Math.abs(f2neg.timestamp - f1.timestamp);
          if (negDist >= config.negativeWindowSec) {
            triplets.push({
              anchor: f1.features,
              positive: bestMatch.features,
              negative: f2neg.features,
              isHard: false,
              type: 'cross-node',
              anchorLabel: `node${f1.nodeId}-t${f1.timestamp.toFixed(2)}`,
              posLabel: `node${bestMatch.nodeId}-t${bestMatch.timestamp.toFixed(2)}`,
              negLabel: `node${f2neg.nodeId}-t${f2neg.timestamp.toFixed(2)}`,
            });
            break;
          }
        }
      }
    }
  }

  // Strategy 5: Hard negatives near scenario transitions
  // Detect transitions via motion_energy spikes in vitals
  const sortedVitals = [...vitals].sort((a, b) => a.timestamp - b.timestamp);
  const transitionTimes = [];
  for (let i = 1; i < sortedVitals.length; i++) {
    const prev = sortedVitals[i - 1];
    const curr = sortedVitals[i];
    const energyDelta = Math.abs(curr.motionEnergy - prev.motionEnergy);
    if (energyDelta > 2.0) {
      transitionTimes.push(curr.timestamp);
    }
  }

  // Add hard negatives from transition boundaries
  for (const transTime of transitionTimes.slice(0, 50)) {
    for (const nid of nodeIds) {
      const frames = byNode[nid];
      let before = null, after = null;
      for (const f of frames) {
        if (f.timestamp < transTime) before = f;
        if (f.timestamp > transTime && !after) after = f;
      }
      if (before && after) {
        const anchorIdx = Math.max(0, frames.indexOf(before) - 5);
        const anchor = frames[anchorIdx];
        if (anchor) {
          triplets.push({
            anchor: anchor.features,
            positive: before.features,
            negative: after.features,
            isHard: true,
            type: 'transition-hard',
            anchorLabel: `node${nid}-pre-transition`,
            posLabel: `node${nid}-before`,
            negLabel: `node${nid}-after`,
          });
        }
      }
    }
  }

  // Strategy 6: Scenario boundary negatives — first 60s vs last 60s
  // Even if total recording is ~120s, first half differs from second half
  // in activity patterns.
  for (const nid of nodeIds) {
    const frames = byNode[nid];
    if (frames.length < 10) continue;
    const tMin = frames[0].timestamp;
    const tMax = frames[frames.length - 1].timestamp;
    const tMid = (tMin + tMax) / 2;

    const firstHalf = frames.filter(f => f.timestamp < tMid);
    const secondHalf = frames.filter(f => f.timestamp >= tMid);
    if (firstHalf.length < 3 || secondHalf.length < 3) continue;

    // Sample scenario boundary triplets
    const nBoundary = Math.min(50, firstHalf.length, secondHalf.length);
    for (let i = 0; i < nBoundary; i++) {
      const anchor = firstHalf[i];
      // Positive: nearby frame in same half
      const posIdx = Math.min(i + 1, firstHalf.length - 1);
      const positive = firstHalf[posIdx];
      // Negative: corresponding frame from other half
      const negIdx = Math.min(i, secondHalf.length - 1);
      const negative = secondHalf[negIdx];

      triplets.push({
        anchor: anchor.features,
        positive: positive.features,
        negative: negative.features,
        isHard: true,
        type: 'scenario-boundary',
        anchorLabel: `node${nid}-first-half-${i}`,
        posLabel: `node${nid}-first-half-${posIdx}`,
        negLabel: `node${nid}-second-half-${negIdx}`,
      });
    }
  }

  return triplets;
}

// ---------------------------------------------------------------------------
// Quantization (TurboQuant simulation)
// ---------------------------------------------------------------------------

/**
 * Quantize Float32Array to N-bit fixed point with actual bit-packing.
 *
 * Bit-packing:
 *   8-bit: 1 byte per weight  -> 4x compression vs fp32
 *   4-bit: 2 weights per byte -> 8x compression vs fp32
 *   2-bit: 4 weights per byte -> 16x compression vs fp32
 *
 * Returns { quantized: Uint8Array, scale, zeroPoint, bits, numWeights,
 *           originalSize, quantizedSize, compressionRatio }.
 */
function quantizeWeights(weights, bits) {
  const maxVal = 2 ** bits - 1; // unsigned range: 0..(2^bits - 1)

  let wMin = Infinity, wMax = -Infinity;
  for (let i = 0; i < weights.length; i++) {
    if (weights[i] < wMin) wMin = weights[i];
    if (weights[i] > wMax) wMax = weights[i];
  }

  const range = wMax - wMin || 1e-10;
  const scale = range / maxVal;
  const zeroPoint = Math.round(-wMin / scale);

  // Quantize to unsigned N-bit integers
  const qValues = new Uint8Array(weights.length); // temporary full-precision quantized
  for (let i = 0; i < weights.length; i++) {
    let q = Math.round((weights[i] - wMin) / scale);
    qValues[i] = Math.max(0, Math.min(maxVal, q));
  }

  // Bit-pack into Uint8Array
  let packedSize;
  let packed;

  if (bits === 8) {
    // 1 value per byte
    packedSize = weights.length;
    packed = new Uint8Array(packedSize);
    for (let i = 0; i < weights.length; i++) packed[i] = qValues[i];
  } else if (bits === 4) {
    // 2 values per byte (high nibble + low nibble)
    packedSize = Math.ceil(weights.length / 2);
    packed = new Uint8Array(packedSize);
    for (let i = 0; i < weights.length; i += 2) {
      const hi = qValues[i] & 0x0F;
      const lo = (i + 1 < weights.length) ? (qValues[i + 1] & 0x0F) : 0;
      packed[i >> 1] = (hi << 4) | lo;
    }
  } else if (bits === 2) {
    // 4 values per byte
    packedSize = Math.ceil(weights.length / 4);
    packed = new Uint8Array(packedSize);
    for (let i = 0; i < weights.length; i += 4) {
      let byte = 0;
      for (let k = 0; k < 4; k++) {
        const val = (i + k < weights.length) ? (qValues[i + k] & 0x03) : 0;
        byte |= val << (6 - k * 2);
      }
      packed[Math.floor(i / 4)] = byte;
    }
  } else {
    // Fallback: 1 byte per value
    packedSize = weights.length;
    packed = new Uint8Array(packedSize);
    for (let i = 0; i < weights.length; i++) packed[i] = qValues[i];
  }

  const originalSize = weights.length * 4; // fp32 = 4 bytes each

  return {
    quantized: packed,
    scale,
    zeroPoint,
    bits,
    numWeights: weights.length,
    originalSize,
    quantizedSize: packed.length,
    compressionRatio: originalSize / packed.length,
  };
}

/**
 * Dequantize bit-packed Uint8Array back to float for quality assessment.
 */
function dequantizeWeights(packed, scale, zeroPoint, bits, numWeights) {
  const result = new Float32Array(numWeights);

  if (bits === 8) {
    for (let i = 0; i < numWeights; i++) {
      result[i] = (packed[i] - zeroPoint) * scale;
    }
  } else if (bits === 4) {
    for (let i = 0; i < numWeights; i++) {
      const byteIdx = i >> 1;
      const nibble = (i % 2 === 0) ? (packed[byteIdx] >> 4) & 0x0F : packed[byteIdx] & 0x0F;
      result[i] = (nibble - zeroPoint) * scale;
    }
  } else if (bits === 2) {
    for (let i = 0; i < numWeights; i++) {
      const byteIdx = Math.floor(i / 4);
      const shift = 6 - (i % 4) * 2;
      const val = (packed[byteIdx] >> shift) & 0x03;
      result[i] = (val - zeroPoint) * scale;
    }
  } else {
    for (let i = 0; i < numWeights; i++) {
      result[i] = (packed[i] - zeroPoint) * scale;
    }
  }

  return result;
}

/**
 * Compute quantization quality loss (RMSE between original and dequantized).
 */
function quantizationQuality(original, dequantized) {
  let sumSqErr = 0;
  const n = Math.min(original.length, dequantized.length);
  for (let i = 0; i < n; i++) {
    const diff = original[i] - dequantized[i];
    sumSqErr += diff * diff;
  }
  return Math.sqrt(sumSqErr / n);
}

// ---------------------------------------------------------------------------
// Training labels from vitals data
// ---------------------------------------------------------------------------

/**
 * Create task-head labels from vitals data for each feature frame.
 * Returns { presence: number, activity: number[], vitalsTarget: number[] }
 */
function createLabels(featureFrame, vitals) {
  // Find nearest vitals for this frame
  let nearest = null;
  let bestDist = Infinity;
  for (const v of vitals) {
    if (v.nodeId !== featureFrame.nodeId) continue;
    const dist = Math.abs(v.timestamp - featureFrame.timestamp);
    if (dist < bestDist) {
      bestDist = dist;
      nearest = v;
    }
  }

  if (!nearest || bestDist > 2.0) {
    return null; // No matching vitals within 2 seconds
  }

  // Presence: binary (threshold at 0.3)
  const presence = nearest.presenceScore > 0.3 ? 1.0 : 0.0;

  // Activity: [still, moving, empty] as one-hot
  let activity;
  if (nearest.presenceScore <= 0.1) {
    activity = [0, 0, 1]; // empty
  } else if (nearest.motionEnergy > 2.0) {
    activity = [0, 1, 0]; // moving
  } else {
    activity = [1, 0, 0]; // still
  }

  // Vitals: [breathing BPM normalized, heartrate BPM normalized]
  const vitalsTarget = [
    nearest.breathingBpm / 30.0,   // normalize to ~0-1 range
    nearest.heartrateBpm / 120.0,  // normalize to ~0-1 range
  ];

  return { presence, activity, vitalsTarget };
}

// ---------------------------------------------------------------------------
// Fix 5: Data augmentation — expand dataset via temporal, noise, cross-node
// ---------------------------------------------------------------------------

/**
 * Augment feature data by the given multiplier.
 *
 * Strategies:
 *   1. Temporal interpolation: blend consecutive frames (50% of augments)
 *   2. Gaussian noise: add small noise sigma=0.02 (30% of augments)
 *   3. Cross-node interpolation: blend node 1 & node 2 at same timestamp (20%)
 */
function augmentData(features, multiplier = 10) {
  if (features.length < 2 || multiplier <= 1) return features;

  const augmented = [...features]; // keep originals
  const targetSize = features.length * multiplier;
  const rng = { s: 7919 }; // deterministic seed for reproducibility
  const nextRand = () => {
    rng.s ^= rng.s << 13; rng.s ^= rng.s >> 17; rng.s ^= rng.s << 5;
    return (rng.s >>> 0) / 4294967296;
  };
  const nextGaussian = () => {
    // Box-Muller transform
    const u1 = nextRand() || 1e-10;
    const u2 = nextRand();
    return Math.sqrt(-2 * Math.log(u1)) * Math.cos(2 * Math.PI * u2);
  };

  // Index by node
  const byNode = {};
  for (const f of features) {
    if (!byNode[f.nodeId]) byNode[f.nodeId] = [];
    byNode[f.nodeId].push(f);
  }
  for (const nid of Object.keys(byNode)) {
    byNode[nid].sort((a, b) => a.timestamp - b.timestamp);
  }
  const nodeIds = Object.keys(byNode).map(Number);

  while (augmented.length < targetSize) {
    const strategy = nextRand();

    if (strategy < 0.5) {
      // Temporal interpolation: blend two consecutive frames
      const nid = nodeIds[Math.floor(nextRand() * nodeIds.length)];
      const frames = byNode[nid];
      if (frames.length < 2) continue;
      const idx = Math.floor(nextRand() * (frames.length - 1));
      const f1 = frames[idx];
      const f2 = frames[idx + 1];
      const alpha = 0.2 + nextRand() * 0.6; // blend factor 0.2-0.8
      const blended = f1.features.map((v, i) => v * alpha + (f2.features[i] || 0) * (1 - alpha));
      augmented.push({
        timestamp: f1.timestamp * alpha + f2.timestamp * (1 - alpha),
        nodeId: nid,
        features: blended,
        rssi: f1.rssi,
        seq: -1, // synthetic marker
      });
    } else if (strategy < 0.8) {
      // Gaussian noise augmentation
      const idx = Math.floor(nextRand() * features.length);
      const f = features[idx];
      const sigma = 0.02;
      const noisy = f.features.map(v => v + nextGaussian() * sigma);
      augmented.push({
        timestamp: f.timestamp + (nextRand() - 0.5) * 0.1, // slight jitter
        nodeId: f.nodeId,
        features: noisy,
        rssi: f.rssi,
        seq: -1,
      });
    } else {
      // Cross-node interpolation
      if (nodeIds.length < 2) {
        // Fallback to noise if only one node
        const idx = Math.floor(nextRand() * features.length);
        const f = features[idx];
        const noisy = f.features.map(v => v + nextGaussian() * 0.01);
        augmented.push({ ...f, features: noisy, seq: -1 });
        continue;
      }
      const n1 = nodeIds[0], n2 = nodeIds[1];
      const frames1 = byNode[n1], frames2 = byNode[n2];
      const idx1 = Math.floor(nextRand() * frames1.length);
      const f1 = frames1[idx1];
      // Find closest frame from node 2
      let bestIdx = 0, bestDist = Infinity;
      for (let j = 0; j < frames2.length; j++) {
        const d = Math.abs(frames2[j].timestamp - f1.timestamp);
        if (d < bestDist) { bestDist = d; bestIdx = j; }
      }
      if (bestDist < 2.0) {
        const f2 = frames2[bestIdx];
        const alpha = 0.3 + nextRand() * 0.4;
        const blended = f1.features.map((v, i) => v * alpha + (f2.features[i] || 0) * (1 - alpha));
        augmented.push({
          timestamp: f1.timestamp,
          nodeId: n1, // keep node 1 ID
          features: blended,
          rssi: Math.round(f1.rssi * alpha + f2.rssi * (1 - alpha)),
          seq: -1,
        });
      }
    }
  }

  return augmented;
}

// ---------------------------------------------------------------------------
// Fix 7: Collect more data from live UDP stream if dataset is too small
// ---------------------------------------------------------------------------

/**
 * Attempt to collect additional CSI features from a live UDP stream.
 * The ESP32 sensing server broadcasts features on port 5006 by default.
 * Times out after durationSec seconds. Returns collected features.
 */
async function collectLiveData(port = 5006, durationSec = 60) {
  let dgram;
  try {
    dgram = require('dgram');
  } catch (e) {
    console.log('  WARN: dgram not available, skipping live data collection.');
    return { features: [], vitals: [] };
  }

  return new Promise((resolve) => {
    const features = [];
    const vitals = [];
    const sock = dgram.createSocket('udp4');
    let resolved = false;

    const finish = () => {
      if (resolved) return;
      resolved = true;
      try { sock.close(); } catch (_) {}
      resolve({ features, vitals });
    };

    sock.on('message', (msg) => {
      try {
        const frame = JSON.parse(msg.toString());
        if (frame.type === 'feature') {
          features.push({
            timestamp: frame.timestamp,
            nodeId: frame.node_id,
            features: frame.features,
            rssi: frame.rssi,
            seq: frame.seq,
          });
        } else if (frame.type === 'vitals') {
          vitals.push({
            timestamp: frame.timestamp,
            nodeId: frame.node_id,
            breathingBpm: frame.breathing_bpm,
            heartrateBpm: frame.heartrate_bpm,
            nPersons: frame.n_persons,
            motionEnergy: frame.motion_energy,
            presenceScore: frame.presence_score,
            rssi: frame.rssi,
          });
        }
      } catch (_) {}
    });

    sock.on('error', () => finish());

    sock.bind(port, () => {
      console.log(`  Listening on UDP :${port} for ${durationSec}s to collect more data...`);
      setTimeout(finish, durationSec * 1000);
    });

    // If bind fails (port in use), just resolve empty
    setTimeout(() => finish(), (durationSec + 2) * 1000);
  });
}

// ---------------------------------------------------------------------------
// Main pipeline
// ---------------------------------------------------------------------------

async function main() {
  const startTime = Date.now();
  console.log('=== WiFi-DensePose CSI Training Pipeline (ruvllm) ===');
  console.log(`Config: epochs=${CONFIG.epochs} batch=${CONFIG.batchSize} lora_rank=${CONFIG.loraRank} quant=${CONFIG.quantizeBits}bit`);
  console.log('');

  // -----------------------------------------------------------------------
  // Step 1: Load CSI data
  // -----------------------------------------------------------------------
  console.log('[1/9] Loading CSI data...');
  const files = resolveGlob(CONFIG.dataGlob);
  if (files.length === 0) {
    console.error(`No files found matching: ${CONFIG.dataGlob}`);
    process.exit(1);
  }

  let allFeatures = [];
  let allVitals = [];
  let allRawCsi = [];

  for (const file of files) {
    console.log(`  Loading: ${path.basename(file)}`);
    const { features, vitals, rawCsi } = loadCsiData(file);
    allFeatures = allFeatures.concat(features);
    allVitals = allVitals.concat(vitals);
    allRawCsi = allRawCsi.concat(rawCsi);
  }

  console.log(`  Loaded: ${allFeatures.length} features, ${allVitals.length} vitals, ${allRawCsi.length} raw CSI frames`);
  console.log(`  Nodes: ${[...new Set(allFeatures.map(f => f.nodeId))].join(', ')}`);

  if (allFeatures.length === 0) {
    console.error('No feature frames found in data. Ensure data contains type="feature" frames.');
    process.exit(1);
  }

  // -----------------------------------------------------------------------
  // Step 1b (Fix 7): Collect more data from live UDP stream if dataset small
  // -----------------------------------------------------------------------
  if (allFeatures.length < 500) {
    console.log(`\n[1b/9] Dataset has only ${allFeatures.length} features (<500), attempting live data collection...`);
    try {
      const live = await collectLiveData(5006, 60);
      if (live.features.length > 0) {
        allFeatures = allFeatures.concat(live.features);
        allVitals = allVitals.concat(live.vitals);
        console.log(`  Collected ${live.features.length} additional features, ${live.vitals.length} vitals from UDP.`);
        console.log(`  Total: ${allFeatures.length} features, ${allVitals.length} vitals`);
      } else {
        console.log('  No live data received (ESP32 may not be streaming). Proceeding with existing data.');
      }
    } catch (e) {
      console.log(`  Live collection failed: ${e.message}. Proceeding with existing data.`);
    }
  }

  // -----------------------------------------------------------------------
  // Step 1c (Fix 5): Augment data to expand training set
  // -----------------------------------------------------------------------
  const originalCount = allFeatures.length;
  allFeatures = augmentData(allFeatures, CONFIG.augmentMultiplier);
  console.log(`\n[1c/9] Data augmentation: ${originalCount} -> ${allFeatures.length} features (${CONFIG.augmentMultiplier}x)`);

  // -----------------------------------------------------------------------
  // Step 2: Generate contrastive triplets
  // -----------------------------------------------------------------------
  console.log('\n[2/9] Generating contrastive triplets...');
  const triplets = generateTriplets(allFeatures, allVitals, CONFIG);

  const temporalCount = triplets.filter(t => t.type === 'temporal').length;
  const crossNodeCount = triplets.filter(t => t.type === 'cross-node').length;
  const scenarioBoundaryCount = triplets.filter(t => t.type === 'scenario-boundary').length;
  const hardCount = triplets.filter(t => t.isHard).length;

  console.log(`  Total triplets: ${triplets.length}`);
  console.log(`  Temporal: ${temporalCount}, Cross-node: ${crossNodeCount}, Scenario-boundary: ${scenarioBoundaryCount}`);
  console.log(`  Hard negatives: ${hardCount} (${(hardCount / (triplets.length || 1) * 100).toFixed(1)}%)`);

  if (triplets.length === 0) {
    console.error('No triplets generated. Data may lack temporal diversity (need >30s span).');
    process.exit(1);
  }

  // -----------------------------------------------------------------------
  // Step 3: Build encoder and encode features
  // -----------------------------------------------------------------------
  console.log('\n[3/9] Building CSI encoder (8 -> 64 -> 128)...');
  const encoder = new CsiEncoder(CONFIG.inputDim, CONFIG.hiddenDim, CONFIG.embeddingDim);

  // Pre-encode all features using batch mode (initializes BN running stats)
  console.log('  Encoding feature vectors (batch mode for BN stats)...');
  const encodingStart = Date.now();
  // Process in batches of 64 to compute proper BN statistics
  const allInputs = allFeatures.map(f => f.features);
  const batchSizeEnc = 64;
  let allEmbeddings = [];
  for (let i = 0; i < allInputs.length; i += batchSizeEnc) {
    const batch = allInputs.slice(i, i + batchSizeEnc);
    const batchEmbs = encoder.encodeBatch(batch);
    allEmbeddings = allEmbeddings.concat(batchEmbs);
  }
  const encodedFeatures = allFeatures.map((f, i) => ({
    ...f,
    embedding: allEmbeddings[i],
  }));
  console.log(`  Encoded ${encodedFeatures.length} frames in ${Date.now() - encodingStart}ms`);

  // -----------------------------------------------------------------------
  // Phase 1: Contrastive pretraining (Fix 1: Actually update encoder weights)
  // -----------------------------------------------------------------------
  console.log('\n[4/9] Phase 1: Contrastive pretraining...');

  // First, run the ruvllm ContrastiveTrainer to compute loss metrics
  const contrastiveTrainer = new ContrastiveTrainer({
    epochs: CONFIG.epochs,
    batchSize: CONFIG.batchSize,
    margin: CONFIG.margin,
    temperature: CONFIG.temperature,
    hardNegativeRatio: CONFIG.hardNegativeRatio,
    learningRate: CONFIG.learningRate,
    outputPath: path.join(CONFIG.outputDir, 'contrastive'),
  });

  for (const triplet of triplets) {
    const anchorEmb = encoder.encode(triplet.anchor);
    const posEmb = encoder.encode(triplet.positive);
    const negEmb = encoder.encode(triplet.negative);
    contrastiveTrainer.addTriplet(
      triplet.anchorLabel, anchorEmb,
      triplet.posLabel, posEmb,
      triplet.negLabel, negEmb,
      triplet.isHard
    );
  }

  console.log(`  Triplets loaded: ${contrastiveTrainer.getTripletCount()}`);
  const contrastiveResult = contrastiveTrainer.train();
  console.log(`  Contrastive trainer baseline loss: ${contrastiveResult.initialLoss.toFixed(6)}`);

  // Now ACTUALLY update encoder weights using gradient descent on triplets.
  // The ContrastiveTrainer.train() computes losses but doesn't update our encoder.
  // We iterate over triplets, compute gradients via computeGradient(), and apply
  // them to update the encoder's w2 layer (the embedding projection layer).
  console.log('  Applying gradient updates to encoder weights...');
  // Same bug class as lossHistory: contrastiveResult.durationMs times the
  // no-op contrastiveTrainer.train() baseline pass above, not this loop --
  // the actual gradient-update work. Time it here instead.
  const contrastiveLoopStart = Date.now();

  const contrastiveLr = CONFIG.learningRate;
  const contrastiveEpochs = CONFIG.epochs;
  let initialContrastiveLoss = 0;
  let finalContrastiveLoss = 0;
  // Real per-epoch loss from the loop that actually updates encoder.w2 below —
  // NOT contrastiveResult.history, which comes from contrastiveTrainer.train()
  // above and never touches the encoder (see its own comment). Exporting that
  // instead of this produced a flat, meaningless lossHistory in the published
  // model's training-metrics.json.
  const realContrastiveLossHistory = [];

  // Compute initial loss
  for (const triplet of triplets) {
    initialContrastiveLoss += tripletLoss(
      encoder.encode(triplet.anchor),
      encoder.encode(triplet.positive),
      encoder.encode(triplet.negative),
      CONFIG.margin
    );
  }
  initialContrastiveLoss /= triplets.length || 1;

  for (let epoch = 0; epoch < contrastiveEpochs; epoch++) {
    let epochLoss = 0;

    // Shuffle triplets each epoch (deterministic shuffle with epoch as seed)
    const shuffled = [...triplets];
    let shuffleSeed = epoch * 31 + 17;
    for (let i = shuffled.length - 1; i > 0; i--) {
      shuffleSeed ^= shuffleSeed << 13; shuffleSeed ^= shuffleSeed >> 17; shuffleSeed ^= shuffleSeed << 5;
      const j = (shuffleSeed >>> 0) % (i + 1);
      [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
    }

    for (const triplet of shuffled) {
      const anchorEmb = encoder.encode(triplet.anchor);
      const posEmb = encoder.encode(triplet.positive);
      const negEmb = encoder.encode(triplet.negative);

      const loss = tripletLoss(anchorEmb, posEmb, negEmb, CONFIG.margin);
      epochLoss += loss;

      if (loss > 0) {
        // Compute gradient and apply to encoder w2 weights
        const grad = computeGradient(anchorEmb, posEmb, negEmb, contrastiveLr);

        // Update w2 weights: for each hidden unit i, output unit j,
        // w2[i][j] += grad[j] * hidden_activation (approximated by anchor embedding direction)
        // This is a simplified gradient update that pushes the encoder's output layer
        // to produce embeddings that respect the triplet constraint.
        const { hidden } = encoder.encodeRaw(triplet.anchor);

        for (let j = 0; j < encoder.outputDim; j++) {
          for (let i = 0; i < encoder.hiddenDim; i++) {
            if (hidden[i] > 0) { // Only update for active ReLU neurons
              encoder.w2[i * encoder.outputDim + j] += grad[j] * hidden[i] * 0.01;
            }
          }
          encoder.b2[j] += grad[j] * 0.01;
        }
      }
    }

    epochLoss /= shuffled.length || 1;

    if (epoch === contrastiveEpochs - 1 || epoch % 5 === 0) {
      if (CONFIG.verbose) console.log(`    Epoch ${epoch + 1}/${contrastiveEpochs}: loss=${epochLoss.toFixed(6)}`);
    }
    finalContrastiveLoss = epochLoss;
    realContrastiveLossHistory.push({ epoch: epoch + 1, loss: epochLoss });
  }
  const realContrastiveDurationMs = Date.now() - contrastiveLoopStart;

  // Re-encode all features with updated encoder
  console.log('  Re-encoding features with updated encoder...');
  const reEncodedInputs = allFeatures.map(f => f.features);
  let reEncodedEmbs = [];
  for (let i = 0; i < reEncodedInputs.length; i += batchSizeEnc) {
    const batch = reEncodedInputs.slice(i, i + batchSizeEnc);
    reEncodedEmbs = reEncodedEmbs.concat(encoder.encodeBatch(batch));
  }
  for (let i = 0; i < encodedFeatures.length; i++) {
    encodedFeatures[i].embedding = reEncodedEmbs[i];
  }

  const contrastiveImprovement = initialContrastiveLoss > 0
    ? ((initialContrastiveLoss - finalContrastiveLoss) / initialContrastiveLoss * 100)
    : 0;

  console.log(`  Initial loss: ${initialContrastiveLoss.toFixed(6)}`);
  console.log(`  Final loss: ${finalContrastiveLoss.toFixed(6)}`);
  console.log(`  Improvement: ${contrastiveImprovement.toFixed(1)}%`);

  // Override contrastive result values for downstream use
  contrastiveResult.initialLoss = initialContrastiveLoss;
  contrastiveResult.finalLoss = finalContrastiveLoss;
  contrastiveResult.improvement = contrastiveImprovement;

  // Export contrastive training data (skip for large datasets to avoid JSON string limit)
  if (contrastiveTrainer.getTripletCount() < 100000) {
    const contrastiveOutDir = contrastiveTrainer.exportTrainingData();
    console.log(`  Training data exported to: ${contrastiveOutDir}`);
  } else {
    console.log(`  Skipping triplet export (${contrastiveTrainer.getTripletCount()} triplets too large for JSON)`);
  }

  // -----------------------------------------------------------------------
  // Phase 2: Task head training via TrainingPipeline
  // -----------------------------------------------------------------------
  console.log('\n[5/9] Phase 2: Task head training...');

  // Create LoRA adapter for the task heads: 128-dim input, 128-dim output
  const taskAdapter = new LoraAdapter(
    { rank: CONFIG.loraRank * 2, alpha: CONFIG.loraRank * 4, dropout: 0.05, targetModules: ['encoder', 'task_heads'] },
    CONFIG.embeddingDim,
    CONFIG.embeddingDim
  );

  const taskPipeline = new TrainingPipeline({
    learningRate: CONFIG.learningRate,
    batchSize: CONFIG.batchSize,
    epochs: Math.max(5, Math.floor(CONFIG.epochs / 2)),
    scheduler: 'cosine',
    warmupSteps: 50,
    earlyStoppingPatience: 5,
    checkpointInterval: 2,
    ewcLambda: 2000,
    validationSplit: 0.1,
  }, taskAdapter);

  // Build training data: input = encoded feature, target = task labels
  let labeledCount = 0;
  const taskTrainingData = [];

  for (const ef of encodedFeatures) {
    const labels = createLabels(ef, allVitals);
    if (!labels) continue;

    // Construct target vector: [presence(1), activity(3), vitals(2), padding(122)]
    // Total: 128-dim to match adapter output dim
    const target = new Array(CONFIG.embeddingDim).fill(0);
    target[0] = labels.presence;
    target[1] = labels.activity[0]; // still
    target[2] = labels.activity[1]; // moving
    target[3] = labels.activity[2]; // empty
    target[4] = labels.vitalsTarget[0]; // breathing normalized
    target[5] = labels.vitalsTarget[1]; // heartrate normalized

    taskTrainingData.push({
      input: ef.embedding,
      target,
      quality: 1.0,
    });
    labeledCount++;
  }

  console.log(`  Labeled samples: ${labeledCount} / ${encodedFeatures.length} (${(labeledCount / encodedFeatures.length * 100).toFixed(1)}%)`);

  if (taskTrainingData.length > 0) {
    taskPipeline.addData(taskTrainingData);
    const taskResult = taskPipeline.train();

    console.log(`  Epochs completed: ${taskResult.epochs}`);
    console.log(`  Final loss: ${taskResult.finalLoss.toFixed(6)}`);
    console.log(`  Best val loss: ${taskResult.bestValLoss.toFixed(6)}`);
    console.log(`  Early stopped: ${taskResult.earlyStopped}`);
    console.log(`  Duration: ${taskResult.durationMs}ms`);
  } else {
    console.log('  WARN: No labeled data available, skipping task head training.');
  }

  // -----------------------------------------------------------------------
  // Phase 2b (Fix 3): Train dedicated PresenceHead (128 -> 1, sigmoid)
  // -----------------------------------------------------------------------
  console.log('\n[5b/9] Phase 2b: Presence head training...');
  const presenceHead = new PresenceHead(CONFIG.embeddingDim);

  const presenceTrainData = [];
  for (const ef of encodedFeatures) {
    const labels = createLabels(ef, allVitals);
    if (!labels) continue;
    presenceTrainData.push({ embedding: ef.embedding, target: labels.presence });
  }

  if (presenceTrainData.length > 0) {
    const presenceEpochs = 30;
    const presenceLr = 0.01;
    let presenceLoss = 0;

    for (let epoch = 0; epoch < presenceEpochs; epoch++) {
      presenceLoss = 0;
      // Shuffle each epoch
      let pSeed = epoch * 41 + 7;
      const pShuffled = [...presenceTrainData];
      for (let i = pShuffled.length - 1; i > 0; i--) {
        pSeed ^= pSeed << 13; pSeed ^= pSeed >> 17; pSeed ^= pSeed << 5;
        const j = (pSeed >>> 0) % (i + 1);
        [pShuffled[i], pShuffled[j]] = [pShuffled[j], pShuffled[i]];
      }

      for (const sample of pShuffled) {
        presenceLoss += presenceHead.trainStep(sample.embedding, sample.target, presenceLr);
      }
      presenceLoss /= pShuffled.length;

      // Decay learning rate
      if (epoch > 0 && epoch % 10 === 0) {
        // lr decay not needed with 30 epochs, but log progress
        if (CONFIG.verbose) console.log(`    Presence epoch ${epoch}: loss=${presenceLoss.toFixed(6)}`);
      }
    }

    // Evaluate presence accuracy
    let presCorrect = 0;
    for (const sample of presenceTrainData) {
      const pred = presenceHead.forward(sample.embedding) > 0.5 ? 1 : 0;
      if (pred === sample.target) presCorrect++;
    }
    const presAccuracy = (presCorrect / presenceTrainData.length * 100).toFixed(1);

    console.log(`  Presence samples: ${presenceTrainData.length}`);
    console.log(`  Final BCE loss: ${presenceLoss.toFixed(6)}`);
    console.log(`  Training accuracy: ${presAccuracy}%`);
  } else {
    console.log('  WARN: No presence labels available.');
  }

  // -----------------------------------------------------------------------
  // Phase 3: LoRA refinement (per-node room adaptation)
  // -----------------------------------------------------------------------
  console.log('\n[6/9] Phase 3: LoRA refinement (per-node adaptation)...');
  const loraManager = new LoraManager({
    rank: CONFIG.loraRank,
    alpha: CONFIG.loraRank * 2,
    dropout: 0.1,
    targetModules: ['room_adapt'],
  });

  const nodeIds = [...new Set(allFeatures.map(f => f.nodeId))];

  for (const nodeId of nodeIds) {
    console.log(`  Training LoRA adapter for node ${nodeId}...`);
    const nodeAdapter = loraManager.create(
      `node-${nodeId}`,
      { rank: CONFIG.loraRank, alpha: CONFIG.loraRank * 2, dropout: 0.1 },
      CONFIG.embeddingDim,
      CONFIG.embeddingDim
    );

    // Train on node-specific data
    const nodeFeatures = encodedFeatures.filter(f => f.nodeId === nodeId);
    const nodePipeline = new TrainingPipeline({
      learningRate: CONFIG.learningRate * 0.5,
      batchSize: Math.min(CONFIG.batchSize, nodeFeatures.length),
      epochs: 5,
      scheduler: 'cosine',
      ewcLambda: 3000,
    }, nodeAdapter);

    const nodeData = [];
    for (const nf of nodeFeatures) {
      const labels = createLabels(nf, allVitals);
      if (!labels) continue;
      const target = new Array(CONFIG.embeddingDim).fill(0);
      target[0] = labels.presence;
      target[1] = labels.activity[0];
      target[2] = labels.activity[1];
      target[3] = labels.activity[2];
      target[4] = labels.vitalsTarget[0];
      target[5] = labels.vitalsTarget[1];
      nodeData.push({ input: nf.embedding, target, quality: 1.0 });
    }

    if (nodeData.length > 0) {
      nodePipeline.addData(nodeData);
      const nodeResult = nodePipeline.train();
      console.log(`    Node ${nodeId}: ${nodeData.length} samples, loss=${nodeResult.finalLoss.toFixed(6)}, ${nodeResult.durationMs}ms`);
    }
  }

  console.log(`  LoRA adapters: ${loraManager.list().join(', ')}`);
  console.log(`  Total LoRA parameters: ${loraManager.stats().totalParameters}`);

  // -----------------------------------------------------------------------
  // Phase 4: Quantization (TurboQuant)
  // -----------------------------------------------------------------------
  console.log('\n[7/9] Phase 4: Quantization (TurboQuant)...');
  const mergedWeights = taskAdapter.merge();
  const flatWeights = new Float32Array(mergedWeights.flat());

  const quantResults = {};
  for (const bits of [2, 4, 8]) {
    const qr = quantizeWeights(flatWeights, bits);
    const deq = dequantizeWeights(qr.quantized, qr.scale, qr.zeroPoint, bits, qr.numWeights);
    const rmse = quantizationQuality(flatWeights, deq);
    quantResults[bits] = { ...qr, rmse };
    console.log(`  ${bits}-bit: compression=${qr.compressionRatio.toFixed(1)}x, RMSE=${rmse.toFixed(6)}, size=${(qr.quantizedSize / 1024).toFixed(1)}KB`);
  }

  // -----------------------------------------------------------------------
  // Phase 5: EWC consolidation
  // -----------------------------------------------------------------------
  console.log('\n[8/9] Phase 5: EWC consolidation...');
  const ewcManager = taskPipeline.getEwcManager();
  const ewcWeights = taskAdapter.merge().flat();
  ewcManager.registerTask('csi-pretraining-v1', ewcWeights);

  // Register per-node tasks for EWC protection
  for (const nodeId of nodeIds) {
    const nodeAdapter = loraManager.get(`node-${nodeId}`);
    if (nodeAdapter) {
      const nodeWeights = nodeAdapter.merge().flat();
      ewcManager.registerTask(`node-${nodeId}-adaptation`, nodeWeights);
    }
  }

  const ewcStats = ewcManager.stats();
  console.log(`  Tasks learned: ${ewcStats.tasksLearned}`);
  console.log(`  Fisher computed: ${ewcStats.fisherComputed}`);
  console.log(`  Protection strength: ${ewcStats.protectionStrength}`);
  console.log(`  Forgetting rate: ${ewcStats.forgettingRate.toFixed(4)}`);

  // -----------------------------------------------------------------------
  // Step 9: Export
  // -----------------------------------------------------------------------
  console.log('\n[9/9] Exporting models...');

  // Ensure output directory exists
  fs.mkdirSync(CONFIG.outputDir, { recursive: true });

  // 9a: SafeTensors export via ModelExporter
  const exporter = new ModelExporter();
  const exportModel = {
    metadata: {
      name: 'wifi-densepose-csi-embedding',
      version: '2.0.1',
      architecture: 'csi-encoder-8-64-128',
      training: {
        // Real epoch count and real per-epoch triplet-update count, reported
        // separately rather than pre-multiplied into one "steps" figure --
        // that reads as optimizer steps, but every triplet with loss>0 gets
        // an immediate direct weight update here, not a batched gradient step.
        epochs: realContrastiveLossHistory.length,
        tripletUpdatesPerEpoch: triplets.length,
        loss: contrastiveResult.finalLoss,
        learningRate: CONFIG.learningRate,
      },
      custom: {
        inputDim: CONFIG.inputDim,
        hiddenDim: CONFIG.hiddenDim,
        embeddingDim: CONFIG.embeddingDim,
        totalFrames: allFeatures.length,
        totalTriplets: triplets.length,
        nodes: nodeIds,
        quantizationBits: CONFIG.quantizeBits,
      },
    },
    loraWeights: taskAdapter.getWeights(),
    loraConfig: taskAdapter.getConfig(),
    ewcStats: ewcStats,
    tensors: new Map(),
  };

  // Add encoder weights as tensors
  exportModel.tensors.set('encoder.w1', new Float32Array(encoder.w1));
  exportModel.tensors.set('encoder.b1', new Float32Array(encoder.b1));
  exportModel.tensors.set('encoder.w2', new Float32Array(encoder.w2));
  exportModel.tensors.set('encoder.b2', new Float32Array(encoder.b2));

  // Batch norm parameters
  exportModel.tensors.set('encoder.bn1_gamma', new Float32Array(encoder.bn1_gamma));
  exportModel.tensors.set('encoder.bn1_beta', new Float32Array(encoder.bn1_beta));
  exportModel.tensors.set('encoder.bn1_runMean', new Float32Array(encoder.bn1_runMean));
  exportModel.tensors.set('encoder.bn1_runVar', new Float32Array(encoder.bn1_runVar));
  exportModel.tensors.set('encoder.bn2_gamma', new Float32Array(encoder.bn2_gamma));
  exportModel.tensors.set('encoder.bn2_beta', new Float32Array(encoder.bn2_beta));
  exportModel.tensors.set('encoder.bn2_runMean', new Float32Array(encoder.bn2_runMean));
  exportModel.tensors.set('encoder.bn2_runVar', new Float32Array(encoder.bn2_runVar));

  // Presence head weights (Fix 3)
  exportModel.tensors.set('presence_head.weights', new Float32Array(presenceHead.weights));
  exportModel.tensors.set('presence_head.bias', new Float32Array([presenceHead.bias]));

  // SafeTensors
  const safetensorsBuffer = exporter.toSafeTensors(exportModel);
  fs.writeFileSync(path.join(CONFIG.outputDir, 'model.safetensors'), safetensorsBuffer);
  console.log(`  SafeTensors: ${path.join(CONFIG.outputDir, 'model.safetensors')} (${(safetensorsBuffer.length / 1024).toFixed(1)} KB)`);

  // HuggingFace export
  const hfExport = exporter.toHuggingFace(exportModel);
  fs.writeFileSync(path.join(CONFIG.outputDir, 'config.json'), hfExport.config);
  console.log(`  HF config: ${path.join(CONFIG.outputDir, 'config.json')}`);

  // JSON export
  const jsonExport = exporter.toJSON(exportModel);
  fs.writeFileSync(path.join(CONFIG.outputDir, 'model.json'), jsonExport);

  // 9a2: Presence head JSON export
  const presenceHeadPath = path.join(CONFIG.outputDir, 'presence-head.json');
  fs.writeFileSync(presenceHeadPath, JSON.stringify(presenceHead.getWeights()));
  console.log(`  Presence head: ${presenceHeadPath}`);

  // 9b: Quantized models
  const quantDir = path.join(CONFIG.outputDir, 'quantized');
  fs.mkdirSync(quantDir, { recursive: true });

  for (const [bits, qr] of Object.entries(quantResults)) {
    const qPath = path.join(quantDir, `model-q${bits}.bin`);
    fs.writeFileSync(qPath, Buffer.from(qr.quantized));
    console.log(`  Quantized ${bits}-bit: ${qPath} (${(qr.quantizedSize / 1024).toFixed(1)} KB)`);
  }

  // 9c: Per-node LoRA adapters
  const loraDir = path.join(CONFIG.outputDir, 'lora');
  fs.mkdirSync(loraDir, { recursive: true });

  for (const adapterId of loraManager.list()) {
    const adapter = loraManager.get(adapterId);
    const loraPath = path.join(loraDir, `${adapterId}.json`);
    fs.writeFileSync(loraPath, adapter.toJSON());
    console.log(`  LoRA adapter: ${loraPath}`);
  }

  // 9d: RVF (RuVector Format) — JSONL for Cognitum Seed ingest
  const rvfPath = path.join(CONFIG.outputDir, 'model.rvf.jsonl');
  const rvfLines = [
    JSON.stringify({ type: 'metadata', ...exportModel.metadata }),
    JSON.stringify({ type: 'encoder', w1_shape: [CONFIG.inputDim, CONFIG.hiddenDim], w2_shape: [CONFIG.hiddenDim, CONFIG.embeddingDim] }),
    JSON.stringify({ type: 'lora', config: taskAdapter.getConfig(), parameters: taskAdapter.numParameters() }),
    JSON.stringify({ type: 'ewc', stats: ewcStats }),
    JSON.stringify({ type: 'quantization', default_bits: CONFIG.quantizeBits, variants: Object.keys(quantResults).map(Number) }),
  ];
  fs.writeFileSync(rvfPath, rvfLines.join('\n'));
  console.log(`  RVF manifest: ${rvfPath}`);

  // 9e: Training metrics
  const metricsPath = path.join(CONFIG.outputDir, 'training-metrics.json');
  const metrics = {
    timestamp: new Date().toISOString(),
    totalDurationMs: Date.now() - startTime,
    data: {
      files: files.map(f => path.basename(f)),
      totalFeatures: allFeatures.length,
      totalVitals: allVitals.length,
      totalRawCsi: allRawCsi.length,
      nodes: nodeIds,
    },
    contrastive: {
      triplets: triplets.length,
      temporal: temporalCount,
      crossNode: crossNodeCount,
      hardNegatives: hardCount,
      initialLoss: contrastiveResult.initialLoss,
      finalLoss: contrastiveResult.finalLoss,
      improvement: contrastiveResult.improvement,
      // Real wall-clock of the actual gradient loop -- contrastiveResult.durationMs
      // timed the no-op baseline pass instead, same bug as lossHistory above.
      durationMs: realContrastiveDurationMs,
      // The real per-epoch loss from the loop that actually updates the
      // encoder (see realContrastiveLossHistory above) -- contrastiveResult.history
      // comes from a separate, non-weight-updating baseline pass and was flat/
      // meaningless here.
      lossHistory: realContrastiveLossHistory,
    },
    taskHeads: taskTrainingData.length > 0 ? {
      samples: labeledCount,
      finalLoss: taskPipeline.getMetrics().trainLoss,
    } : null,
    lora: {
      adapters: loraManager.list(),
      totalParameters: loraManager.stats().totalParameters,
    },
    quantization: Object.fromEntries(
      Object.entries(quantResults).map(([bits, qr]) => [
        `q${bits}`,
        { compressionRatio: qr.compressionRatio, rmse: qr.rmse, sizeKB: qr.quantizedSize / 1024 },
      ])
    ),
    ewc: ewcStats,
    config: CONFIG,
  };
  fs.writeFileSync(metricsPath, JSON.stringify(metrics, null, 2));
  console.log(`  Metrics: ${metricsPath}`);

  // -----------------------------------------------------------------------
  // Summary
  // -----------------------------------------------------------------------
  const totalDuration = Date.now() - startTime;
  console.log('\n=== Training Complete ===');
  console.log(`  Total duration: ${(totalDuration / 1000).toFixed(1)}s`);
  console.log(`  Output directory: ${path.resolve(CONFIG.outputDir)}`);
  console.log(`  Model size (fp32): ${(safetensorsBuffer.length / 1024).toFixed(1)} KB`);
  console.log(`  Model size (q${CONFIG.quantizeBits}): ${(quantResults[CONFIG.quantizeBits]?.quantizedSize / 1024 || 0).toFixed(1)} KB`);
  console.log(`  LoRA adapters: ${loraManager.count()}`);
  console.log(`  EWC tasks protected: ${ewcStats.tasksLearned}`);

  // -----------------------------------------------------------------------
  // Optional benchmark
  // -----------------------------------------------------------------------
  if (CONFIG.benchmark) {
    console.log('\n=== Benchmark Mode ===');
    runBenchmark(encoder, taskAdapter, presenceHead, allFeatures, allVitals, quantResults);
  }
}

// ---------------------------------------------------------------------------
// Benchmark
// ---------------------------------------------------------------------------
function runBenchmark(encoder, adapter, presenceHead, features, vitals, quantResults) {
  const N = Math.min(1000, features.length);
  const testFeatures = features.slice(0, N);

  // Inference latency
  console.log(`\nInference latency (${N} samples):`);
  const latencies = [];
  for (const f of testFeatures) {
    const start = process.hrtime.bigint();
    const emb = encoder.encode(f.features);
    adapter.forward(emb);
    const elapsed = Number(process.hrtime.bigint() - start) / 1e6;
    latencies.push(elapsed);
  }

  latencies.sort((a, b) => a - b);
  const mean = latencies.reduce((a, b) => a + b, 0) / latencies.length;
  const p95 = latencies[Math.floor(latencies.length * 0.95)];
  const p99 = latencies[Math.floor(latencies.length * 0.99)];

  console.log(`  Mean:  ${mean.toFixed(3)}ms`);
  console.log(`  P95:   ${p95.toFixed(3)}ms`);
  console.log(`  P99:   ${p99.toFixed(3)}ms`);
  console.log(`  Throughput: ${(1000 / mean).toFixed(0)} embeddings/sec`);

  // Embedding quality: cosine similarity for temporal pairs
  console.log('\nEmbedding quality (temporal pairs):');
  let posSimilarities = [];
  let negSimilarities = [];

  for (let i = 0; i < Math.min(features.length - 1, 200); i++) {
    const f1 = features[i];
    const f2 = features[i + 1];
    const timeDiff = Math.abs(f2.timestamp - f1.timestamp);

    const emb1 = encoder.encode(f1.features);
    const emb2 = encoder.encode(f2.features);
    const sim = cosineSimilarity(emb1, emb2);

    if (timeDiff <= 1.0) {
      posSimilarities.push(sim);
    } else if (timeDiff >= CONFIG.negativeWindowSec) {
      negSimilarities.push(sim);
    }
  }

  if (posSimilarities.length > 0) {
    const avgPos = posSimilarities.reduce((a, b) => a + b, 0) / posSimilarities.length;
    console.log(`  Positive pair avg similarity: ${avgPos.toFixed(4)} (n=${posSimilarities.length})`);
  }
  if (negSimilarities.length > 0) {
    const avgNeg = negSimilarities.reduce((a, b) => a + b, 0) / negSimilarities.length;
    console.log(`  Negative pair avg similarity: ${avgNeg.toFixed(4)} (n=${negSimilarities.length})`);
  }

  // Presence detection accuracy (using trained PresenceHead)
  console.log('\nPresence detection accuracy:');
  let correct = 0, total = 0;
  for (const f of testFeatures) {
    const labels = createLabels(f, vitals);
    if (!labels) continue;

    const emb = encoder.encode(f.features);
    const presScore = presenceHead.forward(emb);
    const predicted = presScore > 0.5 ? 1 : 0;
    if (predicted === labels.presence) correct++;
    total++;
  }
  if (total > 0) {
    console.log(`  Accuracy: ${(correct / total * 100).toFixed(1)}% (${correct}/${total})`);
  }

  // Memory usage per quantization level
  console.log('\nMemory usage per quantization level:');
  console.log('  Bits | Size (KB) | Compression | RMSE');
  console.log('  -----|-----------|-------------|------');
  for (const [bits, qr] of Object.entries(quantResults)) {
    console.log(`  ${bits.padStart(4)} | ${(qr.quantizedSize / 1024).toFixed(1).padStart(9)} | ${qr.compressionRatio.toFixed(1).padStart(11)}x | ${qr.rmse.toFixed(6)}`);
  }
  console.log(`  fp32 | ${(quantResults[Object.keys(quantResults)[0]].originalSize / 1024).toFixed(1).padStart(9)} | ${' '.padStart(10)}1x | 0.000000`);
}

// Run
main().catch(err => {
  console.error('Training pipeline failed:', err);
  process.exit(1);
});
