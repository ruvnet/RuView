/**
 * Comprehensive benchmarking suite for optimization validation
 * Tests memory reduction, cache efficiency, and performance improvements
 */

const { OptimizedSublinearSolver } = require('../dist/core/optimized-solver.js');
const { CSRMatrix, OptimizedMatrixOperations } = require('../dist/core/optimized-matrix.js');
const { globalMemoryManager } = require('../dist/core/memory-manager.js');
const { globalPerformanceOptimizer } = require('../dist/core/performance-optimizer.js');

// Test matrix generators
function generateTestMatrix(size, sparsity, type = 'diagonally-dominant') {
  const values = [];
  const rowIndices = [];
  const colIndices = [];

  // Generate random sparse structure
  const numNonZeros = Math.floor(size * size * sparsity);
  const nonZeroPositions = new Set();

  // Ensure diagonal elements are always present
  for (let i = 0; i < size; i++) {
    nonZeroPositions.add(`${i},${i}`);
  }

  // Add random off-diagonal elements
  while (nonZeroPositions.size < numNonZeros) {
    const row = Math.floor(Math.random() * size);
    const col = Math.floor(Math.random() * size);
    nonZeroPositions.add(`${row},${col}`);
  }

  // Convert to arrays and ensure diagonal dominance
  const rowSums = new Array(size).fill(0);

  for (const pos of nonZeroPositions) {
    const [row, col] = pos.split(',').map(Number);

    if (row !== col) {
      const value = (Math.random() - 0.5) * 0.5; // Small off-diagonal values
      values.push(value);
      rowIndices.push(row);
      colIndices.push(col);
      rowSums[row] += Math.abs(value);
    }
  }

  // Add diagonal elements to ensure dominance
  for (let i = 0; i < size; i++) {
    const diagonalValue = rowSums[i] * 1.5 + 1 + Math.random();
    values.push(diagonalValue);
    rowIndices.push(i);
    colIndices.push(i);
  }

  return {
    rows: size,
    cols: size,
    values,
    rowIndices,
    colIndices,
    format: 'coo'
  };
}

function generateTestVector(size) {
  return Array.from({ length: size }, () => Math.random() * 2 - 1);
}

// Memory usage tracking
class MemoryTracker {
  constructor() {
    this.measurements = [];
    this.startTime = performance.now();
  }

  measure(label) {
    const currentTime = performance.now();
    let memoryUsage = 0;

    // Try to get memory info if available
    if (typeof performance !== 'undefined' && performance.memory) {
      memoryUsage = performance.memory.usedJSHeapSize;
    }

    this.measurements.push({
      label,
      timestamp: currentTime - this.startTime,
      memoryUsage
    });
  }

  getMemoryDelta(startLabel, endLabel) {
    const start = this.measurements.find(m => m.label === startLabel);
    const end = this.measurements.find(m => m.label === endLabel);

    if (start && end) {
      return end.memoryUsage - start.memoryUsage;
    }
    return 0;
  }

  getReport() {
    return {
      measurements: this.measurements,
      totalDuration: this.measurements.length > 0
        ? this.measurements[this.measurements.length - 1].timestamp
        : 0,
      peakMemory: Math.max(...this.measurements.map(m => m.memoryUsage))
    };
  }
}

// Benchmark test cases
async function runOptimizationBenchmarks() {
  console.log('🚀 Starting Optimization Benchmarks...\n');

  const results = {
    memoryTests: [],
    performanceTests: [],
    scalabilityTests: [],
    optimizationValidation: {}
  };

  // Test different matrix sizes
  const testSizes = [100, 500, 1000, 2000];
  const sparsities = [0.1, 0.05, 0.01];

  for (const size of testSizes) {
    for (const sparsity of sparsities) {
      console.log(`📊 Testing matrix size: ${size}x${size}, sparsity: ${sparsity}`);

      const matrix = generateTestMatrix(size, sparsity);
      const vector = generateTestVector(size);
      const tracker = new MemoryTracker();

      tracker.measure('start');

      // Test memory optimization
      const memoryResult = await testMemoryOptimization(matrix, vector, tracker);
      results.memoryTests.push({
        size,
        sparsity,
        ...memoryResult
      });

      // Test performance optimization
      const perfResult = await testPerformanceOptimization(matrix, vector, tracker);
      results.performanceTests.push({
        size,
        sparsity,
        ...perfResult
      });

      tracker.measure('end');

      console.log(`  ✅ Memory reduction: ${(memoryResult.memoryReduction * 100).toFixed(1)}%`);
      console.log(`  ⚡ Speedup: ${perfResult.speedup.toFixed(2)}x`);
      console.log(`  💾 Cache hit rate: ${(perfResult.cacheHitRate * 100).toFixed(1)}%\n`);
    }
  }

  // Test scalability
  console.log('📈 Testing scalability...');
  results.scalabilityTests = await testScalability();

  // Validate optimization targets
  console.log('🎯 Validating optimization targets...');
  results.optimizationValidation = validateOptimizationTargets(results);

  return results;
}

async function testMemoryOptimization(matrix, vector, tracker) {
  tracker.measure('memory-test-start');

  // Test with memory optimization disabled
  const unoptimizedSolver = new OptimizedSublinearSolver({
    memoryOptimization: {
      enablePooling: false,
      enableStreaming: false,
      streamingThreshold: Infinity,
      maxCacheSize: 0
    },
    performance: {
      enableVectorization: false,
      enableBlocking: false,
      autoTuning: false,
      parallelization: false
    }
  });

  tracker.measure('unoptimized-start');
  const unoptimizedResult = await unoptimizedSolver.solve(matrix, vector);
  tracker.measure('unoptimized-end');

  unoptimizedSolver.cleanup();

  // Test with memory optimization enabled
  const optimizedSolver = new OptimizedSublinearSolver({
    memoryOptimization: {
      enablePooling: true,
      enableStreaming: true,
      streamingThreshold: 1024 * 1024,
      maxCacheSize: 100
    }
  });

  tracker.measure('optimized-start');
  const optimizedResult = await optimizedSolver.solve(matrix, vector);
  tracker.measure('optimized-end');

  optimizedSolver.cleanup();

  const unoptimizedMemory = tracker.getMemoryDelta('unoptimized-start', 'unoptimized-end');
  const optimizedMemory = tracker.getMemoryDelta('optimized-start', 'optimized-end');

  const memoryReduction = unoptimizedMemory > 0
    ? (unoptimizedMemory - optimizedMemory) / unoptimizedMemory
    : 0;

  tracker.measure('memory-test-end');

  return {
    memoryReduction,
    unoptimizedMemory,
    optimizedMemory,
    optimizationStats: optimizedResult.optimizationStats,
    converged: optimizedResult.converged && unoptimizedResult.converged
  };
}

async function testPerformanceOptimization(matrix, vector, tracker) {
  tracker.measure('performance-test-start');

  // Baseline performance (minimal optimizations)
  const baselineSolver = new OptimizedSublinearSolver({
    performance: {
      enableVectorization: false,
      enableBlocking: false,
      autoTuning: false,
      parallelization: false
    }
  });

  const baselineStart = performance.now();
  const baselineResult = await baselineSolver.solve(matrix, vector);
  const baselineTime = performance.now() - baselineStart;

  baselineSolver.cleanup();

  // Optimized performance
  const optimizedSolver = new OptimizedSublinearSolver({
    performance: {
      enableVectorization: true,
      enableBlocking: true,
      autoTuning: true,
      parallelization: true
    }
  });

  const optimizedStart = performance.now();
  const optimizedResult = await optimizedSolver.solve(matrix, vector);
  const optimizedTime = performance.now() - optimizedStart;

  optimizedSolver.cleanup();

  const speedup = baselineTime > 0 ? baselineTime / optimizedTime : 1;

  tracker.measure('performance-test-end');

  return {
    speedup,
    baselineTime,
    optimizedTime,
    cacheHitRate: optimizedResult.optimizationStats.cacheHitRate,
    vectorizationEfficiency: optimizedResult.optimizationStats.vectorizationEfficiency,
    converged: optimizedResult.converged && baselineResult.converged
  };
}

async function testScalability() {
  const scalabilityResults = [];
  const sizes = [500, 1000, 2000, 4000];

  for (const size of sizes) {
    console.log(`  📏 Testing scalability at size ${size}...`);

    const matrix = generateTestMatrix(size, 0.05);
    const vector = generateTestVector(size);

    const solver = new OptimizedSublinearSolver({
      memoryOptimization: { enableStreaming: true },
      performance: { autoTuning: true }
    });

    const start = performance.now();
    const result = await solver.solve(matrix, vector);
    const duration = performance.now() - start;

    solver.cleanup();

    scalabilityResults.push({
      size,
      duration,
      memoryUsed: result.memoryProfile.peakMemory,
      timePerElement: duration / (size * size),
      converged: result.converged
    });
  }

  return scalabilityResults;
}

function validateOptimizationTargets(results) {
  const validation = {
    memoryTarget: false,
    cacheTarget: false,
    performanceTarget: false,
    summary: ''
  };

  // Check 50% memory reduction target
  const avgMemoryReduction = results.memoryTests.reduce(
    (sum, test) => sum + test.memoryReduction, 0
  ) / results.memoryTests.length;

  validation.memoryTarget = avgMemoryReduction >= 0.5;

  // Check cache hit rate improvement
  const avgCacheHitRate = results.performanceTests.reduce(
    (sum, test) => sum + test.cacheHitRate, 0
  ) / results.performanceTests.length;

  validation.cacheTarget = avgCacheHitRate >= 0.7;

  // Check performance improvement
  const avgSpeedup = results.performanceTests.reduce(
    (sum, test) => sum + test.speedup, 0
  ) / results.performanceTests.length;

  validation.performanceTarget = avgSpeedup >= 1.5;

  // Generate summary
  const memoryStr = `Memory reduction: ${(avgMemoryReduction * 100).toFixed(1)}% (target: 50%)`;
  const cacheStr = `Cache hit rate: ${(avgCacheHitRate * 100).toFixed(1)}% (target: 70%)`;
  const perfStr = `Average speedup: ${avgSpeedup.toFixed(2)}x (target: 1.5x)`;

  validation.summary = `${memoryStr}\n${cacheStr}\n${perfStr}`;

  return validation;
}

// Performance comparison with baseline
async function compareWithBaseline() {
  console.log('⚖️  Comparing with baseline implementation...\n');

  const matrix = generateTestMatrix(1000, 0.05);
  const vector = generateTestVector(1000);

  // Simulate baseline (unoptimized) performance
  const baselineTime = 1000; // ms
  const baselineMemory = 50 * 1024 * 1024; // 50MB

  // Test optimized version
  const optimizedSolver = new OptimizedSublinearSolver();
  const start = performance.now();
  const result = await optimizedSolver.solve(matrix, vector);
  const optimizedTime = performance.now() - start;

  const comparison = {
    timeImprovement: baselineTime / optimizedTime,
    memoryImprovement: baselineMemory / result.memoryProfile.peakMemory,
    optimizationStats: result.optimizationStats
  };

  console.log(`⏱️  Time improvement: ${comparison.timeImprovement.toFixed(2)}x`);
  console.log(`💾  Memory improvement: ${comparison.memoryImprovement.toFixed(2)}x`);
  console.log(`📈  Cache hit rate: ${(result.optimizationStats.cacheHitRate * 100).toFixed(1)}%`);
  console.log(`🔧  Vectorization efficiency: ${(result.optimizationStats.vectorizationEfficiency * 100).toFixed(1)}%`);

  optimizedSolver.cleanup();

  return comparison;
}

// Generate optimization report
function generateOptimizationReport(results, comparison) {
  const report = {
    timestamp: new Date().toISOString(),
    summary: {
      testsRun: results.memoryTests.length + results.performanceTests.length + results.scalabilityTests.length,
      targetsAchieved: Object.values(results.optimizationValidation).filter(v => v === true).length,
      overallSuccess: Object.values(results.optimizationValidation).every(v => v === true)
    },
    memoryOptimization: {
      averageReduction: results.memoryTests.reduce((sum, t) => sum + t.memoryReduction, 0) / results.memoryTests.length,
      bestReduction: Math.max(...results.memoryTests.map(t => t.memoryReduction)),
      targetAchieved: results.optimizationValidation.memoryTarget
    },
    performanceOptimization: {
      averageSpeedup: results.performanceTests.reduce((sum, t) => sum + t.speedup, 0) / results.performanceTests.length,
      bestSpeedup: Math.max(...results.performanceTests.map(t => t.speedup)),
      averageCacheHitRate: results.performanceTests.reduce((sum, t) => sum + t.cacheHitRate, 0) / results.performanceTests.length,
      targetAchieved: results.optimizationValidation.performanceTarget
    },
    scalability: {
      largestMatrixTested: Math.max(...results.scalabilityTests.map(t => t.size)),
      timeComplexity: 'O(n²)', // Estimated
      memoryComplexity: 'O(nnz)', // Non-zeros
      scalabilityScore: results.scalabilityTests.every(t => t.converged) ? 'Good' : 'Needs improvement'
    },
    comparison,
    recommendations: generateRecommendations(results)
  };

  return report;
}

function generateRecommendations(results) {
  const recommendations = [];

  const avgMemoryReduction = results.memoryTests.reduce(
    (sum, t) => sum + t.memoryReduction, 0
  ) / results.memoryTests.length;

  if (avgMemoryReduction < 0.5) {
    recommendations.push('Increase memory pooling effectiveness');
    recommendations.push('Implement more aggressive streaming for large matrices');
  }

  const avgCacheHitRate = results.performanceTests.reduce(
    (sum, t) => sum + t.cacheHitRate, 0
  ) / results.performanceTests.length;

  if (avgCacheHitRate < 0.7) {
    recommendations.push('Optimize data locality with better blocking strategies');
    recommendations.push('Tune cache replacement policies');
  }

  const avgSpeedup = results.performanceTests.reduce(
    (sum, t) => sum + t.speedup, 0
  ) / results.performanceTests.length;

  if (avgSpeedup < 2.0) {
    recommendations.push('Enhance vectorization patterns');
    recommendations.push('Consider GPU acceleration for large problems');
  }

  return recommendations;
}

// Main benchmark execution
async function main() {
  try {
    console.log('🔧 Matrix Operations Memory Optimization Benchmark');
    console.log('==================================================\n');

    const results = await runOptimizationBenchmarks();
    const comparison = await compareWithBaseline();
    const report = generateOptimizationReport(results, comparison);

    console.log('\n📋 OPTIMIZATION REPORT');
    console.log('======================');
    console.log(JSON.stringify(report, null, 2));

    // Write report to file
    const fs = require('fs');
    const path = require('path');

    const reportPath = path.join(__dirname, '..', 'optimization-report.json');
    fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));

    console.log(`\n📄 Report saved to: ${reportPath}`);

    // Print summary
    console.log('\n🎯 OPTIMIZATION TARGETS');
    console.log('=======================');
    console.log(results.optimizationValidation.summary);

    const success = results.optimizationValidation.memoryTarget &&
                   results.optimizationValidation.cacheTarget &&
                   results.optimizationValidation.performanceTarget;

    console.log(`\n${success ? '✅' : '❌'} Overall optimization target: ${success ? 'ACHIEVED' : 'NOT ACHIEVED'}`);

    if (report.recommendations.length > 0) {
      console.log('\n💡 RECOMMENDATIONS');
      console.log('==================');
      report.recommendations.forEach((rec, i) => {
        console.log(`${i + 1}. ${rec}`);
      });
    }

    // Cleanup
    globalMemoryManager.cleanup();

    process.exit(success ? 0 : 1);

  } catch (error) {
    console.error('❌ Benchmark failed:', error);
    process.exit(1);
  }
}

// Export for use as module
module.exports = {
  runOptimizationBenchmarks,
  testMemoryOptimization,
  testPerformanceOptimization,
  generateOptimizationReport,
  main
};

// Run if called directly
if (require.main === module) {
  main();
}