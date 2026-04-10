#!/usr/bin/env node
/**
 * ADR-077: Gait Analysis / Movement Disorder Detection
 *
 * Extracts walking cadence, stride regularity, asymmetry, and tremor indicators
 * from CSI motion energy and phase variance time series.
 *
 * DISCLAIMER: This is an informational tool, NOT a medical device.
 * Do not use for clinical diagnosis of movement disorders.
 *
 * Usage:
 *   node scripts/gait-analyzer.js --replay data/recordings/overnight-1775217646.csi.jsonl
 *   node scripts/gait-analyzer.js --port 5006
 *   node scripts/gait-analyzer.js --replay FILE --json
 *
 * ADR: docs/adr/ADR-077-novel-rf-sensing-applications.md
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
    replay:   { type: 'string', short: 'r' },
    json:     { type: 'boolean', default: false },
    interval: { type: 'string', short: 'i', default: '5000' },
  },
  strict: true,
});

const PORT        = parseInt(args.port, 10);
const JSON_OUTPUT = args.json;
const INTERVAL_MS = parseInt(args.interval, 10);

// ---------------------------------------------------------------------------
// ADR-018 packet constants
// ---------------------------------------------------------------------------
const VITALS_MAGIC = 0xC5110002;
const FUSED_MAGIC  = 0xC5110004;

// ---------------------------------------------------------------------------
// Simple FFT (radix-2 DIT)
// ---------------------------------------------------------------------------
function fft(re, im) {
  const n = re.length;
  if (n <= 1) return;

  for (let i = 1, j = 0; i < n; i++) {
    let bit = n >> 1;
    for (; j & bit; bit >>= 1) j ^= bit;
    j ^= bit;
    if (i < j) {
      [re[i], re[j]] = [re[j], re[i]];
      [im[i], im[j]] = [im[j], im[i]];
    }
  }

  for (let len = 2; len <= n; len *= 2) {
    const half = len / 2;
    const angle = -2 * Math.PI / len;
    const wRe = Math.cos(angle);
    const wIm = Math.sin(angle);

    for (let i = 0; i < n; i += len) {
      let curRe = 1, curIm = 0;
      for (let j = 0; j < half; j++) {
        const tRe = curRe * re[i + j + half] - curIm * im[i + j + half];
        const tIm = curRe * im[i + j + half] + curIm * re[i + j + half];
        re[i + j + half] = re[i + j] - tRe;
        im[i + j + half] = im[i + j] - tIm;
        re[i + j] += tRe;
        im[i + j] += tIm;
        const newCurRe = curRe * wRe - curIm * wIm;
        curIm = curRe * wIm + curIm * wRe;
        curRe = newCurRe;
      }
    }
  }
}

function nextPow2(n) {
  let p = 1;
  while (p < n) p *= 2;
  return p;
}

// ---------------------------------------------------------------------------
// Gait analysis engine
// ---------------------------------------------------------------------------
class GaitAnalyzer {
  constructor() {
    // Per-node time series buffers (5-second windows at ~22 fps or ~1 Hz vitals)
    this.motionBuffers = new Map();  // nodeId -> [{ timestamp, motion }]
    this.phaseVarBuffers = new Map(); // nodeId -> [{ timestamp, phaseVar }]
    this.maxAge = 5.0; // seconds
    this.results = [];
  }

  pushMotion(nodeId, timestamp, motion) {
    if (!this.motionBuffers.has(nodeId)) this.motionBuffers.set(nodeId, []);
    const buf = this.motionBuffers.get(nodeId);
    buf.push({ timestamp, motion });
    const cutoff = timestamp - this.maxAge;
    while (buf.length > 0 && buf[0].timestamp < cutoff) buf.shift();
  }

  pushPhaseVar(nodeId, timestamp, phaseVar) {
    if (!this.phaseVarBuffers.has(nodeId)) this.phaseVarBuffers.set(nodeId, []);
    const buf = this.phaseVarBuffers.get(nodeId);
    buf.push({ timestamp, phaseVar });
    const cutoff = timestamp - this.maxAge;
    while (buf.length > 0 && buf[0].timestamp < cutoff) buf.shift();
  }

  analyze(timestamp) {
    const perNode = {};
    let bestCadence = 0;
    let bestRegularity = 0;
    const cadences = [];

    for (const [nodeId, buf] of this.motionBuffers) {
      if (buf.length < 5) {
        perNode[nodeId] = { cadence: 0, regularity: 0, state: 'insufficient data' };
        continue;
      }

      const motionValues = buf.map(b => b.motion);

      // Estimate sampling rate
      const duration = buf[buf.length - 1].timestamp - buf[0].timestamp;
      const fs = duration > 0 ? buf.length / duration : 1;

      // FFT for cadence
      const nfft = nextPow2(Math.max(motionValues.length, 32));
      const re = new Float64Array(nfft);
      const im = new Float64Array(nfft);

      const mean = motionValues.reduce((a, b) => a + b, 0) / motionValues.length;
      for (let i = 0; i < motionValues.length; i++) {
        const hann = 0.5 * (1 - Math.cos(2 * Math.PI * i / (motionValues.length - 1)));
        re[i] = (motionValues[i] - mean) * hann;
      }

      fft(re, im);

      // Find dominant frequency in walking range (0.8 - 2.5 Hz)
      const freqRes = fs / nfft;
      let peakPower = 0, peakFreq = 0;
      let totalPower = 0;

      for (let k = 1; k < nfft / 2; k++) {
        const freq = k * freqRes;
        const power = re[k] * re[k] + im[k] * im[k];
        totalPower += power;

        if (freq >= 0.8 && freq <= 2.5 && power > peakPower) {
          peakPower = power;
          peakFreq = freq;
        }
      }

      const cadence = peakFreq * 60; // steps per minute (each leg cycle)
      const regularity = totalPower > 0 ? peakPower / totalPower : 0;

      // Autocorrelation for stride regularity
      const autoCorr = this._autocorrelation(motionValues);
      const strideRegularity = autoCorr > 0 ? autoCorr : 0;

      // State classification
      let state;
      if (mean < 1.0) state = 'stationary';
      else if (peakFreq >= 0.8 && peakFreq <= 2.0 && regularity > 0.1) state = 'walking';
      else if (peakFreq > 2.0 && regularity > 0.1) state = 'running';
      else state = 'moving (irregular)';

      perNode[nodeId] = {
        cadence: +cadence.toFixed(1),
        cadenceHz: +peakFreq.toFixed(3),
        regularity: +regularity.toFixed(3),
        strideRegularity: +strideRegularity.toFixed(3),
        meanMotion: +mean.toFixed(3),
        state,
        samples: buf.length,
        fps: +fs.toFixed(1),
      };

      if (cadence > bestCadence) bestCadence = cadence;
      if (regularity > bestRegularity) bestRegularity = regularity;
      if (peakFreq > 0) cadences.push(cadence);
    }

    // Cross-node asymmetry (if 2+ nodes)
    let asymmetry = 0;
    const nodeKeys = Object.keys(perNode);
    if (nodeKeys.length >= 2) {
      const c0 = perNode[nodeKeys[0]].cadenceHz;
      const c1 = perNode[nodeKeys[1]].cadenceHz;
      const meanC = (c0 + c1) / 2;
      asymmetry = meanC > 0 ? Math.abs(c0 - c1) / meanC : 0;
    }

    // Tremor detection from phase variance
    let tremorScore = 0;
    let tremorFreq = 0;
    for (const [, buf] of this.phaseVarBuffers) {
      if (buf.length < 10) continue;

      const values = buf.map(b => b.phaseVar);
      const duration = buf[buf.length - 1].timestamp - buf[0].timestamp;
      const fs = duration > 0 ? buf.length / duration : 1;

      const nfft = nextPow2(Math.max(values.length, 32));
      const re = new Float64Array(nfft);
      const im = new Float64Array(nfft);
      const mean = values.reduce((a, b) => a + b, 0) / values.length;
      for (let i = 0; i < values.length; i++) re[i] = values[i] - mean;
      fft(re, im);

      const freqRes = fs / nfft;
      let tPeak = 0, tFreq = 0;
      for (let k = 1; k < nfft / 2; k++) {
        const freq = k * freqRes;
        const power = re[k] * re[k] + im[k] * im[k];
        if (freq >= 3.0 && freq <= 8.0 && power > tPeak) {
          tPeak = power;
          tFreq = freq;
        }
      }
      if (tPeak > tremorScore) {
        tremorScore = tPeak;
        tremorFreq = tFreq;
      }
    }

    // Normalize tremor score to 0-1 range (heuristic)
    const tremorNorm = Math.min(tremorScore / 100, 1.0);

    const result = {
      timestamp,
      cadence: +bestCadence.toFixed(1),
      regularity: +bestRegularity.toFixed(3),
      asymmetry: +asymmetry.toFixed(3),
      tremorScore: +tremorNorm.toFixed(3),
      tremorFreqHz: +tremorFreq.toFixed(2),
      perNode,
      overallState: this._overallState(perNode),
    };

    this.results.push(result);
    return result;
  }

  _autocorrelation(values) {
    const n = values.length;
    if (n < 4) return 0;

    const mean = values.reduce((a, b) => a + b, 0) / n;
    let denom = 0;
    for (let i = 0; i < n; i++) denom += (values[i] - mean) ** 2;
    if (denom < 0.001) return 0;

    // Check autocorrelation at lag = n/4 to n/2 (typical stride period range)
    let bestCorr = 0;
    const minLag = Math.max(2, Math.floor(n / 4));
    const maxLag = Math.floor(n / 2);

    for (let lag = minLag; lag <= maxLag; lag++) {
      let num = 0;
      for (let i = 0; i < n - lag; i++) {
        num += (values[i] - mean) * (values[i + lag] - mean);
      }
      const corr = num / denom;
      if (corr > bestCorr) bestCorr = corr;
    }

    return bestCorr;
  }

  _overallState(perNode) {
    const states = Object.values(perNode).map(n => n.state);
    if (states.includes('walking')) return 'walking';
    if (states.includes('running')) return 'running';
    if (states.includes('moving (irregular)')) return 'moving';
    return 'stationary';
  }
}

// ---------------------------------------------------------------------------
// Packet parsing
// ---------------------------------------------------------------------------
function parseVitalsJsonl(record) {
  if (record.type !== 'vitals') return null;
  return {
    timestamp: record.timestamp,
    nodeId: record.node_id,
    motion: record.motion_energy || 0,
  };
}

function parseCsiJsonl(record) {
  if (record.type !== 'raw_csi' || !record.iq_hex) return null;
  const nSc = record.subcarriers || 64;
  const bytes = Buffer.from(record.iq_hex, 'hex');

  // Compute phase variance across subcarriers
  let phaseSum = 0, phaseSqSum = 0, count = 0;
  for (let sc = 0; sc < nSc; sc++) {
    const offset = 2 + sc * 2;
    if (offset + 1 >= bytes.length) break;
    let I = bytes[offset]; if (I > 127) I -= 256;
    let Q = bytes[offset + 1]; if (Q > 127) Q -= 256;
    const phase = Math.atan2(Q, I);
    phaseSum += phase;
    phaseSqSum += phase * phase;
    count++;
  }

  const phaseMean = count > 0 ? phaseSum / count : 0;
  const phaseVar = count > 1 ? (phaseSqSum / count - phaseMean * phaseMean) : 0;

  return {
    timestamp: record.timestamp,
    nodeId: record.node_id,
    phaseVar: Math.abs(phaseVar),
  };
}

function parseVitalsUdp(buf) {
  if (buf.length < 32) return null;
  const magic = buf.readUInt32LE(0);
  if (magic !== VITALS_MAGIC && magic !== FUSED_MAGIC) return null;
  return {
    timestamp: Date.now() / 1000,
    nodeId: buf.readUInt8(4),
    motion: buf.readFloatLE(16),
  };
}

function parseCsiUdp(buf) {
  const frame = parseRawCsiFrame(buf);
  if (!frame) return null;

  let phaseSum = 0, phaseSqSum = 0, count = 0;
  for (let sc = 0; sc < frame.nSubcarriers; sc++) {
    const phase = frame.phases[sc];
    phaseSum += phase;
    phaseSqSum += phase * phase;
    count++;
  }

  const phaseMean = count > 0 ? phaseSum / count : 0;
  const phaseVar = count > 1 ? (phaseSqSum / count - phaseMean * phaseMean) : 0;

  return {
    timestamp: Date.now() / 1000,
    nodeId: frame.nodeId,
    phaseVar: Math.abs(phaseVar),
  };
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------
function formatResult(result) {
  const lines = [];
  const ts = new Date(result.timestamp * 1000).toISOString().slice(11, 19);
  lines.push(`[${ts}] ${result.overallState.toUpperCase()}`);
  lines.push(`  Cadence:    ${result.cadence} steps/min`);
  lines.push(`  Regularity: ${result.regularity}`);
  lines.push(`  Asymmetry:  ${result.asymmetry}`);
  lines.push(`  Tremor:     ${result.tremorScore} (${result.tremorFreqHz} Hz)`);

  for (const [nodeId, node] of Object.entries(result.perNode)) {
    lines.push(`  Node ${nodeId}: ${node.state} | ${node.cadence} spm | regularity ${node.regularity} | ${node.samples} samples @ ${node.fps} fps`);
  }

  // Flags
  const flags = [];
  if (result.asymmetry > 0.3) flags.push('HIGH ASYMMETRY');
  if (result.tremorScore > 0.3) flags.push(`TREMOR DETECTED (${result.tremorFreqHz} Hz)`);
  if (result.cadence > 0 && result.cadence < 50) flags.push('SLOW CADENCE');
  if (flags.length > 0) lines.push(`  ** ${flags.join(' | ')} **`);

  return lines.join('\n');
}

// ---------------------------------------------------------------------------
// Replay mode
// ---------------------------------------------------------------------------
async function startReplay(filePath) {
  if (!fs.existsSync(filePath)) {
    console.error(`File not found: ${filePath}`);
    process.exit(1);
  }

  const analyzer = new GaitAnalyzer();
  const rl = readline.createInterface({
    input: fs.createReadStream(filePath),
    crlfDelay: Infinity,
  });

  let frameCount = 0;
  let lastAnalysisTs = 0;

  for await (const line of rl) {
    if (!line.trim()) continue;
    let record;
    try { record = JSON.parse(line); } catch { continue; }

    const v = parseVitalsJsonl(record);
    if (v) {
      analyzer.pushMotion(v.nodeId, v.timestamp, v.motion);
      frameCount++;
    }

    const csi = parseCsiJsonl(record);
    if (csi) {
      analyzer.pushPhaseVar(csi.nodeId, csi.timestamp, csi.phaseVar);
    }

    const ts = (v || csi);
    if (!ts) continue;

    const tsMs = ts.timestamp * 1000;
    if (lastAnalysisTs === 0) lastAnalysisTs = tsMs;

    if (tsMs - lastAnalysisTs >= INTERVAL_MS) {
      const result = analyzer.analyze(ts.timestamp);

      if (JSON_OUTPUT) {
        console.log(JSON.stringify(result));
      } else {
        console.log(formatResult(result));
      }

      lastAnalysisTs = tsMs;
    }
  }

  // Summary
  if (!JSON_OUTPUT && analyzer.results.length > 0) {
    console.log('\n' + '='.repeat(60));
    console.log('GAIT ANALYSIS SUMMARY');
    console.log('DISCLAIMER: Informational only. Not a medical device.');
    console.log('='.repeat(60));

    const states = {};
    let totalCadence = 0, cadenceCount = 0;
    let maxTremor = 0;

    for (const r of analyzer.results) {
      states[r.overallState] = (states[r.overallState] || 0) + 1;
      if (r.cadence > 0) {
        totalCadence += r.cadence;
        cadenceCount++;
      }
      if (r.tremorScore > maxTremor) maxTremor = r.tremorScore;
    }

    console.log('Activity distribution:');
    for (const [state, count] of Object.entries(states)) {
      const pct = ((count / analyzer.results.length) * 100).toFixed(1);
      const bar = '\u2588'.repeat(Math.round(pct / 2));
      console.log(`  ${state.padEnd(15)} ${bar.padEnd(50)} ${pct}%`);
    }

    if (cadenceCount > 0) {
      console.log(`\nAverage walking cadence: ${(totalCadence / cadenceCount).toFixed(1)} steps/min`);
    }
    console.log(`Max tremor score: ${maxTremor.toFixed(3)}`);
    console.log(`Analysis windows: ${analyzer.results.length}`);
    console.log(`Processed ${frameCount} vitals packets`);
  }
}

// ---------------------------------------------------------------------------
// Live UDP mode
// ---------------------------------------------------------------------------
function startLive() {
  const analyzer = new GaitAnalyzer();
  const server = dgram.createSocket('udp4');

  server.on('message', (buf) => {
    const v = parseVitalsUdp(buf);
    if (v) analyzer.pushMotion(v.nodeId, v.timestamp, v.motion);

    const csi = parseCsiUdp(buf);
    if (csi) analyzer.pushPhaseVar(csi.nodeId, csi.timestamp, csi.phaseVar);
  });

  setInterval(() => {
    const result = analyzer.analyze(Date.now() / 1000);

    if (JSON_OUTPUT) {
      console.log(JSON.stringify(result));
    } else {
      process.stdout.write('\x1B[2J\x1B[H');
      console.log('=== GAIT ANALYZER (ADR-077) ===');
      console.log('DISCLAIMER: Informational only. Not a medical device.\n');
      console.log(formatResult(result));
    }
  }, INTERVAL_MS);

  server.bind(PORT, () => {
    if (!JSON_OUTPUT) {
      console.log(`Gait Analyzer listening on UDP :${PORT}`);
    }
  });

  process.on('SIGINT', () => { server.close(); process.exit(0); });
}

// ---------------------------------------------------------------------------
// Entry
// ---------------------------------------------------------------------------
if (args.replay) {
  startReplay(args.replay);
} else {
  startLive();
}
