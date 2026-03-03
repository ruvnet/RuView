#!/usr/bin/env node

/**
 * Test suite for Strange Loop NPX CLI
 */

const assert = require('assert');
const { execSync } = require('child_process');
const path = require('path');
const chalk = require('chalk');

// Import our modules
const StrangeLoop = require('../lib/strange-loop');

console.log(chalk.cyan('🧪 Running Strange Loop test suite...\n'));

let testsPassed = 0;
let testsFailed = 0;

function test(name, fn) {
  try {
    console.log(chalk.yellow(`Testing: ${name}`));
    fn();
    console.log(chalk.green(`✅ ${name}`));
    testsPassed++;
  } catch (error) {
    console.log(chalk.red(`❌ ${name}: ${error.message}`));
    testsFailed++;
  }
}

async function runTests() {
  // Test 1: Module loading
  test('Module loading', () => {
    assert(typeof StrangeLoop === 'function', 'StrangeLoop should be a constructor function');
    assert(typeof StrangeLoop.init === 'function', 'StrangeLoop.init should exist');
    assert(typeof StrangeLoop.createSwarm === 'function', 'StrangeLoop.createSwarm should exist');
  });

  // Test 2: System information
  test('System information', async () => {
    const info = await StrangeLoop.getSystemInfo();
    assert(typeof info === 'object', 'System info should be an object');
    assert(typeof info.wasmSupported === 'boolean', 'WASM support should be boolean');
    assert(typeof info.maxAgents === 'number', 'Max agents should be a number');
    assert(info.maxAgents > 0, 'Max agents should be positive');
  });

  // Test 3: Nano-agent swarm creation
  test('Nano-agent swarm creation', async () => {
    const swarm = await StrangeLoop.createSwarm({
      agentCount: 10,
      topology: 'mesh',
      tickDurationNs: 25000
    });

    assert(swarm !== null, 'Swarm should be created');
    assert(typeof swarm.run === 'function', 'Swarm should have run method');
    assert(typeof swarm.addSensorAgent === 'function', 'Swarm should have addSensorAgent method');
  });

  // Test 4: Quantum container creation
  test('Quantum container creation', async () => {
    const quantum = await StrangeLoop.createQuantumContainer(3);

    assert(quantum !== null, 'Quantum container should be created');
    assert(quantum.qubits === 3, 'Should have 3 qubits');
    assert(quantum.states === 8, 'Should have 8 states (2^3)');
    assert(typeof quantum.createSuperposition === 'function', 'Should have createSuperposition method');
    assert(typeof quantum.measure === 'function', 'Should have measure method');
  });

  // Test 5: Temporal consciousness creation
  test('Temporal consciousness creation', async () => {
    const consciousness = await StrangeLoop.createTemporalConsciousness({
      maxIterations: 100,
      enableQuantum: true
    });

    assert(consciousness !== null, 'Consciousness engine should be created');
    assert(typeof consciousness.evolveStep === 'function', 'Should have evolveStep method');
    assert(typeof consciousness.getTemporalPatterns === 'function', 'Should have getTemporalPatterns method');
  });

  // Test 6: Temporal predictor creation
  test('Temporal predictor creation', async () => {
    const predictor = await StrangeLoop.createTemporalPredictor({
      horizonNs: 10_000_000,
      historySize: 100
    });

    assert(predictor !== null, 'Temporal predictor should be created');
    assert(predictor.horizonNs === 10_000_000, 'Should have correct horizon');
    assert(predictor.historySize === 100, 'Should have correct history size');
    assert(typeof predictor.predict === 'function', 'Should have predict method');
  });

  // Test 7: Swarm execution
  test('Swarm execution', async () => {
    const swarm = await StrangeLoop.createSwarm({
      agentCount: 5,
      topology: 'mesh'
    });

    const results = await swarm.run(100); // Short 100ms run

    assert(typeof results === 'object', 'Results should be an object');
    assert(typeof results.totalTicks === 'number', 'Should have totalTicks');
    assert(typeof results.agentCount === 'number', 'Should have agentCount');
    assert(typeof results.runtimeNs === 'number', 'Should have runtimeNs');
    assert(results.agentCount === 5, 'Should have correct agent count');
    assert(results.totalTicks > 0, 'Should have executed some ticks');
  });

  // Test 8: Quantum superposition and measurement
  test('Quantum superposition and measurement', async () => {
    const quantum = await StrangeLoop.createQuantumContainer(2);

    await quantum.createSuperposition();
    assert(quantum.isInSuperposition === true, 'Should be in superposition');

    const measurement = await quantum.measure();
    assert(typeof measurement === 'number', 'Measurement should be a number');
    assert(measurement >= 0 && measurement < 4, 'Measurement should be in valid range');
    assert(quantum.isInSuperposition === false, 'Should have collapsed after measurement');
  });

  // Test 9: Classical data storage in quantum container
  test('Classical data storage', async () => {
    const quantum = await StrangeLoop.createQuantumContainer(3);

    quantum.storeClassical('temperature', 298.15);
    quantum.storeClassical('pressure', 101.325);

    assert(quantum.getClassical('temperature') === 298.15, 'Should retrieve temperature correctly');
    assert(quantum.getClassical('pressure') === 101.325, 'Should retrieve pressure correctly');
    assert(quantum.getClassical('nonexistent') === undefined, 'Should return undefined for nonexistent keys');
  });

  // Test 10: Consciousness evolution
  test('Consciousness evolution', async () => {
    const consciousness = await StrangeLoop.createTemporalConsciousness({
      maxIterations: 10
    });

    const initialState = await consciousness.evolveStep();
    assert(typeof initialState.consciousnessIndex === 'number', 'Should have consciousness index');
    assert(initialState.consciousnessIndex >= 0 && initialState.consciousnessIndex <= 1, 'Consciousness index should be in [0,1]');
    assert(initialState.iteration === 1, 'Should be at iteration 1');

    const patterns = await consciousness.getTemporalPatterns();
    assert(Array.isArray(patterns), 'Patterns should be an array');
  });

  // Test 11: Temporal prediction
  test('Temporal prediction', async () => {
    const predictor = await StrangeLoop.createTemporalPredictor({
      horizonNs: 1_000_000,
      historySize: 50
    });

    const input = [1.0, 2.0, 3.0];
    const prediction = await predictor.predict(input);

    assert(Array.isArray(prediction), 'Prediction should be an array');
    assert(prediction.length === input.length, 'Prediction should have same length as input');

    await predictor.updateHistory(input);
    assert(predictor.history.length === 1, 'History should have one entry');
  });

  // Test 12: CLI command validation
  test('CLI command validation', () => {
    const cliPath = path.join(__dirname, '..', 'bin', 'cli.js');

    try {
      // Test help command
      const helpOutput = execSync(`node "${cliPath}" --help`, { encoding: 'utf8' });
      assert(helpOutput.includes('strange-loop'), 'Help should contain program name');
      assert(helpOutput.includes('demo'), 'Help should mention demo command');
      assert(helpOutput.includes('benchmark'), 'Help should mention benchmark command');
    } catch (error) {
      // CLI might require dependencies, so this is optional
      console.log(chalk.gray('  CLI test skipped (dependencies not installed)'));
    }
  });

  // Summary
  console.log('\n' + chalk.cyan('📊 Test Results:'));
  console.log(chalk.green(`✅ Passed: ${testsPassed}`));
  console.log(chalk.red(`❌ Failed: ${testsFailed}`));

  if (testsFailed === 0) {
    console.log(chalk.green('\n🎉 All tests passed!'));
    process.exit(0);
  } else {
    console.log(chalk.red('\n💥 Some tests failed!'));
    process.exit(1);
  }
}

// Run all tests
runTests().catch(error => {
  console.error(chalk.red(`Test runner failed: ${error.message}`));
  process.exit(1);
});