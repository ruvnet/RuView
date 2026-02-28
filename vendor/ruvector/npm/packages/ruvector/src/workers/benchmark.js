"use strict";
/**
 * Worker Benchmark Suite for RuVector
 *
 * Measures performance of:
 * - ONNX embedding generation (single vs batch)
 * - Vector storage and search
 * - Phase execution times
 * - Worker end-to-end throughput
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.benchmarkEmbeddings = benchmarkEmbeddings;
exports.benchmarkWorkers = benchmarkWorkers;
exports.benchmarkPhases = benchmarkPhases;
exports.formatBenchmarkResults = formatBenchmarkResults;
exports.runFullBenchmark = runFullBenchmark;
const perf_hooks_1 = require("perf_hooks");
const native_worker_1 = require("./native-worker");
const onnx_embedder_1 = require("../core/onnx-embedder");
/**
 * Run a benchmark function multiple times and collect stats
 */
async function runBenchmark(name, fn, iterations = 10, warmup = 2) {
    // Warmup runs
    for (let i = 0; i < warmup; i++) {
        await fn();
    }
    // Actual benchmark runs
    const times = [];
    for (let i = 0; i < iterations; i++) {
        const start = perf_hooks_1.performance.now();
        await fn();
        times.push(perf_hooks_1.performance.now() - start);
    }
    // Calculate statistics
    times.sort((a, b) => a - b);
    const sum = times.reduce((a, b) => a + b, 0);
    return {
        name,
        iterations,
        results: {
            min: times[0],
            max: times[times.length - 1],
            avg: sum / times.length,
            p50: times[Math.floor(times.length * 0.5)],
            p95: times[Math.floor(times.length * 0.95)],
            p99: times[Math.floor(times.length * 0.99)],
        },
    };
}
/**
 * Benchmark ONNX embedding generation
 */
async function benchmarkEmbeddings(iterations = 10) {
    const results = [];
    // Initialize embedder
    await (0, onnx_embedder_1.initOnnxEmbedder)();
    const stats = (0, onnx_embedder_1.getStats)();
    console.log(`\n📊 ONNX Embedder: ${stats.dimension}d, SIMD: ${stats.simd}`);
    // Single embedding benchmark
    const singleResult = await runBenchmark('Single embedding (short text)', async () => {
        await (0, onnx_embedder_1.embed)('This is a test sentence for embedding.');
    }, iterations);
    results.push(singleResult);
    // Single embedding - long text
    const longText = 'This is a much longer text that contains more content. '.repeat(20);
    const singleLongResult = await runBenchmark('Single embedding (long text)', async () => {
        await (0, onnx_embedder_1.embed)(longText);
    }, iterations);
    results.push(singleLongResult);
    // Batch embedding - small batch
    const smallBatch = Array(4).fill(0).map((_, i) => `Test sentence number ${i}`);
    const batchSmallResult = await runBenchmark('Batch embedding (4 texts)', async () => {
        await (0, onnx_embedder_1.embedBatch)(smallBatch);
    }, iterations);
    batchSmallResult.throughput = {
        itemsPerSecond: (4 * 1000) / batchSmallResult.results.avg,
    };
    results.push(batchSmallResult);
    // Batch embedding - medium batch
    const mediumBatch = Array(16).fill(0).map((_, i) => `Test sentence number ${i} with some content`);
    const batchMediumResult = await runBenchmark('Batch embedding (16 texts)', async () => {
        await (0, onnx_embedder_1.embedBatch)(mediumBatch);
    }, iterations);
    batchMediumResult.throughput = {
        itemsPerSecond: (16 * 1000) / batchMediumResult.results.avg,
    };
    results.push(batchMediumResult);
    // Batch embedding - large batch
    const largeBatch = Array(64).fill(0).map((_, i) => `Test sentence number ${i} with additional content here`);
    const batchLargeResult = await runBenchmark('Batch embedding (64 texts)', async () => {
        await (0, onnx_embedder_1.embedBatch)(largeBatch);
    }, Math.min(iterations, 5) // Fewer iterations for large batches
    );
    batchLargeResult.throughput = {
        itemsPerSecond: (64 * 1000) / batchLargeResult.results.avg,
    };
    results.push(batchLargeResult);
    return results;
}
/**
 * Benchmark worker execution
 */
async function benchmarkWorkers(targetPath = '.') {
    const results = [];
    // Security worker (no embeddings - fastest)
    const securityWorker = (0, native_worker_1.createSecurityWorker)();
    const securityResult = await runBenchmark('Security worker (no embeddings)', async () => {
        await securityWorker.run(targetPath);
    }, 5, 1);
    results.push(securityResult);
    // Analysis worker (with embeddings)
    const analysisWorker = (0, native_worker_1.createAnalysisWorker)();
    const analysisResult = await runBenchmark('Analysis worker (with embeddings)', async () => {
        await analysisWorker.run(targetPath);
    }, 3, 1);
    results.push(analysisResult);
    return results;
}
/**
 * Benchmark individual phases
 */
async function benchmarkPhases(targetPath = '.') {
    const results = [];
    // File discovery phase only
    const discoveryWorker = new native_worker_1.NativeWorker({
        name: 'discovery-only',
        phases: [{ type: 'file-discovery' }],
        capabilities: {},
    });
    const discoveryResult = await runBenchmark('Phase: file-discovery', async () => {
        await discoveryWorker.run(targetPath);
    }, 10);
    results.push(discoveryResult);
    // Pattern extraction phase
    const patternWorker = new native_worker_1.NativeWorker({
        name: 'pattern-only',
        phases: [{ type: 'file-discovery' }, { type: 'pattern-extraction' }],
        capabilities: {},
    });
    const patternResult = await runBenchmark('Phase: pattern-extraction', async () => {
        await patternWorker.run(targetPath);
    }, 5);
    results.push(patternResult);
    // Embedding generation phase
    const embeddingWorker = new native_worker_1.NativeWorker({
        name: 'embedding-only',
        phases: [
            { type: 'file-discovery', config: { patterns: ['**/*.ts'], exclude: ['**/node_modules/**'] } },
            { type: 'pattern-extraction' },
            { type: 'embedding-generation' },
        ],
        capabilities: { onnxEmbeddings: true },
    });
    const embeddingResult = await runBenchmark('Phase: embedding-generation', async () => {
        await embeddingWorker.run(targetPath);
    }, 3, 1);
    results.push(embeddingResult);
    return results;
}
/**
 * Format benchmark results as table
 */
function formatBenchmarkResults(results) {
    const lines = [];
    lines.push('');
    lines.push('┌─────────────────────────────────────┬──────────┬──────────┬──────────┬──────────┬──────────────┐');
    lines.push('│ Benchmark                           │ Min (ms) │ Avg (ms) │ P95 (ms) │ Max (ms) │ Throughput   │');
    lines.push('├─────────────────────────────────────┼──────────┼──────────┼──────────┼──────────┼──────────────┤');
    for (const result of results) {
        const name = result.name.padEnd(35).slice(0, 35);
        const min = result.results.min.toFixed(1).padStart(8);
        const avg = result.results.avg.toFixed(1).padStart(8);
        const p95 = result.results.p95.toFixed(1).padStart(8);
        const max = result.results.max.toFixed(1).padStart(8);
        const throughput = result.throughput
            ? `${result.throughput.itemsPerSecond.toFixed(1)}/s`.padStart(12)
            : '           -';
        lines.push(`│ ${name} │ ${min} │ ${avg} │ ${p95} │ ${max} │ ${throughput} │`);
    }
    lines.push('└─────────────────────────────────────┴──────────┴──────────┴──────────┴──────────┴──────────────┘');
    lines.push('');
    return lines.join('\n');
}
/**
 * Run full benchmark suite
 */
async function runFullBenchmark(targetPath = '.') {
    console.log('🚀 RuVector Native Worker Benchmark Suite\n');
    console.log('='.repeat(60));
    // Embeddings benchmark
    console.log('\n📊 Benchmarking ONNX Embeddings...');
    const embeddings = await benchmarkEmbeddings(10);
    console.log(formatBenchmarkResults(embeddings));
    // Phases benchmark
    console.log('\n⚡ Benchmarking Individual Phases...');
    const phases = await benchmarkPhases(targetPath);
    console.log(formatBenchmarkResults(phases));
    // Workers benchmark
    console.log('\n🔧 Benchmarking Full Workers...');
    const workers = await benchmarkWorkers(targetPath);
    console.log(formatBenchmarkResults(workers));
    // Summary
    const stats = (0, onnx_embedder_1.getStats)();
    const summary = `
RuVector Native Worker Benchmark Summary
========================================
ONNX Model: all-MiniLM-L6-v2 (${stats.dimension}d)
SIMD: ${stats.simd ? 'Enabled ✓' : 'Disabled'}
Parallel Workers: ${stats.parallel ? `${stats.parallelWorkers} workers` : 'Disabled'}

Embedding Performance:
  Single: ${embeddings[0].results.avg.toFixed(1)}ms avg
  Batch (16): ${embeddings[3].results.avg.toFixed(1)}ms avg (${embeddings[3].throughput?.itemsPerSecond.toFixed(0)}/s)
  Batch (64): ${embeddings[4].results.avg.toFixed(1)}ms avg (${embeddings[4].throughput?.itemsPerSecond.toFixed(0)}/s)

Worker Performance:
  Security scan: ${workers[0].results.avg.toFixed(0)}ms avg
  Full analysis: ${workers[1].results.avg.toFixed(0)}ms avg
`;
    console.log(summary);
    return { embeddings, phases, workers, summary };
}
exports.default = {
    benchmarkEmbeddings,
    benchmarkWorkers,
    benchmarkPhases,
    runFullBenchmark,
    formatBenchmarkResults,
};
//# sourceMappingURL=benchmark.js.map