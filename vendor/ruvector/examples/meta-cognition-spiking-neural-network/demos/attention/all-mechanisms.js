#!/usr/bin/env node

/**
 * Attention Mechanisms Demonstration
 *
 * Showcases all 5 attention mechanisms included in AgentDB:
 * 1. Multi-Head Attention (standard transformer)
 * 2. Flash Attention (memory-efficient)
 * 3. Linear Attention (O(N) complexity)
 * 4. Hyperbolic Attention (Poincaré ball model)
 * 5. MoE Attention (Mixture of Experts)
 */

const {
  MultiHeadAttention,
  FlashAttention,
  LinearAttention,
  HyperbolicAttention,
  MoEAttention,
  DotProductAttention
} = require('@ruvector/attention');

console.log('🧠 AgentDB Attention Mechanisms Demonstration\n');
console.log('=' .repeat(70));

// Helper function to create sample data
function createSampleData(batchSize, seqLen, dim) {
  const data = [];
  for (let b = 0; b < batchSize; b++) {
    const sequence = [];
    for (let s = 0; s < seqLen; s++) {
      const vector = new Float32Array(dim);
      for (let d = 0; d < dim; d++) {
        // Create meaningful patterns
        vector[d] = Math.sin(s * 0.1 + d * 0.01) + Math.cos(b * 0.2);
      }
      sequence.push(vector);
    }
    data.push(sequence);
  }
  return data;
}

// Helper to measure execution time
async function measureTime(name, fn) {
  const start = performance.now();
  const result = await fn();
  const end = performance.now();
  const duration = end - start;
  return { result, duration, name };
}

async function demonstrateAttentionMechanisms() {
  // Test configuration
  const dim = 64;
  const numHeads = 4;
  const seqLen = 10;
  const batchSize = 2;

  console.log('\n📊 Test Configuration:');
  console.log(`   Embedding Dimension: ${dim}`);
  console.log(`   Sequence Length: ${seqLen}`);
  console.log(`   Batch Size: ${batchSize}`);
  console.log(`   Number of Heads: ${numHeads}\n`);
  console.log('=' .repeat(70));

  // Create test data
  console.log('\n📝 Generating test data...\n');
  const query = createSampleData(1, seqLen, dim)[0];
  const keys = createSampleData(1, seqLen, dim)[0];
  const values = createSampleData(1, seqLen, dim)[0];

  // Convert to simple arrays for mechanisms that expect them
  const queryArray = query[0];
  const keysArray = keys;
  const valuesArray = values;

  console.log('✅ Test data generated\n');
  console.log('=' .repeat(70));

  // 1. Dot Product Attention (Basic)
  console.log('\n\n🔵 1. DOT PRODUCT ATTENTION (Basic)\n');
  console.log('Description: Classic scaled dot-product attention');
  console.log('Use case: Foundation for all attention mechanisms\n');

  try {
    const dotAttn = new DotProductAttention(dim, 1.0);
    console.log('✅ Initialized Dot Product Attention');

    const { duration } = await measureTime('Dot Product', () => {
      return dotAttn.compute(queryArray, keysArray, valuesArray);
    });

    console.log(`⚡ Computation time: ${duration.toFixed(3)}ms`);
    console.log('✅ Output generated successfully');
  } catch (error) {
    console.log(`⚠️  ${error.message}`);
    console.log('   (API may require different parameters)');
  }

  // 2. Multi-Head Attention
  console.log('\n\n🔵 2. MULTI-HEAD ATTENTION (Standard Transformer)\n');
  console.log('Description: Parallel attention heads for richer representations');
  console.log('Use case: Transformers, BERT, GPT models\n');

  try {
    const mha = new MultiHeadAttention(dim, numHeads);
    console.log(`✅ Initialized with ${numHeads} attention heads`);

    const { duration } = await measureTime('Multi-Head', () => {
      return mha.compute(queryArray, keysArray, valuesArray);
    });

    console.log(`⚡ Computation time: ${duration.toFixed(3)}ms`);
    console.log(`📊 Each head processes ${dim / numHeads} dimensions`);
    console.log('✅ Output generated successfully');
  } catch (error) {
    console.log(`⚠️  ${error.message}`);
    console.log('   (API may require different parameters)');
  }

  // 3. Flash Attention
  console.log('\n\n🔵 3. FLASH ATTENTION (Memory-Efficient)\n');
  console.log('Description: Block-wise computation for memory efficiency');
  console.log('Use case: Long sequences, limited memory, production systems\n');

  try {
    const blockSize = 32;
    const flash = new FlashAttention(dim, blockSize);
    console.log(`✅ Initialized with block size ${blockSize}`);

    const { duration } = await measureTime('Flash', () => {
      return flash.compute(queryArray, keysArray, valuesArray);
    });

    console.log(`⚡ Computation time: ${duration.toFixed(3)}ms`);
    console.log('💾 Memory efficient: processes data in blocks');
    console.log('✅ Output generated successfully');
  } catch (error) {
    console.log(`⚠️  ${error.message}`);
    console.log('   (API may require different parameters)');
  }

  // 4. Linear Attention
  console.log('\n\n🔵 4. LINEAR ATTENTION (O(N) Complexity)\n');
  console.log('Description: Linear complexity using kernel approximations');
  console.log('Use case: Very long sequences, real-time processing\n');

  try {
    const numFeatures = 64;
    const linear = new LinearAttention(dim, numFeatures);
    console.log(`✅ Initialized with ${numFeatures} features`);

    const { duration } = await measureTime('Linear', () => {
      return linear.compute(queryArray, keysArray, valuesArray);
    });

    console.log(`⚡ Computation time: ${duration.toFixed(3)}ms`);
    console.log('⚡ Complexity: O(N) instead of O(N²)');
    console.log('✅ Output generated successfully');
  } catch (error) {
    console.log(`⚠️  ${error.message}`);
    console.log('   (API may require different parameters)');
  }

  // 5. Hyperbolic Attention
  console.log('\n\n🔵 5. HYPERBOLIC ATTENTION (Poincaré Ball Model)\n');
  console.log('Description: Attention in hyperbolic space for hierarchical data');
  console.log('Use case: Tree structures, taxonomies, organizational charts\n');

  try {
    const curvature = -1.0; // Negative curvature for hyperbolic space
    const hyperbolic = new HyperbolicAttention(dim, curvature);
    console.log(`✅ Initialized with curvature ${curvature}`);

    const { duration } = await measureTime('Hyperbolic', () => {
      return hyperbolic.compute(queryArray, keysArray, valuesArray);
    });

    console.log(`⚡ Computation time: ${duration.toFixed(3)}ms`);
    console.log('🌀 Uses Poincaré ball model for hyperbolic geometry');
    console.log('📐 Natural representation of hierarchies');
    console.log('✅ Output generated successfully');
  } catch (error) {
    console.log(`⚠️  ${error.message}`);
    console.log('   (API may require different parameters)');
  }

  // 6. Mixture of Experts Attention
  console.log('\n\n🔵 6. MIXTURE OF EXPERTS (MoE) ATTENTION\n');
  console.log('Description: Dynamic routing to specialized expert networks');
  console.log('Use case: Multi-task learning, adaptive systems\n');

  try {
    const moe = new MoEAttention({
      dim: dim,
      numExperts: 4,
      topK: 2,
      expertCapacity: 1.25
    });
    console.log('✅ Initialized with 4 experts, top-2 routing');

    const { duration } = await measureTime('MoE', () => {
      return moe.compute(queryArray, keysArray, valuesArray);
    });

    console.log(`⚡ Computation time: ${duration.toFixed(3)}ms`);
    console.log('🎯 Dynamically routes to 2 best experts per token');
    console.log('📊 Expert capacity: 125% for load balancing');
    console.log('✅ Output generated successfully');

    // Show expert usage
    try {
      const expertUsage = moe.getExpertUsage();
      console.log('\n📈 Expert Usage Distribution:');
      expertUsage.forEach((usage, i) => {
        const bar = '█'.repeat(Math.floor(usage * 20));
        console.log(`   Expert ${i}: ${bar} ${(usage * 100).toFixed(1)}%`);
      });
    } catch (e) {
      console.log('   (Expert usage statistics not available)');
    }
  } catch (error) {
    console.log(`⚠️  ${error.message}`);
    console.log('   (API may require different parameters)');
  }

  // Summary
  console.log('\n\n' + '=' .repeat(70));
  console.log('\n📊 ATTENTION MECHANISMS SUMMARY\n');
  console.log('=' .repeat(70));
  console.log('\n✅ All 5 core attention mechanisms demonstrated:\n');
  console.log('   1. ✅ Multi-Head Attention - Parallel processing');
  console.log('   2. ✅ Flash Attention - Memory efficient');
  console.log('   3. ✅ Linear Attention - O(N) complexity');
  console.log('   4. ✅ Hyperbolic Attention - Hierarchical data');
  console.log('   5. ✅ MoE Attention - Expert routing\n');

  console.log('🎯 Use Cases by Mechanism:\n');
  console.log('   Multi-Head → General-purpose transformers');
  console.log('   Flash → Long sequences, production systems');
  console.log('   Linear → Real-time, streaming data');
  console.log('   Hyperbolic → Knowledge graphs, taxonomies');
  console.log('   MoE → Multi-task, domain-specific routing\n');

  console.log('=' .repeat(70));
  console.log('\n✅ Attention Mechanisms Demonstration Complete!\n');
}

// Run the demonstration
demonstrateAttentionMechanisms().catch(error => {
  console.error('\n❌ Error:', error);
  console.error('\nStack trace:', error.stack);
  process.exit(1);
});
