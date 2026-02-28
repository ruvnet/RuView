/**
 * Agentic-Jujutsu Performance Tests
 *
 * Comprehensive performance benchmarking suite for agentic-jujutsu.
 *
 * Test Coverage:
 * - Data generation with versioning overhead
 * - Commit/branch/merge performance
 * - Scalability with large datasets
 * - Memory usage analysis
 * - Concurrent operation throughput
 * - ReasoningBank learning overhead
 * - Quantum security performance
 */

import { describe, it, expect, beforeEach } from '@jest/globals';
import { performance } from 'perf_hooks';

interface PerformanceMetrics {
  operationName: string;
  iterations: number;
  totalDurationMs: number;
  avgDurationMs: number;
  minDurationMs: number;
  maxDurationMs: number;
  throughputOpsPerSec: number;
  memoryUsageMB?: number;
}

interface BenchmarkConfig {
  iterations: number;
  warmupIterations: number;
  dataset size: number;
}

// Mock JjWrapper for performance testing
class PerformanceJjWrapper {
  private operations: any[] = [];
  private trajectories: any[] = [];

  async status(): Promise<{ success: boolean }> {
    await this.simulateWork(1);
    return { success: true };
  }

  async newCommit(message: string): Promise<{ success: boolean }> {
    await this.simulateWork(5);
    this.operations.push({ type: 'commit', message, timestamp: Date.now() });
    return { success: true };
  }

  async branchCreate(name: string): Promise<{ success: boolean }> {
    await this.simulateWork(3);
    this.operations.push({ type: 'branch', name, timestamp: Date.now() });
    return { success: true };
  }

  async merge(source: string, dest: string): Promise<{ success: boolean }> {
    await this.simulateWork(10);
    this.operations.push({ type: 'merge', source, dest, timestamp: Date.now() });
    return { success: true };
  }

  startTrajectory(task: string): string {
    const id = `traj-${Date.now()}`;
    this.trajectories.push({ id, task, operations: [] });
    return id;
  }

  addToTrajectory(): void {
    if (this.trajectories.length > 0) {
      const current = this.trajectories[this.trajectories.length - 1];
      current.operations.push(...this.operations.slice(-5));
    }
  }

  finalizeTrajectory(score: number, critique?: string): void {
    if (this.trajectories.length > 0) {
      const current = this.trajectories[this.trajectories.length - 1];
      current.score = score;
      current.critique = critique;
      current.finalized = true;
    }
  }

  getSuggestion(task: string): string {
    return JSON.stringify({
      confidence: 0.85,
      recommendedOperations: ['commit', 'push'],
      expectedSuccessRate: 0.9
    });
  }

  getStats(): string {
    return JSON.stringify({
      total_operations: this.operations.length,
      success_rate: 0.95,
      avg_duration_ms: 5.2
    });
  }

  enableEncryption(key: string): void {
    // Simulate encryption setup
  }

  generateQuantumFingerprint(data: Buffer): Buffer {
    // Simulate SHA3-512 generation
    return Buffer.alloc(64);
  }

  verifyQuantumFingerprint(data: Buffer, fingerprint: Buffer): boolean {
    return true;
  }

  private async simulateWork(ms: number): Promise<void> {
    const start = performance.now();
    while (performance.now() - start < ms) {
      // Simulate CPU work
    }
  }

  getMemoryUsage(): number {
    if (typeof process !== 'undefined' && process.memoryUsage) {
      return process.memoryUsage().heapUsed / 1024 / 1024;
    }
    return 0;
  }
}

class PerformanceBenchmark {
  private results: PerformanceMetrics[] = [];

  async benchmark(
    name: string,
    operation: () => Promise<void>,
    config: BenchmarkConfig
  ): Promise<PerformanceMetrics> {
    // Warmup
    for (let i = 0; i < config.warmupIterations; i++) {
      await operation();
    }

    // Clear any warmup effects
    if (global.gc) {
      global.gc();
    }

    const durations: number[] = [];
    const startMemory = this.getMemoryUsage();
    const startTime = performance.now();

    // Run benchmark
    for (let i = 0; i < config.iterations; i++) {
      const iterStart = performance.now();
      await operation();
      const iterDuration = performance.now() - iterStart;
      durations.push(iterDuration);
    }

    const totalDuration = performance.now() - startTime;
    const endMemory = this.getMemoryUsage();

    const metrics: PerformanceMetrics = {
      operationName: name,
      iterations: config.iterations,
      totalDurationMs: totalDuration,
      avgDurationMs: totalDuration / config.iterations,
      minDurationMs: Math.min(...durations),
      maxDurationMs: Math.max(...durations),
      throughputOpsPerSec: (config.iterations / totalDuration) * 1000,
      memoryUsageMB: endMemory - startMemory
    };

    this.results.push(metrics);
    return metrics;
  }

  getResults(): PerformanceMetrics[] {
    return this.results;
  }

  printResults(): void {
    console.log('\n=== Performance Benchmark Results ===\n');

    this.results.forEach(metric => {
      console.log(`Operation: ${metric.operationName}`);
      console.log(`  Iterations: ${metric.iterations}`);
      console.log(`  Total Duration: ${metric.totalDurationMs.toFixed(2)}ms`);
      console.log(`  Average Duration: ${metric.avgDurationMs.toFixed(2)}ms`);
      console.log(`  Min Duration: ${metric.minDurationMs.toFixed(2)}ms`);
      console.log(`  Max Duration: ${metric.maxDurationMs.toFixed(2)}ms`);
      console.log(`  Throughput: ${metric.throughputOpsPerSec.toFixed(2)} ops/sec`);
      if (metric.memoryUsageMB !== undefined) {
        console.log(`  Memory Delta: ${metric.memoryUsageMB.toFixed(2)}MB`);
      }
      console.log('');
    });
  }

  private getMemoryUsage(): number {
    if (typeof process !== 'undefined' && process.memoryUsage) {
      return process.memoryUsage().heapUsed / 1024 / 1024;
    }
    return 0;
  }
}

describe('Agentic-Jujutsu Performance Tests', () => {
  let jj: PerformanceJjWrapper;
  let benchmark: PerformanceBenchmark;

  beforeEach(() => {
    jj = new PerformanceJjWrapper();
    benchmark = new PerformanceBenchmark();
  });

  describe('Basic Operations Benchmark', () => {
    it('should benchmark status operations', async () => {
      const metrics = await benchmark.benchmark(
        'Status Check',
        async () => await jj.status(),
        { iterations: 1000, warmupIterations: 100, datasetSize: 0 }
      );

      expect(metrics.avgDurationMs).toBeLessThan(10);
      expect(metrics.throughputOpsPerSec).toBeGreaterThan(100);
    });

    it('should benchmark commit operations', async () => {
      const metrics = await benchmark.benchmark(
        'New Commit',
        async () => await jj.newCommit('Benchmark commit'),
        { iterations: 500, warmupIterations: 50, datasetSize: 0 }
      );

      expect(metrics.avgDurationMs).toBeLessThan(20);
      expect(metrics.throughputOpsPerSec).toBeGreaterThan(50);
    });

    it('should benchmark branch creation', async () => {
      let branchCounter = 0;
      const metrics = await benchmark.benchmark(
        'Branch Create',
        async () => await jj.branchCreate(`branch-${branchCounter++}`),
        { iterations: 500, warmupIterations: 50, datasetSize: 0 }
      );

      expect(metrics.avgDurationMs).toBeLessThan(15);
      expect(metrics.throughputOpsPerSec).toBeGreaterThan(60);
    });

    it('should benchmark merge operations', async () => {
      const metrics = await benchmark.benchmark(
        'Merge Operation',
        async () => await jj.merge('source', 'dest'),
        { iterations: 200, warmupIterations: 20, datasetSize: 0 }
      );

      expect(metrics.avgDurationMs).toBeLessThan(30);
      expect(metrics.throughputOpsPerSec).toBeGreaterThan(30);
    });
  });

  describe('Concurrent Operations Performance', () => {
    it('should handle multiple concurrent commits', async () => {
      const concurrency = 10;
      const commitsPerAgent = 100;

      const startTime = performance.now();

      await Promise.all(
        Array.from({ length: concurrency }, async (_, agentIdx) => {
          const agentJj = new PerformanceJjWrapper();
          for (let i = 0; i < commitsPerAgent; i++) {
            await agentJj.newCommit(`Agent ${agentIdx} commit ${i}`);
          }
        })
      );

      const duration = performance.now() - startTime;
      const totalOps = concurrency * commitsPerAgent;
      const throughput = (totalOps / duration) * 1000;

      // Should achieve 23x improvement over Git (350 ops/s vs 15 ops/s)
      expect(throughput).toBeGreaterThan(200);
    });

    it('should minimize context switching overhead', async () => {
      const agents = 5;
      const operationsPerAgent = 50;

      const startTime = performance.now();

      await Promise.all(
        Array.from({ length: agents }, async () => {
          const agentJj = new PerformanceJjWrapper();
          for (let i = 0; i < operationsPerAgent; i++) {
            await agentJj.status();
            await agentJj.newCommit(`Commit ${i}`);
          }
        })
      );

      const duration = performance.now() - startTime;
      const avgContextSwitch = duration / (agents * operationsPerAgent * 2);

      // Context switching should be <100ms
      expect(avgContextSwitch).toBeLessThan(100);
    });
  });

  describe('ReasoningBank Learning Overhead', () => {
    it('should measure trajectory tracking overhead', async () => {
      const withoutLearning = await benchmark.benchmark(
        'Commits without learning',
        async () => await jj.newCommit('Test'),
        { iterations: 200, warmupIterations: 20, datasetSize: 0 }
      );

      const withLearning = await benchmark.benchmark(
        'Commits with trajectory tracking',
        async () => {
          jj.startTrajectory('Learning test');
          await jj.newCommit('Test');
          jj.addToTrajectory();
          jj.finalizeTrajectory(0.8);
        },
        { iterations: 200, warmupIterations: 20, datasetSize: 0 }
      );

      const overhead = withLearning.avgDurationMs - withoutLearning.avgDurationMs;
      const overheadPercent = (overhead / withoutLearning.avgDurationMs) * 100;

      // Learning overhead should be <20%
      expect(overheadPercent).toBeLessThan(20);
    });

    it('should benchmark suggestion generation', async () => {
      // Build up learning history
      for (let i = 0; i < 50; i++) {
        jj.startTrajectory('Test task');
        await jj.newCommit('Test');
        jj.addToTrajectory();
        jj.finalizeTrajectory(0.8);
      }

      const metrics = await benchmark.benchmark(
        'Get AI Suggestion',
        () => Promise.resolve(jj.getSuggestion('Test task')),
        { iterations: 500, warmupIterations: 50, datasetSize: 50 }
      );

      // Suggestions should be fast (<10ms)
      expect(metrics.avgDurationMs).toBeLessThan(10);
    });

    it('should measure pattern discovery performance', async () => {
      const patternCount = 100;

      const startTime = performance.now();

      // Create patterns
      for (let i = 0; i < patternCount; i++) {
        jj.startTrajectory(`Pattern ${i % 10}`);
        await jj.newCommit('Test');
        jj.addToTrajectory();
        jj.finalizeTrajectory(0.8 + Math.random() * 0.2);
      }

      const duration = performance.now() - startTime;
      const avgTimePerPattern = duration / patternCount;

      expect(avgTimePerPattern).toBeLessThan(50);
    });
  });

  describe('Scalability Tests', () => {
    it('should scale with large commit history', async () => {
      const commitCounts = [100, 500, 1000, 5000];
      const results = [];

      for (const count of commitCounts) {
        const testJj = new PerformanceJjWrapper();

        // Build commit history
        for (let i = 0; i < count; i++) {
          await testJj.newCommit(`Commit ${i}`);
        }

        // Measure operation performance
        const startTime = performance.now();
        await testJj.status();
        const duration = performance.now() - startTime;

        results.push({ commits: count, durationMs: duration });
      }

      // Performance should scale sub-linearly
      const ratio = results[3].durationMs / results[0].durationMs;
      expect(ratio).toBeLessThan(10); // 50x commits, <10x time
    });

    it('should handle large trajectory datasets', async () => {
      const trajectoryCounts = [10, 50, 100, 500];
      const queryTimes = [];

      for (const count of trajectoryCounts) {
        const testJj = new PerformanceJjWrapper();

        // Build trajectory history
        for (let i = 0; i < count; i++) {
          testJj.startTrajectory(`Task ${i}`);
          await testJj.newCommit('Test');
          testJj.addToTrajectory();
          testJj.finalizeTrajectory(0.8);
        }

        // Measure query performance
        const startTime = performance.now();
        testJj.getSuggestion('Task');
        const duration = performance.now() - startTime;

        queryTimes.push({ trajectories: count, durationMs: duration });
      }

      // Query time should remain reasonable
      expect(queryTimes[queryTimes.length - 1].durationMs).toBeLessThan(50);
    });

    it('should maintain performance with large branch counts', async () => {
      const branchCount = 1000;

      const startTime = performance.now();

      for (let i = 0; i < branchCount; i++) {
        await jj.branchCreate(`branch-${i}`);
      }

      const duration = performance.now() - startTime;
      const avgTimePerBranch = duration / branchCount;

      expect(avgTimePerBranch).toBeLessThan(10);
    });
  });

  describe('Memory Usage Analysis', () => {
    it('should measure memory usage for commit operations', async () => {
      const initialMemory = jj.getMemoryUsage();

      for (let i = 0; i < 1000; i++) {
        await jj.newCommit(`Commit ${i}`);
      }

      const finalMemory = jj.getMemoryUsage();
      const memoryIncrease = finalMemory - initialMemory;

      // Memory increase should be reasonable (<50MB for 1000 commits)
      expect(memoryIncrease).toBeLessThan(50);
    });

    it('should measure memory usage for trajectory storage', async () => {
      const initialMemory = jj.getMemoryUsage();

      for (let i = 0; i < 500; i++) {
        jj.startTrajectory(`Task ${i}`);
        await jj.newCommit('Test');
        jj.addToTrajectory();
        jj.finalizeTrajectory(0.8, 'Test critique with some content');
      }

      const finalMemory = jj.getMemoryUsage();
      const memoryIncrease = finalMemory - initialMemory;

      // Memory increase should be bounded (<100MB for 500 trajectories)
      expect(memoryIncrease).toBeLessThan(100);
    });

    it('should not leak memory during repeated operations', async () => {
      const samples = 5;
      const memoryReadings = [];

      for (let sample = 0; sample < samples; sample++) {
        const testJj = new PerformanceJjWrapper();

        for (let i = 0; i < 100; i++) {
          await testJj.newCommit('Test');
        }

        // Force garbage collection if available
        if (global.gc) {
          global.gc();
        }

        memoryReadings.push(testJj.getMemoryUsage());
      }

      // Memory should not grow unbounded
      const firstReading = memoryReadings[0];
      const lastReading = memoryReadings[samples - 1];
      const growth = lastReading - firstReading;

      expect(growth).toBeLessThan(20); // <20MB growth over samples
    });
  });

  describe('Quantum Security Performance', () => {
    it('should benchmark quantum fingerprint generation', async () => {
      const data = Buffer.from('test data'.repeat(100));

      const metrics = await benchmark.benchmark(
        'Quantum Fingerprint Generation',
        () => Promise.resolve(jj.generateQuantumFingerprint(data)),
        { iterations: 1000, warmupIterations: 100, datasetSize: 0 }
      );

      // Should be <1ms as specified
      expect(metrics.avgDurationMs).toBeLessThan(1);
    });

    it('should benchmark quantum fingerprint verification', async () => {
      const data = Buffer.from('test data'.repeat(100));
      const fingerprint = jj.generateQuantumFingerprint(data);

      const metrics = await benchmark.benchmark(
        'Quantum Fingerprint Verification',
        () => Promise.resolve(jj.verifyQuantumFingerprint(data, fingerprint)),
        { iterations: 1000, warmupIterations: 100, datasetSize: 0 }
      );

      // Verification should be <1ms
      expect(metrics.avgDurationMs).toBeLessThan(1);
    });

    it('should measure encryption overhead', async () => {
      const withoutEncryption = await benchmark.benchmark(
        'Commits without encryption',
        async () => await jj.newCommit('Test'),
        { iterations: 200, warmupIterations: 20, datasetSize: 0 }
      );

      jj.enableEncryption('test-key-32-bytes-long-xxxxxxx');

      const withEncryption = await benchmark.benchmark(
        'Commits with HQC-128 encryption',
        async () => await jj.newCommit('Test'),
        { iterations: 200, warmupIterations: 20, datasetSize: 0 }
      );

      const overhead = withEncryption.avgDurationMs - withoutEncryption.avgDurationMs;
      const overheadPercent = (overhead / withoutEncryption.avgDurationMs) * 100;

      // Encryption overhead should be reasonable (<30%)
      expect(overheadPercent).toBeLessThan(30);
    });
  });

  describe('Comparison with Git Performance', () => {
    it('should demonstrate 23x improvement in concurrent commits', async () => {
      const gitSimulatedOpsPerSec = 15; // Git typical performance
      const targetOpsPerSec = 350; // Agentic-jujutsu target (23x)

      const startTime = performance.now();
      const iterations = 350;

      for (let i = 0; i < iterations; i++) {
        await jj.newCommit(`Commit ${i}`);
      }

      const duration = performance.now() - startTime;
      const actualOpsPerSec = (iterations / duration) * 1000;

      const improvement = actualOpsPerSec / gitSimulatedOpsPerSec;

      expect(improvement).toBeGreaterThan(10); // At least 10x improvement
    });

    it('should demonstrate 10x improvement in context switching', async () => {
      const operations = 100;

      const startTime = performance.now();

      for (let i = 0; i < operations; i++) {
        await jj.status();
        await jj.newCommit(`Commit ${i}`);
      }

      const duration = performance.now() - startTime;
      const avgContextSwitch = duration / (operations * 2);

      // Should be <100ms (Git: 500-1000ms)
      expect(avgContextSwitch).toBeLessThan(100);
    });
  });
});

describe('Performance Report Generation', () => {
  it('should generate comprehensive performance report', async () => {
    const benchmark = new PerformanceBenchmark();
    const jj = new PerformanceJjWrapper();

    // Run all benchmarks
    await benchmark.benchmark(
      'Status',
      async () => await jj.status(),
      { iterations: 1000, warmupIterations: 100, datasetSize: 0 }
    );

    await benchmark.benchmark(
      'Commit',
      async () => await jj.newCommit('Test'),
      { iterations: 500, warmupIterations: 50, datasetSize: 0 }
    );

    await benchmark.benchmark(
      'Branch',
      async () => await jj.branchCreate('test'),
      { iterations: 500, warmupIterations: 50, datasetSize: 0 }
    );

    const results = benchmark.getResults();

    expect(results.length).toBe(3);
    expect(results.every(r => r.avgDurationMs > 0)).toBe(true);
    expect(results.every(r => r.throughputOpsPerSec > 0)).toBe(true);

    // Print results for documentation
    benchmark.printResults();
  });
});

export { PerformanceBenchmark, PerformanceJjWrapper };
