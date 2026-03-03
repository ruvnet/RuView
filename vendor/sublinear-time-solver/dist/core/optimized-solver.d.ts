/**
 * Optimized solver implementation with memory-efficient algorithms
 * Integrates all optimization components for maximum performance
 */
import { Matrix, Vector, SolverConfig, SolverResult } from './types.js';
import { MemoryProfile } from './memory-manager.js';
export interface OptimizedSolverConfig extends SolverConfig {
    memoryOptimization: {
        enablePooling: boolean;
        enableStreaming: boolean;
        streamingThreshold: number;
        maxCacheSize: number;
    };
    performance: {
        enableVectorization: boolean;
        enableBlocking: boolean;
        autoTuning: boolean;
        parallelization: boolean;
    };
    adaptiveAlgorithms: {
        enabled: boolean;
        switchThreshold: number;
        memoryPressureThreshold: number;
    };
}
export interface OptimizedSolverResult extends SolverResult {
    optimizationStats: {
        memoryReduction: number;
        cacheHitRate: number;
        vectorizationEfficiency: number;
        algorithmsSwitched: number;
    };
    memoryProfile: MemoryProfile;
    recommendations: string[];
}
export declare class OptimizedSublinearSolver {
    private config;
    private csrMatrix?;
    private optimizationHints;
    private benchmarkInstance;
    private autoTunedParams?;
    constructor(config?: Partial<OptimizedSolverConfig>);
    private mergeDefaultConfig;
    solve(matrix: Matrix, vector: Vector): Promise<OptimizedSolverResult>;
    private preprocessMatrix;
    private estimateMatrixMemory;
    private selectOptimalAlgorithm;
    private executeSolve;
    private solveVectorizedNeumann;
    private solveBlockedNeumann;
    private solveStreamingNeumann;
    private solveParallelNeumann;
    private calculateOptimizationStats;
    private generateRecommendations;
    runBenchmark(matrices: Matrix[], vectors: Vector[]): Promise<{
        results: OptimizedSolverResult[];
        comparison: {
            averageSpeedup: number;
            averageMemoryReduction: number;
            recommendedConfig: Partial<OptimizedSolverConfig>;
        };
    }>;
    cleanup(): void;
}
