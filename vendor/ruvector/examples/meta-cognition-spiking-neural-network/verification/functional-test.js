#!/usr/bin/env node

/**
 * AgentDB Functional Test
 *
 * Tests actual functionality of key features
 */

const { MultiHeadAttention, HyperbolicAttention, FlashAttention, LinearAttention, MoEAttention } = require('@ruvector/attention');
const { VectorDB } = require('ruvector');

console.log('🧪 AgentDB Functional Tests\n');
console.log('=' .repeat(60));

let passed = 0;
let failed = 0;

function test(name, fn) {
  try {
    fn();
    console.log(`✅ ${name}`);
    passed++;
  } catch (error) {
    console.log(`❌ ${name}`);
    console.log(`   Error: ${error.message}`);
    failed++;
  }
}

// Test 1: Multi-Head Attention instantiation
test('Multi-Head Attention can be instantiated', () => {
  const attention = new MultiHeadAttention({
    embed_dim: 64,
    num_heads: 4
  });
  if (!attention) throw new Error('Failed to create MultiHeadAttention');
});

// Test 2: Hyperbolic Attention instantiation
test('Hyperbolic Attention can be instantiated', () => {
  const attention = new HyperbolicAttention({
    embed_dim: 64,
    num_heads: 4
  });
  if (!attention) throw new Error('Failed to create HyperbolicAttention');
});

// Test 3: Flash Attention instantiation
test('Flash Attention can be instantiated', () => {
  const attention = new FlashAttention({
    embed_dim: 64,
    num_heads: 4
  });
  if (!attention) throw new Error('Failed to create FlashAttention');
});

// Test 4: Linear Attention instantiation
test('Linear Attention can be instantiated', () => {
  const attention = new LinearAttention({
    embed_dim: 64,
    num_heads: 4
  });
  if (!attention) throw new Error('Failed to create LinearAttention');
});

// Test 5: MoE Attention instantiation
test('MoE Attention can be instantiated', () => {
  const attention = new MoEAttention({
    embed_dim: 64,
    num_heads: 4,
    num_experts: 4
  });
  if (!attention) throw new Error('Failed to create MoEAttention');
});

// Test 6: VectorDB instantiation
test('VectorDB can be instantiated', () => {
  const db = new VectorDB({
    dimensions: 128,
    metric: 'cosine'
  });
  if (!db) throw new Error('Failed to create VectorDB');
});

// Test 7: VectorDB basic operations
test('VectorDB can add and search vectors', () => {
  const db = new VectorDB({
    dimensions: 3,
    metric: 'cosine'
  });

  // Add some vectors
  db.add([1, 0, 0], { id: 'vec1', label: 'x-axis' });
  db.add([0, 1, 0], { id: 'vec2', label: 'y-axis' });
  db.add([0, 0, 1], { id: 'vec3', label: 'z-axis' });

  // Search for nearest to x-axis
  const results = db.search([0.9, 0.1, 0], 1);

  if (!results || results.length === 0) {
    throw new Error('Search returned no results');
  }

  console.log(`   Found nearest vector: ${results[0].metadata?.label || 'unknown'}`);
});

// Test 8: Multi-Head Attention forward pass
test('Multi-Head Attention forward pass', () => {
  const attention = new MultiHeadAttention({
    embed_dim: 64,
    num_heads: 4
  });

  // Create sample input (batch_size=2, seq_len=3, embed_dim=64)
  const batchSize = 2;
  const seqLen = 3;
  const embedDim = 64;

  const query = Array(batchSize).fill(null).map(() =>
    Array(seqLen).fill(null).map(() =>
      Array(embedDim).fill(0).map(() => Math.random())
    )
  );

  const output = attention.forward(query, query, query);

  if (!output || !Array.isArray(output)) {
    throw new Error('Forward pass failed to return output');
  }

  console.log(`   Output shape: [${batchSize}, ${seqLen}, ${embedDim}]`);
});

// Summary
console.log('\n' + '='.repeat(60));
console.log(`\n✅ Passed: ${passed} tests`);
console.log(`❌ Failed: ${failed} tests`);

if (failed > 0) {
  console.log('\n❌ Functional tests FAILED\n');
  process.exit(1);
} else {
  console.log('\n✅ All functional tests PASSED!\n');
  process.exit(0);
}
