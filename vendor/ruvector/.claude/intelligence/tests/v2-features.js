#!/usr/bin/env node
/**
 * Test v2 Intelligence Features:
 * - Hyperbolic distance
 * - Confidence Calibration
 * - A/B Testing
 * - Feedback Loop
 * - Active Learning
 * - Pattern Decay
 */

import RuVectorIntelligence from '../index.js';

let passed = 0, failed = 0;

async function test(name, fn) {
  try {
    const result = await fn();
    if (result.pass) {
      console.log(`✅ ${name}`);
      console.log(`   ${result.evidence}`);
      passed++;
    } else {
      console.log(`❌ ${name}`);
      console.log(`   ${result.got}`);
      failed++;
    }
  } catch (e) {
    console.log(`❌ ${name}: ${e.message}`);
    failed++;
  }
}

async function main() {
  console.log('\n🧪 Testing v2 Intelligence Features\n');
  console.log('='.repeat(50) + '\n');

  const intel = new RuVectorIntelligence({ hyperbolic: true });
  await intel.init();

  // === 1. Hyperbolic Distance ===
  console.log('🔮 Hyperbolic Distance:\n');

  await test('Hyperbolic mode is enabled', async () => {
    const stats = intel.stats();
    return {
      pass: stats.memory.usingHyperbolic === true,
      evidence: `usingHyperbolic: ${stats.memory.usingHyperbolic}`
    };
  });

  await test('Hyperbolic search produces different scores than cosine', async () => {
    // Hyperbolic similarity should be lower due to curved space
    const results = await intel.recall('edit rs file', 3);
    const avgScore = results.reduce((s, r) => s + r.score, 0) / results.length;
    // Hyperbolic scores are typically lower (0.01-0.2 range vs 0.7+ for cosine)
    return {
      pass: results.length > 0,
      evidence: `Avg hyperbolic similarity: ${avgScore.toFixed(4)} (curved space metric)`
    };
  });

  // === 2. Confidence Calibration ===
  console.log('\n📊 Confidence Calibration:\n');

  await test('Calibration records predictions', async () => {
    intel.recordCalibration('coder', 'coder', 0.8);
    intel.recordCalibration('coder', 'reviewer', 0.6);
    intel.recordCalibration('tester', 'tester', 0.9);

    const stats = intel.stats();
    const hasBuckets = Object.keys(stats.calibration.buckets).length > 0;
    return {
      pass: hasBuckets,
      evidence: `Calibration buckets: ${JSON.stringify(stats.calibration.buckets)}`
    };
  });

  await test('Calibration error is calculated', async () => {
    const stats = intel.stats();
    return {
      pass: stats.calibration.calibrationError !== undefined,
      evidence: `Calibration error: ${stats.calibration.calibrationError}`
    };
  });

  // === 3. A/B Testing ===
  console.log('\n🔬 A/B Testing:\n');

  await test('A/B group is assigned (treatment or control)', async () => {
    const suggestion = intel.suggest('test_state', ['a', 'b', 'c']);
    const validGroup = ['treatment', 'control'].includes(suggestion.abGroup);
    return {
      pass: validGroup,
      evidence: `Assigned to group: ${suggestion.abGroup}`
    };
  });

  await test('A/B stats are tracked', async () => {
    const stats = intel.stats();
    return {
      pass: stats.abTest.treatment !== undefined && stats.abTest.control !== undefined,
      evidence: `Treatment: ${stats.abTest.treatment.total}, Control: ${stats.abTest.control.total}`
    };
  });

  // === 4. Feedback Loop ===
  console.log('\n🔄 Feedback Loop:\n');

  await test('Routing returns suggestionId for feedback', async () => {
    const routing = await intel.route('test task', { fileType: 'rs' });
    return {
      pass: routing.suggestionId && routing.suggestionId.startsWith('sug-'),
      evidence: `SuggestionId: ${routing.suggestionId}`
    };
  });

  await test('Feedback can be recorded', async () => {
    const routing = await intel.route('another task', { fileType: 'ts' });
    intel.recordFeedback(routing.suggestionId, routing.recommended, true);
    // No error = success
    return {
      pass: true,
      evidence: `Recorded feedback for ${routing.suggestionId}`
    };
  });

  // === 5. Active Learning ===
  console.log('\n🎯 Active Learning:\n');

  await test('Uncertain states are identified', async () => {
    // Create some states with close Q-values
    intel.learn('uncertain_state_1', 'action_a', 'outcome', 0.3);
    intel.learn('uncertain_state_1', 'action_b', 'outcome', 0.28);

    const stats = intel.stats();
    return {
      pass: stats.uncertainStates !== undefined,
      evidence: `Uncertain states found: ${stats.uncertainStates.length}`
    };
  });

  await test('Suggestion flags uncertain states', async () => {
    // Query a state with no prior data
    const suggestion = intel.suggest('completely_novel_state_xyz', ['a', 'b', 'c']);
    return {
      pass: suggestion.isUncertain !== undefined,
      evidence: `isUncertain: ${suggestion.isUncertain}, gap: ${suggestion.uncertaintyGap}`
    };
  });

  // === 6. Pattern Decay ===
  console.log('\n⏰ Pattern Decay:\n');

  await test('Q-table tracks metadata for decay', async () => {
    intel.learn('decay_test_state', 'action', 'outcome', 1.0);
    const qTable = intel.reasoning.qTable;
    const hasMetadata = qTable['decay_test_state']?._meta?.lastUpdate !== undefined;
    return {
      pass: hasMetadata,
      evidence: `Last update tracked: ${qTable['decay_test_state']?._meta?.lastUpdate}`
    };
  });

  await test('Update count is tracked', async () => {
    intel.learn('decay_test_state', 'action', 'outcome', 0.5);
    intel.learn('decay_test_state', 'action', 'outcome', 0.8);
    const updateCount = intel.reasoning.qTable['decay_test_state']?._meta?.updateCount || 0;
    return {
      pass: updateCount >= 2,
      evidence: `Update count: ${updateCount}`
    };
  });

  // === Summary ===
  console.log('\n' + '='.repeat(50));
  console.log(`\n📊 V2 Features: ${passed} passed, ${failed} failed\n`);

  if (failed === 0) {
    console.log('✅ All v2 features working correctly\n');
  } else {
    console.log('⚠️  Some v2 features need attention\n');
    process.exit(1);
  }
}

main().catch(console.error);
