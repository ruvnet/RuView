#!/usr/bin/env node
/**
 * ADR-075: CSI Subcarrier Correlation Graph Visualizer
 *
 * ASCII visualization of the subcarrier correlation graph used by the
 * min-cut person counter. Shows per-person subcarrier clusters, graph
 * connectivity, and correlation heatmap in real-time.
 *
 * Usage:
 *   # Live from ESP32 nodes via UDP
 *   node scripts/csi-graph-visualizer.js --port 5006
 *
 *   # Replay from recorded CSI data
 *   node scripts/csi-graph-visualizer.js --replay data/recordings/pretrain-1775182186.csi.jsonl
 *
 *   # Show correlation heatmap only
 *   node scripts/csi-graph-visualizer.js --replay FILE --mode heatmap
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
    interval:   { type: 'string', short: 'i', default: '2000' },
    window:     { type: 'string', short: 'w', default: '2000' },
    mode:       { type: 'string', short: 'm', default: 'all' },
    node:       { type: 'string', short: 'n', default: '0' },
    'corr-threshold': { type: 'string', default: '0.3' },
    'cut-threshold':  { type: 'string', default: '2.0' },
    'var-floor':      { type: 'string', default: '0.5' },
    width:      { type: 'string', default: '80' },
  },
  strict: true,
});

const PORT           = parseInt(args.port, 10);
const INTERVAL_MS    = parseInt(args.interval, 10);
const WINDOW_MS      = parseInt(args.window, 10);
const CORR_THRESHOLD = parseFloat(args['corr-threshold']);
const CUT_THRESHOLD  = parseFloat(args['cut-threshold']);
const VAR_FLOOR      = parseFloat(args['var-floor']);
const MODE           = args.mode; // 'all', 'heatmap', 'clusters', 'spectrum'
const TARGET_NODE    = parseInt(args.node, 10);
const WIDTH          = parseInt(args.width, 10);

// Color palette for person clusters (ANSI 256)
const PERSON_COLORS = [
  '\x1b[31m', // red
  '\x1b[32m', // green
  '\x1b[34m', // blue
  '\x1b[33m', // yellow
  '\x1b[35m', // magenta
  '\x1b[36m', // cyan
  '\x1b[91m', // bright red
  '\x1b[92m', // bright green
];
const RESET = '\x1b[0m';
const DIM = '\x1b[2m';
const BOLD = '\x1b[1m';

// Heatmap characters (11 levels of intensity)
const HEAT = [' ', '\u2591', '\u2591', '\u2592', '\u2592', '\u2593', '\u2593', '\u2588', '\u2588', '\u2588', '\u2588'];

// Bar chart characters
const BARS = ['\u2581', '\u2582', '\u2583', '\u2584', '\u2585', '\u2586', '\u2587', '\u2588'];

// ---------------------------------------------------------------------------
// Sliding window (same as mincut-person-counter.js)
// ---------------------------------------------------------------------------
class SubcarrierWindow {
  constructor(maxAgeMs) {
    this.maxAgeMs = maxAgeMs;
    this.frames = [];
    this.nSubcarriers = 0;
  }

  push(timestamp, amplitudes) {
    this.nSubcarriers = amplitudes.length;
    this.frames.push({ timestamp, amplitudes: Float64Array.from(amplitudes) });
    const cutoff = timestamp - this.maxAgeMs;
    while (this.frames.length > 0 && this.frames[0].timestamp < cutoff) {
      this.frames.shift();
    }
  }

  get length() { return this.frames.length; }

  correlationMatrix() {
    const nFrames = this.frames.length;
    const nSc = this.nSubcarriers;
    if (nFrames < 5 || nSc === 0) return null;

    const mean = new Float64Array(nSc);
    const std = new Float64Array(nSc);

    for (let f = 0; f < nFrames; f++) {
      const amp = this.frames[f].amplitudes;
      for (let i = 0; i < nSc; i++) mean[i] += amp[i];
    }
    for (let i = 0; i < nSc; i++) mean[i] /= nFrames;

    for (let f = 0; f < nFrames; f++) {
      const amp = this.frames[f].amplitudes;
      for (let i = 0; i < nSc; i++) {
        const d = amp[i] - mean[i];
        std[i] += d * d;
      }
    }
    for (let i = 0; i < nSc; i++) std[i] = Math.sqrt(std[i] / (nFrames - 1));

    const activeIndices = [];
    for (let i = 0; i < nSc; i++) {
      if (std[i] > VAR_FLOOR) activeIndices.push(i);
    }

    const n = activeIndices.length;
    if (n < 2) return { matrix: null, n: 0, activeIndices, mean, std };

    const matrix = new Float64Array(n * n);
    for (let ai = 0; ai < n; ai++) {
      matrix[ai * n + ai] = 1.0;
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

    return { matrix, n, activeIndices, mean, std };
  }

  /** Get latest amplitudes */
  latestAmplitudes() {
    if (this.frames.length === 0) return null;
    return this.frames[this.frames.length - 1].amplitudes;
  }
}

// ---------------------------------------------------------------------------
// Graph + Stoer-Wagner (minimal copy from mincut-person-counter.js)
// ---------------------------------------------------------------------------
class WeightedGraph {
  constructor(n) {
    this.n = n;
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
  static fromCorrelation(matrix, n, threshold) {
    const g = new WeightedGraph(n);
    for (let i = 0; i < n; i++) {
      for (let j = i + 1; j < n; j++) {
        const r = Math.abs(matrix[i * n + j]);
        if (r > threshold) g.addEdge(i, j, r);
      }
    }
    return g;
  }
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
          if (!visited[v]) { visited[v] = 1; queue.push(v); }
        }
      }
      components.push(comp);
    }
    return components;
  }
  subgraph(vertices) {
    const newIdx = new Map();
    vertices.forEach((v, i) => newIdx.set(v, i));
    const sub = new WeightedGraph(vertices.length);
    for (const u of vertices) {
      for (const [v, w] of this.adj[u]) {
        if (newIdx.has(v) && u < v) sub.addEdge(newIdx.get(u), newIdx.get(v), w);
      }
    }
    return { graph: sub, mapping: vertices };
  }
}

function stoerWagner(graph) {
  const n = graph.n;
  if (n <= 1) return { minCutValue: Infinity, partition: [Array.from({length: n}, (_, i) => i), []] };

  const adj = new Array(n);
  for (let i = 0; i < n; i++) adj[i] = new Map(graph.adj[i]);
  const groups = new Array(n);
  for (let i = 0; i < n; i++) groups[i] = [i];

  let activeVertices = Array.from({length: n}, (_, i) => i);
  let bestCut = Infinity;
  let bestPartitionSide = null;

  while (activeVertices.length > 1) {
    const key = new Float64Array(n);
    const inA = new Uint8Array(n);
    let s = -1, t = -1;

    for (let iter = 0; iter < activeVertices.length; iter++) {
      let best = -1, bestKey = -Infinity;
      for (const v of activeVertices) {
        if (!inA[v] && key[v] > bestKey) { bestKey = key[v]; best = v; }
      }
      if (best === -1) {
        for (const v of activeVertices) { if (!inA[v]) { best = v; break; } }
      }
      s = t; t = best; inA[best] = 1;
      if (adj[best]) {
        for (const [nb, w] of adj[best]) {
          if (activeVertices.includes(nb) && !inA[nb]) key[nb] += w;
        }
      }
    }

    let cutOfPhase = 0;
    if (adj[t]) {
      for (const [nb, w] of adj[t]) {
        if (activeVertices.includes(nb) && nb !== t) cutOfPhase += w;
      }
    }

    if (s === -1 || t === -1) break;
    if (cutOfPhase < bestCut) { bestCut = cutOfPhase; bestPartitionSide = [...groups[t]]; }

    if (adj[t]) {
      for (const [nb, w] of adj[t]) {
        if (nb === s) continue;
        const ex = adj[s].get(nb) || 0;
        adj[s].set(nb, ex + w);
        adj[nb].delete(t);
        adj[nb].set(s, ex + w);
      }
    }
    adj[s].delete(t);
    groups[s] = groups[s].concat(groups[t]);
    groups[t] = [];
    activeVertices = activeVertices.filter(v => v !== t);
  }

  if (!bestPartitionSide || bestPartitionSide.length === 0) {
    return { minCutValue: Infinity, partition: [Array.from({length: n}, (_, i) => i), []] };
  }
  const sideSet = new Set(bestPartitionSide);
  const sideA = [], sideB = [];
  for (let i = 0; i < n; i++) { (sideSet.has(i) ? sideA : sideB).push(i); }
  return { minCutValue: bestCut, partition: [sideA, sideB] };
}

function separatePersons(graph, cutThreshold, maxPersons) {
  const components = graph.connectedComponents();
  const personGroups = [];
  for (const comp of components) {
    if (comp.length < 2) continue;
    _split(graph, comp, cutThreshold, maxPersons, personGroups);
  }
  return personGroups;
}

function _split(graph, vertices, cutThreshold, maxPersons, result) {
  if (vertices.length < 2 || result.length >= maxPersons) {
    if (vertices.length >= 2) result.push(vertices);
    return;
  }
  const { graph: sub, mapping } = graph.subgraph(vertices);
  const { minCutValue, partition } = stoerWagner(sub);
  if (minCutValue >= cutThreshold || partition[0].length === 0 || partition[1].length === 0) {
    result.push(vertices);
    return;
  }
  _split(graph, partition[0].map(i => mapping[i]), cutThreshold, maxPersons, result);
  _split(graph, partition[1].map(i => mapping[i]), cutThreshold, maxPersons, result);
}

// ---------------------------------------------------------------------------
// Visualization renderers
// ---------------------------------------------------------------------------

/**
 * Render correlation heatmap (downsampled to fit terminal width).
 * Rows and columns = active subcarrier indices.
 */
function renderHeatmap(corr, width) {
  if (!corr || !corr.matrix) return ['  (insufficient data for heatmap)'];
  const { matrix, n, activeIndices } = corr;

  const lines = [];
  lines.push(`${BOLD}Correlation Heatmap${RESET} (${n} active subcarriers, threshold=${CORR_THRESHOLD})`);

  // Downsample if needed
  const maxCols = Math.min(n, width - 8);
  const step = Math.max(1, Math.ceil(n / maxCols));
  const displayN = Math.ceil(n / step);

  // Header row: subcarrier indices
  let header = '      ';
  for (let j = 0; j < displayN; j++) {
    const sc = activeIndices[j * step];
    header += (sc < 10 ? `${sc} ` : `${sc}`).slice(0, 2);
  }
  lines.push(DIM + header + RESET);

  for (let i = 0; i < displayN; i++) {
    const sc = activeIndices[i * step];
    let row = `  ${String(sc).padStart(3)} `;

    for (let j = 0; j < displayN; j++) {
      const ii = i * step, jj = j * step;
      const val = Math.abs(matrix[ii * n + jj]);
      const level = Math.min(10, Math.floor(val * 10));

      if (val > CORR_THRESHOLD) {
        row += `\x1b[33m${HEAT[level]}${RESET} `;
      } else {
        row += `${DIM}${HEAT[level]}${RESET} `;
      }
    }
    lines.push(row);
  }

  return lines;
}

/**
 * Render subcarrier spectrum bar with person cluster coloring.
 */
function renderSpectrum(window, personGroups, activeIndices) {
  const amp = window.latestAmplitudes();
  if (!amp) return ['  (no data)'];

  const lines = [];
  const nSc = window.nSubcarriers;

  // Build subcarrier-to-person mapping
  const scToPerson = new Int8Array(nSc).fill(-1);
  if (personGroups && activeIndices) {
    for (let p = 0; p < personGroups.length; p++) {
      for (const graphIdx of personGroups[p]) {
        if (graphIdx < activeIndices.length) {
          scToPerson[activeIndices[graphIdx]] = p;
        }
      }
    }
  }

  // Find max amplitude for normalization
  let maxAmp = 0;
  for (let i = 0; i < nSc; i++) {
    if (amp[i] > maxAmp) maxAmp = amp[i];
  }
  if (maxAmp === 0) maxAmp = 1;

  lines.push(`${BOLD}Spectrum${RESET} (${nSc} subcarriers, colored by person cluster)`);

  // Render bar
  let bar = '  ';
  for (let i = 0; i < nSc; i++) {
    const level = Math.floor((amp[i] / maxAmp) * 7.99);
    const ch = BARS[Math.max(0, Math.min(7, level))];
    const personIdx = scToPerson[i];
    if (personIdx >= 0 && personIdx < PERSON_COLORS.length) {
      bar += PERSON_COLORS[personIdx] + ch + RESET;
    } else {
      bar += DIM + ch + RESET;
    }
  }
  lines.push(bar);

  // Legend
  let legend = '  ';
  for (let i = 0; i < nSc; i++) {
    const p = scToPerson[i];
    if (p >= 0 && p < PERSON_COLORS.length) {
      legend += PERSON_COLORS[p] + (p + 1) + RESET;
    } else {
      legend += DIM + '.' + RESET;
    }
  }
  lines.push(legend);

  return lines;
}

/**
 * Render cluster summary with per-person statistics.
 */
function renderClusters(personGroups, activeIndices, corr) {
  if (!personGroups || personGroups.length === 0) {
    return ['  No person clusters detected'];
  }

  const lines = [];
  lines.push(`${BOLD}Person Clusters${RESET} (${personGroups.length} detected)`);

  for (let p = 0; p < personGroups.length; p++) {
    const group = personGroups[p];
    const color = p < PERSON_COLORS.length ? PERSON_COLORS[p] : '';

    // Map back to subcarrier indices
    const scIds = group.map(i => activeIndices[i]);
    const scStr = scIds.length <= 16
      ? scIds.join(', ')
      : scIds.slice(0, 14).join(', ') + `, ...+${scIds.length - 14}`;

    // Compute intra-cluster average correlation
    let avgCorr = 0, count = 0;
    if (corr && corr.matrix) {
      for (let i = 0; i < group.length; i++) {
        for (let j = i + 1; j < group.length; j++) {
          avgCorr += Math.abs(corr.matrix[group[i] * corr.n + group[j]]);
          count++;
        }
      }
      if (count > 0) avgCorr /= count;
    }

    lines.push(`  ${color}Person ${p + 1}${RESET}: ${group.length} subcarriers, avg intra-corr=${avgCorr.toFixed(3)}`);
    lines.push(`    ${DIM}SC: [${scStr}]${RESET}`);
  }

  return lines;
}

/**
 * Render graph connectivity summary.
 */
function renderGraphStats(graph, corr) {
  if (!graph) return ['  (no graph)'];

  const lines = [];
  const components = graph.connectedComponents();
  const density = graph.n > 1 ? (2 * graph.edgeCount) / (graph.n * (graph.n - 1)) : 0;

  lines.push(`${BOLD}Graph${RESET}: ${graph.n} nodes, ${graph.edgeCount} edges, density=${density.toFixed(3)}, components=${components.length}`);

  // Degree distribution summary
  const degrees = new Array(graph.n);
  let minDeg = Infinity, maxDeg = 0, sumDeg = 0;
  for (let i = 0; i < graph.n; i++) {
    degrees[i] = graph.adj[i].size;
    if (degrees[i] < minDeg) minDeg = degrees[i];
    if (degrees[i] > maxDeg) maxDeg = degrees[i];
    sumDeg += degrees[i];
  }
  const avgDeg = graph.n > 0 ? sumDeg / graph.n : 0;
  lines.push(`  Degree: min=${minDeg} max=${maxDeg} avg=${avgDeg.toFixed(1)}`);

  return lines;
}

// ---------------------------------------------------------------------------
// Full render
// ---------------------------------------------------------------------------
function render(window, nodeId) {
  const corr = window.correlationMatrix();
  const lines = [];

  const ts = new Date().toISOString().slice(11, 19);
  lines.push(`${BOLD}ADR-075 CSI Graph Visualizer${RESET} [${ts}] Node ${nodeId} | ${window.length} frames`);
  lines.push('═'.repeat(WIDTH));

  let graph = null;
  let personGroups = null;
  let activeIndices = corr ? corr.activeIndices : [];

  if (corr && corr.matrix && corr.n >= 2) {
    graph = WeightedGraph.fromCorrelation(corr.matrix, corr.n, CORR_THRESHOLD);
    personGroups = separatePersons(graph, CUT_THRESHOLD, 8);
  }

  const personCount = personGroups ? personGroups.length : 0;
  lines.push(`${BOLD}Persons: ${personCount}${RESET}  |  Active subcarriers: ${activeIndices.length}/${window.nSubcarriers}`);
  lines.push('');

  if (MODE === 'all' || MODE === 'spectrum') {
    lines.push(...renderSpectrum(window, personGroups, activeIndices));
    lines.push('');
  }

  if (MODE === 'all' || MODE === 'clusters') {
    lines.push(...renderClusters(personGroups, activeIndices, corr));
    lines.push('');
  }

  if (MODE === 'all' || MODE === 'heatmap') {
    lines.push(...renderHeatmap(corr, WIDTH));
    lines.push('');
  }

  if (graph) {
    lines.push(...renderGraphStats(graph, corr));
  }

  lines.push('═'.repeat(WIDTH));
  lines.push(`${DIM}Thresholds: corr=${CORR_THRESHOLD} cut=${CUT_THRESHOLD} var-floor=${VAR_FLOOR}${RESET}`);

  return lines.join('\n');
}

// ---------------------------------------------------------------------------
// Packet parsing
// ---------------------------------------------------------------------------
function parseIqHex(iqHex, nSubcarriers) {
  const bytes = Buffer.from(iqHex, 'hex');
  const amplitudes = new Float64Array(nSubcarriers);
  for (let sc = 0; sc < nSubcarriers; sc++) {
    const offset = 2 + sc * 2;
    if (offset + 1 >= bytes.length) break;
    let I = bytes[offset]; let Q = bytes[offset + 1];
    if (I > 127) I -= 256;
    if (Q > 127) Q -= 256;
    amplitudes[sc] = Math.sqrt(I * I + Q * Q);
  }
  return amplitudes;
}

function parseUdpPacket(buf) {
  const frame = parseRawCsiFrame(buf, { includePhase: false });
  if (!frame) return null;
  return {
    nodeId: frame.nodeId,
    nSubcarriers: frame.nSubcarriers,
    amplitudes: frame.amplitudes,
    timestamp: Date.now(),
  };
}

// ---------------------------------------------------------------------------
// Main: live mode
// ---------------------------------------------------------------------------
function startLive() {
  const windows = new Map();
  const server = dgram.createSocket('udp4');

  server.on('message', (buf) => {
    const frame = parseUdpPacket(buf);
    if (!frame) return;
    if (!windows.has(frame.nodeId)) {
      windows.set(frame.nodeId, new SubcarrierWindow(WINDOW_MS));
    }
    windows.get(frame.nodeId).push(frame.timestamp, frame.amplitudes);
  });

  setInterval(() => {
    process.stdout.write('\x1b[2J\x1b[H');
    for (const [nodeId, window] of windows) {
      if (TARGET_NODE !== 0 && nodeId !== TARGET_NODE) continue;
      console.log(render(window, nodeId));
      console.log();
    }
    if (windows.size === 0) {
      console.log('Waiting for CSI frames on UDP port ' + PORT + '...');
    }
  }, INTERVAL_MS);

  server.bind(PORT, () => {
    console.log(`CSI Graph Visualizer listening on UDP port ${PORT}`);
  });
}

// ---------------------------------------------------------------------------
// Main: replay mode
// ---------------------------------------------------------------------------
async function startReplay(filePath) {
  if (!fs.existsSync(filePath)) {
    console.error(`File not found: ${filePath}`);
    process.exit(1);
  }

  const windows = new Map();
  const rl = readline.createInterface({
    input: fs.createReadStream(filePath),
    crlfDelay: Infinity,
  });

  let lastRenderTs = 0;
  let frameCount = 0;

  for await (const line of rl) {
    if (!line.trim()) continue;
    let record;
    try { record = JSON.parse(line); } catch { continue; }
    if (record.type !== 'raw_csi' || !record.iq_hex) continue;

    const nSc = record.subcarriers || 64;
    const amplitudes = parseIqHex(record.iq_hex, nSc);
    const nodeId = record.node_id;
    const tsMs = record.timestamp * 1000;

    if (!windows.has(nodeId)) {
      windows.set(nodeId, new SubcarrierWindow(WINDOW_MS));
    }
    windows.get(nodeId).push(tsMs, amplitudes);
    frameCount++;

    if (lastRenderTs === 0) lastRenderTs = tsMs;
    if (tsMs - lastRenderTs >= INTERVAL_MS) {
      process.stdout.write('\x1b[2J\x1b[H');
      for (const [nid, window] of windows) {
        if (TARGET_NODE !== 0 && nid !== TARGET_NODE) continue;
        console.log(render(window, nid));
        console.log();
      }
      lastRenderTs = tsMs;

      // Small delay for visual effect during replay
      await new Promise(r => setTimeout(r, 100));
    }
  }

  // Final render
  console.log();
  console.log('═'.repeat(WIDTH));
  console.log(`${BOLD}Replay complete${RESET}: ${frameCount} frames`);
  for (const [nodeId, window] of windows) {
    if (TARGET_NODE !== 0 && nodeId !== TARGET_NODE) continue;
    console.log();
    console.log(render(window, nodeId));
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
