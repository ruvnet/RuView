/**
 * Performance Benchmarks for AIMDS Gateway
 */

import { describe, bench, beforeAll, afterAll } from 'vitest';
import { AIMDSGateway } from '../../src/gateway/server';
import { Config } from '../../src/utils/config';
import { AIMDSRequest } from '../../src/types';

describe('Performance Benchmarks', () => {
  let gateway: AIMDSGateway;

  beforeAll(async () => {
    const config = Config.getInstance();
    gateway = new AIMDSGateway(
      config.getGatewayConfig(),
      config.getAgentDBConfig(),
      config.getLeanAgenticConfig()
    );
    await gateway.initialize();
  });

  afterAll(async () => {
    await gateway.shutdown();
  });

  const createBenignRequest = (): AIMDSRequest => ({
    id: `bench-${Math.random().toString(36).substr(2, 9)}`,
    timestamp: Date.now(),
    source: {
      ip: '192.168.1.1',
      userAgent: 'benchmark-client',
      headers: {}
    },
    action: {
      type: 'read',
      resource: '/api/data',
      method: 'GET'
    }
  });

  const createSuspiciousRequest = (): AIMDSRequest => ({
    id: `bench-${Math.random().toString(36).substr(2, 9)}`,
    timestamp: Date.now(),
    source: {
      ip: '10.0.0.1',
      userAgent: 'suspicious-client',
      headers: {}
    },
    action: {
      type: 'admin',
      resource: '/api/admin/delete',
      method: 'DELETE',
      payload: { force: true }
    }
  });

  bench('Fast path - benign request (<10ms target)', async () => {
    const request = createBenignRequest();
    await gateway.processRequest(request);
  }, { iterations: 1000 });

  bench('Deep path - suspicious request (<520ms target)', async () => {
    const request = createSuspiciousRequest();
    await gateway.processRequest(request);
  }, { iterations: 100 });

  bench('Throughput - concurrent requests (>10,000 req/s target)', async () => {
    const requests = Array(100).fill(null).map(() => createBenignRequest());
    await Promise.all(requests.map(r => gateway.processRequest(r)));
  }, { iterations: 100 });

  bench('Vector search latency (<2ms target)', async () => {
    const request = createBenignRequest();
    const result = await gateway.processRequest(request);
    // Verify vector search time in result.metadata.vectorSearchTime
  }, { iterations: 1000 });
});
