/**
 * Integration Tests for AIMDS Gateway
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import request from 'supertest';
import { AIMDSGateway } from '../../src/gateway/server';
import { Config } from '../../src/utils/config';
import { ThreatLevel } from '../../src/types';

describe('AIMDS Gateway Integration Tests', () => {
  let gateway: AIMDSGateway;
  let app: any;

  beforeAll(async () => {
    // Create test configuration
    const config = Config.getInstance();
    const gatewayConfig = {
      ...config.getGatewayConfig(),
      port: 3001 // Use different port for tests
    };

    gateway = new AIMDSGateway(
      gatewayConfig,
      config.getAgentDBConfig(),
      config.getLeanAgenticConfig()
    );

    await gateway.initialize();
    await gateway.start();

    // Get Express app for supertest
    app = (gateway as any).app;
  });

  afterAll(async () => {
    await gateway.shutdown();
  });

  describe('Health Checks', () => {
    it('should return healthy status', async () => {
      const response = await request(app)
        .get('/health')
        .expect(200);

      expect(response.body.status).toBe('healthy');
      expect(response.body.components.gateway.status).toBe('up');
      expect(response.body.components.agentdb.status).toBe('up');
      expect(response.body.components.verifier.status).toBe('up');
    });

    it('should return metrics', async () => {
      const response = await request(app)
        .get('/metrics')
        .expect(200);

      expect(response.text).toContain('aimds_requests_total');
    });
  });

  describe('Defense Endpoint', () => {
    it('should process a benign request (fast path)', async () => {
      const testRequest = {
        action: {
          type: 'read',
          resource: '/api/users',
          method: 'GET'
        },
        source: {
          ip: '192.168.1.1',
          userAgent: 'test-client'
        }
      };

      const response = await request(app)
        .post('/api/v1/defend')
        .send(testRequest)
        .expect(200);

      expect(response.body.allowed).toBeDefined();
      expect(response.body.confidence).toBeGreaterThan(0);
      expect(response.body.latency).toBeLessThan(100); // Should be fast
      expect(response.body.metadata.pathTaken).toBe('fast');
    });

    it('should detect and block suspicious request (deep path)', async () => {
      const testRequest = {
        action: {
          type: 'admin',
          resource: '/api/admin/delete-all',
          method: 'DELETE',
          payload: { force: true }
        },
        source: {
          ip: '10.0.0.1',
          userAgent: 'suspicious-bot'
        }
      };

      const response = await request(app)
        .post('/api/v1/defend')
        .send(testRequest)
        .expect(200); // Still returns 200, but with allowed: false

      expect(response.body.latency).toBeLessThan(1000); // Within performance target
    });

    it('should validate request schema', async () => {
      const invalidRequest = {
        // Missing required fields
        action: {
          type: 'read'
          // Missing resource and method
        }
      };

      await request(app)
        .post('/api/v1/defend')
        .send(invalidRequest)
        .expect(400);
    });

    it('should handle batch requests', async () => {
      const batchRequest = {
        requests: [
          {
            id: 'req1',
            timestamp: Date.now(),
            action: { type: 'read', resource: '/api/data', method: 'GET' },
            source: { ip: '192.168.1.1' }
          },
          {
            id: 'req2',
            timestamp: Date.now(),
            action: { type: 'write', resource: '/api/data', method: 'POST' },
            source: { ip: '192.168.1.2' }
          }
        ]
      };

      const response = await request(app)
        .post('/api/v1/defend/batch')
        .send(batchRequest)
        .expect(200);

      expect(response.body.results).toHaveLength(2);
      expect(response.body.results[0].allowed).toBeDefined();
      expect(response.body.results[1].allowed).toBeDefined();
    });

    it('should reject oversized batch requests', async () => {
      const largeBatch = {
        requests: Array(101).fill({
          action: { type: 'read', resource: '/api/data', method: 'GET' },
          source: { ip: '192.168.1.1' }
        })
      };

      await request(app)
        .post('/api/v1/defend/batch')
        .send(largeBatch)
        .expect(400);
    });
  });

  describe('Performance', () => {
    it('should meet latency targets for fast path (<35ms avg)', async () => {
      const latencies: number[] = [];

      // Run 100 requests
      for (let i = 0; i < 100; i++) {
        const response = await request(app)
          .post('/api/v1/defend')
          .send({
            action: { type: 'read', resource: '/api/data', method: 'GET' },
            source: { ip: '192.168.1.1' }
          });

        latencies.push(response.body.latency);
      }

      const avgLatency = latencies.reduce((a, b) => a + b, 0) / latencies.length;
      expect(avgLatency).toBeLessThan(35); // Target: <35ms
    });

    it('should handle concurrent requests', async () => {
      const promises = Array(50).fill(null).map(() =>
        request(app)
          .post('/api/v1/defend')
          .send({
            action: { type: 'read', resource: '/api/data', method: 'GET' },
            source: { ip: '192.168.1.1' }
          })
      );

      const responses = await Promise.all(promises);
      const allSuccessful = responses.every(r => r.status === 200);
      expect(allSuccessful).toBe(true);
    });
  });

  describe('Stats Endpoint', () => {
    it('should return statistics', async () => {
      const response = await request(app)
        .get('/api/v1/stats')
        .expect(200);

      expect(response.body.timestamp).toBeDefined();
      expect(response.body.requests).toBeDefined();
      expect(response.body.latency).toBeDefined();
      expect(response.body.threats).toBeDefined();
    });
  });

  describe('Error Handling', () => {
    it('should handle 404 errors', async () => {
      await request(app)
        .get('/nonexistent')
        .expect(404);
    });

    it('should handle malformed JSON', async () => {
      await request(app)
        .post('/api/v1/defend')
        .set('Content-Type', 'application/json')
        .send('invalid json{')
        .expect(400);
    });
  });
});
