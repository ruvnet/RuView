#!/usr/bin/env node
/**
 * ADR-077: Room Environment Fingerprinting
 *
 * Clusters CSI feature vectors to identify distinct room states (empty,
 * working, sleeping, etc.), tracks transitions, and detects anomalies.
 *
 * Usage:
 *   node scripts/room-fingerprint.js --replay data/recordings/overnight-1775217646.csi.jsonl
 *   node scripts/room-fingerprint.js --port 5006
 *   node scripts/room-fingerprint.js --replay FILE --json
 *
 * ADR: docs/adr/ADR-077-novel-rf-sensing-applications.md
 */

'use strict';

const dgram = require('dgram');
const fs = require('fs');
const readline = require('readline');
const { parseArgs } = require('util');

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------
const { values: args } = parseArgs({
  options: {
    port:     { type: 'string', short: 'p', default: '5006' },
    replay:   { type: 'string', short: 'r' },
    json:     { type: 'boolean', default: false },
    interval: { type: 'string', short: 'i', default: '10000' },
    'k':      { type: 'string', default: '5' },
    'new-cluster-threshold': { type: 'string', default: '2.0' },
  },
  strict: true,
});

const PORT             = parseInt(args.port, 10);
const JSON_OUTPUT      = args.json;
const INTERVAL_MS      = parseInt(args.interval, 10);
const K                = parseInt(args.k, 10);
const NEW_CLUSTER_DIST = parseFloat(args['new-cluster-threshold']);

// ---------------------------------------------------------------------------
// ADR-018 packet constants
// ---------------------------------------------------------------------------
const VITALS_MAGIC  = 0xC5110002;
const FEATURE_MAGIC = 0xC5110003;
const FUSED_MAGIC   = 0xC5110004;

// ---------------------------------------------------------------------------
// Online k-means clustering
// ---------------------------------------------------------------------------
class OnlineKMeans {
  constructor(maxK, featureDim, newClusterThreshold) {
    this.maxK = maxK;
    this.dim = featureDim;
    this.threshold = newClusterThreshold;

    this.centroids = [];  // { center: Float64Array, count: number, label: string }
    this.alpha = 0.01;    // EMA update rate
  }

  _distance(a, b) {
    let sum = 0;
    const len = Math.min(a.length, b.length);
    for (let i = 0; i < len; i++) {
      sum += (a[i] - b[i]) ** 2;
    }
    return Math.sqrt(sum);
  }

  assign(features) {
    if (this.centroids.length === 0) {
      // First point creates first cluster
      this.centroids.push({
        center: Float64Array.from(features),
        count: 1,
        label: `State-0`,
      });
      return { clusterId: 0, distance: 0 };
    }

    // Find nearest centroid
    let bestDist = Infinity;
    let bestIdx = 0;
    for (let i = 0; i < this.centroids.length; i++) {
      const d = this._distance(features, this.centroids[i].center);
      if (d < bestDist) {
        bestDist = d;
        bestIdx = i;
      }
    }

    // If too far from any cluster, create new one (up to maxK)
    if (bestDist > this.threshold && this.centroids.length < this.maxK) {
      const newIdx = this.centroids.length;
      this.centroids.push({
        center: Float64Array.from(features),
        count: 1,
        label: `State-${newIdx}`,
      });
      return { clusterId: newIdx, distance: 0 };
    }

    // Update centroid via EMA
    const c = this.centroids[bestIdx];
    c.count++;
    for (let i = 0; i < this.dim; i++) {
      c.center[i] = c.center[i] * (1 - this.alpha) + features[i] * this.alpha;
    }

    return { clusterId: bestIdx, distance: bestDist };
  }

  labelClusters(clusterMotion) {
    // Sort clusters by average motion to assign labels
    const sorted = Object.entries(clusterMotion)
      .sort((a, b) => a[1] - b[1]);

    const labels = ['sleeping/empty', 'resting', 'working', 'active', 'highly active'];
    for (let i = 0; i < sorted.length; i++) {
      const clusterId = parseInt(sorted[i][0], 10);
      if (clusterId < this.centroids.length) {
        this.centroids[clusterId].label = labels[Math.min(i, labels.length - 1)];
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Room state tracker
// ---------------------------------------------------------------------------
class RoomFingerprinter {
  constructor(maxK, featureDim, newClusterThreshold) {
    this.kmeans = new OnlineKMeans(maxK, featureDim, newClusterThreshold);
    this.featureDim = featureDim;

    // State tracking
    this.currentState = null;
    this.stateHistory = [];     // { timestamp, clusterId, label, distance }
    this.transitions = {};      // "from->to" -> count

    // Vitals correlation
    this.clusterMotionSum = {};  // clusterId -> sum
    this.clusterMotionCount = {}; // clusterId -> count

    // Feature buffer (latest per node)
    this.latestFeatures = new Map(); // nodeId -> { timestamp, features }
    this.latestVitals = new Map();   // nodeId -> { timestamp, motion, presence }

    this.startTime = null;
  }

  pushFeature(timestamp, nodeId, features) {
    if (!this.startTime) this.startTime = timestamp;
    this.latestFeatures.set(nodeId, { timestamp, features });
  }

  pushVitals(timestamp, nodeId, motion, presence) {
    this.latestVitals.set(nodeId, { timestamp, motion, presence });
  }

  analyze(timestamp) {
    // Find latest feature vector (prefer most recent node)
    let bestFeature = null;
    let bestTs = 0;
    for (const [, entry] of this.latestFeatures) {
      if (entry.timestamp > bestTs) {
        bestTs = entry.timestamp;
        bestFeature = entry.features;
      }
    }

    if (!bestFeature || bestFeature.length < this.featureDim) return null;

    // Truncate or pad to featureDim
    const features = new Float64Array(this.featureDim);
    for (let i = 0; i < this.featureDim && i < bestFeature.length; i++) {
      features[i] = bestFeature[i];
    }

    // Assign to cluster
    const { clusterId, distance } = this.kmeans.assign(features);

    // Track motion per cluster for labeling
    let avgMotion = 0;
    let motionCount = 0;
    for (const [, v] of this.latestVitals) {
      avgMotion += v.motion;
      motionCount++;
    }
    avgMotion = motionCount > 0 ? avgMotion / motionCount : 0;

    this.clusterMotionSum[clusterId] = (this.clusterMotionSum[clusterId] || 0) + avgMotion;
    this.clusterMotionCount[clusterId] = (this.clusterMotionCount[clusterId] || 0) + 1;

    // Update labels periodically
    const clusterMotion = {};
    for (const id of Object.keys(this.clusterMotionCount)) {
      clusterMotion[id] = this.clusterMotionSum[id] / this.clusterMotionCount[id];
    }
    this.kmeans.labelClusters(clusterMotion);

    const label = this.kmeans.centroids[clusterId]
      ? this.kmeans.centroids[clusterId].label
      : `State-${clusterId}`;

    // Track transitions
    if (this.currentState !== null && this.currentState !== clusterId) {
      const key = `${this.currentState}->${clusterId}`;
      this.transitions[key] = (this.transitions[key] || 0) + 1;
    }
    const prevState = this.currentState;
    this.currentState = clusterId;

    const entry = {
      timestamp,
      clusterId,
      label,
      distance: +distance.toFixed(4),
      motion: +avgMotion.toFixed(3),
      transitioned: prevState !== null && prevState !== clusterId,
      prevState: prevState !== null ? prevState : undefined,
      totalClusters: this.kmeans.centroids.length,
    };

    this.stateHistory.push(entry);
    return entry;
  }

  anomalyScore() {
    // Anomaly = current state is rarely seen at this time-of-day
    if (this.stateHistory.length < 10) return 0;

    const currentCluster = this.currentState;
    const recentCount = this.stateHistory.slice(-20).filter(e => e.clusterId === currentCluster).length;
    return 1 - (recentCount / 20); // low count = high anomaly
  }

  renderTimeline(width) {
    const w = width || 60;
    if (this.stateHistory.length === 0) return 'No data yet.';

    const step = Math.max(1, Math.floor(this.stateHistory.length / w));
    const chars = '\u2581\u2582\u2583\u2584\u2585\u2586\u2587\u2588';

    let line = '';
    for (let i = 0; i < this.stateHistory.length; i += step) {
      const cid = this.stateHistory[i].clusterId;
      line += chars[Math.min(cid, chars.length - 1)];
    }

    return `State timeline: ${line}`;
  }

  renderTransitionMatrix() {
    if (Object.keys(this.transitions).length === 0) return 'No transitions yet.';

    const lines = ['Transition matrix:'];
    for (const [key, count] of Object.entries(this.transitions).sort((a, b) => b[1] - a[1])) {
      const [from, to] = key.split('->');
      const fromLabel = this.kmeans.centroids[parseInt(from, 10)]?.label || `State-${from}`;
      const toLabel = this.kmeans.centroids[parseInt(to, 10)]?.label || `State-${to}`;
      lines.push(`  ${fromLabel} -> ${toLabel}: ${count}`);
    }
    return lines.join('\n');
  }
}

// ---------------------------------------------------------------------------
// Packet parsing
// ---------------------------------------------------------------------------
function parseFeatureJsonl(record) {
  if (record.type !== 'feature' || !record.features) return null;
  return {
    timestamp: record.timestamp,
    nodeId: record.node_id,
    features: record.features,
  };
}

function parseVitalsJsonl(record) {
  if (record.type !== 'vitals') return null;
  return {
    timestamp: record.timestamp,
    nodeId: record.node_id,
    motion: record.motion_energy || 0,
    presence: record.presence_score || 0,
  };
}

function parseFeatureUdp(buf) {
  if (buf.length < 48) return null;
  const magic = buf.readUInt32LE(0);
  if (magic !== FEATURE_MAGIC) return null;

  const nodeId = buf.readUInt8(4);
  const features = [];
  for (let i = 0; i < 8; i++) {
    features.push(buf.readFloatLE(16 + i * 4));
  }
  return { timestamp: Date.now() / 1000, nodeId, features };
}

function parseVitalsUdp(buf) {
  if (buf.length < 32) return null;
  const magic = buf.readUInt32LE(0);
  if (magic !== VITALS_MAGIC && magic !== FUSED_MAGIC) return null;
  return {
    timestamp: Date.now() / 1000,
    nodeId: buf.readUInt8(4),
    motion: buf.readFloatLE(16),
    presence: buf.readFloatLE(20),
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

  const fingerprinter = new RoomFingerprinter(K, 8, NEW_CLUSTER_DIST);
  const rl = readline.createInterface({
    input: fs.createReadStream(filePath),
    crlfDelay: Infinity,
  });

  let featureCount = 0;
  let vitalsCount = 0;
  let lastAnalysisTs = 0;

  for await (const line of rl) {
    if (!line.trim()) continue;
    let record;
    try { record = JSON.parse(line); } catch { continue; }

    const feat = parseFeatureJsonl(record);
    if (feat) {
      fingerprinter.pushFeature(feat.timestamp, feat.nodeId, feat.features);
      featureCount++;
    }

    const vit = parseVitalsJsonl(record);
    if (vit) {
      fingerprinter.pushVitals(vit.timestamp, vit.nodeId, vit.motion, vit.presence);
      vitalsCount++;
    }

    const ts = feat || vit;
    if (!ts) continue;

    const tsMs = ts.timestamp * 1000;
    if (lastAnalysisTs === 0) lastAnalysisTs = tsMs;

    if (tsMs - lastAnalysisTs >= INTERVAL_MS) {
      const result = fingerprinter.analyze(ts.timestamp);

      if (result) {
        if (JSON_OUTPUT) {
          console.log(JSON.stringify(result));
        } else {
          const tsStr = new Date(ts.timestamp * 1000).toISOString().slice(11, 19);
          const transition = result.transitioned ? ` << TRANSITION from State-${result.prevState}` : '';
          console.log(`[${tsStr}] Cluster ${result.clusterId} (${result.label}) | dist ${result.distance} | motion ${result.motion} | ${result.totalClusters} clusters${transition}`);
        }
      }

      lastAnalysisTs = tsMs;
    }
  }

  // Summary
  if (!JSON_OUTPUT) {
    console.log('\n' + '='.repeat(60));
    console.log('ROOM FINGERPRINT SUMMARY');
    console.log('='.repeat(60));

    console.log(`\nClusters discovered: ${fingerprinter.kmeans.centroids.length}`);
    for (let i = 0; i < fingerprinter.kmeans.centroids.length; i++) {
      const c = fingerprinter.kmeans.centroids[i];
      const stateCount = fingerprinter.stateHistory.filter(e => e.clusterId === i).length;
      const pct = fingerprinter.stateHistory.length > 0
        ? ((stateCount / fingerprinter.stateHistory.length) * 100).toFixed(1)
        : '0';
      const avgMotion = fingerprinter.clusterMotionCount[i] > 0
        ? (fingerprinter.clusterMotionSum[i] / fingerprinter.clusterMotionCount[i]).toFixed(2)
        : '?';
      console.log(`  Cluster ${i} (${c.label}): ${stateCount} windows (${pct}%) | avg motion ${avgMotion} | ${c.count} assignments`);
    }

    console.log('');
    console.log(fingerprinter.renderTimeline(60));
    console.log('');
    console.log(fingerprinter.renderTransitionMatrix());

    const anomaly = fingerprinter.anomalyScore();
    console.log(`\nCurrent anomaly score: ${anomaly.toFixed(3)}`);
    console.log(`Processed: ${featureCount} feature packets, ${vitalsCount} vitals packets`);
  } else {
    console.log(JSON.stringify({
      type: 'summary',
      clusters: fingerprinter.kmeans.centroids.length,
      windows: fingerprinter.stateHistory.length,
      transitions: Object.keys(fingerprinter.transitions).length,
      anomaly: +fingerprinter.anomalyScore().toFixed(3),
    }));
  }
}

// ---------------------------------------------------------------------------
// Live UDP mode
// ---------------------------------------------------------------------------
function startLive() {
  const fingerprinter = new RoomFingerprinter(K, 8, NEW_CLUSTER_DIST);
  const server = dgram.createSocket('udp4');

  server.on('message', (buf) => {
    if (buf.length < 4) return;
    const magic = buf.readUInt32LE(0);

    if (magic === FEATURE_MAGIC) {
      const feat = parseFeatureUdp(buf);
      if (feat) fingerprinter.pushFeature(feat.timestamp, feat.nodeId, feat.features);
    }
    if (magic === VITALS_MAGIC || magic === FUSED_MAGIC) {
      const vit = parseVitalsUdp(buf);
      if (vit) fingerprinter.pushVitals(vit.timestamp, vit.nodeId, vit.motion, vit.presence);
    }
  });

  setInterval(() => {
    const result = fingerprinter.analyze(Date.now() / 1000);

    if (JSON_OUTPUT) {
      if (result) console.log(JSON.stringify(result));
    } else {
      process.stdout.write('\x1B[2J\x1B[H');
      console.log('=== ROOM FINGERPRINT (ADR-077) ===\n');

      if (result) {
        console.log(`Current state: Cluster ${result.clusterId} (${result.label})`);
        console.log(`Distance: ${result.distance} | Motion: ${result.motion}`);
        console.log(`Clusters: ${result.totalClusters}`);
        if (result.transitioned) {
          console.log(`** STATE TRANSITION from State-${result.prevState} **`);
        }
      } else {
        console.log('Collecting data...');
      }

      console.log('');
      console.log(fingerprinter.renderTimeline(50));
      console.log('');
      console.log(fingerprinter.renderTransitionMatrix());
      console.log(`\nAnomaly score: ${fingerprinter.anomalyScore().toFixed(3)}`);
    }
  }, INTERVAL_MS);

  server.bind(PORT, () => {
    if (!JSON_OUTPUT) {
      console.log(`Room Fingerprint listening on UDP :${PORT} (k=${K})`);
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
