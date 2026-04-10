#!/usr/bin/env node
/**
 * RuView RF Scan Benchmark
 *
 * Collects CSI frames from ESP32 nodes and computes quantitative metrics
 * for single-channel and multi-channel scanning performance:
 *
 *   - Frames per second per node per channel
 *   - Null subcarrier count per channel
 *   - Cross-channel null diversity (how many nulls are filled by other channels)
 *   - Subcarrier correlation across channels
 *   - Position accuracy improvement estimate
 *   - Spectrum flatness (lower = more objects)
 *
 * Usage:
 *   node scripts/benchmark-rf-scan.js --port 5006 --duration 30
 *   node scripts/benchmark-rf-scan.js --duration 60 --json
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
    duration: { type: 'string', short: 'd', default: '30' },
    json:     { type: 'boolean', default: false },
  },
  strict: true,
});

const PORT = parseInt(args.port, 10);
const DURATION_S = parseInt(args.duration, 10);
const JSON_OUTPUT = args.json;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const NULL_THRESHOLD = 2.0;

// ---------------------------------------------------------------------------
// Data collection
// ---------------------------------------------------------------------------

/**
 * Per-channel frame collector. Accumulates amplitude snapshots for analysis.
 */
class ChannelCollector {
  constructor(channel) {
    this.channel = channel;
    this.freqMhz = 0;
    this.frames = [];         // array of { amplitudes, phases, rssi, timestamp }
    this.nSubcarriers = 0;
  }

  add(amplitudes, phases, rssi, freqMhz) {
    this.freqMhz = freqMhz;
    this.nSubcarriers = amplitudes.length;
    this.frames.push({
      amplitudes: Float64Array.from(amplitudes),
      phases: Float64Array.from(phases),
      rssi,
      timestamp: Date.now(),
    });
  }
}

class NodeCollector {
  constructor(nodeId) {
    this.nodeId = nodeId;
    this.address = null;
    this.channels = new Map();  // channel -> ChannelCollector
    this.totalFrames = 0;
    this.firstFrameMs = 0;
    this.lastFrameMs = 0;
  }

  getOrCreate(channel) {
    if (!this.channels.has(channel)) {
      this.channels.set(channel, new ChannelCollector(channel));
    }
    return this.channels.get(channel);
  }
}

const nodes = new Map();
let totalFrames = 0;
const startTime = Date.now();

// ---------------------------------------------------------------------------
// Packet parsing
// ---------------------------------------------------------------------------
function parseCSIFrame(buf) {
  return parseRawCsiFrame(buf);
}

function handlePacket(buf, rinfo) {
  const frame = parseCSIFrame(buf);
  if (!frame) return;

  totalFrames++;
  let node = nodes.get(frame.nodeId);
  if (!node) {
    node = new NodeCollector(frame.nodeId);
    nodes.set(frame.nodeId, node);
  }

  node.address = rinfo.address;
  node.totalFrames++;
  const now = Date.now();
  if (node.firstFrameMs === 0) node.firstFrameMs = now;
  node.lastFrameMs = now;

  const cc = node.getOrCreate(frame.channel);
  cc.add(frame.amplitudes, frame.phases, frame.rssi, frame.freqMhz);
}

// ---------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------

function computeMetrics() {
  const results = {
    duration_s: DURATION_S,
    totalFrames,
    nodes: [],
    crossChannel: null,
    summary: null,
  };

  for (const node of nodes.values()) {
    const elapsed = (node.lastFrameMs - node.firstFrameMs) / 1000;
    const nodeFps = elapsed > 0 ? node.totalFrames / elapsed : 0;

    const channelMetrics = [];

    for (const [ch, cc] of node.channels.entries()) {
      if (cc.frames.length === 0) continue;

      const n = cc.nSubcarriers;
      const nFrames = cc.frames.length;

      // FPS for this channel
      let chFps = 0;
      if (nFrames >= 2) {
        const first = cc.frames[0].timestamp;
        const last = cc.frames[nFrames - 1].timestamp;
        const chElapsed = (last - first) / 1000;
        chFps = chElapsed > 0 ? nFrames / chElapsed : 0;
      }

      // Average null count across frames
      let totalNulls = 0;
      for (const f of cc.frames) {
        for (let i = 0; i < n; i++) {
          if (f.amplitudes[i] < NULL_THRESHOLD) totalNulls++;
        }
      }
      const avgNulls = totalNulls / nFrames;
      const nullPct = n > 0 ? (avgNulls / n) * 100 : 0;

      // Mean RSSI
      const meanRssi = cc.frames.reduce((s, f) => s + f.rssi, 0) / nFrames;

      // Spectrum flatness: geometric mean / arithmetic mean of last frame
      const lastFrame = cc.frames[nFrames - 1];
      let logSum = 0, ampSum = 0, count = 0;
      for (let i = 0; i < n; i++) {
        if (lastFrame.amplitudes[i] > 0) {
          logSum += Math.log(lastFrame.amplitudes[i]);
          count++;
        }
        ampSum += lastFrame.amplitudes[i];
      }
      const geoMean = count > 0 ? Math.exp(logSum / count) : 0;
      const ariMean = n > 0 ? ampSum / n : 0;
      const flatness = ariMean > 0 ? geoMean / ariMean : 0;

      // Amplitude variance per subcarrier (average across subcarriers)
      const means = new Float64Array(n);
      const vars = new Float64Array(n);
      for (const f of cc.frames) {
        for (let i = 0; i < n; i++) means[i] += f.amplitudes[i];
      }
      for (let i = 0; i < n; i++) means[i] /= nFrames;
      for (const f of cc.frames) {
        for (let i = 0; i < n; i++) {
          const d = f.amplitudes[i] - means[i];
          vars[i] += d * d;
        }
      }
      let avgVar = 0;
      for (let i = 0; i < n; i++) {
        vars[i] /= Math.max(1, nFrames - 1);
        avgVar += vars[i];
      }
      avgVar /= Math.max(1, n);

      // Null subcarrier indices (from last frame)
      const nullIndices = [];
      for (let i = 0; i < n; i++) {
        if (lastFrame.amplitudes[i] < NULL_THRESHOLD) nullIndices.push(i);
      }

      channelMetrics.push({
        channel: ch,
        freqMhz: cc.freqMhz,
        nSubcarriers: n,
        frameCount: nFrames,
        fps: parseFloat(chFps.toFixed(2)),
        avgNullCount: parseFloat(avgNulls.toFixed(1)),
        nullPercent: parseFloat(nullPct.toFixed(1)),
        meanRssi: parseFloat(meanRssi.toFixed(1)),
        spectrumFlatness: parseFloat(flatness.toFixed(4)),
        avgAmplitudeVariance: parseFloat(avgVar.toFixed(4)),
        nullIndices,
      });
    }

    results.nodes.push({
      nodeId: node.nodeId,
      address: node.address,
      totalFrames: node.totalFrames,
      fps: parseFloat(nodeFps.toFixed(2)),
      channels: channelMetrics,
    });
  }

  // Cross-channel null diversity
  const allChannelData = [];
  for (const node of nodes.values()) {
    for (const [ch, cc] of node.channels.entries()) {
      if (cc.frames.length === 0) continue;
      const n = cc.nSubcarriers;
      const lastFrame = cc.frames[cc.frames.length - 1];
      const nullSet = new Set();
      for (let i = 0; i < n; i++) {
        if (lastFrame.amplitudes[i] < NULL_THRESHOLD) nullSet.add(i);
      }
      allChannelData.push({ channel: ch, nodeId: node.nodeId, nullSet, n });
    }
  }

  if (allChannelData.length >= 2) {
    // Union and intersection of null sets
    const allNullSets = allChannelData.map(d => d.nullSet);
    const union = new Set();
    for (const s of allNullSets) for (const idx of s) union.add(idx);

    let intersectionCount = 0;
    for (const idx of union) {
      if (allNullSets.every(s => s.has(idx))) intersectionCount++;
    }

    const singleNulls = allNullSets[0].size;
    const maxSub = Math.max(...allChannelData.map(d => d.n));

    // Cross-channel correlation (pairwise)
    const correlations = [];
    for (let i = 0; i < allChannelData.length; i++) {
      for (let j = i + 1; j < allChannelData.length; j++) {
        const d1 = allChannelData[i];
        const d2 = allChannelData[j];
        const cc1 = [...nodes.values()].find(n => n.nodeId === d1.nodeId)?.channels.get(d1.channel);
        const cc2 = [...nodes.values()].find(n => n.nodeId === d2.nodeId)?.channels.get(d2.channel);
        if (!cc1 || !cc2) continue;

        const f1 = cc1.frames[cc1.frames.length - 1];
        const f2 = cc2.frames[cc2.frames.length - 1];
        const len = Math.min(f1.amplitudes.length, f2.amplitudes.length);

        let sumXY = 0, sumX = 0, sumY = 0, sumX2 = 0, sumY2 = 0;
        for (let k = 0; k < len; k++) {
          sumX += f1.amplitudes[k]; sumY += f2.amplitudes[k];
          sumXY += f1.amplitudes[k] * f2.amplitudes[k];
          sumX2 += f1.amplitudes[k] ** 2;
          sumY2 += f2.amplitudes[k] ** 2;
        }
        const denom = Math.sqrt((len * sumX2 - sumX * sumX) * (len * sumY2 - sumY * sumY));
        const corr = denom > 0 ? (len * sumXY - sumX * sumY) / denom : 0;

        correlations.push({
          node1: d1.nodeId, ch1: d1.channel,
          node2: d2.nodeId, ch2: d2.channel,
          correlation: parseFloat(corr.toFixed(4)),
        });
      }
    }

    results.crossChannel = {
      totalChannels: allChannelData.length,
      singleChannelNulls: singleNulls,
      fusedNulls: intersectionCount,
      unionNulls: union.size,
      maxSubcarriers: maxSub,
      singleNullPct: parseFloat(maxSub > 0 ? ((singleNulls / maxSub) * 100).toFixed(1) : '0'),
      fusedNullPct: parseFloat(maxSub > 0 ? ((intersectionCount / maxSub) * 100).toFixed(1) : '0'),
      diversityGainPct: parseFloat(singleNulls > 0
        ? ((1 - intersectionCount / singleNulls) * 100).toFixed(1)
        : '0'),
      correlations,
    };
  }

  // Position accuracy estimate
  // With N independent channel observations, accuracy improves by sqrt(N)
  // Baseline: single channel ~30 cm resolution at 2.4 GHz
  const nChannels = allChannelData.length;
  const baselineResolutionCm = 30;
  const estimatedResolutionCm = nChannels > 0
    ? baselineResolutionCm / Math.sqrt(nChannels)
    : baselineResolutionCm;

  results.summary = {
    totalNodes: nodes.size,
    totalChannels: nChannels,
    totalFrames,
    durationS: DURATION_S,
    avgFps: parseFloat((totalFrames / DURATION_S).toFixed(1)),
    baselineResolutionCm,
    estimatedResolutionCm: parseFloat(estimatedResolutionCm.toFixed(1)),
    resolutionImprovement: nChannels > 1 ? `${Math.sqrt(nChannels).toFixed(2)}x` : '1x (single channel)',
    totalSubcarriers: allChannelData.reduce((s, d) => s + d.n, 0),
    subcarrierMultiplier: nChannels > 0
      ? parseFloat((allChannelData.reduce((s, d) => s + d.n, 0) / Math.max(1, allChannelData[0]?.n || 1)).toFixed(1))
      : 1,
  };

  return results;
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

function printReport(metrics) {
  console.log('');
  console.log('=== RUVIEW RF SCAN BENCHMARK ===');
  console.log(`Duration: ${metrics.duration_s}s | Total frames: ${metrics.totalFrames}`);
  console.log('');

  // Per-node per-channel table
  console.log('--- Frames Per Second ---');
  console.log('Node  Channel  Freq       FPS   Frames  Subcarriers  RSSI');
  for (const node of metrics.nodes) {
    for (const ch of node.channels) {
      console.log(`  ${node.nodeId}    ch${String(ch.channel).padStart(2)}     ${ch.freqMhz} MHz  ${String(ch.fps).padStart(5)}  ${String(ch.frameCount).padStart(6)}  ${String(ch.nSubcarriers).padStart(11)}  ${ch.meanRssi} dBm`);
    }
    console.log(`  ${node.nodeId}    TOTAL              ${String(node.fps).padStart(5)}  ${String(node.totalFrames).padStart(6)}`);
  }
  console.log('');

  // Null subcarriers
  console.log('--- Null Subcarriers Per Channel ---');
  console.log('Node  Channel  Nulls  Null%  Flatness  AvgVariance');
  for (const node of metrics.nodes) {
    for (const ch of node.channels) {
      console.log(`  ${node.nodeId}    ch${String(ch.channel).padStart(2)}     ${String(ch.avgNullCount.toFixed(0)).padStart(5)}  ${String(ch.nullPercent.toFixed(1)).padStart(5)}%  ${String(ch.spectrumFlatness.toFixed(4)).padStart(8)}  ${ch.avgAmplitudeVariance.toFixed(4)}`);
    }
  }
  console.log('');

  // Cross-channel diversity
  if (metrics.crossChannel) {
    const cc = metrics.crossChannel;
    console.log('--- Cross-Channel Null Diversity ---');
    console.log(`  Channels scanned:    ${cc.totalChannels}`);
    console.log(`  Single-channel nulls: ${cc.singleChannelNulls} (${cc.singleNullPct}%)`);
    console.log(`  Fused nulls (all ch): ${cc.fusedNulls} (${cc.fusedNullPct}%)`);
    console.log(`  Diversity gain:       ${cc.diversityGainPct}%`);
    console.log('');

    if (cc.correlations.length > 0) {
      console.log('--- Cross-Channel Correlation ---');
      for (const c of cc.correlations) {
        const label = c.node1 === c.node2
          ? `node${c.node1} ch${c.ch1}<->ch${c.ch2}`
          : `node${c.node1}/ch${c.ch1}<->node${c.node2}/ch${c.ch2}`;
        console.log(`  ${label}: ${c.correlation.toFixed(4)}`);
      }
      console.log('');
    }
  }

  // Summary
  if (metrics.summary) {
    const s = metrics.summary;
    console.log('--- Summary ---');
    console.log(`  Nodes:                ${s.totalNodes}`);
    console.log(`  Channels:             ${s.totalChannels}`);
    console.log(`  Total subcarriers:    ${s.totalSubcarriers} (${s.subcarrierMultiplier}x single-channel)`);
    console.log(`  Average FPS:          ${s.avgFps}`);
    console.log(`  Baseline resolution:  ${s.baselineResolutionCm} cm (single channel)`);
    console.log(`  Estimated resolution: ${s.estimatedResolutionCm} cm (${s.resolutionImprovement})`);
    console.log('');
  }

  // Pass/fail targets (from ADR-073)
  console.log('--- ADR-073 Targets ---');
  const s = metrics.summary || {};
  const cc = metrics.crossChannel || {};

  const targets = [
    { name: 'Subcarrier multiplier >= 3x', pass: (s.subcarrierMultiplier || 0) >= 3,
      actual: `${s.subcarrierMultiplier || 0}x` },
    { name: 'Null gap < 5%',               pass: (cc.fusedNullPct || 100) < 5,
      actual: `${cc.fusedNullPct || '?'}%` },
    { name: 'Resolution <= 15 cm',          pass: (s.estimatedResolutionCm || 999) <= 15,
      actual: `${s.estimatedResolutionCm || '?'} cm` },
  ];

  for (const t of targets) {
    const status = t.pass ? 'PASS' : 'FAIL';
    console.log(`  [${status}] ${t.name} (actual: ${t.actual})`);
  }

  console.log('');
  console.log('Note: Targets require multi-channel hopping enabled on both ESP32 nodes.');
  console.log('Single-channel mode will show FAIL for multi-channel targets.');
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
      console.log(`RuView RF Scan Benchmark`);
      console.log(`Listening on ${addr.address}:${addr.port} for ${DURATION_S}s...`);
      console.log('Collecting CSI frames from ESP32 nodes...\n');
    }
  });

  server.bind(PORT);

  // Progress indicator (non-JSON mode)
  let progressTimer;
  if (!JSON_OUTPUT) {
    let dots = 0;
    progressTimer = setInterval(() => {
      dots++;
      const elapsed = ((Date.now() - startTime) / 1000).toFixed(0);
      process.stdout.write(`\r  ${elapsed}s / ${DURATION_S}s | ${totalFrames} frames | ${nodes.size} nodes ${'.'  .repeat(dots % 4)}   `);
    }, 1000);
  }

  setTimeout(() => {
    if (progressTimer) clearInterval(progressTimer);
    if (!JSON_OUTPUT) process.stdout.write('\r' + ' '.repeat(60) + '\r');

    const metrics = computeMetrics();

    if (JSON_OUTPUT) {
      process.stdout.write(JSON.stringify(metrics, null, 2) + '\n');
    } else {
      printReport(metrics);
    }

    server.close();
    process.exit(0);
  }, DURATION_S * 1000);

  process.on('SIGINT', () => {
    if (progressTimer) clearInterval(progressTimer);
    if (!JSON_OUTPUT) console.log('\nInterrupted — computing metrics with collected data...\n');

    const metrics = computeMetrics();
    if (JSON_OUTPUT) {
      process.stdout.write(JSON.stringify(metrics, null, 2) + '\n');
    } else {
      printReport(metrics);
    }

    server.close();
    process.exit(0);
  });
}

main();
