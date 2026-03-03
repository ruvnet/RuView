#!/usr/bin/env node

const wasm = require('./wasm/strange_loop.js');

console.log('Testing fixed WASM functions...\n');

// Initialize
if (wasm.init_wasm) wasm.init_wasm();

// Test each function
const tests = [
    {
        name: 'Quantum Superposition',
        fn: () => wasm.quantum_superposition(3)
    },
    {
        name: 'Quantum Measurement',
        fn: () => wasm.measure_quantum_state(3)
    },
    {
        name: 'Nano Swarm',
        fn: () => wasm.create_nano_swarm(100)
    },
    {
        name: 'Run Swarm Ticks',
        fn: () => wasm.run_swarm_ticks(10)
    },
    {
        name: 'Sublinear Solver',
        fn: () => wasm.solve_linear_system_sublinear(1000, 0.001)
    },
    {
        name: 'Consciousness Evolution',
        fn: () => wasm.evolve_consciousness(100)
    },
    {
        name: 'Temporal Prediction',
        fn: () => wasm.predict_future_state(42.0, 1000)
    },
    {
        name: 'Lorenz Attractor',
        fn: () => wasm.create_lorenz_attractor(10, 28, 8/3)
    },
    {
        name: 'Calculate Phi',
        fn: () => wasm.calculate_phi(50, 200)
    }
];

let passed = 0;
let failed = 0;

for (const test of tests) {
    try {
        const result = test.fn();
        console.log(`✅ ${test.name}: ${String(result).substring(0, 60)}...`);
        passed++;
    } catch (e) {
        console.log(`❌ ${test.name}: ${e.message}`);
        failed++;
    }
}

console.log(`\n========================================`);
console.log(`Results: ${passed} passed, ${failed} failed`);

if (failed === 0) {
    console.log('🎉 All functions work without crashes!');
} else {
    console.log(`⚠️  ${failed} functions still have issues.`);
}