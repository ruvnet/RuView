const { createSolver } = require('../../src/solver.js');

// Generate a simple diagonally dominant matrix
function generateTestMatrix(size) {
  const matrix = [];
  for (let i = 0; i < size; i++) {
    const row = new Array(size).fill(0);

    // Add some off-diagonal elements
    let rowSum = 0;
    for (let j = 0; j < size; j++) {
      if (i !== j) {
        const value = (Math.random() - 0.5) * 0.3;
        row[j] = value;
        rowSum += Math.abs(value);
      }
    }

    // Ensure diagonal dominance
    row[i] = rowSum + 1.0 + Math.random();
    matrix.push(row);
  }

  return {
    data: matrix,
    rows: size,
    cols: size,
    format: 'dense'
  };
}

async function runMiniBenchmark() {
  console.log('🧪 Running Mini Convergence Benchmark');
  console.log('=' .repeat(50));

  const methods = ['jacobi', 'conjugate_gradient'];
  const sizes = [5, 10];

  for (const method of methods) {
    console.log(`\n📊 Testing ${method.toUpperCase()}:`);

    for (const size of sizes) {
      console.log(`\n  Size ${size}x${size}:`);

      try {
        const matrix = generateTestMatrix(size);
        const b = Array.from({ length: size }, () => Math.random() * 10);

        const solver = await createSolver({
          matrix: matrix,
          method: method,
          tolerance: 1e-8,
          maxIterations: 100,
          verbose: false
        });

        const startTime = Date.now();
        const result = await solver.solve(b);
        const endTime = Date.now();

        console.log(`    ✅ Converged: ${result.converged}`);
        console.log(`    📈 Iterations: ${result.iterations}`);
        console.log(`    🎯 Convergence Rate: ${result.convergenceRate?.toFixed(1)}%`);
        console.log(`    📊 Grade: ${result.performanceGrade}`);
        console.log(`    ⏱️  Time: ${endTime - startTime}ms`);
        console.log(`    🔬 Residual: ${result.residual?.toExponential(3)}`);
        console.log(`    📉 Reduction: ${result.reductionFactor?.toExponential(3)}`);

      } catch (error) {
        console.log(`    ❌ Failed: ${error.message}`);
      }
    }
  }

  console.log('\n' + '='.repeat(50));
  console.log('🎉 Mini benchmark completed!');
}

if (require.main === module) {
  runMiniBenchmark().catch(console.error);
}

module.exports = { runMiniBenchmark };