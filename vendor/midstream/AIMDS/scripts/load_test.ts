#!/usr/bin/env tsx
/**
 * Load Testing Script for AIMDS Gateway
 *
 * Simulates realistic load patterns and measures performance metrics
 */

import http from 'http';
import { performance } from 'perf_hooks';

interface LoadTestConfig {
  baseUrl: string;
  totalRequests: number;
  concurrency: number;
  rampUpSeconds: number;
}

interface RequestResult {
  success: boolean;
  latency: number;
  statusCode?: number;
  error?: string;
}

interface LoadTestResults {
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
  totalDuration: number;
  requestsPerSecond: number;
  latencyStats: {
    min: number;
    max: number;
    mean: number;
    p50: number;
    p95: number;
    p99: number;
  };
}

class LoadTester {
  private config: LoadTestConfig;
  private results: RequestResult[] = [];

  constructor(config: LoadTestConfig) {
    this.config = config;
  }

  async run(): Promise<LoadTestResults> {
    console.log('🚀 Starting load test...');
    console.log(`   Target: ${this.config.baseUrl}`);
    console.log(`   Total requests: ${this.config.totalRequests}`);
    console.log(`   Concurrency: ${this.config.concurrency}`);
    console.log(`   Ramp-up: ${this.config.rampUpSeconds}s\n`);

    const startTime = performance.now();

    await this.executeLoadTest();

    const endTime = performance.now();
    const totalDuration = endTime - startTime;

    return this.calculateResults(totalDuration);
  }

  private async executeLoadTest(): Promise<void> {
    const batchSize = this.config.concurrency;
    const numBatches = Math.ceil(this.config.totalRequests / batchSize);
    const delayBetweenBatches = (this.config.rampUpSeconds * 1000) / numBatches;

    for (let batch = 0; batch < numBatches; batch++) {
      const batchRequests = Math.min(
        batchSize,
        this.config.totalRequests - batch * batchSize
      );

      const promises: Promise<RequestResult>[] = [];

      for (let i = 0; i < batchRequests; i++) {
        const requestType = Math.random();

        if (requestType < 0.95) {
          // 95% fast path requests
          promises.push(this.makeRequest({
            action: { type: 'read', resource: '/api/users', method: 'GET' },
            source: { ip: '192.168.1.1' },
          }));
        } else {
          // 5% deep path requests
          promises.push(this.makeRequest({
            action: { type: 'complex_operation' },
            source: { ip: '192.168.1.1' },
            behaviorSequence: this.generateBehaviorSequence(),
          }));
        }
      }

      const batchResults = await Promise.all(promises);
      this.results.push(...batchResults);

      const progress = ((batch + 1) / numBatches * 100).toFixed(1);
      process.stdout.write(`\r   Progress: ${progress}% (${this.results.length}/${this.config.totalRequests} requests)`);

      if (batch < numBatches - 1) {
        await this.sleep(delayBetweenBatches);
      }
    }

    console.log('\n');
  }

  private async makeRequest(payload: any): Promise<RequestResult> {
    const startTime = performance.now();

    return new Promise((resolve) => {
      const data = JSON.stringify(payload);

      const options = {
        hostname: 'localhost',
        port: 3000,
        path: '/api/v1/defend',
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Content-Length': data.length,
        },
      };

      const req = http.request(options, (res) => {
        let responseData = '';

        res.on('data', (chunk) => {
          responseData += chunk;
        });

        res.on('end', () => {
          const latency = performance.now() - startTime;
          resolve({
            success: res.statusCode === 200,
            latency,
            statusCode: res.statusCode,
          });
        });
      });

      req.on('error', (error) => {
        const latency = performance.now() - startTime;
        resolve({
          success: false,
          latency,
          error: error.message,
        });
      });

      req.write(data);
      req.end();
    });
  }

  private generateBehaviorSequence(): number[] {
    const length = 5;
    return Array.from({ length }, () => Math.random());
  }

  private calculateResults(totalDuration: number): LoadTestResults {
    const successful = this.results.filter(r => r.success);
    const latencies = successful.map(r => r.latency).sort((a, b) => a - b);

    const sum = latencies.reduce((a, b) => a + b, 0);
    const mean = sum / latencies.length;

    return {
      totalRequests: this.results.length,
      successfulRequests: successful.length,
      failedRequests: this.results.length - successful.length,
      totalDuration,
      requestsPerSecond: (this.results.length / totalDuration) * 1000,
      latencyStats: {
        min: latencies[0] || 0,
        max: latencies[latencies.length - 1] || 0,
        mean,
        p50: latencies[Math.floor(latencies.length * 0.5)] || 0,
        p95: latencies[Math.floor(latencies.length * 0.95)] || 0,
        p99: latencies[Math.floor(latencies.length * 0.99)] || 0,
      },
    };
  }

  private sleep(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }
}

function printResults(results: LoadTestResults): void {
  console.log('📊 Load Test Results\n');
  console.log('Overall:');
  console.log(`  Total requests:      ${results.totalRequests}`);
  console.log(`  Successful:          ${results.successfulRequests} (${(results.successfulRequests / results.totalRequests * 100).toFixed(1)}%)`);
  console.log(`  Failed:              ${results.failedRequests} (${(results.failedRequests / results.totalRequests * 100).toFixed(1)}%)`);
  console.log(`  Total duration:      ${results.totalDuration.toFixed(0)}ms`);
  console.log(`  Throughput:          ${results.requestsPerSecond.toFixed(0)} req/s`);
  console.log('');
  console.log('Latency (ms):');
  console.log(`  Min:                 ${results.latencyStats.min.toFixed(2)}`);
  console.log(`  Mean:                ${results.latencyStats.mean.toFixed(2)}`);
  console.log(`  p50:                 ${results.latencyStats.p50.toFixed(2)}`);
  console.log(`  p95:                 ${results.latencyStats.p95.toFixed(2)}`);
  console.log(`  p99:                 ${results.latencyStats.p99.toFixed(2)}`);
  console.log(`  Max:                 ${results.latencyStats.max.toFixed(2)}`);
  console.log('');

  // Performance targets
  console.log('Target Validation:');
  const throughputOk = results.requestsPerSecond >= 10000;
  const p95Ok = results.latencyStats.p95 < 35;
  const p99Ok = results.latencyStats.p99 < 100;
  const errorRateOk = (results.failedRequests / results.totalRequests) < 0.01;

  console.log(`  Throughput ≥10,000 req/s:  ${throughputOk ? '✅' : '❌'} (${results.requestsPerSecond.toFixed(0)})`);
  console.log(`  p95 latency <35ms:         ${p95Ok ? '✅' : '❌'} (${results.latencyStats.p95.toFixed(2)}ms)`);
  console.log(`  p99 latency <100ms:        ${p99Ok ? '✅' : '❌'} (${results.latencyStats.p99.toFixed(2)}ms)`);
  console.log(`  Error rate <1%:            ${errorRateOk ? '✅' : '❌'} (${(results.failedRequests / results.totalRequests * 100).toFixed(2)}%)`);
}

// Main execution
async function main() {
  const config: LoadTestConfig = {
    baseUrl: 'http://localhost:3000',
    totalRequests: parseInt(process.env.LOAD_TEST_REQUESTS || '1000'),
    concurrency: parseInt(process.env.LOAD_TEST_CONCURRENCY || '50'),
    rampUpSeconds: parseInt(process.env.LOAD_TEST_RAMP_UP || '5'),
  };

  const tester = new LoadTester(config);
  const results = await tester.run();
  printResults(results);

  // Exit with error code if targets not met
  const allTargetsMet =
    results.requestsPerSecond >= 10000 &&
    results.latencyStats.p95 < 35 &&
    results.latencyStats.p99 < 100 &&
    (results.failedRequests / results.totalRequests) < 0.01;

  process.exit(allTargetsMet ? 0 : 1);
}

if (require.main === module) {
  main().catch(error => {
    console.error('❌ Load test failed:', error);
    process.exit(1);
  });
}

export { LoadTester, LoadTestConfig, LoadTestResults };
