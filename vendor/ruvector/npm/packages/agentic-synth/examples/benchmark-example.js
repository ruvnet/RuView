"use strict";
/**
 * Benchmark usage example
 */
Object.defineProperty(exports, "__esModule", { value: true });
const runner_js_1 = require("../src/benchmarks/runner.js");
const throughput_js_1 = require("../src/benchmarks/throughput.js");
const latency_js_1 = require("../src/benchmarks/latency.js");
const memory_js_1 = require("../src/benchmarks/memory.js");
const cache_js_1 = require("../src/benchmarks/cache.js");
const analyzer_js_1 = require("../src/benchmarks/analyzer.js");
const reporter_js_1 = require("../src/benchmarks/reporter.js");
const index_js_1 = require("../src/index.js");
async function main() {
    console.log('🔥 Running Performance Benchmarks\n');
    // Initialize
    const synth = new index_js_1.AgenticSynth({
        enableCache: true,
        cacheSize: 1000,
        maxConcurrency: 100,
    });
    const runner = new runner_js_1.BenchmarkRunner();
    const analyzer = new analyzer_js_1.BenchmarkAnalyzer();
    const reporter = new reporter_js_1.BenchmarkReporter();
    // Register benchmark suites
    runner.registerSuite(new throughput_js_1.ThroughputBenchmark(synth));
    runner.registerSuite(new latency_js_1.LatencyBenchmark(synth));
    runner.registerSuite(new memory_js_1.MemoryBenchmark(synth));
    runner.registerSuite(new cache_js_1.CacheBenchmark(synth));
    // Run benchmarks
    const result = await runner.runAll({
        name: 'Performance Test',
        iterations: 5,
        concurrency: 50,
        warmupIterations: 1,
        timeout: 300000,
    });
    // Analyze results
    analyzer.analyze(result);
    // Generate reports
    await reporter.generateMarkdown([result], 'benchmark-report.md');
    await reporter.generateJSON([result], 'benchmark-data.json');
    console.log('\n✅ Benchmarks complete!');
    console.log('📄 Reports saved to benchmark-report.md and benchmark-data.json');
}
main().catch(console.error);
//# sourceMappingURL=benchmark-example.js.map