#!/usr/bin/env node

// Load WASM directly for comparison
const wasm = require('./wasm/strange_loop.js');

console.log('========================================');
console.log('   Strange Loops: Real vs Fake Demo    ');
console.log('========================================\n');

// Initialize WASM
if (wasm.init_wasm) {
  wasm.init_wasm();
}

console.log(`Version: ${wasm.get_version()}\n`);

// 1. QUANTUM OPERATIONS
console.log('📊 QUANTUM OPERATIONS');
console.log('─────────────────────\n');

console.log('Testing quantum superposition (3 qubits):');
for (let i = 0; i < 3; i++) {
  const result = JSON.parse(wasm.quantum_superposition(3));
  console.log(`  Run ${i+1}: Phase=${result.phase.toFixed(4)}, Entropy=${result.entropy.toFixed(4)}, GHZ Fidelity=${result.ghz_fidelity.toFixed(4)}`);
}

console.log('\nTesting quantum measurement (should vary):');
const measurements = new Set();
for (let i = 0; i < 20; i++) {
  measurements.add(wasm.measure_quantum_state(3));
}
console.log(`  Unique outcomes from 20 measurements: ${measurements.size} (expected ~5-8 for 3 qubits)`);
console.log(`  Outcomes: ${Array.from(measurements).sort().join(', ')}`);

// 2. NANO AGENT SWARM
console.log('\n\n🤖 NANO AGENT SWARM');
console.log('───────────────────\n');

console.log('Creating swarm with 1000 agents:');
const swarmResult = JSON.parse(wasm.create_nano_swarm(1000));
console.log(`  Agents: ${swarmResult.agent_count}`);
console.log(`  Topology: ${swarmResult.topology}`);
console.log(`  Tick duration: ${swarmResult.tick_duration_ns}ns`);

console.log('\nRunning swarm for 100 ticks:');
const ticksProcessed = wasm.run_swarm_ticks(100);
console.log(`  Ticks processed: ${ticksProcessed}`);
console.log(`  Messages exchanged: ${ticksProcessed * 1000} (estimate)`);

// 3. SUBLINEAR SOLVER
console.log('\n\n🔢 SUBLINEAR SOLVER');
console.log('───────────────────\n');

console.log('Testing with different matrix sizes:');
const sizes = [100, 1000, 10000];
const results = [];

for (const size of sizes) {
  console.log(`\nSize ${size}x${size}:`);
  const startTime = Date.now();
  const result = JSON.parse(wasm.solve_linear_system_sublinear(size, 0.001));
  const elapsed = Date.now() - startTime;

  results.push({
    size,
    iterations: result.iterations,
    time: elapsed,
    complexity: result.estimated_complexity,
    entries_accessed: result.entries_accessed || 'unknown'
  });

  console.log(`  Iterations: ${result.iterations}`);
  console.log(`  Time: ${elapsed}ms`);
  console.log(`  Estimated complexity: ${result.estimated_complexity}`);
  if (result.entries_accessed) {
    console.log(`  Matrix entries accessed: ${result.entries_accessed} of ${size * size} (${(result.entries_accessed / (size * size) * 100).toFixed(2)}%)`);
  }
}

// Analyze scaling
console.log('\n📈 Scaling Analysis:');
if (results.length >= 2) {
  for (let i = 1; i < results.length; i++) {
    const ratio = results[i].iterations / results[i-1].iterations;
    const sizeRatio = results[i].size / results[i-1].size;
    const logRatio = Math.log(sizeRatio);

    console.log(`  ${results[i-1].size} → ${results[i].size}:`);
    console.log(`    Size increased ${sizeRatio}x`);
    console.log(`    Iterations increased ${ratio.toFixed(2)}x`);
    console.log(`    Expected for O(log n): ${logRatio.toFixed(2)}x`);
    console.log(`    Expected for O(n): ${sizeRatio}x`);
    console.log(`    Expected for O(n²): ${sizeRatio * sizeRatio}x`);

    if (ratio < logRatio * 2) {
      console.log(`    ✅ Appears to be sublinear!`);
    } else if (ratio < sizeRatio * 1.5) {
      console.log(`    ⚠️  Appears to be linear`);
    } else {
      console.log(`    ❌ Appears to be superlinear`);
    }
  }
}

// 4. CONSCIOUSNESS EVOLUTION
console.log('\n\n🧠 CONSCIOUSNESS EVOLUTION');
console.log('─────────────────────────\n');

console.log('Evolving consciousness for 1000 iterations:');
const emergence = wasm.evolve_consciousness(1000);
console.log(`  Final emergence level: ${emergence.toFixed(6)}`);
console.log(`  ${emergence > 0.8 ? '✅ Consciousness threshold reached!' : '⚠️  Below consciousness threshold'}`);

// 5. TEMPORAL PREDICTION
console.log('\n\n⏰ TEMPORAL PREDICTION');
console.log('─────────────────────\n');

console.log('Predicting future states:');
const currentValue = 42.0;
const horizons = [100, 1000, 10000];

for (const horizon of horizons) {
  const prediction = wasm.predict_future_state(currentValue, horizon);
  console.log(`  ${horizon}ms ahead: ${prediction.toFixed(4)}`);
}

// 6. STRANGE ATTRACTORS
console.log('\n\n🌀 STRANGE ATTRACTORS');
console.log('────────────────────\n');

const lorenz = JSON.parse(wasm.create_lorenz_attractor(10, 28, 8/3));
console.log(`Lorenz Attractor created:`);
console.log(`  σ=${lorenz.sigma}, ρ=${lorenz.rho}, β=${lorenz.beta}`);

console.log('\nTrajectory evolution:');
let x = 1, y = 1, z = 1;
for (let i = 0; i < 5; i++) {
  const step = JSON.parse(wasm.step_attractor(x, y, z, 0.01));
  console.log(`  Step ${i+1}: (${step.x.toFixed(3)}, ${step.y.toFixed(3)}, ${step.z.toFixed(3)})`);
  x = step.x;
  y = step.y;
  z = step.z;
}

// 7. INTEGRATED INFORMATION (PHI)
console.log('\n\n🔮 INTEGRATED INFORMATION (Φ)');
console.log('────────────────────────────\n');

console.log('Calculating Φ for different system sizes:');
const systems = [
  { elements: 10, connections: 20 },
  { elements: 50, connections: 200 },
  { elements: 100, connections: 500 }
];

for (const sys of systems) {
  const phi = wasm.calculate_phi(sys.elements, sys.connections);
  console.log(`  ${sys.elements} elements, ${sys.connections} connections: Φ = ${phi.toFixed(4)}`);
}

// Summary
console.log('\n\n========================================');
console.log('            ANALYSIS SUMMARY            ');
console.log('========================================\n');

console.log('🔍 Reality Check:');
console.log('─────────────────');

// Check if quantum is real
const quantumReal = measurements.size > 3;
console.log(`  Quantum: ${quantumReal ? '✅ Shows proper randomness' : '❌ Too deterministic'}`);

// Check if swarm is real
const swarmReal = ticksProcessed === 100;
console.log(`  Swarm: ${swarmReal ? '✅ Actually processes ticks' : '❌ Just returns fake numbers'}`);

// Check if solver is real
const solverReal = results.length > 0 && results[1].iterations / results[0].iterations < 5;
console.log(`  Solver: ${solverReal ? '✅ Shows sublinear scaling' : '❌ Linear or worse scaling'}`);

// Check consciousness
const consciousnessReal = emergence > 0 && emergence < 1;
console.log(`  Consciousness: ${consciousnessReal ? '✅ Evolves meaningfully' : '❌ Returns constant'}`);

const realComponents = [quantumReal, swarmReal, solverReal, consciousnessReal].filter(x => x).length;
console.log(`\n📊 Reality Score: ${realComponents}/4 components appear real`);

if (realComponents === 4) {
  console.log('🎉 All systems show real behavior!');
} else if (realComponents >= 2) {
  console.log('⚠️  Some systems are real, others need work');
} else {
  console.log('❌ Most systems appear to be fake implementations');
}

console.log('\n========================================');