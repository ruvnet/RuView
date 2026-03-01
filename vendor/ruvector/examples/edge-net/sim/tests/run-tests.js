#!/usr/bin/env node

/**
 * Test Suite for Edge-Net Simulation
 * Validates simulation logic and phase transitions
 */

import { NetworkSimulation } from '../src/network.js';
import { SimNode } from '../src/node.js';
import { EconomicTracker } from '../src/economics.js';
import { PhaseManager } from '../src/phases.js';

console.log('🧪 Running Edge-Net Simulation Tests\n');

let testsRun = 0;
let testsPassed = 0;
let testsFailed = 0;

async function test(name, fn) {
  testsRun++;
  try {
    await fn();
    testsPassed++;
    console.log(`✅ ${name}`);
  } catch (error) {
    testsFailed++;
    console.error(`❌ ${name}`);
    console.error(`   ${error.message}`);
  }
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message || 'Assertion failed');
  }
}

function assertEquals(actual, expected, message) {
  if (actual !== expected) {
    throw new Error(message || `Expected ${expected}, got ${actual}`);
  }
}

function assertApprox(actual, expected, tolerance, message) {
  if (Math.abs(actual - expected) > tolerance) {
    throw new Error(message || `Expected ~${expected}, got ${actual}`);
  }
}

// ============================================================================
// Node Tests
// ============================================================================

await test('Node: Create genesis node', () => {
  const node = new SimNode('test-1', Date.now(), true);
  assert(node.isGenesis, 'Should be genesis node');
  assertEquals(node.ruvEarned, 0, 'Should start with 0 rUv');
  assert(node.active, 'Should be active');
});

await test('Node: Create regular node', () => {
  const node = new SimNode('test-2', Date.now(), false);
  assert(!node.isGenesis, 'Should not be genesis node');
  assert(node.maxConnections === 50, 'Should have normal connection limit');
});

await test('Node: Genesis multiplier calculation', () => {
  const genesisNode = new SimNode('genesis-1', Date.now(), true);
  const multiplier = genesisNode.calculateMultiplier(0, 'genesis');
  assert(multiplier === 10.0, 'Genesis phase should have 10x multiplier');
});

await test('Node: Transition phase multiplier decay', () => {
  const genesisNode = new SimNode('genesis-1', Date.now(), true);
  const mult1 = genesisNode.calculateMultiplier(0, 'transition');
  const mult2 = genesisNode.calculateMultiplier(500000, 'transition');
  assert(mult1 > mult2, 'Multiplier should decay over time');
  assert(mult2 >= 1.0, 'Multiplier should not go below 1x');
});

await test('Node: Connection management', () => {
  const node = new SimNode('test-1', Date.now(), false);
  assert(node.connectTo('peer-1'), 'Should connect successfully');
  assert(node.connections.has('peer-1'), 'Should track connection');
  node.disconnect('peer-1');
  assert(!node.connections.has('peer-1'), 'Should remove connection');
});

await test('Node: Balance calculation', () => {
  const node = new SimNode('test-1', Date.now(), false);
  node.ruvEarned = 100;
  node.ruvSpent = 30;
  node.ruvStaked = 20;
  assertEquals(node.getBalance(), 50, 'Balance should be earned - spent - staked');
});

// ============================================================================
// Economic Tests
// ============================================================================

await test('Economic: Initialize tracker', () => {
  const econ = new EconomicTracker();
  assertEquals(econ.totalSupply, 0, 'Should start with 0 supply');
  assertEquals(econ.treasury, 0, 'Should start with empty treasury');
});

await test('Economic: Distribution ratios sum to 1.0', () => {
  const econ = new EconomicTracker();
  const sum = econ.distribution.contributors +
              econ.distribution.treasury +
              econ.distribution.protocol +
              econ.distribution.founders;
  assertApprox(sum, 1.0, 0.001, 'Distribution ratios should sum to 1.0');
});

await test('Economic: Stability calculation', () => {
  const econ = new EconomicTracker();
  econ.treasury = 100;
  econ.contributorPool = 100;
  econ.protocolFund = 100;

  const stability = econ.calculateStability();
  assert(stability > 0.9, 'Balanced pools should have high stability');
});

await test('Economic: Self-sustainability check', () => {
  const econ = new EconomicTracker();
  econ.treasury = 100000;
  econ.growthRate = 0.01;

  const sustainable = econ.isSelfSustaining(150, 2000);
  assert(sustainable, 'Should be self-sustaining with sufficient resources');
});

// ============================================================================
// Phase Tests
// ============================================================================

await test('Phase: Initialize with genesis phase', () => {
  const phases = new PhaseManager();
  assertEquals(phases.currentPhase, 'genesis', 'Should start in genesis phase');
});

await test('Phase: Transition tracking', () => {
  const phases = new PhaseManager();
  phases.transition('transition');
  assertEquals(phases.currentPhase, 'transition', 'Should transition to new phase');
  assertEquals(phases.phaseHistory.length, 1, 'Should record transition');
});

await test('Phase: Expected phase for node count', () => {
  const phases = new PhaseManager();

  assertEquals(phases.getExpectedPhase(5000), 'genesis', '5K nodes = genesis');
  assertEquals(phases.getExpectedPhase(25000), 'transition', '25K nodes = transition');
  assertEquals(phases.getExpectedPhase(75000), 'maturity', '75K nodes = maturity');
  assertEquals(phases.getExpectedPhase(150000), 'post-genesis', '150K nodes = post-genesis');
});

// ============================================================================
// Network Tests
// ============================================================================

await test('Network: Initialize with genesis nodes', async () => {
  const sim = new NetworkSimulation({ genesisNodes: 5 });
  await sim.initialize();

  assertEquals(sim.nodes.size, 5, 'Should have 5 genesis nodes');
  assertEquals(sim.getCurrentPhase(), 'genesis', 'Should be in genesis phase');
});

await test('Network: Add regular node', async () => {
  const sim = new NetworkSimulation({ genesisNodes: 3 });
  await sim.initialize();

  const initialCount = sim.nodes.size;
  sim.addNode();

  assertEquals(sim.nodes.size, initialCount + 1, 'Should add one node');
});

await test('Network: Phase transition detection', async () => {
  const sim = new NetworkSimulation({ genesisNodes: 5 });
  await sim.initialize();

  // Manually set node count for transition
  for (let i = 0; i < 10000; i++) {
    sim.nodes.set(`node-${i}`, new SimNode(`node-${i}`, Date.now(), false));
  }

  sim.checkPhaseTransition();
  assertEquals(sim.getCurrentPhase(), 'transition', 'Should transition to transition phase');
});

await test('Network: Metrics update', async () => {
  const sim = new NetworkSimulation({ genesisNodes: 3 });
  await sim.initialize();

  sim.updateMetrics();

  assert(sim.metrics.activeNodeCount > 0, 'Should count active nodes');
  assert(sim.metrics.genesisNodeCount === 3, 'Should count genesis nodes');
});

await test('Network: Health calculation', async () => {
  const sim = new NetworkSimulation({ genesisNodes: 5 });
  await sim.initialize();

  const nodes = sim.getActiveNodes();
  const health = sim.calculateNetworkHealth(nodes);

  assert(health >= 0 && health <= 1, 'Health should be between 0 and 1');
});

// ============================================================================
// Integration Tests
// ============================================================================

await test('Integration: Small simulation run', async () => {
  const sim = new NetworkSimulation({
    genesisNodes: 3,
    targetNodes: 100,
    tickInterval: 100,
    accelerationFactor: 10000,
  });

  await sim.initialize();

  // Run a few ticks
  for (let i = 0; i < 10; i++) {
    await sim.tick();
  }

  assert(sim.currentTick === 10, 'Should complete 10 ticks');
  assert(sim.totalComputeHours >= 0, 'Should accumulate compute hours');
});

await test('Integration: Genesis to transition simulation', async () => {
  const sim = new NetworkSimulation({
    genesisNodes: 5,
    targetNodes: 10500, // Just past transition threshold
    tickInterval: 100,
    accelerationFactor: 100000,
  });

  await sim.initialize();
  await sim.run('transition');

  assertEquals(sim.getCurrentPhase(), 'transition', 'Should reach transition phase');
  assert(sim.nodes.size >= 10000, 'Should have at least 10K nodes');
  assert(sim.phaseTransitions.length >= 1, 'Should record phase transition');
});

// ============================================================================
// Results
// ============================================================================

console.log('\n' + '='.repeat(60));
console.log('TEST RESULTS');
console.log('='.repeat(60));
console.log(`Total:  ${testsRun}`);
console.log(`Passed: ${testsPassed} ✅`);
console.log(`Failed: ${testsFailed} ${testsFailed > 0 ? '❌' : ''}`);
console.log('='.repeat(60));

process.exit(testsFailed > 0 ? 1 : 0);
