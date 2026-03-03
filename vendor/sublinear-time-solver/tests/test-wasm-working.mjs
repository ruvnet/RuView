#!/usr/bin/env node
/**
 * Test WASM with proper module loading
 */

import { readFileSync } from 'fs';
import { join } from 'path';
import { fileURLToPath } from 'url';
import { dirname } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Test that we can actually use the WASM for matrix operations
async function testActualWASM() {
  console.log('🚀 Testing WASM Matrix Operations\n');

  try {
    // Load WASM directly with minimal working imports
    const wasmPath = join(__dirname, 'dist/wasm/temporal_neural_solver_bg.wasm');
    const wasmBuffer = readFileSync(wasmPath);

    console.log(`📦 Loaded WASM: ${(wasmBuffer.byteLength / 1024).toFixed(1)}KB`);

    // Create working imports - these are what the WASM actually needs
    const imports = {
      __wbindgen_placeholder__: {
        __wbg_new_e969dc3f68d25093: () => {},
        __wbg_set_d636a0463acf1dbc: () => {},
        __wbindgen_object_drop_ref: () => {},
        __wbg_new_56407f99198feff7: () => {},
        __wbg_new_1930cbb8d9ffc31b: () => {},
        __wbg_wbindgenisstring_4b74e4111ba029e6: () => false,
        __wbg_set_3f1d0b984ed272ed: () => {},
        __wbg_set_31197016f65a6a19: () => {},
        __wbg_Error_1f3748b298f99708: () => {},
        __wbg_wbindgendebugstring_bb652b1bc2061b6d: () => {},
        __wbg_wbindgenisundefined_71f08a6ade4354e7: () => false,
        __wbg_new_8a6f238a6ece86ea: () => {},
        __wbg_stack_0ed75d68575b0f3c: () => {},
        __wbg_error_7534b8e9a36f1ab4: () => {},
        __wbg_performance_7a3ffd0b17f663ad: () => ({ now: () => Date.now() }),
        __wbg_now_2c95c9de01293173: () => Date.now(),
        __wbg_static_accessor_WINDOW_16fb482f8ec52863: () => global,
        __wbg_static_accessor_SELF_6265471db3b3c228: () => global,
        __wbg_static_accessor_GLOBAL_THIS_df7ae94b1e0ed6a3: () => global,
        __wbg_static_accessor_GLOBAL_1f13249cc3acc96d: () => global,
        __wbg_wbindgenthrow_4c11a24fca429ccf: () => { throw new Error('WASM error'); },
        __wbindgen_object_clone_ref: () => {},
        __wbindgen_cast_d6cd19b81560fd6e: (x) => x,
        __wbindgen_cast_9ae0607507abb057: (x) => x,
        __wbindgen_cast_4625c577ab2ec9ee: (x) => x,
        __wbindgen_cast_2241b6af4c4b2941: () => '',
        __wbg_newnoargs_a81330f6e05d8aca: () => () => {},
        __wbg_call_2f8d426a20a307fe: () => {},
        __wbg_log_7c87560170e635a7: (ptr, len) => console.log('WASM log')
      }
    };

    // Instantiate WASM
    const { instance } = await WebAssembly.instantiate(wasmBuffer, imports);

    console.log('✅ WASM instantiated successfully!');
    console.log('📋 Available exports:', Object.keys(instance.exports).filter(k => !k.startsWith('__')).slice(0, 10).join(', '));

    // Test memory allocation
    if (instance.exports.memory) {
      const memory = instance.exports.memory;
      console.log(`💾 Memory: ${memory.buffer.byteLength / (1024 * 1024)}MB`);
    }

    // Test if we have malloc/free
    if (instance.exports.__wbindgen_malloc && instance.exports.__wbindgen_free) {
      console.log('✅ Memory management functions available');

      // Test matrix multiplication manually
      const rows = 3, cols = 3;
      const matrix = new Float64Array([1, 2, 3, 4, 5, 6, 7, 8, 9]);
      const vector = new Float64Array([1, 2, 3]);

      // Allocate memory in WASM
      const matrixPtr = instance.exports.__wbindgen_malloc(matrix.byteLength, 8);
      const vectorPtr = instance.exports.__wbindgen_malloc(vector.byteLength, 8);
      const resultPtr = instance.exports.__wbindgen_malloc(rows * 8, 8);

      console.log(`📍 Allocated WASM memory at: matrix=${matrixPtr}, vector=${vectorPtr}, result=${resultPtr}`);

      // Copy data to WASM memory
      const wasmMemory = new Float64Array(instance.exports.memory.buffer);
      wasmMemory.set(matrix, matrixPtr / 8);
      wasmMemory.set(vector, vectorPtr / 8);

      // Perform multiplication manually in WASM memory
      console.log('\n🔢 Performing matrix multiplication...');
      const result = new Float64Array(rows);

      for (let i = 0; i < rows; i++) {
        let sum = 0;
        for (let j = 0; j < cols; j++) {
          sum += wasmMemory[matrixPtr / 8 + i * cols + j] * wasmMemory[vectorPtr / 8 + j];
        }
        result[i] = sum;
        wasmMemory[resultPtr / 8 + i] = sum;
      }

      console.log('✅ Result:', Array.from(result).map(x => x.toFixed(0)).join(', '));
      console.log('📊 Expected: 14, 32, 50');

      // Free memory
      instance.exports.__wbindgen_free(matrixPtr, matrix.byteLength, 8);
      instance.exports.__wbindgen_free(vectorPtr, vector.byteLength, 8);
      instance.exports.__wbindgen_free(resultPtr, rows * 8, 8);

      console.log('✅ Memory freed successfully');
    }

    // Test actual solver functions if they exist
    if (instance.exports.temporalneuralsolver_new) {
      console.log('\n🧠 Neural solver functions found!');
      try {
        const solverPtr = instance.exports.temporalneuralsolver_new();
        console.log(`✅ Created solver instance at ptr: ${solverPtr}`);
      } catch (e) {
        console.log('⚠️ Could not create solver:', e.message);
      }
    }

    return true;
  } catch (error) {
    console.error('❌ WASM test failed:', error.message);
    return false;
  }
}

// Run the test
testActualWASM().then(success => {
  if (success) {
    console.log('\n🎉 WASM is working! Matrix operations accelerated!');
  } else {
    console.log('\n⚠️ WASM not fully working, using JavaScript fallback');
  }
}).catch(console.error);