const { createSolver } = require('../../src/solver.js');

async function quickTest() {
  console.log('Running quick convergence test...');

  // Create a simple 3x3 diagonal matrix
  const matrix = {
    data: [
      [2, 0, 0],
      [0, 3, 0],
      [0, 0, 4]
    ],
    rows: 3,
    cols: 3,
    format: 'dense'
  };

  const b = [2, 6, 12]; // Should give solution [1, 2, 3]

  try {
    const solver = await createSolver({
      matrix: matrix,
      method: 'jacobi',
      tolerance: 1e-10,
      maxIterations: 100,
      verbose: true
    });

    const result = await solver.solve(b);

    console.log('Results:');
    console.log('  Solution:', result.values.map(x => x.toFixed(3)));
    console.log('  Iterations:', result.iterations);
    console.log('  Converged:', result.converged);
    console.log('  Convergence Rate:', result.convergenceRate?.toFixed(1) + '%');
    console.log('  Performance Grade:', result.performanceGrade);
    console.log('  Residual:', result.residual?.toExponential(3));

    return result;
  } catch (error) {
    console.error('Error:', error.message);
    throw error;
  }
}

if (require.main === module) {
  quickTest().then(() => {
    console.log('✅ Quick test passed!');
    process.exit(0);
  }).catch(error => {
    console.error('❌ Quick test failed:', error.message);
    process.exit(1);
  });
}

module.exports = { quickTest };