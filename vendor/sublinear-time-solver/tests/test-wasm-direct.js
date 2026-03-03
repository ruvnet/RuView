#!/usr/bin/env node
/**
 * Test WASM modules directly
 */

import { readFileSync } from 'fs';
import { join } from 'path';

async function testWASMDirect() {
  console.log('🧪 Direct WASM Test\n');

  // Test 1: Load temporal_neural_solver
  try {
    console.log('Loading temporal_neural_solver...');
    const wasmPath = join(process.cwd(), 'dist/wasm/temporal_neural_solver_bg.wasm');
    const wasmBuffer = readFileSync(wasmPath);

    console.log(`  WASM size: ${(wasmBuffer.byteLength / 1024).toFixed(1)}KB`);

    // Try minimal imports
    const imports = {
      wbg: {
        __wbindgen_throw: () => {},
        __wbg_random_e6e0a85ff4db8ab6: () => Math.random()
      },
      env: {
        memory: new WebAssembly.Memory({ initial: 256 })
      }
    };

    const module = await WebAssembly.compile(wasmBuffer);
    const instance = await WebAssembly.instantiate(module, imports);

    console.log('✅ Temporal Neural Solver loaded');
    console.log('  Exports:', Object.keys(instance.exports).slice(0, 10).join(', '));

    // Test if we can use it
    if (instance.exports.memory) {
      console.log(`  Memory: ${instance.exports.memory.buffer.byteLength / (1024 * 1024)}MB`);
    }

  } catch (error) {
    console.log('❌ Temporal Neural Solver failed:', error.message);
  }

  // Test 2: Load graph_reasoner
  try {
    console.log('\nLoading graph_reasoner...');
    const wasmPath = join(process.cwd(), 'dist/wasm/graph_reasoner_bg.wasm');
    const wasmBuffer = readFileSync(wasmPath);

    console.log(`  WASM size: ${(wasmBuffer.byteLength / 1024).toFixed(1)}KB`);

    // More complete imports for graph reasoner
    const imports = {
      wbg: {
        __wbindgen_object_drop_ref: () => {},
        __wbindgen_string_new: () => {},
        __wbindgen_throw: () => {},
        __wbg_random_e6e0a85ff4db8ab6: () => Math.random(),
        __wbg_now_3141b3797eb98e0b: () => Date.now()
      },
      env: {
        memory: new WebAssembly.Memory({ initial: 256 })
      }
    };

    const module = await WebAssembly.compile(wasmBuffer);
    const instance = await WebAssembly.instantiate(module, imports);

    console.log('✅ Graph Reasoner loaded');
    console.log('  Exports:', Object.keys(instance.exports).slice(0, 10).join(', '));

  } catch (error) {
    console.log('❌ Graph Reasoner failed:', error.message);
  }

  // Test 3: Load strange_loop
  try {
    console.log('\nLoading strange_loop...');
    const wasmPath = join(process.cwd(), 'dist/wasm/strange_loop_bg.wasm');
    const wasmBuffer = readFileSync(wasmPath);

    console.log(`  WASM size: ${(wasmBuffer.byteLength / 1024).toFixed(1)}KB`);

    const imports = {
      wbg: {},
      env: {
        memory: new WebAssembly.Memory({ initial: 256 })
      }
    };

    const module = await WebAssembly.compile(wasmBuffer);
    const instance = await WebAssembly.instantiate(module, imports);

    console.log('✅ Strange Loop loaded');
    console.log('  Exports:', Object.keys(instance.exports).slice(0, 10).join(', '));

  } catch (error) {
    console.log('❌ Strange Loop failed:', error.message);
  }

  // Test 4: Try to use the JS bindings
  try {
    console.log('\nTesting with JS bindings...');

    // Dynamic import the temporal neural solver
    const TNS = await import('./dist/wasm/temporal_neural_solver.js');

    if (TNS.TemporalNeuralSolver) {
      console.log('✅ TemporalNeuralSolver class found');

      // Try to create an instance
      const solver = new TNS.TemporalNeuralSolver();
      console.log('✅ Created TemporalNeuralSolver instance');

      // Test prediction
      const input = new Float32Array(128).fill(0.5);
      const result = await solver.predict(input);
      console.log('✅ Prediction successful:', result);

    } else {
      console.log('⚠️ TemporalNeuralSolver class not found in exports');
    }

  } catch (error) {
    console.log('❌ JS bindings test failed:', error.message);
  }

  console.log('\n✨ Direct WASM test complete');
}

testWASMDirect().catch(console.error);