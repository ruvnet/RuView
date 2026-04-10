#!/usr/bin/env node
/**
 * RuView Multi-Frequency RF Room Scanner
 *
 * Extended version of rf-scan.js that tracks CSI data per WiFi channel and
 * merges multi-channel data into a wideband view. Works when channel hopping
 * is enabled on ESP32 nodes via provision.py --hop-channels.
 *
 * Key capabilities:
 *   - Per-channel subcarrier tracking across 6 WiFi channels
 *   - Wideband merged spectrum (up to 6x subcarrier count)
 *   - Null diversity analysis (what one channel misses, another may see)
 *   - Frequency-dependent scattering identification
 *   - Neighbor network illuminator tracking
 *   - Per-channel penetration quality scoring
 *
 * Usage:
 *   node scripts/rf-scan-multifreq.js
 *   node scripts/rf-scan-multifreq.js --port 5006 --duration 60
 *   node scripts/rf-scan-multifreq.js --json
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
// Constants
// ---------------------------------------------------------------------------
const VITALS_MAGIC  = 0xC5110002;
const FEATURE_MAGIC = 0xC5110003;
const FUSED_MAGIC   = 0xC5110004;

const BARS = ['\u2581', '\u2582', '\u2583', '\u2584', '\u2585', '\u2586', '\u2587', '\u2588'];

const NULL_THRESHOLD      = 2.0;
const DYNAMIC_VAR_THRESH  = 0.15;
const STRONG_AMP_THRESH   = 0.85;

// WiFi 2.4 GHz channel -> center frequency
const CHANNEL_FREQ = {};
for (let ch = 1; ch <= 13; ch++) CHANNEL_FREQ[ch] = 2412 + (ch - 1) * 5;
CHANNEL_FREQ[14] = 2484;

// Non-overlapping channel sets for 2-node mesh
const NODE1_CHANNELS = [1, 6, 11];  // non-overlapping
const NODE2_CHANNELS = [3, 5, 9];   // interleaved, near neighbor APs

// Known neighbor networks (from WiFi scan, used as illuminators)
const KNOWN_ILLUMINATORS = [
  { ssid: 'ruv.net',                    channel: 5,  freq: 2432, signal: 100 },
  { ssid: 'Cohen-Guest',                channel: 5,  freq: 2432, signal: 100 },
  { ssid: 'COGECO-21B20',               channel: 11, freq: 2462, signal: 100 },
  { ssid: 'DIRECT-fa-HP M255 LaserJet', channel: 5,  freq: 2432, signal: 94  },
  { ssid: 'conclusion mesh',            channel: 3,  freq: 2422, signal: 44  },
  { ssid: 'NETGEAR72',                  channel: 9,  freq: 2452, signal: 42  },
  { ssid: 'NETGEAR72-Guest',            channel: 9,  freq: 2452, signal: 42  },
  { ssid: 'COGECO-4321',                channel: 11, freq: 2462, signal: 30  },
  { ssid: 'Innanen',                    channel: 6,  freq: 2437, signal: 19  },
];

// ---------------------------------------------------------------------------
// Per-channel state within a node
// ---------------------------------------------------------------------------
class ChannelState {
  constructor(channel) {
    this.channel = channel;
    this.freqMhz = CHANNEL_FREQ[channel] || 0;
    this.nSubcarriers = 0;
    this.frameCount = 0;
    this.firstFrameMs = 0;
    this.lastFrameMs = 0;

    this.amplitudes = new Float64Array(256);
    this.phases = new Float64Array(256);

    // Welford variance per subcarrier
    this.ampMean  = new Float64Array(256);
    this.ampM2    = new Float64Array(256);
    this.ampCount = new Uint32Array(256);

    // Illuminators active on this channel
    this.illuminators = KNOWN_ILLUMINATORS.filter(n => n.channel === channel);
  }

  get fps() {
    if (this.firstFrameMs === 0) return 0;
    const elapsed = (this.lastFrameMs - this.firstFrameMs) / 1000;
    return elapsed > 0 ? this.frameCount / elapsed : 0;
  }

  update(amplitudes, phases) {
    const n = amplitudes.length;
    this.nSubcarriers = n;
    this.frameCount++;
    const now = Date.now();
    if (this.firstFrameMs === 0) this.firstFrameMs = now;
    this.lastFrameMs = now;

    for (let i = 0; i < n; i++) {
      this.amplitudes[i] = amplitudes[i];
      this.phases[i] = phases[i];

      this.ampCount[i]++;
      const delta = amplitudes[i] - this.ampMean[i];
      this.ampMean[i] += delta / this.ampCount[i];
      const delta2 = amplitudes[i] - this.ampMean[i];
      this.ampM2[i] += delta * delta2;
    }
  }

  getVariance(i) {
    return this.ampCount[i] > 1 ? this.ampM2[i] / (this.ampCount[i] - 1) : 0;
  }

  getNulls() {
    const nulls = [];
    for (let i = 0; i < this.nSubcarriers; i++) {
      if (this.amplitudes[i] < NULL_THRESHOLD) nulls.push(i);
    }
    return nulls;
  }

  getNullPercent() {
    if (this.nSubcarriers === 0) return 0;
    return (this.getNulls().length / this.nSubcarriers) * 100;
  }

  classify() {
    const n = this.nSubcarriers;
    if (n === 0) return { nulls: [], dynamic: [], reflectors: [], walls: [] };

    let maxAmp = 0;
    for (let i = 0; i < n; i++) {
      if (this.amplitudes[i] > maxAmp) maxAmp = this.amplitudes[i];
    }
    if (maxAmp === 0) maxAmp = 1;

    const nulls = [], dynamic = [], reflectors = [], walls = [];
    for (let i = 0; i < n; i++) {
      const normAmp = this.amplitudes[i] / maxAmp;
      const variance = this.getVariance(i);

      if (this.amplitudes[i] < NULL_THRESHOLD) nulls.push(i);
      else if (variance > DYNAMIC_VAR_THRESH) dynamic.push(i);
      else if (normAmp > STRONG_AMP_THRESH) reflectors.push(i);
      else walls.push(i);
    }

    return { nulls, dynamic, reflectors, walls };
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
// Per-node state (multi-channel)
// ---------------------------------------------------------------------------
class NodeState {
  constructor(nodeId) {
    this.nodeId = nodeId;
    this.address = null;
    this.channels = new Map();  // channel number -> ChannelState
    this.totalFrames = 0;
    this.firstFrameMs = Date.now();
    this.lastFrameMs = Date.now();
    this.rssi = 0;
    this.vitals = null;
    this.features = null;
  }

  get fps() {
    const elapsed = (this.lastFrameMs - this.firstFrameMs) / 1000;
    return elapsed > 0 ? this.totalFrames / elapsed : 0;
  }

  getOrCreateChannel(channel) {
    if (!this.channels.has(channel)) {
      this.channels.set(channel, new ChannelState(channel));
    }
    return this.channels.get(channel);
  }

  getActiveChannels() {
    return [...this.channels.values()]
      .filter(cs => cs.frameCount > 0)
      .sort((a, b) => a.channel - b.channel);
  }
}

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------
const nodes = new Map();
const startTime = Date.now();
let totalFrames = 0;

// ---------------------------------------------------------------------------
// Packet parsing (same as rf-scan.js)
// ---------------------------------------------------------------------------
function parseCSIFrame(buf) {
  return parseRawCsiFrame(buf);
}

function parseVitalsPacket(buf) {
  if (buf.length < 32) return null;
  const magic = buf.readUInt32LE(0);
  if (magic !== VITALS_MAGIC && magic !== FUSED_MAGIC) return null;

  return {
    nodeId:        buf.readUInt8(4),
    flags:         buf.readUInt8(5),
    presence:      !!(buf.readUInt8(5) & 0x01),
    fall:          !!(buf.readUInt8(5) & 0x02),
    motion:        !!(buf.readUInt8(5) & 0x04),
    breathingRate: buf.readUInt16LE(6) / 100,
    heartrate:     buf.readUInt32LE(8) / 10000,
    rssi:          buf.readInt8(12),
    nPersons:      buf.readUInt8(13),
    motionEnergy:  buf.readFloatLE(16),
    presenceScore: buf.readFloatLE(20),
    timestampMs:   buf.readUInt32LE(24),
  };
}

function parseFeaturePacket(buf) {
  if (buf.length < 48) return null;
  const magic = buf.readUInt32LE(0);
  if (magic !== FEATURE_MAGIC) return null;

  const features = [];
  for (let i = 0; i < 8; i++) features.push(buf.readFloatLE(12 + i * 4));
  return { nodeId: buf.readUInt8(4), seq: buf.readUInt16LE(6), features };
}

function handlePacket(buf, rinfo) {
  if (buf.length < 4) return;
  const frame = parseCSIFrame(buf);
  if (frame) {
    totalFrames++;
    let node = nodes.get(frame.nodeId);
    if (!node) {
      node = new NodeState(frame.nodeId);
      nodes.set(frame.nodeId, node);
    }

    node.address = rinfo.address;
    node.rssi = frame.rssi;
    node.totalFrames++;
    node.lastFrameMs = Date.now();

    const cs = node.getOrCreateChannel(frame.channel);
    cs.update(frame.amplitudes, frame.phases);
    return;
  }

  const magic = buf.readUInt32LE(0);

  if (magic === VITALS_MAGIC || magic === FUSED_MAGIC) {
    const vitals = parseVitalsPacket(buf);
    if (!vitals) return;
    let node = nodes.get(vitals.nodeId);
    if (!node) { node = new NodeState(vitals.nodeId); nodes.set(vitals.nodeId, node); }
    node.vitals = vitals;
    return;
  }

  if (magic === FEATURE_MAGIC) {
    const feat = parseFeaturePacket(buf);
    if (!feat) return;
    let node = nodes.get(feat.nodeId);
    if (!node) { node = new NodeState(feat.nodeId); nodes.set(feat.nodeId, node); }
    node.features = feat;
  }
}

// ---------------------------------------------------------------------------
// Multi-frequency analysis
// ---------------------------------------------------------------------------

/**
 * Compute null diversity: how many null subcarriers on one channel are
 * resolved (non-null) on another channel. This is the core benefit of
 * multi-frequency scanning.
 */
function computeNullDiversity() {
  // Collect all channel states across all nodes
  const allChannelStates = [];
  for (const node of nodes.values()) {
    for (const cs of node.channels.values()) {
      if (cs.frameCount > 0) allChannelStates.push(cs);
    }
  }

  if (allChannelStates.length < 2) return null;

  // For each channel, get its null set
  const channelNulls = new Map();
  for (const cs of allChannelStates) {
    const key = cs.channel;
    if (!channelNulls.has(key)) {
      channelNulls.set(key, { channel: key, nulls: new Set(cs.getNulls()), nSub: cs.nSubcarriers });
    }
  }

  if (channelNulls.size < 2) return null;

  const channels = [...channelNulls.keys()].sort((a, b) => a - b);

  // Compute pairwise null diversity
  const pairwise = [];
  for (let i = 0; i < channels.length; i++) {
    for (let j = i + 1; j < channels.length; j++) {
      const c1 = channelNulls.get(channels[i]);
      const c2 = channelNulls.get(channels[j]);

      // Nulls on c1 that c2 resolves (non-null on c2)
      let c1ResolvedByC2 = 0;
      let c2ResolvedByC1 = 0;
      let sharedNulls = 0;

      for (const idx of c1.nulls) {
        if (!c2.nulls.has(idx)) c1ResolvedByC2++;
        else sharedNulls++;
      }
      for (const idx of c2.nulls) {
        if (!c1.nulls.has(idx)) c2ResolvedByC1++;
      }

      pairwise.push({
        ch1: channels[i], ch2: channels[j],
        ch1Nulls: c1.nulls.size, ch2Nulls: c2.nulls.size,
        sharedNulls,
        ch1ResolvedByC2: c1ResolvedByC2,
        ch2ResolvedByC1: c2ResolvedByC1,
      });
    }
  }

  // Global: union of all nulls vs intersection
  const allNullSets = [...channelNulls.values()].map(c => c.nulls);
  const unionNulls = new Set();
  for (const s of allNullSets) for (const idx of s) unionNulls.add(idx);

  let intersectionCount = 0;
  for (const idx of unionNulls) {
    if (allNullSets.every(s => s.has(idx))) intersectionCount++;
  }

  // Effective null rate after multi-channel fusion
  const maxSub = Math.max(...[...channelNulls.values()].map(c => c.nSub));
  const singleChannelNulls = allNullSets[0].size;
  const fusedNulls = intersectionCount;  // only nulls present on ALL channels

  return {
    channels,
    pairwise,
    singleChannelNulls,
    fusedNulls,
    unionNulls: unionNulls.size,
    maxSubcarriers: maxSub,
    singleNullPct: maxSub > 0 ? ((singleChannelNulls / maxSub) * 100).toFixed(1) : '0',
    fusedNullPct: maxSub > 0 ? ((fusedNulls / maxSub) * 100).toFixed(1) : '0',
    diversityGain: singleChannelNulls > 0
      ? ((1 - fusedNulls / singleChannelNulls) * 100).toFixed(1)
      : '0',
  };
}

/**
 * Find objects visible on some channels but not others.
 * These are frequency-dependent scatterers (interesting for material classification).
 */
function findFrequencyDependentObjects() {
  const allChannelStates = [];
  for (const node of nodes.values()) {
    for (const cs of node.channels.values()) {
      if (cs.frameCount > 0 && cs.nSubcarriers > 0) allChannelStates.push(cs);
    }
  }

  if (allChannelStates.length < 2) return [];

  const results = [];
  const nSub = Math.min(...allChannelStates.map(cs => cs.nSubcarriers));

  for (let i = 0; i < nSub; i++) {
    const amps = allChannelStates.map(cs => cs.amplitudes[i]);
    const vars = allChannelStates.map(cs => cs.getVariance(i));
    const maxAmp = Math.max(...amps);
    const minAmp = Math.min(...amps);

    // Large amplitude spread across channels = frequency-dependent scatterer
    if (maxAmp > 0 && (maxAmp - minAmp) / maxAmp > 0.5) {
      const bestCh = allChannelStates[amps.indexOf(maxAmp)].channel;
      const worstCh = allChannelStates[amps.indexOf(minAmp)].channel;
      results.push({
        subcarrier: i,
        maxAmp: maxAmp.toFixed(1),
        minAmp: minAmp.toFixed(1),
        bestChannel: bestCh,
        worstChannel: worstCh,
        spread: ((maxAmp - minAmp) / maxAmp * 100).toFixed(0),
      });
    }
  }

  return results.slice(0, 20);  // top 20
}

/**
 * Compute per-channel penetration quality score.
 * Lower frequency channels (ch 1 = 2412 MHz) have slightly longer wavelength
 * and better penetration through some materials.
 */
function computePenetrationScores() {
  const scores = [];

  for (const node of nodes.values()) {
    for (const cs of node.channels.values()) {
      if (cs.frameCount === 0 || cs.nSubcarriers === 0) continue;

      // Mean amplitude (higher = better penetration)
      let sumAmp = 0;
      for (let i = 0; i < cs.nSubcarriers; i++) sumAmp += cs.amplitudes[i];
      const meanAmp = sumAmp / cs.nSubcarriers;

      // Null rate (lower = better)
      const nullPct = cs.getNullPercent();

      // Spectrum flatness = geometric mean / arithmetic mean
      // Flatter spectrum = more uniform penetration
      let logSum = 0;
      let count = 0;
      for (let i = 0; i < cs.nSubcarriers; i++) {
        if (cs.amplitudes[i] > 0) {
          logSum += Math.log(cs.amplitudes[i]);
          count++;
        }
      }
      const geoMean = count > 0 ? Math.exp(logSum / count) : 0;
      const flatness = sumAmp > 0 ? geoMean / meanAmp : 0;

      // Quality score: weighted combination
      const quality = (meanAmp / 20) * 0.4 + (1 - nullPct / 100) * 0.3 + flatness * 0.3;

      scores.push({
        nodeId: node.nodeId,
        channel: cs.channel,
        freqMhz: cs.freqMhz,
        fps: cs.fps.toFixed(1),
        meanAmp: meanAmp.toFixed(1),
        nullPct: nullPct.toFixed(1),
        flatness: flatness.toFixed(3),
        quality: quality.toFixed(3),
        illuminators: cs.illuminators.map(il => il.ssid),
      });
    }
  }

  return scores.sort((a, b) => parseFloat(b.quality) - parseFloat(a.quality));
}

// ---------------------------------------------------------------------------
// Wideband merged view
// ---------------------------------------------------------------------------
function buildWidebandSpectrum() {
  // Collect all channel amplitudes into one wide view
  const allChannels = [];
  for (const node of nodes.values()) {
    for (const cs of node.getActiveChannels()) {
      allChannels.push(cs);
    }
  }

  if (allChannels.length === 0) return { bar: '', channels: 0, totalSubcarriers: 0 };

  // Sort by frequency
  allChannels.sort((a, b) => a.freqMhz - b.freqMhz);

  let totalSub = 0;
  for (const cs of allChannels) totalSub += cs.nSubcarriers;

  // Find global max amplitude for normalization
  let globalMax = 0;
  for (const cs of allChannels) {
    for (let i = 0; i < cs.nSubcarriers; i++) {
      if (cs.amplitudes[i] > globalMax) globalMax = cs.amplitudes[i];
    }
  }
  if (globalMax === 0) globalMax = 1;

  // Build wideband bar with channel separators
  let bar = '';
  let labels = '';
  for (let c = 0; c < allChannels.length; c++) {
    const cs = allChannels[c];
    if (c > 0) {
      bar += '|';
      labels += '|';
    }

    const chLabel = `ch${cs.channel}`;
    labels += chLabel + ' '.repeat(Math.max(0, cs.nSubcarriers - chLabel.length));

    for (let i = 0; i < cs.nSubcarriers; i++) {
      const level = Math.floor((cs.amplitudes[i] / globalMax) * 7.99);
      bar += BARS[Math.max(0, Math.min(7, level))];
    }
  }

  return { bar, labels, channels: allChannels.length, totalSubcarriers: totalSub };
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
  const nodeList = [...nodes.values()];
  const activeNodes = nodeList.filter(n => n.totalFrames > 0);

  if (activeNodes.length === 0) {
    lines.push(`=== RUVIEW MULTI-FREQ RF SCAN === Listening on UDP :${PORT}`);
    lines.push('Waiting for CSI frames from ESP32 nodes...');
    lines.push('Enable channel hopping: python provision.py --port COMx --hop-channels 1,6,11');
    lines.push(`Elapsed: ${((Date.now() - startTime) / 1000).toFixed(0)}s | Frames: ${totalFrames}`);
    return lines.join('\n');
  }

  lines.push('=== RUVIEW MULTI-FREQUENCY RF SCAN ===');
  lines.push('');

  // Per-node, per-channel view
  for (const node of activeNodes) {
    lines.push(`--- Node ${node.nodeId} (${node.address || '?'}) | ${node.fps.toFixed(1)} fps total | RSSI ${node.rssi} dBm ---`);

    const activeChannels = node.getActiveChannels();
    if (activeChannels.length === 0) {
      lines.push('  (no channel data yet)');
      continue;
    }

    for (const cs of activeChannels) {
      const cls = cs.classify();
      const spectrum = cs.getSpectrumBar();
      const nullPct = cs.getNullPercent().toFixed(0);
      const ilNames = cs.illuminators.length > 0
        ? cs.illuminators.map(il => il.ssid).join(', ')
        : 'none';

      lines.push(`  Ch ${String(cs.channel).padStart(2)} (${cs.freqMhz} MHz) | ${cs.fps.toFixed(1)} fps | nulls: ${nullPct}% | illuminators: ${ilNames}`);
      if (spectrum.length > 0) {
        // Truncate spectrum to terminal width (approx)
        const maxWidth = 80;
        const truncated = spectrum.length > maxWidth
          ? spectrum.slice(0, maxWidth) + '...'
          : spectrum;
        lines.push(`    ${truncated}`);
      }
      lines.push(`    ${cls.nulls.length} null | ${cls.dynamic.length} dynamic | ${cls.reflectors.length} reflector | ${cls.walls.length} static`);
    }

    // Vitals
    if (node.vitals) {
      const v = node.vitals;
      lines.push(`  Vitals: BR ${v.breathingRate.toFixed(0)} BPM | HR ${v.heartrate.toFixed(0)} BPM | presence ${v.presenceScore.toFixed(2)} | ${v.nPersons} person(s)`);
    }

    lines.push('');
  }

  // Wideband merged view
  const wideband = buildWidebandSpectrum();
  if (wideband.channels > 1) {
    lines.push('--- Wideband Merged Spectrum ---');
    const maxWidth = 100;
    const truncBar = wideband.bar.length > maxWidth
      ? wideband.bar.slice(0, maxWidth) + '...'
      : wideband.bar;
    lines.push(`  ${truncBar}`);
    lines.push(`  ${wideband.channels} channels | ${wideband.totalSubcarriers} total subcarriers`);
    lines.push('');
  }

  // Null diversity analysis
  const diversity = computeNullDiversity();
  if (diversity) {
    lines.push('--- Null Diversity Analysis ---');
    lines.push(`  Single-channel nulls: ${diversity.singleChannelNulls} (${diversity.singleNullPct}%)`);
    lines.push(`  Multi-channel fused:  ${diversity.fusedNulls} (${diversity.fusedNullPct}%) -- only nulls on ALL channels`);
    lines.push(`  Diversity gain:       ${diversity.diversityGain}% of nulls resolved by other channels`);

    if (diversity.pairwise.length > 0) {
      lines.push('  Pairwise:');
      for (const p of diversity.pairwise) {
        lines.push(`    ch${p.ch1}<->ch${p.ch2}: ${p.sharedNulls} shared | ch${p.ch1} resolves ${p.ch2ResolvedByC1} of ch${p.ch2}'s nulls | ch${p.ch2} resolves ${p.ch1ResolvedByC2} of ch${p.ch1}'s nulls`);
      }
    }
    lines.push('');
  }

  // Penetration scores
  const penScores = computePenetrationScores();
  if (penScores.length > 0) {
    lines.push('--- Per-Channel Penetration Quality ---');
    lines.push('  Ch   Freq     FPS   MeanAmp  Null%  Flat   Quality  Illuminators');
    for (const s of penScores) {
      const ilStr = s.illuminators.length > 0 ? s.illuminators.slice(0, 2).join(', ') : '-';
      lines.push(`  ${String(s.channel).padStart(2)}   ${s.freqMhz} MHz  ${String(s.fps).padStart(5)}  ${String(s.meanAmp).padStart(7)}  ${String(s.nullPct).padStart(5)}  ${s.flatness}  ${s.quality}    ${ilStr}`);
    }
    lines.push('');
  }

  // Frequency-dependent scatterers
  const scatterers = findFrequencyDependentObjects();
  if (scatterers.length > 0) {
    lines.push(`--- Frequency-Dependent Scatterers (${scatterers.length} found) ---`);
    lines.push('  Sub#  Best Ch  Worst Ch  Spread  MaxAmp  MinAmp');
    for (const s of scatterers.slice(0, 10)) {
      lines.push(`  ${String(s.subcarrier).padStart(4)}  ch${String(s.bestChannel).padStart(2)}     ch${String(s.worstChannel).padStart(2)}      ${String(s.spread).padStart(3)}%    ${String(s.maxAmp).padStart(6)}  ${String(s.minAmp).padStart(6)}`);
    }
    lines.push('  (Objects visible on some frequencies but not others -- different materials)');
    lines.push('');
  }

  // Summary
  const elapsed = ((Date.now() - startTime) / 1000).toFixed(0);
  lines.push(`Elapsed: ${elapsed}s | Total frames: ${totalFrames} | Nodes: ${activeNodes.length}`);
  if (DURATION_MS) {
    const remaining = Math.max(0, (DURATION_MS - (Date.now() - startTime)) / 1000).toFixed(0);
    lines.push(`Remaining: ${remaining}s`);
  }

  return lines.join('\n');
}

function buildJsonOutput() {
  const activeNodes = [...nodes.values()].filter(n => n.totalFrames > 0);

  return {
    timestamp: new Date().toISOString(),
    elapsedMs: Date.now() - startTime,
    totalFrames,
    nodes: activeNodes.map(node => ({
      nodeId: node.nodeId,
      address: node.address,
      fps: parseFloat(node.fps.toFixed(2)),
      totalFrames: node.totalFrames,
      channels: node.getActiveChannels().map(cs => {
        const cls = cs.classify();
        return {
          channel: cs.channel,
          freqMhz: cs.freqMhz,
          fps: parseFloat(cs.fps.toFixed(2)),
          nSubcarriers: cs.nSubcarriers,
          frameCount: cs.frameCount,
          classification: {
            nullCount: cls.nulls.length,
            dynamicCount: cls.dynamic.length,
            reflectorCount: cls.reflectors.length,
            staticCount: cls.walls.length,
            nullPercent: parseFloat(cs.getNullPercent().toFixed(1)),
          },
          illuminators: cs.illuminators.map(il => il.ssid),
          amplitudes: Array.from(cs.amplitudes.subarray(0, cs.nSubcarriers)),
          phases: Array.from(cs.phases.subarray(0, cs.nSubcarriers)),
        };
      }),
      vitals: node.vitals,
      features: node.features ? node.features.features : null,
    })),
    nullDiversity: computeNullDiversity(),
    penetrationScores: computePenetrationScores(),
    frequencyDependentScatterers: findFrequencyDependentObjects(),
    wideband: (() => {
      const wb = buildWidebandSpectrum();
      return { channels: wb.channels, totalSubcarriers: wb.totalSubcarriers };
    })(),
  };
}

function display() {
  if (JSON_OUTPUT) {
    process.stdout.write(JSON.stringify(buildJsonOutput()) + '\n');
  } else {
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
      console.log(`RuView Multi-Frequency RF Scanner listening on ${addr.address}:${addr.port}`);
      console.log('Waiting for CSI frames from ESP32 nodes...');
      console.log('Tip: Enable channel hopping with provision.py --hop-channels 1,6,11\n');
    }
  });

  server.bind(PORT);

  const displayTimer = setInterval(display, INTERVAL_MS);

  if (DURATION_MS) {
    setTimeout(() => {
      clearInterval(displayTimer);

      if (JSON_OUTPUT) {
        const summary = buildJsonOutput();
        summary.final = true;
        process.stdout.write(JSON.stringify(summary) + '\n');
      } else {
        display();
        console.log('\n--- Multi-frequency scan complete ---');

        const diversity = computeNullDiversity();
        if (diversity) {
          console.log(`Null diversity gain: ${diversity.diversityGain}% (${diversity.singleNullPct}% -> ${diversity.fusedNullPct}%)`);
        }

        console.log(`Total frames: ${totalFrames}`);
        console.log(`Nodes: ${nodes.size}`);

        for (const node of nodes.values()) {
          const chList = node.getActiveChannels().map(cs => `ch${cs.channel}`).join(', ');
          console.log(`  Node ${node.nodeId}: ${node.totalFrames} frames, channels: [${chList}]`);
        }
      }

      server.close();
      process.exit(0);
    }, DURATION_MS);
  }

  process.on('SIGINT', () => {
    clearInterval(displayTimer);
    if (!JSON_OUTPUT) console.log('\nShutting down...');
    server.close();
    process.exit(0);
  });
}

main();
