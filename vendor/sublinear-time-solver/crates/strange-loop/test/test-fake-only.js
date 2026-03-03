#!/usr/bin/env node

// Test just the fake version to see what it really does

const wasm = require('../wasm/strange_loop.js');
const chalk = require('chalk');

wasm.init_wasm();

console.log(chalk.cyan.bold('\n════════════════════════════════════════════'));
console.log(chalk.cyan.bold('      Testing Current WASM Implementation     '));
console.log(chalk.cyan.bold('════════════════════════════════════════════\n'));

// Test quantum functions
console.log(chalk.yellow('▶ Quantum Superposition:'));
console.log('  ', wasm.quantum_superposition(4));

console.log(chalk.yellow('\n▶ Quantum Measurements (10 samples):'));
const measurements = [];
for (let i = 0; i < 10; i++) {
    measurements.push(wasm.measure_quantum_state(4));
}
console.log('  ', measurements);

// Check if it's truly random
const unique = new Set(measurements).size;
console.log(chalk.gray(`  Unique values: ${unique}/10`));

// Test multiple calls to same function
console.log(chalk.yellow('\n▶ Consciousness Evolution (same input):'));
for (let i = 0; i < 3; i++) {
    console.log(`  100 iterations: ${wasm.evolve_consciousness(100)}`);
}

console.log(chalk.yellow('\n▶ Bell State:'));
console.log('  ', wasm.create_bell_state(0));

console.log(chalk.yellow('\n▶ Sublinear Solver:'));
console.log('  ', wasm.solve_linear_system_sublinear(1000, 0.001));

console.log(chalk.yellow('\n▶ PageRank:'));
console.log('  ', wasm.compute_pagerank(10000, 0.85));

// Performance test
const { performance } = require('perf_hooks');

console.log(chalk.yellow('\n▶ Performance Test:'));
const start = performance.now();
for (let i = 0; i < 10000; i++) {
    wasm.measure_quantum_state(8);
}
const end = performance.now();
const time = end - start;
console.log(`  10,000 measurements: ${time.toFixed(2)}ms`);
console.log(`  ${(10000 / time * 1000).toFixed(0)} ops/sec`);

// Check what functions are actually exported
console.log(chalk.yellow('\n▶ Available Functions:'));
const funcs = Object.keys(wasm).filter(k => typeof wasm[k] === 'function');
console.log('  Total functions:', funcs.length);
console.log('  First 10:', funcs.slice(0, 10).join(', '));

// Look for "real" vs "old" versions
const realFuncs = funcs.filter(f => !f.includes('_old') && !f.includes('__'));
const oldFuncs = funcs.filter(f => f.includes('_old'));
console.log('  Regular functions:', realFuncs.length);
console.log('  Old functions:', oldFuncs.length);

// If there are old versions, test them
if (oldFuncs.length > 0) {
    console.log(chalk.cyan('\n▶ Testing "_old" versions:'));
    if (wasm.quantum_superposition_old) {
        console.log('  quantum_superposition_old:', wasm.quantum_superposition_old(4));
    }
    if (wasm.measure_quantum_state_old) {
        const oldMeasurements = [];
        for (let i = 0; i < 5; i++) {
            oldMeasurements.push(wasm.measure_quantum_state_old(4));
        }
        console.log('  measure_quantum_state_old:', oldMeasurements);
    }
}

process.exit(0);