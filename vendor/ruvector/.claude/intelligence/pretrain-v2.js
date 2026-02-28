#!/usr/bin/env node
/**
 * Pretrain Intelligence System v2 - Enhanced with all v2 features
 *
 * Improvements over v1:
 * - Uses ALL available data (no arbitrary limits)
 * - Bootstraps Confidence Calibration from performance-metrics
 * - Adds Pattern Decay timestamps to Q-table
 * - Identifies Uncertain States for Active Learning
 * - Prepares A/B Testing baseline metrics
 */

import Database from 'better-sqlite3';
import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'fs';
import { join, dirname, extname, basename } from 'path';
import { fileURLToPath } from 'url';
import { createHash } from 'crypto';

const __dirname = dirname(fileURLToPath(import.meta.url));
const DATA_DIR = join(__dirname, 'data');
const MEMORY_DB = '/workspaces/ruvector/.swarm/memory.db';

// Ensure data directory exists
if (!existsSync(DATA_DIR)) mkdirSync(DATA_DIR, { recursive: true });

/**
 * Text to embedding (same as in index.js)
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

  const magnitude = Math.sqrt(embedding.reduce((sum, v) => sum + v * v, 0));
  if (magnitude > 0) {
    for (let i = 0; i < dims; i++) embedding[i] /= magnitude;
  }

  return Array.from(embedding);
}

/**
 * Main pretraining function
 */
async function pretrain() {
  console.log('🧠 RuVector Intelligence Pretraining v2');
  console.log('========================================\n');

  if (!existsSync(MEMORY_DB)) {
    console.error('❌ Memory database not found:', MEMORY_DB);
    process.exit(1);
  }

  const db = new Database(MEMORY_DB, { readonly: true });
  const stats = { commands: 0, agents: 0, files: 0, patterns: 0, coordination: 0, calibration: 0 };

  // ========== 1. Extract Command Patterns → Q-Table with Decay Metadata ==========
  console.log('📊 Extracting command patterns (ALL data)...');

  const qTable = {};
  const trajectories = [];

  // Get ALL commands (no limit)
  const commands = db.prepare(`
    SELECT key, value, created_at FROM memory_entries
    WHERE namespace = 'command-history'
    ORDER BY created_at DESC
  `).all();

  for (const row of commands) {
    try {
      const data = JSON.parse(row.value);
      const cmd = data.command || '';
      const success = data.success === true || data.exitCode === '0';
      const timestamp = row.created_at ? new Date(row.created_at * 1000).toISOString() : new Date().toISOString();

      // Classify command type
      let cmdType = 'other';
      if (cmd.startsWith('cargo')) cmdType = 'cargo';
      else if (cmd.startsWith('npm')) cmdType = 'npm';
      else if (cmd.startsWith('git')) cmdType = 'git';
      else if (cmd.startsWith('wasm-pack')) cmdType = 'wasm';
      else if (cmd.includes('test')) cmdType = 'test';
      else if (cmd.includes('build')) cmdType = 'build';

      // Detect context from command
      let context = 'general';
      if (cmd.includes('rvlite')) context = 'rvlite';
      else if (cmd.includes('ruvector-core')) context = 'ruvector-core';
      else if (cmd.includes('ruvector-graph')) context = 'ruvector-graph';
      else if (cmd.includes('wasm')) context = 'wasm';
      else if (cmd.includes('postgres')) context = 'postgres';
      else if (cmd.includes('mincut')) context = 'mincut';
      else if (cmd.includes('gnn')) context = 'gnn';
      else if (cmd.includes('attention')) context = 'attention';
      else if (cmd.includes('sona')) context = 'sona';

      const state = `${cmdType}_in_${context}`;
      const action = success ? 'command-succeeded' : 'command-failed';
      const reward = success ? 1.0 : -0.5;

      // Initialize state with v2 metadata
      if (!qTable[state]) {
        qTable[state] = {
          'command-succeeded': 0,
          'command-failed': 0,
          _meta: {
            lastUpdate: timestamp,
            updateCount: 0,
            firstSeen: timestamp
          }
        };
      }

      const stateCount = (qTable[state]._meta?.updateCount || 0) + 1;
      qTable[state]._meta.updateCount = stateCount;
      qTable[state]._meta.lastUpdate = timestamp;

      // Decaying learning rate with Q-value caps
      const learningRate = Math.max(0.01, 0.3 / Math.sqrt(stateCount));
      const currentQ = qTable[state][action] || 0;
      const newQ = currentQ + learningRate * (reward - currentQ);
      qTable[state][action] = Math.min(0.8, Math.max(-0.5, newQ));

      // Record trajectory with timestamp
      trajectories.push({
        id: `pretrain-cmd-${stats.commands}`,
        state,
        action,
        outcome: cmd.slice(0, 100),
        reward,
        timestamp
      });

      stats.commands++;
    } catch (e) { /* skip malformed */ }
  }

  console.log(`   ✅ Processed ${stats.commands} commands`);

  // ========== 2. Extract Agent Assignments → Q-Table ==========
  console.log('🤖 Extracting agent assignments (ALL data)...');

  const agentAssignments = db.prepare(`
    SELECT key, value, created_at FROM memory_entries
    WHERE namespace = 'agent-assignments'
    ORDER BY created_at DESC
  `).all();

  for (const row of agentAssignments) {
    try {
      const data = JSON.parse(row.value);
      const file = data.file || '';
      const ext = extname(file).slice(1) || 'unknown';
      const agentType = data.type || 'coder';
      const recommended = data.recommended === true;
      const timestamp = row.created_at ? new Date(row.created_at * 1000).toISOString() : new Date().toISOString();

      // Extract crate if applicable
      const crateMatch = file.match(/crates\/([^/]+)/);
      const crate = crateMatch ? crateMatch[1] : 'project';

      const state = `edit_${ext}_in_${crate}`;
      const action = agentType;
      const reward = recommended ? 1.0 : 0.5;

      // Initialize with v2 metadata
      if (!qTable[state]) {
        qTable[state] = {
          _meta: {
            lastUpdate: timestamp,
            updateCount: 0,
            firstSeen: timestamp
          }
        };
      }

      const stateCount = (qTable[state]._meta?.updateCount || 0) + 1;
      qTable[state]._meta.updateCount = stateCount;
      qTable[state]._meta.lastUpdate = timestamp;

      const learningRate = Math.max(0.01, 0.2 / Math.sqrt(stateCount));
      const currentQ = qTable[state][action] || 0;
      qTable[state][action] = Math.min(0.75, currentQ + learningRate * (reward - currentQ));

      trajectories.push({
        id: `pretrain-agent-${stats.agents}`,
        state,
        action,
        outcome: `recommended for ${basename(file)}`,
        reward,
        timestamp
      });

      stats.agents++;
    } catch (e) { /* skip */ }
  }

  console.log(`   ✅ Processed ${stats.agents} agent assignments`);

  // ========== 3. Bootstrap Calibration from Performance Metrics ==========
  console.log('📈 Bootstrapping confidence calibration...');

  const calibrationBuckets = {};
  const performanceMetrics = db.prepare(`
    SELECT key, value FROM memory_entries
    WHERE namespace = 'performance-metrics'
    AND key LIKE 'command-metrics:%'
  `).all();

  // Group by complexity (as a proxy for confidence)
  const complexityToConfidence = { 'low': 0.9, 'medium': 0.7, 'high': 0.5 };

  for (const row of performanceMetrics) {
    try {
      const data = JSON.parse(row.value);
      const success = data.success === true;
      const complexity = data.complexity || 'medium';
      const confidence = complexityToConfidence[complexity] || 0.7;

      // Round to bucket (0.5, 0.6, 0.7, 0.8, 0.9)
      const bucket = (Math.round(confidence * 10) / 10).toFixed(1);

      if (!calibrationBuckets[bucket]) {
        calibrationBuckets[bucket] = { correct: 0, total: 0 };
      }
      calibrationBuckets[bucket].total++;
      if (success) calibrationBuckets[bucket].correct++;

      stats.calibration++;
    } catch (e) { /* skip */ }
  }

  // Calculate calibration - format must match CalibrationTracker expected format
  // CalibrationTracker expects: { buckets: { "0.9": { total, correct } }, predictions: [] }
  const calibration = { buckets: {}, predictions: [] };

  for (const [bucket, data] of Object.entries(calibrationBuckets)) {
    calibration.buckets[bucket] = {
      total: data.total,
      correct: data.correct  // CalibrationTracker uses "correct", not "accuracy"
    };
  }

  console.log(`   ✅ Bootstrapped calibration from ${stats.calibration} metrics`);
  console.log(`   📊 Calibration buckets: ${Object.keys(calibration.buckets).length}`);

  // ========== 4. Extract File History → Vector Memory ==========
  console.log('📁 Extracting file edit history (ALL data)...');

  const memories = [];

  const fileHistory = db.prepare(`
    SELECT key, value, created_at FROM memory_entries
    WHERE namespace = 'file-history'
    ORDER BY created_at DESC
  `).all();

  for (const row of fileHistory) {
    try {
      const data = JSON.parse(row.value);
      const file = data.file || '';
      const ext = extname(file).slice(1);
      const crateMatch = file.match(/crates\/([^/]+)/);
      const crate = crateMatch ? crateMatch[1] : null;
      const timestamp = row.created_at ? new Date(row.created_at * 1000).toISOString() : new Date().toISOString();

      const content = `edit ${ext} file ${basename(file)} in ${crate || 'project'}`;

      memories.push({
        id: `pretrain-file-${stats.files}`,
        type: 'edit',
        content,
        embedding: textToEmbedding(content),
        metadata: {
          file,
          crate,
          ext,
          timestamp
        }
      });

      stats.files++;
    } catch (e) { /* skip */ }
  }

  console.log(`   ✅ Processed ${stats.files} file edits`);

  // ========== 5. Extract Reasoning Patterns ==========
  console.log('🧩 Extracting reasoning patterns...');

  const patterns = db.prepare(`
    SELECT id, type, pattern_data, confidence, created_at FROM patterns
    ORDER BY confidence DESC
  `).all();

  for (const row of patterns) {
    try {
      const data = JSON.parse(row.pattern_data);
      const content = data.content || data.title || JSON.stringify(data).slice(0, 200);
      const timestamp = row.created_at || new Date().toISOString();

      memories.push({
        id: `pretrain-pattern-${stats.patterns}`,
        type: 'pattern',
        content,
        embedding: textToEmbedding(content),
        metadata: {
          patternId: row.id,
          patternType: row.type,
          confidence: row.confidence,
          timestamp
        }
      });

      stats.patterns++;
    } catch (e) { /* skip */ }
  }

  console.log(`   ✅ Processed ${stats.patterns} patterns`);

  // ========== 6. Identify Uncertain States for Active Learning ==========
  console.log('🎯 Identifying uncertain states...');

  const uncertainStates = [];
  for (const [state, actions] of Object.entries(qTable)) {
    const qValues = Object.entries(actions)
      .filter(([k, v]) => k !== '_meta' && k !== '_count' && typeof v === 'number')
      .map(([k, v]) => ({ action: k, q: v }))
      .sort((a, b) => b.q - a.q);

    if (qValues.length >= 2) {
      const gap = qValues[0].q - qValues[1].q;
      if (gap < 0.1 && qValues[0].q > 0) { // Close Q-values = uncertain
        uncertainStates.push({
          state,
          bestAction: qValues[0].action,
          secondBest: qValues[1].action,
          gap: gap.toFixed(4),
          needsExploration: true
        });
      }
    }
  }

  console.log(`   ✅ Found ${uncertainStates.length} uncertain states for active learning`);

  // ========== 7. Build Swarm Coordination Graph ==========
  console.log('🔗 Building swarm coordination graph...');

  const nodes = {};
  const edges = {};

  const agents = new Set();
  for (const row of agentAssignments) {
    try {
      const data = JSON.parse(row.value);
      if (data.type) agents.add(data.type);
    } catch (e) { /* skip */ }
  }

  const agentCapabilities = {
    'coder': ['rust', 'typescript', 'implementation'],
    'technical-writer': ['documentation', 'markdown'],
    'reviewer': ['code-review', 'security'],
    'tester': ['unit-test', 'integration'],
    'general-developer': ['general', 'debugging'],
    'rust-developer': ['rust', 'cargo', 'wasm'],
    'typescript-developer': ['typescript', 'javascript', 'node'],
    'ml-developer': ['gnn', 'attention', 'neural'],
    'documentation-writer': ['docs', 'readme', 'api-docs']
  };

  for (const agent of agents) {
    nodes[agent] = {
      type: agent,
      capabilities: agentCapabilities[agent] || [agent],
      load: 0,
      active: true
    };
    stats.coordination++;
  }

  // Create edges based on common file edits
  const agentFiles = {};
  for (const row of agentAssignments) {
    try {
      const data = JSON.parse(row.value);
      const agent = data.type;
      const file = data.file;
      if (!agentFiles[agent]) agentFiles[agent] = [];
      agentFiles[agent].push(file);
    } catch (e) { /* skip */ }
  }

  const agentList = Object.keys(agentFiles);
  for (let i = 0; i < agentList.length; i++) {
    for (let j = i + 1; j < agentList.length; j++) {
      const a1 = agentList[i];
      const a2 = agentList[j];
      const files1 = new Set(agentFiles[a1].map(f => dirname(f)));
      const files2 = new Set(agentFiles[a2].map(f => dirname(f)));

      let overlap = 0;
      for (const dir of files1) {
        if (files2.has(dir)) overlap++;
      }

      if (overlap > 0) {
        edges[`${a1}:${a2}`] = { weight: overlap, interactions: overlap };
      }
    }
  }

  console.log(`   ✅ Built graph with ${Object.keys(nodes).length} agents, ${Object.keys(edges).length} edges`);

  // ========== 8. Save All Data ==========
  console.log('\n💾 Saving pretrained data (v2)...');

  // Save Q-Table with decay metadata
  writeFileSync(
    join(DATA_DIR, 'patterns.json'),
    JSON.stringify(qTable, null, 2)
  );
  console.log(`   ✅ Q-Table: ${Object.keys(qTable).length} states (with decay metadata)`);

  // Save Trajectories (keep last 2000 for more history)
  writeFileSync(
    join(DATA_DIR, 'trajectories.json'),
    JSON.stringify(trajectories.slice(-2000), null, 2)
  );
  console.log(`   ✅ Trajectories: ${Math.min(trajectories.length, 2000)} entries`);

  // Save Memories
  writeFileSync(
    join(DATA_DIR, 'memory.json'),
    JSON.stringify(memories, null, 2)
  );
  console.log(`   ✅ Vector Memory: ${memories.length} entries`);

  // Save Calibration (NEW)
  writeFileSync(
    join(DATA_DIR, 'calibration.json'),
    JSON.stringify(calibration, null, 2)
  );
  console.log(`   ✅ Calibration: ${Object.keys(calibration.buckets).length} buckets`);

  // Save Uncertain States for Active Learning (NEW)
  writeFileSync(
    join(DATA_DIR, 'uncertain-states.json'),
    JSON.stringify({ states: uncertainStates, lastUpdated: new Date().toISOString() }, null, 2)
  );
  console.log(`   ✅ Uncertain States: ${uncertainStates.length} entries`);

  // Save Swarm Graph
  writeFileSync(
    join(DATA_DIR, 'coordination-graph.json'),
    JSON.stringify({ nodes, edges, lastUpdated: new Date().toISOString() }, null, 2)
  );
  console.log(`   ✅ Swarm Graph: ${Object.keys(nodes).length} nodes`);

  // Save Swarm State
  writeFileSync(
    join(DATA_DIR, 'swarm-state.json'),
    JSON.stringify({
      tasks: [],
      optimizations: 0,
      pretrained: true,
      pretrainVersion: 2,
      pretrainedAt: new Date().toISOString(),
      stats,
      features: {
        patternDecay: true,
        calibration: true,
        activeLearning: true,
        uncertainStates: uncertainStates.length
      }
    }, null, 2)
  );

  // Initialize empty feedback tracking (suggestions must be array, followRates must be object)
  writeFileSync(
    join(DATA_DIR, 'feedback.json'),
    JSON.stringify({ suggestions: [], followRates: {}, lastUpdated: new Date().toISOString() }, null, 2)
  );
  console.log(`   ✅ Feedback tracking initialized`);

  db.close();

  // ========== Summary ==========
  console.log('\n✅ Pretraining v2 Complete!');
  console.log('===========================');
  console.log(`   Commands processed:    ${stats.commands.toLocaleString()}`);
  console.log(`   Agent assignments:     ${stats.agents}`);
  console.log(`   File edits:            ${stats.files.toLocaleString()}`);
  console.log(`   Patterns:              ${stats.patterns}`);
  console.log(`   Calibration samples:   ${stats.calibration.toLocaleString()}`);
  console.log(`   Uncertain states:      ${uncertainStates.length}`);
  console.log(`   Swarm nodes:           ${Object.keys(nodes).length}`);
  console.log(`   Total Q-states:        ${Object.keys(qTable).length}`);
  console.log(`   Total memories:        ${memories.length.toLocaleString()}`);
  console.log('\n🧠 Intelligence system v2 pretrained with:');
  console.log('   ✅ Pattern decay timestamps');
  console.log('   ✅ Confidence calibration bootstrap');
  console.log('   ✅ Active learning uncertain states');
  console.log('   ✅ All available training data\n');
}

pretrain().catch(console.error);
