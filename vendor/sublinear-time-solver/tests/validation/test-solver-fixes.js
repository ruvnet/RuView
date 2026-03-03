#!/usr/bin/env node

const { createSolver, JSSolver } = require('../src/solver.js');
const { MatrixUtils } = require('../src/utils/matrix-utils.js');

/**
 * Comprehensive test suite for solver fixes
 */
async function runSolverFixTests() {
  console.log('🧪 Comprehensive Solver Fix Test Suite');
  console.log('=====================================\n');

  let totalTests = 0;
  let passedTests = 0;
  const results = [];

  // Test Case 1: Auto-fix diagonal issues
  console.log('Test 1: Auto-fix missing diagonal elements');
  console.log('-'.repeat(45));

  try {
    totalTests++;

    // Create matrix with missing diagonal
    const problematicMatrix = {
      rows: 3,
      cols: 3,
      format: 'coo',
      entries: 5,
      data: {
        rowIndices: [0, 0, 1, 2, 2],
        colIndices: [1, 2, 2, 0, 1],
        values: [1, -1, 2, -1, 1]
      }
    };

    const vector = [1, 2, 3];

    // Should auto-fix the matrix
    const solver = await createSolver({
      matrix: problematicMatrix,
      method: 'jacobi',
      tolerance: 1e-8,
      maxIterations: 100,
      autoFixMatrix: true,
      verbose: true
    });

    const result = await solver.solve(vector);

    if (result.converged) {
      console.log('✅ PASS: Auto-fix enabled successful convergence');
      passedTests++;
      results.push({ test: 'Auto-fix diagonal', status: 'PASS', details: `Converged in ${result.iterations} iterations` });
    } else {
      console.log('❌ FAIL: Auto-fix did not achieve convergence');
      results.push({ test: 'Auto-fix diagonal', status: 'FAIL', details: `Did not converge after ${result.iterations} iterations` });
    }

  } catch (error) {
    console.log(`❌ FAIL: Auto-fix test error: ${error.message}`);
    results.push({ test: 'Auto-fix diagonal', status: 'FAIL', details: error.message });
  }

  console.log();

  // Test Case 2: Well-conditioned matrix generation
  console.log('Test 2: Well-conditioned matrix generation');
  console.log('-'.repeat(45));

  try {
    totalTests++;

    for (const size of [50, 100, 200]) {
      console.log(`  Testing ${size}×${size} matrix...`);

      const matrix = MatrixUtils.generateWellConditionedSparseMatrix(size, 0.05, {
        diagonalStrategy: 'rowsum_plus_one',
        ensureDominance: true
      });

      const conditioning = MatrixUtils.analyzeConditioning(matrix);

      if (conditioning.isWellConditioned && conditioning.isDiagonallyDominant) {
        console.log(`    ✅ Size ${size}: Grade ${conditioning.conditioningGrade}, dominance ratio ${conditioning.diagonalDominanceRatio.toFixed(3)}`);
      } else {
        console.log(`    ❌ Size ${size}: Poor conditioning (Grade ${conditioning.conditioningGrade})`);
        throw new Error(`Poor conditioning for size ${size}`);
      }
    }

    console.log('✅ PASS: All matrix sizes well-conditioned');
    passedTests++;
    results.push({ test: 'Well-conditioned generation', status: 'PASS', details: 'All sizes passed conditioning checks' });

  } catch (error) {
    console.log(`❌ FAIL: Matrix generation test error: ${error.message}`);
    results.push({ test: 'Well-conditioned generation', status: 'FAIL', details: error.message });
  }

  console.log();

  // Test Case 3: Convergence rate testing
  console.log('Test 3: Convergence rate analysis');
  console.log('-'.repeat(45));

  try {
    totalTests++;

    const testConfigs = [
      { size: 50, sparsity: 0.05, method: 'jacobi', matrixType: 'general' },
      { size: 50, sparsity: 0.05, method: 'gauss-seidel', matrixType: 'general' },
      { size: 50, sparsity: 0.05, method: 'conjugate-gradient', matrixType: 'symmetric' },
      { size: 100, sparsity: 0.03, method: 'jacobi', matrixType: 'general' },
      { size: 100, sparsity: 0.03, method: 'gauss-seidel', matrixType: 'general' },
      { size: 100, sparsity: 0.03, method: 'conjugate-gradient', matrixType: 'symmetric' }
    ];

    let convergenceCount = 0;
    const convergenceResults = [];

    for (const config of testConfigs) {
      console.log(`  Testing ${config.method} on ${config.size}×${config.size} ${config.matrixType} matrix...`);

      const matrix = config.matrixType === 'symmetric'
        ? MatrixUtils.generateSymmetricPositiveDefiniteMatrix(config.size, config.sparsity)
        : MatrixUtils.generateWellConditionedSparseMatrix(config.size, config.sparsity);

      const vector = Array.from({ length: config.size }, () => Math.random() * 10 - 5);

      const solver = await createSolver({
        matrix,
        method: config.method,
        tolerance: 1e-8,
        maxIterations: 500,
        verbose: false
      });

      const result = await solver.solve(vector);

      const testResult = {
        ...config,
        converged: result.converged,
        iterations: result.iterations,
        residual: result.residual
      };

      convergenceResults.push(testResult);

      if (result.converged) {
        convergenceCount++;
        console.log(`    ✅ Converged in ${result.iterations} iterations (residual: ${result.residual.toExponential(2)})`);
      } else {
        console.log(`    ❌ Failed to converge (residual: ${result.residual.toExponential(2)})`);
      }
    }

    const convergenceRate = (convergenceCount / testConfigs.length) * 100;
    console.log(`\nOverall convergence rate: ${convergenceRate.toFixed(1)}%`);

    if (convergenceRate >= 90) {
      console.log('✅ PASS: Convergence rate ≥ 90%');
      passedTests++;
      results.push({ test: 'Convergence rate', status: 'PASS', details: `${convergenceRate.toFixed(1)}% convergence rate` });
    } else {
      console.log('❌ FAIL: Convergence rate < 90%');
      results.push({ test: 'Convergence rate', status: 'FAIL', details: `Only ${convergenceRate.toFixed(1)}% convergence rate` });
    }

  } catch (error) {
    console.log(`❌ FAIL: Convergence rate test error: ${error.message}`);
    results.push({ test: 'Convergence rate', status: 'FAIL', details: error.message });
  }

  console.log();

  // Test Case 4: Validation and error handling
  console.log('Test 4: Enhanced validation and error handling');
  console.log('-'.repeat(45));

  try {
    totalTests++;

    // Test that invalid matrices are properly detected
    const invalidMatrices = [
      {
        name: "Missing diagonal with autoFix disabled",
        matrix: {
          rows: 3, cols: 3, format: 'coo', entries: 3,
          data: { rowIndices: [0, 1, 2], colIndices: [1, 2, 0], values: [1, 1, 1] }
        },
        shouldFail: true,
        autoFix: false
      },
      {
        name: "Zero diagonal elements",
        matrix: {
          rows: 2, cols: 2, format: 'dense',
          data: [[0, 1], [1, 2]]
        },
        shouldFail: true,
        autoFix: false
      }
    ];

    let validationTestsPassed = 0;

    for (const test of invalidMatrices) {
      try {
        const solver = await createSolver({
          matrix: test.matrix,
          method: 'jacobi',
          autoFixMatrix: test.autoFix,
          verbose: false
        });

        const result = await solver.solve([1, 1]);

        if (test.shouldFail) {
          console.log(`  ❌ ${test.name}: Should have failed but didn't`);
        } else {
          console.log(`  ✅ ${test.name}: Passed as expected`);
          validationTestsPassed++;
        }

      } catch (error) {
        if (test.shouldFail) {
          console.log(`  ✅ ${test.name}: Correctly failed with: ${error.message.slice(0, 50)}...`);
          validationTestsPassed++;
        } else {
          console.log(`  ❌ ${test.name}: Unexpectedly failed with: ${error.message}`);
        }
      }
    }

    if (validationTestsPassed === invalidMatrices.length) {
      console.log('✅ PASS: All validation tests behaved correctly');
      passedTests++;
      results.push({ test: 'Validation handling', status: 'PASS', details: 'All validation cases handled correctly' });
    } else {
      console.log(`❌ FAIL: ${validationTestsPassed}/${invalidMatrices.length} validation tests passed`);
      results.push({ test: 'Validation handling', status: 'FAIL', details: `Only ${validationTestsPassed}/${invalidMatrices.length} passed` });
    }

  } catch (error) {
    console.log(`❌ FAIL: Validation test error: ${error.message}`);
    results.push({ test: 'Validation handling', status: 'FAIL', details: error.message });
  }

  console.log();

  // Test Case 5: Performance with large matrices
  console.log('Test 5: Performance with larger matrices');
  console.log('-'.repeat(45));

  try {
    totalTests++;

    const largeMatrix = MatrixUtils.generateWellConditionedSparseMatrix(500, 0.02);
    const largeVector = Array.from({ length: 500 }, () => Math.random() * 5);

    console.log(`  Testing 500×500 matrix (${largeMatrix.entries} non-zeros)...`);

    const startTime = Date.now();

    const solver = await createSolver({
      matrix: largeMatrix,
      method: 'jacobi',
      tolerance: 1e-6,
      maxIterations: 1000,
      verbose: false
    });

    const result = await solver.solve(largeVector);
    const elapsed = Date.now() - startTime;

    console.log(`  Solve time: ${elapsed}ms`);
    console.log(`  Iterations: ${result.iterations}`);
    console.log(`  Converged: ${result.converged ? 'Yes' : 'No'}`);
    console.log(`  Final residual: ${result.residual.toExponential(2)}`);

    if (result.converged && elapsed < 10000) { // Should solve within 10 seconds
      console.log('✅ PASS: Large matrix solved efficiently');
      passedTests++;
      results.push({ test: 'Large matrix performance', status: 'PASS', details: `Solved in ${elapsed}ms with ${result.iterations} iterations` });
    } else {
      console.log('❌ FAIL: Large matrix performance unsatisfactory');
      results.push({ test: 'Large matrix performance', status: 'FAIL', details: `${elapsed}ms, converged: ${result.converged}` });
    }

  } catch (error) {
    console.log(`❌ FAIL: Large matrix test error: ${error.message}`);
    results.push({ test: 'Large matrix performance', status: 'FAIL', details: error.message });
  }

  // Summary
  console.log('\n' + '='.repeat(60));
  console.log('🎯 TEST SUMMARY');
  console.log('='.repeat(60));
  console.log(`Total tests: ${totalTests}`);
  console.log(`Passed: ${passedTests}`);
  console.log(`Failed: ${totalTests - passedTests}`);
  console.log(`Success rate: ${((passedTests / totalTests) * 100).toFixed(1)}%`);

  console.log('\nDetailed Results:');
  for (const result of results) {
    const status = result.status === 'PASS' ? '✅' : '❌';
    console.log(`  ${status} ${result.test}: ${result.details}`);
  }

  if (passedTests === totalTests) {
    console.log('\n🎉 ALL TESTS PASSED! The Jacobi solver fixes are working correctly.');
    return true;
  } else {
    console.log(`\n⚠️  ${totalTests - passedTests} tests failed. Review the fixes.`);
    return false;
  }
}

// Run the test suite
if (require.main === module) {
  runSolverFixTests()
    .then(success => {
      process.exit(success ? 0 : 1);
    })
    .catch(error => {
      console.error('Fatal test error:', error);
      process.exit(1);
    });
}

module.exports = { runSolverFixTests };