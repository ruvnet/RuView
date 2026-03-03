/**
 * Performance optimization utilities for matrix operations
 * Implements cache-friendly patterns, vectorization hints, and benchmarking
 */
import { Vector } from './types.js';
import { CSRMatrix } from './optimized-matrix.js';
import { MemoryStreamManager, MemoryProfile } from './memory-manager.js';
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
    blocking: {
        enabled: boolean;
        size: number;
    };
    streaming: {
        enabled: boolean;
        chunkSize: number;
    };
}
export declare class VectorizedOperations {
    private static readonly UNROLL_FACTOR;
    private static readonly PREFETCH_DISTANCE;
    static dotProduct(a: Vector, b: Vector, hints?: OptimizationHints): number;
    static vectorAdd(a: Vector, b: Vector, result: Vector, hints?: OptimizationHints): void;
    private static vectorAddBlock;
    static streamingOperation<T>(operation: 'add' | 'multiply' | 'dot', vectors: Vector[], chunkSize?: number): Promise<Vector | number>;
}
export declare class OptimizedMatrixMultiplication {
    static sparseMatVec(matrix: CSRMatrix, vector: Vector, result: Vector, blockSize?: number): void;
    static parallelMatVec(matrix: CSRMatrix, vector: Vector, numWorkers?: number): Promise<Vector>;
    private static createMatVecWorker;
    static selectOptimalAlgorithm(matrix: CSRMatrix, vector: Vector): {
        algorithm: 'sequential' | 'blocked' | 'parallel' | 'streaming';
        params: any;
    };
}
export declare class PerformanceBenchmark {
    private memoryManager;
    constructor(memoryManager?: MemoryStreamManager);
    benchmarkMatrixOperations(matrices: CSRMatrix[], vectors: Vector[], iterations?: number): Promise<BenchmarkResult[]>;
    private benchmarkOperation;
    generateOptimizationReport(benchmarks: BenchmarkResult[]): {
        recommendations: string[];
        bottlenecks: string[];
        memoryEfficiency: number;
        cacheEfficiency: number;
    };
    autoTuneParameters(matrix: CSRMatrix, vector: Vector): Promise<{
        optimalBlockSize: number;
        optimalUnrollFactor: number;
        recommendedAlgorithm: string;
    }>;
}
export declare const globalPerformanceOptimizer: PerformanceBenchmark;
