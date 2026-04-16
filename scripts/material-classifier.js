#!/usr/bin/env node
/**
 * Frequency-Selective Material Classification — Multi-Frequency Mesh Application
 *
 * Compares CSI null/attenuation patterns across 6 WiFi channels to classify
 * materials in the room. Different materials absorb WiFi at different rates
 * depending on frequency:
 *
 *   Metal:  blocks all frequencies equally (frequency-flat null)
 *   Water:  absorbs strongly, increasing with frequency (dielectric loss)
 *   Wood:   mild attenuation, increases with frequency (moisture)
 *   Glass:  low attenuation, nearly frequency-flat
 *   Human:  60-70% water, strong frequency-dependent absorption
 *
 * Requires multi-frequency mesh scanning (ADR-073): 2 ESP32 nodes hopping
 * across channels 1, 3, 5, 6, 9, 11.
 *
 * Usage:
 *   node scripts/material-classifier.js
 *   node scripts/material-classifier.js --port 5006 --duration 60
 *   node scripts/material-classifier.js --replay data/recordings/overnight-1775217646.csi.jsonl
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
    port:     { type: 'string', short: 'p', default: '5006' },
    duration: { type: 'string', short: 'd' },
    replay:   { type: 'string', short: 'r' },
    interval: { type: 'string', short: 'i', default: '5000' },
    json:     { type: 'boolean', default: false },
    window:   { type: 'string', short: 'w', default: '20' },
  },
  strict: true,
});

const PORT = parseInt(args.port, 10);
const DURATION_MS = args.duration ? parseInt(args.duration, 10) * 1000 : null;
const INTERVAL_MS = parseInt(args.interval, 10);
const JSON_OUTPUT = args.json;
const WINDOW_FRAMES = parseInt(args.window, 10);

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const CHANNEL_FREQ = {};
for (let ch = 1; ch <= 13; ch++) CHANNEL_FREQ[ch] = 2412 + (ch - 1) * 5;

const NODE1_CHANNELS = [1, 6, 11];
const NODE2_CHANNELS = [3, 5, 9];

// Material classification thresholds
const NULL_THRESHOLD = 2.0;

// Material types
const MATERIAL = {
  METAL:   { name: 'Metal',   char: '#', desc: 'Total block, frequency-flat' },
  WATER:   { name: 'Water',   char: '~', desc: 'Strong absorption, freq-dependent' },
  HUMAN:   { name: 'Human',   char: '@', desc: '60-70% water, strong freq-dependent' },
  WOOD:    { name: 'Wood',    char: '|', desc: 'Mild attenuation, freq-increasing' },
  GLASS:   { name: 'Glass',   char: ':', desc: 'Low attenuation, frequency-flat' },
  AIR:     { name: 'Air',     char: '.', desc: 'Minimal attenuation' },
  COMPLEX: { name: 'Complex', char: '?', desc: 'Mixed/unclassifiable' },
};

// ---------------------------------------------------------------------------
// Per-channel amplitude accumulator
// ---------------------------------------------------------------------------
class ChannelAccumulator {
  constructor() {
    // channel -> { amplitudes: Float64Array[], count: number }
    this.channels = new Map();
  }

  ingest(channel, amplitudes) {
    if (!this.channels.has(channel)) {
      this.channels.set(channel, {
        sum: new Float64Array(amplitudes.length),
        sumSq: new Float64Array(amplitudes.length),
        count: 0,
        nSub: amplitudes.length,
      });
    }

    const ch = this.channels.get(channel);
    ch.count++;
    for (let i = 0; i < amplitudes.length && i < ch.nSub; i++) {
      ch.sum[i] += amplitudes[i];
      ch.sumSq[i] += amplitudes[i] * amplitudes[i];
    }
  }

  /** Get mean amplitude per subcarrier per channel */
  getMeans() {
    const means = new Map();
    for (const [channel, ch] of this.channels) {
      if (ch.count === 0) continue;
      const mean = new Float64Array(ch.nSub);
      for (let i = 0; i < ch.nSub; i++) {
        mean[i] = ch.sum[i] / ch.count;
      }
      means.set(channel, { mean, count: ch.count, nSub: ch.nSub });
    }
    return means;
  }

  /** Get variance per subcarrier per channel */
  getVariances() {
    const variances = new Map();
    for (const [channel, ch] of this.channels) {
      if (ch.count < 2) continue;
      const variance = new Float64Array(ch.nSub);
      for (let i = 0; i < ch.nSub; i++) {
        const mean = ch.sum[i] / ch.count;
        variance[i] = (ch.sumSq[i] / ch.count) - (mean * mean);
      }
      variances.set(channel, variance);
    }
    return variances;
  }

  /** Get active channel list sorted by frequency */
  getActiveChannels() {
    return [...this.channels.keys()]
      .filter(ch => this.channels.get(ch).count > 0)
      .sort((a, b) => a - b);
  }

  reset() {
    this.channels.clear();
  }
}

// ---------------------------------------------------------------------------
// Material classifier
// ---------------------------------------------------------------------------
class MaterialClassifier {
  constructor() {
    this.accumulator = new ChannelAccumulator();
    this.frameCount = 0;
    this.classifications = [];
  }

  ingestFrame(channel, amplitudes) {
    this.accumulator.ingest(channel, amplitudes);
    this.frameCount++;
  }

  /**
   * Classify each subcarrier group by comparing attenuation across channels.
   *
   * For each subcarrier index:
   *   1. Collect mean amplitude on each channel
   *   2. Compute frequency selectivity metrics:
   *      - Flat ratio = std / mean (low = frequency-flat)
   *      - Slope = linear regression of amplitude vs frequency
   *      - Mean level = overall attenuation (high = strong absorber)
   *   3. Decision tree:
   *      - All channels null -> Metal (frequency-flat total block)
   *      - Flat ratio < 0.15 AND mean < 3.0 -> Metal
   *      - Flat ratio < 0.15 AND mean > 8.0 -> Glass/Air
   *      - Negative slope (amp decreases with freq) AND mean < 6.0 -> Water/Human
   *      - Negative slope AND mean 6.0-8.0 -> Wood
   *      - High variance across channels -> Complex
   */
  classify() {
    const means = this.accumulator.getMeans();
    const channels = this.accumulator.getActiveChannels();

    if (channels.length < 2) {
      return { error: 'Need at least 2 channels for material classification', channels: channels.length };
    }

    const nSub = Math.min(...[...means.values()].map(m => m.nSub));
    const freqs = channels.map(ch => CHANNEL_FREQ[ch] || 2432);

    const results = [];
    const materialCounts = {};
    for (const m of Object.values(MATERIAL)) materialCounts[m.name] = 0;

    for (let sc = 0; sc < nSub; sc++) {
      // Collect amplitudes across channels for this subcarrier
      const amps = channels.map(ch => means.get(ch).mean[sc]);

      // Is this a null on all channels?
      const allNull = amps.every(a => a < NULL_THRESHOLD);
      const anyNull = amps.some(a => a < NULL_THRESHOLD);

      // Mean amplitude
      const meanAmp = amps.reduce((a, b) => a + b, 0) / amps.length;

      // Standard deviation
      const variance = amps.reduce((a, b) => a + (b - meanAmp) ** 2, 0) / amps.length;
      const stdAmp = Math.sqrt(variance);

      // Flat ratio (coefficient of variation)
      const flatRatio = meanAmp > 0.01 ? stdAmp / meanAmp : 0;

      // Frequency slope: linear regression of amplitude vs frequency
      let sumF = 0, sumA = 0, sumFF = 0, sumFA = 0;
      for (let i = 0; i < channels.length; i++) {
        sumF += freqs[i];
        sumA += amps[i];
        sumFF += freqs[i] * freqs[i];
        sumFA += freqs[i] * amps[i];
      }
      const nCh = channels.length;
      const meanF = sumF / nCh;
      const denomF = sumFF - sumF * meanF;
      const slope = Math.abs(denomF) > 1e-6
        ? (sumFA - sumF * (sumA / nCh)) / denomF
        : 0;

      // Normalized slope (per MHz)
      const slopePerMHz = slope;

      // Classification decision tree
      let material;
      if (allNull) {
        material = MATERIAL.METAL;
      } else if (flatRatio < 0.15 && meanAmp < 3.0) {
        material = MATERIAL.METAL;
      } else if (flatRatio < 0.15 && meanAmp > 10.0) {
        material = MATERIAL.AIR;
      } else if (flatRatio < 0.15 && meanAmp > 6.0) {
        material = MATERIAL.GLASS;
      } else if (slopePerMHz < -0.005 && meanAmp < 5.0) {
        // Amplitude decreases with frequency = frequency-dependent absorption
        material = MATERIAL.HUMAN;
      } else if (slopePerMHz < -0.003 && meanAmp < 8.0) {
        material = MATERIAL.WATER;
      } else if (slopePerMHz < -0.001 && meanAmp >= 5.0) {
        material = MATERIAL.WOOD;
      } else if (flatRatio > 0.5) {
        material = MATERIAL.COMPLEX;
      } else {
        material = MATERIAL.AIR;
      }

      materialCounts[material.name]++;
      results.push({
        subcarrier: sc,
        material: material.name,
        char: material.char,
        meanAmp: meanAmp.toFixed(1),
        flatRatio: flatRatio.toFixed(3),
        slopePerMHz: slopePerMHz.toFixed(5),
        amps: amps.map(a => a.toFixed(1)),
      });
    }

    this.classifications = results;

    return {
      channels,
      nSubcarriers: nSub,
      frameCount: this.frameCount,
      materialCounts,
      classifications: results,
    };
  }

  reset() {
    this.accumulator.reset();
    this.frameCount = 0;
    this.classifications = [];
  }
}

// ---------------------------------------------------------------------------
// CSI parsing
// ---------------------------------------------------------------------------
function parseIqHex(iqHex, nSubcarriers) {
  const bytes = Buffer.from(iqHex, 'hex');
  const amplitudes = new Float64Array(nSubcarriers);

  for (let sc = 0; sc < nSubcarriers; sc++) {
    const offset = 2 + sc * 2;
    if (offset + 1 >= bytes.length) break;
    let I = bytes[offset];
    let Q = bytes[offset + 1];
    if (I > 127) I -= 256;
    if (Q > 127) Q -= 256;
    amplitudes[sc] = Math.sqrt(I * I + Q * Q);
  }

  return amplitudes;
}

function parseCSIFrame(buf) {
  return parseRawCsiFrame(buf, { includePhase: false });
}

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
function renderMaterialMap(result) {
  const { classifications, channels, nSubcarriers, materialCounts } = result;
  if (!classifications || classifications.length === 0) return '  No classifications available';

  const lines = [];
  lines.push('');
  lines.push('  FREQUENCY-SELECTIVE MATERIAL CLASSIFICATION');
  lines.push('  ' + '='.repeat(55));
  lines.push('');

  // Material map: one char per subcarrier
  lines.push('  Subcarrier Material Map (1 char = 1 subcarrier):');
  let mapRow = '  ';
  for (let i = 0; i < classifications.length; i++) {
    mapRow += classifications[i].char;
    if ((i + 1) % 64 === 0) {
      lines.push(mapRow);
      mapRow = '  ';
    }
  }
  if (mapRow.trim()) lines.push(mapRow);

  lines.push('');
  lines.push('  Legend:');
  for (const m of Object.values(MATERIAL)) {
    const count = materialCounts[m.name] || 0;
    const pct = nSubcarriers > 0 ? (count / nSubcarriers * 100).toFixed(1) : '0.0';
    lines.push(`    ${m.char} = ${m.name.padEnd(8)} (${pct}%) ${m.desc}`);
  }

  return lines.join('\n');
}

function renderFrequencyProfile(result) {
  const { classifications, channels } = result;
  if (!classifications || channels.length < 2) return '';

  const lines = [];
  lines.push('');
  lines.push('  Frequency Profile (mean amplitude per channel):');
  lines.push('  ' + '-'.repeat(50));

  // Compute mean per channel across all subcarriers
  const channelMeans = {};
  for (const ch of channels) channelMeans[ch] = { sum: 0, count: 0 };

  for (const cls of classifications) {
    for (let i = 0; i < channels.length && i < cls.amps.length; i++) {
      channelMeans[channels[i]].sum += parseFloat(cls.amps[i]);
      channelMeans[channels[i]].count++;
    }
  }

  const BARS = '\u2581\u2582\u2583\u2584\u2585\u2586\u2587\u2588';
  let maxMean = 0;
  for (const ch of channels) {
    const m = channelMeans[ch].count > 0 ? channelMeans[ch].sum / channelMeans[ch].count : 0;
    if (m > maxMean) maxMean = m;
  }
  if (maxMean === 0) maxMean = 1;

  for (const ch of channels) {
    const mean = channelMeans[ch].count > 0 ? channelMeans[ch].sum / channelMeans[ch].count : 0;
    const freq = CHANNEL_FREQ[ch] || 0;
    const barLen = Math.floor((mean / maxMean) * 30);
    const bar = BARS[7].repeat(barLen);
    lines.push(`  ch${String(ch).padStart(2)} (${freq} MHz): ${bar} ${mean.toFixed(1)}`);
  }

  // Slope analysis
  const freqs = channels.map(ch => CHANNEL_FREQ[ch]);
  const means = channels.map(ch => {
    const c = channelMeans[ch];
    return c.count > 0 ? c.sum / c.count : 0;
  });

  let sumF = 0, sumA = 0, sumFF = 0, sumFA = 0;
  for (let i = 0; i < channels.length; i++) {
    sumF += freqs[i]; sumA += means[i];
    sumFF += freqs[i] * freqs[i]; sumFA += freqs[i] * means[i];
  }
  const nCh = channels.length;
  const meanF = sumF / nCh;
  const denomF = sumFF - sumF * meanF;
  const slope = Math.abs(denomF) > 1e-6 ? (sumFA - sumF * (sumA / nCh)) / denomF : 0;

  lines.push('');
  if (slope < -0.003) {
    lines.push('  Overall trend: DECREASING with frequency (water/organic absorption)');
  } else if (slope > 0.003) {
    lines.push('  Overall trend: INCREASING with frequency (unusual, possible reflection)');
  } else {
    lines.push('  Overall trend: FLAT across frequency (metal or air dominant)');
  }
  lines.push(`  Slope: ${(slope * 1000).toFixed(3)} amplitude/GHz`);

  return lines.join('\n');
}

function renderDetailedSubcarriers(result) {
  const { classifications, channels } = result;
  if (!classifications) return '';

  const lines = [];
  lines.push('');
  lines.push('  Notable Subcarriers (high frequency selectivity):');
  lines.push('  ' + '-'.repeat(60));
  lines.push('  SC#  Material  Mean   Flat   Slope/MHz  Per-channel amps');

  // Find most interesting subcarriers (high flat ratio or steep slope)
  const interesting = classifications
    .filter(c => parseFloat(c.flatRatio) > 0.3 || Math.abs(parseFloat(c.slopePerMHz)) > 0.005)
    .sort((a, b) => parseFloat(b.flatRatio) - parseFloat(a.flatRatio))
    .slice(0, 15);

  for (const cls of interesting) {
    const amps = cls.amps.join(' ');
    lines.push(`  ${String(cls.subcarrier).padStart(3)}  ${cls.material.padEnd(8)}  ` +
      `${cls.meanAmp.padStart(5)}  ${cls.flatRatio}  ${cls.slopePerMHz.padStart(9)}  [${amps}]`);
  }

  if (interesting.length === 0) {
    lines.push('  (no highly frequency-selective subcarriers detected)');
  }

  return lines.join('\n');
}

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------
const classifier = new MaterialClassifier();
let lastDisplayMs = 0;

function processFrame(channel, amplitudes) {
  classifier.ingestFrame(channel, amplitudes);
}

function displayUpdate() {
  const result = classifier.classify();

  if (JSON_OUTPUT) {
    console.log(JSON.stringify({
      timestamp: Date.now() / 1000,
      channels: result.channels,
      frameCount: result.frameCount,
      materialCounts: result.materialCounts,
      topClassifications: (result.classifications || [])
        .filter(c => c.material !== 'Air')
        .slice(0, 20)
        .map(c => ({ sc: c.subcarrier, material: c.material, meanAmp: c.meanAmp })),
    }));
  } else {
    process.stdout.write('\x1B[2J\x1B[H');
    console.log(renderMaterialMap(result));
    console.log(renderFrequencyProfile(result));
    console.log(renderDetailedSubcarriers(result));
    console.log('');
    console.log(`  Frames: ${result.frameCount} | Channels: ${(result.channels || []).length}`);
    console.log('  Press Ctrl+C to exit');
  }
}

// ---------------------------------------------------------------------------
// Live mode
// ---------------------------------------------------------------------------
function startLive() {
  const sock = dgram.createSocket('udp4');

  sock.on('message', (buf) => {
    const frame = parseCSIFrame(buf);
    if (!frame) return;

    processFrame(frame.channel, frame.amplitudes);

    const now = Date.now();
    if (now - lastDisplayMs >= INTERVAL_MS) {
      displayUpdate();
      lastDisplayMs = now;
    }
  });

  sock.bind(PORT, () => {
    if (!JSON_OUTPUT) {
      console.log(`Material Classifier listening on UDP port ${PORT}`);
      console.log('Waiting for multi-channel CSI frames...');
    }
  });

  if (DURATION_MS) {
    setTimeout(() => { displayUpdate(); sock.close(); process.exit(0); }, DURATION_MS);
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

    const amplitudes = parseIqHex(record.iq_hex, record.subcarriers || 64);
    const channel = record.channel || assignChannel(record.node_id);

    processFrame(channel, amplitudes);
    frameCount++;

    const tsMs = record.timestamp * 1000;
    if (lastAnalysisTs === 0) lastAnalysisTs = tsMs;

    if (tsMs - lastAnalysisTs >= INTERVAL_MS) {
      windowCount++;
      const result = classifier.classify();

      if (JSON_OUTPUT) {
        console.log(JSON.stringify({
          window: windowCount, timestamp: record.timestamp,
          materialCounts: result.materialCounts,
        }));
      } else {
        console.log(`\n${'='.repeat(60)}`);
        console.log(`Window ${windowCount} | t=${record.timestamp.toFixed(1)}s | frames=${frameCount}`);
        console.log('='.repeat(60));
        console.log(renderMaterialMap(result));
        console.log(renderFrequencyProfile(result));
      }
      lastAnalysisTs = tsMs;
    }
  }

  // Final
  if (!JSON_OUTPUT) {
    const result = classifier.classify();
    console.log(`\n${'='.repeat(60)}`);
    console.log('FINAL MATERIAL CLASSIFICATION');
    console.log('='.repeat(60));
    console.log(renderMaterialMap(result));
    console.log(renderFrequencyProfile(result));
    console.log(renderDetailedSubcarriers(result));
    console.log(`\nProcessed ${frameCount} frames in ${windowCount} windows`);
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
