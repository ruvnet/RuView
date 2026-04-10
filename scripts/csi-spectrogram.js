#!/usr/bin/env node
/**
 * ADR-076: CSI Spectrogram Embedding Pipeline
 *
 * Converts raw CSI frames into 128-dim CNN embeddings by treating the
 * subcarrier x time matrix as a grayscale spectrogram image.
 *
 * Modes:
 *   --live          Listen on UDP for real-time CSI frames
 *   --file FILE     Read from a .csi.jsonl recording
 *   --ascii         Print ASCII spectrogram visualization
 *   --ingest        Send 128-dim embeddings to Cognitum Seed
 *   --knn K         Find K most similar past spectrograms
 *
 * Usage:
 *   node scripts/csi-spectrogram.js --file data/recordings/pretrain-1775182186.csi.jsonl --ascii
 *   node scripts/csi-spectrogram.js --live --port 5006 --ingest --seed-url https://169.254.42.1:8443
 *   node scripts/csi-spectrogram.js --file data/recordings/pretrain-1775182186.csi.jsonl --knn 5
 *
 * ADR: docs/adr/ADR-076-csi-spectrogram-embeddings.md
 */

'use strict';

const dgram = require('dgram');
const fs = require('fs');
const path = require('path');
const readline = require('readline');
const { parseArgs } = require('util');
const { parseRawCsiFrame } = require('./lib/raw-csi');

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------
const { values: args } = parseArgs({
  options: {
    file:     { type: 'string', short: 'f' },
    live:     { type: 'boolean', default: false },
    port:     { type: 'string', short: 'p', default: '5006' },
    ascii:    { type: 'boolean', default: false },
    ingest:   { type: 'boolean', default: false },
    knn:      { type: 'string', short: 'k' },
    'seed-url':  { type: 'string', default: 'https://169.254.42.1:8443' },
    'seed-token': { type: 'string', default: '' },
    window:   { type: 'string', short: 'w', default: '20' },
    stride:   { type: 'string', short: 's', default: '10' },
    dim:      { type: 'string', short: 'd', default: '128' },
    json:     { type: 'boolean', default: false },
    limit:    { type: 'string', short: 'l' },
  },
  strict: true,
});

const WINDOW_SIZE = parseInt(args.window, 10);   // frames per spectrogram
const STRIDE = parseInt(args.stride, 10);         // frames between windows
const EMBED_DIM = parseInt(args.dim, 10);         // CNN output dimension
const KNN_K = args.knn ? parseInt(args.knn, 10) : 0;
const LIMIT = args.limit ? parseInt(args.limit, 10) : Infinity;
const PORT = parseInt(args.port, 10);
const JSON_OUTPUT = args.json;

// ADR-018 raw CSI parser

// CNN input size (ruvector/cnn expects 224x224 RGB)
const CNN_INPUT_SIZE = 224;

// ASCII visualization characters (8 intensity levels)
const BARS = [' ', '\u2581', '\u2582', '\u2583', '\u2584', '\u2585', '\u2586', '\u2587', '\u2588'];

// ---------------------------------------------------------------------------
// IQ Hex Parsing
// ---------------------------------------------------------------------------

/**
 * Parse iq_hex string into subcarrier amplitudes.
 * Format: 4 hex chars per subcarrier (I byte + Q byte).
 * @param {string} iqHex - Hex-encoded I/Q data
 * @param {number} nSubcarriers - Expected number of subcarriers
 * @returns {Float32Array} Amplitude per subcarrier
 */
function parseIqHex(iqHex, nSubcarriers) {
  const amps = new Float32Array(nSubcarriers);
  for (let sc = 0; sc < nSubcarriers; sc++) {
    const offset = sc * 4;
    if (offset + 4 > iqHex.length) break;
    const iVal = parseInt(iqHex.substring(offset, offset + 2), 16);
    const qVal = parseInt(iqHex.substring(offset + 2, offset + 4), 16);
    amps[sc] = Math.sqrt(iVal * iVal + qVal * qVal);
  }
  return amps;
}

/**
 * Parse an ADR-018 binary UDP packet into subcarrier amplitudes.
 * @param {Buffer} buf - Raw UDP packet
 * @returns {{ nodeId: number, rssi: number, nSubcarriers: number, amplitudes: Float32Array } | null}
 */
function parseBinaryFrame(buf) {
  const frame = parseRawCsiFrame(buf, { includePhase: false });
  if (!frame) return null;
  return {
    nodeId: frame.nodeId,
    rssi: frame.rssi,
    nSubcarriers: frame.nSubcarriers,
    amplitudes: Float32Array.from(frame.amplitudes),
  };
}

// ---------------------------------------------------------------------------
// Spectrogram Window
// ---------------------------------------------------------------------------

class SpectrogramWindow {
  /**
   * @param {number} nSubcarriers - Number of subcarriers per frame
   * @param {number} windowSize - Number of time frames per window
   */
  constructor(nSubcarriers, windowSize) {
    this.nSubcarriers = nSubcarriers;
    this.windowSize = windowSize;
    /** @type {Float32Array[]} Ring buffer of amplitude vectors */
    this.frames = [];
    this.totalPushed = 0;
  }

  /** Push a new amplitude vector. */
  push(amplitudes) {
    if (amplitudes.length !== this.nSubcarriers) {
      // Pad or truncate to expected size
      const padded = new Float32Array(this.nSubcarriers);
      padded.set(amplitudes.subarray(0, Math.min(amplitudes.length, this.nSubcarriers)));
      this.frames.push(padded);
    } else {
      this.frames.push(new Float32Array(amplitudes));
    }
    if (this.frames.length > this.windowSize) {
      this.frames.shift();
    }
    this.totalPushed++;
  }

  /** @returns {boolean} True when window is full */
  isFull() {
    return this.frames.length >= this.windowSize;
  }

  /**
   * Get the subcarrier x time matrix as a flat grayscale image (0-255).
   * Layout: row-major, rows = subcarriers, cols = time frames.
   * @returns {{ pixels: Uint8Array, width: number, height: number }}
   */
  toGrayscale() {
    const h = this.nSubcarriers;
    const w = this.windowSize;
    const pixels = new Uint8Array(h * w);

    // Find min/max across entire window for normalization
    let min = Infinity;
    let max = -Infinity;
    for (let t = 0; t < w; t++) {
      const frame = this.frames[t];
      for (let sc = 0; sc < h; sc++) {
        const v = frame[sc];
        if (v < min) min = v;
        if (v > max) max = v;
      }
    }

    const range = max - min || 1;
    for (let sc = 0; sc < h; sc++) {
      for (let t = 0; t < w; t++) {
        const v = this.frames[t][sc];
        pixels[sc * w + t] = Math.round(255 * (v - min) / range);
      }
    }

    return { pixels, width: w, height: h };
  }

  /**
   * Upsample grayscale to CNN input size using nearest-neighbor interpolation.
   * Replicates to 3-channel RGB as required by @ruvector/cnn.
   * @returns {Uint8Array} RGB pixel data (CNN_INPUT_SIZE * CNN_INPUT_SIZE * 3)
   */
  toCnnInput() {
    const { pixels, width, height } = this.toGrayscale();
    const out = new Uint8Array(CNN_INPUT_SIZE * CNN_INPUT_SIZE * 3);

    for (let y = 0; y < CNN_INPUT_SIZE; y++) {
      const srcY = Math.min(Math.floor(y * height / CNN_INPUT_SIZE), height - 1);
      for (let x = 0; x < CNN_INPUT_SIZE; x++) {
        const srcX = Math.min(Math.floor(x * width / CNN_INPUT_SIZE), width - 1);
        const gray = pixels[srcY * width + srcX];
        const dstIdx = (y * CNN_INPUT_SIZE + x) * 3;
        out[dstIdx] = gray;
        out[dstIdx + 1] = gray;
        out[dstIdx + 2] = gray;
      }
    }

    return out;
  }
}

// ---------------------------------------------------------------------------
// ASCII Visualization
// ---------------------------------------------------------------------------

/**
 * Print an ASCII spectrogram of the current window.
 * Rows = subcarrier index (downsampled), columns = time.
 */
function printAsciiSpectrogram(window, meta = {}) {
  const { pixels, width, height } = window.toGrayscale();

  // Downsample rows to fit terminal (max 32 rows)
  const maxRows = Math.min(height, 32);
  const rowStep = Math.ceil(height / maxRows);

  const lines = [];
  lines.push(`--- Spectrogram [${height}sc x ${width}t] node=${meta.nodeId || '?'} rssi=${meta.rssi || '?'} ---`);

  for (let r = 0; r < maxRows; r++) {
    const sc = r * rowStep;
    const label = String(sc).padStart(3);
    let row = `sc${label} |`;
    for (let t = 0; t < width; t++) {
      const v = pixels[sc * width + t];
      const level = Math.min(Math.floor(v / 29), BARS.length - 1);
      row += BARS[level];
    }
    row += '|';
    lines.push(row);
  }

  lines.push(`       ${''.padStart(width + 2, '-')}`);
  lines.push(`       t=0${''.padStart(width - 6)}t=${width - 1}`);
  console.log(lines.join('\n'));
}

// ---------------------------------------------------------------------------
// CNN Embedding
// ---------------------------------------------------------------------------

let cnnEmbedder = null;
let cnnInitialized = false;

/**
 * Initialize the CNN embedder from vendor WASM.
 */
async function initCnn() {
  if (cnnInitialized) return;

  // Load WASM bindings directly to work around the CnnEmbedder wrapper bug:
  // The wrapper's constructor calls `new wasm.WasmCnnEmbedder(wasmConfig)` which
  // consumes (destroys) the EmbedderConfig pointer, then tries to read
  // `wasmConfig.embedding_dim` from the now-null pointer. We use the WASM
  // classes directly and track the dimension ourselves.
  const wasmPath = path.resolve(
    __dirname, '..', 'vendor', 'ruvector', 'npm', 'packages', 'ruvector-cnn'
  );
  const wasmModule = require(path.join(wasmPath, 'ruvector_cnn_wasm.js'));
  const wasmBuffer = fs.readFileSync(path.join(wasmPath, 'ruvector_cnn_wasm_bg.wasm'));
  await wasmModule.default(wasmBuffer);

  const config = new wasmModule.EmbedderConfig();
  config.input_size = CNN_INPUT_SIZE;
  config.embedding_dim = EMBED_DIM;
  config.normalize = true;

  // Save dim before construction (constructor consumes config)
  const savedDim = EMBED_DIM;
  const inner = new wasmModule.WasmCnnEmbedder(config);

  // Wrap in a compatible interface
  cnnEmbedder = {
    _inner: inner,
    embeddingDim: savedDim,
    extract(imageData, width, height) {
      return new Float32Array(inner.extract(imageData, width, height));
    },
    cosineSimilarity(a, b) {
      return inner.cosine_similarity(a, b);
    },
  };

  cnnInitialized = true;
  if (!JSON_OUTPUT) {
    console.log(`[cnn] Initialized: embeddingDim=${savedDim}, inputSize=${CNN_INPUT_SIZE}x${CNN_INPUT_SIZE}`);
  }
}

/**
 * Extract CNN embedding from a spectrogram window.
 * @param {SpectrogramWindow} window
 * @returns {Float32Array} 128-dim embedding
 */
function extractEmbedding(window) {
  const rgbPixels = window.toCnnInput();
  return cnnEmbedder.extract(rgbPixels, CNN_INPUT_SIZE, CNN_INPUT_SIZE);
}

// ---------------------------------------------------------------------------
// Embedding Store (in-memory kNN)
// ---------------------------------------------------------------------------

class EmbeddingStore {
  constructor() {
    /** @type {{ embedding: Float32Array, timestamp: number, nodeId: number, windowIdx: number }[]} */
    this.entries = [];
  }

  add(embedding, meta) {
    this.entries.push({ embedding, ...meta });
  }

  /**
   * Find k nearest neighbors by cosine similarity.
   * @param {Float32Array} query
   * @param {number} k
   * @returns {{ index: number, similarity: number, meta: object }[]}
   */
  knn(query, k) {
    const scores = this.entries.map((entry, index) => ({
      index,
      similarity: cosineSimilarity(query, entry.embedding),
      timestamp: entry.timestamp,
      nodeId: entry.nodeId,
      windowIdx: entry.windowIdx,
    }));
    scores.sort((a, b) => b.similarity - a.similarity);
    return scores.slice(0, k);
  }

  get size() { return this.entries.length; }
}

function cosineSimilarity(a, b) {
  let dot = 0, normA = 0, normB = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }
  const denom = Math.sqrt(normA) * Math.sqrt(normB);
  return denom > 0 ? dot / denom : 0;
}

// ---------------------------------------------------------------------------
// Cognitum Seed Ingest
// ---------------------------------------------------------------------------

/**
 * Send a 128-dim embedding to Cognitum Seed's RVF vector store.
 * @param {Float32Array} embedding
 * @param {object} meta
 */
async function ingestToSeed(embedding, meta) {
  const seedUrl = args['seed-url'];
  const token = args['seed-token'] || process.env.SEED_TOKEN;
  if (!token) {
    console.error('[seed] No token provided (--seed-token or $SEED_TOKEN)');
    return;
  }

  const https = require('https');
  const payload = JSON.stringify({
    store: 'csi-spectrograms',
    vectors: [{
      id: `spectrogram-${meta.nodeId}-${meta.windowIdx}`,
      values: Array.from(embedding),
      metadata: {
        node_id: meta.nodeId,
        timestamp: meta.timestamp,
        window_idx: meta.windowIdx,
        rssi: meta.rssi,
        subcarriers: meta.nSubcarriers,
      },
    }],
  });

  return new Promise((resolve, reject) => {
    const url = new URL('/v1/vectors/upsert', seedUrl);
    const req = https.request(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${token}`,
        'Content-Length': Buffer.byteLength(payload),
      },
      rejectUnauthorized: false,
    }, (res) => {
      let body = '';
      res.on('data', (chunk) => body += chunk);
      res.on('end', () => {
        if (res.statusCode >= 200 && res.statusCode < 300) {
          resolve(JSON.parse(body));
        } else {
          reject(new Error(`Seed HTTP ${res.statusCode}: ${body}`));
        }
      });
    });
    req.on('error', reject);
    req.write(payload);
    req.end();
  });
}

// ---------------------------------------------------------------------------
// File Mode: Read JSONL Recording
// ---------------------------------------------------------------------------

async function processFile(filePath) {
  await initCnn();

  const store = new EmbeddingStore();
  const windows = new Map(); // nodeId -> SpectrogramWindow

  const rl = readline.createInterface({
    input: fs.createReadStream(filePath),
    crlfDelay: Infinity,
  });

  let frameCount = 0;
  let windowCount = 0;
  let lastNodeId = 0;
  let lastRssi = 0;

  for await (const line of rl) {
    if (frameCount >= LIMIT) break;

    let frame;
    try {
      frame = JSON.parse(line);
    } catch {
      continue;
    }

    const nodeId = frame.node_id || 0;
    const nSubcarriers = frame.subcarriers || 64;
    const iqHex = frame.iq_hex || '';

    if (!iqHex) continue;

    const amplitudes = parseIqHex(iqHex, nSubcarriers);
    lastNodeId = nodeId;
    lastRssi = frame.rssi || 0;

    if (!windows.has(nodeId)) {
      windows.set(nodeId, new SpectrogramWindow(nSubcarriers, WINDOW_SIZE));
    }

    const win = windows.get(nodeId);
    win.push(amplitudes);
    frameCount++;

    // Check if this window is ready and stride condition met
    if (win.isFull() && (win.totalPushed - WINDOW_SIZE) % STRIDE === 0) {
      const t0 = Date.now();
      const embedding = extractEmbedding(win);
      const embedMs = Date.now() - t0;

      const meta = {
        timestamp: frame.timestamp,
        nodeId,
        windowIdx: windowCount,
        rssi: frame.rssi || 0,
        nSubcarriers,
      };

      store.add(embedding, meta);

      if (args.ascii) {
        printAsciiSpectrogram(win, { nodeId, rssi: frame.rssi });
      }

      if (JSON_OUTPUT) {
        console.log(JSON.stringify({
          type: 'embedding',
          windowIdx: windowCount,
          nodeId,
          dim: embedding.length,
          embedMs,
          embedding: Array.from(embedding).map(v => +v.toFixed(6)),
        }));
      } else {
        const embSnippet = Array.from(embedding.subarray(0, 4)).map(v => v.toFixed(4)).join(', ');
        console.log(`[window ${windowCount}] node=${nodeId} embed=[${embSnippet}, ...] (${embedMs}ms)`);
      }

      // kNN search against previous windows
      if (KNN_K > 0 && store.size > 1) {
        const neighbors = store.knn(embedding, KNN_K + 1);
        // Skip self (first result)
        const results = neighbors.filter(n => n.windowIdx !== windowCount).slice(0, KNN_K);
        if (JSON_OUTPUT) {
          console.log(JSON.stringify({ type: 'knn', query: windowCount, results }));
        } else {
          console.log(`  kNN(${KNN_K}): ${results.map(r => `w${r.windowIdx}(${r.similarity.toFixed(3)})`).join(' ')}`);
        }
      }

      // Cognitum Seed ingest
      if (args.ingest) {
        try {
          await ingestToSeed(embedding, meta);
          if (!JSON_OUTPUT) console.log(`  -> ingested to Seed`);
        } catch (err) {
          console.error(`  -> Seed ingest failed: ${err.message}`);
        }
      }

      windowCount++;
    }
  }

  if (!JSON_OUTPUT) {
    console.log(`\nProcessed ${frameCount} frames -> ${windowCount} spectrogram windows`);
    console.log(`Store contains ${store.size} embeddings of dimension ${EMBED_DIM}`);
  }

  return store;
}

// ---------------------------------------------------------------------------
// Live Mode: UDP Listener
// ---------------------------------------------------------------------------

async function processLive() {
  await initCnn();

  const store = new EmbeddingStore();
  const windows = new Map();
  let windowCount = 0;

  const server = dgram.createSocket('udp4');

  server.on('message', async (msg, rinfo) => {
    // Try binary ADR-018 format first
    let parsed = parseBinaryFrame(msg);
    let nodeId, nSubcarriers, amplitudes, rssi;

    if (parsed) {
      nodeId = parsed.nodeId;
      nSubcarriers = parsed.nSubcarriers;
      amplitudes = parsed.amplitudes;
      rssi = parsed.rssi;
    } else {
      // Try JSONL format
      try {
        const frame = JSON.parse(msg.toString());
        nodeId = frame.node_id || 0;
        nSubcarriers = frame.subcarriers || 64;
        amplitudes = parseIqHex(frame.iq_hex || '', nSubcarriers);
        rssi = frame.rssi || 0;
      } catch {
        return; // Unknown format
      }
    }

    if (!windows.has(nodeId)) {
      windows.set(nodeId, new SpectrogramWindow(nSubcarriers, WINDOW_SIZE));
    }

    const win = windows.get(nodeId);
    win.push(amplitudes);

    if (win.isFull() && (win.totalPushed - WINDOW_SIZE) % STRIDE === 0) {
      const t0 = Date.now();
      const embedding = extractEmbedding(win);
      const embedMs = Date.now() - t0;

      const meta = {
        timestamp: Date.now() / 1000,
        nodeId,
        windowIdx: windowCount,
        rssi,
        nSubcarriers,
      };

      store.add(embedding, meta);

      if (args.ascii) {
        printAsciiSpectrogram(win, { nodeId, rssi });
      }

      if (JSON_OUTPUT) {
        console.log(JSON.stringify({
          type: 'embedding',
          windowIdx: windowCount,
          nodeId,
          dim: embedding.length,
          embedMs,
          embedding: Array.from(embedding).map(v => +v.toFixed(6)),
        }));
      } else {
        const embSnippet = Array.from(embedding.subarray(0, 4)).map(v => v.toFixed(4)).join(', ');
        console.log(`[window ${windowCount}] node=${nodeId} rssi=${rssi} embed=[${embSnippet}, ...] (${embedMs}ms)`);
      }

      if (KNN_K > 0 && store.size > 1) {
        const neighbors = store.knn(embedding, KNN_K + 1);
        const results = neighbors.filter(n => n.windowIdx !== windowCount).slice(0, KNN_K);
        if (!JSON_OUTPUT) {
          console.log(`  kNN(${KNN_K}): ${results.map(r => `w${r.windowIdx}(${r.similarity.toFixed(3)})`).join(' ')}`);
        }
      }

      if (args.ingest) {
        try {
          await ingestToSeed(embedding, meta);
        } catch (err) {
          console.error(`  -> Seed ingest failed: ${err.message}`);
        }
      }

      windowCount++;
    }
  });

  server.on('listening', () => {
    const addr = server.address();
    console.log(`[live] Listening for CSI on UDP ${addr.address}:${addr.port}`);
    console.log(`[live] Window: ${WINDOW_SIZE} frames, stride: ${STRIDE}, embed dim: ${EMBED_DIM}`);
    if (KNN_K > 0) console.log(`[live] kNN search: k=${KNN_K}`);
    if (args.ingest) console.log(`[live] Ingesting to Cognitum Seed at ${args['seed-url']}`);
  });

  server.bind(PORT);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
  if (!args.file && !args.live) {
    console.error('Usage: node scripts/csi-spectrogram.js --file <path> [--ascii] [--knn K]');
    console.error('       node scripts/csi-spectrogram.js --live [--port 5006] [--ingest]');
    process.exit(1);
  }

  if (args.file) {
    const filePath = path.resolve(args.file);
    if (!fs.existsSync(filePath)) {
      console.error(`File not found: ${filePath}`);
      process.exit(1);
    }
    await processFile(filePath);
  } else {
    await processLive();
  }
}

main().catch((err) => {
  console.error('Fatal:', err);
  process.exit(1);
});
