/**
 * Worker Benchmark Suite for RuVector
 *
 * Measures performance of:
 * - ONNX embedding generation (single vs batch)
 * - Vector storage and search
 * - Phase execution times
 * - Worker end-to-end throughput
 */
import { BenchmarkResult } from './types';
/**
 * Benchmark ONNX embedding generation
 */
export declare function benchmarkEmbeddings(iterations?: number): Promise<BenchmarkResult[]>;
/**
 * Benchmark worker execution
 */
export declare function benchmarkWorkers(targetPath?: string): Promise<BenchmarkResult[]>;
/**
 * Benchmark individual phases
 */
export declare function benchmarkPhases(targetPath?: string): Promise<BenchmarkResult[]>;
/**
 * Format benchmark results as table
 */
export declare function formatBenchmarkResults(results: BenchmarkResult[]): string;
/**
 * Run full benchmark suite
 */
export declare function runFullBenchmark(targetPath?: string): Promise<{
    embeddings: BenchmarkResult[];
    phases: BenchmarkResult[];
    workers: BenchmarkResult[];
    summary: string;
}>;
declare const _default: {
    benchmarkEmbeddings: typeof benchmarkEmbeddings;
    benchmarkWorkers: typeof benchmarkWorkers;
    benchmarkPhases: typeof benchmarkPhases;
    runFullBenchmark: typeof runFullBenchmark;
    formatBenchmarkResults: typeof formatBenchmarkResults;
};
export default _default;
//# sourceMappingURL=benchmark.d.ts.map