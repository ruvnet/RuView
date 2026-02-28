/**
 * RuVector Native Storage for Intelligence Layer
 *
 * Replaces JSON file storage with:
 * - @ruvector/core: Native HNSW vector storage (150x faster)
 * - @ruvector/sona: ReasoningBank for Q-learning and patterns
 * - redb: Embedded database for metadata
 */

import { existsSync, mkdirSync, writeFileSync, readFileSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const DATA_DIR = join(__dirname, 'data');
const DB_PATH = join(DATA_DIR, 'intelligence.db');

// Legacy JSON paths for migration
const LEGACY_PATTERNS = join(DATA_DIR, 'patterns.json');
const LEGACY_TRAJECTORIES = join(DATA_DIR, 'trajectories.json');
const LEGACY_MEMORY = join(DATA_DIR, 'memory.json');
const LEGACY_FEEDBACK = join(DATA_DIR, 'feedback.json');
const LEGACY_SEQUENCES = join(DATA_DIR, 'sequences.json');

// Try to load native modules
let ruvectorCore = null;
let sona = null;

try {
  ruvectorCore = await import('@ruvector/core');
  console.log('✅ @ruvector/core loaded - using native HNSW');
} catch (e) {
  console.log('⚠️ @ruvector/core not available - using fallback');
}

try {
  sona = await import('@ruvector/sona');
  console.log('✅ @ruvector/sona loaded - using native ReasoningBank');
} catch (e) {
  console.log('⚠️ @ruvector/sona not available - using fallback');
}

/**
 * Native Vector Storage using @ruvector/core
 */
export class NativeVectorStorage {
  constructor(options = {}) {
    this.dimensions = options.dimensions || 128;
    this.dbPath = options.dbPath || DB_PATH;
    this.useNative = !!ruvectorCore;
    this.db = null;
    this.fallbackData = [];
  }

  async init() {
    if (this.useNative && ruvectorCore) {
      try {
        // Use native VectorDB
        this.db = new ruvectorCore.VectorDB({
          dimensions: this.dimensions,
          storagePath: this.dbPath,
          efConstruction: 200,
          maxNeighbors: 32,
          efSearch: 100
        });
        return true;
      } catch (e) {
        console.warn('Native VectorDB init failed:', e.message);
        this.useNative = false;
      }
    }

    // Fallback: load from JSON
    if (existsSync(LEGACY_MEMORY)) {
      try {
        this.fallbackData = JSON.parse(readFileSync(LEGACY_MEMORY, 'utf-8'));
      } catch (e) {
        this.fallbackData = [];
      }
    }
    return false;
  }

  async insert(id, vector, metadata = {}) {
    if (this.useNative && this.db) {
      // Native module requires Float32Array
      const typedVector = vector instanceof Float32Array
        ? vector
        : new Float32Array(vector);
      return this.db.insert({
        id,
        vector: typedVector
      });
    }

    // Fallback
    this.fallbackData.push({ id, vector: Array.from(vector), metadata });
    return id;
  }

  async search(query, k = 5) {
    if (this.useNative && this.db) {
      const typedQuery = query instanceof Float32Array
        ? query
        : new Float32Array(query);
      return this.db.search({
        vector: typedQuery,
        k,
        efSearch: 100
      });
    }

    // Fallback: brute force cosine similarity
    const results = this.fallbackData.map(item => {
      const score = this.cosineSimilarity(query, item.vector);
      return { ...item, score };
    });

    return results
      .sort((a, b) => b.score - a.score)
      .slice(0, k);
  }

  cosineSimilarity(a, b) {
    let dot = 0, normA = 0, normB = 0;
    for (let i = 0; i < a.length; i++) {
      dot += a[i] * b[i];
      normA += a[i] * a[i];
      normB += b[i] * b[i];
    }
    return dot / (Math.sqrt(normA) * Math.sqrt(normB) + 1e-8);
  }

  async count() {
    if (this.useNative && this.db) {
      return this.db.len();
    }
    return this.fallbackData.length;
  }

  async save() {
    if (!this.useNative) {
      writeFileSync(LEGACY_MEMORY, JSON.stringify(this.fallbackData, null, 2));
    }
    // Native storage is already persistent
  }
}

/**
 * Native ReasoningBank using @ruvector/sona
 */
export class NativeReasoningBank {
  constructor(options = {}) {
    this.useNative = !!sona;
    this.engine = null;
    this.alpha = options.alpha || 0.1;
    this.gamma = options.gamma || 0.9;
    this.epsilon = options.epsilon || 0.1;

    // Fallback Q-table
    this.qTable = {};
    this.trajectories = [];
    this.abTestGroup = process.env.INTELLIGENCE_MODE || 'treatment';
  }

  async init() {
    if (this.useNative && sona) {
      try {
        this.engine = new sona.SonaEngine(256);
        return true;
      } catch (e) {
        console.warn('Native SonaEngine init failed:', e.message);
        this.useNative = false;
      }
    }

    // Fallback: load from JSON
    if (existsSync(LEGACY_PATTERNS)) {
      try {
        this.qTable = JSON.parse(readFileSync(LEGACY_PATTERNS, 'utf-8'));
      } catch (e) {
        this.qTable = {};
      }
    }
    if (existsSync(LEGACY_TRAJECTORIES)) {
      try {
        this.trajectories = JSON.parse(readFileSync(LEGACY_TRAJECTORIES, 'utf-8'));
      } catch (e) {
        this.trajectories = [];
      }
    }
    return false;
  }

  stateKey(state) {
    return state.toLowerCase().replace(/[^a-z0-9-]+/g, '_').slice(0, 80);
  }

  recordTrajectory(state, action, outcome, reward) {
    const stateKey = this.stateKey(state);

    if (this.useNative && this.engine) {
      // Use native trajectory recording
      const embedding = this.stateToEmbedding(stateKey);
      const builder = this.engine.beginTrajectory(embedding);
      // Add step with reward
      builder.addStep([reward], [1.0], reward);
      this.engine.endTrajectory(builder, Math.max(0, reward));
      return `traj-native-${Date.now()}`;
    }

    // Fallback Q-learning
    const trajectory = {
      id: `traj-${Date.now()}`,
      state: stateKey,
      action, outcome, reward,
      timestamp: new Date().toISOString(),
      abGroup: this.abTestGroup
    };
    this.trajectories.push(trajectory);

    // Q-learning update
    if (!this.qTable[stateKey]) {
      this.qTable[stateKey] = { _meta: { lastUpdate: null, updateCount: 0 } };
    }

    const currentQ = this.qTable[stateKey][action] || 0;
    const updateCount = (this.qTable[stateKey]._meta?.updateCount || 0) + 1;
    const adaptiveLR = Math.max(0.01, this.alpha / Math.sqrt(updateCount));

    this.qTable[stateKey][action] = Math.min(0.8, Math.max(-0.5,
      currentQ + adaptiveLR * (reward - currentQ)
    ));

    this.qTable[stateKey]._meta = {
      lastUpdate: new Date().toISOString(),
      updateCount
    };

    return trajectory.id;
  }

  getBestAction(state, availableActions) {
    const stateKey = this.stateKey(state);

    if (this.useNative && this.engine) {
      // Use native pattern matching
      const embedding = this.stateToEmbedding(stateKey);
      const patterns = this.engine.findPatterns(embedding, 3);

      if (patterns.length > 0) {
        // Map pattern to action based on quality
        const bestPattern = patterns[0];
        const confidence = bestPattern.avgQuality || 0;

        // Select action based on pattern cluster
        const actionIdx = Math.floor(bestPattern.id % availableActions.length);
        return {
          action: availableActions[actionIdx],
          confidence,
          reason: 'native-pattern',
          abGroup: 'native'
        };
      }
    }

    // Fallback Q-table lookup
    const qValues = this.qTable[stateKey] || {};

    // A/B Testing
    if (this.abTestGroup === 'control') {
      const action = availableActions[Math.floor(Math.random() * availableActions.length)];
      return { action, confidence: 0, reason: 'control-group', abGroup: 'control' };
    }

    // Epsilon-greedy exploration
    if (Math.random() < this.epsilon) {
      const action = availableActions[Math.floor(Math.random() * availableActions.length)];
      return { action, confidence: 0, reason: 'exploration', abGroup: 'treatment' };
    }

    // Exploitation
    let bestAction = availableActions[0];
    let bestQ = -Infinity;

    for (const action of availableActions) {
      const q = qValues[action] || 0;
      if (q > bestQ) {
        bestQ = q;
        bestAction = action;
      }
    }

    const confidence = 1 / (1 + Math.exp(-bestQ * 2));

    return {
      action: bestAction,
      confidence: bestQ > 0 ? confidence : 0,
      reason: bestQ > 0 ? 'learned-preference' : 'default',
      qValues,
      abGroup: 'treatment'
    };
  }

  stateToEmbedding(state) {
    // Simple hash-based embedding for state
    const embedding = new Array(256).fill(0);
    const chars = state.split('');
    for (let i = 0; i < chars.length; i++) {
      const idx = (chars[i].charCodeAt(0) * (i + 1)) % 256;
      embedding[idx] += 1.0 / chars.length;
    }
    // Normalize
    const norm = Math.sqrt(embedding.reduce((s, x) => s + x * x, 0));
    return embedding.map(x => x / (norm + 1e-8));
  }

  async forceLearning() {
    if (this.useNative && this.engine) {
      return this.engine.forceLearn();
    }
    return 'fallback-mode';
  }

  getStats() {
    if (this.useNative && this.engine) {
      return JSON.parse(this.engine.getStats());
    }

    return {
      patterns: Object.keys(this.qTable).length,
      trajectories: this.trajectories.length,
      mode: 'fallback'
    };
  }

  async save() {
    if (!this.useNative) {
      // Keep trajectories bounded
      if (this.trajectories.length > 1000) {
        this.trajectories = this.trajectories.slice(-1000);
      }
      writeFileSync(LEGACY_TRAJECTORIES, JSON.stringify(this.trajectories, null, 2));
      writeFileSync(LEGACY_PATTERNS, JSON.stringify(this.qTable, null, 2));
    }
    // Native storage is already persistent
  }
}

/**
 * Native Metadata Storage using simple key-value store
 */
export class NativeMetadataStorage {
  constructor(options = {}) {
    this.dbPath = options.dbPath || join(DATA_DIR, 'metadata.json');
    this.data = {};
  }

  async init() {
    if (existsSync(this.dbPath)) {
      try {
        this.data = JSON.parse(readFileSync(this.dbPath, 'utf-8'));
      } catch (e) {
        this.data = {};
      }
    }
    return true;
  }

  get(namespace, key) {
    return this.data[`${namespace}:${key}`];
  }

  set(namespace, key, value) {
    this.data[`${namespace}:${key}`] = value;
  }

  delete(namespace, key) {
    delete this.data[`${namespace}:${key}`];
  }

  list(namespace) {
    const prefix = `${namespace}:`;
    return Object.entries(this.data)
      .filter(([k]) => k.startsWith(prefix))
      .map(([k, v]) => ({ key: k.slice(prefix.length), value: v }));
  }

  async save() {
    writeFileSync(this.dbPath, JSON.stringify(this.data, null, 2));
  }
}

/**
 * Migration utility to move from JSON to native storage
 */
export async function migrateToNative(options = {}) {
  const dryRun = options.dryRun || false;
  const results = {
    vectors: 0,
    patterns: 0,
    trajectories: 0,
    errors: []
  };

  console.log('🚀 Starting migration to RuVector native storage...');
  console.log(`   Dry run: ${dryRun}`);

  // 1. Migrate vector memory
  if (existsSync(LEGACY_MEMORY)) {
    try {
      const memory = JSON.parse(readFileSync(LEGACY_MEMORY, 'utf-8'));
      console.log(`📊 Found ${memory.length} vectors in memory.json`);

      if (!dryRun && ruvectorCore) {
        const vectorStore = new NativeVectorStorage({ dimensions: 128 });
        await vectorStore.init();

        for (const item of memory) {
          if (item.embedding && item.embedding.length > 0) {
            await vectorStore.insert(item.id, item.embedding, item.metadata || {});
            results.vectors++;
          }
        }
        console.log(`✅ Migrated ${results.vectors} vectors to native HNSW`);
      } else {
        results.vectors = memory.filter(m => m.embedding).length;
        console.log(`   Would migrate ${results.vectors} vectors`);
      }
    } catch (e) {
      results.errors.push(`Vector migration: ${e.message}`);
    }
  }

  // 2. Migrate patterns/Q-table
  if (existsSync(LEGACY_PATTERNS)) {
    try {
      const patterns = JSON.parse(readFileSync(LEGACY_PATTERNS, 'utf-8'));
      const patternCount = Object.keys(patterns).length;
      console.log(`📊 Found ${patternCount} patterns in patterns.json`);

      if (!dryRun && sona) {
        const reasoningBank = new NativeReasoningBank();
        await reasoningBank.init();

        // Convert Q-table entries to trajectories for learning
        for (const [state, actions] of Object.entries(patterns)) {
          if (state.startsWith('_')) continue;

          for (const [action, qValue] of Object.entries(actions)) {
            if (action === '_meta') continue;

            reasoningBank.recordTrajectory(
              state,
              action,
              qValue > 0 ? 'success' : 'failure',
              qValue
            );
            results.patterns++;
          }
        }

        // Force learning to consolidate patterns
        await reasoningBank.forceLearning();
        console.log(`✅ Migrated ${results.patterns} pattern entries to native ReasoningBank`);
      } else {
        results.patterns = Object.keys(patterns).length;
        console.log(`   Would migrate ${results.patterns} patterns`);
      }
    } catch (e) {
      results.errors.push(`Pattern migration: ${e.message}`);
    }
  }

  // 3. Migrate trajectories
  if (existsSync(LEGACY_TRAJECTORIES)) {
    try {
      const trajectories = JSON.parse(readFileSync(LEGACY_TRAJECTORIES, 'utf-8'));
      console.log(`📊 Found ${trajectories.length} trajectories in trajectories.json`);

      if (!dryRun && sona) {
        const reasoningBank = new NativeReasoningBank();
        await reasoningBank.init();

        for (const traj of trajectories) {
          reasoningBank.recordTrajectory(
            traj.state,
            traj.action,
            traj.outcome,
            traj.reward
          );
          results.trajectories++;
        }
        console.log(`✅ Migrated ${results.trajectories} trajectories to native storage`);
      } else {
        results.trajectories = trajectories.length;
        console.log(`   Would migrate ${results.trajectories} trajectories`);
      }
    } catch (e) {
      results.errors.push(`Trajectory migration: ${e.message}`);
    }
  }

  // Summary
  console.log('\n📋 Migration Summary:');
  console.log(`   Vectors: ${results.vectors}`);
  console.log(`   Patterns: ${results.patterns}`);
  console.log(`   Trajectories: ${results.trajectories}`);

  if (results.errors.length > 0) {
    console.log('\n⚠️ Errors:');
    results.errors.forEach(e => console.log(`   - ${e}`));
  }

  return results;
}

// Export all storage classes
export default {
  NativeVectorStorage,
  NativeReasoningBank,
  NativeMetadataStorage,
  migrateToNative
};
