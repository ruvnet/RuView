const path = require('path');
const { ConvergenceDetector } = require('./convergence/convergence-detector');
const { MetricsReporter } = require('./convergence/metrics-reporter');
const { MatrixUtils } = require('./utils/matrix-utils');

// Import the existing WASM solver when available, fallback to stub
let WasmSolver;
let wasmAvailable = false;

try {
  // Try to import the WASM solver
  const wasmPath = path.join(__dirname, '../js/solver.js');
  // Note: This would need to be adjusted for actual WASM module loading in Node.js
  // For now, we'll create a compatible interface
  wasmAvailable = false; // Disable for CLI demo
} catch (error) {
  console.warn('WASM solver not available, using JavaScript fallback');
  wasmAvailable = false;
}

/**
 * JavaScript fallback solver for CLI and server use
 */
class JSSolver {
  constructor(config = {}) {
    this.config = {
      matrix: config.matrix,
      method: config.method || 'jacobi',
      tolerance: config.tolerance || 1e-10,
      maxIterations: config.maxIterations || 1000,
      enableVerification: config.enableVerification || false,
      verbose: config.verbose || false,
      ...config
    };

    this.initialized = false;
    this.currentSolution = null;
    this.currentIteration = 0;
    this.currentResidual = Infinity;
    this.converged = false;

    // Initialize convergence detection and metrics
    this.convergenceDetector = new ConvergenceDetector({
      tolerance: this.config.tolerance,
      maxIterations: this.config.maxIterations,
      relativeToleranceEnabled: this.config.relativeToleranceEnabled !== false
    });

    this.metricsReporter = new MetricsReporter({
      verbose: this.config.verbose,
      enableProfiling: true
    });
  }

  async initialize() {
    if (this.initialized) return;

    // Validate matrix and setup solver
    this.validateMatrix(this.config.matrix);
    this.initialized = true;
  }

  validateMatrix(matrix) {
    if (!matrix) {
      throw new Error('Matrix is required');
    }

    if (!matrix.rows || !matrix.cols) {
      throw new Error('Matrix must have rows and cols properties');
    }

    if (!matrix.data) {
      throw new Error('Matrix must have data property');
    }

    // Validate matrix format
    if (matrix.format === 'dense') {
      if (!Array.isArray(matrix.data) || matrix.data.length !== matrix.rows) {
        throw new Error('Dense matrix data must be array of rows');
      }
    } else if (matrix.format === 'coo') {
      if (!matrix.data.values || !matrix.data.rowIndices || !matrix.data.colIndices) {
        throw new Error('COO matrix must have values, rowIndices, and colIndices');
      }
    } else {
      throw new Error(`Unsupported matrix format: ${matrix.format}`);
    }

    // Enhanced diagonal validation
    const diagonalValidation = MatrixUtils.validateDiagonalElements(matrix);

    if (!diagonalValidation.valid) {
      const issues = [];

      if (diagonalValidation.missingDiagonals.length > 0) {
        issues.push(`Missing diagonal elements at rows: ${diagonalValidation.missingDiagonals.join(', ')}`);
      }

      if (diagonalValidation.smallDiagonals.length > 0) {
        const smallDiagInfo = diagonalValidation.smallDiagonals
          .map(d => `row ${d.index}: ${d.value}`)
          .join(', ');
        issues.push(`Near-zero diagonal elements: ${smallDiagInfo}`);
      }

      // Auto-fix option for diagonal issues
      if (this.config.autoFixMatrix !== false) {
        if (this.config.verbose) {
          console.warn('Matrix has diagonal issues, attempting auto-fix...');
          console.warn('Issues found:', issues.join('; '));
        }

        try {
          const fixResult = MatrixUtils.ensureDiagonalDominance(matrix, {
            strategy: this.config.diagonalStrategy || 'rowsum_plus_one',
            verbose: this.config.verbose
          });

          // Update the matrix in config with the fixed version
          this.config.matrix = fixResult.matrix;

          if (this.config.verbose) {
            console.log(`Applied ${fixResult.fixes.length} diagonal fixes`);
          }

          return; // Matrix is now valid
        } catch (fixError) {
          if (this.config.verbose) {
            console.error('Auto-fix failed:', fixError.message);
          }
        }
      }

      throw new Error(`Matrix validation failed: ${issues.join('; ')}. ` +
        `Set config.autoFixMatrix=true to enable automatic fixes.`);
    }

    // Check conditioning
    const conditioning = MatrixUtils.analyzeConditioning(matrix);

    if (this.config.verbose && conditioning.conditioningGrade !== 'A') {
      console.warn(`Matrix conditioning grade: ${conditioning.conditioningGrade}`);
      console.warn('Recommendations:', conditioning.recommendations.join(', '));

      if (!conditioning.isDiagonallyDominant) {
        console.warn(`Diagonal dominance ratio: ${conditioning.diagonalDominanceRatio.toFixed(2)} (should be ≤ 1.0)`);
      }
    }

    // Store conditioning info for solver optimization
    this.matrixConditioning = conditioning;

    // Check if matrix is symmetric (important for CG)
    this.isSymmetric = MatrixUtils.isSymmetric(matrix);

    if (this.config.verbose) {
      console.log(`Matrix symmetry: ${this.isSymmetric ? 'Yes' : 'No'}`);
      if (!this.isSymmetric && this.config.method === 'conjugate-gradient') {
        console.warn('Warning: Conjugate Gradient works best with symmetric positive definite matrices');
      }
    }
  }

  async solve(vector, options = {}) {
    await this.initialize();

    const onProgress = options.onProgress || (() => {});
    const signal = options.signal;

    // Reset convergence detector and start metrics collection
    this.convergenceDetector.reset();
    this.convergenceDetector.initialize(vector);

    this.metricsReporter.startSolve(this.config, {
      rows: this.config.matrix.rows,
      cols: this.config.matrix.cols,
      format: this.config.matrix.format || 'dense'
    });

    // Initialize solution vector
    this.currentSolution = new Array(vector.length).fill(0);
    this.currentIteration = 0;
    this.currentResidual = Infinity;
    this.converged = false;

    const startTime = Date.now();

    try {
      // Choose solver method
      switch (this.config.method) {
        case 'jacobi':
          await this.solveJacobi(vector, onProgress, signal);
          break;
        case 'gauss-seidel':
        case 'gauss_seidel':
          await this.solveGaussSeidel(vector, onProgress, signal);
          break;
        case 'conjugate-gradient':
        case 'cg':
          await this.solveConjugateGradient(vector, onProgress, signal);
          break;
        case 'hybrid':
        case 'adaptive':
        default:
          await this.solveAdaptive(vector, onProgress, signal);
          break;
      }

      // Generate final report
      const report = this.metricsReporter.finalizeSolve(this.convergenceDetector, this.currentSolution);
      const elapsed = Date.now() - startTime;

      return {
        values: this.currentSolution,
        iterations: this.currentIteration,
        residual: this.currentResidual,
        converged: this.converged,
        solveTime: elapsed,
        method: this.config.method,
        memoryUsage: this.getMemoryUsage(),

        // Enhanced metrics
        convergenceReport: report,
        convergenceRate: report.convergence.convergenceRatePercent,
        reductionFactor: report.convergence.reductionFactor,
        performanceGrade: report.performance.grade
      };

    } catch (error) {
      throw new Error(`Solve failed: ${error.message}`);
    }
  }

  async *streamSolve(vector) {
    await this.initialize();

    // Initialize solution vector
    this.currentSolution = new Array(vector.length).fill(0);
    this.currentIteration = 0;
    this.currentResidual = Infinity;
    this.converged = false;

    const startTime = Date.now();

    try {
      // Stream solver based on method
      switch (this.config.method) {
        case 'jacobi':
          yield* this.streamJacobi(vector);
          break;
        case 'gauss-seidel':
        case 'gauss_seidel':
          yield* this.streamGaussSeidel(vector);
          break;
        case 'conjugate-gradient':
        case 'cg':
          yield* this.streamConjugateGradient(vector);
          break;
        case 'hybrid':
        case 'adaptive':
        default:
          yield* this.streamAdaptive(vector);
          break;
      }

    } catch (error) {
      yield {
        error: error.message,
        iteration: this.currentIteration,
        timestamp: new Date().toISOString()
      };
    }
  }

  async *streamJacobi(vector) {
    const matrix = this.config.matrix;
    const n = vector.length;
    let x = new Array(n).fill(0);
    let xNew = new Array(n).fill(0);

    for (let iter = 0; iter < this.config.maxIterations; iter++) {
      this.currentIteration = iter;

      // Jacobi iteration
      for (let i = 0; i < n; i++) {
        let sum = 0;
        let diagonal = 0;

        if (matrix.format === 'dense') {
          for (let j = 0; j < n; j++) {
            if (i !== j) {
              sum += matrix.data[i][j] * x[j];
            } else {
              diagonal = matrix.data[i][j];
            }
          }
        } else if (matrix.format === 'coo') {
          // Coordinate format
          for (let k = 0; k < matrix.data.values.length; k++) {
            const row = matrix.data.rowIndices[k];
            const col = matrix.data.colIndices[k];
            const val = matrix.data.values[k];

            if (row === i) {
              if (col !== i) {
                sum += val * x[col];
              } else {
                diagonal = val;
              }
            }
          }
        }

        if (Math.abs(diagonal) < 1e-14) {
          throw new Error(`Zero diagonal element at position ${i}`);
        }

        xNew[i] = (vector[i] - sum) / diagonal;
      }

      // Update solution
      x = [...xNew];
      this.currentSolution = x;

      // Use convergence detector for proper metrics
      const convergenceMetrics = this.convergenceDetector.update(matrix, x, vector);
      this.currentIteration = convergenceMetrics.iteration;
      this.currentResidual = convergenceMetrics.relativeResidualNorm;
      this.converged = convergenceMetrics.isConverged;

      // Record metrics
      const iterationMetrics = this.metricsReporter.recordIteration(convergenceMetrics);

      // Yield progress with enhanced metrics
      yield {
        iteration: iter,
        residual: convergenceMetrics.relativeResidualNorm,
        residualNorm: convergenceMetrics.residualNorm,
        convergenceRate: convergenceMetrics.convergenceRate,
        convergenceRatePercent: (1 - convergenceMetrics.convergenceRate) * 100,
        reductionFactor: convergenceMetrics.reductionFactor,
        memoryUsage: this.getMemoryUsage(),
        converged: this.converged,
        shouldStop: convergenceMetrics.shouldStop,
        estimatedIterationsRemaining: convergenceMetrics.estimatedIterationsRemaining,
        solution: this.converged ? x : undefined,
        verified: this.config.enableVerification ? await this.verify(x, vector) : undefined
      };

      // Early stopping when convergence criteria met
      if (convergenceMetrics.shouldStop) {
        break;
      }

      // Allow other operations to run
      await new Promise(resolve => setImmediate(resolve));
    }
  }

  async *streamGaussSeidel(vector) {
    const matrix = this.config.matrix;
    const n = vector.length;
    let x = new Array(n).fill(0);

    for (let iter = 0; iter < this.config.maxIterations; iter++) {
      this.currentIteration = iter;

      // Gauss-Seidel iteration
      for (let i = 0; i < n; i++) {
        let sum = 0;
        let diagonal = 0;

        if (matrix.format === 'dense') {
          for (let j = 0; j < n; j++) {
            if (i !== j) {
              sum += matrix.data[i][j] * x[j];
            } else {
              diagonal = matrix.data[i][j];
            }
          }
        } else if (matrix.format === 'coo') {
          for (let k = 0; k < matrix.data.values.length; k++) {
            const row = matrix.data.rowIndices[k];
            const col = matrix.data.colIndices[k];
            const val = matrix.data.values[k];

            if (row === i) {
              if (col !== i) {
                sum += val * x[col];
              } else {
                diagonal = val;
              }
            }
          }
        }

        if (Math.abs(diagonal) < 1e-14) {
          throw new Error(`Zero diagonal element at position ${i}`);
        }

        x[i] = (vector[i] - sum) / diagonal;
      }

      // Compute residual
      const residual = this.computeResidual(matrix, x, vector);
      this.currentResidual = this.vectorNorm(residual);

      // Convergence check
      this.converged = this.currentResidual < this.config.tolerance;

      // Update solution
      this.currentSolution = [...x];

      // Yield progress
      yield {
        iteration: iter,
        residual: this.currentResidual,
        convergenceRate: iter > 0 ? this.currentResidual / this.previousResidual : 1.0,
        memoryUsage: this.getMemoryUsage(),
        converged: this.converged,
        solution: this.converged ? x : undefined,
        verified: this.config.enableVerification ? await this.verify(x, vector) : undefined
      };

      this.previousResidual = this.currentResidual;

      if (this.converged) {
        break;
      }

      await new Promise(resolve => setImmediate(resolve));
    }
  }

  async *streamConjugateGradient(vector) {
    const matrix = this.config.matrix;
    const n = vector.length;
    let x = new Array(n).fill(0);

    // CG initialization: r = b - Ax (initial residual)
    let r = [...vector]; // Since x starts at 0, r = b - A*0 = b
    let p = [...r];      // Initial search direction
    let rsold = this.dotProduct(r, r);

    for (let iter = 0; iter < this.config.maxIterations; iter++) {
      this.currentIteration = iter;

      // Compute Ap (matrix-vector product)
      const Ap = this.multiplyMatrixVector(matrix, p);
      const pAp = this.dotProduct(p, Ap);

      // Check for non-positive curvature (matrix not positive definite)
      if (pAp <= 1e-16) {
        if (this.config.verbose) {
          console.warn(`CG: Non-positive curvature detected at iteration ${iter}, switching to steepest descent`);
        }
        // Fall back to steepest descent step
        const rAr = this.dotProduct(r, this.multiplyMatrixVector(matrix, r));
        if (rAr > 1e-16) {
          const alpha = rsold / rAr;
          for (let i = 0; i < n; i++) {
            x[i] += alpha * r[i];
          }
        }
        break;
      }

      // CG step size
      const alpha = rsold / pAp;

      // Update solution: x = x + alpha * p
      for (let i = 0; i < n; i++) {
        x[i] += alpha * p[i];
      }

      // Update residual: r = r - alpha * Ap
      for (let i = 0; i < n; i++) {
        r[i] -= alpha * Ap[i];
      }

      const rsnew = this.dotProduct(r, r);
      this.currentResidual = Math.sqrt(rsnew);

      // Update solution
      this.currentSolution = [...x];

      // Use convergence detector for proper metrics
      const convergenceMetrics = this.convergenceDetector.update(matrix, x, vector);
      this.currentIteration = convergenceMetrics.iteration;
      this.currentResidual = convergenceMetrics.relativeResidualNorm;
      this.converged = convergenceMetrics.isConverged;

      // Record metrics
      const iterationMetrics = this.metricsReporter.recordIteration(convergenceMetrics);

      // Yield progress with enhanced metrics
      yield {
        iteration: iter,
        residual: convergenceMetrics.relativeResidualNorm,
        residualNorm: convergenceMetrics.residualNorm,
        convergenceRate: convergenceMetrics.convergenceRate,
        convergenceRatePercent: (1 - convergenceMetrics.convergenceRate) * 100,
        reductionFactor: convergenceMetrics.reductionFactor,
        memoryUsage: this.getMemoryUsage(),
        converged: this.converged,
        shouldStop: convergenceMetrics.shouldStop,
        estimatedIterationsRemaining: convergenceMetrics.estimatedIterationsRemaining,
        solution: this.converged ? x : undefined,
        verified: this.config.enableVerification ? await this.verify(x, vector) : undefined
      };

      // Early stopping when convergence criteria met
      if (convergenceMetrics.shouldStop) {
        break;
      }

      // Check for stagnation
      if (rsnew > rsold * 0.999) {
        if (this.config.verbose) {
          console.warn(`CG: Convergence stagnation detected at iteration ${iter}`);
        }
      }

      // Update search direction: p = r + beta * p
      const beta = rsnew / rsold;
      for (let i = 0; i < n; i++) {
        p[i] = r[i] + beta * p[i];
      }

      rsold = rsnew;
      await new Promise(resolve => setImmediate(resolve));
    }
  }

  async *streamAdaptive(vector) {
    // Start with Jacobi, switch to CG if convergence is slow
    let method = 'jacobi';
    let slowConvergence = false;
    let previousResiduals = [];

    const jacobiSolver = new JSSolver({
      ...this.config,
      method: 'jacobi'
    });

    await jacobiSolver.initialize();

    for await (const update of jacobiSolver.streamSolve(vector)) {
      previousResiduals.push(update.residual);

      // Check for slow convergence after 50 iterations
      if (update.iteration > 50 && !slowConvergence) {
        const recent = previousResiduals.slice(-10);
        const improvement = recent[0] / recent[recent.length - 1];

        if (improvement < 1.1) { // Less than 10% improvement
          slowConvergence = true;
          method = 'conjugate-gradient';

          // Switch to CG
          const cgSolver = new JSSolver({
            ...this.config,
            method: 'conjugate-gradient'
          });

          await cgSolver.initialize();

          // Continue with CG from current solution
          const remainingVector = vector; // In practice, would adjust for current progress

          for await (const cgUpdate of cgSolver.streamSolve(remainingVector)) {
            yield {
              ...cgUpdate,
              iteration: update.iteration + cgUpdate.iteration,
              method: 'adaptive-cg'
            };

            if (cgUpdate.converged) {
              return;
            }
          }
          return;
        }
      }

      yield {
        ...update,
        method: 'adaptive-jacobi'
      };

      if (update.converged) {
        return;
      }
    }
  }

  // Non-streaming versions
  async solveJacobi(vector, onProgress, signal) {
    for await (const update of this.streamJacobi(vector)) {
      if (signal && signal.aborted) {
        throw new Error('Solve aborted');
      }

      onProgress(update);

      if (update.converged) {
        break;
      }
    }
  }

  async solveGaussSeidel(vector, onProgress, signal) {
    for await (const update of this.streamGaussSeidel(vector)) {
      if (signal && signal.aborted) {
        throw new Error('Solve aborted');
      }

      onProgress(update);

      if (update.converged) {
        break;
      }
    }
  }

  async solveConjugateGradient(vector, onProgress, signal) {
    for await (const update of this.streamConjugateGradient(vector)) {
      if (signal && signal.aborted) {
        throw new Error('Solve aborted');
      }

      onProgress(update);

      if (update.converged) {
        break;
      }
    }
  }

  async solveAdaptive(vector, onProgress, signal) {
    for await (const update of this.streamAdaptive(vector)) {
      if (signal && signal.aborted) {
        throw new Error('Solve aborted');
      }

      onProgress(update);

      if (update.converged) {
        break;
      }
    }
  }

  // Utility methods
  computeResidual(matrix, x, b) {
    const Ax = this.multiplyMatrixVector(matrix, x);
    return Ax.map((val, i) => b[i] - val);
  }

  multiplyMatrixVector(matrix, vector) {
    const result = new Array(matrix.rows).fill(0);

    if (matrix.format === 'dense') {
      for (let i = 0; i < matrix.rows; i++) {
        for (let j = 0; j < matrix.cols; j++) {
          result[i] += matrix.data[i][j] * vector[j];
        }
      }
    } else if (matrix.format === 'coo') {
      for (let k = 0; k < matrix.data.values.length; k++) {
        const row = matrix.data.rowIndices[k];
        const col = matrix.data.colIndices[k];
        const val = matrix.data.values[k];
        result[row] += val * vector[col];
      }
    }

    return result;
  }

  vectorNorm(vector) {
    return Math.sqrt(vector.reduce((sum, val) => sum + val * val, 0));
  }

  dotProduct(a, b) {
    return a.reduce((sum, val, i) => sum + val * b[i], 0);
  }

  async verify(solution, vector) {
    if (!this.config.enableVerification) {
      return { verified: true };
    }

    // Simple verification: check residual
    const residual = this.computeResidual(this.config.matrix, solution, vector);
    const residualNorm = this.vectorNorm(residual);

    return {
      verified: residualNorm < this.config.tolerance * 10,
      residualNorm,
      maxError: Math.max(...residual.map(Math.abs))
    };
  }

  getMemoryUsage() {
    const memUsage = process.memoryUsage();
    return Math.round(memUsage.heapUsed / 1024 / 1024); // MB
  }

  stop() {
    // For stopping streaming solves
    this.stopped = true;
  }
}

/**
 * Factory function to create solver instances
 */
async function createSolver(config = {}) {
  const solver = wasmAvailable
    ? new WasmSolver(config)
    : new JSSolver(config);

  await solver.initialize();
  return solver;
}

module.exports = {
  createSolver,
  JSSolver,
  WasmSolver: wasmAvailable ? WasmSolver : JSSolver
};