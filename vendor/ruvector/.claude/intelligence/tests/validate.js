#!/usr/bin/env node
/**
 * RuVector Intelligence Validation Suite
 *
 * Validates pretrained data for:
 * - Q-table integrity (no overfitting)
 * - Vector memory retrieval
 * - Swarm graph connectivity
 * - Agent routing accuracy
 */

import { readFileSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const DATA_DIR = join(__dirname, '..', 'data');

const results = { passed: 0, failed: 0, warnings: 0 };

function test(name, fn) {
  try {
    const result = fn();
    if (result === true) {
      console.log(`  ✅ ${name}`);
      results.passed++;
    } else if (result === 'warn') {
      console.log(`  ⚠️  ${name}`);
      results.warnings++;
    } else {
      console.log(`  ❌ ${name}: ${result}`);
      results.failed++;
    }
  } catch (e) {
    console.log(`  ❌ ${name}: ${e.message}`);
    results.failed++;
  }
}

console.log('\n🧠 RuVector Intelligence Validation');
console.log('====================================\n');

// === 1. Data Files Exist ===
console.log('📁 Data Files:');
const requiredFiles = ['patterns.json', 'memory.json', 'trajectories.json', 'coordination-graph.json', 'swarm-state.json'];
for (const file of requiredFiles) {
  test(`${file} exists`, () => {
    return existsSync(join(DATA_DIR, file)) || `File not found`;
  });
}

// === 2. Q-Table Validation ===
console.log('\n📊 Q-Table (patterns.json):');
const patterns = JSON.parse(readFileSync(join(DATA_DIR, 'patterns.json'), 'utf-8'));
const states = Object.keys(patterns);

test(`Has learned states (${states.length})`, () => {
  return states.length >= 10 || `Only ${states.length} states`;
});

test('No overfitting (Q-values < 0.85)', () => {
  const overfit = [];
  for (const [state, actions] of Object.entries(patterns)) {
    for (const [action, value] of Object.entries(actions)) {
      if (action !== '_count' && typeof value === 'number' && value > 0.85) {
        overfit.push(`${state}:${action}=${value.toFixed(3)}`);
      }
    }
  }
  return overfit.length === 0 || `Overfit: ${overfit.slice(0, 3).join(', ')}...`;
});

test('No negative Q-values below -0.6', () => {
  const tooNegative = [];
  for (const [state, actions] of Object.entries(patterns)) {
    for (const [action, value] of Object.entries(actions)) {
      if (action !== '_count' && typeof value === 'number' && value < -0.6) {
        tooNegative.push(`${state}:${action}=${value.toFixed(3)}`);
      }
    }
  }
  return tooNegative.length === 0 || `Too negative: ${tooNegative.slice(0, 3).join(', ')}`;
});

test('Sample counts are tracked', () => {
  const withCounts = states.filter(s => patterns[s]._count > 0);
  return withCounts.length > 0 || 'No _count fields found';
});

// Q-value distribution check
const qValues = [];
for (const actions of Object.values(patterns)) {
  for (const [k, v] of Object.entries(actions)) {
    if (k !== '_count' && typeof v === 'number') qValues.push(v);
  }
}
const avgQ = qValues.reduce((a, b) => a + b, 0) / qValues.length;
const minQ = Math.min(...qValues);
const maxQ = Math.max(...qValues);

test(`Q-value range is reasonable (${minQ.toFixed(2)} to ${maxQ.toFixed(2)})`, () => {
  return maxQ <= 0.85 && minQ >= -0.6 || `Range too extreme`;
});

test(`Average Q-value not too high (avg=${avgQ.toFixed(3)})`, () => {
  return avgQ < 0.7 || 'warn';
});

// === 3. Vector Memory Validation ===
console.log('\n🧠 Vector Memory (memory.json):');
const memory = JSON.parse(readFileSync(join(DATA_DIR, 'memory.json'), 'utf-8'));

test(`Has memories (${memory.length})`, () => {
  return memory.length > 100 || `Only ${memory.length} memories`;
});

test('Memories have embeddings', () => {
  const withEmbeddings = memory.filter(m => m.embedding && m.embedding.length === 128);
  return withEmbeddings.length === memory.length || `${memory.length - withEmbeddings.length} missing embeddings`;
});

test('Embeddings are normalized', () => {
  const sample = memory.slice(0, 10);
  for (const m of sample) {
    if (!m.embedding) continue;
    const magnitude = Math.sqrt(m.embedding.reduce((sum, v) => sum + v * v, 0));
    if (Math.abs(magnitude - 1.0) > 0.01) {
      return `Magnitude ${magnitude.toFixed(3)} not ~1.0`;
    }
  }
  return true;
});

test('Memories have types', () => {
  const types = new Set(memory.map(m => m.type));
  return types.size > 0 || 'No types found';
});

// === 4. Trajectories Validation ===
console.log('\n📈 Trajectories (trajectories.json):');
const trajectories = JSON.parse(readFileSync(join(DATA_DIR, 'trajectories.json'), 'utf-8'));

test(`Has trajectories (${trajectories.length})`, () => {
  return trajectories.length > 100 || `Only ${trajectories.length} trajectories`;
});

test('Trajectories have required fields', () => {
  const required = ['state', 'action', 'reward'];
  const missing = trajectories.slice(0, 50).filter(t => !required.every(f => t[f] !== undefined));
  return missing.length === 0 || `${missing.length} missing fields`;
});

const rewardDistribution = { positive: 0, negative: 0, neutral: 0 };
for (const t of trajectories) {
  if (t.reward > 0) rewardDistribution.positive++;
  else if (t.reward < 0) rewardDistribution.negative++;
  else rewardDistribution.neutral++;
}

test(`Reward distribution is realistic`, () => {
  const negativeRatio = rewardDistribution.negative / trajectories.length;
  // Expect some failures but not too many (real systems have ~10-30% failures)
  return negativeRatio < 0.5 || `${(negativeRatio * 100).toFixed(0)}% negative rewards seems high`;
});

// === 5. Swarm Graph Validation ===
console.log('\n🔗 Swarm Graph (coordination-graph.json):');
const graph = JSON.parse(readFileSync(join(DATA_DIR, 'coordination-graph.json'), 'utf-8'));

test(`Has agent nodes (${Object.keys(graph.nodes || {}).length})`, () => {
  return Object.keys(graph.nodes || {}).length >= 3 || 'Too few agents';
});

test(`Has coordination edges (${Object.keys(graph.edges || {}).length})`, () => {
  return Object.keys(graph.edges || {}).length >= 5 || 'Too few edges';
});

test('Agents have capabilities', () => {
  const withCaps = Object.values(graph.nodes || {}).filter(n => n.capabilities?.length > 0);
  return withCaps.length > 0 || 'No capabilities defined';
});

test('Graph is connected', () => {
  const nodes = Object.keys(graph.nodes || {});
  const edges = Object.keys(graph.edges || {});
  if (nodes.length <= 1) return true;

  // Simple connectivity check
  const connected = new Set();
  connected.add(nodes[0]);

  let changed = true;
  while (changed) {
    changed = false;
    for (const edge of edges) {
      const [a, b] = edge.split(':');
      if (connected.has(a) && !connected.has(b)) {
        connected.add(b);
        changed = true;
      }
      if (connected.has(b) && !connected.has(a)) {
        connected.add(a);
        changed = true;
      }
    }
  }

  return connected.size === nodes.length || `Only ${connected.size}/${nodes.length} nodes connected`;
});

// === 6. Swarm State Validation ===
console.log('\n📋 Swarm State (swarm-state.json):');
const swarmState = JSON.parse(readFileSync(join(DATA_DIR, 'swarm-state.json'), 'utf-8'));

test('Pretrained flag is set', () => {
  return swarmState.pretrained === true || 'Not marked as pretrained';
});

test('Has pretraining timestamp', () => {
  return swarmState.pretrainedAt ? true : 'No timestamp';
});

test('Has stats', () => {
  return swarmState.stats && swarmState.stats.commands > 0 || 'No stats';
});

// === Summary ===
console.log('\n====================================');
console.log(`📊 Results: ${results.passed} passed, ${results.failed} failed, ${results.warnings} warnings`);

if (results.failed > 0) {
  console.log('\n❌ Validation FAILED - issues found');
  process.exit(1);
} else if (results.warnings > 0) {
  console.log('\n⚠️  Validation PASSED with warnings');
  process.exit(0);
} else {
  console.log('\n✅ Validation PASSED - system is healthy');
  process.exit(0);
}
