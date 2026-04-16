#!/usr/bin/env node
/**
 * Passive Bistatic Radar — Multi-Frequency Mesh Application
 *
 * Uses neighbor WiFi APs as illuminators of opportunity to build range-Doppler
 * maps for moving target detection. Each neighbor AP is an uncontrolled
 * transmitter whose signals pass through the room and are modulated by people
 * and objects. The ESP32 nodes capture CSI from these transmissions across
 * 6 channels.
 *
 * This is the same principle used by military passive radar (Kolchuga, VERA-NG)
 * but with WiFi APs instead of broadcast towers.
 *
 * Requires multi-frequency mesh scanning (ADR-073): 2 ESP32 nodes hopping
 * across channels 1, 3, 5, 6, 9, 11.
 *
 * Usage:
 *   node scripts/passive-radar.js
 *   node scripts/passive-radar.js --port 5006 --duration 60
 *   node scripts/passive-radar.js --replay data/recordings/overnight-1775217646.csi.jsonl
 *   node scripts/passive-radar.js --node-distance 3.0
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
    interval:      { type: 'string', short: 'i', default: '3000' },
    json:          { type: 'boolean', default: false },
    'node-distance': { type: 'string', default: '3.0' },
    'doppler-bins': { type: 'string', default: '16' },
    'range-bins':   { type: 'string', default: '12' },
  },
  strict: true,
});

const PORT = parseInt(args.port, 10);
const DURATION_MS = args.duration ? parseInt(args.duration, 10) * 1000 : null;
const INTERVAL_MS = parseInt(args.interval, 10);
const JSON_OUTPUT = args.json;
const NODE_DISTANCE = parseFloat(args['node-distance']);
const DOPPLER_BINS = parseInt(args['doppler-bins'], 10);
const RANGE_BINS = parseInt(args['range-bins'], 10);

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const SPEED_OF_LIGHT = 3e8; // m/s

const CHANNEL_FREQ = {};
for (let ch = 1; ch <= 13; ch++) CHANNEL_FREQ[ch] = 2412 + (ch - 1) * 5;

const NODE1_CHANNELS = [1, 6, 11];
const NODE2_CHANNELS = [3, 5, 9];

// Neighbor APs as illuminators with estimated positions
const ILLUMINATORS = [
  { ssid: 'ruv.net',       channel: 5,  signal: 100, pos: [1.5, 3.5], freq: 2432e6 },
  { ssid: 'Cohen-Guest',   channel: 5,  signal: 100, pos: [2.0, 3.8], freq: 2432e6 },
  { ssid: 'COGECO-21B20',  channel: 11, signal: 100, pos: [4.0, 2.0], freq: 2462e6 },
  { ssid: 'HP M255',       channel: 5,  signal: 94,  pos: [0.5, 1.5], freq: 2432e6 },
  { ssid: 'conclusion',    channel: 3,  signal: 44,  pos: [3.5, 3.0], freq: 2422e6 },
  { ssid: 'NETGEAR72',     channel: 9,  signal: 42,  pos: [4.5, 1.0], freq: 2452e6 },
  { ssid: 'COGECO-4321',   channel: 11, signal: 30,  pos: [4.0, 3.5], freq: 2462e6 },
  { ssid: 'Innanen',       channel: 6,  signal: 19,  pos: [1.0, 4.0], freq: 2437e6 },
];

const NODE_POS = {
  1: [0, 2.0],
  2: [NODE_DISTANCE, 2.0],
};

// Range-Doppler plot characters
const RD_CHARS = [' ', '\u2581', '\u2582', '\u2583', '\u2584', '\u2585', '\u2586', '\u2587', '\u2588'];

// ---------------------------------------------------------------------------
// Per-illuminator CSI history for Doppler processing
// ---------------------------------------------------------------------------
class IlluminatorTracker {
  constructor(illuminator, nodeId) {
    this.illuminator = illuminator;
    this.nodeId = nodeId;
    this.ssid = illuminator.ssid;
    this.channel = illuminator.channel;
    this.freqHz = illuminator.freq;
    this.wavelength = SPEED_OF_LIGHT / this.freqHz;

    // Phase history per subcarrier (ring buffer)
    this.maxHistory = 64;
    this.phaseHistory = []; // array of { timestamp, phases: Float64Array }
    this.amplitudeHistory = [];

    // Range-Doppler map
    this.rangeDoppler = null;
    this.lastUpdateMs = 0;
  }

  /** Ingest a new CSI frame */
  ingest(timestamp, amplitudes, phases) {
    this.phaseHistory.push({ timestamp, phases: new Float64Array(phases) });
    this.amplitudeHistory.push({ timestamp, amplitudes: new Float64Array(amplitudes) });

    if (this.phaseHistory.length > this.maxHistory) {
      this.phaseHistory.shift();
      this.amplitudeHistory.shift();
    }
  }

  /**
   * Compute range-Doppler map from CSI phase history.
   *
   * Doppler: phase change rate across consecutive frames for each subcarrier.
   *   fd = d(phase)/dt / (2*pi) -> velocity = fd * wavelength / 2
   *
   * Range: phase slope across subcarriers within each frame.
   *   tau = d(phase)/d(subcarrier_freq) / (2*pi) -> range = c * tau
   */
  computeRangeDoppler(dopplerBins, rangeBins) {
    const n = this.phaseHistory.length;
    if (n < 4) return null;

    const nSub = this.phaseHistory[0].phases.length;
    if (nSub < 4) return null;

    // Initialize range-Doppler map
    const rd = new Float64Array(rangeBins * dopplerBins);

    // Doppler processing: compute phase change rate per subcarrier
    const dopplerPerSub = new Float64Array(nSub);
    const rangePerFrame = new Float64Array(n);

    for (let sc = 0; sc < nSub; sc++) {
      // Linear regression of phase vs time for this subcarrier
      let sumT = 0, sumP = 0, sumTT = 0, sumTP = 0;
      let prevPhase = this.phaseHistory[0].phases[sc];

      for (let f = 0; f < n; f++) {
        const t = this.phaseHistory[f].timestamp;
        // Unwrap phase
        let phase = this.phaseHistory[f].phases[sc];
        while (phase - prevPhase > Math.PI) phase -= 2 * Math.PI;
        while (phase - prevPhase < -Math.PI) phase += 2 * Math.PI;
        prevPhase = phase;

        sumT += t;
        sumP += phase;
        sumTT += t * t;
        sumTP += t * phase;
      }

      const meanT = sumT / n;
      const denom = sumTT - sumT * meanT;
      if (Math.abs(denom) > 1e-10) {
        const slope = (sumTP - sumT * (sumP / n)) / denom;
        // Doppler frequency (Hz) = slope / (2*pi)
        dopplerPerSub[sc] = slope / (2 * Math.PI);
      }
    }

    // Range processing: phase slope across subcarriers per frame
    const subcarrierSpacing = 312.5e3; // OFDM subcarrier spacing: 312.5 kHz

    for (let f = 0; f < n; f++) {
      const phases = this.phaseHistory[f].phases;
      // Linear regression of phase vs subcarrier index
      let sumI = 0, sumP = 0, sumII = 0, sumIP = 0;
      let prevPhase = phases[0];

      for (let sc = 0; sc < nSub; sc++) {
        let phase = phases[sc];
        // Unwrap
        while (phase - prevPhase > Math.PI) phase -= 2 * Math.PI;
        while (phase - prevPhase < -Math.PI) phase += 2 * Math.PI;
        prevPhase = phase;

        sumI += sc;
        sumP += phase;
        sumII += sc * sc;
        sumIP += sc * phase;
      }

      const meanI = sumI / nSub;
      const denom = sumII - sumI * meanI;
      if (Math.abs(denom) > 1e-10) {
        const slope = (sumIP - sumI * (sumP / nSub)) / denom;
        // Time delay (seconds) = slope / (2*pi * subcarrier_spacing)
        const tau = Math.abs(slope) / (2 * Math.PI * subcarrierSpacing);
        rangePerFrame[f] = SPEED_OF_LIGHT * tau / 2; // bistatic range / 2
      }
    }

    // Map to bins
    const maxDoppler = 5.0;  // Hz (corresponds to ~0.3 m/s at 2.4 GHz)
    const maxRange = 10.0;   // meters

    for (let sc = 0; sc < nSub; sc++) {
      const doppler = dopplerPerSub[sc];
      const dBin = Math.floor(((doppler + maxDoppler) / (2 * maxDoppler)) * (dopplerBins - 1));
      if (dBin < 0 || dBin >= dopplerBins) continue;

      // Use mean amplitude as intensity
      let meanAmp = 0;
      for (let f = 0; f < n; f++) {
        meanAmp += this.amplitudeHistory[f].amplitudes[sc];
      }
      meanAmp /= n;

      // Average range across frames for this subcarrier's range bin
      let meanRange = 0;
      for (let f = 0; f < n; f++) meanRange += rangePerFrame[f];
      meanRange /= n;

      const rBin = Math.floor((meanRange / maxRange) * (rangeBins - 1));
      if (rBin < 0 || rBin >= rangeBins) continue;

      rd[rBin * dopplerBins + dBin] += meanAmp;
    }

    this.rangeDoppler = {
      map: rd,
      dopplerBins,
      rangeBins,
      maxDoppler,
      maxRange,
      nFrames: n,
    };

    return this.rangeDoppler;
  }

  /** Get dominant Doppler (strongest moving target) */
  getDominantDoppler() {
    if (!this.rangeDoppler) return null;
    const { map, dopplerBins, rangeBins, maxDoppler } = this.rangeDoppler;

    let maxVal = 0, maxD = 0, maxR = 0;
    for (let r = 0; r < rangeBins; r++) {
      for (let d = 0; d < dopplerBins; d++) {
        const val = map[r * dopplerBins + d];
        if (val > maxVal) {
          maxVal = val;
          maxD = d;
          maxR = r;
        }
      }
    }

    if (maxVal < 0.01) return null;

    const doppler = (maxD / (dopplerBins - 1)) * 2 * maxDoppler - maxDoppler;
    const velocity = doppler * this.wavelength / 2;
    const range = (maxR / (rangeBins - 1)) * this.rangeDoppler.maxRange;

    return { doppler: doppler.toFixed(2), velocity: velocity.toFixed(3), range: range.toFixed(1), intensity: maxVal.toFixed(1) };
  }
}

// ---------------------------------------------------------------------------
// Multi-static fusion
// ---------------------------------------------------------------------------
class MultiStaticFusion {
  constructor() {
    this.trackers = new Map(); // key: `${ssid}-node${nodeId}` -> IlluminatorTracker
  }

  getOrCreateTracker(illuminator, nodeId) {
    const key = `${illuminator.ssid}-node${nodeId}`;
    if (!this.trackers.has(key)) {
      this.trackers.set(key, new IlluminatorTracker(illuminator, nodeId));
    }
    return this.trackers.get(key);
  }

  ingestFrame(nodeId, channel, timestamp, amplitudes, phases) {
    // Find illuminators on this channel
    for (const il of ILLUMINATORS) {
      if (il.channel === channel) {
        const tracker = this.getOrCreateTracker(il, nodeId);
        tracker.ingest(timestamp, amplitudes, phases);
      }
    }
  }

  /** Compute all range-Doppler maps */
  computeAll(dopplerBins, rangeBins) {
    const results = [];
    for (const [key, tracker] of this.trackers) {
      const rd = tracker.computeRangeDoppler(dopplerBins, rangeBins);
      if (rd) {
        results.push({ key, tracker, rd });
      }
    }
    return results;
  }

  /**
   * Fuse multi-static detections.
   * Each illuminator provides a range measurement to the target.
   * The target lies on an ellipse with foci at TX (illuminator) and RX (ESP32 node).
   * Intersection of multiple ellipses gives position.
   */
  fuseDetections() {
    const detections = [];
    for (const [key, tracker] of this.trackers) {
      const dom = tracker.getDominantDoppler();
      if (dom && parseFloat(dom.intensity) > 1.0) {
        detections.push({
          key,
          ssid: tracker.ssid,
          channel: tracker.channel,
          nodeId: tracker.nodeId,
          txPos: tracker.illuminator.pos,
          rxPos: NODE_POS[tracker.nodeId],
          bistaticRange: parseFloat(dom.range),
          velocity: parseFloat(dom.velocity),
          intensity: parseFloat(dom.intensity),
        });
      }
    }

    if (detections.length < 2) {
      return { detections, fusedPosition: null };
    }

    // Simple centroid-based fusion:
    // For each detection, compute the midpoint of the TX-RX baseline
    // weighted by intensity. This is a rough approximation.
    // (Full ellipse intersection requires nonlinear optimization.)
    let sumX = 0, sumY = 0, sumW = 0;
    for (const det of detections) {
      // Midpoint between TX and RX, offset by bistatic range
      const mx = (det.txPos[0] + det.rxPos[0]) / 2;
      const my = (det.txPos[1] + det.rxPos[1]) / 2;
      const w = det.intensity;
      sumX += mx * w;
      sumY += my * w;
      sumW += w;
    }

    const fusedPosition = sumW > 0
      ? { x: (sumX / sumW).toFixed(2), y: (sumY / sumW).toFixed(2), confidence: Math.min(1, detections.length / 4).toFixed(2) }
      : null;

    return { detections, fusedPosition };
  }
}

// ---------------------------------------------------------------------------
// CSI parsing
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

// Channel assignment for legacy JSONL
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
function renderRangeDoppler(tracker) {
  const rd = tracker.rangeDoppler;
  if (!rd) return `  ${tracker.ssid} (ch${tracker.channel}): insufficient data`;

  const { map, dopplerBins, rangeBins, maxDoppler, maxRange, nFrames } = rd;
  const lines = [];

  lines.push(`  ${tracker.ssid} (ch${tracker.channel}, node${tracker.nodeId}) | ${nFrames} frames`);

  // Find max for normalization
  let maxVal = 0;
  for (let i = 0; i < map.length; i++) {
    if (map[i] > maxVal) maxVal = map[i];
  }
  if (maxVal === 0) maxVal = 1;

  // Render range (y-axis) vs Doppler (x-axis)
  for (let r = rangeBins - 1; r >= 0; r--) {
    const range = (r / (rangeBins - 1)) * maxRange;
    let row = `  ${range.toFixed(1).padStart(5)}m |`;
    for (let d = 0; d < dopplerBins; d++) {
      const val = map[r * dopplerBins + d] / maxVal;
      const level = Math.floor(val * 8.99);
      row += RD_CHARS[Math.max(0, Math.min(8, level))];
    }
    row += '|';
    lines.push(row);
  }

  // X-axis (Doppler)
  lines.push('  ' + ' '.repeat(7) + '+' + '-'.repeat(dopplerBins) + '+');
  const dLabel = `  ${' '.repeat(7)}-${maxDoppler}Hz${' '.repeat(Math.max(0, dopplerBins - 10))}+${maxDoppler}Hz`;
  lines.push(dLabel);

  // Dominant detection
  const dom = tracker.getDominantDoppler();
  if (dom) {
    lines.push(`  Peak: range=${dom.range}m  doppler=${dom.doppler}Hz  vel=${dom.velocity}m/s`);
  }

  return lines.join('\n');
}

function renderFusion(fusion) {
  const { detections, fusedPosition } = fusion;
  const lines = [];

  lines.push('');
  lines.push('  MULTI-STATIC FUSION');
  lines.push('  ' + '='.repeat(50));

  if (detections.length === 0) {
    lines.push('  No detections above threshold');
    return lines.join('\n');
  }

  lines.push(`  Active bistatic pairs: ${detections.length}`);
  for (const det of detections) {
    lines.push(`    ${det.ssid.padEnd(16)} ch${det.channel} -> node${det.nodeId} | ` +
      `range=${det.bistaticRange.toFixed(1)}m vel=${det.velocity.toFixed(3)}m/s`);
  }

  if (fusedPosition) {
    lines.push(`  Fused position: (${fusedPosition.x}, ${fusedPosition.y}) m  confidence=${fusedPosition.confidence}`);
  } else {
    lines.push('  Insufficient detections for position fusion (need 2+)');
  }

  return lines.join('\n');
}

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------
const multiStatic = new MultiStaticFusion();
let lastDisplayMs = 0;

function processFrame(nodeId, channel, timestamp, amplitudes, phases) {
  multiStatic.ingestFrame(nodeId, channel, timestamp, amplitudes, phases);
}

function displayUpdate() {
  const results = multiStatic.computeAll(DOPPLER_BINS, RANGE_BINS);
  const fusion = multiStatic.fuseDetections();

  if (JSON_OUTPUT) {
    const output = {
      timestamp: Date.now() / 1000,
      bistaticPairs: results.length,
      detections: fusion.detections.map(d => ({
        ssid: d.ssid, channel: d.channel, nodeId: d.nodeId,
        bistaticRange: d.bistaticRange, velocity: d.velocity,
      })),
      fusedPosition: fusion.fusedPosition,
    };
    console.log(JSON.stringify(output));
  } else {
    process.stdout.write('\x1B[2J\x1B[H');
    console.log('  PASSIVE BISTATIC RADAR');
    console.log('  Using neighbor WiFi APs as illuminators of opportunity');
    console.log('  ' + '-'.repeat(55));
    console.log('');

    // Show top 3 trackers by signal strength
    const sorted = results.sort((a, b) => b.tracker.illuminator.signal - a.tracker.illuminator.signal);
    for (const r of sorted.slice(0, 3)) {
      console.log(renderRangeDoppler(r.tracker));
      console.log('');
    }

    console.log(renderFusion(fusion));
    console.log('');
    console.log(`  Total bistatic pairs: ${multiStatic.trackers.size}`);
    console.log('  Press Ctrl+C to exit');
  }
}

// ---------------------------------------------------------------------------
// Live mode
// ---------------------------------------------------------------------------
function startLive() {
  const sock = dgram.createSocket('udp4');

  sock.on('message', (buf, rinfo) => {
    const frame = parseCSIFrame(buf);
    if (!frame) return;

    processFrame(frame.nodeId, frame.channel, Date.now() / 1000, frame.amplitudes, frame.phases);

    const now = Date.now();
    if (now - lastDisplayMs >= INTERVAL_MS) {
      displayUpdate();
      lastDisplayMs = now;
    }
  });

  sock.bind(PORT, () => {
    if (!JSON_OUTPUT) {
      console.log(`Passive Bistatic Radar listening on UDP port ${PORT}`);
      console.log(`Illuminators: ${ILLUMINATORS.length} neighbor APs`);
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
// Replay mode
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

    processFrame(record.node_id, channel, record.timestamp, amplitudes, phases);
    frameCount++;

    const tsMs = record.timestamp * 1000;
    if (lastAnalysisTs === 0) lastAnalysisTs = tsMs;

    if (tsMs - lastAnalysisTs >= INTERVAL_MS) {
      windowCount++;
      const results = multiStatic.computeAll(DOPPLER_BINS, RANGE_BINS);
      const fusion = multiStatic.fuseDetections();

      if (JSON_OUTPUT) {
        console.log(JSON.stringify({
          window: windowCount,
          timestamp: record.timestamp,
          frames: frameCount,
          detections: fusion.detections.length,
          fusedPosition: fusion.fusedPosition,
        }));
      } else {
        console.log(`\n${'='.repeat(60)}`);
        console.log(`Window ${windowCount} | t=${record.timestamp.toFixed(1)}s | frames=${frameCount}`);
        console.log('='.repeat(60));

        const sorted = results.sort((a, b) => b.tracker.illuminator.signal - a.tracker.illuminator.signal);
        for (const r of sorted.slice(0, 3)) {
          console.log(renderRangeDoppler(r.tracker));
          console.log('');
        }

        console.log(renderFusion(fusion));
      }
      lastAnalysisTs = tsMs;
    }
  }

  // Final
  if (!JSON_OUTPUT) {
    const results = multiStatic.computeAll(DOPPLER_BINS, RANGE_BINS);
    const fusion = multiStatic.fuseDetections();

    console.log(`\n${'='.repeat(60)}`);
    console.log('FINAL PASSIVE RADAR SUMMARY');
    console.log('='.repeat(60));

    for (const [key, tracker] of multiStatic.trackers) {
      const dom = tracker.getDominantDoppler();
      const domStr = dom ? `range=${dom.range}m vel=${dom.velocity}m/s` : 'no detection';
      console.log(`  ${key.padEnd(30)} ${domStr}`);
    }

    console.log(renderFusion(fusion));
    console.log(`\nProcessed ${frameCount} frames in ${windowCount} windows`);
    console.log(`Bistatic pairs tracked: ${multiStatic.trackers.size}`);
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
