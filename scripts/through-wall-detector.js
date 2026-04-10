#!/usr/bin/env node
/**
 * Through-Wall Motion Detection — Multi-Frequency Mesh Application
 *
 * Detects motion behind walls by exploiting the fact that lower WiFi frequencies
 * penetrate walls better than higher frequencies. With 6 channels spanning
 * 2412-2462 MHz, we can:
 *
 *   1. Baseline each channel's attenuation through the wall (calibration phase)
 *   2. Detect changes above baseline = motion behind wall
 *   3. Weight lower channels more heavily (better through-wall SNR)
 *   4. Cross-validate across channels (real motion is coherent; noise is not)
 *
 * Requires multi-frequency mesh scanning (ADR-073): 2 ESP32 nodes hopping
 * across channels 1, 3, 5, 6, 9, 11.
 *
 * Usage:
 *   node scripts/through-wall-detector.js --calibrate 60
 *   node scripts/through-wall-detector.js --port 5006 --duration 300
 *   node scripts/through-wall-detector.js --replay data/recordings/overnight-1775217646.csi.jsonl
 *   node scripts/through-wall-detector.js --threshold 3.0
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
    port:      { type: 'string', short: 'p', default: '5006' },
    duration:  { type: 'string', short: 'd' },
    replay:    { type: 'string', short: 'r' },
    interval:  { type: 'string', short: 'i', default: '1000' },
    calibrate: { type: 'string', short: 'c', default: '30' },
    threshold: { type: 'string', short: 't', default: '2.5' },
    json:      { type: 'boolean', default: false },
    'consecutive-frames': { type: 'string', default: '3' },
  },
  strict: true,
});

const PORT = parseInt(args.port, 10);
const DURATION_MS = args.duration ? parseInt(args.duration, 10) * 1000 : null;
const INTERVAL_MS = parseInt(args.interval, 10);
const CALIBRATE_S = parseInt(args.calibrate, 10);
const ALERT_THRESHOLD = parseFloat(args.threshold);
const CONSECUTIVE_FRAMES = parseInt(args['consecutive-frames'], 10);
const JSON_OUTPUT = args.json;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const CHANNEL_FREQ = {};
for (let ch = 1; ch <= 13; ch++) CHANNEL_FREQ[ch] = 2412 + (ch - 1) * 5;

const NODE1_CHANNELS = [1, 6, 11];
const NODE2_CHANNELS = [3, 5, 9];

// Channel penetration weights: lower freq = better wall penetration
// Approximate wall loss at each channel for drywall+stud:
//   ch1 (2412 MHz) = 2.5 dB, ch11 (2462 MHz) = 2.7 dB
// Weight inversely proportional to loss
const PENETRATION_WEIGHT = {
  1:  1.00,  // 2412 MHz - best penetration
  3:  0.96,
  5:  0.92,
  6:  0.90,
  9:  0.85,
  11: 0.80,  // 2462 MHz - worst penetration
};

// Status display
const STATUS = {
  CALIBRATING: 'CALIBRATING',
  MONITORING:  'MONITORING',
  ALERT:       'ALERT',
};

// ---------------------------------------------------------------------------
// Per-channel baseline
// ---------------------------------------------------------------------------
class ChannelBaseline {
  constructor(channel) {
    this.channel = channel;
    this.freqMhz = CHANNEL_FREQ[channel] || 2432;
    this.weight = PENETRATION_WEIGHT[channel] || 0.9;

    // Welford online mean/variance
    this.nSub = 0;
    this.count = 0;
    this.mean = null;    // Float64Array
    this.m2 = null;      // Float64Array
    this.calibrated = false;
  }

  /** Ingest a frame during calibration */
  calibrate(amplitudes) {
    const n = amplitudes.length;
    if (!this.mean) {
      this.nSub = n;
      this.mean = new Float64Array(n);
      this.m2 = new Float64Array(n);
    }

    this.count++;
    for (let i = 0; i < n && i < this.nSub; i++) {
      const delta = amplitudes[i] - this.mean[i];
      this.mean[i] += delta / this.count;
      const delta2 = amplitudes[i] - this.mean[i];
      this.m2[i] += delta * delta2;
    }
  }

  /** Finalize calibration */
  finalize() {
    if (this.count < 5) return;
    this.calibrated = true;
  }

  /** Get standard deviation per subcarrier */
  getStd() {
    if (!this.mean || this.count < 2) return null;
    const std = new Float64Array(this.nSub);
    for (let i = 0; i < this.nSub; i++) {
      std[i] = Math.sqrt(this.m2[i] / (this.count - 1));
      // Minimum std to avoid division by zero
      if (std[i] < 0.1) std[i] = 0.1;
    }
    return std;
  }

  /**
   * Compute deviation score for a new frame.
   * Score = mean(|amplitude - baseline_mean| / baseline_std) across subcarriers
   */
  computeDeviation(amplitudes) {
    if (!this.calibrated || !this.mean) return 0;

    const std = this.getStd();
    if (!std) return 0;

    let sumDeviation = 0;
    let count = 0;
    for (let i = 0; i < amplitudes.length && i < this.nSub; i++) {
      const z = Math.abs(amplitudes[i] - this.mean[i]) / std[i];
      sumDeviation += z;
      count++;
    }

    return count > 0 ? sumDeviation / count : 0;
  }
}

// ---------------------------------------------------------------------------
// Through-wall detector
// ---------------------------------------------------------------------------
class ThroughWallDetector {
  constructor(calibrateDuration, alertThreshold, consecutiveFrames) {
    this.calibrateDuration = calibrateDuration;
    this.alertThreshold = alertThreshold;
    this.consecutiveFrames = consecutiveFrames;

    this.baselines = new Map(); // channel -> ChannelBaseline
    this.status = STATUS.CALIBRATING;
    this.startTime = null;

    // Detection state
    this.perChannelScores = new Map();
    this.fusedScore = 0;
    this.alertStreak = 0;
    this.alertActive = false;
    this.alerts = [];

    // History for display
    this.scoreHistory = []; // { timestamp, fusedScore, perChannel }
    this.maxHistory = 60;

    this.totalFrames = 0;
  }

  ingestFrame(channel, amplitudes, timestamp) {
    this.totalFrames++;

    if (!this.startTime) this.startTime = timestamp;

    // Get or create baseline
    if (!this.baselines.has(channel)) {
      this.baselines.set(channel, new ChannelBaseline(channel));
    }
    const baseline = this.baselines.get(channel);

    // Calibration phase
    if (this.status === STATUS.CALIBRATING) {
      baseline.calibrate(amplitudes);

      if (timestamp - this.startTime >= this.calibrateDuration) {
        // Finalize all baselines
        for (const bl of this.baselines.values()) bl.finalize();
        this.status = STATUS.MONITORING;
      }
      return;
    }

    // Detection phase
    const deviation = baseline.computeDeviation(amplitudes);
    const weight = PENETRATION_WEIGHT[channel] || 0.9;
    const weightedScore = deviation * weight;

    this.perChannelScores.set(channel, {
      deviation: deviation,
      weighted: weightedScore,
      channel,
      freqMhz: CHANNEL_FREQ[channel],
    });

    // Fused score: weighted average across all channels
    let sumWeighted = 0, sumWeights = 0;
    for (const [ch, score] of this.perChannelScores) {
      sumWeighted += score.weighted;
      sumWeights += PENETRATION_WEIGHT[ch] || 0.9;
    }
    this.fusedScore = sumWeights > 0 ? sumWeighted / sumWeights : 0;

    // Cross-channel coherence: how many channels agree on motion?
    let agreeCount = 0;
    for (const score of this.perChannelScores.values()) {
      if (score.deviation > this.alertThreshold * 0.5) agreeCount++;
    }
    const coherence = this.perChannelScores.size > 0
      ? agreeCount / this.perChannelScores.size
      : 0;

    // Alert logic
    if (this.fusedScore > this.alertThreshold && coherence > 0.4) {
      this.alertStreak++;
    } else {
      this.alertStreak = Math.max(0, this.alertStreak - 1);
    }

    const wasAlert = this.alertActive;
    this.alertActive = this.alertStreak >= this.consecutiveFrames;

    if (this.alertActive && !wasAlert) {
      this.status = STATUS.ALERT;
      this.alerts.push({
        timestamp,
        fusedScore: this.fusedScore,
        coherence,
        channels: [...this.perChannelScores.values()].map(s => ({
          ch: s.channel, dev: s.deviation.toFixed(2),
        })),
      });
    } else if (!this.alertActive && wasAlert) {
      this.status = STATUS.MONITORING;
    }

    // Store history
    this.scoreHistory.push({
      timestamp,
      fusedScore: this.fusedScore,
      coherence,
      perChannel: [...this.perChannelScores.entries()].map(([ch, s]) => ({
        ch, dev: s.deviation.toFixed(2), weight: (PENETRATION_WEIGHT[ch] || 0.9).toFixed(2),
      })),
    });
    if (this.scoreHistory.length > this.maxHistory) this.scoreHistory.shift();
  }

  getState() {
    return {
      status: this.status,
      fusedScore: this.fusedScore,
      alertActive: this.alertActive,
      alertStreak: this.alertStreak,
      totalFrames: this.totalFrames,
      calibratedChannels: [...this.baselines.values()]
        .filter(b => b.calibrated)
        .map(b => b.channel)
        .sort((a, b) => a - b),
      perChannelScores: [...this.perChannelScores.entries()]
        .sort((a, b) => a[0] - b[0])
        .map(([ch, s]) => ({ ch, deviation: s.deviation.toFixed(2), weighted: s.weighted.toFixed(2) })),
      alertCount: this.alerts.length,
      scoreHistory: this.scoreHistory,
    };
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
function renderStatus(detector) {
  const state = detector.getState();
  const lines = [];

  lines.push('');
  lines.push('  THROUGH-WALL MOTION DETECTOR');
  lines.push('  ' + '='.repeat(55));
  lines.push('');

  // Status banner
  const statusBanner = {
    [STATUS.CALIBRATING]: '  [ CALIBRATING ] Establishing wall baseline...',
    [STATUS.MONITORING]:  '  [ MONITORING  ] Watching for through-wall motion',
    [STATUS.ALERT]:       '  [ ** ALERT ** ] Motion detected behind wall!',
  };
  lines.push(statusBanner[state.status] || `  [ ${state.status} ]`);
  lines.push('');

  if (state.status === STATUS.CALIBRATING) {
    const progress = Math.min(100, (state.totalFrames / (CALIBRATE_S * 12)) * 100);
    const barLen = Math.floor(progress / 2);
    const bar = '\u2588'.repeat(barLen) + '\u2591'.repeat(50 - barLen);
    lines.push(`  Calibration progress: [${bar}] ${progress.toFixed(0)}%`);
    lines.push(`  Frames collected: ${state.totalFrames}`);
    lines.push(`  Channels: ${state.calibratedChannels.length > 0 ? state.calibratedChannels.join(', ') : 'accumulating...'}`);
    return lines.join('\n');
  }

  // Fused score meter
  const maxMeter = 40;
  const meterFill = Math.min(maxMeter, Math.floor((state.fusedScore / (ALERT_THRESHOLD * 2)) * maxMeter));
  const meterChar = state.alertActive ? '\u2588' : '\u2593';
  const meterEmpty = '\u2591';
  const meter = meterChar.repeat(meterFill) + meterEmpty.repeat(maxMeter - meterFill);
  const threshMark = Math.floor((ALERT_THRESHOLD / (ALERT_THRESHOLD * 2)) * maxMeter);
  lines.push(`  Fused score: [${meter}] ${state.fusedScore.toFixed(2)}`);
  lines.push(`  ${''.padStart(15 + threshMark)}^ threshold=${ALERT_THRESHOLD}`);

  // Per-channel breakdown
  lines.push('');
  lines.push('  Per-Channel Deviation (weighted by penetration quality):');
  lines.push('  ' + '-'.repeat(55));
  lines.push('  Ch  Freq(MHz)  Weight  Deviation  Weighted   Status');

  for (const score of state.perChannelScores) {
    const ch = score.ch;
    const freq = CHANNEL_FREQ[ch] || 0;
    const wt = (PENETRATION_WEIGHT[ch] || 0.9).toFixed(2);
    const dev = score.deviation;
    const wtd = score.weighted;
    const above = parseFloat(dev) > ALERT_THRESHOLD * 0.5;
    const marker = above ? ' <--' : '';
    lines.push(`  ${String(ch).padStart(2)}    ${freq}      ${wt}     ${dev.padStart(6)}     ${wtd.padStart(6)}  ${marker}`);
  }

  // Score timeline (last 30 readings)
  const history = state.scoreHistory.slice(-30);
  if (history.length > 0) {
    lines.push('');
    lines.push('  Score Timeline (last 30 readings):');
    const SPARK = '\u2581\u2582\u2583\u2584\u2585\u2586\u2587\u2588';
    let timeline = '  ';
    for (const h of history) {
      const level = Math.min(7, Math.floor((h.fusedScore / (ALERT_THRESHOLD * 2)) * 7.99));
      timeline += SPARK[level];
    }
    lines.push(timeline);
    lines.push(`  ${''.padStart(2)}${'oldest'.padEnd(15)}${''.padEnd(Math.max(0, history.length - 21))}newest`);
  }

  // Alert summary
  lines.push('');
  lines.push(`  Alert history: ${state.alertCount} alert(s)`);
  lines.push(`  Consecutive frames above threshold: ${state.alertStreak}/${CONSECUTIVE_FRAMES}`);
  lines.push(`  Calibrated channels: ${state.calibratedChannels.join(', ')}`);
  lines.push(`  Total frames: ${state.totalFrames}`);

  return lines.join('\n');
}

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------
const detector = new ThroughWallDetector(CALIBRATE_S, ALERT_THRESHOLD, CONSECUTIVE_FRAMES);
let lastDisplayMs = 0;

function displayUpdate() {
  const state = detector.getState();

  if (JSON_OUTPUT) {
    console.log(JSON.stringify({
      timestamp: Date.now() / 1000,
      status: state.status,
      fusedScore: +state.fusedScore.toFixed(3),
      alertActive: state.alertActive,
      perChannel: state.perChannelScores,
      alertCount: state.alertCount,
    }));
  } else {
    process.stdout.write('\x1B[2J\x1B[H');
    console.log(renderStatus(detector));
    console.log('');
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

    detector.ingestFrame(frame.channel, frame.amplitudes, Date.now() / 1000);

    const now = Date.now();
    if (now - lastDisplayMs >= INTERVAL_MS) {
      displayUpdate();
      lastDisplayMs = now;
    }
  });

  sock.bind(PORT, () => {
    if (!JSON_OUTPUT) {
      console.log(`Through-Wall Detector listening on UDP port ${PORT}`);
      console.log(`Calibration period: ${CALIBRATE_S}s`);
      console.log(`Alert threshold: ${ALERT_THRESHOLD}`);
      console.log('Waiting for CSI frames...');
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
  let firstAlertTs = null;
  let totalAlertWindows = 0;

  for await (const line of rl) {
    if (!line.trim()) continue;

    let record;
    try { record = JSON.parse(line); } catch { continue; }
    if (record.type !== 'raw_csi' || !record.iq_hex) continue;

    const amplitudes = parseIqHex(record.iq_hex, record.subcarriers || 64);
    const channel = record.channel || assignChannel(record.node_id);

    detector.ingestFrame(channel, amplitudes, record.timestamp);
    frameCount++;

    const tsMs = record.timestamp * 1000;
    if (lastAnalysisTs === 0) lastAnalysisTs = tsMs;

    if (tsMs - lastAnalysisTs >= INTERVAL_MS) {
      windowCount++;
      const state = detector.getState();

      if (state.alertActive) {
        totalAlertWindows++;
        if (!firstAlertTs) firstAlertTs = record.timestamp;
      }

      if (JSON_OUTPUT) {
        console.log(JSON.stringify({
          window: windowCount,
          timestamp: record.timestamp,
          status: state.status,
          fusedScore: +state.fusedScore.toFixed(3),
          alertActive: state.alertActive,
        }));
      } else {
        const statusTag = state.status === STATUS.ALERT ? ' ** ALERT **' :
          state.status === STATUS.CALIBRATING ? ' calibrating' : '';
        console.log(
          `  [${windowCount.toString().padStart(4)}] t=${record.timestamp.toFixed(1)}s` +
          `  score=${state.fusedScore.toFixed(2).padStart(5)}` +
          `  channels=${state.calibratedChannels.length}` +
          `  streak=${state.alertStreak}/${CONSECUTIVE_FRAMES}` +
          statusTag
        );
      }

      lastAnalysisTs = tsMs;
    }
  }

  // Final summary
  if (!JSON_OUTPUT) {
    const state = detector.getState();
    console.log('');
    console.log('='.repeat(60));
    console.log('THROUGH-WALL DETECTION SUMMARY');
    console.log('='.repeat(60));
    console.log(`  Total frames: ${frameCount}`);
    console.log(`  Analysis windows: ${windowCount}`);
    console.log(`  Calibrated channels: ${state.calibratedChannels.join(', ')}`);
    console.log(`  Alert windows: ${totalAlertWindows} / ${windowCount} (${windowCount > 0 ? (totalAlertWindows / windowCount * 100).toFixed(1) : 0}%)`);
    console.log(`  Total alerts: ${state.alertCount}`);
    if (firstAlertTs) {
      console.log(`  First alert at: t=${firstAlertTs.toFixed(1)}s`);
    }
    console.log(`  Threshold: ${ALERT_THRESHOLD}, Consecutive frames: ${CONSECUTIVE_FRAMES}`);
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
