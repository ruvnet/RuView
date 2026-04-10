#!/usr/bin/env node
/**
 * ADR-076: Multi-Node Graph Transformer for CSI Fusion
 *
 * Builds a graph from multiple ESP32 nodes and applies graph attention to
 * fuse their CSI feature vectors (either 8-dim hand-crafted or 128-dim CNN)
 * into a single multi-viewpoint representation.
 *
 * The graph structure:
 *   - Each ESP32 node = graph node with a feature vector
 *   - Edge between nodes weighted by cross-node correlation
 *   - Attention learns which node to trust more per prediction
 *
 * Modes:
 *   --live           Listen on UDP for real-time multi-node CSI
 *   --file FILE      Read from a .csi.jsonl recording with multiple node_ids
 *   --dim DIM        Feature dimension (8 for hand-crafted, 128 for CNN)
 *   --heads H        Number of attention heads (default: 4)
 *   --json           JSON output
 *
 * Usage:
 *   node scripts/mesh-graph-transformer.js --file data/recordings/pretrain-1775182186.csi.jsonl
 *   node scripts/mesh-graph-transformer.js --live --port 5006 --dim 128
 *
 * ADR: docs/adr/ADR-076-csi-spectrogram-embeddings.md
 */

'use strict';

const dgram = require('dgram');
const fs = require('fs');
const path = require('path');
const readline = require('readline');
const { parseArgs } = require('util');
const { parseRawCsiFrame } = require('./lib/raw-csi');

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------
const { values: args } = parseArgs({
  options: {
    file:     { type: 'string', short: 'f' },
    live:     { type: 'boolean', default: false },
    port:     { type: 'string', short: 'p', default: '5006' },
    dim:      { type: 'string', short: 'd', default: '8' },
    heads:    { type: 'string', short: 'h', default: '4' },
    window:   { type: 'string', short: 'w', default: '20' },
    json:     { type: 'boolean', default: false },
    limit:    { type: 'string', short: 'l' },
  },
  strict: true,
});

const FEAT_DIM = parseInt(args.dim, 10);
const NUM_HEADS = parseInt(args.heads, 10);
const WINDOW_SIZE = parseInt(args.window, 10);
const PORT = parseInt(args.port, 10);
const LIMIT = args.limit ? parseInt(args.limit, 10) : Infinity;
const JSON_OUTPUT = args.json;

// ADR-018 raw CSI parser

// ---------------------------------------------------------------------------
// IQ Parsing (shared with csi-spectrogram.js)
// ---------------------------------------------------------------------------

function parseIqHex(iqHex, nSubcarriers) {
  const amps = new Float32Array(nSubcarriers);
  for (let sc = 0; sc < nSubcarriers; sc++) {
    const offset = sc * 4;
    if (offset + 4 > iqHex.length) break;
    const iVal = parseInt(iqHex.substring(offset, offset + 2), 16);
    const qVal = parseInt(iqHex.substring(offset + 2, offset + 4), 16);
    amps[sc] = Math.sqrt(iVal * iVal + qVal * qVal);
  }
  return amps;
}

// ---------------------------------------------------------------------------
// 8-dim Hand-Crafted Feature Extraction
// ---------------------------------------------------------------------------

/**
 * Extract 8-dim feature vector from subcarrier amplitudes.
 * Matches the features used by seed_csi_bridge.py (ADR-069).
 * @param {Float32Array} amplitudes
 * @param {number} rssi
 * @returns {Float32Array}
 */
function extract8DimFeatures(amplitudes, rssi) {
  const n = amplitudes.length;
  if (n === 0) return new Float32Array(8);

  let sum = 0, sumSq = 0, maxAmp = 0;
  for (let i = 0; i < n; i++) {
    const v = amplitudes[i];
    sum += v;
    sumSq += v * v;
    if (v > maxAmp) maxAmp = v;
  }
  const mean = sum / n;
  const variance = sumSq / n - mean * mean;

  // Phase: approximate from I/Q sign pattern (simplified)
  const phaseMean = 0; // Would need raw I/Q for true phase
  const phaseVariance = 0;

  // Bandwidth: number of subcarriers above noise floor
  const noiseFloor = mean * 0.1;
  let bw = 0;
  for (let i = 0; i < n; i++) {
    if (amplitudes[i] > noiseFloor) bw++;
  }

  // Spectral centroid
  let weightedSum = 0;
  for (let i = 0; i < n; i++) {
    weightedSum += i * amplitudes[i];
  }
  const centroid = sum > 0 ? weightedSum / sum : n / 2;

  return new Float32Array([
    mean,
    variance,
    maxAmp,
    phaseMean,
    phaseVariance,
    bw / n,               // normalized bandwidth
    centroid / n,          // normalized centroid
    Math.abs(rssi) / 100,  // normalized RSSI
  ]);
}

// ---------------------------------------------------------------------------
// Graph Attention Layer (Pure JS, no WASM dependency)
// ---------------------------------------------------------------------------

/**
 * Multi-head graph attention network (GATv2-style).
 *
 * For a graph with N nodes each having D-dimensional features:
 *   1. Project features to Q, K, V using learned weights
 *   2. Compute attention scores with edge weight bias
 *   3. Aggregate via softmax-weighted sum
 *   4. Produce fused D-dimensional output
 */
class GraphAttentionLayer {
  /**
   * @param {number} inputDim - Feature dimension per node
   * @param {number} numHeads - Number of attention heads
   */
  constructor(inputDim, numHeads) {
    this.inputDim = inputDim;
    this.numHeads = numHeads;
    this.headDim = Math.max(1, Math.floor(inputDim / numHeads));

    // Initialize projection weights (Xavier uniform)
    this.Wq = this._initWeights(inputDim, this.headDim * numHeads);
    this.Wk = this._initWeights(inputDim, this.headDim * numHeads);
    this.Wv = this._initWeights(inputDim, this.headDim * numHeads);
    this.Wo = this._initWeights(this.headDim * numHeads, inputDim);

    // Edge weight bias scale
    this.edgeBiasScale = 0.5;
  }

  /** Xavier-uniform weight initialization. */
  _initWeights(rows, cols) {
    const limit = Math.sqrt(6 / (rows + cols));
    const w = new Float32Array(rows * cols);
    for (let i = 0; i < w.length; i++) {
      w[i] = (Math.random() * 2 - 1) * limit;
    }
    return { data: w, rows, cols };
  }

  /** Matrix-vector multiply: out = W * x. */
  _matvec(W, x) {
    const out = new Float32Array(W.rows);
    for (let r = 0; r < W.rows; r++) {
      let sum = 0;
      for (let c = 0; c < W.cols; c++) {
        sum += W.data[r * W.cols + c] * x[c];
      }
      out[r] = sum;
    }
    return out;
  }

  /**
   * Compute attention-fused output for a set of nodes.
   *
   * @param {Float32Array[]} nodeFeatures - Array of D-dim feature vectors, one per node
   * @param {Map<string, number>} edgeWeights - Map of "i-j" -> weight (cross-correlation)
   * @returns {{ fused: Float32Array, attentionWeights: number[][] }}
   */
  forward(nodeFeatures, edgeWeights) {
    const N = nodeFeatures.length;
    if (N === 0) return { fused: new Float32Array(this.inputDim), attentionWeights: [] };
    if (N === 1) return { fused: new Float32Array(nodeFeatures[0]), attentionWeights: [[1.0]] };

    const D = this.headDim;
    const H = this.numHeads;

    // Project to Q, K, V for each node
    const queries = nodeFeatures.map(f => this._matvec(this.Wq, f));
    const keys = nodeFeatures.map(f => this._matvec(this.Wk, f));
    const values = nodeFeatures.map(f => this._matvec(this.Wv, f));

    // Compute per-head attention scores with edge bias
    const scale = 1 / Math.sqrt(D);
    const allAttentionWeights = [];

    // Aggregate output per node (we produce a fused vector for each node)
    const nodeOutputs = [];

    for (let i = 0; i < N; i++) {
      const headOutputs = [];

      for (let h = 0; h < H; h++) {
        const hOff = h * D;

        // Compute attention scores from node i to all other nodes
        const scores = new Float32Array(N);
        for (let j = 0; j < N; j++) {
          let dot = 0;
          for (let d = 0; d < D; d++) {
            dot += queries[i][hOff + d] * keys[j][hOff + d];
          }
          // Add edge weight bias
          const edgeKey = i < j ? `${i}-${j}` : `${j}-${i}`;
          const ew = edgeWeights.get(edgeKey) || 0;
          scores[j] = dot * scale + ew * this.edgeBiasScale;
        }

        // Softmax
        let maxScore = -Infinity;
        for (let j = 0; j < N; j++) {
          if (scores[j] > maxScore) maxScore = scores[j];
        }
        let sumExp = 0;
        const attn = new Float32Array(N);
        for (let j = 0; j < N; j++) {
          attn[j] = Math.exp(scores[j] - maxScore);
          sumExp += attn[j];
        }
        for (let j = 0; j < N; j++) {
          attn[j] /= sumExp;
        }

        if (i === 0 && h === 0) {
          allAttentionWeights.push(Array.from(attn));
        }

        // Weighted sum of values
        const headOut = new Float32Array(D);
        for (let j = 0; j < N; j++) {
          for (let d = 0; d < D; d++) {
            headOut[d] += attn[j] * values[j][hOff + d];
          }
        }
        headOutputs.push(headOut);
      }

      // Concatenate heads
      const concat = new Float32Array(H * D);
      for (let h = 0; h < H; h++) {
        concat.set(headOutputs[h], h * D);
      }

      // Project back to input dimension
      nodeOutputs.push(this._matvec(this.Wo, concat));
    }

    // Fuse all node outputs via mean pooling
    const fused = new Float32Array(this.inputDim);
    for (let i = 0; i < N; i++) {
      for (let d = 0; d < this.inputDim; d++) {
        fused[d] += nodeOutputs[i][d] / N;
      }
    }

    return { fused, attentionWeights: allAttentionWeights };
  }
}

// ---------------------------------------------------------------------------
// Cross-Node Correlation
// ---------------------------------------------------------------------------

/**
 * Compute Pearson correlation between two amplitude vectors.
 * Used as edge weight in the graph.
 */
function pearsonCorrelation(a, b) {
  const n = Math.min(a.length, b.length);
  if (n === 0) return 0;

  let sumA = 0, sumB = 0, sumAB = 0, sumA2 = 0, sumB2 = 0;
  for (let i = 0; i < n; i++) {
    sumA += a[i];
    sumB += b[i];
    sumAB += a[i] * b[i];
    sumA2 += a[i] * a[i];
    sumB2 += b[i] * b[i];
  }

  const num = n * sumAB - sumA * sumB;
  const den = Math.sqrt((n * sumA2 - sumA * sumA) * (n * sumB2 - sumB * sumB));
  return den > 0 ? num / den : 0;
}

// ---------------------------------------------------------------------------
// Graph Builder
// ---------------------------------------------------------------------------

/**
 * Build and maintain a graph of ESP32 nodes.
 * Stores the latest feature vector per node and computes edge weights.
 */
class MeshGraph {
  constructor(featDim, numHeads) {
    this.featDim = featDim;
    /** @type {Map<number, { features: Float32Array, amplitudes: Float32Array, rssi: number, timestamp: number }>} */
    this.nodes = new Map();
    this.attention = new GraphAttentionLayer(featDim, numHeads);
    this.fusionCount = 0;
  }

  /**
   * Update a node's features.
   * @param {number} nodeId
   * @param {Float32Array} features - D-dim feature vector
   * @param {Float32Array} amplitudes - Raw subcarrier amplitudes (for cross-correlation)
   * @param {number} rssi
   * @param {number} timestamp
   */
  updateNode(nodeId, features, amplitudes, rssi, timestamp) {
    this.nodes.set(nodeId, { features, amplitudes, rssi, timestamp });
  }

  /**
   * Compute edge weights between all node pairs.
   * @returns {Map<string, number>}
   */
  computeEdgeWeights() {
    const weights = new Map();
    const nodeIds = Array.from(this.nodes.keys()).sort();

    for (let i = 0; i < nodeIds.length; i++) {
      for (let j = i + 1; j < nodeIds.length; j++) {
        const a = this.nodes.get(nodeIds[i]);
        const b = this.nodes.get(nodeIds[j]);
        const corr = pearsonCorrelation(a.amplitudes, b.amplitudes);
        weights.set(`${i}-${j}`, corr);
      }
    }

    return weights;
  }

  /**
   * Run graph attention to produce a fused feature vector.
   * @returns {{ fused: Float32Array, attentionWeights: number[][], nodeIds: number[], edgeWeights: Map<string, number> } | null}
   */
  fuse() {
    if (this.nodes.size < 2) return null;

    const nodeIds = Array.from(this.nodes.keys()).sort();
    const features = nodeIds.map(id => this.nodes.get(id).features);
    const edgeWeights = this.computeEdgeWeights();

    const { fused, attentionWeights } = this.attention.forward(features, edgeWeights);
    this.fusionCount++;

    return { fused, attentionWeights, nodeIds, edgeWeights };
  }

  /** Pretty-print graph state. */
  toString() {
    const nodeIds = Array.from(this.nodes.keys()).sort();
    const lines = [`Graph: ${nodeIds.length} nodes [${nodeIds.join(', ')}]`];

    if (nodeIds.length >= 2) {
      const edgeWeights = this.computeEdgeWeights();
      for (const [key, weight] of edgeWeights) {
        const [i, j] = key.split('-').map(Number);
        lines.push(`  Edge ${nodeIds[i]}->${nodeIds[j]}: correlation=${weight.toFixed(4)}`);
      }
    }

    return lines.join('\n');
  }
}

// ---------------------------------------------------------------------------
// Optional: Graph-WASM Visualization
// ---------------------------------------------------------------------------

let graphDb = null;

/**
 * Initialize @ruvector/graph-wasm for persistent graph storage.
 * Optional -- only used if the WASM file exists.
 */
async function initGraphDb() {
  try {
    const graphWasmPath = path.resolve(
      __dirname, '..', 'vendor', 'ruvector', 'npm', 'packages', 'graph-wasm'
    );
    const graphWasm = require(graphWasmPath);
    await graphWasm.default();
    graphDb = new graphWasm.GraphDB('cosine');
    if (!JSON_OUTPUT) console.log('[graph-wasm] Initialized persistent graph DB');
    return true;
  } catch {
    if (!JSON_OUTPUT) console.log('[graph-wasm] Not available, using in-memory graph only');
    return false;
  }
}

/**
 * Persist the mesh graph to @ruvector/graph-wasm.
 * @param {MeshGraph} mesh
 * @param {object} fusionResult
 */
function persistToGraphDb(mesh, fusionResult) {
  if (!graphDb) return;

  const { nodeIds, edgeWeights, fused, attentionWeights } = fusionResult;

  // Create/update nodes
  for (const nodeId of nodeIds) {
    const node = mesh.nodes.get(nodeId);
    const existingId = `esp32-node-${nodeId}`;
    try { graphDb.deleteNode(existingId); } catch { /* ignore */ }
    graphDb.createNode(['ESP32', 'SensingNode'], {
      id: existingId,
      node_id: nodeId,
      rssi: node.rssi,
      timestamp: node.timestamp,
      feature_dim: mesh.featDim,
    });
  }

  // Create edges with correlation weights
  for (const [key, weight] of edgeWeights) {
    const [i, j] = key.split('-').map(Number);
    try {
      graphDb.createEdge(
        `esp32-node-${nodeIds[i]}`,
        `esp32-node-${nodeIds[j]}`,
        'CSI_CORRELATION',
        { weight, fusion_count: mesh.fusionCount }
      );
    } catch { /* ignore duplicate edges */ }
  }
}

// ---------------------------------------------------------------------------
// File Mode
// ---------------------------------------------------------------------------

async function processFile(filePath) {
  await initGraphDb();

  const mesh = new MeshGraph(FEAT_DIM, NUM_HEADS);
  const rl = readline.createInterface({
    input: fs.createReadStream(filePath),
    crlfDelay: Infinity,
  });

  let frameCount = 0;
  let fusionCount = 0;
  const nodeFrameCounts = new Map();

  for await (const line of rl) {
    if (frameCount >= LIMIT) break;

    let frame;
    try {
      frame = JSON.parse(line);
    } catch {
      continue;
    }

    const nodeId = frame.node_id || 0;
    const nSubcarriers = frame.subcarriers || 64;
    const iqHex = frame.iq_hex || '';
    if (!iqHex) continue;

    const amplitudes = parseIqHex(iqHex, nSubcarriers);
    const rssi = frame.rssi || 0;

    // Extract feature vector based on configured dimension
    let features;
    if (FEAT_DIM === 8) {
      features = extract8DimFeatures(amplitudes, rssi);
    } else {
      // For CNN embeddings, we need the csi-spectrogram.js pipeline.
      // In file mode without CNN, use padded 8-dim features as a placeholder.
      const base = extract8DimFeatures(amplitudes, rssi);
      features = new Float32Array(FEAT_DIM);
      features.set(base.subarray(0, Math.min(8, FEAT_DIM)));
    }

    mesh.updateNode(nodeId, features, amplitudes, rssi, frame.timestamp || 0);
    frameCount++;

    const nc = (nodeFrameCounts.get(nodeId) || 0) + 1;
    nodeFrameCounts.set(nodeId, nc);

    // Attempt fusion every WINDOW_SIZE frames (when we have data from multiple nodes)
    if (frameCount % WINDOW_SIZE === 0 && mesh.nodes.size >= 2) {
      const result = mesh.fuse();
      if (result) {
        fusionCount++;
        persistToGraphDb(mesh, result);

        if (JSON_OUTPUT) {
          console.log(JSON.stringify({
            type: 'fusion',
            fusionIdx: fusionCount,
            nodeIds: result.nodeIds,
            edgeWeights: Object.fromEntries(result.edgeWeights),
            attentionWeights: result.attentionWeights,
            fused: Array.from(result.fused).map(v => +v.toFixed(6)),
          }));
        } else {
          console.log(`\n[fusion ${fusionCount}] ${mesh.toString()}`);
          if (result.attentionWeights.length > 0) {
            const aw = result.attentionWeights[0].map(w => w.toFixed(3));
            console.log(`  Attention (head 0): [${aw.join(', ')}]`);
          }
          const fusedSnippet = Array.from(result.fused.subarray(0, 4)).map(v => v.toFixed(4)).join(', ');
          console.log(`  Fused: [${fusedSnippet}, ...] (dim=${FEAT_DIM})`);
        }
      }
    }
  }

  if (!JSON_OUTPUT) {
    console.log(`\nProcessed ${frameCount} frames from ${nodeFrameCounts.size} nodes`);
    console.log(`Produced ${fusionCount} fusions with ${NUM_HEADS}-head attention`);
    for (const [nodeId, count] of nodeFrameCounts) {
      console.log(`  Node ${nodeId}: ${count} frames`);
    }
    if (graphDb) {
      const stats = graphDb.stats();
      console.log(`Graph DB: ${stats.nodeCount} nodes, ${stats.edgeCount} edges`);
    }
  }
}

// ---------------------------------------------------------------------------
// Live Mode
// ---------------------------------------------------------------------------

async function processLive() {
  await initGraphDb();

  const mesh = new MeshGraph(FEAT_DIM, NUM_HEADS);
  let frameCount = 0;
  let fusionCount = 0;

  const server = dgram.createSocket('udp4');

  server.on('message', (msg) => {
    let nodeId, nSubcarriers, amplitudes, rssi;

    // Try binary ADR-018 format
    const csiFrame = parseRawCsiFrame(msg, { includePhase: false });
    if (csiFrame) {
      nodeId = csiFrame.nodeId;
      rssi = csiFrame.rssi;
      nSubcarriers = csiFrame.nSubcarriers;
      amplitudes = Float32Array.from(csiFrame.amplitudes);
    } else {
      // Try JSONL
      try {
        const frame = JSON.parse(msg.toString());
        nodeId = frame.node_id || 0;
        nSubcarriers = frame.subcarriers || 64;
        amplitudes = parseIqHex(frame.iq_hex || '', nSubcarriers);
        rssi = frame.rssi || 0;
      } catch {
        return;
      }
    }

    let features;
    if (FEAT_DIM === 8) {
      features = extract8DimFeatures(amplitudes, rssi);
    } else {
      const base = extract8DimFeatures(amplitudes, rssi);
      features = new Float32Array(FEAT_DIM);
      features.set(base.subarray(0, Math.min(8, FEAT_DIM)));
    }

    mesh.updateNode(nodeId, features, amplitudes, rssi, Date.now() / 1000);
    frameCount++;

    if (frameCount % WINDOW_SIZE === 0 && mesh.nodes.size >= 2) {
      const result = mesh.fuse();
      if (result) {
        fusionCount++;
        persistToGraphDb(mesh, result);

        if (JSON_OUTPUT) {
          console.log(JSON.stringify({
            type: 'fusion',
            fusionIdx: fusionCount,
            nodeIds: result.nodeIds,
            edgeWeights: Object.fromEntries(result.edgeWeights),
            attentionWeights: result.attentionWeights,
            fused: Array.from(result.fused).map(v => +v.toFixed(6)),
          }));
        } else {
          console.log(`[fusion ${fusionCount}] nodes=${result.nodeIds.join(',')}` +
            ` corr=${Array.from(result.edgeWeights.values()).map(v => v.toFixed(3)).join(',')}`);
        }
      }
    }
  });

  server.on('listening', () => {
    const addr = server.address();
    console.log(`[live] Mesh graph transformer on UDP ${addr.address}:${addr.port}`);
    console.log(`[live] Feature dim: ${FEAT_DIM}, heads: ${NUM_HEADS}, window: ${WINDOW_SIZE}`);
  });

  server.bind(PORT);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
  if (!args.file && !args.live) {
    console.error('Usage: node scripts/mesh-graph-transformer.js --file <path> [--dim 8|128] [--heads 4]');
    console.error('       node scripts/mesh-graph-transformer.js --live [--port 5006] [--dim 128]');
    process.exit(1);
  }

  if (args.file) {
    const filePath = path.resolve(args.file);
    if (!fs.existsSync(filePath)) {
      console.error(`File not found: ${filePath}`);
      process.exit(1);
    }
    await processFile(filePath);
  } else {
    await processLive();
  }
}

main().catch((err) => {
  console.error('Fatal:', err);
  process.exit(1);
});
