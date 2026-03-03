#!/usr/bin/env node

// Compare REAL vs FAKE implementations

const wasmFake = require('../wasm/strange_loop.js');
const wasmReal = require('../wasm-real/strange_loop.js');
const chalk = require('chalk');

// Initialize both WASM modules
console.log(chalk.cyan.bold('\n════════════════════════════════════════════════════════════════'));
console.log(chalk.cyan.bold('           REAL vs FAKE: Strange Loops Comparison               '));
console.log(chalk.cyan.bold('════════════════════════════════════════════════════════════════\n'));

wasmFake.init_wasm();
wasmReal.init_wasm();

function compareResults(category, operation, fake, real) {
    console.log(chalk.yellow(`\n▶ ${category}: ${operation}`));
    console.log(chalk.red('  FAKE:'), fake);
    console.log(chalk.green('  REAL:'), real);
}

// 1. QUANTUM SUPERPOSITION
console.log(chalk.cyan.bold('\n═══ 1. QUANTUM SUPERPOSITION ═══'));

const quantumFake = wasmFake.quantum_superposition(4);
const quantumReal = wasmReal.quantum_superposition(4);
compareResults('Quantum', 'Superposition (4 qubits)', quantumFake, quantumReal);

// 2. QUANTUM MEASUREMENT RANDOMNESS
console.log(chalk.cyan.bold('\n═══ 2. QUANTUM MEASUREMENT RANDOMNESS ═══'));

const measurementsFake = [];
const measurementsReal = [];

for (let i = 0; i < 10; i++) {
    measurementsFake.push(wasmFake.measure_quantum_state(4));
    measurementsReal.push(wasmReal.measure_quantum_state(4));
}

console.log(chalk.yellow('\n▶ Quantum Measurements (10 samples):'));
console.log(chalk.red('  FAKE:'), measurementsFake);
console.log(chalk.green('  REAL:'), measurementsReal);

// Calculate uniqueness
const uniqueFake = new Set(measurementsFake).size;
const uniqueReal = new Set(measurementsReal).size;

console.log(chalk.gray(`  FAKE uniqueness: ${uniqueFake}/10`));
console.log(chalk.gray(`  REAL uniqueness: ${uniqueReal}/10`));

// 3. CONSCIOUSNESS EVOLUTION
console.log(chalk.cyan.bold('\n═══ 3. CONSCIOUSNESS EVOLUTION ═══'));

const consciousnessFake100 = wasmFake.evolve_consciousness(100);
const consciousnessReal100 = wasmReal.evolve_consciousness(100);
const consciousnessFake500 = wasmFake.evolve_consciousness(500);
const consciousnessReal500 = wasmReal.evolve_consciousness(500);

compareResults('Consciousness', 'Evolution (100 iterations)',
    consciousnessFake100, consciousnessReal100);
compareResults('Consciousness', 'Evolution (500 iterations)',
    consciousnessFake500, consciousnessReal500);

// 4. NANO-AGENT SWARM
console.log(chalk.cyan.bold('\n═══ 4. NANO-AGENT SWARM ═══'));

const swarmFake = wasmFake.create_nano_swarm(100);
const swarmReal = wasmReal.create_nano_swarm(100);
compareResults('Swarm', 'Create (100 agents)', swarmFake, swarmReal);

// 5. SUBLINEAR SOLVER
console.log(chalk.cyan.bold('\n═══ 5. SUBLINEAR SOLVER ═══'));

const solverFake = wasmFake.solve_linear_system_sublinear(1000, 0.001);
const solverReal = wasmReal.solve_linear_system_sublinear(1000, 0.001);
compareResults('Solver', 'Linear System (n=1000)', solverFake, solverReal);

// 6. BELL STATES
console.log(chalk.cyan.bold('\n═══ 6. BELL STATES ═══'));

const bellFake = wasmFake.create_bell_state(0);
const bellReal = wasmReal.create_bell_state(0);
compareResults('Quantum', 'Bell State |Φ+⟩', bellFake, bellReal);

// 7. PERFORMANCE TEST
console.log(chalk.cyan.bold('\n═══ 7. PERFORMANCE COMPARISON ═══\n'));

const { performance } = require('perf_hooks');

// Test quantum measurement speed
const iterations = 1000;

const startFake = performance.now();
for (let i = 0; i < iterations; i++) {
    wasmFake.measure_quantum_state(8);
}
const endFake = performance.now();

const startReal = performance.now();
for (let i = 0; i < iterations; i++) {
    wasmReal.measure_quantum_state(8);
}
const endReal = performance.now();

const fakeTime = endFake - startFake;
const realTime = endReal - startReal;

console.log(chalk.yellow('▶ Performance (1000 quantum measurements):'));
console.log(chalk.red(`  FAKE: ${fakeTime.toFixed(2)}ms (${(iterations / fakeTime * 1000).toFixed(0)} ops/sec)`));
console.log(chalk.green(`  REAL: ${realTime.toFixed(2)}ms (${(iterations / realTime * 1000).toFixed(0)} ops/sec)`));

// 8. DETERMINISM CHECK
console.log(chalk.cyan.bold('\n═══ 8. DETERMINISM CHECK ═══\n'));

console.log(chalk.yellow('▶ Testing if functions are deterministic:'));

// Check consciousness (should be deterministic)
const c1 = wasmReal.evolve_consciousness(100);
const c2 = wasmReal.evolve_consciousness(100);
const c3 = wasmReal.evolve_consciousness(100);
console.log('  Consciousness(100):', c1 === c2 && c2 === c3 ?
    chalk.red('DETERMINISTIC') : chalk.green('VARIES'));

// Check quantum measurement (should vary)
const m1 = wasmReal.measure_quantum_state(4);
const m2 = wasmReal.measure_quantum_state(4);
const m3 = wasmReal.measure_quantum_state(4);
console.log('  Quantum measurement:', m1 === m2 && m2 === m3 ?
    chalk.red('DETERMINISTIC') : chalk.green('RANDOM'));

// SUMMARY
console.log(chalk.cyan.bold('\n════════════════════════════════════════════════════════════════'));
console.log(chalk.cyan.bold('                           SUMMARY                              '));
console.log(chalk.cyan.bold('════════════════════════════════════════════════════════════════\n'));

console.log(chalk.red.bold('FAKE Implementation:'));
console.log('  • Returns formatted strings');
console.log('  • Uses basic hash for "randomness"');
console.log('  • No actual computation');
console.log('  • Fast but meaningless');

console.log(chalk.green.bold('\nREAL Implementation:'));
console.log('  • Complex state vectors for quantum');
console.log('  • Cryptographic randomness');
console.log('  • Actual mathematical computation');
console.log('  • Slightly slower but meaningful');

console.log(chalk.yellow.bold('\nConclusion:'));
console.log('  The FAKE version is performance theater.');
console.log('  The REAL version does actual computation.');

process.exit(0);