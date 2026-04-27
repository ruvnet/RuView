#!/usr/bin/env node
/**
 * Ground-Truth Alignment — Camera Keypoints <-> CSI Recording
 *
 * Time-aligns camera keypoint data with CSI recording data to produce
 * paired training samples for WiFlow supervised training (ADR-079).
 *
 * Camera keypoints:  data/ground-truth/gt-{timestamp}.jsonl
 * CSI recordings:    data/recordings/*.csi.jsonl
 * Paired output:     data/paired/*.paired.jsonl
 *
 * Usage:
 *   node scripts/align-ground-truth.js \
 *     --gt data/ground-truth/gt-1775300000.jsonl \
 *     --csi data/recordings/overnight-1775217646.csi.jsonl \
 *     --output data/paired/aligned.paired.jsonl
 *
 *   # With clock offset correction (camera ahead by 50ms)
 *   node scripts/align-ground-truth.js \
 *     --gt data/ground-truth/gt-1775300000.jsonl \
 *     --csi data/recordings/overnight-1775217646.csi.jsonl \
 *     --clock-offset-ms -50
 *
 * ADR: docs/adr/ADR-079
 */

'use strict';

const fs = require('fs');
const path = require('path');
const { parseArgs } = require('util');

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------
const { values: args } = parseArgs({
  options: {
    gt:                  { type: 'string' },
    csi:                 { type: 'string' },
    output:              { type: 'string', short: 'o' },
    'window-ms':         { type: 'string', default: '200' },
    'window-frames':     { type: 'string', default: '20' },
    'min-camera-frames': { type: 'string', default: '3' },
    'min-confidence':    { type: 'string', default: '0.5' },
    'clock-offset-ms':   { type: 'string', default: '0' },
    help:                { type: 'boolean', short: 'h', default: false },
  },
  strict: true,
});

if (args.help || !args.gt || !args.csi) {
  console.log(`
Usage: node scripts/align-ground-truth.js --gt <gt.jsonl> --csi <csi.jsonl> [options]

Required:
  --gt <path>               Camera ground-truth JSONL file
  --csi <path>              CSI recording JSONL file

Options:
  --output, -o <path>       Output paired JSONL (default: data/paired/<basename>.paired.jsonl)
  --window-ms <ms>          CSI window size in ms (default: 200)
  --window-frames <n>       Frames per CSI window (default: 20)
  --min-camera-frames <n>   Minimum camera frames per window (default: 3)
  --min-confidence <f>      Minimum average confidence threshold (default: 0.5)
  --clock-offset-ms <ms>    Manual clock offset: added to camera timestamps (default: 0)
  --help, -h                Show this help
`);
  process.exit(args.help ? 0 : 1);
}

const WINDOW_FRAMES     = parseInt(args['window-frames'], 10);
const WINDOW_MS         = parseInt(args['window-ms'], 10);
const MIN_CAMERA_FRAMES = parseInt(args['min-camera-frames'], 10);
const MIN_CONFIDENCE    = parseFloat(args['min-confidence']);
const CLOCK_OFFSET_MS   = parseFloat(args['clock-offset-ms']);
const NUM_KEYPOINTS     = 17; // COCO 17-keypoint format

// ---------------------------------------------------------------------------
// Timestamp conversion
// ---------------------------------------------------------------------------

/**
 * Convert camera nanosecond timestamp to milliseconds.
 * Applies clock offset correction.
 */
function cameraTsToMs(tsNs) {
  return tsNs / 1e6 + CLOCK_OFFSET_MS;
}

/**
 * Convert ISO 8601 timestamp string to milliseconds since epoch.
 */
function isoToMs(isoStr) {
  return new Date(isoStr).getTime();
}

// ---------------------------------------------------------------------------
// IQ hex parsing (matches train-wiflow.js conventions)
// ---------------------------------------------------------------------------

/**
 * Parse IQ hex string into signed byte pairs [I0, Q0, I1, Q1, ...].
 */
function parseIqHex(hexStr) {
  const bytes = [];
  for (let i = 0; i < hexStr.length; i += 2) {
    let val = parseInt(hexStr.substr(i, 2), 16);
    if (val > 127) val -= 256; // signed byte
    bytes.push(val);
  }
  return bytes;
}

/**
 * Extract amplitude from IQ data for a given number of subcarriers.
 * Returns Float32Array of amplitudes [nSubcarriers].
 * Skips first I/Q pair (DC offset) per WiFlow paper recommendation.
 */
function extractAmplitude(iqBytes, nSubcarriers) {
  const amp = new Float32Array(nSubcarriers);
  const start = 2; // skip first IQ pair (DC offset)
  for (let sc = 0; sc < nSubcarriers; sc++) {
    const idx = start + sc * 2;
    if (idx + 1 < iqBytes.length) {
      const I = iqBytes[idx];
      const Q = iqBytes[idx + 1];
      amp[sc] = Math.sqrt(I * I + Q * Q);
    }
  }
  return amp;
}

// ---------------------------------------------------------------------------
// File loading
// ---------------------------------------------------------------------------

/**
 * Load and parse a JSONL file, skipping blank/malformed lines.
 */
function loadJsonl(filePath) {
  const lines = fs.readFileSync(filePath, 'utf8').split('\n');
  const records = [];
  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    try {
      records.push(JSON.parse(trimmed));
    } catch {
      // skip malformed lines
    }
  }
  return records;
}

/**
 * Load camera ground-truth file.
 * Returns array of { tsMs, keypoints, confidence, nVisible, nPersons }.
 */
function loadGroundTruth(filePath) {
  const raw = loadJsonl(filePath);
  const frames = [];
  for (const r of raw) {
    if (r.ts_ns == null || !r.keypoints) continue;
    frames.push({
      tsMs: cameraTsToMs(r.ts_ns),
      keypoints: r.keypoints,
      confidence: r.confidence ?? 0,
      nVisible: r.n_visible ?? 0,
      nPersons: r.n_persons ?? 1,
    });
  }
  // Sort by timestamp
  frames.sort((a, b) => a.tsMs - b.tsMs);
  return frames;
}

/**
 * Load CSI recording file.
 * Separates raw_csi frames and feature frames.
 */
function loadCsi(filePath) {
  const raw = loadJsonl(filePath);
  const rawCsi = [];
  const features = [];

  for (const r of raw) {
    if (!r.timestamp) continue;
    const tsMs = isoToMs(r.timestamp);
    if (isNaN(tsMs)) continue;

    if (r.type === 'raw_csi') {
      rawCsi.push({
        tsMs,
        nodeId: r.node_id,
        subcarriers: r.subcarriers ?? 128,
        iqHex: r.iq_hex,
        rssi: r.rssi,
        seq: r.seq,
      });
    } else if (r.type === 'feature') {
      features.push({
        tsMs,
        nodeId: r.node_id,
        features: r.features,
        rssi: r.rssi,
        seq: r.seq,
      });
    }
  }

  // Sort by timestamp
  rawCsi.sort((a, b) => a.tsMs - b.tsMs);
  features.sort((a, b) => a.tsMs - b.tsMs);
  return { rawCsi, features };
}

// ---------------------------------------------------------------------------
// Windowing
// ---------------------------------------------------------------------------

/**
 * Group frames into non-overlapping windows of `windowSize` consecutive frames.
 */
function groupIntoWindows(frames, windowSize) {
  const windows = [];
  for (let i = 0; i + windowSize <= frames.length; i += windowSize) {
    windows.push(frames.slice(i, i + windowSize));
  }
  return windows;
}

// ---------------------------------------------------------------------------
// Camera frame matching (binary search)
// ---------------------------------------------------------------------------

/**
 * Find all camera frames within [tStart, tEnd] using binary search.
 */
function findCameraFramesInRange(cameraFrames, tStartMs, tEndMs) {
  // Binary search for first frame >= tStartMs
  let lo = 0;
  let hi = cameraFrames.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (cameraFrames[mid].tsMs < tStartMs) lo = mid + 1;
    else hi = mid;
  }

  const matched = [];
  for (let i = lo; i < cameraFrames.length; i++) {
    if (cameraFrames[i].tsMs > tEndMs) break;
    matched.push(cameraFrames[i]);
  }
  return matched;
}

// ---------------------------------------------------------------------------
// Keypoint averaging (confidence-weighted)
// ---------------------------------------------------------------------------

/**
 * Average keypoints weighted by per-frame confidence.
 * Returns { keypoints: [[x,y],...], avgConfidence }.
 */
function averageKeypoints(cameraFrames) {
  let totalWeight = 0;
  const sumKp = new Array(NUM_KEYPOINTS).fill(null).map(() => [0, 0]);

  for (const f of cameraFrames) {
    const w = f.confidence || 1e-6;
    totalWeight += w;
    for (let k = 0; k < NUM_KEYPOINTS && k < f.keypoints.length; k++) {
      sumKp[k][0] += f.keypoints[k][0] * w;
      sumKp[k][1] += f.keypoints[k][1] * w;
    }
  }

  if (totalWeight === 0) totalWeight = 1;
  const keypoints = sumKp.map(([x, y]) => [x / totalWeight, y / totalWeight]);
  const avgConfidence = cameraFrames.reduce((s, f) => s + (f.confidence || 0), 0) / cameraFrames.length;

  return { keypoints, avgConfidence };
}

// ---------------------------------------------------------------------------
// CSI matrix extraction
// ---------------------------------------------------------------------------

/**
 * Extract CSI amplitude matrix from raw_csi window.
 * Returns { data: flat Float32Array, shape: [subcarriers, windowFrames] }.
 */
function extractCsiMatrix(window) {
  const nFrames = window.length;
  const nSc = window[0].subcarriers || 128;
  const matrix = new Float32Array(nSc * nFrames);

  for (let f = 0; f < nFrames; f++) {
    const frame = window[f];
    if (frame.iqHex) {
      const iq = parseIqHex(frame.iqHex);
      const amp = extractAmplitude(iq, nSc);
      matrix.set(amp, f * nSc);
    }
  }

  return { data: Array.from(matrix), shape: [nSc, nFrames] };
}

/**
 * Extract feature matrix from feature-type window.
 * Returns { data: flat array, shape: [featureDim, windowFrames] }.
 */
function extractFeatureMatrix(window) {
  const nFrames = window.length;
  const dim = window[0].features ? window[0].features.length : 8;
  const matrix = new Float32Array(dim * nFrames);

  for (let f = 0; f < nFrames; f++) {
    const feats = window[f].features || new Array(dim).fill(0);
    for (let d = 0; d < dim; d++) {
      matrix[f * dim + d] = feats[d] || 0;
    }
  }

  return { data: Array.from(matrix), shape: [dim, nFrames] };
}

// ---------------------------------------------------------------------------
// Main alignment
// ---------------------------------------------------------------------------

function align() {
  const gtPath = path.resolve(args.gt);
  const csiPath = path.resolve(args.csi);

  // Determine output path
  let outputPath;
  if (args.output) {
    outputPath = path.resolve(args.output);
  } else {
    const baseName = path.basename(csiPath, '.csi.jsonl');
    outputPath = path.resolve('data', 'paired', `${baseName}.paired.jsonl`);
  }

  // Ensure output directory exists
  const outputDir = path.dirname(outputPath);
  if (!fs.existsSync(outputDir)) {
    fs.mkdirSync(outputDir, { recursive: true });
  }

  console.log('=== Ground-Truth Alignment (ADR-079) ===');
  console.log(`  GT file:          ${gtPath}`);
  console.log(`  CSI file:         ${csiPath}`);
  console.log(`  Output:           ${outputPath}`);
  console.log(`  Window:           ${WINDOW_FRAMES} frames / ${WINDOW_MS} ms`);
  console.log(`  Min camera frames: ${MIN_CAMERA_FRAMES}`);
  console.log(`  Min confidence:   ${MIN_CONFIDENCE}`);
  console.log(`  Clock offset:     ${CLOCK_OFFSET_MS} ms`);
  console.log();

  // Load data
  console.log('Loading ground-truth...');
  const cameraFrames = loadGroundTruth(gtPath);
  console.log(`  ${cameraFrames.length} camera frames loaded`);
  if (cameraFrames.length > 0) {
    console.log(`  Time range: ${new Date(cameraFrames[0].tsMs).toISOString()} -> ${new Date(cameraFrames[cameraFrames.length - 1].tsMs).toISOString()}`);
  }

  console.log('Loading CSI data...');
  const { rawCsi, features } = loadCsi(csiPath);
  console.log(`  ${rawCsi.length} raw_csi frames, ${features.length} feature frames`);

  // Decide which CSI source to use
  const useRawCsi = rawCsi.length >= WINDOW_FRAMES;
  const csiSource = useRawCsi ? rawCsi : features;
  const sourceLabel = useRawCsi ? 'raw_csi' : 'feature';

  if (csiSource.length < WINDOW_FRAMES) {
    console.error(`ERROR: Not enough CSI frames (${csiSource.length}) for even one window of ${WINDOW_FRAMES} frames.`);
    process.exit(1);
  }

  console.log(`  Using ${sourceLabel} frames (${csiSource.length} total)`);
  if (csiSource.length > 0) {
    console.log(`  CSI time range: ${new Date(csiSource[0].tsMs).toISOString()} -> ${new Date(csiSource[csiSource.length - 1].tsMs).toISOString()}`);
  }
  console.log();

  // Group CSI into windows
  const windows = groupIntoWindows(csiSource, WINDOW_FRAMES);
  console.log(`Grouped into ${windows.length} CSI windows`);

  // Align
  const paired = [];
  let totalConfidence = 0;

  for (const window of windows) {
    const tStartMs = window[0].tsMs;
    const tEndMs = window[window.length - 1].tsMs;

    // Expand window if actual time span is smaller than window-ms
    const halfWindow = WINDOW_MS / 2;
    const midpoint = (tStartMs + tEndMs) / 2;
    const searchStart = Math.min(tStartMs, midpoint - halfWindow);
    const searchEnd = Math.max(tEndMs, midpoint + halfWindow);

    // Find matching camera frames
    const matched = findCameraFramesInRange(cameraFrames, searchStart, searchEnd);

    if (matched.length < MIN_CAMERA_FRAMES) continue;

    // Check average confidence
    const avgConf = matched.reduce((s, f) => s + (f.confidence || 0), 0) / matched.length;
    if (avgConf < MIN_CONFIDENCE) continue;

    // Average keypoints weighted by confidence
    const { keypoints, avgConfidence } = averageKeypoints(matched);

    // Extract CSI matrix
    const csiMatrix = useRawCsi
      ? extractCsiMatrix(window)
      : extractFeatureMatrix(window);

    paired.push({
      csi: csiMatrix.data,
      csi_shape: csiMatrix.shape,
      kp: keypoints,
      conf: Math.round(avgConfidence * 1000) / 1000,
      n_camera_frames: matched.length,
      ts_start: new Date(tStartMs).toISOString(),
      ts_end: new Date(tEndMs).toISOString(),
    });

    totalConfidence += avgConfidence;
  }

  // Write output
  const outputLines = paired.map(s => JSON.stringify(s));
  fs.writeFileSync(outputPath, outputLines.join('\n') + (outputLines.length > 0 ? '\n' : ''));

  // Print summary
  const alignmentRate = windows.length > 0 ? (paired.length / windows.length * 100) : 0;
  const avgPairedConf = paired.length > 0 ? (totalConfidence / paired.length) : 0;

  console.log();
  console.log('=== Alignment Summary ===');
  console.log(`  Total CSI windows:       ${windows.length}`);
  console.log(`  Paired samples:          ${paired.length}`);
  console.log(`  Alignment rate:          ${alignmentRate.toFixed(1)}%`);
  console.log(`  Avg confidence (paired): ${avgPairedConf.toFixed(3)}`);
  console.log(`  CSI source:              ${sourceLabel} (${csiMatrix_shapeLabel(paired, useRawCsi)})`);
  if (paired.length > 0) {
    console.log(`  Time range covered:      ${paired[0].ts_start} -> ${paired[paired.length - 1].ts_end}`);
  }
  console.log(`  Output written:          ${outputPath}`);
  console.log();

  if (paired.length === 0) {
    console.log('WARNING: No paired samples produced. Check that camera and CSI time ranges overlap.');
    console.log('  Hint: Use --clock-offset-ms to correct misaligned clocks.');
  }
}

/**
 * Format CSI matrix shape label for summary.
 */
function csiMatrix_shapeLabel(paired, useRawCsi) {
  if (paired.length === 0) return useRawCsi ? `[128, ${WINDOW_FRAMES}]` : `[8, ${WINDOW_FRAMES}]`;
  const shape = paired[0].csi_shape;
  return `[${shape[0]}, ${shape[1]}]`;
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------
align();
