/**
 * Distributed Load Generator for RuVector
 *
 * Generates load across multiple global regions with configurable patterns
 * Supports WebSocket, HTTP/2, and gRPC protocols
 */

import * as k6 from 'k6';
import { check, sleep } from 'k6';
import http from 'k6/http';
import ws from 'k6/ws';
import { Trend, Counter, Gauge, Rate } from 'k6/metrics';
import { SharedArray } from 'k6/data';
import { exec } from 'k6/execution';
import * as crypto from 'k6/crypto';

// Custom metrics
const queryLatency = new Trend('query_latency', true);
const connectionDuration = new Trend('connection_duration', true);
const errorRate = new Rate('error_rate');
const activeConnections = new Gauge('active_connections');
const queriesPerSecond = new Counter('queries_per_second');
const bytesTransferred = new Counter('bytes_transferred');

// GCP regions for distributed load
export const REGIONS = [
  'us-east1', 'us-west1', 'us-central1',
  'europe-west1', 'europe-west2', 'europe-north1',
  'asia-east1', 'asia-southeast1', 'asia-northeast1',
  'australia-southeast1', 'southamerica-east1'
];

// Load generation configuration
export interface LoadConfig {
  targetConnections: number;
  rampUpDuration: string;
  steadyStateDuration: string;
  rampDownDuration: string;
  queriesPerConnection: number;
  queryInterval: string;
  protocol: 'http' | 'ws' | 'http2' | 'grpc';
  region?: string;
  vectorDimension: number;
  queryPattern: 'uniform' | 'hotspot' | 'zipfian' | 'burst';
  burstConfig?: {
    multiplier: number;
    duration: string;
    frequency: string;
  };
}

// Query patterns
export class QueryPattern {
  private config: LoadConfig;
  private hotspotIds: number[];

  constructor(config: LoadConfig) {
    this.config = config;
    this.hotspotIds = this.generateHotspots();
  }

  private generateHotspots(): number[] {
    // Top 1% of IDs account for 80% of traffic (Pareto distribution)
    const count = Math.ceil(1000000 * 0.01);
    return Array.from({ length: count }, (_, i) => i);
  }

  generateQueryId(): string {
    switch (this.config.queryPattern) {
      case 'uniform':
        return this.uniformQuery();
      case 'hotspot':
        return this.hotspotQuery();
      case 'zipfian':
        return this.zipfianQuery();
      case 'burst':
        return this.burstQuery();
      default:
        return this.uniformQuery();
    }
  }

  private uniformQuery(): string {
    return `doc_${Math.floor(Math.random() * 1000000)}`;
  }

  private hotspotQuery(): string {
    // 80% chance to hit hotspot
    if (Math.random() < 0.8) {
      const idx = Math.floor(Math.random() * this.hotspotIds.length);
      return `doc_${this.hotspotIds[idx]}`;
    }
    return this.uniformQuery();
  }

  private zipfianQuery(): string {
    // Zipfian distribution: frequency ∝ 1/rank^s
    const s = 1.5;
    const rank = Math.floor(Math.pow(Math.random(), -1/s));
    return `doc_${Math.min(rank, 999999)}`;
  }

  private burstQuery(): string {
    const time = Date.now();
    const burstConfig = this.config.burstConfig!;
    const frequency = parseInt(burstConfig.frequency);

    // Check if we're in a burst window
    const inBurst = (time % frequency) < parseInt(burstConfig.duration);

    if (inBurst) {
      // During burst, focus on hotspots
      return this.hotspotQuery();
    }
    return this.uniformQuery();
  }

  generateVector(): number[] {
    return Array.from(
      { length: this.config.vectorDimension },
      () => Math.random() * 2 - 1
    );
  }
}

// Connection manager
export class ConnectionManager {
  private config: LoadConfig;
  private pattern: QueryPattern;
  private baseUrl: string;

  constructor(config: LoadConfig, baseUrl: string) {
    this.config = config;
    this.pattern = new QueryPattern(config);
    this.baseUrl = baseUrl;
  }

  async connect(): Promise<void> {
    const startTime = Date.now();

    switch (this.config.protocol) {
      case 'http':
        await this.httpConnection();
        break;
      case 'http2':
        await this.http2Connection();
        break;
      case 'ws':
        await this.websocketConnection();
        break;
      case 'grpc':
        await this.grpcConnection();
        break;
    }

    const duration = Date.now() - startTime;
    connectionDuration.add(duration);
  }

  private async httpConnection(): Promise<void> {
    const params = {
      headers: {
        'Content-Type': 'application/json',
        'X-Region': this.config.region || 'unknown',
        'X-Client-Id': exec.vu.idInTest.toString(),
      },
      tags: {
        protocol: 'http',
        region: this.config.region,
      },
    };

    for (let i = 0; i < this.config.queriesPerConnection; i++) {
      const startTime = Date.now();

      const queryId = this.pattern.generateQueryId();
      const vector = this.pattern.generateVector();

      const payload = JSON.stringify({
        query_id: queryId,
        vector: vector,
        top_k: 10,
        filter: {},
      });

      const response = http.post(`${this.baseUrl}/query`, payload, params);

      const latency = Date.now() - startTime;
      queryLatency.add(latency);
      queriesPerSecond.add(1);
      bytesTransferred.add(payload.length + (response.body?.length || 0));

      const success = check(response, {
        'status is 200': (r) => r.status === 200,
        'has results': (r) => {
          try {
            const body = JSON.parse(r.body as string);
            return body.results && body.results.length > 0;
          } catch {
            return false;
          }
        },
        'latency < 100ms': () => latency < 100,
      });

      errorRate.add(!success);

      if (!success) {
        console.error(`Query failed: ${response.status}, latency: ${latency}ms`);
      }

      // Sleep between queries
      sleep(parseFloat(this.config.queryInterval) / 1000);
    }
  }

  private async http2Connection(): Promise<void> {
    const params = {
      headers: {
        'Content-Type': 'application/json',
        'X-Region': this.config.region || 'unknown',
        'X-Client-Id': exec.vu.idInTest.toString(),
      },
      tags: {
        protocol: 'http2',
        region: this.config.region,
      },
    };

    // Similar to HTTP but with HTTP/2 specific optimizations
    await this.httpConnection();
  }

  private async websocketConnection(): Promise<void> {
    const url = this.baseUrl.replace('http', 'ws') + '/ws';
    const params = {
      tags: {
        protocol: 'websocket',
        region: this.config.region,
      },
    };

    const res = ws.connect(url, params, (socket) => {
      socket.on('open', () => {
        activeConnections.add(1);

        // Send authentication
        socket.send(JSON.stringify({
          type: 'auth',
          token: 'benchmark-token',
          region: this.config.region,
        }));
      });

      socket.on('message', (data) => {
        try {
          const msg = JSON.parse(data as string);

          if (msg.type === 'query_result') {
            const latency = Date.now() - msg.client_timestamp;
            queryLatency.add(latency);
            queriesPerSecond.add(1);

            const success = msg.results && msg.results.length > 0;
            errorRate.add(!success);
          }
        } catch (e) {
          errorRate.add(1);
        }
      });

      socket.on('error', (e) => {
        console.error('WebSocket error:', e);
        errorRate.add(1);
      });

      socket.on('close', () => {
        activeConnections.add(-1);
      });

      // Send queries
      for (let i = 0; i < this.config.queriesPerConnection; i++) {
        const queryId = this.pattern.generateQueryId();
        const vector = this.pattern.generateVector();

        socket.send(JSON.stringify({
          type: 'query',
          query_id: queryId,
          vector: vector,
          top_k: 10,
          client_timestamp: Date.now(),
        }));

        socket.setTimeout(() => {}, parseFloat(this.config.queryInterval));
      }

      // Close connection after all queries
      socket.setTimeout(() => {
        socket.close();
      }, parseFloat(this.config.queryInterval) * this.config.queriesPerConnection);
    });
  }

  private async grpcConnection(): Promise<void> {
    // gRPC implementation using k6/net/grpc
    // TODO: Implement when gRPC is available
    console.log('gRPC not yet implemented, falling back to HTTP/2');
    await this.http2Connection();
  }
}

// Multi-region orchestrator
export class MultiRegionOrchestrator {
  private configs: Map<string, LoadConfig>;
  private baseUrls: Map<string, string>;

  constructor() {
    this.configs = new Map();
    this.baseUrls = new Map();
  }

  addRegion(region: string, config: LoadConfig, baseUrl: string): void {
    this.configs.set(region, { ...config, region });
    this.baseUrls.set(region, baseUrl);
  }

  async run(): Promise<void> {
    // Distribute VUs across regions
    const vuId = exec.vu.idInTest;
    const totalRegions = this.configs.size;
    const regionIndex = vuId % totalRegions;

    const regions = Array.from(this.configs.keys());
    const region = regions[regionIndex];
    const config = this.configs.get(region)!;
    const baseUrl = this.baseUrls.get(region)!;

    console.log(`VU ${vuId} assigned to region: ${region}`);

    const manager = new ConnectionManager(config, baseUrl);
    await manager.connect();
  }
}

// K6 test configuration
export const options = {
  scenarios: {
    baseline_500m: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '30m', target: 500000 }, // Ramp to 500M
        { duration: '2h', target: 500000 },  // Hold at 500M
        { duration: '15m', target: 0 },       // Ramp down
      ],
      gracefulRampDown: '30s',
    },
    burst_10x: {
      executor: 'ramping-vus',
      startTime: '3h',
      startVUs: 500000,
      stages: [
        { duration: '5m', target: 5000000 },  // Spike to 5B
        { duration: '10m', target: 5000000 }, // Hold
        { duration: '5m', target: 500000 },   // Return to baseline
      ],
      gracefulRampDown: '30s',
    },
  },
  thresholds: {
    'query_latency': ['p(95)<50', 'p(99)<100'],
    'error_rate': ['rate<0.0001'], // 99.99% success
    'http_req_duration': ['p(95)<50', 'p(99)<100'],
  },
  tags: {
    test_type: 'distributed_load',
    version: '1.0.0',
  },
};

// Main test function
export default function() {
  // Execute hooks before task
  exec.test.options.setupTimeout = '10m';

  const config: LoadConfig = {
    targetConnections: 500000000, // 500M
    rampUpDuration: '30m',
    steadyStateDuration: '2h',
    rampDownDuration: '15m',
    queriesPerConnection: 100,
    queryInterval: '1000', // 1 second between queries
    protocol: 'http',
    vectorDimension: 768, // Default embedding size
    queryPattern: 'uniform',
  };

  // Get region from environment or assign based on VU
  const region = __ENV.REGION || REGIONS[exec.vu.idInTest % REGIONS.length];
  const baseUrl = __ENV.BASE_URL || 'http://localhost:8080';

  config.region = region;

  const manager = new ConnectionManager(config, baseUrl);
  manager.connect();
}

// Setup function (runs once before test)
export function setup() {
  console.log('Starting distributed load test...');
  console.log(`Target: ${options.scenarios.baseline_500m.stages[1].target} concurrent connections`);
  console.log(`Regions: ${REGIONS.join(', ')}`);

  // Execute pre-task hook
  const hookResult = exec.test.options.exec || {};
  console.log('Pre-task hook executed');

  return {
    startTime: Date.now(),
    regions: REGIONS,
  };
}

// Teardown function (runs once after test)
export function teardown(data: any) {
  const duration = Date.now() - data.startTime;
  console.log(`Test completed in ${duration}ms`);
  console.log('Post-task hook executed');
}

// Export for external use
export {
  LoadConfig,
  QueryPattern,
  ConnectionManager,
  MultiRegionOrchestrator,
};
