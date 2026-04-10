#!/usr/bin/env node
/**
 * ADR-075: Min-Cut Person Counter — Subcarrier correlation graph partitioning
 *
 * Fixes issue #348: n_persons always shows 4. Instead of threshold-based
 * counting, builds a subcarrier correlation graph and uses Stoer-Wagner
 * min-cut to find naturally independent groups of correlated subcarriers.
 * Each group = one person's Fresnel zone perturbation.
 *
 * Usage:
 *   # Live from ESP32 nodes via UDP
 *   node scripts/mincut-person-counter.js --port 5006
 *
 *   # Replay from recorded CSI data
 *   node scripts/mincut-person-counter.js --replay data/recordings/pretrain-1775182186.csi.jsonl
 *
 *   # JSON output for piping to seed bridge
 *   node scripts/mincut-person-counter.js --replay FILE --json
 *
 *   # Override feature vector dim 5 and forward to seed bridge
 *   node scripts/mincut-person-counter.js --port 5006 --forward 5007
 *
 * ADR: docs/adr/ADR-075-mincut-person-separation.md
 */

'use strict';

const dgram = require('dgram');
const fs = require('fs');
const readline = require('readline');
const { parseArgs } = require('util');
const { parseRawCsiFrame } = require('./lib/raw-csi');

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------
const { values: args } = parseArgs({
  options: {
    port:       { type: 'string', short: 'p', default: '5006' },
    replay:     { type: 'string', short: 'r' },
    json:       { type: 'boolean', default: false },
    forward:    { type: 'string', short: 'f' },
    interval:   { type: 'string', short: 'i', default: '2000' },
    window:     { type: 'string', short: 'w', default: '2000' },
    'corr-threshold': { type: 'string', default: '0.3' },
    'cut-threshold':  { type: 'string', default: '2.0' },
    'var-floor':      { type: 'string', default: '0.5' },
    'max-persons':    { type: 'string', default: '8' },
  },
  strict: true,
});

const PORT            = parseInt(args.port, 10);
const INTERVAL_MS     = parseInt(args.interval, 10);
const WINDOW_MS       = parseInt(args.window, 10);
const CORR_THRESHOLD  = parseFloat(args['corr-threshold']);
const CUT_THRESHOLD   = parseFloat(args['cut-threshold']);
const VAR_FLOOR       = parseFloat(args['var-floor']);
const MAX_PERSONS     = parseInt(args['max-persons'], 10);
const JSON_OUTPUT     = args.json;
const FORWARD_PORT    = args.forward ? parseInt(args.forward, 10) : null;

// ---------------------------------------------------------------------------
// ADR-018 raw CSI parser

// ---------------------------------------------------------------------------
// Per-node sliding window of subcarrier amplitudes
// ---------------------------------------------------------------------------
class SubcarrierWindow {
  constructor(maxAgeMs) {
    this.maxAgeMs = maxAgeMs;
    this.frames = [];       // { timestamp, amplitudes: Float64Array }
    this.nSubcarriers = 0;
  }

  push(timestamp, amplitudes) {
    this.nSubcarriers = amplitudes.length;
    this.frames.push({ timestamp, amplitudes: Float64Array.from(amplitudes) });
    this._prune(timestamp);
  }

  _prune(now) {
    const cutoff = now - this.maxAgeMs;
    while (this.frames.length > 0 && this.frames[0].timestamp < cutoff) {
      this.frames.shift();
    }
  }

  get length() { return this.frames.length; }

  /**
   * Compute pairwise Pearson correlation matrix for all subcarrier pairs.
   * Returns { matrix: Float64Array (n*n row-major), n, activeIndices }
   */
  correlationMatrix() {
    const nFrames = this.frames.length;
    const nSc = this.nSubcarriers;
    if (nFrames < 5 || nSc === 0) return null;

    // Compute mean and std for each subcarrier
    const mean = new Float64Array(nSc);
    const std = new Float64Array(nSc);

    for (let f = 0; f < nFrames; f++) {
      const amp = this.frames[f].amplitudes;
      for (let i = 0; i < nSc; i++) {
        mean[i] += amp[i];
      }
    }
    for (let i = 0; i < nSc; i++) mean[i] /= nFrames;

    for (let f = 0; f < nFrames; f++) {
      const amp = this.frames[f].amplitudes;
      for (let i = 0; i < nSc; i++) {
        const d = amp[i] - mean[i];
        std[i] += d * d;
      }
    }
    for (let i = 0; i < nSc; i++) {
      std[i] = Math.sqrt(std[i] / (nFrames - 1));
    }

    // Filter out null/static subcarriers (std below noise floor)
    const activeIndices = [];
    for (let i = 0; i < nSc; i++) {
      if (std[i] > VAR_FLOOR) {
        activeIndices.push(i);
      }
    }

    const n = activeIndices.length;
    if (n < 2) return { matrix: null, n: 0, activeIndices };

    // Compute Pearson correlation for active pairs
    const matrix = new Float64Array(n * n);

    for (let ai = 0; ai < n; ai++) {
      matrix[ai * n + ai] = 1.0; // self-correlation
      const si = activeIndices[ai];

      for (let aj = ai + 1; aj < n; aj++) {
        const sj = activeIndices[aj];

        let cov = 0;
        for (let f = 0; f < nFrames; f++) {
          const amp = this.frames[f].amplitudes;
          cov += (amp[si] - mean[si]) * (amp[sj] - mean[sj]);
        }
        cov /= (nFrames - 1);

        const denom = std[si] * std[sj];
        const r = denom > 1e-10 ? cov / denom : 0;

        matrix[ai * n + aj] = r;
        matrix[aj * n + ai] = r;
      }
    }

    return { matrix, n, activeIndices };
  }
}

// ---------------------------------------------------------------------------
// Weighted undirected graph (adjacency list)
// ---------------------------------------------------------------------------
class WeightedGraph {
  constructor(n) {
    this.n = n;
    // adj[i] = Map<j, weight>
    this.adj = new Array(n);
    for (let i = 0; i < n; i++) this.adj[i] = new Map();
    this.edgeCount = 0;
  }

  addEdge(u, v, w) {
    if (u === v) return;
    if (!this.adj[u].has(v)) this.edgeCount++;
    this.adj[u].set(v, w);
    this.adj[v].set(u, w);
  }

  /** Build graph from correlation matrix, keeping edges above threshold */
  static fromCorrelation(matrix, n, threshold) {
    const g = new WeightedGraph(n);
    for (let i = 0; i < n; i++) {
      for (let j = i + 1; j < n; j++) {
        const r = Math.abs(matrix[i * n + j]);
        if (r > threshold) {
          g.addEdge(i, j, r);
        }
      }
    }
    return g;
  }

  /**
   * Find connected components via BFS.
   * Returns array of arrays: each inner array = vertex indices in component.
   */
  connectedComponents() {
    const visited = new Uint8Array(this.n);
    const components = [];

    for (let start = 0; start < this.n; start++) {
      if (visited[start]) continue;
      const comp = [];
      const queue = [start];
      visited[start] = 1;

      while (queue.length > 0) {
        const u = queue.shift();
        comp.push(u);
        for (const [v] of this.adj[u]) {
          if (!visited[v]) {
            visited[v] = 1;
            queue.push(v);
          }
        }
      }
      components.push(comp);
    }
    return components;
  }

  /**
   * Extract a subgraph containing only the specified vertices.
   * Returns a new WeightedGraph with vertices relabeled 0..vertices.length-1,
   * plus a mapping array from new index to original index.
   */
  subgraph(vertices) {
    const newIdx = new Map();
    vertices.forEach((v, i) => newIdx.set(v, i));

    const sub = new WeightedGraph(vertices.length);
    for (const u of vertices) {
      for (const [v, w] of this.adj[u]) {
        if (newIdx.has(v) && u < v) {
          sub.addEdge(newIdx.get(u), newIdx.get(v), w);
        }
      }
    }
    return { graph: sub, mapping: vertices };
  }
}

// ---------------------------------------------------------------------------
// Stoer-Wagner minimum cut algorithm
//
// Finds the global minimum s-t cut of an undirected weighted graph.
// Complexity: O(V * E) using adjacency list with priority tracking.
//
// Reference: Stoer & Wagner (1997), "A Simple Min-Cut Algorithm", JACM.
// ---------------------------------------------------------------------------

/**
 * Run one "minimum cut phase" of Stoer-Wagner.
 *
 * Starting from an arbitrary vertex, greedily add the most tightly connected
 * vertex to the growing set A until all vertices are absorbed.
 *
 * @param {number} n - Number of active vertices
 * @param {Map<number, Map<number, number>>} adj - Adjacency: adj[u].get(v) = weight
 * @param {number[]} activeVertices - List of active vertex IDs
 * @returns {{ s: number, t: number, cutOfPhase: number }}
 */
function minimumCutPhase(n, adj, activeVertices) {
  // key[v] = sum of edge weights from v to vertices already in A
  const key = new Float64Array(n);
  const inA = new Uint8Array(n);
  const active = new Uint8Array(n);
  for (const v of activeVertices) active[v] = 1;

  let s = -1, t = -1;

  for (let iter = 0; iter < activeVertices.length; iter++) {
    // Find vertex not in A with maximum key value
    let best = -1, bestKey = -Infinity;
    for (const v of activeVertices) {
      if (!inA[v] && key[v] > bestKey) {
        bestKey = key[v];
        best = v;
      }
    }

    // On first iteration when all keys are 0, just pick the first active vertex
    if (best === -1) {
      for (const v of activeVertices) {
        if (!inA[v]) { best = v; break; }
      }
    }

    s = t;
    t = best;
    inA[best] = 1;

    // Update keys: for each neighbor of best, increase key
    if (adj[best]) {
      for (const [neighbor, weight] of adj[best]) {
        if (active[neighbor] && !inA[neighbor]) {
          key[neighbor] += weight;
        }
      }
    }
  }

  // Cut of the phase = sum of edges from t to all other active vertices
  let cutOfPhase = 0;
  if (adj[t]) {
    for (const [neighbor, weight] of adj[t]) {
      if (active[neighbor] && neighbor !== t) {
        cutOfPhase += weight;
      }
    }
  }

  return { s, t, cutOfPhase };
}

/**
 * Stoer-Wagner global minimum cut.
 *
 * @param {WeightedGraph} graph
 * @returns {{ minCutValue: number, partition: [number[], number[]] }}
 *   partition[0] = vertices on one side, partition[1] = vertices on the other side
 */
function stoerWagner(graph) {
  const n = graph.n;
  if (n <= 1) return { minCutValue: Infinity, partition: [Array.from({length: n}, (_, i) => i), []] };

  // Build mutable adjacency (Map-based for efficient merge)
  const adj = new Array(n);
  for (let i = 0; i < n; i++) adj[i] = new Map(graph.adj[i]);

  // Track which original vertices each super-vertex contains
  const groups = new Array(n);
  for (let i = 0; i < n; i++) groups[i] = [i];

  let activeVertices = Array.from({length: n}, (_, i) => i);
  let bestCut = Infinity;
  let bestPartitionSide = null; // group of vertices on the "t" side of the best cut

  while (activeVertices.length > 1) {
    const { s, t, cutOfPhase } = minimumCutPhase(n, adj, activeVertices);

    if (s === -1 || t === -1) break;

    if (cutOfPhase < bestCut) {
      bestCut = cutOfPhase;
      bestPartitionSide = [...groups[t]];
    }

    // Merge t into s: move all edges from t to s
    if (adj[t]) {
      for (const [neighbor, weight] of adj[t]) {
        if (neighbor === s) continue;
        const existing = adj[s].get(neighbor) || 0;
        adj[s].set(neighbor, existing + weight);
        // Update neighbor's adjacency
        adj[neighbor].delete(t);
        adj[neighbor].set(s, existing + weight);
      }
    }
    adj[s].delete(t);

    // Merge group membership
    groups[s] = groups[s].concat(groups[t]);
    groups[t] = [];

    // Remove t from active vertices
    activeVertices = activeVertices.filter(v => v !== t);
  }

  // Build full partition
  if (!bestPartitionSide || bestPartitionSide.length === 0) {
    return { minCutValue: Infinity, partition: [Array.from({length: n}, (_, i) => i), []] };
  }

  const sideSet = new Set(bestPartitionSide);
  const sideA = [], sideB = [];
  for (let i = 0; i < n; i++) {
    if (sideSet.has(i)) sideA.push(i);
    else sideB.push(i);
  }

  return { minCutValue: bestCut, partition: [sideA, sideB] };
}

// ---------------------------------------------------------------------------
// Recursive min-cut person separator
//
// Recursively applies Stoer-Wagner to split the correlation graph into
// independent clusters. Each cluster = one person's Fresnel zone group.
// ---------------------------------------------------------------------------

/**
 * @param {WeightedGraph} graph
 * @param {number} cutThreshold - min-cut below this = real person boundary
 * @param {number} maxPersons - stop splitting after this many partitions
 * @returns {number[][]} - array of vertex groups (each = one person's subcarriers)
 */
function separatePersons(graph, cutThreshold, maxPersons) {
  // Start with connected components (disconnected groups are trivially separate)
  const components = graph.connectedComponents();
  const personGroups = [];

  for (const comp of components) {
    if (comp.length < 2) {
      // Single vertex — not enough for a person
      continue;
    }
    _splitComponent(graph, comp, cutThreshold, maxPersons, personGroups);
  }

  return personGroups;
}

function _splitComponent(graph, vertices, cutThreshold, maxPersons, result) {
  if (vertices.length < 2 || result.length >= maxPersons) {
    if (vertices.length >= 2) result.push(vertices);
    return;
  }

  // Extract subgraph
  const { graph: sub, mapping } = graph.subgraph(vertices);

  // Run Stoer-Wagner on the subgraph
  const { minCutValue, partition } = stoerWagner(sub);

  // If the min-cut is above threshold, this is one coherent group (one person)
  if (minCutValue >= cutThreshold || partition[0].length === 0 || partition[1].length === 0) {
    result.push(vertices);
    return;
  }

  // Map partition indices back to original vertex IDs
  const groupA = partition[0].map(i => mapping[i]);
  const groupB = partition[1].map(i => mapping[i]);

  // Recurse on each side
  _splitComponent(graph, groupA, cutThreshold, maxPersons, result);
  _splitComponent(graph, groupB, cutThreshold, maxPersons, result);
}

// ---------------------------------------------------------------------------
// CSI frame parsing (from JSONL recording or UDP)
// ---------------------------------------------------------------------------

/** Parse IQ hex string into amplitude array */
function parseIqHex(iqHex, nSubcarriers) {
  const bytes = Buffer.from(iqHex, 'hex');
  const amplitudes = new Float64Array(nSubcarriers);

  // IQ data: pairs of signed int8 (I, Q) for each subcarrier
  // First 2 bytes are header/padding, then I/Q pairs
  for (let sc = 0; sc < nSubcarriers; sc++) {
    const offset = 2 + sc * 2; // skip 2-byte header
    if (offset + 1 >= bytes.length) break;

    // Read as signed int8
    let I = bytes[offset];
    let Q = bytes[offset + 1];
    if (I > 127) I -= 256;
    if (Q > 127) Q -= 256;

    amplitudes[sc] = Math.sqrt(I * I + Q * Q);
  }

  return amplitudes;
}

/** Parse binary UDP CSI packet (ADR-018 format) */
function parseUdpPacket(buf) {
  const frame = parseRawCsiFrame(buf, { includePhase: false });
  if (!frame) return null;
  return {
    nodeId: frame.nodeId,
    nSubcarriers: frame.nSubcarriers,
    freqMhz: frame.freqMhz,
    rssi: frame.rssi,
    amplitudes: frame.amplitudes,
    timestamp: Date.now() / 1000,
  };
}

// ---------------------------------------------------------------------------
// Analysis engine
// ---------------------------------------------------------------------------
class PersonCounter {
  constructor(opts) {
    this.windowMs = opts.windowMs;
    this.corrThreshold = opts.corrThreshold;
    this.cutThreshold = opts.cutThreshold;
    this.maxPersons = opts.maxPersons;

    // Per-node sliding windows
    this.windows = new Map();  // nodeId -> SubcarrierWindow

    // Latest result
    this.lastResult = null;
    this.analysisCount = 0;
  }

  ingestFrame(nodeId, timestamp, amplitudes) {
    if (!this.windows.has(nodeId)) {
      this.windows.set(nodeId, new SubcarrierWindow(this.windowMs));
    }
    this.windows.get(nodeId).push(timestamp * 1000, amplitudes);
  }

  /**
   * Run the min-cut analysis on accumulated data.
   * Merges subcarrier data from all nodes into a single correlation graph.
   *
   * @returns {{ personCount, groups, graphStats, perNode }}
   */
  analyze() {
    this.analysisCount++;
    const perNode = {};
    const allGroups = [];
    let totalPersons = 0;

    for (const [nodeId, window] of this.windows) {
      const corr = window.correlationMatrix();
      if (!corr || !corr.matrix || corr.n < 2) {
        perNode[nodeId] = { personCount: 0, activeSubcarriers: corr ? corr.n : 0, groups: [], edges: 0 };
        continue;
      }

      // Build correlation graph
      const graph = WeightedGraph.fromCorrelation(corr.matrix, corr.n, this.corrThreshold);

      // Separate persons via recursive min-cut
      const groups = separatePersons(graph, this.cutThreshold, this.maxPersons);

      // Map group indices back to original subcarrier indices
      const mappedGroups = groups.map(g => g.map(i => corr.activeIndices[i]));

      const nodeResult = {
        personCount: groups.length,
        activeSubcarriers: corr.n,
        totalSubcarriers: window.nSubcarriers,
        groups: mappedGroups,
        edges: graph.edgeCount,
        frames: window.length,
      };

      perNode[nodeId] = nodeResult;
      totalPersons = Math.max(totalPersons, groups.length);
      allGroups.push(...mappedGroups);
    }

    this.lastResult = {
      personCount: totalPersons,
      groups: allGroups,
      perNode,
      timestamp: Date.now() / 1000,
      analysisIndex: this.analysisCount,
    };

    return this.lastResult;
  }
}

// ---------------------------------------------------------------------------
// ASCII output
// ---------------------------------------------------------------------------
function formatResult(result) {
  const lines = [];
  const ts = new Date(result.timestamp * 1000).toISOString().slice(11, 19);

  lines.push(`\x1b[1m[${ts}] Persons: ${result.personCount}\x1b[0m  (analysis #${result.analysisIndex})`);

  for (const [nodeId, nodeResult] of Object.entries(result.perNode)) {
    const { personCount, activeSubcarriers, totalSubcarriers, groups, edges, frames } = nodeResult;
    lines.push(`  Node ${nodeId}: ${personCount} person(s) | ${activeSubcarriers}/${totalSubcarriers} active subcarriers | ${edges} edges | ${frames} frames`);

    for (let i = 0; i < groups.length; i++) {
      const g = groups[i];
      const scList = g.length <= 12 ? g.join(',') : g.slice(0, 10).join(',') + `...+${g.length - 10}`;
      lines.push(`    Person ${i + 1}: subcarriers [${scList}] (${g.length} sc)`);
    }
  }

  return lines.join('\n');
}

function formatJson(result) {
  return JSON.stringify(result);
}

// ---------------------------------------------------------------------------
// UDP forwarding (override person count in feature vector)
// ---------------------------------------------------------------------------
let forwardSocket = null;
function forwardWithCorrectedCount(buf, personCount) {
  if (!FORWARD_PORT || !forwardSocket) return;
  // If it's a vitals packet (magic 0xC5110002), override byte 13 (nPersons)
  const magic = buf.readUInt32LE(0);
  if (magic === 0xC5110002 && buf.length >= 14) {
    const copy = Buffer.from(buf);
    copy.writeUInt8(Math.min(personCount, 255), 13);
    forwardSocket.send(copy, FORWARD_PORT, '127.0.0.1');
  } else {
    // Forward as-is
    forwardSocket.send(buf, FORWARD_PORT, '127.0.0.1');
  }
}

// ---------------------------------------------------------------------------
// Main: live UDP mode
// ---------------------------------------------------------------------------
function startLive() {
  const counter = new PersonCounter({
    windowMs: WINDOW_MS,
    corrThreshold: CORR_THRESHOLD,
    cutThreshold: CUT_THRESHOLD,
    maxPersons: MAX_PERSONS,
  });

  const server = dgram.createSocket('udp4');

  if (FORWARD_PORT) {
    forwardSocket = dgram.createSocket('udp4');
  }

  server.on('message', (buf, rinfo) => {
    const frame = parseUdpPacket(buf);
    if (frame) {
      counter.ingestFrame(frame.nodeId, frame.timestamp, frame.amplitudes);
    }

    // Forward all packets with corrected person count
    if (counter.lastResult) {
      forwardWithCorrectedCount(buf, counter.lastResult.personCount);
    }
  });

  // Periodic analysis
  setInterval(() => {
    const result = counter.analyze();
    if (JSON_OUTPUT) {
      console.log(formatJson(result));
    } else {
      process.stdout.write('\x1b[2J\x1b[H'); // clear screen
      console.log('ADR-075 Min-Cut Person Counter (live UDP)');
      console.log('─'.repeat(60));
      console.log(formatResult(result));
      console.log('─'.repeat(60));
      console.log(`Thresholds: corr=${CORR_THRESHOLD} cut=${CUT_THRESHOLD} var-floor=${VAR_FLOOR}`);
    }
  }, INTERVAL_MS);

  server.bind(PORT, () => {
    if (!JSON_OUTPUT) {
      console.log(`Listening on UDP port ${PORT} (analysis every ${INTERVAL_MS}ms, window ${WINDOW_MS}ms)`);
      if (FORWARD_PORT) console.log(`Forwarding corrected packets to UDP port ${FORWARD_PORT}`);
    }
  });
}

// ---------------------------------------------------------------------------
// Main: replay mode (from .csi.jsonl recording)
// ---------------------------------------------------------------------------
async function startReplay(filePath) {
  const counter = new PersonCounter({
    windowMs: WINDOW_MS,
    corrThreshold: CORR_THRESHOLD,
    cutThreshold: CUT_THRESHOLD,
    maxPersons: MAX_PERSONS,
  });

  if (!fs.existsSync(filePath)) {
    console.error(`File not found: ${filePath}`);
    process.exit(1);
  }

  const rl = readline.createInterface({
    input: fs.createReadStream(filePath),
    crlfDelay: Infinity,
  });

  let frameCount = 0;
  let lastAnalysisTs = 0;
  let analysisResults = [];

  for await (const line of rl) {
    if (!line.trim()) continue;

    let record;
    try {
      record = JSON.parse(line);
    } catch {
      continue;
    }

    if (record.type !== 'raw_csi' || !record.iq_hex) continue;

    const amplitudes = parseIqHex(record.iq_hex, record.subcarriers || 64);
    counter.ingestFrame(record.node_id, record.timestamp, amplitudes);
    frameCount++;

    // Run analysis every INTERVAL_MS worth of frames
    const tsMs = record.timestamp * 1000;
    if (lastAnalysisTs === 0) lastAnalysisTs = tsMs;

    if (tsMs - lastAnalysisTs >= INTERVAL_MS) {
      const result = counter.analyze();
      analysisResults.push(result);

      if (JSON_OUTPUT) {
        console.log(formatJson(result));
      } else {
        console.log(formatResult(result));
        console.log();
      }

      lastAnalysisTs = tsMs;
    }
  }

  // Final analysis
  const result = counter.analyze();
  analysisResults.push(result);

  if (!JSON_OUTPUT) {
    console.log('─'.repeat(60));
    console.log('FINAL ANALYSIS');
    console.log('─'.repeat(60));
    console.log(formatResult(result));
    console.log();
    console.log(`Processed ${frameCount} frames, ${analysisResults.length} analysis windows`);

    // Summary statistics
    const counts = analysisResults.map(r => r.personCount);
    const avg = counts.reduce((a, b) => a + b, 0) / counts.length;
    const max = Math.max(...counts);
    const min = Math.min(...counts);
    console.log(`Person count: min=${min} max=${max} avg=${avg.toFixed(1)}`);
    console.log(`Thresholds: corr=${CORR_THRESHOLD} cut=${CUT_THRESHOLD} var-floor=${VAR_FLOOR}`);
  } else {
    console.log(formatJson(result));
  }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------
if (args.replay) {
  startReplay(args.replay);
} else {
  startLive();
}
