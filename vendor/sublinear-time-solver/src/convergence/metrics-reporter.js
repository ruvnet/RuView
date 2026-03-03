/**
 * Advanced Metrics Reporting System
 *
 * Provides comprehensive performance metrics, convergence analysis,
 * and visualization support for solver benchmarks.
 */

class MetricsReporter {
  constructor(options = {}) {
    this.verboseOutput = options.verbose || false;
    this.saveHistory = options.saveHistory !== false;
    this.maxHistorySize = options.maxHistorySize || 1000;
    this.enableProfiling = options.enableProfiling || false;

    this.reset();
  }

  reset() {
    this.startTime = null;
    this.endTime = null;
    this.solverMetrics = [];
    this.performanceProfile = {
      matrixVectorMultiplications: 0,
      normComputations: 0,
      convergenceChecks: 0,
      memoryAllocations: 0
    };
    this.convergenceData = null;
  }

  /**
   * Start tracking metrics for a new solve
   */
  startSolve(solverConfig, matrixInfo) {
    this.reset();
    this.startTime = Date.now();
    this.solverConfig = { ...solverConfig };
    this.matrixInfo = { ...matrixInfo };

    if (this.verboseOutput) {
      console.log('📊 Starting metrics collection...');
      console.log(`   Matrix: ${matrixInfo.rows}×${matrixInfo.cols}, format: ${matrixInfo.format}`);
      console.log(`   Method: ${solverConfig.method}, tolerance: ${solverConfig.tolerance}`);
    }
  }

  /**
   * Record iteration metrics
   */
  recordIteration(convergenceMetrics, solverState = {}) {
    const iterationMetrics = {
      timestamp: Date.now(),
      iteration: convergenceMetrics.iteration,
      residualNorm: convergenceMetrics.residualNorm,
      relativeResidualNorm: convergenceMetrics.relativeResidualNorm,
      convergenceRate: convergenceMetrics.convergenceRate,
      reductionFactor: convergenceMetrics.reductionFactor,
      isConverged: convergenceMetrics.isConverged,
      shouldStop: convergenceMetrics.shouldStop,
      elapsedTime: convergenceMetrics.elapsedTime,
      iterationsPerSecond: convergenceMetrics.iterationsPerSecond,
      estimatedTimeRemaining: this.estimateTimeRemaining(convergenceMetrics),
      memoryUsage: this.getCurrentMemoryUsage(),
      ...solverState
    };

    // Store history if enabled
    if (this.saveHistory) {
      this.solverMetrics.push(iterationMetrics);

      // Limit history size to prevent memory issues
      if (this.solverMetrics.length > this.maxHistorySize) {
        this.solverMetrics.shift();
      }
    }

    // Update profiling counters
    if (this.enableProfiling) {
      this.performanceProfile.convergenceChecks++;
      if (convergenceMetrics.iteration > 0) {
        this.performanceProfile.matrixVectorMultiplications++;
        this.performanceProfile.normComputations++;
      }
    }

    return iterationMetrics;
  }

  /**
   * Finalize solve and generate comprehensive report
   */
  finalizeSolve(convergenceDetector, finalSolution = null) {
    this.endTime = Date.now();
    this.convergenceData = convergenceDetector.getConvergenceReport();

    const report = this.generateComprehensiveReport(finalSolution);

    if (this.verboseOutput) {
      this.printDetailedReport(report);
    }

    return report;
  }

  /**
   * Generate comprehensive performance and convergence report
   */
  generateComprehensiveReport(finalSolution = null) {
    const totalTime = this.endTime - this.startTime;
    const iterationCount = this.convergenceData.iterations;

    // Basic timing metrics
    const timingMetrics = {
      totalTime,
      averageTimePerIteration: iterationCount > 0 ? totalTime / iterationCount : 0,
      iterationsPerSecond: iterationCount / (totalTime / 1000),
      convergenceTime: this.convergenceData.elapsedTime
    };

    // Convergence analysis
    const convergenceAnalysis = this.analyzeConvergence();

    // Performance classification
    const performanceGrade = this.classifyPerformance();

    // Memory analysis
    const memoryAnalysis = this.analyzeMemoryUsage();

    // Solution quality (if solution provided)
    const solutionQuality = finalSolution ? this.assessSolutionQuality(finalSolution) : null;

    const report = {
      summary: {
        method: this.solverConfig.method,
        matrixSize: `${this.matrixInfo.rows}×${this.matrixInfo.cols}`,
        converged: this.convergenceData.converged,
        iterations: iterationCount,
        finalResidual: this.convergenceData.finalResidual,
        reductionFactor: this.convergenceData.reductionFactor,
        grade: performanceGrade
      },
      timing: timingMetrics,
      convergence: convergenceAnalysis,
      performance: performanceGrade,
      memory: memoryAnalysis,
      solution: solutionQuality,
      raw: {
        convergenceData: this.convergenceData,
        solverMetrics: this.saveHistory ? this.solverMetrics : [],
        performanceProfile: this.performanceProfile
      }
    };

    return report;
  }

  /**
   * Analyze convergence behavior
   */
  analyzeConvergence() {
    const analysis = {
      converged: this.convergenceData.converged,
      iterations: this.convergenceData.iterations,
      finalResidual: this.convergenceData.finalResidual,
      initialResidual: this.convergenceData.initialResidual,
      reductionFactor: this.convergenceData.reductionFactor,
      averageConvergenceRate: this.convergenceData.averageConvergenceRate,
      relativeToleranceUsed: this.convergenceData.relativeToleranceUsed,
      stagnated: this.convergenceData.stagnated,
      diverged: this.convergenceData.diverged
    };

    // Convergence rate classification
    if (analysis.averageConvergenceRate > 0 && analysis.averageConvergenceRate < 1) {
      analysis.convergenceType = 'linear';
      analysis.convergenceQuality = analysis.averageConvergenceRate < 0.1 ? 'excellent' :
                                   analysis.averageConvergenceRate < 0.5 ? 'good' :
                                   analysis.averageConvergenceRate < 0.9 ? 'acceptable' : 'slow';
    } else {
      analysis.convergenceType = 'unknown';
      analysis.convergenceQuality = 'poor';
    }

    // Efficiency assessment
    const theoreticalIterations = analysis.initialResidual > 0 && analysis.finalResidual > 0
      ? Math.log(analysis.finalResidual / analysis.initialResidual) / Math.log(analysis.averageConvergenceRate)
      : analysis.iterations;

    analysis.efficiency = analysis.iterations > 0 ? Math.min(1.0, theoreticalIterations / analysis.iterations) : 0;

    // Convergence rate percentage (what users expect to see)
    analysis.convergenceRatePercent = analysis.converged ? 100 :
      analysis.reductionFactor > 0 ? Math.min(99, Math.max(0, (1 - analysis.reductionFactor) * 100)) : 0;

    return analysis;
  }

  /**
   * Classify overall performance
   */
  classifyPerformance() {
    const iterations = this.convergenceData.iterations;
    const converged = this.convergenceData.converged;
    const time = this.endTime - this.startTime;
    const matrixSize = this.matrixInfo.rows;

    let score = 0;
    let grade = 'F';
    let description = 'Failed';

    // Convergence score (40%)
    if (converged) {
      score += 40;
      const optimalIterations = Math.sqrt(matrixSize); // Rough estimate for well-conditioned systems
      if (iterations <= optimalIterations) score += 20;
      else if (iterations <= optimalIterations * 2) score += 15;
      else if (iterations <= optimalIterations * 5) score += 10;
    }

    // Speed score (30%)
    const timePerElement = time / (matrixSize * matrixSize);
    if (timePerElement < 0.001) score += 30;
    else if (timePerElement < 0.01) score += 25;
    else if (timePerElement < 0.1) score += 20;
    else if (timePerElement < 1) score += 10;

    // Convergence rate score (30%)
    const avgRate = this.convergenceData.averageConvergenceRate;
    if (avgRate > 0 && avgRate < 0.1) score += 30;
    else if (avgRate < 0.3) score += 25;
    else if (avgRate < 0.7) score += 15;
    else if (avgRate < 0.95) score += 10;

    // Assign letter grade
    if (score >= 90) { grade = 'A+'; description = 'Excellent performance'; }
    else if (score >= 85) { grade = 'A'; description = 'Very good performance'; }
    else if (score >= 80) { grade = 'A-'; description = 'Good performance'; }
    else if (score >= 75) { grade = 'B+'; description = 'Above average performance'; }
    else if (score >= 70) { grade = 'B'; description = 'Average performance'; }
    else if (score >= 65) { grade = 'B-'; description = 'Below average performance'; }
    else if (score >= 60) { grade = 'C+'; description = 'Acceptable performance'; }
    else if (score >= 55) { grade = 'C'; description = 'Poor performance'; }
    else if (score >= 50) { grade = 'C-'; description = 'Very poor performance'; }
    else if (score >= 30) { grade = 'D'; description = 'Barely functional'; }

    return {
      score,
      grade,
      description,
      factors: {
        convergence: converged ? 'Good' : 'Poor',
        speed: timePerElement < 0.01 ? 'Good' : timePerElement < 0.1 ? 'Average' : 'Slow',
        efficiency: avgRate < 0.3 ? 'Good' : avgRate < 0.7 ? 'Average' : 'Poor'
      }
    };
  }

  /**
   * Analyze memory usage patterns
   */
  analyzeMemoryUsage() {
    if (!this.saveHistory || this.solverMetrics.length === 0) {
      return {
        available: false,
        reason: 'Memory tracking disabled or no data'
      };
    }

    const memoryValues = this.solverMetrics.map(m => m.memoryUsage).filter(m => m !== undefined);
    if (memoryValues.length === 0) {
      return {
        available: false,
        reason: 'No memory data collected'
      };
    }

    const initial = memoryValues[0];
    const peak = Math.max(...memoryValues);
    const final = memoryValues[memoryValues.length - 1];
    const average = memoryValues.reduce((a, b) => a + b, 0) / memoryValues.length;

    return {
      available: true,
      initialMB: initial,
      peakMB: peak,
      finalMB: final,
      averageMB: average,
      growthMB: final - initial,
      efficiency: this.matrixInfo.rows > 0 ? peak / (this.matrixInfo.rows * this.matrixInfo.rows * 8 / 1024 / 1024) : null
    };
  }

  /**
   * Assess solution quality if solution vector is provided
   */
  assessSolutionQuality(solution) {
    return {
      solutionNorm: this.vectorNorm(solution),
      maxElement: Math.max(...solution.map(Math.abs)),
      minElement: Math.min(...solution.map(Math.abs)),
      hasNaN: solution.some(x => isNaN(x)),
      hasInf: solution.some(x => !isFinite(x))
    };
  }

  /**
   * Estimate time remaining based on current convergence rate
   */
  estimateTimeRemaining(convergenceMetrics) {
    if (convergenceMetrics.isConverged || convergenceMetrics.shouldStop) {
      return 0;
    }

    const remainingIterations = convergenceMetrics.estimatedIterationsRemaining || 0;
    const avgTimePerIteration = convergenceMetrics.elapsedTime / Math.max(1, convergenceMetrics.iteration);

    return remainingIterations * avgTimePerIteration;
  }

  /**
   * Get current memory usage
   */
  getCurrentMemoryUsage() {
    try {
      const usage = process.memoryUsage();
      return Math.round(usage.heapUsed / 1024 / 1024); // MB
    } catch (error) {
      return undefined;
    }
  }

  /**
   * Print detailed report to console
   */
  printDetailedReport(report) {
    console.log('\n📊 DETAILED PERFORMANCE REPORT');
    console.log('=' .repeat(60));

    // Summary
    console.log(`\n🎯 SUMMARY`);
    console.log(`   Method: ${report.summary.method}`);
    console.log(`   Matrix: ${report.summary.matrixSize}`);
    console.log(`   Result: ${report.summary.converged ? '✅ Converged' : '❌ Did not converge'}`);
    console.log(`   Iterations: ${report.summary.iterations}`);
    console.log(`   Final Residual: ${report.summary.finalResidual.toExponential(3)}`);
    console.log(`   Grade: ${report.performance.grade} (${report.performance.description})`);

    // Convergence analysis
    console.log(`\n📈 CONVERGENCE ANALYSIS`);
    console.log(`   Convergence Rate: ${(report.convergence.convergenceRatePercent).toFixed(1)}%`);
    console.log(`   Reduction Factor: ${report.convergence.reductionFactor.toExponential(3)}`);
    console.log(`   Type: ${report.convergence.convergenceType} (${report.convergence.convergenceQuality})`);
    console.log(`   Efficiency: ${(report.convergence.efficiency * 100).toFixed(1)}%`);

    // Timing
    console.log(`\n⏱️  TIMING`);
    console.log(`   Total Time: ${report.timing.totalTime}ms`);
    console.log(`   Avg Time/Iteration: ${report.timing.averageTimePerIteration.toFixed(2)}ms`);
    console.log(`   Iterations/Second: ${report.timing.iterationsPerSecond.toFixed(1)}`);

    // Memory (if available)
    if (report.memory.available) {
      console.log(`\n💾 MEMORY`);
      console.log(`   Peak Usage: ${report.memory.peakMB.toFixed(1)}MB`);
      console.log(`   Final Usage: ${report.memory.finalMB.toFixed(1)}MB`);
      console.log(`   Growth: ${report.memory.growthMB > 0 ? '+' : ''}${report.memory.growthMB.toFixed(1)}MB`);
    }

    console.log('\n' + '=' .repeat(60));
  }

  /**
   * Export metrics for external analysis
   */
  exportMetrics(format = 'json') {
    const data = {
      config: this.solverConfig,
      matrix: this.matrixInfo,
      convergence: this.convergenceData,
      metrics: this.saveHistory ? this.solverMetrics : [],
      performance: this.performanceProfile,
      exportTime: new Date().toISOString()
    };

    if (format === 'json') {
      return JSON.stringify(data, null, 2);
    } else if (format === 'csv') {
      return this.convertToCsv(data);
    }

    return data;
  }

  // Utility methods
  vectorNorm(vector) {
    return Math.sqrt(vector.reduce((sum, val) => sum + val * val, 0));
  }

  convertToCsv(data) {
    if (!this.saveHistory || this.solverMetrics.length === 0) {
      return 'No iteration data available';
    }

    const headers = Object.keys(this.solverMetrics[0]);
    const rows = this.solverMetrics.map(metric =>
      headers.map(h => metric[h] !== undefined ? metric[h] : '').join(',')
    );

    return [headers.join(','), ...rows].join('\n');
  }
}

module.exports = { MetricsReporter };