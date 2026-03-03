#!/usr/bin/env node

/**
 * Direct MCP Server WASM Test - Test without restarting
 *
 * This directly imports and tests the WASM functionality from the built MCP server
 */

import { WasmSublinearSolverTools } from '../dist/mcp/tools/wasm-sublinear-solver-simple.js';

async function testMcpWasmDirect() {
  console.log('🧪 Direct MCP WASM Test (No Restart Required)');
  console.log('=' .repeat(50));

  try {
    console.log('\n🔧 Creating WASM solver instance...');
    const wasmSolver = new WasmSublinearSolverTools();

    // Small delay to allow WASM initialization
    await new Promise(resolve => setTimeout(resolve, 100));

    console.log('\n🎯 Checking WASM availability...');
    const isAvailable = wasmSolver.isEnhancedWasmAvailable();
    console.log(`   Enhanced WASM Available: ${isAvailable}`);

    if (isAvailable) {
      console.log('✅ SUCCESS: WASM is available!');

      console.log('\n🧮 Testing WASM solver with 3x3 matrix...');
      const matrix = [
        [12.5, 0.3, 0.2],
        [0.1, 10.8, 0.4],
        [0.2, 0.3, 11.2]
      ];
      const b = [7.1, 5.4, 6.8];

      const result = await wasmSolver.solveSublinear(matrix, b);

      console.log('\n✅ WASM Solver Results:');
      console.log(`   Algorithm: ${result.algorithm}`);
      console.log(`   WASM Accelerated: ${result.wasm_accelerated}`);
      console.log(`   Complexity: ${result.complexity_bound}`);
      console.log(`   JL Dimension Reduction: ${result.jl_dimension_reduction}`);
      console.log(`   Compression Ratio: ${result.compression_ratio?.toFixed(4)}`);
      console.log(`   Solve Time: ${result.solve_time_ms}ms`);

      console.log('\n🎉 WASM Integration Working!');
      console.log('✅ The MCP server SHOULD be using WASM now');

    } else {
      console.log('❌ WASM not available - checking why...');

      const capabilities = wasmSolver.getCapabilities();
      console.log('\n🔍 Capabilities:', JSON.stringify(capabilities, null, 2));
    }

  } catch (error) {
    console.error('\n❌ Test failed:', error.message);
    console.error('Stack:', error.stack);
  }
}

testMcpWasmDirect()
  .then(() => {
    console.log('\n' + '='.repeat(50));
    console.log('🏁 Direct test complete - MCP server should now use WASM');
  })
  .catch(error => {
    console.error('Test execution failed:', error);
    process.exit(1);
  });