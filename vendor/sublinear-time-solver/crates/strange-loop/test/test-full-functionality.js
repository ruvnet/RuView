#!/usr/bin/env node

const wasm = require('../wasm/strange_loop.js');

console.log('🔬 Strange Loops Full Functionality Test\n');
console.log('========================================\n');

// Initialize WASM
wasm.init_wasm();

// Test all 22 WASM exports
const allTests = [
  // Core
  { name: 'get_version', test: () => wasm.get_version() },
  { name: 'get_system_info', test: () => wasm.get_system_info() },

  // Nano-Agents
  { name: 'create_nano_swarm', test: () => wasm.create_nano_swarm(100) },
  { name: 'run_swarm_ticks', test: () => wasm.run_swarm_ticks(1000) },
  { name: 'benchmark_nano_agents', test: () => wasm.benchmark_nano_agents(50) },

  // Quantum
  { name: 'quantum_superposition', test: () => wasm.quantum_superposition(4) },
  { name: 'measure_quantum_state', test: () => wasm.measure_quantum_state(4) },
  { name: 'quantum_classical_hybrid', test: () => wasm.quantum_classical_hybrid(3, 64) },

  // Consciousness
  { name: 'evolve_consciousness', test: () => wasm.evolve_consciousness(500) },
  { name: 'calculate_phi', test: () => wasm.calculate_phi(10, 30) },
  { name: 'verify_consciousness', test: () => wasm.verify_consciousness(0.5, 0.7, 0.6) },

  // Strange Attractors
  { name: 'create_lorenz_attractor', test: () => wasm.create_lorenz_attractor(10, 28, 2.667) },
  { name: 'step_attractor', test: () => wasm.step_attractor(1, 1, 1, 0.01) },

  // Sublinear Solvers
  { name: 'solve_linear_system_sublinear', test: () => wasm.solve_linear_system_sublinear(1000, 0.001) },
  { name: 'compute_pagerank', test: () => wasm.compute_pagerank(10000, 0.85) },

  // Temporal
  { name: 'create_retrocausal_loop', test: () => wasm.create_retrocausal_loop(100) },
  { name: 'predict_future_state', test: () => wasm.predict_future_state(10, 500) },
  { name: 'detect_temporal_patterns', test: () => wasm.detect_temporal_patterns(1000) },

  // Loops
  { name: 'create_lipschitz_loop', test: () => wasm.create_lipschitz_loop(0.9) },
  { name: 'verify_convergence', test: () => wasm.verify_convergence(0.9, 100) },
  { name: 'create_self_modifying_loop', test: () => wasm.create_self_modifying_loop(0.7) },
];

let passed = 0;
let failed = 0;

console.log('Running', allTests.length, 'tests...\n');

for (const { name, test } of allTests) {
  try {
    const result = test();
    console.log(`✅ ${name}: ${typeof result === 'object' ? JSON.stringify(result) : result}`);
    passed++;
  } catch (error) {
    console.log(`❌ ${name}: ${error.message}`);
    failed++;
  }
}

console.log('\n========================================');
console.log(`Results: ${passed}/${allTests.length} passed, ${failed} failed`);

if (failed === 0) {
  console.log('🎉 All tests passed! Full functionality verified.');
  process.exit(0);
} else {
  console.log('⚠️  Some tests failed. Please review.');
  process.exit(1);
}