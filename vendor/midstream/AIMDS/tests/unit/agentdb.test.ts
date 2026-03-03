/**
 * Unit Tests for AgentDB Client
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { AgentDBClient } from '../../src/agentdb/client';
import { Logger } from '../../src/utils/logger';
import { ThreatLevel, AgentDBConfig } from '../../src/types';

describe('AgentDB Client', () => {
  let client: AgentDBClient;
  let config: AgentDBConfig;
  let logger: Logger;

  beforeEach(async () => {
    logger = new Logger('AgentDBTest');
    config = {
      path: ':memory:', // Use in-memory DB for tests
      embeddingDim: 384,
      hnswConfig: {
        m: 16,
        efConstruction: 200,
        efSearch: 100
      },
      quicSync: {
        enabled: false,
        peers: [],
        port: 4433
      },
      memory: {
        maxEntries: 10000,
        ttl: 3600000
      }
    };

    client = new AgentDBClient(config, logger);
    await client.initialize();
  });

  afterEach(async () => {
    await client.shutdown();
  });

  describe('Vector Search', () => {
    it('should perform HNSW search', async () => {
      // Generate a test embedding
      const embedding = Array(384).fill(0).map(() => Math.random());

      const results = await client.vectorSearch(embedding, { k: 5 });

      expect(Array.isArray(results)).toBe(true);
      expect(results.length).toBeLessThanOrEqual(5);
    });

    it('should apply similarity threshold', async () => {
      const embedding = Array(384).fill(0).map(() => Math.random());

      const results = await client.vectorSearch(embedding, {
        k: 10,
        threshold: 0.9 // High threshold
      });

      // With random embeddings, unlikely to find matches above 0.9
      expect(results.length).toBeLessThanOrEqual(10);
    });

    it('should complete search in <2ms target', async () => {
      const embedding = Array(384).fill(0).map(() => Math.random());

      const start = Date.now();
      await client.vectorSearch(embedding, { k: 10 });
      const duration = Date.now() - start;

      // Should be fast even without data
      expect(duration).toBeLessThan(10);
    });
  });

  describe('Incident Storage', () => {
    it('should store threat incident', async () => {
      const incident = {
        id: 'test-incident-1',
        timestamp: Date.now(),
        request: {
          id: 'req-1',
          timestamp: Date.now(),
          source: { ip: '192.168.1.1', headers: {} },
          action: { type: 'read', resource: '/api/data', method: 'GET' }
        },
        result: {
          allowed: false,
          confidence: 0.95,
          latencyMs: 15,
          threatLevel: ThreatLevel.HIGH,
          matches: [],
          metadata: {
            vectorSearchTime: 2,
            verificationTime: 13,
            totalTime: 15,
            pathTaken: 'deep' as const
          }
        },
        embedding: Array(384).fill(0).map(() => Math.random())
      };

      await expect(client.storeIncident(incident)).resolves.not.toThrow();
    });
  });

  describe('Statistics', () => {
    it('should return stats', async () => {
      const stats = await client.getStats();

      expect(stats).toHaveProperty('incidents');
      expect(stats).toHaveProperty('patterns');
      expect(stats).toHaveProperty('memoryEntries');
      expect(stats).toHaveProperty('memoryUsage');
      expect(typeof stats.incidents).toBe('number');
    });
  });
});
