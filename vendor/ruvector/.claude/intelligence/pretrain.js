#!/usr/bin/env node
/**
 * Pretrain Intelligence System from memory.db
 *
 * Extracts learned patterns from existing swarm memory:
 * - Command success/failure patterns → Q-Table
 * - Agent assignments → Neural Router training
 * - File edit history → Vector Memory
 * - Coordination patterns → Swarm Graph
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
  console.log('🧠 RuVector Intelligence Pretraining');
  console.log('=====================================\n');

  if (!existsSync(MEMORY_DB)) {
    console.error('❌ Memory database not found:', MEMORY_DB);
    process.exit(1);
  }

  const db = new Database(MEMORY_DB, { readonly: true });
  const stats = { commands: 0, agents: 0, files: 0, patterns: 0, coordination: 0 };

  // ========== 1. Extract Command Patterns → Q-Table ==========
  console.log('📊 Extracting command patterns...');

  const qTable = {};
  const trajectories = [];

  const commands = db.prepare(`
    SELECT key, value FROM memory_entries
    WHERE namespace = 'command-history'
    ORDER BY created_at DESC
    LIMIT 5000
  `).all();

  for (const row of commands) {
    try {
      const data = JSON.parse(row.value);
      const cmd = data.command || '';
      const success = data.success === true || data.exitCode === '0';

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

      const state = `${cmdType}_in_${context}`;
      const action = success ? 'command-succeeded' : 'command-failed';
      const reward = success ? 1.0 : -0.5;

      // Update Q-table with strong regularization to prevent overfitting
      if (!qTable[state]) qTable[state] = { 'command-succeeded': 0, 'command-failed': 0 };
      const stateCount = (qTable[state]._count || 0) + 1;
      qTable[state]._count = stateCount;

      // Decaying learning rate: starts at 0.3, decays to 0.01
      const learningRate = Math.max(0.01, 0.3 / Math.sqrt(stateCount));
      const currentQ = qTable[state][action] || 0;

      // Update with capped value (max 0.8) to prevent overconfidence
      const newQ = currentQ + learningRate * (reward - currentQ);
      qTable[state][action] = Math.min(0.8, Math.max(-0.5, newQ));

      // Record trajectory
      trajectories.push({
        id: `pretrain-cmd-${stats.commands}`,
        state,
        action,
        outcome: cmd.slice(0, 100),
        reward,
        timestamp: data.timestamp || new Date().toISOString()
      });

      stats.commands++;
    } catch (e) { /* skip malformed */ }
  }

  console.log(`   ✅ Processed ${stats.commands} commands`);

  // ========== 2. Extract Agent Assignments → Q-Table ==========
  console.log('🤖 Extracting agent assignments...');

  const agentAssignments = db.prepare(`
    SELECT key, value FROM memory_entries
    WHERE namespace = 'agent-assignments'
    ORDER BY created_at DESC
    LIMIT 1000
  `).all();

  for (const row of agentAssignments) {
    try {
      const data = JSON.parse(row.value);
      const file = data.file || '';
      const ext = extname(file).slice(1) || 'unknown';
      const agentType = data.type || 'coder';
      const recommended = data.recommended === true;

      // Extract crate if applicable
      const crateMatch = file.match(/crates\/([^/]+)/);
      const crate = crateMatch ? crateMatch[1] : 'project';

      const state = `edit_${ext}_in_${crate}`;
      const action = agentType;
      const reward = recommended ? 1.0 : 0.5;

      // Anti-overfitting: cap Q-values and use count-based decay
      if (!qTable[state]) qTable[state] = {};
      const stateCount = (qTable[state]._count || 0) + 1;
      qTable[state]._count = stateCount;

      const learningRate = Math.max(0.01, 0.2 / Math.sqrt(stateCount));
      const currentQ = qTable[state][action] || 0;
      qTable[state][action] = Math.min(0.75, currentQ + learningRate * (reward - currentQ));

      trajectories.push({
        id: `pretrain-agent-${stats.agents}`,
        state,
        action,
        outcome: `recommended for ${basename(file)}`,
        reward,
        timestamp: new Date().toISOString()
      });

      stats.agents++;
    } catch (e) { /* skip */ }
  }

  console.log(`   ✅ Processed ${stats.agents} agent assignments`);

  // ========== 3. Extract File History → Vector Memory ==========
  console.log('📁 Extracting file edit history...');

  const memories = [];

  const fileHistory = db.prepare(`
    SELECT key, value FROM memory_entries
    WHERE namespace = 'file-history'
    ORDER BY created_at DESC
    LIMIT 2000
  `).all();

  for (const row of fileHistory) {
    try {
      const data = JSON.parse(row.value);
      const file = data.file || '';
      const ext = extname(file).slice(1);
      const crateMatch = file.match(/crates\/([^/]+)/);
      const crate = crateMatch ? crateMatch[1] : null;

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
          timestamp: data.timestamp || new Date().toISOString()
        }
      });

      stats.files++;
    } catch (e) { /* skip */ }
  }

  console.log(`   ✅ Processed ${stats.files} file edits`);

  // ========== 4. Extract Patterns → Vector Memory ==========
  console.log('🧩 Extracting reasoning patterns...');

  const patterns = db.prepare(`
    SELECT id, type, pattern_data, confidence FROM patterns
    ORDER BY confidence DESC
    LIMIT 100
  `).all();

  for (const row of patterns) {
    try {
      const data = JSON.parse(row.pattern_data);
      const content = data.content || data.title || JSON.stringify(data).slice(0, 200);

      memories.push({
        id: `pretrain-pattern-${stats.patterns}`,
        type: 'pattern',
        content,
        embedding: textToEmbedding(content),
        metadata: {
          patternId: row.id,
          patternType: row.type,
          confidence: row.confidence,
          timestamp: new Date().toISOString()
        }
      });

      stats.patterns++;
    } catch (e) { /* skip */ }
  }

  console.log(`   ✅ Processed ${stats.patterns} patterns`);

  // ========== 5. Extract Coordination → Swarm Graph ==========
  console.log('🔗 Building swarm coordination graph...');

  const nodes = {};
  const edges = {};

  // Extract unique agents from assignments
  const agents = new Set();
  for (const row of agentAssignments) {
    try {
      const data = JSON.parse(row.value);
      if (data.type) agents.add(data.type);
    } catch (e) { /* skip */ }
  }

  // Create nodes for each agent type
  const agentCapabilities = {
    'coder': ['rust', 'typescript', 'implementation'],
    'technical-writer': ['documentation', 'markdown'],
    'reviewer': ['code-review', 'security'],
    'tester': ['unit-test', 'integration'],
    'general-developer': ['general', 'debugging'],
    'rust-developer': ['rust', 'cargo', 'wasm'],
    'ml-developer': ['gnn', 'attention', 'neural']
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

  // Create edges based on common file edits (simplified)
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

  // Connect agents that work on similar files
  const agentList = Object.keys(agentFiles);
  for (let i = 0; i < agentList.length; i++) {
    for (let j = i + 1; j < agentList.length; j++) {
      const a1 = agentList[i];
      const a2 = agentList[j];
      const files1 = new Set(agentFiles[a1].map(f => dirname(f)));
      const files2 = new Set(agentFiles[a2].map(f => dirname(f)));

      // Count overlapping directories
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

  // ========== 6. Save All Data ==========
  console.log('\n💾 Saving pretrained data...');

  // Save Q-Table (patterns.json)
  writeFileSync(
    join(DATA_DIR, 'patterns.json'),
    JSON.stringify(qTable, null, 2)
  );
  console.log(`   ✅ Q-Table: ${Object.keys(qTable).length} states`);

  // Save Trajectories
  writeFileSync(
    join(DATA_DIR, 'trajectories.json'),
    JSON.stringify(trajectories.slice(-1000), null, 2)
  );
  console.log(`   ✅ Trajectories: ${Math.min(trajectories.length, 1000)} entries`);

  // Save Memories
  writeFileSync(
    join(DATA_DIR, 'memory.json'),
    JSON.stringify(memories, null, 2)
  );
  console.log(`   ✅ Vector Memory: ${memories.length} entries`);

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
      pretrainedAt: new Date().toISOString(),
      stats
    }, null, 2)
  );

  db.close();

  // ========== Summary ==========
  console.log('\n✅ Pretraining Complete!');
  console.log('========================');
  console.log(`   Commands processed:    ${stats.commands}`);
  console.log(`   Agent assignments:     ${stats.agents}`);
  console.log(`   File edits:            ${stats.files}`);
  console.log(`   Patterns:              ${stats.patterns}`);
  console.log(`   Swarm nodes:           ${Object.keys(nodes).length}`);
  console.log(`   Total Q-states:        ${Object.keys(qTable).length}`);
  console.log(`   Total memories:        ${memories.length}`);
  console.log('\n🧠 Intelligence system is now pretrained!');
}

pretrain().catch(console.error);
