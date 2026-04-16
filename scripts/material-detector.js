#!/usr/bin/env node
/**
 * ADR-077: Material/Object Change Detection
 *
 * Monitors CSI subcarrier null patterns to detect when objects (metal, water,
 * wood, glass) are introduced, removed, or moved in the sensing area.
 *
 * Usage:
 *   node scripts/material-detector.js --replay data/recordings/overnight-1775217646.csi.jsonl
 *   node scripts/material-detector.js --port 5006
 *   node scripts/material-detector.js --replay FILE --json
 *   node scripts/material-detector.js --replay FILE --baseline-time 120
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
    port:           { type: 'string', short: 'p', default: '5006' },
    replay:         { type: 'string', short: 'r' },
    json:           { type: 'boolean', default: false },
    interval:       { type: 'string', short: 'i', default: '5000' },
    'baseline-time': { type: 'string', default: '60' },
    'null-threshold': { type: 'string', default: '2.0' },
    'change-threshold': { type: 'string', default: '3' },
  },
  strict: true,
});

const PORT             = parseInt(args.port, 10);
const JSON_OUTPUT      = args.json;
const INTERVAL_MS      = parseInt(args.interval, 10);
const BASELINE_SEC     = parseInt(args['baseline-time'], 10);
const NULL_THRESHOLD   = parseFloat(args['null-threshold']);
const CHANGE_THRESHOLD = parseInt(args['change-threshold'], 10); // min subcarriers changed

// ---------------------------------------------------------------------------
// ADR-018 raw CSI parser

// ---------------------------------------------------------------------------
// Subcarrier null pattern tracker
// ---------------------------------------------------------------------------
class NullPatternTracker {
  constructor(nSubcarriers) {
    this.nSc = nSubcarriers || 64;

    // Baseline (Welford mean per subcarrier)
    this.baselineMean = new Float64Array(256);
    this.baselineCount = new Uint32Array(256);
    this.baselineEstablished = false;
    this.baselineNulls = new Set();

    // Current window state
    this.currentAmps = new Float64Array(256);
    this.currentCount = 0;

    // Events
    this.events = [];
    this.startTime = null;
    this.lastTime = null;
  }

  updateBaseline(amplitudes) {
    const n = amplitudes.length;
    this.nSc = n;
    for (let i = 0; i < n; i++) {
      this.baselineCount[i]++;
      const delta = amplitudes[i] - this.baselineMean[i];
      this.baselineMean[i] += delta / this.baselineCount[i];
    }
  }

  finalizeBaseline() {
    this.baselineNulls = new Set();
    for (let i = 0; i < this.nSc; i++) {
      if (this.baselineMean[i] < NULL_THRESHOLD) {
        this.baselineNulls.add(i);
      }
    }
    this.baselineEstablished = true;
  }

  updateCurrent(amplitudes) {
    const n = amplitudes.length;
    // Exponential moving average for current window
    const alpha = 0.1;
    if (this.currentCount === 0) {
      for (let i = 0; i < n; i++) this.currentAmps[i] = amplitudes[i];
    } else {
      for (let i = 0; i < n; i++) {
        this.currentAmps[i] = this.currentAmps[i] * (1 - alpha) + amplitudes[i] * alpha;
      }
    }
    this.currentCount++;
  }

  detectChanges(timestamp) {
    if (!this.baselineEstablished || this.currentCount < 5) return null;

    const currentNulls = new Set();
    for (let i = 0; i < this.nSc; i++) {
      if (this.currentAmps[i] < NULL_THRESHOLD) {
        currentNulls.add(i);
      }
    }

    // Find differences
    const newNulls = [];      // appeared (something blocking)
    const removedNulls = [];  // disappeared (object removed)
    const shiftedNulls = [];  // nearby shifts

    for (const sc of currentNulls) {
      if (!this.baselineNulls.has(sc)) newNulls.push(sc);
    }
    for (const sc of this.baselineNulls) {
      if (!currentNulls.has(sc)) removedNulls.push(sc);
    }

    // Detect shifts (null moved by 1-3 subcarriers)
    for (const newSc of newNulls) {
      for (const rmSc of removedNulls) {
        if (Math.abs(newSc - rmSc) <= 3) {
          shiftedNulls.push({ from: rmSc, to: newSc });
        }
      }
    }

    // Amplitude changes (non-null subcarriers with significant amplitude shift)
    const ampChanges = [];
    for (let i = 0; i < this.nSc; i++) {
      if (this.baselineMean[i] > NULL_THRESHOLD && this.currentAmps[i] > NULL_THRESHOLD) {
        const ratio = this.currentAmps[i] / this.baselineMean[i];
        if (ratio < 0.5 || ratio > 2.0) {
          ampChanges.push({ sc: i, baseline: this.baselineMean[i], current: this.currentAmps[i], ratio });
        }
      }
    }

    // Material classification
    let material = 'unknown';
    if (newNulls.length > 0) {
      // Null pattern indicates metal
      if (newNulls.length <= 5) material = 'metal (small object)';
      else if (newNulls.length <= 15) material = 'metal (medium)';
      else material = 'metal (large)';
    } else if (ampChanges.length > this.nSc * 0.3) {
      // Broad amplitude change = water or human
      const avgRatio = ampChanges.reduce((s, c) => s + c.ratio, 0) / ampChanges.length;
      material = avgRatio < 1 ? 'water/human (absorption)' : 'reflective surface';
    } else if (ampChanges.length > 0 && ampChanges.length <= this.nSc * 0.1) {
      material = 'wood/plastic (minimal)';
    }

    const totalChanges = newNulls.length + removedNulls.length + ampChanges.length;

    // Only report if significant changes
    if (totalChanges < CHANGE_THRESHOLD) {
      return {
        timestamp,
        changeDetected: false,
        currentNullCount: currentNulls.size,
        baselineNullCount: this.baselineNulls.size,
      };
    }

    // Determine event type
    let eventType;
    if (shiftedNulls.length > 0) eventType = 'moved';
    else if (newNulls.length > removedNulls.length) eventType = 'added';
    else if (removedNulls.length > newNulls.length) eventType = 'removed';
    else eventType = 'changed';

    const event = {
      timestamp,
      changeDetected: true,
      eventType,
      material,
      newNulls: newNulls.length,
      removedNulls: removedNulls.length,
      shiftedNulls: shiftedNulls.length,
      ampChanges: ampChanges.length,
      newNullRange: newNulls.length > 0 ? [Math.min(...newNulls), Math.max(...newNulls)] : null,
      removedNullRange: removedNulls.length > 0 ? [Math.min(...removedNulls), Math.max(...removedNulls)] : null,
      currentNullCount: currentNulls.size,
      baselineNullCount: this.baselineNulls.size,
      nullDelta: currentNulls.size - this.baselineNulls.size,
    };

    this.events.push(event);
    return event;
  }

  renderNullMap() {
    const chars = [];
    for (let i = 0; i < this.nSc; i++) {
      if (this.currentAmps[i] < NULL_THRESHOLD) {
        if (this.baselineNulls.has(i)) chars.push('_'); // baseline null
        else chars.push('X'); // new null
      } else if (this.baselineNulls.has(i)) {
        chars.push('O'); // removed null
      } else {
        chars.push('\u2581'); // normal
      }
    }
    return chars.join('');
  }
}

// ---------------------------------------------------------------------------
// Multi-node manager
// ---------------------------------------------------------------------------
class MaterialDetector {
  constructor() {
    this.trackers = new Map(); // nodeId -> NullPatternTracker
    this.startTime = null;
    this.allEvents = [];
  }

  ingestCSI(nodeId, timestamp, amplitudes) {
    if (!this.startTime) this.startTime = timestamp;

    if (!this.trackers.has(nodeId)) {
      this.trackers.set(nodeId, new NullPatternTracker(amplitudes.length));
    }
    const tracker = this.trackers.get(nodeId);
    tracker.lastTime = timestamp;
    if (!tracker.startTime) tracker.startTime = timestamp;

    // Baseline phase
    const elapsed = timestamp - tracker.startTime;
    if (elapsed < BASELINE_SEC) {
      tracker.updateBaseline(amplitudes);
      return null;
    }

    // Finalize baseline on transition
    if (!tracker.baselineEstablished) {
      tracker.finalizeBaseline();
    }

    tracker.updateCurrent(amplitudes);
    return null; // actual detection happens on analyze() call
  }

  analyze(timestamp) {
    const results = {};
    for (const [nodeId, tracker] of this.trackers) {
      const result = tracker.detectChanges(timestamp);
      if (result) {
        result.nodeId = nodeId;
        results[nodeId] = result;
        if (result.changeDetected) {
          this.allEvents.push(result);
        }
      }
    }
    return results;
  }
}

// ---------------------------------------------------------------------------
// Packet parsing
// ---------------------------------------------------------------------------
function parseCsiJsonl(record) {
  if (record.type !== 'raw_csi' || !record.iq_hex) return null;
  const nSc = record.subcarriers || 64;
  const bytes = Buffer.from(record.iq_hex, 'hex');
  const amplitudes = new Float64Array(nSc);

  for (let sc = 0; sc < nSc; sc++) {
    const offset = 2 + sc * 2;
    if (offset + 1 >= bytes.length) break;
    let I = bytes[offset]; if (I > 127) I -= 256;
    let Q = bytes[offset + 1]; if (Q > 127) Q -= 256;
    amplitudes[sc] = Math.sqrt(I * I + Q * Q);
  }

  return { timestamp: record.timestamp, nodeId: record.node_id, amplitudes };
}

function parseCsiUdp(buf) {
  const frame = parseRawCsiFrame(buf, { includePhase: false });
  if (!frame) return null;
  return {
    timestamp: Date.now() / 1000,
    nodeId: frame.nodeId,
    amplitudes: frame.amplitudes,
  };
}

// ---------------------------------------------------------------------------
// Replay mode
// ---------------------------------------------------------------------------
async function startReplay(filePath) {
  if (!fs.existsSync(filePath)) {
    console.error(`File not found: ${filePath}`);
    process.exit(1);
  }

  const detector = new MaterialDetector();
  const rl = readline.createInterface({
    input: fs.createReadStream(filePath),
    crlfDelay: Infinity,
  });

  let frameCount = 0;
  let lastAnalysisTs = 0;
  let baselineReported = new Set();

  for await (const line of rl) {
    if (!line.trim()) continue;
    let record;
    try { record = JSON.parse(line); } catch { continue; }

    const csi = parseCsiJsonl(record);
    if (!csi) continue;

    detector.ingestCSI(csi.nodeId, csi.timestamp, csi.amplitudes);
    frameCount++;

    // Report baseline completion
    for (const [nodeId, tracker] of detector.trackers) {
      if (tracker.baselineEstablished && !baselineReported.has(nodeId)) {
        baselineReported.add(nodeId);
        if (!JSON_OUTPUT) {
          console.log(`Node ${nodeId}: baseline established (${tracker.baselineNulls.size} nulls, ${((tracker.baselineNulls.size / tracker.nSc) * 100).toFixed(0)}%)`);
        }
      }
    }

    const tsMs = csi.timestamp * 1000;
    if (lastAnalysisTs === 0) lastAnalysisTs = tsMs;

    if (tsMs - lastAnalysisTs >= INTERVAL_MS) {
      const results = detector.analyze(csi.timestamp);

      for (const [nodeId, result] of Object.entries(results)) {
        if (JSON_OUTPUT) {
          console.log(JSON.stringify(result));
        } else if (result.changeDetected) {
          const ts = new Date(csi.timestamp * 1000).toISOString().slice(11, 19);
          console.log(`[${ts}] Node ${nodeId}: ${result.eventType.toUpperCase()} | ${result.material} | nulls ${result.baselineNullCount} -> ${result.currentNullCount} (delta ${result.nullDelta > 0 ? '+' : ''}${result.nullDelta})`);
          if (result.newNullRange) console.log(`  New nulls: sc ${result.newNullRange[0]}-${result.newNullRange[1]} (${result.newNulls} subcarriers)`);
          if (result.removedNullRange) console.log(`  Removed nulls: sc ${result.removedNullRange[0]}-${result.removedNullRange[1]} (${result.removedNulls} subcarriers)`);
          if (result.ampChanges > 0) console.log(`  Amplitude changes: ${result.ampChanges} subcarriers`);
        }
      }

      lastAnalysisTs = tsMs;
    }
  }

  // Summary
  if (!JSON_OUTPUT) {
    console.log('\n' + '='.repeat(60));
    console.log('MATERIAL/OBJECT CHANGE DETECTION SUMMARY');
    console.log('='.repeat(60));

    for (const [nodeId, tracker] of detector.trackers) {
      console.log(`\nNode ${nodeId}:`);
      console.log(`  Baseline nulls: ${tracker.baselineNulls.size} / ${tracker.nSc} (${((tracker.baselineNulls.size / tracker.nSc) * 100).toFixed(0)}%)`);
      console.log(`  Current map:  ${tracker.renderNullMap()}`);
      console.log(`  Legend: _ = baseline null, X = new null, O = removed null, \u2581 = normal`);
    }

    console.log(`\nTotal change events: ${detector.allEvents.length}`);
    if (detector.allEvents.length > 0) {
      const types = {};
      const materials = {};
      for (const e of detector.allEvents) {
        types[e.eventType] = (types[e.eventType] || 0) + 1;
        materials[e.material] = (materials[e.material] || 0) + 1;
      }
      console.log('Event types:');
      for (const [t, c] of Object.entries(types)) console.log(`  ${t}: ${c}`);
      console.log('Materials:');
      for (const [m, c] of Object.entries(materials)) console.log(`  ${m}: ${c}`);
    }

    console.log(`\nProcessed ${frameCount} CSI frames`);
  } else {
    console.log(JSON.stringify({
      type: 'summary',
      events: detector.allEvents.length,
      frames: frameCount,
    }));
  }
}

// ---------------------------------------------------------------------------
// Live UDP mode
// ---------------------------------------------------------------------------
function startLive() {
  const detector = new MaterialDetector();
  const server = dgram.createSocket('udp4');

  server.on('message', (buf) => {
    const csi = parseCsiUdp(buf);
    if (csi) detector.ingestCSI(csi.nodeId, csi.timestamp, csi.amplitudes);
  });

  setInterval(() => {
    const results = detector.analyze(Date.now() / 1000);

    if (JSON_OUTPUT) {
      for (const result of Object.values(results)) {
        console.log(JSON.stringify(result));
      }
    } else {
      process.stdout.write('\x1B[2J\x1B[H');
      console.log('=== MATERIAL/OBJECT DETECTOR (ADR-077) ===\n');

      for (const [nodeId, tracker] of detector.trackers) {
        if (!tracker.baselineEstablished) {
          const elapsed = tracker.lastTime ? tracker.lastTime - tracker.startTime : 0;
          console.log(`Node ${nodeId}: establishing baseline... ${elapsed.toFixed(0)}/${BASELINE_SEC}s`);
        } else {
          console.log(`Node ${nodeId}: ${tracker.renderNullMap()}`);
          console.log(`  Baseline: ${tracker.baselineNulls.size} nulls | Current: ${[...Array(tracker.nSc)].filter((_, i) => tracker.currentAmps[i] < NULL_THRESHOLD).length} nulls`);
        }
      }

      if (detector.allEvents.length > 0) {
        console.log('\nRecent events:');
        for (const e of detector.allEvents.slice(-5)) {
          const ts = new Date(e.timestamp * 1000).toISOString().slice(11, 19);
          console.log(`  [${ts}] ${e.eventType} | ${e.material} | delta ${e.nullDelta}`);
        }
      }

      console.log(`\nTotal events: ${detector.allEvents.length}`);
    }
  }, INTERVAL_MS);

  server.bind(PORT, () => {
    if (!JSON_OUTPUT) {
      console.log(`Material Detector listening on UDP :${PORT} (baseline: ${BASELINE_SEC}s)`);
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
