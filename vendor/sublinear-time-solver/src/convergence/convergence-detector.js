/**
 * Advanced Convergence Detection and Metrics System
 *
 * Provides proper residual norm calculation, convergence rate tracking,
 * and early stopping mechanisms for iterative solvers.
 */

class ConvergenceDetector {
  constructor(options = {}) {
    this.tolerance = options.tolerance || 1e-10;
    this.maxIterations = options.maxIterations || 1000;
    this.relativeToleranceEnabled = options.relativeToleranceEnabled !== false;
    this.minIterations = options.minIterations || 1;
    this.stagnationThreshold = options.stagnationThreshold || 1e-14;
    this.convergenceWindowSize = options.convergenceWindowSize || 10;

    // State tracking
    this.reset();
  }

  reset() {
    this.iteration = 0;
    this.residualHistory = [];
    this.convergenceRateHistory = [];
    this.relativeResidualHistory = [];
    this.initialResidualNorm = null;
    this.rhsNorm = null;
    this.isConverged = false;
    this.stagnationDetected = false;
    this.divergenceDetected = false;
    this.startTime = Date.now();
    this.lastUpdateTime = Date.now();
  }

  /**
   * Initialize with the right-hand side vector for relative residual calculation
   * @param {Array<number>} rhs - Right-hand side vector b
   */
  initialize(rhs) {
    this.rhsNorm = this.vectorNorm(rhs);
    if (this.rhsNorm === 0) {
      console.warn('Zero RHS vector detected - using absolute residual tolerance');
      this.relativeToleranceEnabled = false;
    }
  }

  /**
   * Compute proper residual: r = b - Ax
   * @param {Object} matrix - Matrix A in supported format
   * @param {Array<number>} solution - Current solution vector x
   * @param {Array<number>} rhs - Right-hand side vector b
   * @returns {Array<number>} - Residual vector
   */
  computeResidual(matrix, solution, rhs) {
    const Ax = this.multiplyMatrixVector(matrix, solution);
    return rhs.map((bi, i) => bi - Ax[i]);
  }

  /**
   * Compute relative residual norm: ||r|| / ||b||
   * @param {Array<number>} residual - Residual vector
   * @returns {number} - Relative residual norm
   */
  computeRelativeResidualNorm(residual) {
    const residualNorm = this.vectorNorm(residual);

    if (this.relativeToleranceEnabled && this.rhsNorm > 0) {
      return residualNorm / this.rhsNorm;
    } else {
      return residualNorm;
    }
  }

  /**
   * Update convergence state with new iteration data
   * @param {Object} matrix - Matrix A
   * @param {Array<number>} solution - Current solution x
   * @param {Array<number>} rhs - Right-hand side b
   * @returns {Object} - Convergence metrics
   */
  update(matrix, solution, rhs) {
    this.iteration++;
    this.lastUpdateTime = Date.now();

    // Compute residual and norms
    const residual = this.computeResidual(matrix, solution, rhs);
    const residualNorm = this.vectorNorm(residual);
    const relativeResidualNorm = this.computeRelativeResidualNorm(residual);

    // Store history
    this.residualHistory.push(residualNorm);
    this.relativeResidualHistory.push(relativeResidualNorm);

    // Set initial residual for convergence rate calculation
    if (this.iteration === 1) {
      this.initialResidualNorm = relativeResidualNorm;
    }

    // Compute convergence rate
    const convergenceRate = this.computeConvergenceRate();
    this.convergenceRateHistory.push(convergenceRate);

    // Check convergence conditions
    this.checkConvergence(relativeResidualNorm);
    this.checkStagnation();
    this.checkDivergence();

    const metrics = {
      iteration: this.iteration,
      residualNorm: residualNorm,
      relativeResidualNorm: relativeResidualNorm,
      convergenceRate: convergenceRate,
      isConverged: this.isConverged,
      stagnationDetected: this.stagnationDetected,
      divergenceDetected: this.divergenceDetected,
      shouldStop: this.shouldStop(),
      reductionFactor: this.getReductionFactor(),
      estimatedIterationsRemaining: this.estimateIterationsRemaining(),
      elapsedTime: this.lastUpdateTime - this.startTime,
      iterationsPerSecond: this.iteration / ((this.lastUpdateTime - this.startTime) / 1000)
    };

    return metrics;
  }

  /**
   * Compute logarithmic convergence rate: log(r_k / r_{k-1})
   * Uses averaging over recent iterations for stability
   */
  computeConvergenceRate() {
    if (this.relativeResidualHistory.length < 2) {
      return 0.0;
    }

    const current = this.relativeResidualHistory[this.relativeResidualHistory.length - 1];
    const previous = this.relativeResidualHistory[this.relativeResidualHistory.length - 2];

    if (previous === 0 || current === 0) {
      return 0.0;
    }

    // Single-step convergence rate
    const singleStepRate = current / previous;

    // Average convergence rate over recent iterations
    if (this.relativeResidualHistory.length >= this.convergenceWindowSize) {
      const windowStart = this.relativeResidualHistory.length - this.convergenceWindowSize;
      const windowEnd = this.relativeResidualHistory.length - 1;

      const initialWindow = this.relativeResidualHistory[windowStart];
      const finalWindow = this.relativeResidualHistory[windowEnd];

      if (initialWindow > 0 && finalWindow > 0) {
        const averageRate = Math.pow(finalWindow / initialWindow, 1.0 / (this.convergenceWindowSize - 1));
        return averageRate;
      }
    }

    return singleStepRate;
  }

  /**
   * Check if convergence criteria are met
   */
  checkConvergence(relativeResidualNorm) {
    if (this.iteration < this.minIterations) {
      this.isConverged = false;
      return;
    }

    this.isConverged = relativeResidualNorm < this.tolerance;
  }

  /**
   * Detect if iteration is stagnating
   */
  checkStagnation() {
    if (this.residualHistory.length < this.convergenceWindowSize) {
      return;
    }

    const recentResiduals = this.residualHistory.slice(-this.convergenceWindowSize);
    const maxRecent = Math.max(...recentResiduals);
    const minRecent = Math.min(...recentResiduals);

    // Check if residual has barely changed
    if (maxRecent > 0 && (maxRecent - minRecent) / maxRecent < this.stagnationThreshold) {
      this.stagnationDetected = true;
    }
  }

  /**
   * Detect if iteration is diverging
   */
  checkDivergence() {
    if (this.residualHistory.length < 5) {
      return;
    }

    const current = this.residualHistory[this.residualHistory.length - 1];
    const previous = this.residualHistory[this.residualHistory.length - 2];
    const initial = this.residualHistory[0];

    // Check for explosive growth
    if (current > 1000 * initial || (previous > 0 && current / previous > 10)) {
      this.divergenceDetected = true;
    }
  }

  /**
   * Determine if solver should stop
   */
  shouldStop() {
    return this.isConverged ||
           this.iteration >= this.maxIterations ||
           this.stagnationDetected ||
           this.divergenceDetected;
  }

  /**
   * Get overall reduction factor from initial residual
   */
  getReductionFactor() {
    if (this.initialResidualNorm === null || this.initialResidualNorm === 0) {
      return 1.0;
    }

    const current = this.relativeResidualHistory[this.relativeResidualHistory.length - 1] || 0;
    return current / this.initialResidualNorm;
  }

  /**
   * Estimate iterations remaining based on convergence rate
   */
  estimateIterationsRemaining() {
    if (this.isConverged) {
      return 0;
    }

    const currentResidual = this.relativeResidualHistory[this.relativeResidualHistory.length - 1];
    const convergenceRate = this.convergenceRateHistory[this.convergenceRateHistory.length - 1];

    if (!currentResidual || !convergenceRate || convergenceRate >= 1.0 || convergenceRate <= 0) {
      return this.maxIterations - this.iteration;
    }

    // Estimate iterations to reach tolerance: n = log(tol/current) / log(rate)
    const iterationsNeeded = Math.log(this.tolerance / currentResidual) / Math.log(convergenceRate);

    return Math.max(0, Math.min(iterationsNeeded, this.maxIterations - this.iteration));
  }

  /**
   * Get comprehensive convergence report
   */
  getConvergenceReport() {
    const current = this.relativeResidualHistory[this.relativeResidualHistory.length - 1] || 0;
    const avgConvergenceRate = this.convergenceRateHistory.length > 0
      ? this.convergenceRateHistory.reduce((a, b) => a + b, 0) / this.convergenceRateHistory.length
      : 0;

    return {
      iterations: this.iteration,
      finalResidual: current,
      initialResidual: this.initialResidualNorm,
      reductionFactor: this.getReductionFactor(),
      averageConvergenceRate: avgConvergenceRate,
      converged: this.isConverged,
      stagnated: this.stagnationDetected,
      diverged: this.divergenceDetected,
      tolerance: this.tolerance,
      relativeToleranceUsed: this.relativeToleranceEnabled,
      elapsedTime: this.lastUpdateTime - this.startTime,
      residualHistory: [...this.residualHistory],
      convergenceRateHistory: [...this.convergenceRateHistory]
    };
  }

  // Utility methods
  vectorNorm(vector) {
    return Math.sqrt(vector.reduce((sum, val) => sum + val * val, 0));
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
    } else if (matrix.format === 'csr') {
      for (let i = 0; i < matrix.rows; i++) {
        const start = matrix.data.rowPointers[i];
        const end = matrix.data.rowPointers[i + 1];
        for (let k = start; k < end; k++) {
          const col = matrix.data.colIndices[k];
          const val = matrix.data.values[k];
          result[i] += val * vector[col];
        }
      }
    }

    return result;
  }
}

module.exports = { ConvergenceDetector };