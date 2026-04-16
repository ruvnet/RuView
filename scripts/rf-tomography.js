#!/usr/bin/env node
/**
 * RF Tomographic Imaging — Multi-Frequency Mesh Application
 *
 * Back-projects CSI attenuation along each TX->RX path across 6 WiFi channels
 * to build a 2D heatmap of RF absorption in the room. Areas with high absorption
 * correspond to people, furniture, or walls.
 *
 * Requires multi-frequency mesh scanning (ADR-073): 2 ESP32 nodes hopping
 * across channels 1, 3, 5, 6, 9, 11.
 *
 * Usage:
 *   node scripts/rf-tomography.js
 *   node scripts/rf-tomography.js --port 5006 --duration 60
 *   node scripts/rf-tomography.js --replay data/recordings/overnight-1775217646.csi.jsonl
 *   node scripts/rf-tomography.js --grid 15 --node-distance 4.0
 *
 * ADR: docs/adr/ADR-078-multifreq-mesh-applications.md
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
    port:          { type: 'string', short: 'p', default: '5006' },
    duration:      { type: 'string', short: 'd' },
    replay:        { type: 'string', short: 'r' },
    interval:      { type: 'string', short: 'i', default: '2000' },
    grid:          { type: 'string', short: 'g', default: '10' },
    json:          { type: 'boolean', default: false },
    'node-distance': { type: 'string', default: '3.0' },
    'room-width':  { type: 'string', default: '5.0' },
    'room-height': { type: 'string', default: '4.0' },
  },
  strict: true,
});

const PORT = parseInt(args.port, 10);
const DURATION_MS = args.duration ? parseInt(args.duration, 10) * 1000 : null;
const INTERVAL_MS = parseInt(args.interval, 10);
const GRID_SIZE = parseInt(args.grid, 10);
const JSON_OUTPUT = args.json;
const NODE_DISTANCE = parseFloat(args['node-distance']);
const ROOM_WIDTH = parseFloat(args['room-width']);
const ROOM_HEIGHT = parseFloat(args['room-height']);

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const CHANNEL_FREQ = {};
for (let ch = 1; ch <= 13; ch++) CHANNEL_FREQ[ch] = 2412 + (ch - 1) * 5;
CHANNEL_FREQ[14] = 2484;

const NODE1_CHANNELS = [1, 6, 11];
const NODE2_CHANNELS = [3, 5, 9];

// Known neighbor APs as additional illuminators (TX positions estimated)
const ILLUMINATORS = [
  { ssid: 'ruv.net',       channel: 5,  signal: 100, pos: [1.5, 3.5] },
  { ssid: 'Cohen-Guest',   channel: 5,  signal: 100, pos: [2.0, 3.8] },
  { ssid: 'COGECO-21B20',  channel: 11, signal: 100, pos: [4.0, 2.0] },
  { ssid: 'HP M255',       channel: 5,  signal: 94,  pos: [0.5, 1.5] },
  { ssid: 'conclusion',    channel: 3,  signal: 44,  pos: [3.5, 3.0] },
  { ssid: 'NETGEAR72',     channel: 9,  signal: 42,  pos: [4.5, 1.0] },
  { ssid: 'COGECO-4321',   channel: 11, signal: 30,  pos: [4.0, 3.5] },
  { ssid: 'Innanen',       channel: 6,  signal: 19,  pos: [1.0, 4.0] },
];

// Node positions (meters)
const NODE_POS = {
  1: [0, ROOM_HEIGHT / 2],
  2: [NODE_DISTANCE, ROOM_HEIGHT / 2],
};

// Heatmap characters (8 levels: transparent -> opaque)
const HEAT = [' ', '\u2591', '\u2591', '\u2592', '\u2592', '\u2593', '\u2593', '\u2588'];
const HEAT_LABELS = ['air', 'low', 'low', 'med', 'med', 'high', 'high', 'solid'];

// ---------------------------------------------------------------------------
// Tomographic grid
// ---------------------------------------------------------------------------
class TomographyGrid {
  constructor(gridSize, roomWidth, roomHeight) {
    this.gridSize = gridSize;
    this.roomWidth = roomWidth;
    this.roomHeight = roomHeight;
    this.cellWidth = roomWidth / gridSize;
    this.cellHeight = roomHeight / gridSize;

    // Accumulated attenuation per cell
    this.attenuation = new Float64Array(gridSize * gridSize);
    // Number of paths passing through each cell (for normalization)
    this.pathCount = new Float64Array(gridSize * gridSize);
    // Per-channel attenuation (for frequency analysis)
    this.channelAttenuation = new Map(); // channel -> Float64Array

    this.frameCount = 0;
    this.channelFrames = new Map();
  }

  /** Get center position of grid cell (row, col) in meters */
  cellCenter(row, col) {
    return [
      (col + 0.5) * this.cellWidth,
      (row + 0.5) * this.cellHeight,
    ];
  }

  /**
   * Perpendicular distance from point P to line segment AB.
   * Returns minimum distance to the infinite line through A and B.
   */
  pointToLineDistance(px, py, ax, ay, bx, by) {
    const dx = bx - ax;
    const dy = by - ay;
    const len = Math.sqrt(dx * dx + dy * dy);
    if (len < 1e-6) return Math.sqrt((px - ax) ** 2 + (py - ay) ** 2);
    // Signed distance using cross product
    return Math.abs((dy * px - dx * py + bx * ay - by * ax)) / len;
  }

  /**
   * Back-project attenuation along a TX->RX path.
   * Each cell near the path receives a weighted contribution.
   *
   * @param {number[]} txPos - Transmitter position [x, y]
   * @param {number[]} rxPos - Receiver position [x, y]
   * @param {number} atten - Measured attenuation (dB or normalized)
   * @param {number} channel - WiFi channel number
   */
  backProject(txPos, rxPos, atten, channel) {
    const [ax, ay] = txPos;
    const [bx, by] = rxPos;
    const pathLen = Math.sqrt((bx - ax) ** 2 + (by - ay) ** 2);
    if (pathLen < 0.01) return;

    // Kernel width: how far from the path the contribution extends
    // Approximately lambda/2 at 2.4 GHz = ~6 cm, but we use wider for stability
    const kernelWidth = Math.max(this.cellWidth, this.cellHeight) * 1.5;

    if (!this.channelAttenuation.has(channel)) {
      this.channelAttenuation.set(channel, new Float64Array(this.gridSize * this.gridSize));
    }
    const chAtten = this.channelAttenuation.get(channel);

    for (let r = 0; r < this.gridSize; r++) {
      for (let c = 0; c < this.gridSize; c++) {
        const [cx, cy] = this.cellCenter(r, c);
        const dist = this.pointToLineDistance(cx, cy, ax, ay, bx, by);

        if (dist < kernelWidth) {
          // Weight by proximity to path (Gaussian-like)
          const weight = Math.exp(-0.5 * (dist / (kernelWidth * 0.4)) ** 2);
          const idx = r * this.gridSize + c;
          this.attenuation[idx] += atten * weight;
          this.pathCount[idx] += weight;
          chAtten[idx] += atten * weight;
        }
      }
    }

    this.frameCount++;
    this.channelFrames.set(channel, (this.channelFrames.get(channel) || 0) + 1);
  }

  /** Get normalized attenuation image */
  getImage() {
    const img = new Float64Array(this.gridSize * this.gridSize);
    let maxVal = 0;

    for (let i = 0; i < img.length; i++) {
      img[i] = this.pathCount[i] > 0 ? this.attenuation[i] / this.pathCount[i] : 0;
      if (img[i] > maxVal) maxVal = img[i];
    }

    // Normalize to 0-1
    if (maxVal > 0) {
      for (let i = 0; i < img.length; i++) img[i] /= maxVal;
    }

    return img;
  }

  /** Get per-channel images for frequency analysis */
  getChannelImages() {
    const images = {};
    for (const [ch, chAtten] of this.channelAttenuation) {
      const img = new Float64Array(this.gridSize * this.gridSize);
      let maxVal = 0;
      for (let i = 0; i < img.length; i++) {
        img[i] = this.pathCount[i] > 0 ? chAtten[i] / this.pathCount[i] : 0;
        if (img[i] > maxVal) maxVal = img[i];
      }
      if (maxVal > 0) for (let i = 0; i < img.length; i++) img[i] /= maxVal;
      images[ch] = img;
    }
    return images;
  }

  /** Detect high-attenuation regions (potential person locations) */
  detectObjects(threshold = 0.6) {
    const img = this.getImage();
    const objects = [];

    for (let r = 0; r < this.gridSize; r++) {
      for (let c = 0; c < this.gridSize; c++) {
        const val = img[r * this.gridSize + c];
        if (val >= threshold) {
          const [x, y] = this.cellCenter(r, c);
          objects.push({
            row: r, col: c,
            x: x.toFixed(2), y: y.toFixed(2),
            attenuation: val.toFixed(3),
          });
        }
      }
    }

    return objects;
  }

  /** Reset accumulator for next window */
  reset() {
    this.attenuation.fill(0);
    this.pathCount.fill(0);
    this.channelAttenuation.clear();
    this.frameCount = 0;
    this.channelFrames.clear();
  }
}

// ---------------------------------------------------------------------------
// CSI parsing (shared with other scripts)
// ---------------------------------------------------------------------------
function parseIqHex(iqHex, nSubcarriers) {
  const bytes = Buffer.from(iqHex, 'hex');
  const amplitudes = new Float64Array(nSubcarriers);
  const phases = new Float64Array(nSubcarriers);

  for (let sc = 0; sc < nSubcarriers; sc++) {
    const offset = 2 + sc * 2;
    if (offset + 1 >= bytes.length) break;
    let I = bytes[offset];
    let Q = bytes[offset + 1];
    if (I > 127) I -= 256;
    if (Q > 127) Q -= 256;
    amplitudes[sc] = Math.sqrt(I * I + Q * Q);
    phases[sc] = Math.atan2(Q, I);
  }

  return { amplitudes, phases };
}

function parseCSIFrame(buf) {
  return parseRawCsiFrame(buf);
}

/**
 * Compute mean amplitude as a proxy for path attenuation.
 * Higher amplitude = less attenuation. We invert for the tomography grid.
 */
function computeAttenuation(amplitudes) {
  let sum = 0;
  for (let i = 0; i < amplitudes.length; i++) sum += amplitudes[i];
  const mean = sum / amplitudes.length;
  // Free-space reference (approximate, empirically calibrated)
  const freeSpaceRef = 15.0;
  // Attenuation: how much below free-space reference
  return Math.max(0, freeSpaceRef - mean);
}

// ---------------------------------------------------------------------------
// Channel assignment for legacy JSONL (no freq field)
// ---------------------------------------------------------------------------
const nodeChannelIdx = { 1: 0, 2: 0 };

function assignChannel(nodeId) {
  const channels = nodeId === 1 ? NODE1_CHANNELS : NODE2_CHANNELS;
  const ch = channels[nodeChannelIdx[nodeId] % channels.length];
  nodeChannelIdx[nodeId]++;
  return ch;
}

// ---------------------------------------------------------------------------
// Visualization
// ---------------------------------------------------------------------------
function renderHeatmap(grid) {
  const img = grid.getImage();
  const gs = grid.gridSize;

  const lines = [];
  lines.push('');
  lines.push('  RF Tomographic Image');
  lines.push('  ' + '='.repeat(gs * 2 + 2));

  // Y-axis label
  for (let r = 0; r < gs; r++) {
    const y = ((gs - r - 0.5) / gs * grid.roomHeight).toFixed(1);
    let row = `${y.padStart(4)}m |`;
    for (let c = 0; c < gs; c++) {
      const val = img[r * gs + c];
      const level = Math.floor(val * 7.99);
      row += HEAT[Math.max(0, Math.min(7, level))] + ' ';
    }
    row += '|';
    lines.push('  ' + row);
  }

  // X-axis
  lines.push('  ' + ' '.repeat(6) + '+' + '-'.repeat(gs * 2) + '+');
  let xLabels = ' '.repeat(7);
  for (let c = 0; c < gs; c += Math.max(1, Math.floor(gs / 5))) {
    const x = (c / gs * grid.roomWidth).toFixed(1);
    xLabels += x.padEnd(Math.floor(gs / 5) * 2 || 2);
  }
  lines.push('  ' + xLabels + ' (m)');

  // Legend
  lines.push('');
  lines.push('  Legend: ' + HEAT.map((ch, i) =>
    `${ch}=${HEAT_LABELS[i]}`
  ).join('  '));

  // Node positions
  const n1c = Math.floor(NODE_POS[1][0] / grid.roomWidth * gs);
  const n1r = gs - 1 - Math.floor(NODE_POS[1][1] / grid.roomHeight * gs);
  const n2c = Math.floor(NODE_POS[2][0] / grid.roomWidth * gs);
  const n2r = gs - 1 - Math.floor(NODE_POS[2][1] / grid.roomHeight * gs);
  lines.push(`  Node 1: (${NODE_POS[1][0]}, ${NODE_POS[1][1]}) m  [grid ${n1r},${n1c}]`);
  lines.push(`  Node 2: (${NODE_POS[2][0]}, ${NODE_POS[2][1]}) m  [grid ${n2r},${n2c}]`);

  return lines.join('\n');
}

function renderStats(grid) {
  const lines = [];
  lines.push(`  Frames: ${grid.frameCount}`);

  const chFrames = [...grid.channelFrames.entries()].sort((a, b) => a[0] - b[0]);
  if (chFrames.length > 0) {
    lines.push('  Per-channel frames: ' + chFrames.map(([ch, n]) =>
      `ch${ch}=${n}`
    ).join(' '));
  }

  const objects = grid.detectObjects(0.6);
  if (objects.length > 0) {
    lines.push(`  Detected ${objects.length} high-attenuation region(s):`);
    for (const obj of objects.slice(0, 5)) {
      lines.push(`    (${obj.x}, ${obj.y}) m  attenuation=${obj.attenuation}`);
    }
  } else {
    lines.push('  No high-attenuation regions detected');
  }

  return lines.join('\n');
}

function renderChannelComparison(grid) {
  const images = grid.getChannelImages();
  const channels = Object.keys(images).map(Number).sort((a, b) => a - b);
  if (channels.length < 2) return '';

  const gs = grid.gridSize;
  const lines = [];
  lines.push('');
  lines.push('  Per-Channel Attenuation (middle row):');

  const midRow = Math.floor(gs / 2);
  for (const ch of channels) {
    const img = images[ch];
    let bar = `  ch${String(ch).padStart(2)}: `;
    for (let c = 0; c < gs; c++) {
      const val = img[midRow * gs + c];
      const level = Math.floor(val * 7.99);
      bar += HEAT[Math.max(0, Math.min(7, level))] + ' ';
    }
    lines.push(bar);
  }

  return lines.join('\n');
}

// ---------------------------------------------------------------------------
// Process a single CSI record
// ---------------------------------------------------------------------------
const grid = new TomographyGrid(GRID_SIZE, ROOM_WIDTH, ROOM_HEIGHT);
let lastDisplayMs = 0;

function processFrame(nodeId, amplitudes, channel, timestamp) {
  const atten = computeAttenuation(amplitudes);

  // Back-project along node-to-node path
  const txPos = NODE_POS[nodeId] || [0, 0];
  const otherNode = nodeId === 1 ? 2 : 1;
  const rxPos = NODE_POS[otherNode] || [NODE_DISTANCE, ROOM_HEIGHT / 2];

  grid.backProject(txPos, rxPos, atten, channel);

  // Also back-project along paths to known illuminators on this channel
  for (const il of ILLUMINATORS) {
    if (il.channel === channel) {
      grid.backProject(il.pos, txPos, atten * (il.signal / 100), channel);
    }
  }
}

function displayUpdate() {
  if (JSON_OUTPUT) {
    const img = grid.getImage();
    const objects = grid.detectObjects(0.6);
    console.log(JSON.stringify({
      timestamp: Date.now() / 1000,
      frames: grid.frameCount,
      channels: [...grid.channelFrames.keys()].sort(),
      image: Array.from(img).map(v => +v.toFixed(3)),
      gridSize: GRID_SIZE,
      roomWidth: ROOM_WIDTH,
      roomHeight: ROOM_HEIGHT,
      objects,
    }));
  } else {
    process.stdout.write('\x1B[2J\x1B[H'); // clear screen
    console.log(renderHeatmap(grid));
    console.log(renderStats(grid));
    console.log(renderChannelComparison(grid));
    console.log('');
    console.log('  Press Ctrl+C to exit');
  }
}

// ---------------------------------------------------------------------------
// Live mode (UDP)
// ---------------------------------------------------------------------------
function startLive() {
  const sock = dgram.createSocket('udp4');

  sock.on('message', (buf, rinfo) => {
    const frame = parseCSIFrame(buf);
    if (!frame) return;

    processFrame(frame.nodeId, frame.amplitudes, frame.channel, Date.now() / 1000);

    const now = Date.now();
    if (now - lastDisplayMs >= INTERVAL_MS) {
      displayUpdate();
      lastDisplayMs = now;
    }
  });

  sock.bind(PORT, () => {
    if (!JSON_OUTPUT) {
      console.log(`RF Tomography listening on UDP port ${PORT}`);
      console.log(`Grid: ${GRID_SIZE}x${GRID_SIZE}, Room: ${ROOM_WIDTH}x${ROOM_HEIGHT} m`);
      console.log(`Node distance: ${NODE_DISTANCE} m`);
      console.log('Waiting for CSI frames...');
    }
  });

  if (DURATION_MS) {
    setTimeout(() => {
      displayUpdate();
      sock.close();
      process.exit(0);
    }, DURATION_MS);
  }
}

// ---------------------------------------------------------------------------
// Replay mode (JSONL)
// ---------------------------------------------------------------------------
async function startReplay(filePath) {
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
  let windowCount = 0;

  for await (const line of rl) {
    if (!line.trim()) continue;

    let record;
    try { record = JSON.parse(line); } catch { continue; }
    if (record.type !== 'raw_csi' || !record.iq_hex) continue;

    const { amplitudes, phases } = parseIqHex(record.iq_hex, record.subcarriers || 64);
    const channel = record.channel || assignChannel(record.node_id);

    processFrame(record.node_id, amplitudes, channel, record.timestamp);
    frameCount++;

    const tsMs = record.timestamp * 1000;
    if (lastAnalysisTs === 0) lastAnalysisTs = tsMs;

    if (tsMs - lastAnalysisTs >= INTERVAL_MS) {
      windowCount++;
      if (JSON_OUTPUT) {
        displayUpdate();
      } else {
        console.log(`\n${'='.repeat(60)}`);
        console.log(`Window ${windowCount} | t=${record.timestamp.toFixed(1)}s | frames=${frameCount}`);
        console.log('='.repeat(60));
        console.log(renderHeatmap(grid));
        console.log(renderStats(grid));
        console.log(renderChannelComparison(grid));
      }
      lastAnalysisTs = tsMs;
    }
  }

  // Final output
  if (!JSON_OUTPUT) {
    console.log(`\n${'='.repeat(60)}`);
    console.log('FINAL RF TOMOGRAPHIC IMAGE');
    console.log('='.repeat(60));
    console.log(renderHeatmap(grid));
    console.log(renderStats(grid));
    console.log(renderChannelComparison(grid));
    console.log(`\nProcessed ${frameCount} frames in ${windowCount} windows`);
  } else {
    displayUpdate();
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
