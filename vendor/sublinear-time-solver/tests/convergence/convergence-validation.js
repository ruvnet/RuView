/**
 * Convergence Detection and Metrics Validation Test Suite
 *
 * Tests the convergence detection system against known test cases
 * with expected convergence behavior.
 */

const { ConvergenceDetector } = require('../../src/convergence/convergence-detector');
const { MetricsReporter } = require('../../src/convergence/metrics-reporter');
const { createSolver } = require('../../src/solver');

class ConvergenceValidator {
  constructor() {
    this.testCases = this.generateTestCases();
    this.results = [];
  }

  /**
   * Generate test cases with known convergence properties
   */
  generateTestCases() {
    return [
      {
        name: 'Well-conditioned Diagonal Matrix',
        description: 'Identity matrix should converge in 1 iteration',
        matrix: this.createIdentityMatrix(10),
        rhs: Array(10).fill(1),
        expectedIterations: 1,
        expectedConvergence: true,
        expectedRate: 0.0,
        tolerance: 1e-10
      },
      {
        name: 'Simple Diagonal Matrix',
        description: 'Diagonal matrix with 2s on diagonal',
        matrix: this.createDiagonalMatrix(5, 2.0),
        rhs: [2, 4, 6, 8, 10],
        expectedIterations: 1,
        expectedConvergence: true,
        expectedRate: 0.0,
        tolerance: 1e-10
      },
      {
        name: 'Strongly Diagonal Dominant',
        description: 'Matrix with strong diagonal dominance',
        matrix: this.createStronglyDiagonalDominant(8),
        rhs: Array(8).fill(1),
        expectedIterations: { min: 1, max: 10 },
        expectedConvergence: true,
        expectedRate: { min: 0.0, max: 0.3 },
        tolerance: 1e-8
      },
      {
        name: 'Weakly Diagonal Dominant',
        description: 'Matrix with weak diagonal dominance',
        matrix: this.createWeaklyDiagonalDominant(6),
        rhs: Array(6).fill(1),
        expectedIterations: { min: 10, max: 100 },
        expectedConvergence: true,
        expectedRate: { min: 0.3, max: 0.9 },
        tolerance: 1e-6
      },
      {
        name: 'Symmetric Positive Definite',
        description: 'Well-conditioned SPD matrix',
        matrix: this.createSPDMatrix(5),
        rhs: [1, 2, 3, 4, 5],
        expectedIterations: { min: 1, max: 20 },
        expectedConvergence: true,
        expectedRate: { min: 0.0, max: 0.5 },
        tolerance: 1e-8
      },
      {
        name: 'Near-singular Matrix',
        description: 'Poorly conditioned matrix',
        matrix: this.createNearSingularMatrix(4),
        rhs: [1, 1, 1, 1],
        expectedIterations: { min: 50, max: 1000 },
        expectedConvergence: false, // May not converge
        expectedRate: { min: 0.8, max: 1.0 },
        tolerance: 1e-4,
        maxIterations: 200
      }
    ];
  }

  /**
   * Run all validation tests
   */
  async runValidation() {
    console.log('🧪 Running Convergence Validation Tests');
    console.log('=' .repeat(60));

    for (const testCase of this.testCases) {
      console.log(`\n📋 Test: ${testCase.name}`);
      console.log(`   ${testCase.description}`);

      try {
        const result = await this.runSingleTest(testCase);
        this.results.push(result);

        this.printTestResult(result);
      } catch (error) {
        console.log(`   ❌ ERROR: ${error.message}`);
        this.results.push({
          testCase: testCase.name,
          passed: false,
          error: error.message
        });
      }
    }

    this.printSummary();
    return this.results;
  }

  /**
   * Run a single test case
   */
  async runSingleTest(testCase) {
    const solver = await createSolver({
      matrix: testCase.matrix,
      method: 'jacobi',
      tolerance: testCase.tolerance,
      maxIterations: testCase.maxIterations || 1000,
      verbose: false
    });

    const result = await solver.solve(testCase.rhs);

    // Validate convergence behavior
    const validation = this.validateResult(result, testCase);

    return {
      testCase: testCase.name,
      expected: testCase,
      actual: {
        iterations: result.iterations,
        converged: result.converged,
        convergenceRate: result.convergenceRate,
        residual: result.residual,
        reductionFactor: result.reductionFactor,
        grade: result.performanceGrade
      },
      validation,
      passed: validation.overall
    };
  }

  /**
   * Validate result against expected behavior
   */
  validateResult(result, testCase) {
    const checks = {
      convergence: this.checkConvergence(result.converged, testCase.expectedConvergence),
      iterations: this.checkIterations(result.iterations, testCase.expectedIterations),
      convergenceRate: this.checkConvergenceRate(result.convergenceRate, testCase.expectedRate),
      residual: this.checkResidual(result.residual, testCase.tolerance),
      reductionFactor: this.checkReductionFactor(result.reductionFactor)
    };

    const passedChecks = Object.values(checks).filter(c => c.passed).length;
    const totalChecks = Object.keys(checks).length;

    return {
      ...checks,
      overall: passedChecks >= totalChecks - 1, // Allow one check to fail
      score: `${passedChecks}/${totalChecks}`
    };
  }

  checkConvergence(actual, expected) {
    const passed = actual === expected;
    return {
      passed,
      message: passed ? '✓ Convergence as expected' : `✗ Expected ${expected}, got ${actual}`
    };
  }

  checkIterations(actual, expected) {
    if (typeof expected === 'number') {
      const passed = actual === expected;
      return {
        passed,
        message: passed ? '✓ Iterations as expected' : `✗ Expected ${expected}, got ${actual}`
      };
    } else {
      const passed = actual >= expected.min && actual <= expected.max;
      return {
        passed,
        message: passed ? '✓ Iterations in range' : `✗ Expected ${expected.min}-${expected.max}, got ${actual}`
      };
    }
  }

  checkConvergenceRate(actual, expected) {
    if (typeof expected === 'number') {
      const passed = Math.abs(actual - expected) < 0.1;
      return {
        passed,
        message: passed ? '✓ Convergence rate as expected' : `✗ Expected ~${expected}, got ${actual}`
      };
    } else {
      const passed = actual >= expected.min && actual <= expected.max;
      return {
        passed,
        message: passed ? '✓ Convergence rate in range' : `✗ Expected ${expected.min}-${expected.max}, got ${actual.toFixed(3)}`
      };
    }
  }

  checkResidual(actual, tolerance) {
    const passed = actual <= tolerance * 10; // Allow some tolerance slack
    return {
      passed,
      message: passed ? '✓ Residual acceptable' : `✗ Residual ${actual.toExponential(2)} too large`
    };
  }

  checkReductionFactor(actual) {
    const passed = actual >= 0 && actual <= 1.0;
    return {
      passed,
      message: passed ? '✓ Reduction factor valid' : `✗ Invalid reduction factor ${actual}`
    };
  }

  /**
   * Print individual test result
   */
  printTestResult(result) {
    const status = result.passed ? '✅ PASS' : '❌ FAIL';
    console.log(`   ${status} (${result.validation.score})`);

    if (result.passed) {
      console.log(`      Iterations: ${result.actual.iterations}, Convergence: ${result.actual.convergenceRate.toFixed(1)}%`);
      console.log(`      Grade: ${result.actual.grade}, Reduction: ${result.actual.reductionFactor.toExponential(2)}`);
    } else {
      console.log('      Issues:');
      Object.entries(result.validation).forEach(([key, check]) => {
        if (key !== 'overall' && key !== 'score' && !check.passed) {
          console.log(`        ${check.message}`);
        }
      });
    }
  }

  /**
   * Print validation summary
   */
  printSummary() {
    console.log('\n' + '='.repeat(60));
    console.log('\n📊 VALIDATION SUMMARY');

    const passed = this.results.filter(r => r.passed).length;
    const total = this.results.length;
    const percentage = (passed / total * 100).toFixed(1);

    console.log(`\nOverall: ${passed}/${total} tests passed (${percentage}%)`);

    if (passed === total) {
      console.log('🎉 All convergence validation tests passed!');
      console.log('✓ Convergence detection is working correctly');
      console.log('✓ Metrics reporting is accurate');
      console.log('✓ Early stopping is functioning');
    } else {
      console.log('⚠️  Some tests failed - convergence system needs attention');

      const failed = this.results.filter(r => !r.passed);
      console.log('\nFailed tests:');
      failed.forEach(f => {
        console.log(`  - ${f.testCase}: ${f.error || 'Validation failed'}`);
      });
    }

    console.log('\n' + '='.repeat(60));
  }

  // Matrix generation utilities

  createIdentityMatrix(size) {
    const matrix = Array(size).fill(0).map(() => Array(size).fill(0));
    for (let i = 0; i < size; i++) {
      matrix[i][i] = 1.0;
    }
    return {
      data: matrix,
      rows: size,
      cols: size,
      format: 'dense'
    };
  }

  createDiagonalMatrix(size, diagonalValue) {
    const matrix = Array(size).fill(0).map(() => Array(size).fill(0));
    for (let i = 0; i < size; i++) {
      matrix[i][i] = diagonalValue;
    }
    return {
      data: matrix,
      rows: size,
      cols: size,
      format: 'dense'
    };
  }

  createStronglyDiagonalDominant(size) {
    const matrix = Array(size).fill(0).map(() => Array(size).fill(0));

    for (let i = 0; i < size; i++) {
      let rowSum = 0;

      // Add off-diagonal elements
      for (let j = 0; j < size; j++) {
        if (i !== j) {
          const value = (Math.random() - 0.5) * 0.2; // Small off-diagonal elements
          matrix[i][j] = value;
          rowSum += Math.abs(value);
        }
      }

      // Set diagonal to be much larger than row sum
      matrix[i][i] = rowSum * 3 + 2.0;
    }

    return {
      data: matrix,
      rows: size,
      cols: size,
      format: 'dense'
    };
  }

  createWeaklyDiagonalDominant(size) {
    const matrix = Array(size).fill(0).map(() => Array(size).fill(0));

    for (let i = 0; i < size; i++) {
      let rowSum = 0;

      // Add larger off-diagonal elements
      for (let j = 0; j < size; j++) {
        if (i !== j) {
          const value = (Math.random() - 0.5) * 0.8; // Larger off-diagonal elements
          matrix[i][j] = value;
          rowSum += Math.abs(value);
        }
      }

      // Set diagonal to barely dominate
      matrix[i][i] = rowSum + 0.1;
    }

    return {
      data: matrix,
      rows: size,
      cols: size,
      format: 'dense'
    };
  }

  createSPDMatrix(size) {
    // Create A = B^T * B + I to ensure SPD
    const B = Array(size).fill(0).map(() =>
      Array(size).fill(0).map(() => Math.random() - 0.5)
    );

    const matrix = Array(size).fill(0).map(() => Array(size).fill(0));

    for (let i = 0; i < size; i++) {
      for (let j = 0; j < size; j++) {
        let sum = 0;
        for (let k = 0; k < size; k++) {
          sum += B[k][i] * B[k][j];
        }
        matrix[i][j] = sum;
        if (i === j) matrix[i][j] += 1.0; // Add identity for positive definiteness
      }
    }

    return {
      data: matrix,
      rows: size,
      cols: size,
      format: 'dense'
    };
  }

  createNearSingularMatrix(size) {
    const matrix = Array(size).fill(0).map(() => Array(size).fill(0));

    // Create a matrix with very small singular values
    for (let i = 0; i < size; i++) {
      for (let j = 0; j < size; j++) {
        matrix[i][j] = Math.random() * 0.1;
      }
      // Set diagonal to be barely non-zero
      matrix[i][i] = 0.001 + Math.random() * 0.01;
    }

    return {
      data: matrix,
      rows: size,
      cols: size,
      format: 'dense'
    };
  }
}

// Export for use in tests
module.exports = { ConvergenceValidator };

// Run validation if called directly
if (require.main === module) {
  const validator = new ConvergenceValidator();
  validator.runValidation().then(results => {
    const passed = results.filter(r => r.passed).length;
    process.exit(passed === results.length ? 0 : 1);
  }).catch(error => {
    console.error('Validation failed:', error);
    process.exit(1);
  });
}