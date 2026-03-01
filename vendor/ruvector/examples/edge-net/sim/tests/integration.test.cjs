/**
 * Integration Scenario Tests
 * Tests combined learning + RAC workflows, high-throughput, concurrent access, and memory usage
 */

const assert = require('assert');
const crypto = require('crypto');
const { createMockLearning } = require('./learning-lifecycle.test.cjs');
const { createMockRAC } = require('./rac-coherence.test.cjs');

/**
 * Test 1: Combined Learning + Coherence Workflow
 */
function testCombinedLearningCoherence() {
  console.log('\n=== Test 1: Combined Learning + Coherence Workflow ===');

  const learningWasm = createMockLearning();
  const racWasm = createMockRAC();

  const learning = new learningWasm.NetworkLearning();
  const coherence = new racWasm.CoherenceEngine();

  // Scenario: AI model makes predictions, RAC validates them
  const context = crypto.randomBytes(32);

  // Step 1: Learning phase - record successful patterns
  for (let i = 0; i < 20; i++) {
    const trajectory = {
      task_vector: [Math.random(), Math.random(), Math.random()],
      latency_ms: 50 + Math.random() * 50,
      energy_spent: 50,
      energy_earned: 100,
      success: true,
      executor_id: `node-${i % 5}`,
      timestamp: Date.now() + i * 1000
    };
    learning.recordTrajectory(JSON.stringify(trajectory));

    // Extract pattern
    if (i % 5 === 0) {
      const pattern = {
        centroid: trajectory.task_vector,
        optimal_allocation: 0.8,
        optimal_energy: 100,
        confidence: 0.9,
        sample_count: 5,
        avg_latency_ms: 60,
        avg_success_rate: 1.0
      };
      learning.storePattern(JSON.stringify(pattern));
    }
  }

  console.log(`✓ Learning: ${learning.trajectoryCount()} trajectories, ${learning.patternCount()} patterns`);

  // Step 2: Make prediction and assert it to RAC
  const query = [0.5, 0.5, 0.0];
  const similar = JSON.parse(learning.lookupPatterns(JSON.stringify(query), 1));

  const prediction = {
    Assert: {
      proposition: Buffer.from(`prediction: energy=${similar[0].optimal_energy}`),
      evidence: [{
        kind: 'hash',
        pointer: Array.from(crypto.randomBytes(32))
      }],
      confidence: similar[0].confidence,
      expires_at_unix_ms: null
    }
  };

  const predEvent = {
    id: Array.from(crypto.randomBytes(32)),
    prev: null,
    ts_unix_ms: Date.now(),
    author: Array.from(crypto.randomBytes(32)),
    context: Array.from(context),
    ruvector: { dims: query },
    kind: prediction,
    sig: Array.from(crypto.randomBytes(64))
  };

  coherence.ingest(predEvent);
  console.log('✓ Prediction asserted to RAC');

  // Step 3: Another model challenges the prediction
  const counterPrediction = {
    Assert: {
      proposition: Buffer.from(`prediction: energy=150`),
      evidence: [],
      confidence: 0.7,
      expires_at_unix_ms: null
    }
  };

  const counterEvent = {
    id: Array.from(crypto.randomBytes(32)),
    prev: null,
    ts_unix_ms: Date.now(),
    author: Array.from(crypto.randomBytes(32)),
    context: Array.from(context),
    ruvector: { dims: [0.6, 0.4, 0.0] },
    kind: counterPrediction,
    sig: Array.from(crypto.randomBytes(64))
  };

  coherence.ingest(counterEvent);
  console.log('✓ Counter-prediction asserted');

  // Step 4: Challenge and resolve
  const challenge = {
    id: Array.from(crypto.randomBytes(32)),
    prev: null,
    ts_unix_ms: Date.now(),
    author: Array.from(crypto.randomBytes(32)),
    context: Array.from(context),
    ruvector: { dims: [0, 0, 0] },
    kind: {
      Challenge: {
        conflict_id: Array.from(crypto.randomBytes(32)),
        claim_ids: [predEvent.id, counterEvent.id],
        reason: 'Conflicting predictions',
        requested_proofs: ['model_trace']
      }
    },
    sig: Array.from(crypto.randomBytes(64))
  };

  coherence.ingest(challenge);
  console.log('✓ Challenge opened');

  const resolution = {
    id: Array.from(crypto.randomBytes(32)),
    prev: null,
    ts_unix_ms: Date.now(),
    author: Array.from(crypto.randomBytes(32)),
    context: Array.from(context),
    ruvector: { dims: [0, 0, 0] },
    kind: {
      Resolution: {
        conflict_id: challenge.kind.Challenge.conflict_id,
        accepted: [predEvent.id], // Higher confidence wins
        deprecated: [counterEvent.id],
        rationale: [],
        authority_sigs: []
      }
    },
    sig: Array.from(crypto.randomBytes(64))
  };

  coherence.ingest(resolution);
  console.log('✓ Resolution applied');

  // Verify integration
  assert.strictEqual(coherence.eventCount(), 5);
  assert.strictEqual(coherence.conflictCount(), 1);

  const stats = JSON.parse(coherence.getStats());
  assert.strictEqual(stats.conflicts_resolved, 1);

  console.log('✅ Combined Learning + Coherence Test PASSED');
  return {
    learning_patterns: learning.patternCount(),
    learning_trajectories: learning.trajectoryCount(),
    rac_events: coherence.eventCount(),
    rac_conflicts: coherence.conflictCount(),
    integrated_workflow: 'success'
  };
}

/**
 * Test 2: High-Throughput Event Processing
 */
function testHighThroughputIntegration() {
  console.log('\n=== Test 2: High-Throughput Event Processing ===');

  const learningWasm = createMockLearning();
  const racWasm = createMockRAC();

  const learning = new learningWasm.NetworkLearning();
  const coherence = new racWasm.CoherenceEngine();

  const startTime = Date.now();
  const iterations = 500;

  for (let i = 0; i < iterations; i++) {
    // Learning trajectory
    learning.recordTrajectory(JSON.stringify({
      task_vector: [Math.random(), Math.random(), Math.random()],
      latency_ms: 50 + Math.random() * 50,
      energy_spent: 50,
      energy_earned: Math.random() > 0.2 ? 100 : 0,
      success: Math.random() > 0.2,
      executor_id: `node-${i % 10}`,
      timestamp: Date.now() + i
    }));

    // RAC event
    if (i % 2 === 0) {
      coherence.ingest({
        id: Array.from(crypto.randomBytes(32)),
        prev: null,
        ts_unix_ms: Date.now() + i,
        author: Array.from(crypto.randomBytes(32)),
        context: Array.from(crypto.randomBytes(32)),
        ruvector: { dims: [Math.random(), Math.random(), Math.random()] },
        kind: {
          Assert: {
            proposition: Buffer.from(`claim-${i}`),
            evidence: [],
            confidence: 0.7 + Math.random() * 0.3,
            expires_at_unix_ms: null
          }
        },
        sig: Array.from(crypto.randomBytes(64))
      });
    }

    // Pattern extraction every 10 iterations
    if (i % 10 === 0 && i > 0) {
      learning.storePattern(JSON.stringify({
        centroid: [Math.random(), Math.random(), Math.random()],
        optimal_allocation: 0.7 + Math.random() * 0.3,
        optimal_energy: 100,
        confidence: 0.8 + Math.random() * 0.2,
        sample_count: 10,
        avg_latency_ms: 60,
        avg_success_rate: 0.9
      }));
    }
  }

  const duration = Date.now() - startTime;
  const totalOps = learning.trajectoryCount() + coherence.eventCount() + learning.patternCount();
  const throughput = totalOps / (duration / 1000);

  console.log(`✓ Processed ${totalOps} total operations in ${duration}ms`);
  console.log(`✓ Learning: ${learning.trajectoryCount()} trajectories, ${learning.patternCount()} patterns`);
  console.log(`✓ RAC: ${coherence.eventCount()} events`);
  console.log(`✓ Combined throughput: ${throughput.toFixed(2)} ops/sec`);

  assert.ok(throughput > 100, 'Throughput should exceed 100 ops/sec');

  console.log('✅ High-Throughput Integration Test PASSED');
  return {
    duration_ms: duration,
    throughput_ops_per_sec: throughput,
    learning_ops: learning.trajectoryCount() + learning.patternCount(),
    rac_ops: coherence.eventCount()
  };
}

/**
 * Test 3: Concurrent Access Patterns
 */
function testConcurrentAccess() {
  console.log('\n=== Test 3: Concurrent Access Patterns ===');

  const learningWasm = createMockLearning();
  const racWasm = createMockRAC();

  const learning = new learningWasm.NetworkLearning();
  const coherence = new racWasm.CoherenceEngine();

  // Simulate concurrent writers
  const contexts = Array(5).fill(0).map(() => crypto.randomBytes(32));
  const writers = 10;
  const opsPerWriter = 50;

  const startTime = Date.now();

  // Simulate interleaved operations from multiple "threads"
  for (let op = 0; op < opsPerWriter; op++) {
    for (let writer = 0; writer < writers; writer++) {
      const context = contexts[writer % contexts.length];

      // Learning write
      learning.recordTrajectory(JSON.stringify({
        task_vector: [Math.random(), Math.random(), Math.random()],
        latency_ms: 50,
        energy_spent: 50,
        energy_earned: 100,
        success: true,
        executor_id: `writer-${writer}`,
        timestamp: Date.now() + op * writers + writer
      }));

      // RAC write
      coherence.ingest({
        id: Array.from(crypto.randomBytes(32)),
        prev: null,
        ts_unix_ms: Date.now() + op * writers + writer,
        author: Array.from(crypto.randomBytes(32)),
        context: Array.from(context),
        ruvector: { dims: [0, 0, 0] },
        kind: {
          Assert: {
            proposition: Buffer.from(`writer-${writer}-op-${op}`),
            evidence: [],
            confidence: 0.8,
            expires_at_unix_ms: null
          }
        },
        sig: Array.from(crypto.randomBytes(64))
      });

      // Concurrent reads
      if (learning.patternCount() > 0) {
        learning.lookupPatterns(JSON.stringify([0.5, 0.5, 0.0]), 3);
      }

      if (coherence.eventCount() > 0) {
        coherence.getStats();
      }
    }
  }

  const duration = Date.now() - startTime;
  const totalOps = writers * opsPerWriter * 2; // 2 ops per iteration

  console.log(`✓ Simulated ${writers} concurrent writers`);
  console.log(`✓ ${opsPerWriter} ops per writer`);
  console.log(`✓ Total: ${totalOps} interleaved operations`);
  console.log(`✓ Duration: ${duration}ms`);

  assert.strictEqual(learning.trajectoryCount(), writers * opsPerWriter);
  assert.strictEqual(coherence.eventCount(), writers * opsPerWriter);

  console.log('✅ Concurrent Access Test PASSED');
  return {
    concurrent_writers: writers,
    ops_per_writer: opsPerWriter,
    total_ops: totalOps,
    duration_ms: duration
  };
}

/**
 * Test 4: Memory Usage Under Load
 */
function testMemoryUsage() {
  console.log('\n=== Test 4: Memory Usage Under Load ===');

  const learningWasm = createMockLearning();
  const racWasm = createMockRAC();

  const learning = new learningWasm.NetworkLearning();
  const coherence = new racWasm.CoherenceEngine();

  const memBefore = process.memoryUsage();

  // Load test
  const loadIterations = 1000;

  for (let i = 0; i < loadIterations; i++) {
    learning.recordTrajectory(JSON.stringify({
      task_vector: Array(128).fill(0).map(() => Math.random()), // Large vectors
      latency_ms: 50,
      energy_spent: 50,
      energy_earned: 100,
      success: true,
      executor_id: `node-${i % 20}`,
      timestamp: Date.now() + i
    }));

    if (i % 10 === 0) {
      learning.storePattern(JSON.stringify({
        centroid: Array(128).fill(0).map(() => Math.random()),
        optimal_allocation: 0.8,
        optimal_energy: 100,
        confidence: 0.9,
        sample_count: 10,
        avg_latency_ms: 50,
        avg_success_rate: 0.95
      }));
    }

    coherence.ingest({
      id: Array.from(crypto.randomBytes(32)),
      prev: null,
      ts_unix_ms: Date.now() + i,
      author: Array.from(crypto.randomBytes(32)),
      context: Array.from(crypto.randomBytes(32)),
      ruvector: { dims: Array(128).fill(0).map(() => Math.random()) },
      kind: {
        Assert: {
          proposition: Buffer.from(`claim-${i}`.repeat(10)), // Larger payloads
          evidence: Array(5).fill(0).map(() => ({
            kind: 'hash',
            pointer: Array.from(crypto.randomBytes(32))
          })),
          confidence: 0.8,
          expires_at_unix_ms: null
        }
      },
      sig: Array.from(crypto.randomBytes(64))
    });
  }

  global.gc && global.gc(); // Force GC if available

  const memAfter = process.memoryUsage();
  const heapGrowth = memAfter.heapUsed - memBefore.heapUsed;
  const heapGrowthMB = heapGrowth / 1024 / 1024;

  console.log(`✓ Loaded ${loadIterations} iterations`);
  console.log(`✓ Heap growth: ${heapGrowthMB.toFixed(2)} MB`);
  console.log(`✓ Per-operation: ${(heapGrowth / loadIterations / 1024).toFixed(2)} KB`);

  // Memory should be reasonable (< 100MB for 1000 iterations)
  assert.ok(heapGrowthMB < 100, `Heap growth ${heapGrowthMB}MB exceeds limit`);

  console.log('✅ Memory Usage Test PASSED');
  return {
    iterations: loadIterations,
    heap_growth_mb: heapGrowthMB,
    per_op_kb: heapGrowth / loadIterations / 1024
  };
}

/**
 * Test 5: Network Phase Transitions
 */
function testNetworkPhaseTransitions() {
  console.log('\n=== Test 5: Network Phase Transitions ===');

  const learningWasm = createMockLearning();
  const racWasm = createMockRAC();

  // Phase 1: Genesis (0-10 nodes)
  console.log('\n--- Phase 1: Genesis (0-10 nodes) ---');
  let learning = new learningWasm.NetworkLearning();
  let coherence = new racWasm.CoherenceEngine();

  for (let i = 0; i < 10; i++) {
    learning.recordTrajectory(JSON.stringify({
      task_vector: [0.1, 0.1, 0.1],
      latency_ms: 200, // Slower initially
      energy_spent: 50,
      energy_earned: 60,
      success: true,
      executor_id: `genesis-node-${i}`,
      timestamp: Date.now() + i * 1000
    }));
  }

  const genesisStats = JSON.parse(learning.getStats());
  console.log(`✓ Genesis: ${genesisStats.trajectories.total} trajectories`);
  console.log(`✓ Average latency: ${genesisStats.trajectories.avg_latency_ms.toFixed(2)}ms`);

  // Phase 2: Growth (11-100 nodes)
  console.log('\n--- Phase 2: Growth (11-100 nodes) ---');
  for (let i = 10; i < 100; i++) {
    learning.recordTrajectory(JSON.stringify({
      task_vector: [0.3, 0.3, 0.3],
      latency_ms: 150, // Improving
      energy_spent: 50,
      energy_earned: 80,
      success: true,
      executor_id: `growth-node-${i}`,
      timestamp: Date.now() + i * 1000
    }));

    // Start extracting patterns
    if (i % 10 === 0) {
      learning.storePattern(JSON.stringify({
        centroid: [0.3, 0.3, 0.3],
        optimal_allocation: 0.7,
        optimal_energy: 80,
        confidence: 0.8,
        sample_count: 10,
        avg_latency_ms: 150,
        avg_success_rate: 0.85
      }));
    }

    // RAC becomes active
    if (i % 5 === 0) {
      coherence.ingest({
        id: Array.from(crypto.randomBytes(32)),
        prev: null,
        ts_unix_ms: Date.now() + i * 1000,
        author: Array.from(crypto.randomBytes(32)),
        context: Array.from(crypto.randomBytes(32)),
        ruvector: { dims: [0.3, 0.3, 0.3] },
        kind: {
          Assert: {
            proposition: Buffer.from(`growth-claim-${i}`),
            evidence: [],
            confidence: 0.75,
            expires_at_unix_ms: null
          }
        },
        sig: Array.from(crypto.randomBytes(64))
      });
    }
  }

  const growthStats = JSON.parse(learning.getStats());
  console.log(`✓ Growth: ${growthStats.trajectories.total} trajectories, ${learning.patternCount()} patterns`);
  console.log(`✓ RAC events: ${coherence.eventCount()}`);

  // Phase 3: Maturation (100+ nodes, optimized)
  console.log('\n--- Phase 3: Maturation (optimized performance) ---');
  for (let i = 100; i < 200; i++) {
    learning.recordTrajectory(JSON.stringify({
      task_vector: [0.8, 0.8, 0.8],
      latency_ms: 60, // Optimal
      energy_spent: 50,
      energy_earned: 120,
      success: true,
      executor_id: `mature-node-${i}`,
      timestamp: Date.now() + i * 1000
    }));
  }

  const matureStats = JSON.parse(learning.getStats());
  console.log(`✓ Maturation: ${matureStats.trajectories.total} trajectories`);
  console.log(`✓ Average efficiency: ${matureStats.trajectories.avg_efficiency.toFixed(2)}`);

  // Phase 4: Independence (self-sustaining)
  console.log('\n--- Phase 4: Independence (self-sustaining) ---');
  const pruned = learning.prune(3, 0.6);
  console.log(`✓ Pruned ${pruned} low-quality patterns`);
  console.log(`✓ Remaining patterns: ${learning.patternCount()}`);

  assert.ok(genesisStats.trajectories.avg_latency_ms > matureStats.trajectories.avg_latency_ms);
  assert.ok(matureStats.trajectories.avg_efficiency > genesisStats.trajectories.avg_efficiency);

  console.log('✅ Network Phase Transitions Test PASSED');
  return {
    genesis_latency: genesisStats.trajectories.avg_latency_ms,
    mature_latency: matureStats.trajectories.avg_latency_ms,
    mature_efficiency: matureStats.trajectories.avg_efficiency,
    final_patterns: learning.patternCount(),
    rac_events: coherence.eventCount()
  };
}

/**
 * Run all integration tests
 */
function runIntegrationTests() {
  console.log('\n╔══════════════════════════════════════════════════════╗');
  console.log('║  Integration Scenario Simulation Tests              ║');
  console.log('╚══════════════════════════════════════════════════════╝');

  const results = {
    timestamp: new Date().toISOString(),
    test_suite: 'integration_scenarios',
    tests: {}
  };

  try {
    results.tests.combined_workflow = testCombinedLearningCoherence();
    results.tests.high_throughput = testHighThroughputIntegration();
    results.tests.concurrent_access = testConcurrentAccess();
    results.tests.memory_usage = testMemoryUsage();
    results.tests.phase_transitions = testNetworkPhaseTransitions();

    results.summary = {
      total_tests: 5,
      passed: 5,
      failed: 0,
      success_rate: 1.0
    };

    console.log('\n╔══════════════════════════════════════════════════════╗');
    console.log('║  All Integration Tests PASSED ✅                     ║');
    console.log('╚══════════════════════════════════════════════════════╝\n');

  } catch (error) {
    console.error('\n❌ Test failed:', error.message);
    console.error(error.stack);
    results.summary = { total_tests: 5, passed: 0, failed: 1, error: error.message };
    process.exit(1);
  }

  return results;
}

// Run if called directly
if (require.main === module) {
  const results = runIntegrationTests();
  const fs = require('fs');
  const path = require('path');

  const reportsDir = path.join(__dirname, '../reports');
  if (!fs.existsSync(reportsDir)) {
    fs.mkdirSync(reportsDir, { recursive: true });
  }

  fs.writeFileSync(
    path.join(reportsDir, 'integration-results.json'),
    JSON.stringify(results, null, 2)
  );
  console.log('📊 Results saved to: sim/reports/integration-results.json');
}

module.exports = { runIntegrationTests };
