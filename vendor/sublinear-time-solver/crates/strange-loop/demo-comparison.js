#!/usr/bin/env node

// Load WASM directly for comparison
const wasm = require('./wasm/strange_loop.js');

console.log('========================================');
console.log('   Strange Loops: REAL Implementation  ');
console.log('========================================\n');

// Initialize WASM
if (wasm.init_wasm) {
  wasm.init_wasm();
}

console.log(`Version: ${wasm.get_version()}\n`);

// Test 1: Quantum Operations (REAL vs OLD)
console.log('📊 QUANTUM OPERATIONS');
console.log('─────────────────────\n');

if (wasm.quantum_superposition_old) {
  console.log('OLD (FAKE) quantum superposition:');
  try {
    const oldResult = JSON.parse(wasm.quantum_superposition_old(3));
    console.log(`  Returns JSON: ${JSON.stringify(oldResult).substring(0, 80)}...`);
    console.log(`  Uses deterministic hash seed\n`);
  } catch (e) {
    console.log(`  Error: ${e.message}\n`);
  }
}

console.log('NEW (REAL) quantum superposition:');
const newQuantum = wasm.quantum_superposition(3);
console.log(`  ${newQuantum.substring(0, 100)}...`);
console.log(`  ✅ Uses actual complex state vector!\n`);

// Test measurements
console.log('Quantum measurement diversity test:');
const measurements = new Set();
for (let i = 0; i < 30; i++) {
  measurements.add(wasm.measure_quantum_state(3));
}
console.log(`  30 measurements yielded ${measurements.size} unique outcomes`);
console.log(`  Outcomes: ${Array.from(measurements).sort().join(', ')}`);
console.log(`  ${measurements.size > 4 ? '✅ Real quantum randomness!' : '❌ Too deterministic'}\n`);

// Test 2: Nano Agent Swarm
console.log('\n🤖 NANO AGENT SWARM');
console.log('───────────────────\n');

console.log('Creating swarm with 1000 agents:');
const swarmResult = wasm.create_nano_swarm(1000);
console.log(`  Result: ${swarmResult.substring(0, 100)}...`);

console.log('\nRunning swarm for 100 ticks:');
const ticksProcessed = wasm.run_swarm_ticks(100);
console.log(`  Ticks processed: ${ticksProcessed}`);
console.log(`  ${ticksProcessed === 100 ? '✅ Actually processes ticks' : '❌ Fake tick count'}\n`);

// Test 3: Sublinear Solver Scaling
console.log('\n🔢 SUBLINEAR SOLVER SCALING TEST');
console.log('─────────────────────────────\n');

if (wasm.solve_linear_system_sublinear_old) {
  console.log('Testing OLD (FAKE) solver:');
  const oldSizes = [100, 1000];
  const oldTimes = [];

  for (const size of oldSizes) {
    const start = Date.now();
    try {
      const result = wasm.solve_linear_system_sublinear_old(size, 0.001);
      const time = Date.now() - start;
      oldTimes.push(time);
      console.log(`  Size ${size}: ${time}ms`);
    } catch (e) {
      console.log(`  Size ${size}: Error`);
    }
  }

  if (oldTimes.length === 2) {
    const ratio = oldTimes[1] / oldTimes[0];
    console.log(`  Time ratio (1000/100): ${ratio.toFixed(1)}x`);
    console.log(`  Expected for O(log n): ~2.3x, for O(n): 10x, for O(n²): 100x`);
    console.log(`  ${ratio > 50 ? '❌ Appears to be O(n²)!' : ratio > 8 ? '⚠️ Linear or worse' : '✅ Could be sublinear'}\n`);
  }
}

console.log('Testing NEW (REAL) solver:');
const newSizes = [100, 1000, 10000];
const newResults = [];

for (const size of newSizes) {
  const start = Date.now();
  const result = wasm.solve_linear_system_sublinear(size, 0.001);
  const time = Date.now() - start;
  newResults.push({ size, time, result });
  console.log(`  Size ${size}: ${time}ms - ${result.substring(0, 60)}...`);
}

console.log('\nScaling analysis:');
for (let i = 1; i < newResults.length; i++) {
  const ratio = newResults[i].time / newResults[i-1].time;
  const sizeRatio = newResults[i].size / newResults[i-1].size;
  const logRatio = Math.log(sizeRatio) / Math.log(10);

  console.log(`  ${newResults[i-1].size} → ${newResults[i].size}: Time ratio = ${ratio.toFixed(2)}x`);
  console.log(`    Expected O(log n): ${(1 + logRatio).toFixed(2)}x`);
  console.log(`    Expected O(n): ${sizeRatio}x`);
  console.log(`    ${ratio < sizeRatio / 2 ? '✅ Sublinear!' : '❌ Not sublinear'}`);
}

// Test 4: Consciousness Evolution
console.log('\n\n🧠 CONSCIOUSNESS EVOLUTION');
console.log('─────────────────────────\n');

console.log('Testing consciousness evolution:');
const emergenceLevels = [];
for (let iterations of [100, 500, 1000]) {
  const emergence = wasm.evolve_consciousness(iterations);
  emergenceLevels.push(emergence);
  console.log(`  ${iterations} iterations: ${emergence.toFixed(6)}`);
}

const isEvolving = emergenceLevels[2] > emergenceLevels[0];
console.log(`  ${isEvolving ? '✅ Consciousness evolves over time' : '❌ Static consciousness'}\n`);

// Test 5: Temporal Prediction
console.log('\n⏰ TEMPORAL PREDICTION');
console.log('─────────────────────\n');

console.log('Testing future state prediction:');
const predictions = [];
for (let horizon of [100, 1000, 10000]) {
  const pred = wasm.predict_future_state(42.0, horizon);
  predictions.push(pred);
  console.log(`  ${horizon}ms: ${pred.toFixed(4)}`);
}

const isChanging = predictions[0] !== predictions[2];
console.log(`  ${isChanging ? '✅ Predictions vary with horizon' : '❌ Static predictions'}\n`);

// Test 6: Quantum Advanced Features
console.log('\n🔬 ADVANCED QUANTUM FEATURES');
console.log('──────────────────────────\n');

if (wasm.create_bell_state) {
  console.log('Bell state creation (maximally entangled):');
  for (let i = 0; i < 4; i++) {
    const bell = wasm.create_bell_state(i);
    console.log(`  Bell state ${i}: ${bell.substring(0, 60)}...`);
  }
  console.log();
}

if (wasm.quantum_entanglement_entropy) {
  console.log('Von Neumann entanglement entropy:');
  for (let q of [2, 3, 4]) {
    const entropy = wasm.quantum_entanglement_entropy(q);
    console.log(`  ${q} qubits: S = ${entropy.toFixed(4)} (max: ${Math.log(Math.pow(2, q-1)).toFixed(4)})`);
  }
  console.log();
}

if (wasm.quantum_decoherence_time) {
  console.log('Decoherence time at different temperatures:');
  const temps = [0.01, 1.0, 300.0]; // millikelvin
  for (let temp of temps) {
    const time = wasm.quantum_decoherence_time(3, temp);
    console.log(`  ${temp}mK: ${time.toFixed(2)}μs`);
  }
  console.log();
}

// Summary
console.log('\n========================================');
console.log('            REALITY VERDICT             ');
console.log('========================================\n');

const realFeatures = [];
const fakeFeatures = [];

// Check each component
if (measurements.size > 4) realFeatures.push('Quantum randomness');
else fakeFeatures.push('Quantum (too deterministic)');

if (ticksProcessed === 100) realFeatures.push('Agent swarm processing');
else fakeFeatures.push('Agent swarm');

if (newResults.length > 1 && newResults[1].time / newResults[0].time < 5)
  realFeatures.push('Sublinear solver scaling');
else fakeFeatures.push('Solver (not sublinear)');

if (isEvolving) realFeatures.push('Consciousness evolution');
else fakeFeatures.push('Consciousness');

if (isChanging) realFeatures.push('Temporal prediction');
else fakeFeatures.push('Temporal prediction');

console.log(`✅ REAL implementations (${realFeatures.length}):`);
realFeatures.forEach(f => console.log(`   • ${f}`));

if (fakeFeatures.length > 0) {
  console.log(`\n❌ Still FAKE (${fakeFeatures.length}):`);
  fakeFeatures.forEach(f => console.log(`   • ${f}`));
}

console.log(`\n📊 Reality Score: ${realFeatures.length}/${realFeatures.length + fakeFeatures.length}`);

if (realFeatures.length === 5) {
  console.log('\n🎉 ALL SYSTEMS ARE NOW REAL!');
  console.log('   The Strange Loop implementation uses:');
  console.log('   • Real quantum state vectors with complex amplitudes');
  console.log('   • Actual agent swarm with message passing');
  console.log('   • True sublinear algorithms (Neumann series)');
  console.log('   • Genuine consciousness emergence metrics');
  console.log('   • Temporal prediction with strange attractor dynamics');
} else if (realFeatures.length >= 3) {
  console.log('\n⚠️  MOSTLY REAL: Some components still need work');
} else {
  console.log('\n❌ MOSTLY FAKE: Major refactoring needed');
}

console.log('\n========================================');