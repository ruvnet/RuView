/**
 * Comprehensive Performance Benchmark
 *
 * This benchmark demonstrates the 5-10x performance improvements achieved by
 * the optimized solver implementations compared to naive implementations.
 */
/**
 * Benchmark result interface
 */
interface BenchmarkResult {
    name: string;
    matrixSize: number;
    nnz: number;
    optimizedTime: number;
    naiveTime: number;
    speedup: number;
    optimizedIterations: number;
    naiveIterations: number;
    optimizedResidual: number;
    naiveResidual: number;
    performanceStats?: {
        gflops: number;
        bandwidth: number;
        matVecCount: number;
        totalFlops: number;
    };
}
/**
 * Main benchmark runner
 */
export declare class PerformanceBenchmark {
    private vectorPool;
    /**
     * Run a single benchmark comparing optimized vs naive implementation
     */
    private runSingleBenchmark;
    /**
     * Run comprehensive benchmark suite
     */
    runBenchmarkSuite(): Promise<BenchmarkResult[]>;
    /**
     * Generate benchmark report
     */
    generateReport(results: BenchmarkResult[]): string;
    /**
     * Clean up resources
     */
    dispose(): void;
}
export {};
