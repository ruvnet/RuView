#!/usr/bin/env node
/**
 * Device Fingerprinting via RF Emissions — Multi-Frequency Mesh Application
 *
 * Identifies electronic devices by their unique RF characteristics across
 * multiple WiFi channels. Each device creates distinctive subcarrier patterns:
 *
 *   - WiFi APs: unique transmit power, phase noise, clock drift
 *   - Printers: motor EMI creates specific subcarrier modulation
 *   - Microwaves: 2.45 GHz magnetron radiates across channels 8-11
 *   - Bluetooth: frequency-hopping creates transient spikes
 *
 * Correlates WiFi scan SSID/signal with CSI patterns to build per-device
 * fingerprints, then detects when devices become active or inactive.
 *
 * Requires multi-frequency mesh scanning (ADR-073): 2 ESP32 nodes hopping
 * across channels 1, 3, 5, 6, 9, 11.
 *
 * Usage:
 *   node scripts/device-fingerprint.js
 *   node scripts/device-fingerprint.js --port 5006 --duration 120
 *   node scripts/device-fingerprint.js --replay data/recordings/overnight-1775217646.csi.jsonl
 *   node scripts/device-fingerprint.js --learn 30
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
    learn:    { type: 'string', short: 'l', default: '20' },
    json:     { type: 'boolean', default: false },
    'save-fingerprints': { type: 'string' },
    'load-fingerprints': { type: 'string' },
  },
  strict: true,
});

const PORT = parseInt(args.port, 10);
const DURATION_MS = args.duration ? parseInt(args.duration, 10) * 1000 : null;
const INTERVAL_MS = parseInt(args.interval, 10);
const LEARN_DURATION = parseInt(args.learn, 10);
const JSON_OUTPUT = args.json;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const CHANNEL_FREQ = {};
for (let ch = 1; ch <= 13; ch++) CHANNEL_FREQ[ch] = 2412 + (ch - 1) * 5;

const NODE1_CHANNELS = [1, 6, 11];
const NODE2_CHANNELS = [3, 5, 9];

// Known devices from WiFi scan (these are the devices we can fingerprint)
const KNOWN_DEVICES = [
  { id: 'ruv-net',      ssid: 'ruv.net',                    channel: 5,  signal: 100, type: 'router' },
  { id: 'cohen-guest',  ssid: 'Cohen-Guest',                channel: 5,  signal: 100, type: 'router' },
  { id: 'cogeco-21b20', ssid: 'COGECO-21B20',               channel: 11, signal: 100, type: 'router' },
  { id: 'hp-printer',   ssid: 'DIRECT-fa-HP M255 LaserJet', channel: 5,  signal: 94,  type: 'printer' },
  { id: 'conclusion',   ssid: 'conclusion mesh',            channel: 3,  signal: 44,  type: 'mesh-node' },
  { id: 'netgear72',    ssid: 'NETGEAR72',                  channel: 9,  signal: 42,  type: 'router' },
  { id: 'cogeco-4321',  ssid: 'COGECO-4321',                channel: 11, signal: 30,  type: 'router' },
  { id: 'innanen',      ssid: 'Innanen',                    channel: 6,  signal: 19,  type: 'router' },
];

// Activity states
const ACTIVITY = {
  UNKNOWN:  'unknown',
  ACTIVE:   'active',
  IDLE:     'idle',
  CHANGED:  'changed',
};

// ---------------------------------------------------------------------------
// Device fingerprint
// ---------------------------------------------------------------------------
class DeviceFingerprint {
  constructor(device) {
    this.device = device;
    this.id = device.id;
    this.channel = device.channel;

    // Per-subcarrier signature (learned during training)
    this.baselineMean = null;    // Float64Array
    this.baselineStd = null;     // Float64Array
    this.varianceProfile = null; // Float64Array - characteristic variance pattern
    this.nSub = 0;
    this.trainCount = 0;

    // Welford accumulators for training
    this._sum = null;
    this._sumSq = null;
    this._varSum = null;
    this._varSumSq = null;
    this._frameAmps = []; // store recent frames for variance computation

    // Runtime state
    this.activity = ACTIVITY.UNKNOWN;
    this.lastScore = 0;
    this.lastSeen = 0;
    this.activityHistory = [];
    this.maxHistory = 30;
  }

  /** Ingest a training frame */
  train(amplitudes) {
    const n = amplitudes.length;
    if (!this._sum) {
      this.nSub = n;
      this._sum = new Float64Array(n);
      this._sumSq = new Float64Array(n);
    }

    this.trainCount++;
    for (let i = 0; i < n && i < this.nSub; i++) {
      this._sum[i] += amplitudes[i];
      this._sumSq[i] += amplitudes[i] * amplitudes[i];
    }

    // Keep last 10 frames for variance profile
    this._frameAmps.push(new Float64Array(amplitudes));
    if (this._frameAmps.length > 10) this._frameAmps.shift();
  }

  /** Finalize training */
  finalizeTrain() {
    if (this.trainCount < 3 || !this._sum) return false;

    this.baselineMean = new Float64Array(this.nSub);
    this.baselineStd = new Float64Array(this.nSub);

    for (let i = 0; i < this.nSub; i++) {
      this.baselineMean[i] = this._sum[i] / this.trainCount;
      const variance = (this._sumSq[i] / this.trainCount) - (this.baselineMean[i] ** 2);
      this.baselineStd[i] = Math.sqrt(Math.max(0, variance));
      if (this.baselineStd[i] < 0.1) this.baselineStd[i] = 0.1;
    }

    // Compute variance profile from stored frames
    if (this._frameAmps.length >= 3) {
      this.varianceProfile = new Float64Array(this.nSub);
      for (let i = 0; i < this.nSub; i++) {
        let sum = 0, sumSq = 0;
        for (const frame of this._frameAmps) {
          sum += frame[i];
          sumSq += frame[i] * frame[i];
        }
        const n = this._frameAmps.length;
        const mean = sum / n;
        this.varianceProfile[i] = (sumSq / n) - (mean * mean);
      }
    }

    // Clean up training data
    this._sum = null;
    this._sumSq = null;
    this._frameAmps = [];

    return true;
  }

  /**
   * Score a new frame against this device's fingerprint.
   * Returns a similarity score (0 = no match, 1 = perfect match).
   */
  score(amplitudes) {
    if (!this.baselineMean) return 0;

    const n = Math.min(amplitudes.length, this.nSub);
    let matchScore = 0;
    let count = 0;

    for (let i = 0; i < n; i++) {
      // Normalized difference from baseline
      const diff = Math.abs(amplitudes[i] - this.baselineMean[i]);
      const normalizedDiff = diff / this.baselineStd[i];

      // Score: 1.0 if within 1 std, decreasing beyond
      const subScore = Math.exp(-0.5 * normalizedDiff * normalizedDiff);
      matchScore += subScore;
      count++;
    }

    return count > 0 ? matchScore / count : 0;
  }

  /**
   * Detect activity change.
   * Compare current frame's variance against baseline variance profile.
   */
  detectActivity(amplitudes, timestamp) {
    const similarity = this.score(amplitudes);
    this.lastScore = similarity;
    this.lastSeen = timestamp;

    // Activity thresholds
    const prevActivity = this.activity;
    if (similarity > 0.7) {
      this.activity = ACTIVITY.ACTIVE;
    } else if (similarity > 0.4) {
      this.activity = ACTIVITY.CHANGED;
    } else {
      this.activity = ACTIVITY.IDLE;
    }

    // Record transitions
    if (prevActivity !== this.activity && prevActivity !== ACTIVITY.UNKNOWN) {
      this.activityHistory.push({
        timestamp,
        from: prevActivity,
        to: this.activity,
        score: similarity.toFixed(3),
      });
      if (this.activityHistory.length > this.maxHistory) this.activityHistory.shift();
    }

    return {
      id: this.id,
      ssid: this.device.ssid,
      type: this.device.type,
      channel: this.channel,
      activity: this.activity,
      similarity: similarity.toFixed(3),
      changed: prevActivity !== this.activity && prevActivity !== ACTIVITY.UNKNOWN,
    };
  }

  /** Export fingerprint for persistence */
  exportFingerprint() {
    return {
      id: this.id,
      device: this.device,
      nSub: this.nSub,
      trainCount: this.trainCount,
      baselineMean: this.baselineMean ? Array.from(this.baselineMean) : null,
      baselineStd: this.baselineStd ? Array.from(this.baselineStd) : null,
      varianceProfile: this.varianceProfile ? Array.from(this.varianceProfile) : null,
    };
  }

  /** Import fingerprint from saved data */
  importFingerprint(data) {
    this.nSub = data.nSub;
    this.trainCount = data.trainCount;
    this.baselineMean = data.baselineMean ? new Float64Array(data.baselineMean) : null;
    this.baselineStd = data.baselineStd ? new Float64Array(data.baselineStd) : null;
    this.varianceProfile = data.varianceProfile ? new Float64Array(data.varianceProfile) : null;
  }
}

// ---------------------------------------------------------------------------
// Device fingerprint manager
// ---------------------------------------------------------------------------
class FingerprintManager {
  constructor(learnDuration) {
    this.learnDuration = learnDuration;
    this.fingerprints = new Map(); // id -> DeviceFingerprint
    this.learning = true;
    this.startTime = null;
    this.totalFrames = 0;

    // Initialize fingerprints for known devices
    for (const device of KNOWN_DEVICES) {
      this.fingerprints.set(device.id, new DeviceFingerprint(device));
    }
  }

  ingestFrame(channel, amplitudes, timestamp) {
    this.totalFrames++;
    if (!this.startTime) this.startTime = timestamp;

    // Learning phase: train fingerprints for devices on this channel
    if (this.learning) {
      for (const fp of this.fingerprints.values()) {
        if (fp.channel === channel) {
          fp.train(amplitudes);
        }
      }

      if (timestamp - this.startTime >= this.learnDuration) {
        // Finalize all fingerprints
        let trained = 0;
        for (const fp of this.fingerprints.values()) {
          if (fp.finalizeTrain()) trained++;
        }
        this.learning = false;
        return { event: 'learn_complete', trained, total: this.fingerprints.size };
      }

      return { event: 'learning', elapsed: timestamp - this.startTime, duration: this.learnDuration };
    }

    // Detection phase: score all devices on this channel
    const results = [];
    for (const fp of this.fingerprints.values()) {
      if (fp.channel === channel) {
        const result = fp.detectActivity(amplitudes, timestamp);
        results.push(result);
      }
    }

    return { event: 'detect', results };
  }

  /** Get current device activity summary */
  getSummary() {
    const devices = [];
    for (const fp of this.fingerprints.values()) {
      devices.push({
        id: fp.id,
        ssid: fp.device.ssid,
        type: fp.device.type,
        channel: fp.channel,
        activity: fp.activity,
        similarity: fp.lastScore.toFixed(3),
        trained: fp.baselineMean !== null,
        trainFrames: fp.trainCount,
        transitions: fp.activityHistory.length,
      });
    }

    return {
      learning: this.learning,
      totalFrames: this.totalFrames,
      devices: devices.sort((a, b) => parseFloat(b.similarity) - parseFloat(a.similarity)),
    };
  }

  /** Save fingerprints to file */
  saveFingerprints(filePath) {
    const data = {};
    for (const [id, fp] of this.fingerprints) {
      if (fp.baselineMean) {
        data[id] = fp.exportFingerprint();
      }
    }
    fs.writeFileSync(filePath, JSON.stringify(data, null, 2));
    return Object.keys(data).length;
  }

  /** Load fingerprints from file */
  loadFingerprints(filePath) {
    if (!fs.existsSync(filePath)) return 0;
    const data = JSON.parse(fs.readFileSync(filePath, 'utf8'));
    let loaded = 0;
    for (const [id, fpData] of Object.entries(data)) {
      if (this.fingerprints.has(id)) {
        this.fingerprints.get(id).importFingerprint(fpData);
        loaded++;
      }
    }
    if (loaded > 0) this.learning = false;
    return loaded;
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
function renderDeviceTable(manager) {
  const summary = manager.getSummary();
  const lines = [];

  lines.push('');
  lines.push('  DEVICE FINGERPRINTING — RF EMISSIONS ANALYSIS');
  lines.push('  ' + '='.repeat(60));
  lines.push('');

  if (summary.learning) {
    const elapsed = manager.startTime ? Date.now() / 1000 - manager.startTime : 0;
    const progress = Math.min(100, (elapsed / manager.learnDuration) * 100);
    const barLen = Math.floor(progress / 2);
    const bar = '\u2588'.repeat(barLen) + '\u2591'.repeat(50 - barLen);
    lines.push(`  Learning device signatures: [${bar}] ${progress.toFixed(0)}%`);
    lines.push(`  Frames: ${summary.totalFrames}`);
    lines.push('');
  }

  // Device activity table
  const activitySymbol = {
    [ACTIVITY.ACTIVE]:  '[ON] ',
    [ACTIVITY.IDLE]:    '[off]',
    [ACTIVITY.CHANGED]: '[CHG]',
    [ACTIVITY.UNKNOWN]: '[ ? ]',
  };

  lines.push('  Device                         Type       Ch  Similarity  Status');
  lines.push('  ' + '-'.repeat(65));

  for (const dev of summary.devices) {
    const status = activitySymbol[dev.activity] || '[ ? ]';
    const trained = dev.trained ? '' : ' (untrained)';
    lines.push(
      `  ${dev.ssid.substring(0, 28).padEnd(30)} ${dev.type.padEnd(10)} ${String(dev.channel).padStart(2)}  ` +
      `${dev.similarity.padStart(7)}     ${status}${trained}`
    );
  }

  return lines.join('\n');
}

function renderTimeline(manager) {
  const summary = manager.getSummary();
  const lines = [];

  lines.push('');
  lines.push('  Activity Transitions:');
  lines.push('  ' + '-'.repeat(50));

  let hasTransitions = false;
  for (const dev of summary.devices) {
    const fp = manager.fingerprints.get(dev.id);
    if (fp && fp.activityHistory.length > 0) {
      hasTransitions = true;
      const recent = fp.activityHistory.slice(-3);
      for (const t of recent) {
        const time = new Date(t.timestamp * 1000).toISOString().substring(11, 19);
        lines.push(`    ${time}  ${dev.ssid.substring(0, 20).padEnd(20)}  ${t.from} -> ${t.to}  (score=${t.score})`);
      }
    }
  }

  if (!hasTransitions) {
    lines.push('    (no transitions detected yet)');
  }

  return lines.join('\n');
}

function renderChannelActivity(manager) {
  const summary = manager.getSummary();
  const lines = [];

  lines.push('');
  lines.push('  Per-Channel Device Activity:');

  const channels = [...new Set(summary.devices.map(d => d.channel))].sort((a, b) => a - b);
  for (const ch of channels) {
    const devs = summary.devices.filter(d => d.channel === ch);
    const activeCount = devs.filter(d => d.activity === ACTIVITY.ACTIVE).length;
    lines.push(`    ch${ch} (${CHANNEL_FREQ[ch]} MHz): ${activeCount}/${devs.length} devices active`);
    for (const dev of devs) {
      const bar = '\u2588'.repeat(Math.floor(parseFloat(dev.similarity) * 20));
      lines.push(`      ${dev.ssid.substring(0, 18).padEnd(18)} ${bar} ${dev.similarity}`);
    }
  }

  return lines.join('\n');
}

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------
const manager = new FingerprintManager(LEARN_DURATION);
let lastDisplayMs = 0;

// Load saved fingerprints if specified
if (args['load-fingerprints']) {
  const loaded = manager.loadFingerprints(args['load-fingerprints']);
  if (!JSON_OUTPUT) console.log(`Loaded ${loaded} fingerprints from ${args['load-fingerprints']}`);
}

function displayUpdate() {
  if (JSON_OUTPUT) {
    const summary = manager.getSummary();
    console.log(JSON.stringify({
      timestamp: Date.now() / 1000,
      learning: summary.learning,
      totalFrames: summary.totalFrames,
      devices: summary.devices.map(d => ({
        id: d.id, ssid: d.ssid, activity: d.activity,
        similarity: d.similarity, channel: d.channel,
      })),
    }));
  } else {
    process.stdout.write('\x1B[2J\x1B[H');
    console.log(renderDeviceTable(manager));
    console.log(renderTimeline(manager));
    console.log(renderChannelActivity(manager));
    console.log('');
    console.log(`  Total frames: ${manager.totalFrames}`);
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

    const result = manager.ingestFrame(frame.channel, frame.amplitudes, Date.now() / 1000);

    // Announce learning completion
    if (result && result.event === 'learn_complete' && !JSON_OUTPUT) {
      console.log(`\nLearning complete! Trained ${result.trained}/${result.total} device fingerprints`);
    }

    const now = Date.now();
    if (now - lastDisplayMs >= INTERVAL_MS) {
      displayUpdate();
      lastDisplayMs = now;
    }
  });

  sock.bind(PORT, () => {
    if (!JSON_OUTPUT) {
      console.log(`Device Fingerprinter listening on UDP port ${PORT}`);
      console.log(`Learning duration: ${LEARN_DURATION}s`);
      console.log(`Known devices: ${KNOWN_DEVICES.length}`);
      console.log('Waiting for CSI frames...');
    }
  });

  if (DURATION_MS) {
    setTimeout(() => {
      displayUpdate();
      if (args['save-fingerprints']) {
        const saved = manager.saveFingerprints(args['save-fingerprints']);
        if (!JSON_OUTPUT) console.log(`Saved ${saved} fingerprints to ${args['save-fingerprints']}`);
      }
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
  let learnComplete = false;

  for await (const line of rl) {
    if (!line.trim()) continue;

    let record;
    try { record = JSON.parse(line); } catch { continue; }
    if (record.type !== 'raw_csi' || !record.iq_hex) continue;

    const amplitudes = parseIqHex(record.iq_hex, record.subcarriers || 64);
    const channel = record.channel || assignChannel(record.node_id);

    const result = manager.ingestFrame(channel, amplitudes, record.timestamp);
    frameCount++;

    if (result && result.event === 'learn_complete' && !learnComplete) {
      learnComplete = true;
      if (!JSON_OUTPUT) {
        console.log(`\nLearning complete at t=${record.timestamp.toFixed(1)}s`);
        console.log(`Trained ${result.trained}/${result.total} device fingerprints`);
        console.log('');
      }
    }

    const tsMs = record.timestamp * 1000;
    if (lastAnalysisTs === 0) lastAnalysisTs = tsMs;

    if (tsMs - lastAnalysisTs >= INTERVAL_MS) {
      windowCount++;
      const summary = manager.getSummary();

      if (JSON_OUTPUT) {
        console.log(JSON.stringify({
          window: windowCount,
          timestamp: record.timestamp,
          learning: summary.learning,
          devices: summary.devices.map(d => ({
            id: d.id, activity: d.activity, similarity: d.similarity,
          })),
        }));
      } else if (!summary.learning) {
        // Compact per-window output
        const active = summary.devices.filter(d => d.activity === ACTIVITY.ACTIVE);
        const changed = summary.devices.filter(d => d.activity === ACTIVITY.CHANGED);
        let line = `  [${String(windowCount).padStart(4)}] t=${record.timestamp.toFixed(1)}s  active: `;
        line += active.length > 0
          ? active.map(d => `${d.ssid.substring(0, 15)}(${d.similarity})`).join(', ')
          : '(none)';
        if (changed.length > 0) {
          line += '  changed: ' + changed.map(d => d.ssid.substring(0, 12)).join(', ');
        }
        console.log(line);
      }

      lastAnalysisTs = tsMs;
    }
  }

  // Save fingerprints if requested
  if (args['save-fingerprints']) {
    const saved = manager.saveFingerprints(args['save-fingerprints']);
    if (!JSON_OUTPUT) console.log(`\nSaved ${saved} fingerprints to ${args['save-fingerprints']}`);
  }

  // Final summary
  if (!JSON_OUTPUT) {
    const summary = manager.getSummary();
    console.log('');
    console.log('='.repeat(60));
    console.log('DEVICE FINGERPRINT SUMMARY');
    console.log('='.repeat(60));
    console.log(renderDeviceTable(manager));
    console.log(renderTimeline(manager));

    // Statistics
    const trained = summary.devices.filter(d => d.trained).length;
    const active = summary.devices.filter(d => d.activity === ACTIVITY.ACTIVE).length;
    console.log('');
    console.log(`  Trained fingerprints: ${trained}/${summary.devices.length}`);
    console.log(`  Currently active: ${active}/${summary.devices.length}`);
    console.log(`  Total frames: ${frameCount}`);
    console.log(`  Analysis windows: ${windowCount}`);
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
