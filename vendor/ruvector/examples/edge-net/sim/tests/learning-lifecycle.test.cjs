/**
 * Learning Module Lifecycle Simulation Tests
 * Tests pattern storage, trajectory recording, spike attention, and multi-head routing
 */

const assert = require('assert');

// Mock WASM module for testing
const createMockLearning = () => ({
  ReasoningBank: class {
    constructor() {
      this.patterns = new Map();
      this.nextId = 0;
    }

    store(patternJson) {
      try {
        const pattern = JSON.parse(patternJson);
        const id = this.nextId++;
        this.patterns.set(id, {
          pattern,
          usageCount: 0,
          lastUsed: Date.now()
        });
        return id;
      } catch {
        return -1;
      }
    }

    lookup(queryJson, k) {
      try {
        const query = JSON.parse(queryJson);
        const results = [];

        for (const [id, entry] of this.patterns.entries()) {
          const similarity = this.cosineSimilarity(query, entry.pattern.centroid);
          results.push({
            id,
            similarity,
            confidence: entry.pattern.confidence,
            optimal_allocation: entry.pattern.optimal_allocation,
            optimal_energy: entry.pattern.optimal_energy
          });
        }

        results.sort((a, b) => (b.similarity * b.confidence) - (a.similarity * a.confidence));
        return JSON.stringify(results.slice(0, k));
      } catch {
        return '[]';
      }
    }

    cosineSimilarity(a, b) {
      if (a.length !== b.length) return 0;
      let dot = 0, normA = 0, normB = 0;
      for (let i = 0; i < a.length; i++) {
        dot += a[i] * b[i];
        normA += a[i] * a[i];
        normB += b[i] * b[i];
      }
      normA = Math.sqrt(normA);
      normB = Math.sqrt(normB);
      return normA === 0 || normB === 0 ? 0 : dot / (normA * normB);
    }

    prune(minUsage, minConfidence) {
      let removed = 0;
      for (const [id, entry] of this.patterns.entries()) {
        if (entry.usageCount < minUsage || entry.pattern.confidence < minConfidence) {
          this.patterns.delete(id);
          removed++;
        }
      }
      return removed;
    }

    count() {
      return this.patterns.size;
    }

    getStats() {
      if (this.patterns.size === 0) return '{"total":0}';

      const entries = Array.from(this.patterns.values());
      const totalSamples = entries.reduce((sum, e) => sum + e.pattern.sample_count, 0);
      const avgConfidence = entries.reduce((sum, e) => sum + e.pattern.confidence, 0) / entries.length;
      const totalUsage = entries.reduce((sum, e) => sum + e.usageCount, 0);

      return JSON.stringify({
        total_patterns: this.patterns.size,
        total_samples: totalSamples,
        avg_confidence: avgConfidence,
        total_usage: totalUsage
      });
    }
  },

  TrajectoryTracker: class {
    constructor(maxSize) {
      this.trajectories = [];
      this.maxSize = maxSize;
      this.writePos = 0;
    }

    record(trajectoryJson) {
      try {
        const traj = JSON.parse(trajectoryJson);
        if (this.trajectories.length < this.maxSize) {
          this.trajectories.push(traj);
        } else {
          this.trajectories[this.writePos] = traj;
        }
        this.writePos = (this.writePos + 1) % this.maxSize;
        return true;
      } catch {
        return false;
      }
    }

    getStats() {
      if (this.trajectories.length === 0) return '{"total":0}';

      const total = this.trajectories.length;
      const successful = this.trajectories.filter(t => t.success).length;
      const avgLatency = this.trajectories.reduce((sum, t) => sum + t.latency_ms, 0) / total;
      const avgEfficiency = this.trajectories.reduce((sum, t) => {
        return sum + (t.energy_spent === 0 ? 0 : t.energy_earned / t.energy_spent);
      }, 0) / total;

      return JSON.stringify({
        total,
        successful,
        success_rate: successful / total,
        avg_latency_ms: avgLatency,
        avg_efficiency: avgEfficiency
      });
    }

    count() {
      return this.trajectories.length;
    }
  },

  SpikeDrivenAttention: class {
    energyRatio(seqLen, hiddenDim) {
      if (seqLen === 0 || hiddenDim === 0) return 1.0;

      const standardMults = 2 * seqLen * seqLen * hiddenDim;
      const avgSpikesPerNeuron = 8 * 0.3;
      const spikeAdds = seqLen * avgSpikesPerNeuron * hiddenDim;
      const multEnergyFactor = 3.7;

      const standardEnergy = standardMults * multEnergyFactor;
      const spikeEnergy = spikeAdds;

      return spikeEnergy === 0 ? 1.0 : standardEnergy / spikeEnergy;
    }
  },

  MultiHeadAttention: class {
    constructor(dim, numHeads) {
      this.dimValue = dim;
      this.numHeadsValue = numHeads;
    }

    dim() { return this.dimValue; }
    numHeads() { return this.numHeadsValue; }
  },

  NetworkLearning: class {
    constructor() {
      const mocks = createMockLearning();
      this.bank = new mocks.ReasoningBank();
      this.tracker = new mocks.TrajectoryTracker(1000);
      this.spike = new mocks.SpikeDrivenAttention();
      this.attention = new mocks.MultiHeadAttention(64, 4);
    }

    recordTrajectory(json) { return this.tracker.record(json); }
    storePattern(json) { return this.bank.store(json); }
    lookupPatterns(json, k) { return this.bank.lookup(json, k); }
    getEnergyRatio(seq, hidden) { return this.spike.energyRatio(seq, hidden); }

    getStats() {
      const bankStats = this.bank.getStats();
      const trajStats = this.tracker.getStats();
      const energyRatio = this.spike.energyRatio(64, 256);

      return JSON.stringify({
        reasoning_bank: JSON.parse(bankStats),
        trajectories: JSON.parse(trajStats),
        spike_energy_ratio: energyRatio,
        learning_rate: 0.01
      });
    }

    trajectoryCount() { return this.tracker.count(); }
    patternCount() { return this.bank.count(); }
    prune(minUsage, minConf) { return this.bank.prune(minUsage, minConf); }
  }
});

/**
 * Test 1: Pattern Storage and Retrieval Cycles
 */
function testPatternStorageRetrieval() {
  console.log('\n=== Test 1: Pattern Storage and Retrieval Cycles ===');

  const wasm = createMockLearning();
  const learning = new wasm.NetworkLearning();

  const patterns = [
    {
      centroid: [1.0, 0.0, 0.0],
      optimal_allocation: 0.8,
      optimal_energy: 100,
      confidence: 0.9,
      sample_count: 10,
      avg_latency_ms: 50.0,
      avg_success_rate: 0.95
    },
    {
      centroid: [0.0, 1.0, 0.0],
      optimal_allocation: 0.7,
      optimal_energy: 120,
      confidence: 0.85,
      sample_count: 8,
      avg_latency_ms: 60.0,
      avg_success_rate: 0.90
    },
    {
      centroid: [0.707, 0.707, 0.0],
      optimal_allocation: 0.75,
      optimal_energy: 110,
      confidence: 0.88,
      sample_count: 9,
      avg_latency_ms: 55.0,
      avg_success_rate: 0.92
    }
  ];

  // Store patterns
  const ids = patterns.map(p => learning.storePattern(JSON.stringify(p)));
  console.log(`✓ Stored ${ids.length} patterns`);
  assert.strictEqual(learning.patternCount(), 3);

  // Lookup similar patterns
  const query = [0.9, 0.1, 0.0];
  const results = JSON.parse(learning.lookupPatterns(JSON.stringify(query), 2));
  console.log(`✓ Retrieved ${results.length} similar patterns`);
  assert.strictEqual(results.length, 2);
  assert.ok(results[0].similarity > results[1].similarity);

  // Verify pattern quality
  const stats = JSON.parse(learning.getStats());
  console.log(`✓ Pattern bank stats:`, stats.reasoning_bank);
  assert.strictEqual(stats.reasoning_bank.total_patterns, 3);
  assert.ok(stats.reasoning_bank.avg_confidence > 0.8);

  console.log('✅ Pattern Storage and Retrieval Test PASSED');
  return {
    patterns_stored: ids.length,
    retrieval_accuracy: results[0].similarity,
    avg_confidence: stats.reasoning_bank.avg_confidence
  };
}

/**
 * Test 2: Trajectory Recording and Analysis
 */
function testTrajectoryRecording() {
  console.log('\n=== Test 2: Trajectory Recording and Analysis ===');

  const wasm = createMockLearning();
  const learning = new wasm.NetworkLearning();

  // Record diverse trajectories
  const trajectories = [];
  for (let i = 0; i < 100; i++) {
    const success = Math.random() > 0.2; // 80% success rate
    const traj = {
      task_vector: Array(16).fill(0).map(() => Math.random()),
      latency_ms: 50 + Math.random() * 100,
      energy_spent: 50 + Math.floor(Math.random() * 50),
      energy_earned: success ? 100 + Math.floor(Math.random() * 50) : 0,
      success,
      executor_id: `node-${i % 10}`,
      timestamp: Date.now() + i * 1000
    };
    trajectories.push(traj);
    learning.recordTrajectory(JSON.stringify(traj));
  }

  console.log(`✓ Recorded ${trajectories.length} trajectories`);
  assert.strictEqual(learning.trajectoryCount(), 100);

  // Analyze statistics
  const stats = JSON.parse(learning.getStats());
  const trajStats = stats.trajectories;
  console.log(`✓ Trajectory stats:`, trajStats);

  assert.ok(trajStats.success_rate > 0.7);
  assert.ok(trajStats.avg_latency_ms > 50 && trajStats.avg_latency_ms < 150);
  assert.ok(trajStats.avg_efficiency > 1.0);

  console.log('✅ Trajectory Recording Test PASSED');
  return {
    total_trajectories: trajStats.total,
    success_rate: trajStats.success_rate,
    avg_efficiency: trajStats.avg_efficiency
  };
}

/**
 * Test 3: Spike-Driven Attention Energy Efficiency
 */
function testSpikeAttentionEnergy() {
  console.log('\n=== Test 3: Spike-Driven Attention Energy Efficiency ===');

  const wasm = createMockLearning();
  const learning = new wasm.NetworkLearning();

  const testCases = [
    { seqLen: 64, hiddenDim: 256, expectedMin: 50, expectedMax: 250 },
    { seqLen: 128, hiddenDim: 512, expectedMin: 70, expectedMax: 500 },
    { seqLen: 32, hiddenDim: 128, expectedMin: 40, expectedMax: 150 }
  ];

  const results = testCases.map(tc => {
    const ratio = learning.getEnergyRatio(tc.seqLen, tc.hiddenDim);
    console.log(`✓ Seq=${tc.seqLen}, Hidden=${tc.hiddenDim}: ${ratio.toFixed(2)}x energy savings`);

    assert.ok(ratio >= tc.expectedMin, `Expected >= ${tc.expectedMin}, got ${ratio}`);
    assert.ok(ratio <= tc.expectedMax, `Expected <= ${tc.expectedMax}, got ${ratio}`);

    return { seqLen: tc.seqLen, hiddenDim: tc.hiddenDim, ratio };
  });

  // Verify edge cases
  const emptyRatio = learning.getEnergyRatio(0, 0);
  assert.strictEqual(emptyRatio, 1.0);
  console.log('✓ Empty case handled correctly');

  console.log('✅ Spike Attention Energy Test PASSED');
  return { energy_savings: results };
}

/**
 * Test 4: Multi-Head Attention Task Routing
 */
function testMultiHeadRouting() {
  console.log('\n=== Test 4: Multi-Head Attention Task Routing ===');

  const wasm = createMockLearning();
  const attention = new wasm.MultiHeadAttention(64, 4);

  assert.strictEqual(attention.dim(), 64);
  assert.strictEqual(attention.numHeads(), 4);
  console.log(`✓ Multi-head attention: ${attention.numHeads()} heads, ${attention.dim()} dims`);

  // Test different configurations
  const configs = [
    { dim: 128, heads: 8 },
    { dim: 256, heads: 16 },
    { dim: 512, heads: 32 }
  ];

  configs.forEach(cfg => {
    const attn = new wasm.MultiHeadAttention(cfg.dim, cfg.heads);
    assert.strictEqual(attn.dim(), cfg.dim);
    assert.strictEqual(attn.numHeads(), cfg.heads);
    console.log(`✓ Config validated: ${cfg.heads} heads x ${cfg.dim} dims`);
  });

  console.log('✅ Multi-Head Routing Test PASSED');
  return { configurations_tested: configs.length };
}

/**
 * Test 5: Pattern Pruning and Memory Management
 */
function testPatternPruning() {
  console.log('\n=== Test 5: Pattern Pruning and Memory Management ===');

  const wasm = createMockLearning();
  const learning = new wasm.NetworkLearning();

  // Store high and low quality patterns
  const patterns = [
    { centroid: [1, 0, 0], optimal_allocation: 0.9, optimal_energy: 100, confidence: 0.95, sample_count: 20, avg_latency_ms: 50, avg_success_rate: 0.98 },
    { centroid: [0, 1, 0], optimal_allocation: 0.5, optimal_energy: 100, confidence: 0.4, sample_count: 2, avg_latency_ms: 200, avg_success_rate: 0.5 },
    { centroid: [0, 0, 1], optimal_allocation: 0.3, optimal_energy: 100, confidence: 0.3, sample_count: 1, avg_latency_ms: 300, avg_success_rate: 0.3 }
  ];

  patterns.forEach(p => learning.storePattern(JSON.stringify(p)));
  console.log(`✓ Stored ${learning.patternCount()} patterns (mixed quality)`);

  // Prune low quality patterns
  const pruned = learning.prune(5, 0.5);
  console.log(`✓ Pruned ${pruned} low-quality patterns`);

  assert.ok(pruned >= 1);
  assert.ok(learning.patternCount() < patterns.length);

  console.log('✅ Pattern Pruning Test PASSED');
  return { patterns_pruned: pruned, patterns_remaining: learning.patternCount() };
}

/**
 * Test 6: High-Throughput Learning Pipeline
 */
function testHighThroughputLearning() {
  console.log('\n=== Test 6: High-Throughput Learning Pipeline ===');

  const wasm = createMockLearning();
  const learning = new wasm.NetworkLearning();

  const startTime = Date.now();

  // Simulate high-throughput scenario
  const trajCount = 1000;
  const patternCount = 100;

  for (let i = 0; i < trajCount; i++) {
    learning.recordTrajectory(JSON.stringify({
      task_vector: [Math.random(), Math.random(), Math.random()],
      latency_ms: 50 + Math.random() * 50,
      energy_spent: 50,
      energy_earned: Math.random() > 0.2 ? 100 : 0,
      success: Math.random() > 0.2,
      executor_id: `node-${i % 10}`,
      timestamp: Date.now() + i
    }));
  }

  for (let i = 0; i < patternCount; i++) {
    learning.storePattern(JSON.stringify({
      centroid: [Math.random(), Math.random(), Math.random()],
      optimal_allocation: 0.5 + Math.random() * 0.5,
      optimal_energy: 100,
      confidence: 0.5 + Math.random() * 0.5,
      sample_count: 5 + Math.floor(Math.random() * 15),
      avg_latency_ms: 50 + Math.random() * 100,
      avg_success_rate: 0.7 + Math.random() * 0.3
    }));
  }

  const duration = Date.now() - startTime;
  const throughput = (trajCount + patternCount) / (duration / 1000);

  console.log(`✓ Processed ${trajCount} trajectories + ${patternCount} patterns in ${duration}ms`);
  console.log(`✓ Throughput: ${throughput.toFixed(2)} ops/sec`);

  assert.strictEqual(learning.trajectoryCount(), trajCount);
  assert.strictEqual(learning.patternCount(), patternCount);

  console.log('✅ High-Throughput Learning Test PASSED');
  return { throughput_ops_per_sec: throughput, duration_ms: duration };
}

/**
 * Run all learning lifecycle tests
 */
function runLearningTests() {
  console.log('\n╔══════════════════════════════════════════════════════╗');
  console.log('║  Learning Module Lifecycle Simulation Tests         ║');
  console.log('╚══════════════════════════════════════════════════════╝');

  const results = {
    timestamp: new Date().toISOString(),
    test_suite: 'learning_lifecycle',
    tests: {}
  };

  try {
    results.tests.pattern_storage = testPatternStorageRetrieval();
    results.tests.trajectory_recording = testTrajectoryRecording();
    results.tests.spike_attention = testSpikeAttentionEnergy();
    results.tests.multi_head_routing = testMultiHeadRouting();
    results.tests.pattern_pruning = testPatternPruning();
    results.tests.high_throughput = testHighThroughputLearning();

    results.summary = {
      total_tests: 6,
      passed: 6,
      failed: 0,
      success_rate: 1.0
    };

    console.log('\n╔══════════════════════════════════════════════════════╗');
    console.log('║  All Learning Lifecycle Tests PASSED ✅              ║');
    console.log('╚══════════════════════════════════════════════════════╝\n');

  } catch (error) {
    console.error('\n❌ Test failed:', error.message);
    console.error(error.stack);
    results.summary = { total_tests: 6, passed: 0, failed: 1, error: error.message };
    process.exit(1);
  }

  return results;
}

// Run if called directly
if (require.main === module) {
  const results = runLearningTests();
  const fs = require('fs');
  fs.writeFileSync(
    './reports/learning-lifecycle-results.json',
    JSON.stringify(results, null, 2)
  );
  console.log('📊 Results saved to: reports/learning-lifecycle-results.json');
}

module.exports = { runLearningTests, createMockLearning };
