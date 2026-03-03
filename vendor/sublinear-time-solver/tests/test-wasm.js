#!/usr/bin/env node
/**
 * Test that WASM modules actually work
 */

import { WASMAccelerator } from './dist/core/wasm-integration.js';
import { SublinearSolver } from './dist/core/solver.js';

async function testWASM() {
  console.log('🧪 Testing WASM Integration\n');

  // Test 1: Initialize WASM
  console.log('1️⃣ Initializing WASM modules...');
  const accelerator = new WASMAccelerator();
  const initialized = await accelerator.initialize();

  if (initialized) {
    console.log('✅ WASM modules loaded successfully\n');
  } else {
    console.log('⚠️ WASM modules not loading - fallback to JS\n');
  }

  // Test 2: Test PageRank with WASM
  console.log('2️⃣ Testing PageRank with WASM...');
  try {
    const graphReasoner = accelerator.getGraphReasoner();
    const adjacency = {
      rows: 4,
      cols: 4,
      data: [
        [0, 1, 1, 0],
        [1, 0, 0, 1],
        [0, 1, 0, 1],
        [1, 0, 1, 0]
      ],
      format: 'dense'
    };

    const ranks = graphReasoner.computePageRank(adjacency, 0.85, 100);
    console.log(`✅ PageRank computed: [${Array.from(ranks).map(r => r.toFixed(3)).join(', ')}]\n`);
  } catch (error) {
    console.log(`❌ PageRank failed: ${error.message}\n`);
  }

  // Test 3: Test Temporal Neural Solver
  console.log('3️⃣ Testing Temporal Neural Solver...');
  try {
    const temporal = accelerator.getTemporalNeural();
    const matrix = new Float64Array([1, 2, 3, 4, 5, 6, 7, 8, 9]);
    const vector = new Float64Array([1, 2, 3]);

    const result = temporal.multiplyMatrixVector(matrix, vector, 3, 3);
    console.log(`✅ Matrix multiplication: [${Array.from(result).map(r => r.toFixed(1)).join(', ')}]\n`);
  } catch (error) {
    console.log(`❌ Matrix multiplication failed: ${error.message}\n`);
  }

  // Test 4: Test Temporal Advantage
  console.log('4️⃣ Testing Temporal Advantage Prediction...');
  try {
    const temporal = accelerator.getTemporalNeural();
    const matrix = {
      rows: 3,
      cols: 3,
      data: [[2, -1, 0], [-1, 2, -1], [0, -1, 2]],
      format: 'dense'
    };
    const vector = [1, 2, 1];

    const result = await temporal.predictWithTemporalAdvantage(matrix, vector, 10900);
    console.log(`✅ Temporal Advantage:`);
    console.log(`   Light travel time: ${result.lightTravelTimeMs.toFixed(2)}ms`);
    console.log(`   Compute time: ${result.computeTimeMs.toFixed(2)}ms`);
    console.log(`   Temporal advantage: ${result.temporalAdvantageMs.toFixed(2)}ms`);
    console.log(`   Solution: [${result.solution.map(x => x.toFixed(3)).join(', ')}]\n`);
  } catch (error) {
    console.log(`❌ Temporal prediction failed: ${error.message}\n`);
  }

  // Test 5: Full solver with WASM
  console.log('5️⃣ Testing Full Solver with WASM...');
  try {
    const solver = new SublinearSolver({
      method: 'neumann',
      epsilon: 1e-6,
      maxIterations: 100
    });

    const matrix = {
      rows: 3,
      cols: 3,
      data: [[4, -1, 0], [-1, 4, -1], [0, -1, 4]],
      format: 'dense'
    };
    const vector = [3, 2, 3];

    const result = await solver.solve(matrix, vector);
    console.log(`✅ Solver converged: ${result.converged}`);
    console.log(`   Iterations: ${result.iterations}`);
    console.log(`   Solution: [${result.solution.map(x => x.toFixed(3)).join(', ')}]\n`);
  } catch (error) {
    console.log(`❌ Solver failed: ${error.message}\n`);
  }

  console.log('✨ WASM testing complete!');
}

testWASM().catch(console.error);