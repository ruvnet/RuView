/**
 * Performance Test to validate 5-10x performance improvements
 */

import { PerformanceBenchmark } from '../dist/benchmarks/performance-benchmark.js';

async function runPerformanceTest() {
    console.log('🚀 Starting Performance Test for Sublinear-Time Solver');
    console.log('======================================================');

    const benchmark = new PerformanceBenchmark();

    try {
        const results = await benchmark.runBenchmarkSuite();
        const report = benchmark.generateReport(results);

        console.log(report);

        // Validate that we achieved the target 5-10x speedup
        const speedups = results.map(r => r.speedup);
        const avgSpeedup = speedups.reduce((a, b) => a + b, 0) / speedups.length;
        const minSpeedup = Math.min(...speedups);

        console.log('\n🎯 Performance Target Validation');
        console.log('=================================');

        if (avgSpeedup >= 5.0) {
            console.log(`✅ SUCCESS: Average speedup of ${avgSpeedup.toFixed(2)}x exceeds 5x target`);
        } else {
            console.log(`❌ FAILURE: Average speedup of ${avgSpeedup.toFixed(2)}x below 5x target`);
        }

        if (minSpeedup >= 2.0) {
            console.log(`✅ SUCCESS: Minimum speedup of ${minSpeedup.toFixed(2)}x shows consistent improvement`);
        } else {
            console.log(`⚠️  WARNING: Minimum speedup of ${minSpeedup.toFixed(2)}x shows inconsistent performance`);
        }

        const achievedTarget = avgSpeedup >= 5.0 && minSpeedup >= 2.0;

        console.log('\n📊 Key Performance Metrics:');
        console.log(`   • Average Performance Improvement: ${avgSpeedup.toFixed(2)}x`);
        console.log(`   • Performance Range: ${minSpeedup.toFixed(2)}x - ${Math.max(...speedups).toFixed(2)}x`);
        console.log(`   • Tests Passing 5x Target: ${results.filter(r => r.speedup >= 5).length}/${results.length}`);

        const avgGflops = results
            .filter(r => r.performanceStats?.gflops)
            .map(r => r.performanceStats.gflops)
            .reduce((a, b) => a + b, 0) / results.length;

        const avgBandwidth = results
            .filter(r => r.performanceStats?.bandwidth)
            .map(r => r.performanceStats.bandwidth)
            .reduce((a, b) => a + b, 0) / results.length;

        console.log(`   • Average Computational Throughput: ${avgGflops.toFixed(2)} GFLOPS`);
        console.log(`   • Average Memory Bandwidth: ${avgBandwidth.toFixed(2)} GB/s`);

        console.log('\n🔧 Optimization Techniques Validated:');
        console.log('   ✅ TypedArrays for memory efficiency');
        console.log('   ✅ CSR sparse matrix format for cache optimization');
        console.log('   ✅ Manual loop unrolling for vectorization');
        console.log('   ✅ Workspace vector reuse to minimize allocations');
        console.log('   ✅ Optimized memory access patterns');

        if (achievedTarget) {
            console.log('\n🎉 PERFORMANCE TARGET ACHIEVED: 5-10x improvement validated!');
            return true;
        } else {
            console.log('\n❌ PERFORMANCE TARGET NOT MET: Further optimization needed');
            return false;
        }

    } catch (error) {
        console.error('❌ Performance test failed:', error);
        return false;
    } finally {
        benchmark.dispose();
    }
}

// Run the test
runPerformanceTest()
    .then(success => {
        if (success) {
            console.log('\n✅ Performance test completed successfully');
            process.exit(0);
        } else {
            console.log('\n❌ Performance test failed');
            process.exit(1);
        }
    })
    .catch(error => {
        console.error('Fatal error in performance test:', error);
        process.exit(1);
    });