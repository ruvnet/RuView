/**
 * RuVector Intelligence Layer v2 for Claude Code
 *
 * Enhanced with:
 * 1. Native HNSW rebuild on startup (150x faster search)
 * 2. Hyperbolic distance for hierarchical embeddings
 * 3. Confidence Calibration (track predicted vs actual)
 * 4. A/B Testing (holdout group comparison)
 * 5. Feedback Loop (learn from followed/ignored suggestions)
 * 6. Active Learning (identify uncertain states)
 * 7. Pattern Decay (time-weighted trajectories)
 */

import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { createHash } from 'crypto';

const __dirname = dirname(fileURLToPath(import.meta.url));
const DATA_DIR = join(__dirname, 'data');
const MEMORY_FILE = join(DATA_DIR, 'memory.json');
const TRAJECTORIES_FILE = join(DATA_DIR, 'trajectories.json');
const PATTERNS_FILE = join(DATA_DIR, 'patterns.json');
const CALIBRATION_FILE = join(DATA_DIR, 'calibration.json');
const FEEDBACK_FILE = join(DATA_DIR, 'feedback.json');
const ERROR_PATTERNS_FILE = join(DATA_DIR, 'error-patterns.json');
const SEQUENCES_FILE = join(DATA_DIR, 'sequences.json');

// Ensure data directory exists
if (!existsSync(DATA_DIR)) {
  mkdirSync(DATA_DIR, { recursive: true });
}

// Try to load @ruvector/core VectorDB
let VectorDB = null;
let ruvectorAvailable = false;

try {
  const ruvector = await import('@ruvector/core');
  VectorDB = ruvector.VectorDB;
  ruvectorAvailable = true;
  console.error('✅ @ruvector/core loaded - using native HNSW vector search');
} catch (e) {
  console.error('⚠️ @ruvector/core not available, using fallback cosine similarity');
}

// Try to load attention WASM for hyperbolic distance
let attentionWasm = null;
try {
  attentionWasm = await import('../../crates/ruvector-attention-wasm/pkg/ruvector_attention_wasm.js');
  console.error('✅ Hyperbolic attention WASM loaded');
} catch (e) {
  // Hyperbolic not available - use fallback
}

/**
 * Hyperbolic distance in Poincaré ball model
 * Better for hierarchical/tree-like data (crates, packages, file paths)
 */
function poincareDistance(u, v, c = 1.0) {
  const EPS = 1e-7;
  const sqrtC = Math.sqrt(c);

  let normDiffSq = 0, normUSq = 0, normVSq = 0;
  for (let i = 0; i < u.length; i++) {
    const diff = u[i] - (v[i] || 0);
    normDiffSq += diff * diff;
    normUSq += u[i] * u[i];
    normVSq += (v[i] || 0) * (v[i] || 0);
  }

  const lambdaU = 1.0 - c * normUSq;
  const lambdaV = 1.0 - c * normVSq;
  const numerator = 2.0 * c * normDiffSq;
  const denominator = Math.max(EPS, lambdaU * lambdaV);

  const arg = Math.max(1.0, 1.0 + numerator / denominator);
  return (1.0 / sqrtC) * Math.acosh(arg);
}

/**
 * Text to embedding with hierarchical awareness
 */
function textToEmbedding(text, dims = 128) {
  const embedding = new Float32Array(dims).fill(0);
  const normalized = text.toLowerCase().replace(/[^a-z0-9\s]/g, ' ');
  const words = normalized.split(/\s+/).filter(w => w.length > 1);

  const wordFreq = {};
  for (const word of words) {
    wordFreq[word] = (wordFreq[word] || 0) + 1;
  }

  for (const [word, freq] of Object.entries(wordFreq)) {
    const hash = createHash('sha256').update(word).digest();
    const idfWeight = 1 / Math.log(1 + freq);
    for (let i = 0; i < dims; i++) {
      const byteIdx = i % hash.length;
      const val = ((hash[byteIdx] & 0xFF) / 127.5) - 1;
      embedding[i] += val * idfWeight;
    }
  }

  // L2 normalize
  const magnitude = Math.sqrt(embedding.reduce((sum, v) => sum + v * v, 0));
  if (magnitude > 0) {
    for (let i = 0; i < dims; i++) embedding[i] /= magnitude;
  }

  // Scale down to fit in Poincaré ball (|x| < 1)
  const maxNorm = 0.95;
  for (let i = 0; i < dims; i++) embedding[i] *= maxNorm;

  return Array.from(embedding);
}

/**
 * Cosine similarity (fallback)
 */
function cosineSimilarity(a, b) {
  let dot = 0, magA = 0, magB = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * (b[i] || 0);
    magA += a[i] * a[i];
    magB += (b[i] || 0) * (b[i] || 0);
  }
  return dot / (Math.sqrt(magA) * Math.sqrt(magB) || 1);
}

/**
 * Vector Memory with Native HNSW + Hyperbolic distance option
 */
class VectorMemory {
  constructor(options = {}) {
    this.useHyperbolic = options.hyperbolic ?? true;
    this.curvature = options.curvature ?? 1.0;
    this.db = null;
    this.memories = this.loadMemories();
    this.dimensions = 128;
  }

  loadMemories() {
    if (existsSync(MEMORY_FILE)) {
      try { return JSON.parse(readFileSync(MEMORY_FILE, 'utf-8')); }
      catch { return []; }
    }
    return [];
  }

  saveMemories() {
    writeFileSync(MEMORY_FILE, JSON.stringify(this.memories, null, 2));
  }

  async init() {
    if (ruvectorAvailable && VectorDB && !this.db) {
      try {
        this.db = new VectorDB({
          dimensions: this.dimensions,
          distanceMetric: 'Cosine', // Native HNSW uses cosine, we post-process with hyperbolic
          hnswConfig: { m: 16, efConstruction: 200, efSearch: 100, maxElements: 50000 }
        });

        // Rebuild index from stored memories
        let rebuilt = 0;
        for (const mem of this.memories) {
          if (mem.embedding) {
            await this.db.insert({ id: mem.id, vector: new Float32Array(mem.embedding) });
            rebuilt++;
          }
        }
        console.error(`📊 VectorDB rebuilt with ${rebuilt} memories (HNSW index ready)`);
      } catch (e) {
        console.error('VectorDB init failed:', e.message);
        this.db = null;
      }
    }
  }

  async store(type, content, metadata = {}) {
    const id = `${type}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    const embedding = textToEmbedding(content, this.dimensions);

    const memory = {
      id, type, content, embedding,
      metadata: { ...metadata, timestamp: new Date().toISOString() }
    };

    this.memories.push(memory);
    if (this.db) {
      try { await this.db.insert({ id, vector: new Float32Array(embedding) }); }
      catch (e) { /* fallback works */ }
    }

    this.saveMemories();
    return id;
  }

  async search(query, limit = 5) {
    const queryEmbedding = textToEmbedding(query, this.dimensions);

    // Use native HNSW for candidate retrieval
    let candidates = this.memories;
    if (this.db) {
      try {
        const results = await this.db.search({
          vector: new Float32Array(queryEmbedding),
          k: Math.min(limit * 3, 50) // Get more candidates for reranking
        });
        candidates = results.map(r => this.memories.find(m => m.id === r.id)).filter(Boolean);
      } catch (e) { /* use all memories */ }
    }

    // Rerank with hyperbolic distance if enabled
    const scored = candidates.map(mem => {
      let score;
      if (this.useHyperbolic && mem.embedding) {
        const dist = poincareDistance(queryEmbedding, mem.embedding, this.curvature);
        score = 1 / (1 + dist); // Convert distance to similarity
      } else {
        score = cosineSimilarity(queryEmbedding, mem.embedding || []);
      }
      return { ...mem, score };
    });

    return scored.sort((a, b) => b.score - a.score).slice(0, limit);
  }

  getStats() {
    const typeCount = {};
    for (const mem of this.memories) {
      typeCount[mem.type] = (typeCount[mem.type] || 0) + 1;
    }
    return {
      total: this.memories.length,
      byType: typeCount,
      usingNativeHNSW: !!this.db,
      usingHyperbolic: this.useHyperbolic
    };
  }
}

/**
 * Calibration Tracker - measures if confidence matches reality
 */
class CalibrationTracker {
  constructor() {
    this.data = this.load();
  }

  load() {
    if (existsSync(CALIBRATION_FILE)) {
      try { return JSON.parse(readFileSync(CALIBRATION_FILE, 'utf-8')); }
      catch { return { buckets: {}, predictions: [] }; }
    }
    return { buckets: {}, predictions: [] };
  }

  save() {
    writeFileSync(CALIBRATION_FILE, JSON.stringify(this.data, null, 2));
  }

  record(predicted, actual, confidence) {
    const correct = predicted === actual;
    const bucket = Math.floor(confidence * 10) / 10; // 0.0, 0.1, ..., 0.9

    if (!this.data.buckets[bucket]) {
      this.data.buckets[bucket] = { total: 0, correct: 0 };
    }
    this.data.buckets[bucket].total++;
    if (correct) this.data.buckets[bucket].correct++;

    this.data.predictions.push({
      predicted, actual, correct, confidence,
      timestamp: new Date().toISOString()
    });

    // Keep last 500 predictions
    if (this.data.predictions.length > 500) {
      this.data.predictions = this.data.predictions.slice(-500);
    }

    this.save();
    return correct;
  }

  getCalibrationError() {
    let totalError = 0, count = 0;
    for (const [bucket, { total, correct }] of Object.entries(this.data.buckets)) {
      if (total >= 5) {
        const expectedRate = parseFloat(bucket) + 0.05;
        const actualRate = correct / total;
        totalError += Math.abs(expectedRate - actualRate);
        count++;
      }
    }
    return count > 0 ? totalError / count : 0;
  }

  getStats() {
    const stats = {};
    for (const [bucket, { total, correct }] of Object.entries(this.data.buckets)) {
      stats[bucket] = {
        total,
        accuracy: (correct / total).toFixed(3),
        expected: (parseFloat(bucket) + 0.05).toFixed(2)
      };
    }
    return { buckets: stats, calibrationError: this.getCalibrationError().toFixed(3) };
  }
}

/**
 * Feedback Loop - track if suggestions were followed
 */
class FeedbackLoop {
  constructor() {
    this.data = this.load();
  }

  load() {
    if (existsSync(FEEDBACK_FILE)) {
      try { return JSON.parse(readFileSync(FEEDBACK_FILE, 'utf-8')); }
      catch { return { suggestions: [], followRates: {} }; }
    }
    return { suggestions: [], followRates: {} };
  }

  save() {
    writeFileSync(FEEDBACK_FILE, JSON.stringify(this.data, null, 2));
  }

  recordSuggestion(suggestionId, suggested, confidence) {
    this.data.suggestions.push({
      id: suggestionId,
      suggested,
      confidence,
      followed: null,
      outcome: null,
      timestamp: new Date().toISOString()
    });
    this.save();
    return suggestionId;
  }

  recordOutcome(suggestionId, actualUsed, success) {
    const suggestion = this.data.suggestions.find(s => s.id === suggestionId);
    if (suggestion) {
      suggestion.followed = suggestion.suggested === actualUsed;
      suggestion.outcome = success;

      // Update follow rates
      const key = suggestion.suggested;
      if (!this.data.followRates[key]) {
        this.data.followRates[key] = { total: 0, followed: 0, followedSuccess: 0, ignoredSuccess: 0 };
      }
      const r = this.data.followRates[key];
      r.total++;
      if (suggestion.followed) {
        r.followed++;
        if (success) r.followedSuccess++;
      } else {
        if (success) r.ignoredSuccess++;
      }

      this.save();
    }
  }

  getAdviceValue() {
    const result = {};
    for (const [key, r] of Object.entries(this.data.followRates)) {
      if (r.total >= 5) {
        const followRate = r.followed / r.total;
        const followedSuccessRate = r.followed > 0 ? r.followedSuccess / r.followed : 0;
        const ignoredSuccessRate = (r.total - r.followed) > 0
          ? r.ignoredSuccess / (r.total - r.followed) : 0;

        result[key] = {
          followRate: followRate.toFixed(3),
          followedSuccessRate: followedSuccessRate.toFixed(3),
          ignoredSuccessRate: ignoredSuccessRate.toFixed(3),
          adviceValue: (followedSuccessRate - ignoredSuccessRate).toFixed(3)
        };
      }
    }
    return result;
  }
}

/**
 * ReasoningBank with A/B Testing, Decay, and Active Learning
 */
class ReasoningBank {
  constructor() {
    this.trajectories = this.loadTrajectories();
    this.qTable = this.loadPatterns();
    this.alpha = 0.1;
    this.gamma = 0.9;
    this.epsilon = 0.1;
    // A/B testing: Use environment override, or persistent session-based assignment
    // INTELLIGENCE_MODE=treatment forces learning mode (for development/testing)
    // INTELLIGENCE_MODE=control forces control mode (for baseline comparison)
    this.abTestGroup = process.env.INTELLIGENCE_MODE ||
      (this.getSessionId() % 100 < 5 ? 'control' : 'treatment'); // 5% holdout
    this.decayHalfLife = 7 * 24 * 60 * 60 * 1000; // 7 days in ms
  }

  loadTrajectories() {
    if (existsSync(TRAJECTORIES_FILE)) {
      try { return JSON.parse(readFileSync(TRAJECTORIES_FILE, 'utf-8')); }
      catch { return []; }
    }
    return [];
  }

  loadPatterns() {
    if (existsSync(PATTERNS_FILE)) {
      try { return JSON.parse(readFileSync(PATTERNS_FILE, 'utf-8')); }
      catch { return {}; }
    }
    return {};
  }

  /**
   * Get persistent session ID for consistent A/B assignment
   * Uses process PID + startup time hash for session-stable assignment
   */
  getSessionId() {
    // Combine PID with a time bucket (hourly) for session-stable but varied assignment
    const hourBucket = Math.floor(Date.now() / (60 * 60 * 1000));
    return (process.pid || 0) + hourBucket;
  }

  save() {
    writeFileSync(TRAJECTORIES_FILE, JSON.stringify(this.trajectories.slice(-1000), null, 2));
    writeFileSync(PATTERNS_FILE, JSON.stringify(this.qTable, null, 2));
  }

  stateKey(state) {
    // Preserve hyphens in crate names (e.g., ruvector-core, micro-hnsw-wasm)
    return state.toLowerCase().replace(/[^a-z0-9-]+/g, '_').slice(0, 80);
  }

  /**
   * Calculate decay weight based on trajectory age
   */
  getDecayWeight(timestamp) {
    const age = Date.now() - new Date(timestamp).getTime();
    return Math.pow(0.5, age / this.decayHalfLife);
  }

  /**
   * Record trajectory with time-weighted learning
   */
  recordTrajectory(state, action, outcome, reward) {
    const stateKey = this.stateKey(state);
    const trajectory = {
      id: `traj-${Date.now()}`,
      state: stateKey,
      action, outcome, reward,
      timestamp: new Date().toISOString(),
      abGroup: this.abTestGroup
    };
    this.trajectories.push(trajectory);

    // Time-weighted Q-learning with decay
    if (!this.qTable[stateKey]) this.qTable[stateKey] = { _meta: { lastUpdate: null, updateCount: 0 } };

    const meta = this.qTable[stateKey]._meta || { lastUpdate: null, updateCount: 0 };
    const decayWeight = meta.lastUpdate ? this.getDecayWeight(meta.lastUpdate) : 1.0;

    // Decayed current Q + new update
    const currentQ = (this.qTable[stateKey][action] || 0) * decayWeight;
    const updateCount = (meta.updateCount || 0) + 1;
    const adaptiveLR = Math.max(0.01, this.alpha / Math.sqrt(updateCount));

    this.qTable[stateKey][action] = Math.min(0.8, Math.max(-0.5,
      currentQ + adaptiveLR * (reward - currentQ)
    ));

    this.qTable[stateKey]._meta = {
      lastUpdate: new Date().toISOString(),
      updateCount
    };

    this.save();
    return trajectory.id;
  }

  /**
   * Get best action with A/B testing and active learning
   */
  getBestAction(state, availableActions) {
    const stateKey = this.stateKey(state);
    const qValues = this.qTable[stateKey] || {};

    // A/B Testing: Control group gets random actions
    if (this.abTestGroup === 'control') {
      const action = availableActions[Math.floor(Math.random() * availableActions.length)];
      return { action, confidence: 0, reason: 'control-group', qValues, abGroup: 'control' };
    }

    // Exploration with probability ε
    if (Math.random() < this.epsilon) {
      const action = availableActions[Math.floor(Math.random() * availableActions.length)];
      return { action, confidence: 0, reason: 'exploration', qValues, abGroup: 'treatment' };
    }

    // Exploitation
    let bestAction = availableActions[0];
    let bestQ = -Infinity;
    let secondBestQ = -Infinity;

    for (const action of availableActions) {
      const q = qValues[action] || 0;
      if (q > bestQ) {
        secondBestQ = bestQ;
        bestQ = q;
        bestAction = action;
      } else if (q > secondBestQ) {
        secondBestQ = q;
      }
    }

    const confidence = 1 / (1 + Math.exp(-bestQ * 2));

    // Active Learning: flag uncertain states
    const uncertainty = bestQ - secondBestQ;
    const isUncertain = uncertainty < 0.1 && bestQ < 0.5;

    return {
      action: bestAction,
      confidence: bestQ > 0 ? confidence : 0,
      reason: bestQ > 0 ? 'learned-preference' : 'no-data',
      qValues,
      abGroup: 'treatment',
      isUncertain,
      uncertaintyGap: uncertainty.toFixed(3)
    };
  }

  /**
   * Get uncertain states for active learning
   */
  getUncertainStates(threshold = 0.1) {
    const uncertain = [];
    for (const [state, actions] of Object.entries(this.qTable)) {
      if (state === '_meta') continue;

      const qVals = Object.entries(actions)
        .filter(([k]) => k !== '_meta')
        .map(([, v]) => v)
        .sort((a, b) => b - a);

      if (qVals.length >= 2) {
        const gap = qVals[0] - qVals[1];
        if (gap < threshold && qVals[0] < 0.5) {
          uncertain.push({ state, gap, topQ: qVals[0] });
        }
      }
    }
    return uncertain.sort((a, b) => a.gap - b.gap).slice(0, 10);
  }

  getTopPatterns(limit = 10) {
    const patterns = [];
    for (const [state, actions] of Object.entries(this.qTable)) {
      const sorted = Object.entries(actions)
        .filter(([k]) => k !== '_meta')
        .sort((a, b) => b[1] - a[1]);
      if (sorted.length > 0) {
        patterns.push({
          state,
          bestAction: sorted[0][0],
          qValue: sorted[0][1].toFixed(3),
          alternatives: sorted.slice(1, 3).map(([a, q]) => `${a}:${q.toFixed(2)}`)
        });
      }
    }
    return patterns.sort((a, b) => parseFloat(b.qValue) - parseFloat(a.qValue)).slice(0, limit);
  }

  getABStats() {
    const treatment = this.trajectories.filter(t => t.abGroup === 'treatment');
    const control = this.trajectories.filter(t => t.abGroup === 'control');

    const treatmentSuccess = treatment.filter(t => t.reward > 0).length;
    const controlSuccess = control.filter(t => t.reward > 0).length;

    return {
      treatment: { total: treatment.length, successRate: treatment.length > 0 ? (treatmentSuccess / treatment.length).toFixed(3) : 'N/A' },
      control: { total: control.length, successRate: control.length > 0 ? (controlSuccess / control.length).toFixed(3) : 'N/A' },
      lift: treatment.length > 10 && control.length > 10
        ? ((treatmentSuccess / treatment.length) - (controlSuccess / control.length)).toFixed(3)
        : 'insufficient-data'
    };
  }
}

/**
 * Error Pattern Tracker - learns from specific error types
 */
class ErrorPatternTracker {
  constructor() {
    this.data = this.load();
  }

  load() {
    if (existsSync(ERROR_PATTERNS_FILE)) {
      try { return JSON.parse(readFileSync(ERROR_PATTERNS_FILE, 'utf-8')); }
      catch { return { patterns: {}, fixes: {}, recentErrors: [] }; }
    }
    return { patterns: {}, fixes: {}, recentErrors: [] };
  }

  save() {
    writeFileSync(ERROR_PATTERNS_FILE, JSON.stringify(this.data, null, 2));
  }

  /**
   * Parse error output to extract error codes and types
   */
  parseError(stderr) {
    const errors = [];

    // Rust error codes (E0308, E0433, etc.)
    const rustErrors = stderr.match(/error\[E\d{4}\]/g) || [];
    for (const e of rustErrors) {
      const code = e.match(/E\d{4}/)[0];
      errors.push({ type: 'rust', code, category: this.categorizeRustError(code) });
    }

    // TypeScript errors (TS2304, TS2322, etc.)
    const tsErrors = stderr.match(/TS\d{4}/g) || [];
    for (const code of tsErrors) {
      errors.push({ type: 'typescript', code, category: this.categorizeTsError(code) });
    }

    // npm/node errors
    if (stderr.includes('ENOENT')) errors.push({ type: 'npm', code: 'ENOENT', category: 'file-not-found' });
    if (stderr.includes('EACCES')) errors.push({ type: 'npm', code: 'EACCES', category: 'permission' });
    if (stderr.includes('MODULE_NOT_FOUND')) errors.push({ type: 'node', code: 'MODULE_NOT_FOUND', category: 'missing-module' });

    return errors;
  }

  categorizeRustError(code) {
    const categories = {
      'E0308': 'type-mismatch',
      'E0433': 'missing-import',
      'E0412': 'undefined-type',
      'E0425': 'undefined-value',
      'E0599': 'missing-method',
      'E0277': 'trait-not-implemented',
      'E0382': 'use-after-move',
      'E0502': 'borrow-conflict',
      'E0507': 'cannot-move-out',
      'E0515': 'return-local-reference'
    };
    return categories[code] || 'other';
  }

  categorizeTsError(code) {
    const categories = {
      'TS2304': 'undefined-name',
      'TS2322': 'type-mismatch',
      'TS2339': 'missing-property',
      'TS2345': 'argument-type',
      'TS2769': 'overload-mismatch'
    };
    return categories[code] || 'other';
  }

  /**
   * Record an error occurrence
   */
  recordError(command, stderr, file = null, crate = null) {
    const errors = this.parseError(stderr);
    const timestamp = new Date().toISOString();

    for (const error of errors) {
      const key = `${error.type}:${error.code}`;
      if (!this.data.patterns[key]) {
        this.data.patterns[key] = { count: 0, category: error.category, contexts: [], lastSeen: null };
      }
      this.data.patterns[key].count++;
      this.data.patterns[key].lastSeen = timestamp;
      if (crate && !this.data.patterns[key].contexts.includes(crate)) {
        this.data.patterns[key].contexts.push(crate);
      }
    }

    // Store recent errors for sequence detection
    if (errors.length > 0) {
      this.data.recentErrors.push({ errors, command, file, crate, timestamp });
      if (this.data.recentErrors.length > 100) {
        this.data.recentErrors = this.data.recentErrors.slice(-100);
      }
    }

    this.save();
    return errors;
  }

  /**
   * Record a successful fix for an error pattern
   */
  recordFix(errorCode, fixDescription) {
    if (!this.data.fixes[errorCode]) {
      this.data.fixes[errorCode] = [];
    }
    this.data.fixes[errorCode].push({
      fix: fixDescription,
      timestamp: new Date().toISOString()
    });
    // Keep last 5 fixes per error
    if (this.data.fixes[errorCode].length > 5) {
      this.data.fixes[errorCode] = this.data.fixes[errorCode].slice(-5);
    }
    this.save();
  }

  /**
   * Suggest fixes for an error code
   */
  suggestFix(errorCode) {
    const fixes = this.data.fixes[errorCode] || [];
    const pattern = this.data.patterns[errorCode];

    return {
      errorCode,
      category: pattern?.category || 'unknown',
      occurrences: pattern?.count || 0,
      commonContexts: pattern?.contexts?.slice(0, 3) || [],
      recentFixes: fixes.slice(-3).map(f => f.fix)
    };
  }

  getStats() {
    const totalErrors = Object.values(this.data.patterns).reduce((s, p) => s + p.count, 0);
    const topErrors = Object.entries(this.data.patterns)
      .sort((a, b) => b[1].count - a[1].count)
      .slice(0, 5)
      .map(([code, p]) => ({ code, count: p.count, category: p.category }));

    return { totalErrors, topErrors, fixesRecorded: Object.keys(this.data.fixes).length };
  }
}

/**
 * File Sequence Tracker - learns which files are often edited together
 */
class SequenceTracker {
  constructor() {
    this.data = this.load();
    this.sessionEdits = []; // Track edits in current session
  }

  load() {
    if (existsSync(SEQUENCES_FILE)) {
      try { return JSON.parse(readFileSync(SEQUENCES_FILE, 'utf-8')); }
      catch { return { sequences: {}, coedits: {}, testPairs: {} }; }
    }
    return { sequences: {}, coedits: {}, testPairs: {} };
  }

  save() {
    writeFileSync(SEQUENCES_FILE, JSON.stringify(this.data, null, 2));
  }

  /**
   * Record a file edit and learn sequences
   */
  recordEdit(file) {
    const timestamp = Date.now();
    const normalizedFile = this.normalizePath(file);

    // Check for sequence from previous edit
    if (this.sessionEdits.length > 0) {
      const lastEdit = this.sessionEdits[this.sessionEdits.length - 1];
      const timeDiff = timestamp - lastEdit.timestamp;

      // If edited within 5 minutes, consider it a sequence
      if (timeDiff < 5 * 60 * 1000) {
        this.recordSequence(lastEdit.file, normalizedFile);
      }
    }

    // Detect test file pairing
    this.detectTestPair(normalizedFile);

    this.sessionEdits.push({ file: normalizedFile, timestamp });

    // Keep session to last 20 edits
    if (this.sessionEdits.length > 20) {
      this.sessionEdits = this.sessionEdits.slice(-20);
    }

    this.save();
  }

  normalizePath(file) {
    // Normalize to relative path from crates/ or src/
    const match = file.match(/(crates\/[^/]+\/.*|src\/.*|tests\/.*)/);
    return match ? match[1] : file.split('/').slice(-3).join('/');
  }

  recordSequence(from, to) {
    if (from === to) return;

    if (!this.data.sequences[from]) {
      this.data.sequences[from] = {};
    }
    if (!this.data.sequences[from][to]) {
      this.data.sequences[from][to] = { count: 0, lastSeen: null };
    }
    this.data.sequences[from][to].count++;
    this.data.sequences[from][to].lastSeen = new Date().toISOString();

    // Also record as co-edit (bidirectional)
    const pairKey = [from, to].sort().join('|');
    if (!this.data.coedits[pairKey]) {
      this.data.coedits[pairKey] = { count: 0, files: [from, to] };
    }
    this.data.coedits[pairKey].count++;
  }

  detectTestPair(file) {
    // Match source file to test file patterns
    let testFile = null;
    let sourceFile = null;

    if (file.includes('/tests/') || file.includes('.test.') || file.includes('_test.')) {
      testFile = file;
      // Try to find corresponding source
      sourceFile = file
        .replace('/tests/', '/src/')
        .replace('.test.', '.')
        .replace('_test.', '.');
    } else if (file.includes('/src/')) {
      sourceFile = file;
      // Construct potential test file paths
      const ext = file.split('.').pop();
      testFile = file
        .replace('/src/', '/tests/')
        .replace(`.${ext}`, `.test.${ext}`);
    }

    if (testFile && sourceFile) {
      const pairKey = [sourceFile, testFile].sort().join('|');
      if (!this.data.testPairs[pairKey]) {
        this.data.testPairs[pairKey] = { source: sourceFile, test: testFile, editCount: 0 };
      }
      this.data.testPairs[pairKey].editCount++;
    }
  }

  /**
   * Suggest next files based on current file
   */
  suggestNextFiles(currentFile, limit = 3) {
    const normalized = this.normalizePath(currentFile);
    const sequences = this.data.sequences[normalized] || {};

    const suggestions = Object.entries(sequences)
      .sort((a, b) => b[1].count - a[1].count)
      .slice(0, limit)
      .map(([file, data]) => ({
        file,
        probability: Math.min(0.9, data.count / 10),
        timesSequenced: data.count
      }));

    // Also check for test file suggestion
    const testSuggestion = this.suggestTestFile(currentFile);
    if (testSuggestion && !suggestions.find(s => s.file === testSuggestion.file)) {
      suggestions.push(testSuggestion);
    }

    return suggestions.slice(0, limit);
  }

  /**
   * Suggest test file for a source file
   */
  suggestTestFile(sourceFile) {
    const normalized = this.normalizePath(sourceFile);

    // Find matching test pair
    for (const [, pair] of Object.entries(this.data.testPairs)) {
      if (pair.source === normalized || normalized.includes(pair.source)) {
        return {
          file: pair.test,
          type: 'test-file',
          probability: 0.8,
          reason: 'Corresponding test file'
        };
      }
    }

    // Generate test file path if not found
    if (sourceFile.includes('/src/') && !sourceFile.includes('test')) {
      const ext = sourceFile.split('.').pop();
      const testPath = sourceFile
        .replace('/src/', '/tests/')
        .replace(`.${ext}`, ext === 'rs' ? `_test.${ext}` : `.test.${ext}`);
      return {
        file: this.normalizePath(testPath),
        type: 'suggested-test',
        probability: 0.5,
        reason: 'Suggested test location'
      };
    }

    return null;
  }

  /**
   * Suggest running tests after editing source files
   */
  shouldSuggestTests(file) {
    const normalized = this.normalizePath(file);

    // Always suggest tests for Rust source files
    if (file.endsWith('.rs') && file.includes('/src/') && !file.includes('test')) {
      const crateMatch = file.match(/crates\/([^/]+)/);
      const crate = crateMatch ? crateMatch[1] : null;
      return {
        suggest: true,
        command: crate ? `cargo test -p ${crate}` : 'cargo test',
        reason: 'Source file modified'
      };
    }

    // Suggest tests for TypeScript source files
    if ((file.endsWith('.ts') || file.endsWith('.tsx')) && !file.includes('.test.')) {
      return {
        suggest: true,
        command: 'npm test',
        reason: 'TypeScript source modified'
      };
    }

    return { suggest: false };
  }

  getStats() {
    return {
      totalSequences: Object.keys(this.data.sequences).length,
      totalCoedits: Object.keys(this.data.coedits).length,
      testPairs: Object.keys(this.data.testPairs).length,
      sessionEdits: this.sessionEdits.length
    };
  }
}

/**
 * Neural Router with enhanced intelligence
 */
class NeuralRouter {
  constructor(memory, reasoning, calibration, feedback) {
    this.memory = memory;
    this.reasoning = reasoning;
    this.calibration = calibration;
    this.feedback = feedback;
  }

  async route(task, context = {}) {
    const { fileType, crate, operation = 'edit' } = context;
    // Use underscore format to match pretrained Q-table
    const state = `${operation}_${fileType || 'file'}_in_${crate || 'project'}`;
    const agents = this.getAgentsForContext(fileType, crate);

    const suggestion = this.reasoning.getBestAction(state, agents);
    const similar = await this.memory.search(task, 3);

    let finalAgent = suggestion.action;
    let finalConf = suggestion.confidence;

    if (similar.length > 0 && similar[0].score > 0.7) {
      const pastAgent = similar[0].metadata?.agent;
      if (pastAgent && agents.includes(pastAgent)) {
        finalAgent = pastAgent;
        finalConf = Math.min(1, finalConf + 0.2);
      }
    }

    // Record for feedback tracking
    const suggestionId = `sug-${Date.now()}`;
    this.feedback.recordSuggestion(suggestionId, finalAgent, finalConf);

    return {
      recommended: finalAgent,
      confidence: finalConf,
      reason: this.buildReason(finalAgent, suggestion.reason, similar),
      alternatives: agents.filter(a => a !== finalAgent).slice(0, 3),
      context: { state, agents, similar: similar.slice(0, 2) },
      suggestionId,
      abGroup: suggestion.abGroup,
      isUncertain: suggestion.isUncertain
    };
  }

  getAgentsForContext(fileType, crate) {
    const base = ['coder', 'reviewer', 'tester'];

    const typeMap = {
      'rs': ['rust-developer', 'code-analyzer'],
      'ts': ['typescript-developer', 'backend-dev'],
      'js': ['javascript-developer', 'backend-dev'],
      'md': ['technical-writer'],
      'json': ['config-specialist'],
      'py': ['python-developer'],
      'css': ['frontend-developer'],
      'html': ['frontend-developer'],
      'tsx': ['frontend-developer'],
      'yml': ['devops-engineer'],
      'yaml': ['devops-engineer'],
      'sql': ['database-expert'],
      'sh': ['system-admin']
    };

    if (typeMap[fileType]) base.push(...typeMap[fileType]);

    // Crate-specific specializations
    if (fileType === 'rs') {
      if (crate?.includes('wasm') || crate === 'rvlite') base.push('production-validator');
      if (crate?.includes('gnn') || crate?.includes('attention') || crate === 'sona') base.push('ml-developer');
      if (crate?.includes('postgres')) base.push('backend-dev', 'system-architect');
      if (crate?.includes('mincut') || crate?.includes('graph')) base.push('system-architect');
    }

    return [...new Set(base)];
  }

  buildReason(agent, qReason, similar) {
    const parts = [];
    if (qReason === 'learned-preference') parts.push('learned from past success');
    if (similar.length > 0 && similar[0].score > 0.6) parts.push('similar past task succeeded');
    if (parts.length === 0) parts.push('default selection');
    return `${agent}: ${parts.join(', ')}`;
  }
}

/**
 * Main Intelligence API v2
 */
class RuVectorIntelligence {
  constructor(options = {}) {
    this.memory = new VectorMemory({ hyperbolic: options.hyperbolic ?? true });
    this.reasoning = new ReasoningBank();
    this.calibration = new CalibrationTracker();
    this.feedback = new FeedbackLoop();
    this.errorPatterns = new ErrorPatternTracker();
    this.sequences = new SequenceTracker();
    this.router = new NeuralRouter(this.memory, this.reasoning, this.calibration, this.feedback);
    this.initialized = false;
  }

  async init() {
    if (!this.initialized) {
      await this.memory.init();
      this.initialized = true;
    }
  }

  async remember(type, content, metadata = {}) {
    await this.init();
    return this.memory.store(type, content, metadata);
  }

  async recall(query, limit = 5) {
    await this.init();
    return this.memory.search(query, limit);
  }

  learn(state, action, outcome, reward) {
    return this.reasoning.recordTrajectory(state, action, outcome, reward);
  }

  suggest(state, actions) {
    return this.reasoning.getBestAction(state, actions);
  }

  async route(task, context = {}) {
    await this.init();
    return this.router.route(task, context);
  }

  recordCalibration(predicted, actual, confidence) {
    return this.calibration.record(predicted, actual, confidence);
  }

  recordFeedback(suggestionId, actualUsed, success) {
    this.feedback.recordOutcome(suggestionId, actualUsed, success);
  }

  // === New v3 Features ===

  /**
   * Record an error from command output
   */
  recordError(command, stderr, file = null, crate = null) {
    return this.errorPatterns.recordError(command, stderr, file, crate);
  }

  /**
   * Record a fix for an error pattern
   */
  recordFix(errorCode, fixDescription) {
    this.errorPatterns.recordFix(errorCode, fixDescription);
  }

  /**
   * Get suggested fixes for an error
   */
  suggestFix(errorCode) {
    return this.errorPatterns.suggestFix(errorCode);
  }

  /**
   * Record a file edit for sequence learning
   */
  recordFileEdit(file) {
    this.sequences.recordEdit(file);
  }

  /**
   * Suggest next files based on current file
   */
  suggestNextFiles(file, limit = 3) {
    return this.sequences.suggestNextFiles(file, limit);
  }

  /**
   * Check if tests should be suggested after editing a file
   */
  shouldSuggestTests(file) {
    return this.sequences.shouldSuggestTests(file);
  }

  stats() {
    return {
      memory: this.memory.getStats(),
      trajectories: this.reasoning.trajectories.length,
      patterns: Object.keys(this.reasoning.qTable).length,
      topPatterns: this.reasoning.getTopPatterns(5),
      calibration: this.calibration.getStats(),
      abTest: this.reasoning.getABStats(),
      adviceValue: this.feedback.getAdviceValue(),
      uncertainStates: this.reasoning.getUncertainStates(0.15),
      // v3 stats
      errorPatterns: this.errorPatterns.getStats(),
      sequences: this.sequences.getStats(),
      ruvectorNative: ruvectorAvailable
    };
  }
}

export { RuVectorIntelligence, VectorMemory, ReasoningBank, NeuralRouter, CalibrationTracker, FeedbackLoop, ErrorPatternTracker, SequenceTracker };
export default RuVectorIntelligence;
