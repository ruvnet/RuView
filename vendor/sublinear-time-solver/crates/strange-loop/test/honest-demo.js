#!/usr/bin/env node

// HONEST Demo - Shows what actually works

const wasmHonest = require('../wasm-honest/strange_loop.js');
const chalk = require('chalk');

wasmHonest.init_wasm();

console.log(chalk.cyan.bold('\n════════════════════════════════════════════════════════════════'));
console.log(chalk.cyan.bold('           HONEST WASM Demo - No Bullshit Edition               '));
console.log(chalk.cyan.bold('════════════════════════════════════════════════════════════════\n'));

// Test all honest functions
console.log(chalk.green.bold('✅ HONEST FUNCTIONS THAT ACTUALLY WORK:\n'));

// 1. Honest quantum simulation
console.log(chalk.yellow('1. Quantum Simulation (simplified but real):'));
console.log('  ', wasmHonest.quantum_simulate_honest(4));
console.log('  ', wasmHonest.quantum_simulate_honest(8));

// 2. Real random quantum measurement
console.log(chalk.yellow('\n2. Quantum Measurement (real randomness):'));
const measurements = [];
for (let i = 0; i < 10; i++) {
    measurements.push(wasmHonest.quantum_measure_honest(4));
}
console.log('   10 measurements:', measurements);
console.log('   Unique values:', new Set(measurements).size);

// 3. Honest consciousness model
console.log(chalk.yellow('\n3. Consciousness Model (admits it\'s just math):'));
console.log('  ', wasmHonest.consciousness_simulate_honest(50));
console.log('  ', wasmHonest.consciousness_simulate_honest(150));

// 4. Honest swarm simulation
console.log(chalk.yellow('\n4. Swarm Simulation (single-threaded):'));
console.log('  ', wasmHonest.swarm_simulate_honest(10));
console.log('  ', wasmHonest.swarm_simulate_honest(100));

// 5. Honest solver
console.log(chalk.yellow('\n5. Simple Solver (actually computes):'));
console.log('  ', wasmHonest.solve_simple_honest(10));
console.log('  ', wasmHonest.solve_simple_honest(50));

// 6. Real random numbers
console.log(chalk.yellow('\n6. Real Random Numbers:'));
const randoms = [];
for (let i = 0; i < 5; i++) {
    randoms.push(wasmHonest.random_real().toFixed(4));
}
console.log('   5 random values:', randoms.join(', '));

// 7. Honest benchmark
console.log(chalk.yellow('\n7. Honest Benchmark:'));
console.log('  ', wasmHonest.benchmark_honest());

// Test randomness quality
console.log(chalk.cyan.bold('\n════════════════════════════════════════════════════════════════'));
console.log(chalk.cyan.bold('                    RANDOMNESS QUALITY TEST                     '));
console.log(chalk.cyan.bold('════════════════════════════════════════════════════════════════\n'));

const testSamples = 1000;
const quantumSamples = [];
for (let i = 0; i < testSamples; i++) {
    quantumSamples.push(wasmHonest.quantum_measure_honest(4));
}

// Calculate distribution
const distribution = {};
for (let i = 0; i < 16; i++) {
    distribution[i] = 0;
}
quantumSamples.forEach(s => distribution[s]++);

console.log('Distribution of 1000 measurements (4 qubits = 16 states):');
for (let i = 0; i < 16; i++) {
    const count = distribution[i];
    const percent = (count / testSamples * 100).toFixed(1);
    const bar = '█'.repeat(Math.floor(count / 20));
    console.log(`  State ${i.toString().padStart(2)}: ${bar} ${count} (${percent}%)`);
}

// Check if it's uniform (good randomness)
const expected = testSamples / 16;
const chiSquare = Object.values(distribution)
    .reduce((sum, observed) => sum + Math.pow(observed - expected, 2) / expected, 0);

console.log(`\nChi-square statistic: ${chiSquare.toFixed(2)}`);
console.log(`Expected for uniform: ~15.5 (actual: ${chiSquare.toFixed(2)})`);
console.log(chiSquare < 30 ? chalk.green('✅ Good randomness!') : chalk.red('❌ Poor randomness'));

// Summary
console.log(chalk.cyan.bold('\n════════════════════════════════════════════════════════════════'));
console.log(chalk.cyan.bold('                           SUMMARY                              '));
console.log(chalk.cyan.bold('════════════════════════════════════════════════════════════════\n'));

console.log(chalk.green.bold('What This HONESTLY Does:'));
console.log('  ✅ Simplified quantum simulation with real probability calculations');
console.log('  ✅ Cryptographic randomness using getrandom');
console.log('  ✅ Mathematical models (clearly labeled as such)');
console.log('  ✅ Single-threaded simulations (not real parallelism)');
console.log('  ✅ Simple numerical solvers that actually iterate');
console.log('  ✅ Real benchmarks that measure actual computation');

console.log(chalk.yellow.bold('\nWhat It DOESN\'T Claim:'));
console.log('  ❌ NOT real quantum computing');
console.log('  ❌ NOT real consciousness');
console.log('  ❌ NOT real parallel swarms');
console.log('  ❌ NOT nanosecond precision in browser');
console.log('  ❌ NOT solving million-variable systems');

console.log(chalk.cyan.bold('\nThe Bottom Line:'));
console.log('  This is an HONEST implementation that does real (simplified) computation.');
console.log('  It doesn\'t lie about what it\'s doing.');
console.log('  It\'s not bullshit - it\'s just honest about its limitations.\n');

process.exit(0);