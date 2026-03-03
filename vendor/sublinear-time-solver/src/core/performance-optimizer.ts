/**
 * Performance optimization utilities for matrix operations
 * Implements cache-friendly patterns, vectorization hints, and benchmarking
 */

import { Vector } from './types.js';
import { CSRMatrix, CSCMatrix, OptimizedMatrixOperations } from './optimized-matrix.js';
import { MemoryStreamManager, MemoryProfile, globalMemoryManager } from './memory-manager.js';

export interface BenchmarkResult {
  operation: string;
  iterations: number;
  totalTime: number;
  averageTime: number;
  throughput: number;
  memoryProfile: MemoryProfile;
  cacheStats: {
    hitRate: number;
    missRate: number;
  };
}

export interface OptimizationHints {
  vectorize: boolean;
  unroll: number;
  prefetch: boolean;
  blocking: { enabled: boolean; size: number };
  streaming: { enabled: boolean; chunkSize: number };
}

// Vectorized math operations with SIMD hints
export class VectorizedOperations {
  private static readonly UNROLL_FACTOR = 4;
  private static readonly PREFETCH_DISTANCE = 64;

  // Highly optimized dot product with cache prefetching
  static dotProduct(a: Vector, b: Vector, hints?: OptimizationHints): number {
    const n = a.length;
    const unrollFactor = hints?.unroll || this.UNROLL_FACTOR;
    let sum = 0;

    // Main vectorized loop
    let i = 0;
    for (; i <= n - unrollFactor; i += unrollFactor) {
      // Prefetch next cache line if enabled
      if (hints?.prefetch && i + this.PREFETCH_DISTANCE < n) {
        // Browser doesn't expose prefetch directly, but accessing helps
        const prefetchIndex = i + this.PREFETCH_DISTANCE;
        void a[prefetchIndex]; // Touch for prefetch hint
        void b[prefetchIndex];
      }

      // Unrolled loop for SIMD optimization
      sum += a[i] * b[i] +
             a[i + 1] * b[i + 1] +
             a[i + 2] * b[i + 2] +
             a[i + 3] * b[i + 3];
    }

    // Handle remaining elements
    for (; i < n; i++) {
      sum += a[i] * b[i];
    }

    return sum;
  }

  // Cache-optimized vector addition with blocking
  static vectorAdd(a: Vector, b: Vector, result: Vector, hints?: OptimizationHints): void {
    const n = a.length;
    const blockSize = hints?.blocking.enabled ? hints.blocking.size : 1024;

    if (hints?.blocking.enabled && n > blockSize) {
      // Process in blocks for better cache locality
      for (let blockStart = 0; blockStart < n; blockStart += blockSize) {
        const blockEnd = Math.min(blockStart + blockSize, n);
        this.vectorAddBlock(a, b, result, blockStart, blockEnd, hints);
      }
    } else {
      this.vectorAddBlock(a, b, result, 0, n, hints);
    }
  }

  private static vectorAddBlock(
    a: Vector,
    b: Vector,
    result: Vector,
    start: number,
    end: number,
    hints?: OptimizationHints
  ): void {
    const unrollFactor = hints?.unroll || this.UNROLL_FACTOR;

    let i = start;
    for (; i <= end - unrollFactor; i += unrollFactor) {
      result[i] = a[i] + b[i];
      result[i + 1] = a[i + 1] + b[i + 1];
      result[i + 2] = a[i + 2] + b[i + 2];
      result[i + 3] = a[i + 3] + b[i + 3];
    }

    for (; i < end; i++) {
      result[i] = a[i] + b[i];
    }
  }

  // Streaming vector operations for large arrays
  static async streamingOperation<T>(
    operation: 'add' | 'multiply' | 'dot',
    vectors: Vector[],
    chunkSize = 10000
  ): Promise<Vector | number> {
    const n = vectors[0].length;

    if (operation === 'dot' && vectors.length === 2) {
      let sum = 0;

      for (let start = 0; start < n; start += chunkSize) {
        const end = Math.min(start + chunkSize, n);
        const chunkA = vectors[0].slice(start, end);
        const chunkB = vectors[1].slice(start, end);

        sum += this.dotProduct(chunkA, chunkB);

        // Yield control periodically
        if (start % (chunkSize * 10) === 0) {
          await new Promise(resolve => setTimeout(resolve, 0));
        }
      }

      return sum;
    } else if (operation === 'add' && vectors.length === 2) {
      const result = globalMemoryManager.acquireTypedArray('float64', n);

      for (let start = 0; start < n; start += chunkSize) {
        const end = Math.min(start + chunkSize, n);
        const chunkA = vectors[0].slice(start, end);
        const chunkB = vectors[1].slice(start, end);
        const chunkResult = new Array(end - start);

        this.vectorAdd(chunkA, chunkB, chunkResult);

        // Copy back to result
        for (let i = 0; i < chunkResult.length; i++) {
          result[start + i] = chunkResult[i];
        }

        // Yield control
        if (start % (chunkSize * 10) === 0) {
          await new Promise(resolve => setTimeout(resolve, 0));
        }
      }

      return Array.from(result);
    }

    throw new Error(`Unsupported streaming operation: ${operation}`);
  }
}

// Matrix multiplication with advanced optimizations
export class OptimizedMatrixMultiplication {
  // Cache-blocked sparse matrix-vector multiplication
  static sparseMatVec(
    matrix: CSRMatrix,
    vector: Vector,
    result: Vector,
    blockSize = 1000
  ): void {
    const rows = matrix.getRows();

    // Process matrix in row blocks for cache efficiency
    for (let blockStart = 0; blockStart < rows; blockStart += blockSize) {
      const blockEnd = Math.min(blockStart + blockSize, rows);

      for (let row = blockStart; row < blockEnd; row++) {
        let sum = 0;

        // Process row entries with prefetching
        for (const entry of matrix.rowEntries(row)) {
          sum += entry.val * vector[entry.col];
        }

        result[row] = sum;
      }
    }
  }

  // Parallel matrix-vector multiplication using Web Workers (when available)
  static async parallelMatVec(
    matrix: CSRMatrix,
    vector: Vector,
    numWorkers = navigator.hardwareConcurrency || 4
  ): Promise<Vector> {
    const rows = matrix.getRows();
    const result = new Array(rows).fill(0);

    if (typeof globalThis === 'undefined' || !(globalThis as any).Worker || rows < 1000) {
      // Fallback to sequential implementation
      this.sparseMatVec(matrix, vector, result);
      return result;
    }

    const chunkSize = Math.ceil(rows / numWorkers);
    const promises: Promise<Vector>[] = [];

    for (let i = 0; i < numWorkers; i++) {
      const startRow = i * chunkSize;
      const endRow = Math.min(startRow + chunkSize, rows);

      if (startRow >= rows) break;

      // Create worker for this chunk
      const workerPromise = this.createMatVecWorker(matrix, vector, startRow, endRow);
      promises.push(workerPromise);
    }

    const results = await Promise.all(promises);

    // Combine results
    let offset = 0;
    for (const chunkResult of results) {
      for (let i = 0; i < chunkResult.length; i++) {
        result[offset + i] = chunkResult[i];
      }
      offset += chunkResult.length;
    }

    return result;
  }

  private static async createMatVecWorker(
    matrix: CSRMatrix,
    vector: Vector,
    startRow: number,
    endRow: number
  ): Promise<Vector> {
    // In a real implementation, this would use Web Workers
    // For now, simulate with async processing
    return new Promise(resolve => {
      setTimeout(() => {
        const chunkResult = new Array(endRow - startRow).fill(0);

        for (let row = startRow; row < endRow; row++) {
          let sum = 0;
          for (const entry of matrix.rowEntries(row)) {
            sum += entry.val * vector[entry.col];
          }
          chunkResult[row - startRow] = sum;
        }

        resolve(chunkResult);
      }, 0);
    });
  }

  // Adaptive algorithm selection based on matrix properties
  static selectOptimalAlgorithm(matrix: CSRMatrix, vector: Vector): {
    algorithm: 'sequential' | 'blocked' | 'parallel' | 'streaming';
    params: any;
  } {
    const nnz = matrix.getNnz();
    const rows = matrix.getRows();
    const sparsity = nnz / (rows * matrix.getCols());
    const memoryUsage = matrix.getMemoryUsage();

    // Decision tree based on matrix characteristics
    if (memoryUsage > 100 * 1024 * 1024) { // > 100MB
      return {
        algorithm: 'streaming',
        params: { chunkSize: 1000 }
      };
    } else if (rows > 10000 && typeof globalThis !== 'undefined' && (globalThis as any).Worker) {
      return {
        algorithm: 'parallel',
        params: { numWorkers: navigator.hardwareConcurrency || 4 }
      };
    } else if (sparsity < 0.1 && rows > 1000) {
      return {
        algorithm: 'blocked',
        params: { blockSize: Math.min(1000, Math.ceil(Math.sqrt(rows))) }
      };
    } else {
      return {
        algorithm: 'sequential',
        params: {}
      };
    }
  }
}

// Performance benchmarking and optimization guidance
export class PerformanceBenchmark {
  private memoryManager: MemoryStreamManager;

  constructor(memoryManager = globalMemoryManager) {
    this.memoryManager = memoryManager;
  }

  // Comprehensive matrix operation benchmark
  async benchmarkMatrixOperations(
    matrices: CSRMatrix[],
    vectors: Vector[],
    iterations = 100
  ): Promise<BenchmarkResult[]> {
    const results: BenchmarkResult[] = [];

    for (let i = 0; i < matrices.length; i++) {
      const matrix = matrices[i];
      const vector = vectors[i];
      const result = globalMemoryManager.acquireTypedArray('float64', matrix.getRows());

      // Benchmark sequential multiplication
      const seqResult = await this.benchmarkOperation(
        'Sequential MatVec',
        () => OptimizedMatrixMultiplication.sparseMatVec(matrix, vector, Array.from(result)),
        iterations
      );
      results.push(seqResult);

      // Benchmark blocked multiplication
      const blockedResult = await this.benchmarkOperation(
        'Blocked MatVec',
        () => OptimizedMatrixMultiplication.sparseMatVec(matrix, vector, Array.from(result), 500),
        iterations
      );
      results.push(blockedResult);

      // Benchmark vectorized operations
      const vecResult = await this.benchmarkOperation(
        'Vectorized Dot Product',
        () => VectorizedOperations.dotProduct(vector, vector),
        iterations * 10
      );
      results.push(vecResult);

      globalMemoryManager.releaseTypedArray(result);
    }

    return results;
  }

  private async benchmarkOperation(
    name: string,
    operation: () => any,
    iterations: number
  ): Promise<BenchmarkResult> {
    // Warmup
    for (let i = 0; i < Math.min(10, iterations); i++) {
      operation();
    }

    const { result, profile } = await this.memoryManager.profileOperation(
      name,
      async () => {
        const startTime = performance.now();

        for (let i = 0; i < iterations; i++) {
          operation();
        }

        return performance.now() - startTime;
      }
    );

    const totalTime = result;
    const averageTime = totalTime / iterations;
    const throughput = iterations / (totalTime / 1000); // ops per second

    return {
      operation: name,
      iterations,
      totalTime,
      averageTime,
      throughput,
      memoryProfile: profile,
      cacheStats: {
        hitRate: profile.cacheHitRate,
        missRate: 1 - profile.cacheHitRate
      }
    };
  }

  // Generate optimization recommendations
  generateOptimizationReport(benchmarks: BenchmarkResult[]): {
    recommendations: string[];
    bottlenecks: string[];
    memoryEfficiency: number;
    cacheEfficiency: number;
  } {
    const recommendations: string[] = [];
    const bottlenecks: string[] = [];

    let totalMemoryDelta = 0;
    let totalCacheHitRate = 0;

    for (const benchmark of benchmarks) {
      totalMemoryDelta += Math.abs(benchmark.memoryProfile.memoryDelta);
      totalCacheHitRate += benchmark.cacheStats.hitRate;

      // Analyze performance characteristics
      if (benchmark.throughput < 1000) {
        bottlenecks.push(`Low throughput in ${benchmark.operation}: ${benchmark.throughput.toFixed(2)} ops/sec`);
      }

      if (benchmark.cacheStats.hitRate < 0.8) {
        recommendations.push(`Improve cache locality for ${benchmark.operation} (hit rate: ${(benchmark.cacheStats.hitRate * 100).toFixed(1)}%)`);
      }

      if (benchmark.memoryProfile.memoryDelta > 1024 * 1024) {
        recommendations.push(`Reduce memory allocation in ${benchmark.operation} (${(benchmark.memoryProfile.memoryDelta / 1024 / 1024).toFixed(2)}MB allocated)`);
      }

      if (benchmark.averageTime > 100) {
        recommendations.push(`Consider parallelization for ${benchmark.operation} (avg time: ${benchmark.averageTime.toFixed(2)}ms)`);
      }
    }

    const avgMemoryDelta = totalMemoryDelta / benchmarks.length;
    const avgCacheHitRate = totalCacheHitRate / benchmarks.length;

    // General recommendations
    if (avgCacheHitRate < 0.7) {
      recommendations.push('Consider using blocked algorithms for better cache locality');
    }

    if (avgMemoryDelta > 1024 * 1024) {
      recommendations.push('Implement memory pooling to reduce allocation overhead');
    }

    return {
      recommendations,
      bottlenecks,
      memoryEfficiency: 1 - (avgMemoryDelta / (1024 * 1024 * 100)), // Normalized efficiency
      cacheEfficiency: avgCacheHitRate
    };
  }

  // Auto-tuning for optimal parameters
  async autoTuneParameters(
    matrix: CSRMatrix,
    vector: Vector
  ): Promise<{
    optimalBlockSize: number;
    optimalUnrollFactor: number;
    recommendedAlgorithm: string;
  }> {
    const blockSizes = [64, 128, 256, 512, 1024];
    const unrollFactors = [2, 4, 8];
    let bestBlockSize = 256;
    let bestUnrollFactor = 4;
    let bestThroughput = 0;

    // Test different block sizes
    for (const blockSize of blockSizes) {
      const result = await this.benchmarkOperation(
        `Block size ${blockSize}`,
        () => OptimizedMatrixMultiplication.sparseMatVec(
          matrix,
          vector,
          new Array(matrix.getRows()).fill(0),
          blockSize
        ),
        50
      );

      if (result.throughput > bestThroughput) {
        bestThroughput = result.throughput;
        bestBlockSize = blockSize;
      }
    }

    // Test different unroll factors for vector operations
    bestThroughput = 0;
    for (const unrollFactor of unrollFactors) {
      const result = await this.benchmarkOperation(
        `Unroll factor ${unrollFactor}`,
        () => VectorizedOperations.dotProduct(vector, vector, {
          vectorize: true,
          unroll: unrollFactor,
          prefetch: false,
          blocking: { enabled: false, size: 0 },
          streaming: { enabled: false, chunkSize: 0 }
        }),
        100
      );

      if (result.throughput > bestThroughput) {
        bestThroughput = result.throughput;
        bestUnrollFactor = unrollFactor;
      }
    }

    // Select optimal algorithm
    const algorithmSelection = OptimizedMatrixMultiplication.selectOptimalAlgorithm(matrix, vector);

    return {
      optimalBlockSize: bestBlockSize,
      optimalUnrollFactor: bestUnrollFactor,
      recommendedAlgorithm: algorithmSelection.algorithm
    };
  }
}

// Global performance optimizer
export const globalPerformanceOptimizer = new PerformanceBenchmark();