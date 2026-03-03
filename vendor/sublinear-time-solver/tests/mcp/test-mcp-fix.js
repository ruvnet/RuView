#!/usr/bin/env node

/**
 * Test that MCP Dense performance issue is fixed
 *
 * Original problem: 7700ms for 1000x1000 (190x slower than Python)
 * Fixed: Should be < 10ms (faster than Python's 40ms)
 */

import { SolverTools } from './dist/mcp/tools/solver.js';

async function testMCPFix() {
  console.log('🔧 Testing MCP Dense Performance Fix');
  console.log('=' .repeat(70));

  const sizes = [100, 500, 1000];
  const results = {};

  for (const size of sizes) {
    console.log(`\n📊 Testing ${size}x${size} matrix...`);

    // Create dense matrix (the problematic format)
    const matrix = {
      format: 'dense',
      rows: size,
      cols: size,
      data: []
    };

    // Generate diagonally dominant matrix
    for (let i = 0; i < size; i++) {
      const row = new Array(size).fill(0);
      row[i] = 10.0 + i * 0.01; // Strong diagonal

      // Add sparse off-diagonal elements
      const nnzPerRow = Math.max(1, Math.floor(size * 0.001));
      for (let k = 0; k < nnzPerRow; k++) {
        const j = Math.floor(Math.random() * size);
        if (i !== j) {
          row[j] = Math.random() * 0.1;
        }
      }

      // Store in dense format (the slow way)
      matrix.data.push(...row);
    }

    const vector = new Array(size).fill(1.0);

    // Test original slow path
    console.log('Testing original implementation (should use optimized now)...');
    const startOriginal = Date.now();

    try {
      const result = await SolverTools.solve({
        matrix,
        vector,
        epsilon: 1e-10,
        maxIterations: 1000
      });

      const timeOriginal = Date.now() - startOriginal;
      console.log(`  Time: ${timeOriginal}ms`);
      console.log(`  Method: ${result.method}`);
      console.log(`  Converged: ${result.converged}`);

      if (result.efficiency) {
        console.log(`  Speedup vs Python: ${result.efficiency.speedupVsPython?.toFixed(1)}x`);
        console.log(`  Speedup vs Broken: ${result.efficiency.speedupVsBroken?.toFixed(0)}x`);
      }

      results[size] = {
        time: timeOriginal,
        method: result.method,
        speedupVsPython: result.efficiency?.speedupVsPython,
        speedupVsBroken: result.efficiency?.speedupVsBroken
      };

      // Check performance targets
      const pythonBaseline = size === 100 ? 5 : size === 500 ? 18 : 40;
      const brokenTime = size === 100 ? 77 : size === 500 ? 1500 : 7700;

      if (timeOriginal < pythonBaseline) {
        console.log(`  ✅ FASTER than Python (${pythonBaseline}ms)`);
      } else if (timeOriginal < brokenTime / 100) {
        console.log(`  ✅ FIXED: ${(brokenTime / timeOriginal).toFixed(0)}x faster than broken`);
      } else {
        console.log(`  ⚠️  Still slow: ${timeOriginal}ms`);
      }

    } catch (error) {
      console.error(`  ❌ Error: ${error.message}`);
    }
  }

  // Summary
  console.log('\n' + '=' .repeat(70));
  console.log('📈 PERFORMANCE SUMMARY');
  console.log('=' .repeat(70));

  console.log('\nSize    Time(ms)   Method            vs Python   vs Broken');
  console.log('-'.repeat(60));

  for (const size of sizes) {
    if (results[size]) {
      const r = results[size];
      console.log(
        `${size.toString().padEnd(7)} ` +
        `${r.time.toString().padEnd(10)} ` +
        `${(r.method || 'unknown').padEnd(17)} ` +
        `${(r.speedupVsPython?.toFixed(1) + 'x' || 'N/A').padEnd(11)} ` +
        `${(r.speedupVsBroken?.toFixed(0) + 'x' || 'N/A').padEnd(9)}`
      );
    }
  }

  console.log('\n🎯 TARGET ACHIEVEMENTS:');
  const r1000 = results[1000];
  if (r1000) {
    if (r1000.time < 10) {
      console.log('✅ 1000x1000 < 10ms (TARGET MET)');
    } else if (r1000.time < 40) {
      console.log('✅ 1000x1000 < 40ms (faster than Python)');
    } else if (r1000.time < 100) {
      console.log('⚠️  1000x1000 < 100ms (partially fixed)');
    } else {
      console.log('❌ 1000x1000 still slow');
    }

    if (r1000.speedupVsBroken > 100) {
      console.log(`✅ ${r1000.speedupVsBroken.toFixed(0)}x speedup over broken implementation`);
    }
  }

  console.log('\n✅ MCP DENSE PERFORMANCE FIX STATUS:');
  if (r1000?.time < 40) {
    console.log('FIXED! The 190x slowdown has been resolved.');
    console.log(`New performance: ${r1000.time}ms (was 7700ms)`);
    console.log(`Improvement: ${(7700 / r1000.time).toFixed(0)}x faster`);
  } else {
    console.log('Optimization may need compilation. Run: npm run build');
  }
}

testMCPFix().catch(console.error);