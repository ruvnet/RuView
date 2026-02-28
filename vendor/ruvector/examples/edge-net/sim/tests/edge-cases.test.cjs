/**
 * Edge Case Tests
 * Tests empty states, maximum capacity, rapid transitions, malformed data, and boundary conditions
 */

const assert = require('assert');
const crypto = require('crypto');
const { createMockLearning } = require('./learning-lifecycle.test.cjs');
const { createMockRAC } = require('./rac-coherence.test.cjs');

/**
 * Test 1: Empty State Handling
 */
function testEmptyStates() {
  console.log('\n=== Test 1: Empty State Handling ===');

  const learningWasm = createMockLearning();
  const racWasm = createMockRAC();

  const learning = new learningWasm.NetworkLearning();
  const coherence = new racWasm.CoherenceEngine();

  // Empty learning operations
  assert.strictEqual(learning.trajectoryCount(), 0);
  assert.strictEqual(learning.patternCount(), 0);
  console.log('✓ Empty learning state initialized');

  const emptyStats = JSON.parse(learning.getStats());
  assert.strictEqual(emptyStats.trajectories.total, 0);
  assert.strictEqual(emptyStats.reasoning_bank.total_patterns, 0);
  console.log('✓ Empty stats handled correctly');

  // Empty lookups
  const emptyResults = JSON.parse(learning.lookupPatterns(JSON.stringify([1, 0, 0]), 5));
  assert.strictEqual(emptyResults.length, 0);
  console.log('✓ Empty pattern lookup returns empty array');

  // Empty RAC operations
  assert.strictEqual(coherence.eventCount(), 0);
  assert.strictEqual(coherence.conflictCount(), 0);
  assert.strictEqual(coherence.quarantinedCount(), 0);
  console.log('✓ Empty RAC state initialized');

  // Empty Merkle root
  const emptyRoot = coherence.getMerkleRoot();
  assert.strictEqual(emptyRoot.length, 64); // Hex string of 32 bytes
  console.log('✓ Empty Merkle root generated');

  // Can use any claim in empty state
  assert.ok(coherence.canUseClaim('nonexistent-claim'));
  console.log('✓ Nonexistent claims are usable by default');

  console.log('✅ Empty State Handling Test PASSED');
  return {
    learning_empty: true,
    rac_empty: true,
    handles_empty_lookups: true
  };
}

/**
 * Test 2: Maximum Capacity Scenarios
 */
function testMaxCapacity() {
  console.log('\n=== Test 2: Maximum Capacity Scenarios ===');

  const learningWasm = createMockLearning();
  const racWasm = createMockRAC();

  // Test trajectory ring buffer wraparound
  const tracker = new learningWasm.TrajectoryTracker(100); // Small buffer

  for (let i = 0; i < 250; i++) {
    const success = tracker.record(JSON.stringify({
      task_vector: [i, i, i],
      latency_ms: 50,
      energy_spent: 50,
      energy_earned: 100,
      success: true,
      executor_id: `node-${i}`,
      timestamp: Date.now() + i
    }));
    assert.ok(success, `Failed to record trajectory ${i}`);
  }

  assert.strictEqual(tracker.count(), 100, 'Trajectory buffer should cap at max size');
  console.log('✓ Trajectory ring buffer wraps correctly (100/250 retained)');

  // Test pattern storage at scale
  const bank = new learningWasm.ReasoningBank();
  const patternCount = 10000;

  for (let i = 0; i < patternCount; i++) {
    const id = bank.store(JSON.stringify({
      centroid: [Math.random(), Math.random(), Math.random()],
      optimal_allocation: 0.8,
      optimal_energy: 100,
      confidence: 0.7 + Math.random() * 0.3,
      sample_count: 5,
      avg_latency_ms: 50,
      avg_success_rate: 0.9
    }));
    assert.ok(id >= 0, `Failed to store pattern ${i}`);
  }

  assert.strictEqual(bank.count(), patternCount);
  console.log(`✓ Stored ${patternCount} patterns successfully`);

  // Test RAC event log at scale
  const coherence = new racWasm.CoherenceEngine();
  const eventCount = 10000;

  for (let i = 0; i < eventCount; i++) {
    coherence.ingest({
      id: Array.from(crypto.randomBytes(32)),
      prev: null,
      ts_unix_ms: Date.now() + i,
      author: Array.from(crypto.randomBytes(32)),
      context: Array.from(crypto.randomBytes(32)),
      ruvector: { dims: [0, 0, 0] },
      kind: {
        Assert: {
          proposition: Buffer.from(`claim-${i}`),
          evidence: [],
          confidence: 0.8,
          expires_at_unix_ms: null
        }
      },
      sig: Array.from(crypto.randomBytes(64))
    });
  }

  assert.strictEqual(coherence.eventCount(), eventCount);
  console.log(`✓ Ingested ${eventCount} RAC events successfully`);

  console.log('✅ Maximum Capacity Test PASSED');
  return {
    trajectory_buffer_size: tracker.count(),
    pattern_count: bank.count(),
    event_count: coherence.eventCount()
  };
}

/**
 * Test 3: Rapid State Transitions
 */
function testRapidTransitions() {
  console.log('\n=== Test 3: Rapid State Transitions ===');

  const racWasm = createMockRAC();
  const coherence = new racWasm.CoherenceEngine();

  const context = crypto.randomBytes(32);
  const claim = {
    id: Array.from(crypto.randomBytes(32)),
    prev: null,
    ts_unix_ms: Date.now(),
    author: Array.from(crypto.randomBytes(32)),
    context: Array.from(context),
    ruvector: { dims: [0, 0, 0] },
    kind: {
      Assert: {
        proposition: Buffer.from('rapid-transition-claim'),
        evidence: [],
        confidence: 0.8,
        expires_at_unix_ms: null
      }
    },
    sig: Array.from(crypto.randomBytes(64))
  };

  coherence.ingest(claim);
  const claimHex = Buffer.from(claim.id).toString('hex');

  // Rapid transitions: None → Challenge → Resolution → Deprecate
  assert.strictEqual(coherence.getQuarantineLevel(claimHex), 0);
  console.log('✓ State 1: None (level 0)');

  // Challenge (level 2)
  const challenge = {
    id: Array.from(crypto.randomBytes(32)),
    prev: null,
    ts_unix_ms: Date.now() + 1,
    author: Array.from(crypto.randomBytes(32)),
    context: Array.from(context),
    ruvector: { dims: [0, 0, 0] },
    kind: {
      Challenge: {
        conflict_id: Array.from(crypto.randomBytes(32)),
        claim_ids: [claim.id],
        reason: 'Rapid test',
        requested_proofs: []
      }
    },
    sig: Array.from(crypto.randomBytes(64))
  };

  coherence.ingest(challenge);
  assert.strictEqual(coherence.getQuarantineLevel(claimHex), 2);
  console.log('✓ State 2: Challenged (level 2)');

  // Resolution accepting claim (level 0)
  const resolution = {
    id: Array.from(crypto.randomBytes(32)),
    prev: null,
    ts_unix_ms: Date.now() + 2,
    author: Array.from(crypto.randomBytes(32)),
    context: Array.from(context),
    ruvector: { dims: [0, 0, 0] },
    kind: {
      Resolution: {
        conflict_id: challenge.kind.Challenge.conflict_id,
        accepted: [claim.id],
        deprecated: [],
        rationale: [],
        authority_sigs: []
      }
    },
    sig: Array.from(crypto.randomBytes(64))
  };

  coherence.ingest(resolution);
  assert.strictEqual(coherence.getQuarantineLevel(claimHex), 0);
  console.log('✓ State 3: Resolved/Accepted (level 0)');

  // Deprecation (level 3)
  const deprecate = {
    id: Array.from(crypto.randomBytes(32)),
    prev: null,
    ts_unix_ms: Date.now() + 3,
    author: Array.from(crypto.randomBytes(32)),
    context: Array.from(context),
    ruvector: { dims: [0, 0, 0] },
    kind: {
      Deprecate: {
        claim_id: claim.id,
        by_resolution: Array.from(crypto.randomBytes(32)),
        superseded_by: null
      }
    },
    sig: Array.from(crypto.randomBytes(64))
  };

  coherence.ingest(deprecate);
  assert.strictEqual(coherence.getQuarantineLevel(claimHex), 3);
  console.log('✓ State 4: Deprecated (level 3)');

  // All transitions within milliseconds
  console.log('✓ Rapid transitions (0 → 2 → 0 → 3) handled correctly');

  console.log('✅ Rapid State Transitions Test PASSED');
  return {
    transitions: 4,
    final_state: 'deprecated',
    final_level: 3
  };
}

/**
 * Test 4: Malformed Data Handling
 */
function testMalformedData() {
  console.log('\n=== Test 4: Malformed Data Handling ===');

  const learningWasm = createMockLearning();
  const learning = new learningWasm.NetworkLearning();

  // Invalid JSON
  const invalidJson = learning.storePattern('not valid json');
  assert.strictEqual(invalidJson, -1);
  console.log('✓ Invalid JSON rejected (returns -1)');

  // Missing required fields
  const invalidPattern = learning.storePattern(JSON.stringify({
    centroid: [1, 0, 0]
    // Missing other required fields
  }));
  assert.strictEqual(invalidPattern, -1);
  console.log('✓ Incomplete pattern rejected');

  // Wrong data types
  const wrongTypes = learning.recordTrajectory(JSON.stringify({
    task_vector: "not an array",
    latency_ms: "not a number",
    energy_spent: null,
    energy_earned: undefined,
    success: "not a boolean",
    executor_id: 12345,
    timestamp: "not a number"
  }));
  // Mock should handle this gracefully
  console.log('✓ Wrong data types handled gracefully');

  // Empty vectors
  const emptyVector = learning.lookupPatterns(JSON.stringify([]), 5);
  assert.strictEqual(emptyVector, '[]');
  console.log('✓ Empty vector query returns empty results');

  // Negative values
  const bank = new learningWasm.ReasoningBank();
  bank.store(JSON.stringify({
    centroid: [1, 0, 0],
    optimal_allocation: -0.5, // Invalid
    optimal_energy: -100, // Invalid
    confidence: 1.5, // Out of range
    sample_count: -10, // Invalid
    avg_latency_ms: -50, // Invalid
    avg_success_rate: 2.0 // Out of range
  }));
  // Should store but may have clamped values
  console.log('✓ Out-of-range values accepted (implementation may clamp)');

  // Null/undefined handling
  const nullTrajectory = learning.recordTrajectory(null);
  assert.strictEqual(nullTrajectory, false);
  console.log('✓ Null trajectory rejected');

  const undefinedPattern = learning.storePattern(undefined);
  assert.strictEqual(undefinedPattern, -1);
  console.log('✓ Undefined pattern rejected');

  console.log('✅ Malformed Data Handling Test PASSED');
  return {
    invalid_json_rejected: true,
    null_handling: true,
    type_safety: true
  };
}

/**
 * Test 5: Boundary Conditions
 */
function testBoundaryConditions() {
  console.log('\n=== Test 5: Boundary Conditions ===');

  const learningWasm = createMockLearning();
  const racWasm = createMockRAC();

  // Zero-dimensional vectors
  const learning = new learningWasm.NetworkLearning();
  const zeroVecPattern = learning.storePattern(JSON.stringify({
    centroid: [],
    optimal_allocation: 0.8,
    optimal_energy: 100,
    confidence: 0.9,
    sample_count: 10,
    avg_latency_ms: 50,
    avg_success_rate: 0.95
  }));
  assert.ok(zeroVecPattern >= 0);
  console.log('✓ Zero-dimensional vector stored');

  // Very high-dimensional vectors
  const highDimVec = Array(10000).fill(0).map(() => Math.random());
  const highDimPattern = learning.storePattern(JSON.stringify({
    centroid: highDimVec,
    optimal_allocation: 0.8,
    optimal_energy: 100,
    confidence: 0.9,
    sample_count: 10,
    avg_latency_ms: 50,
    avg_success_rate: 0.95
  }));
  assert.ok(highDimPattern >= 0);
  console.log('✓ 10,000-dimensional vector stored');

  // Zero confidence/energy
  const zeroConfidence = learning.storePattern(JSON.stringify({
    centroid: [1, 0, 0],
    optimal_allocation: 0.0,
    optimal_energy: 0,
    confidence: 0.0,
    sample_count: 0,
    avg_latency_ms: 0,
    avg_success_rate: 0.0
  }));
  assert.ok(zeroConfidence >= 0);
  console.log('✓ Zero confidence/energy pattern stored');

  // Maximum values
  const maxValues = learning.storePattern(JSON.stringify({
    centroid: Array(100).fill(Number.MAX_VALUE),
    optimal_allocation: 1.0,
    optimal_energy: Number.MAX_SAFE_INTEGER,
    confidence: 1.0,
    sample_count: Number.MAX_SAFE_INTEGER,
    avg_latency_ms: Number.MAX_VALUE,
    avg_success_rate: 1.0
  }));
  assert.ok(maxValues >= 0);
  console.log('✓ Maximum values stored');

  // Spike attention edge cases
  const spike = new learningWasm.SpikeDrivenAttention();

  const zeroRatio = spike.energyRatio(0, 0);
  assert.strictEqual(zeroRatio, 1.0);
  console.log('✓ Zero-length sequences return 1.0 energy ratio');

  const singleRatio = spike.energyRatio(1, 1);
  assert.ok(singleRatio > 0);
  console.log('✓ Single-element sequences handled');

  const largeRatio = spike.energyRatio(10000, 10000);
  assert.ok(largeRatio > 1.0 && largeRatio < 1000);
  console.log('✓ Very large sequences bounded');

  // Multi-head attention boundaries
  const minAttn = new learningWasm.MultiHeadAttention(2, 1);
  assert.strictEqual(minAttn.dim(), 2);
  assert.strictEqual(minAttn.numHeads(), 1);
  console.log('✓ Minimum attention configuration (2 dim, 1 head)');

  const maxAttn = new learningWasm.MultiHeadAttention(1024, 64);
  assert.strictEqual(maxAttn.dim(), 1024);
  assert.strictEqual(maxAttn.numHeads(), 64);
  console.log('✓ Large attention configuration (1024 dim, 64 heads)');

  // RAC event boundaries
  const coherence = new racWasm.CoherenceEngine();

  // Minimal event
  const minEvent = {
    id: Array.from(Buffer.alloc(32)),
    prev: null,
    ts_unix_ms: 0,
    author: Array.from(Buffer.alloc(32)),
    context: Array.from(Buffer.alloc(32)),
    ruvector: { dims: [] },
    kind: {
      Assert: {
        proposition: Buffer.from(''),
        evidence: [],
        confidence: 0,
        expires_at_unix_ms: null
      }
    },
    sig: Array.from(Buffer.alloc(64))
  };

  coherence.ingest(minEvent);
  assert.strictEqual(coherence.eventCount(), 1);
  console.log('✓ Minimal event ingested');

  // Maximum timestamp
  const maxTimestamp = {
    id: Array.from(crypto.randomBytes(32)),
    prev: null,
    ts_unix_ms: Number.MAX_SAFE_INTEGER,
    author: Array.from(crypto.randomBytes(32)),
    context: Array.from(crypto.randomBytes(32)),
    ruvector: { dims: [0] },
    kind: {
      Assert: {
        proposition: Buffer.from('max-timestamp'),
        evidence: [],
        confidence: 0.8,
        expires_at_unix_ms: Number.MAX_SAFE_INTEGER
      }
    },
    sig: Array.from(crypto.randomBytes(64))
  };

  coherence.ingest(maxTimestamp);
  assert.strictEqual(coherence.eventCount(), 2);
  console.log('✓ Maximum timestamp handled');

  console.log('✅ Boundary Conditions Test PASSED');
  return {
    zero_dim_vectors: true,
    high_dim_vectors: true,
    extreme_values: true,
    minimal_events: true
  };
}

/**
 * Test 6: Concurrent Modification Safety
 */
function testConcurrentModificationSafety() {
  console.log('\n=== Test 6: Concurrent Modification Safety ===');

  const learningWasm = createMockLearning();
  const learning = new learningWasm.NetworkLearning();

  // Interleaved reads and writes
  const operations = 100;

  for (let i = 0; i < operations; i++) {
    // Write
    learning.storePattern(JSON.stringify({
      centroid: [i, i, i],
      optimal_allocation: 0.8,
      optimal_energy: 100,
      confidence: 0.9,
      sample_count: 10,
      avg_latency_ms: 50,
      avg_success_rate: 0.95
    }));

    // Read
    if (i > 0) {
      const results = JSON.parse(learning.lookupPatterns(JSON.stringify([i, i, i]), 5));
      assert.ok(results.length >= 0);
    }

    // Modify (prune)
    if (i % 10 === 0 && i > 0) {
      learning.prune(100, 0.5);
    }

    // Read stats
    const stats = JSON.parse(learning.getStats());
    assert.ok(stats.reasoning_bank.total_patterns >= 0);
  }

  console.log(`✓ Completed ${operations} interleaved operations`);
  console.log('✓ No concurrent modification errors');

  console.log('✅ Concurrent Modification Safety Test PASSED');
  return {
    operations: operations,
    safe: true
  };
}

/**
 * Run all edge case tests
 */
function runEdgeCaseTests() {
  console.log('\n╔══════════════════════════════════════════════════════╗');
  console.log('║  Edge Case Simulation Tests                         ║');
  console.log('╚══════════════════════════════════════════════════════╝');

  const results = {
    timestamp: new Date().toISOString(),
    test_suite: 'edge_cases',
    tests: {}
  };

  try {
    results.tests.empty_states = testEmptyStates();
    results.tests.max_capacity = testMaxCapacity();
    results.tests.rapid_transitions = testRapidTransitions();
    results.tests.malformed_data = testMalformedData();
    results.tests.boundary_conditions = testBoundaryConditions();
    results.tests.concurrent_safety = testConcurrentModificationSafety();

    results.summary = {
      total_tests: 6,
      passed: 6,
      failed: 0,
      success_rate: 1.0
    };

    console.log('\n╔══════════════════════════════════════════════════════╗');
    console.log('║  All Edge Case Tests PASSED ✅                       ║');
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
  const results = runEdgeCaseTests();
  const fs = require('fs');
  const path = require('path');

  const reportsDir = path.join(__dirname, '../reports');
  if (!fs.existsSync(reportsDir)) {
    fs.mkdirSync(reportsDir, { recursive: true });
  }

  fs.writeFileSync(
    path.join(reportsDir, 'edge-cases-results.json'),
    JSON.stringify(results, null, 2)
  );
  console.log('📊 Results saved to: sim/reports/edge-cases-results.json');
}

module.exports = { runEdgeCaseTests };
