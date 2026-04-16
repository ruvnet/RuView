#!/usr/bin/env node
/**
 * RuView RF Room Scanner — Live CSI spectrum analyzer
 *
 * Listens on UDP for ADR-018 CSI frames from ESP32 nodes and builds a
 * real-time RF map of the room showing null zones (metal), static reflectors,
 * dynamic subcarriers (people), and cross-node correlation.
 *
 * Usage:
 *   node scripts/rf-scan.js
 *   node scripts/rf-scan.js --port 5006 --duration 30
 *   node scripts/rf-scan.js --json
 *
 * ADR: docs/adr/ADR-073-multifrequency-mesh-scan.md
 */

'use strict';

const dgram = require('dgram');
const { parseArgs } = require('util');
const { parseRawCsiFrame } = require('./lib/raw-csi');

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------
const { values: args } = parseArgs({
  options: {
    port:     { type: 'string', short: 'p', default: '5006' },
    bind:     { type: 'string', short: 'b', default: '0.0.0.0' },
    duration: { type: 'string', short: 'd' },
    json:     { type: 'boolean', default: false },
    interval: { type: 'string', short: 'i', default: '2000' },
  },
  strict: true,
});

const PORT = parseInt(args.port, 10);
const DURATION_MS = args.duration ? parseInt(args.duration, 10) * 1000 : null;
const INTERVAL_MS = parseInt(args.interval, 10);
const JSON_OUTPUT = args.json;

// ---------------------------------------------------------------------------
// ADR-018 packet constants
// ---------------------------------------------------------------------------
const VITALS_MAGIC  = 0xC5110002;
const FEATURE_MAGIC = 0xC5110003;
const FUSED_MAGIC   = 0xC5110004;

// Spectrum visualization characters (8 levels)
const BARS = ['\u2581', '\u2582', '\u2583', '\u2584', '\u2585', '\u2586', '\u2587', '\u2588'];

// Subcarrier type markers
const TYPE_WALL      = '.';
const TYPE_PERSON    = '^';
const TYPE_REFLECTOR = '#';
const TYPE_NULL      = '_';
const TYPE_UNKNOWN   = ' ';

// Thresholds
const NULL_THRESHOLD      = 2.0;    // Amplitude below this = null subcarrier
const DYNAMIC_VAR_THRESH  = 0.15;   // Variance above this = dynamic (person/motion)
const STRONG_AMP_THRESH   = 0.85;   // Normalized amplitude above this = strong reflector
const COHERENCE_THRESH    = 0.7;    // Phase coherence above this = line-of-sight

// ---------------------------------------------------------------------------
// Per-node state
// ---------------------------------------------------------------------------
class NodeState {
  constructor(nodeId) {
    this.nodeId = nodeId;
    this.address = null;
    this.channel = 0;
    this.freqMhz = 0;
    this.rssi = 0;
    this.noiseFloor = 0;
    this.nSubcarriers = 0;
    this.frameCount = 0;
    this.firstFrameMs = Date.now();
    this.lastFrameMs = Date.now();

    // Per-subcarrier rolling state
    this.amplitudes = new Float64Array(256);
    this.phases = new Float64Array(256);
    this.ampHistory = [];      // circular buffer of amplitude snapshots
    this.phaseHistory = [];    // circular buffer of phase snapshots
    this.historyMaxLen = 50;   // ~10 seconds at 5 fps

    // Welford variance per subcarrier
    this.ampMean  = new Float64Array(256);
    this.ampM2    = new Float64Array(256);
    this.ampCount = new Uint32Array(256);

    // Latest vitals
    this.vitals = null;
    this.features = null;
  }

  get fps() {
    const elapsed = (this.lastFrameMs - this.firstFrameMs) / 1000;
    return elapsed > 0 ? this.frameCount / elapsed : 0;
  }

  channelFromFreq() {
    if (this.freqMhz >= 2412 && this.freqMhz <= 2484) {
      if (this.freqMhz === 2484) return 14;
      return Math.round((this.freqMhz - 2412) / 5) + 1;
    }
    if (this.freqMhz >= 5180) {
      return Math.round((this.freqMhz - 5000) / 5);
    }
    return 0;
  }

  updateAmplitudes(amplitudes, phases) {
    const n = amplitudes.length;
    this.nSubcarriers = n;

    for (let i = 0; i < n; i++) {
      this.amplitudes[i] = amplitudes[i];
      this.phases[i] = phases[i];

      // Welford online variance
      this.ampCount[i]++;
      const delta = amplitudes[i] - this.ampMean[i];
      this.ampMean[i] += delta / this.ampCount[i];
      const delta2 = amplitudes[i] - this.ampMean[i];
      this.ampM2[i] += delta * delta2;
    }

    // Store history snapshot
    this.ampHistory.push(Float64Array.from(amplitudes));
    this.phaseHistory.push(Float64Array.from(phases));
    if (this.ampHistory.length > this.historyMaxLen) {
      this.ampHistory.shift();
      this.phaseHistory.shift();
    }
  }

  getVariance(i) {
    return this.ampCount[i] > 1 ? this.ampM2[i] / (this.ampCount[i] - 1) : 0;
  }

  classify() {
    const n = this.nSubcarriers;
    if (n === 0) return { nulls: [], dynamic: [], reflectors: [], walls: [] };

    // Find max amplitude for normalization
    let maxAmp = 0;
    for (let i = 0; i < n; i++) {
      if (this.amplitudes[i] > maxAmp) maxAmp = this.amplitudes[i];
    }
    if (maxAmp === 0) maxAmp = 1;

    const nulls = [];
    const dynamic = [];
    const reflectors = [];
    const walls = [];

    for (let i = 0; i < n; i++) {
      const normAmp = this.amplitudes[i] / maxAmp;
      const variance = this.getVariance(i);

      if (this.amplitudes[i] < NULL_THRESHOLD) {
        nulls.push(i);
      } else if (variance > DYNAMIC_VAR_THRESH) {
        dynamic.push(i);
      } else if (normAmp > STRONG_AMP_THRESH) {
        reflectors.push(i);
      } else {
        walls.push(i);
      }
    }

    return { nulls, dynamic, reflectors, walls };
  }

  getTypeMap() {
    const n = this.nSubcarriers;
    const types = new Array(n).fill(TYPE_UNKNOWN);
    const { nulls, dynamic, reflectors, walls } = this.classify();

    for (const i of nulls) types[i] = TYPE_NULL;
    for (const i of dynamic) types[i] = TYPE_PERSON;
    for (const i of reflectors) types[i] = TYPE_REFLECTOR;
    for (const i of walls) types[i] = TYPE_WALL;

    return types;
  }

  getSpectrumBar() {
    const n = this.nSubcarriers;
    if (n === 0) return '';

    let maxAmp = 0;
    for (let i = 0; i < n; i++) {
      if (this.amplitudes[i] > maxAmp) maxAmp = this.amplitudes[i];
    }
    if (maxAmp === 0) maxAmp = 1;

    let bar = '';
    for (let i = 0; i < n; i++) {
      const level = Math.floor((this.amplitudes[i] / maxAmp) * 7.99);
      bar += BARS[Math.max(0, Math.min(7, level))];
    }
    return bar;
  }
}

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------
const nodes = new Map();           // nodeId -> NodeState
const startTime = Date.now();
let totalFrames = 0;

// ---------------------------------------------------------------------------
// Packet parsing
// ---------------------------------------------------------------------------
function parseCSIFrame(buf) {
  return parseRawCsiFrame(buf);
}

function parseVitalsPacket(buf) {
  if (buf.length < 32) return null;

  const magic = buf.readUInt32LE(0);
  if (magic !== VITALS_MAGIC && magic !== FUSED_MAGIC) return null;

  const nodeId        = buf.readUInt8(4);
  const flags         = buf.readUInt8(5);
  const breathingRate = buf.readUInt16LE(6) / 100;
  const heartrate     = buf.readUInt32LE(8) / 10000;
  const rssi          = buf.readInt8(12);
  const nPersons      = buf.readUInt8(13);
  const motionEnergy  = buf.readFloatLE(16);
  const presenceScore = buf.readFloatLE(20);
  const timestampMs   = buf.readUInt32LE(24);

  return {
    nodeId, flags,
    presence: !!(flags & 0x01),
    fall: !!(flags & 0x02),
    motion: !!(flags & 0x04),
    breathingRate, heartrate, rssi, nPersons,
    motionEnergy, presenceScore, timestampMs,
    isFused: magic === FUSED_MAGIC,
  };
}

function parseFeaturePacket(buf) {
  if (buf.length < 48) return null;

  const magic = buf.readUInt32LE(0);
  if (magic !== FEATURE_MAGIC) return null;

  const nodeId = buf.readUInt8(4);
  const seq    = buf.readUInt16LE(6);
  const features = [];
  for (let i = 0; i < 8; i++) {
    features.push(buf.readFloatLE(12 + i * 4));
  }

  return { nodeId, seq, features };
}

function handlePacket(buf, rinfo) {
  // Try CSI frame first (most common)
  if (buf.length >= 4) {
    const frame = parseCSIFrame(buf);
    if (frame) {
      totalFrames++;
      let node = nodes.get(frame.nodeId);
      if (!node) {
        node = new NodeState(frame.nodeId);
        nodes.set(frame.nodeId, node);
      }

      node.address = rinfo.address;
      node.freqMhz = frame.freqMhz;
      node.channel = node.channelFromFreq();
      node.rssi = frame.rssi;
      node.noiseFloor = frame.noiseFloor;
      node.frameCount++;
      node.lastFrameMs = Date.now();
      node.updateAmplitudes(frame.amplitudes, frame.phases);
      return;
    }

    const magic = buf.readUInt32LE(0);

    if (magic === VITALS_MAGIC || magic === FUSED_MAGIC) {
      const vitals = parseVitalsPacket(buf);
      if (!vitals) return;

      let node = nodes.get(vitals.nodeId);
      if (!node) {
        node = new NodeState(vitals.nodeId);
        nodes.set(vitals.nodeId, node);
      }
      node.vitals = vitals;
      return;
    }

    if (magic === FEATURE_MAGIC) {
      const feat = parseFeaturePacket(buf);
      if (!feat) return;

      let node = nodes.get(feat.nodeId);
      if (!node) {
        node = new NodeState(feat.nodeId);
        nodes.set(feat.nodeId, node);
      }
      node.features = feat;
      return;
    }
  }
}

// ---------------------------------------------------------------------------
// Cross-node analysis
// ---------------------------------------------------------------------------
function computeCrossNodeCorrelation() {
  const nodeList = [...nodes.values()].filter(n => n.nSubcarriers > 0);
  if (nodeList.length < 2) return null;

  const n0 = nodeList[0];
  const n1 = nodeList[1];
  const len = Math.min(n0.nSubcarriers, n1.nSubcarriers);

  // Pearson correlation of amplitude vectors
  let sumXY = 0, sumX = 0, sumY = 0, sumX2 = 0, sumY2 = 0;
  for (let i = 0; i < len; i++) {
    const x = n0.amplitudes[i];
    const y = n1.amplitudes[i];
    sumX += x; sumY += y;
    sumXY += x * y;
    sumX2 += x * x;
    sumY2 += y * y;
  }

  const denom = Math.sqrt((len * sumX2 - sumX * sumX) * (len * sumY2 - sumY * sumY));
  const correlation = denom > 0 ? (len * sumXY - sumX * sumY) / denom : 0;

  // Phase coherence between nodes
  let coherenceSum = 0;
  for (let i = 0; i < len; i++) {
    const phaseDiff = n0.phases[i] - n1.phases[i];
    coherenceSum += Math.cos(phaseDiff);
  }
  const phaseCoherence = len > 0 ? coherenceSum / len : 0;

  // Count matching nulls
  const c0 = n0.classify();
  const c1 = n1.classify();
  const nullSet0 = new Set(c0.nulls);
  const sharedNulls = c1.nulls.filter(i => nullSet0.has(i));

  return {
    correlation: correlation.toFixed(3),
    phaseCoherence: phaseCoherence.toFixed(3),
    los: phaseCoherence > COHERENCE_THRESH ? 'LINE-OF-SIGHT' : 'MULTIPATH',
    sharedNulls: sharedNulls.length,
    uniqueNulls0: c0.nulls.length - sharedNulls.length,
    uniqueNulls1: c1.nulls.length - sharedNulls.length,
  };
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------
function buildProgressBar(value, max, width) {
  const filled = Math.round((value / max) * width);
  return '\u2588'.repeat(Math.min(filled, width)) +
         '\u2591'.repeat(Math.max(0, width - filled));
}

function renderASCII() {
  const lines = [];
  const nodeList = [...nodes.values()].filter(n => n.nSubcarriers > 0);

  if (nodeList.length === 0) {
    lines.push(`=== RUVIEW RF SCAN === Listening on UDP :${PORT} ... no data yet`);
    lines.push('Waiting for CSI frames from ESP32 nodes...');
    lines.push(`Elapsed: ${((Date.now() - startTime) / 1000).toFixed(0)}s | Frames: ${totalFrames}`);
    return lines.join('\n');
  }

  for (const node of nodeList) {
    const ch = node.channel || '?';
    const freq = node.freqMhz || '?';
    lines.push(`=== RUVIEW RF SCAN -- Channel ${ch} (${freq} MHz) ===`);
    lines.push(`Node ${node.nodeId} (${node.address || '?'}) | ${node.fps.toFixed(1)} fps | RSSI ${node.rssi} dBm | Noise ${node.noiseFloor} dBm`);

    // Spectrum bar
    const spectrum = node.getSpectrumBar();
    if (spectrum.length > 0) {
      lines.push(`Spectrum: ${spectrum}`);

      // Type map
      const types = node.getTypeMap();
      lines.push(`Type:     ${types.join('')}`);
      lines.push(`          ${TYPE_WALL} wall  ${TYPE_PERSON} person  ${TYPE_REFLECTOR} reflector  ${TYPE_NULL} null(metal)`);
    }

    // Classification summary
    const cls = node.classify();
    lines.push('');
    lines.push(`Objects: ${cls.nulls.length} null zones (metal) | ${cls.dynamic.length} dynamic (person/motion) | ${cls.reflectors.length} strong reflectors | ${cls.walls.length} static`);

    const nullPct = node.nSubcarriers > 0
      ? ((cls.nulls.length / node.nSubcarriers) * 100).toFixed(0)
      : '0';
    lines.push(`Nulls:   ${nullPct}% of subcarriers blocked`);

    // Vitals
    if (node.vitals) {
      const v = node.vitals;
      const presenceBar = buildProgressBar(v.presenceScore, 1, 10);
      const motionBar = buildProgressBar(Math.min(v.motionEnergy, 1), 1, 10);
      const position = v.presenceScore > 0.5 ? 'CENTERED' : v.presenceScore > 0.2 ? 'PERIPHERAL' : 'EMPTY';

      lines.push(`Person:  ${position} | BR ${v.breathingRate.toFixed(0)} BPM | HR ${v.heartrate.toFixed(0)} BPM | Motion ${v.motion ? 'HIGH' : 'LOW'}${v.fall ? ' | !! FALL !!' : ''}`);
      lines.push(`Vitals:  ${presenceBar} ${v.presenceScore.toFixed(2)} presence | ${motionBar} ${v.motionEnergy.toFixed(2)} motion | ${v.nPersons} person(s)`);
    } else {
      lines.push('Person:  (awaiting vitals packet)');
    }

    // Feature vector
    if (node.features) {
      const fv = node.features.features.map(f => f.toFixed(3)).join(', ');
      lines.push(`Feature: [${fv}]`);
    }

    lines.push('');
  }

  // Cross-node analysis
  if (nodeList.length >= 2) {
    const cross = computeCrossNodeCorrelation();
    if (cross) {
      lines.push('--- Cross-Node Analysis ---');
      lines.push(`Correlation: ${cross.correlation} | Phase coherence: ${cross.phaseCoherence} | ${cross.los}`);
      lines.push(`Nulls: ${cross.sharedNulls} shared | ${cross.uniqueNulls0} node-0-only | ${cross.uniqueNulls1} node-1-only`);
      lines.push('');
    }
  }

  // Summary line
  const elapsed = ((Date.now() - startTime) / 1000).toFixed(0);
  lines.push(`Elapsed: ${elapsed}s | Total frames: ${totalFrames} | Nodes: ${nodeList.length}`);
  if (DURATION_MS) {
    const remaining = Math.max(0, (DURATION_MS - (Date.now() - startTime)) / 1000).toFixed(0);
    lines.push(`Remaining: ${remaining}s`);
  }

  return lines.join('\n');
}

function buildJsonOutput() {
  const nodeList = [...nodes.values()].filter(n => n.nSubcarriers > 0);

  const result = {
    timestamp: new Date().toISOString(),
    elapsedMs: Date.now() - startTime,
    totalFrames,
    nodes: nodeList.map(node => {
      const cls = node.classify();
      return {
        nodeId: node.nodeId,
        address: node.address,
        channel: node.channel,
        freqMhz: node.freqMhz,
        rssi: node.rssi,
        noiseFloor: node.noiseFloor,
        fps: parseFloat(node.fps.toFixed(2)),
        nSubcarriers: node.nSubcarriers,
        frameCount: node.frameCount,
        classification: {
          nullCount: cls.nulls.length,
          dynamicCount: cls.dynamic.length,
          reflectorCount: cls.reflectors.length,
          staticCount: cls.walls.length,
          nullPercent: node.nSubcarriers > 0
            ? parseFloat(((cls.nulls.length / node.nSubcarriers) * 100).toFixed(1))
            : 0,
        },
        vitals: node.vitals ? {
          presence: node.vitals.presence,
          presenceScore: node.vitals.presenceScore,
          motionEnergy: node.vitals.motionEnergy,
          breathingRate: node.vitals.breathingRate,
          heartrate: node.vitals.heartrate,
          nPersons: node.vitals.nPersons,
          fall: node.vitals.fall,
        } : null,
        features: node.features ? node.features.features : null,
        amplitudes: Array.from(node.amplitudes.subarray(0, node.nSubcarriers)),
        phases: Array.from(node.phases.subarray(0, node.nSubcarriers)),
      };
    }),
    crossNode: computeCrossNodeCorrelation(),
  };

  return result;
}

function display() {
  if (JSON_OUTPUT) {
    const data = buildJsonOutput();
    process.stdout.write(JSON.stringify(data) + '\n');
  } else {
    // Clear screen and move cursor to top
    process.stdout.write('\x1B[2J\x1B[H');
    process.stdout.write(renderASCII() + '\n');
  }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------
function main() {
  const server = dgram.createSocket('udp4');

  server.on('error', (err) => {
    console.error(`UDP error: ${err.message}`);
    server.close();
    process.exit(1);
  });

  server.on('message', (msg, rinfo) => {
    handlePacket(msg, rinfo);
  });

  server.on('listening', () => {
    const addr = server.address();
    if (!JSON_OUTPUT) {
      console.log(`RuView RF Scanner listening on ${addr.address}:${addr.port}`);
      console.log('Waiting for CSI frames from ESP32 nodes...\n');
    }
  });

  // On Windows, binding to 0.0.0.0 may be blocked by firewall.
  // Use --bind <ip> to specify your WiFi IP (e.g., --bind 192.168.1.20)
  server.bind(PORT, args.bind);

  // Periodic display update
  const displayTimer = setInterval(display, INTERVAL_MS);

  // Duration timeout
  if (DURATION_MS) {
    setTimeout(() => {
      clearInterval(displayTimer);

      if (JSON_OUTPUT) {
        // Final JSON summary
        const summary = buildJsonOutput();
        summary.final = true;
        process.stdout.write(JSON.stringify(summary) + '\n');
      } else {
        display();
        console.log('\n--- Scan complete ---');

        const nodeList = [...nodes.values()].filter(n => n.nSubcarriers > 0);
        console.log(`Duration: ${(DURATION_MS / 1000).toFixed(0)}s`);
        console.log(`Total frames: ${totalFrames}`);
        console.log(`Nodes detected: ${nodeList.length}`);

        for (const node of nodeList) {
          const cls = node.classify();
          console.log(`  Node ${node.nodeId}: ${node.frameCount} frames, ${node.fps.toFixed(1)} fps, ch ${node.channel}, ${cls.nulls.length} nulls (${((cls.nulls.length / Math.max(1, node.nSubcarriers)) * 100).toFixed(0)}%)`);
        }
      }

      server.close();
      process.exit(0);
    }, DURATION_MS);
  }

  // Graceful shutdown
  process.on('SIGINT', () => {
    clearInterval(displayTimer);
    if (!JSON_OUTPUT) {
      console.log('\nShutting down...');
    }
    server.close();
    process.exit(0);
  });
}

main();
