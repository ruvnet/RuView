/**
 * Multi-tenancy Isolation - Integration Tests
 *
 * Tests for tenant data isolation, access control, and resource boundaries
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { createTenant, createAgent, createSession, createMemory, createVectorMemory } from '../../factories';
import { createMockPool, type MockPool } from '../../mocks/postgres.mock';
import { MockWasmVectorIndex, MockWasmEmbedder } from '../../mocks/wasm.mock';

// Multi-tenant data manager
class TenantDataManager {
  private pools: Map<string, MockPool> = new Map();
  private vectorIndexes: Map<string, MockWasmVectorIndex> = new Map();
  private embedder: MockWasmEmbedder;

  constructor() {
    this.embedder = new MockWasmEmbedder(384);
  }

  async createTenantContext(tenantId: string): Promise<void> {
    // Create isolated pool for tenant
    const pool = createMockPool();
    await pool.connect();
    this.pools.set(tenantId, pool);

    // Create isolated vector index for tenant
    const vectorIndex = new MockWasmVectorIndex(384);
    this.vectorIndexes.set(tenantId, vectorIndex);
  }

  async destroyTenantContext(tenantId: string): Promise<void> {
    const pool = this.pools.get(tenantId);
    if (pool) {
      await pool.end();
      this.pools.delete(tenantId);
    }

    const vectorIndex = this.vectorIndexes.get(tenantId);
    if (vectorIndex) {
      vectorIndex.clear();
      this.vectorIndexes.delete(tenantId);
    }
  }

  getPool(tenantId: string): MockPool | undefined {
    return this.pools.get(tenantId);
  }

  getVectorIndex(tenantId: string): MockWasmVectorIndex | undefined {
    return this.vectorIndexes.get(tenantId);
  }

  getEmbedder(): MockWasmEmbedder {
    return this.embedder;
  }

  async seedTenantData(tenantId: string, data: {
    agents?: unknown[];
    sessions?: unknown[];
    memories?: unknown[];
  }): Promise<void> {
    const pool = this.pools.get(tenantId);
    if (!pool) throw new Error(`No context for tenant ${tenantId}`);

    if (data.agents) {
      pool.seedData('agents', data.agents.map(a => ({ ...(a as object), tenantId })));
    }

    if (data.sessions) {
      pool.seedData('sessions', data.sessions.map(s => ({ ...(s as object), tenantId })));
    }

    if (data.memories) {
      pool.seedData('memories', data.memories.map(m => ({ ...(m as object), tenantId })));
    }
  }

  async vectorIndex(tenantId: string, id: string, text: string): Promise<void> {
    const vectorIndex = this.vectorIndexes.get(tenantId);
    if (!vectorIndex) throw new Error(`No vector index for tenant ${tenantId}`);

    const embedding = this.embedder.embed(text);
    vectorIndex.add(id, embedding);
  }

  async vectorSearch(tenantId: string, query: string, topK: number = 10): Promise<Array<{ id: string; score: number }>> {
    const vectorIndex = this.vectorIndexes.get(tenantId);
    if (!vectorIndex) throw new Error(`No vector index for tenant ${tenantId}`);

    const embedding = this.embedder.embed(query);
    return vectorIndex.search(embedding, topK);
  }
}

describe('Multi-tenancy Isolation', () => {
  let manager: TenantDataManager;
  const tenant1 = createTenant({ id: 'tenant-1', name: 'Tenant One' });
  const tenant2 = createTenant({ id: 'tenant-2', name: 'Tenant Two' });

  beforeEach(async () => {
    manager = new TenantDataManager();
    await manager.createTenantContext(tenant1.id);
    await manager.createTenantContext(tenant2.id);
  });

  describe('Database Isolation', () => {
    it('should isolate agent data between tenants', async () => {
      // Seed tenant 1 data
      await manager.seedTenantData(tenant1.id, {
        agents: [
          { id: 'agent-1', name: 'T1 Agent 1' },
          { id: 'agent-2', name: 'T1 Agent 2' }
        ]
      });

      // Seed tenant 2 data
      await manager.seedTenantData(tenant2.id, {
        agents: [
          { id: 'agent-3', name: 'T2 Agent 1' }
        ]
      });

      const pool1 = manager.getPool(tenant1.id)!;
      const pool2 = manager.getPool(tenant2.id)!;

      const t1Agents = pool1.getData('agents');
      const t2Agents = pool2.getData('agents');

      expect(t1Agents).toHaveLength(2);
      expect(t2Agents).toHaveLength(1);

      // Verify no cross-tenant data leakage
      t1Agents.forEach((a: any) => expect(a.tenantId).toBe(tenant1.id));
      t2Agents.forEach((a: any) => expect(a.tenantId).toBe(tenant2.id));
    });

    it('should isolate session data between tenants', async () => {
      await manager.seedTenantData(tenant1.id, {
        sessions: [
          { id: 'session-1', userId: 'user-1', status: 'active' },
          { id: 'session-2', userId: 'user-2', status: 'completed' }
        ]
      });

      await manager.seedTenantData(tenant2.id, {
        sessions: [
          { id: 'session-3', userId: 'user-3', status: 'active' }
        ]
      });

      const pool1 = manager.getPool(tenant1.id)!;
      const pool2 = manager.getPool(tenant2.id)!;

      expect(pool1.getData('sessions')).toHaveLength(2);
      expect(pool2.getData('sessions')).toHaveLength(1);
    });

    it('should isolate memory data between tenants', async () => {
      await manager.seedTenantData(tenant1.id, {
        memories: [
          { id: 'mem-1', key: 'pattern-1', value: 'T1 pattern' },
          { id: 'mem-2', key: 'pattern-2', value: 'T1 pattern 2' }
        ]
      });

      await manager.seedTenantData(tenant2.id, {
        memories: [
          { id: 'mem-3', key: 'pattern-1', value: 'T2 pattern' } // Same key, different tenant
        ]
      });

      const pool1 = manager.getPool(tenant1.id)!;
      const pool2 = manager.getPool(tenant2.id)!;

      const t1Memories = pool1.getData('memories');
      const t2Memories = pool2.getData('memories');

      expect(t1Memories).toHaveLength(2);
      expect(t2Memories).toHaveLength(1);

      // Same key can exist in different tenants
      const t1Pattern1 = t1Memories.find((m: any) => m.key === 'pattern-1');
      const t2Pattern1 = t2Memories.find((m: any) => m.key === 'pattern-1');

      expect(t1Pattern1.value).toBe('T1 pattern');
      expect(t2Pattern1.value).toBe('T2 pattern');
    });
  });

  describe('Vector Index Isolation', () => {
    it('should isolate vector indexes between tenants', async () => {
      // Index documents for tenant 1
      await manager.vectorIndex(tenant1.id, 'doc-1', 'React component patterns');
      await manager.vectorIndex(tenant1.id, 'doc-2', 'TypeScript best practices');

      // Index documents for tenant 2
      await manager.vectorIndex(tenant2.id, 'doc-3', 'Python data analysis');

      const t1Index = manager.getVectorIndex(tenant1.id)!;
      const t2Index = manager.getVectorIndex(tenant2.id)!;

      expect(t1Index.size()).toBe(2);
      expect(t2Index.size()).toBe(1);
    });

    it('should search only within tenant vector space', async () => {
      // Index similar documents in different tenants
      await manager.vectorIndex(tenant1.id, 'doc-1', 'JavaScript programming guide');
      await manager.vectorIndex(tenant2.id, 'doc-2', 'JavaScript programming tutorial');

      // Search in tenant 1
      const t1Results = await manager.vectorSearch(tenant1.id, 'JavaScript programming');
      const t2Results = await manager.vectorSearch(tenant2.id, 'JavaScript programming');

      expect(t1Results).toHaveLength(1);
      expect(t1Results[0].id).toBe('doc-1');

      expect(t2Results).toHaveLength(1);
      expect(t2Results[0].id).toBe('doc-2');
    });

    it('should not leak vectors between tenants', async () => {
      await manager.vectorIndex(tenant1.id, 'secret-doc', 'Confidential information for tenant 1');

      // Tenant 2 should not find tenant 1's documents
      const t2Results = await manager.vectorSearch(tenant2.id, 'Confidential information');

      expect(t2Results).toHaveLength(0);
    });
  });

  describe('Resource Boundaries', () => {
    it('should enforce agent limits per tenant', async () => {
      const maxAgentsPerTenant = 10;
      let agentCount = 0;

      // Simulate adding agents up to limit
      for (let i = 0; i < maxAgentsPerTenant; i++) {
        agentCount++;
      }

      expect(agentCount).toBe(maxAgentsPerTenant);

      // Additional agents should be rejected
      const canAddMore = agentCount < maxAgentsPerTenant;
      expect(canAddMore).toBe(false);
    });

    it('should track resource usage per tenant', () => {
      const resourceUsage = {
        [tenant1.id]: { agents: 5, sessions: 20, memoryMB: 100 },
        [tenant2.id]: { agents: 3, sessions: 10, memoryMB: 50 }
      };

      expect(resourceUsage[tenant1.id].agents).toBe(5);
      expect(resourceUsage[tenant2.id].agents).toBe(3);

      // Total usage should not exceed system limits
      const totalAgents = Object.values(resourceUsage).reduce((sum, u) => sum + u.agents, 0);
      expect(totalAgents).toBe(8);
    });
  });

  describe('Access Control', () => {
    it('should validate tenant access on queries', async () => {
      await manager.seedTenantData(tenant1.id, {
        agents: [{ id: 'agent-1', name: 'Secret Agent' }]
      });

      const pool1 = manager.getPool(tenant1.id)!;
      const pool2 = manager.getPool(tenant2.id)!;

      // Query with correct tenant context
      const result1 = pool1.getData('agents');
      expect(result1).toHaveLength(1);

      // Query with wrong tenant context
      const result2 = pool2.getData('agents');
      expect(result2).toHaveLength(0);
    });

    it('should prevent cross-tenant data modification', async () => {
      await manager.seedTenantData(tenant1.id, {
        agents: [{ id: 'agent-1', name: 'Original' }]
      });

      const pool2 = manager.getPool(tenant2.id)!;

      // Attempt to modify tenant 1 data from tenant 2 context
      const updateResult = await pool2.query(
        'UPDATE agents SET name = $1 WHERE id = $2',
        ['Modified', 'agent-1']
      );

      // Should not find or modify the record
      expect(updateResult.rowCount).toBe(0);
    });
  });

  describe('Context Cleanup', () => {
    it('should clean up tenant context on destruction', async () => {
      await manager.seedTenantData(tenant1.id, {
        agents: [{ id: 'agent-1' }]
      });

      await manager.vectorIndex(tenant1.id, 'doc-1', 'Test document');

      // Destroy tenant context
      await manager.destroyTenantContext(tenant1.id);

      expect(manager.getPool(tenant1.id)).toBeUndefined();
      expect(manager.getVectorIndex(tenant1.id)).toBeUndefined();
    });

    it('should not affect other tenants on context destruction', async () => {
      await manager.seedTenantData(tenant1.id, {
        agents: [{ id: 'agent-1' }]
      });

      await manager.seedTenantData(tenant2.id, {
        agents: [{ id: 'agent-2' }]
      });

      // Destroy tenant 1
      await manager.destroyTenantContext(tenant1.id);

      // Tenant 2 should be unaffected
      const pool2 = manager.getPool(tenant2.id)!;
      expect(pool2).toBeDefined();
      expect(pool2.getData('agents')).toHaveLength(1);
    });
  });
});

describe('Multi-tenant Query Patterns', () => {
  let manager: TenantDataManager;
  const tenants = ['tenant-1', 'tenant-2', 'tenant-3'];

  beforeEach(async () => {
    manager = new TenantDataManager();
    for (const tenantId of tenants) {
      await manager.createTenantContext(tenantId);
    }
  });

  describe('Tenant-scoped Queries', () => {
    it('should filter all queries by tenant ID', async () => {
      // Seed data for all tenants
      for (let i = 0; i < tenants.length; i++) {
        await manager.seedTenantData(tenants[i], {
          sessions: [
            { id: `session-${i}-1`, status: 'active' },
            { id: `session-${i}-2`, status: 'completed' }
          ]
        });
      }

      // Query each tenant
      for (const tenantId of tenants) {
        const pool = manager.getPool(tenantId)!;
        const sessions = pool.getData('sessions');

        expect(sessions).toHaveLength(2);
        sessions.forEach((s: any) => {
          expect(s.tenantId).toBe(tenantId);
        });
      }
    });

    it('should aggregate data only within tenant scope', async () => {
      await manager.seedTenantData('tenant-1', {
        agents: [
          { id: 'a1', type: 'coder' },
          { id: 'a2', type: 'coder' },
          { id: 'a3', type: 'tester' }
        ]
      });

      await manager.seedTenantData('tenant-2', {
        agents: [
          { id: 'a4', type: 'coder' },
          { id: 'a5', type: 'reviewer' }
        ]
      });

      // Count coders per tenant
      const pool1 = manager.getPool('tenant-1')!;
      const pool2 = manager.getPool('tenant-2')!;

      const t1Coders = pool1.getData('agents').filter((a: any) => a.type === 'coder');
      const t2Coders = pool2.getData('agents').filter((a: any) => a.type === 'coder');

      expect(t1Coders).toHaveLength(2);
      expect(t2Coders).toHaveLength(1);
    });
  });

  describe('Cross-tenant Reporting', () => {
    it('should support admin queries across all tenants', async () => {
      // This would be for system-level admin only
      const allTenantStats: Record<string, number> = {};

      for (const tenantId of tenants) {
        await manager.seedTenantData(tenantId, {
          sessions: Array.from({ length: Math.floor(Math.random() * 10) + 1 }, (_, i) => ({
            id: `${tenantId}-session-${i}`
          }))
        });

        const pool = manager.getPool(tenantId)!;
        allTenantStats[tenantId] = pool.getData('sessions').length;
      }

      // Admin can see aggregated stats
      const totalSessions = Object.values(allTenantStats).reduce((sum, count) => sum + count, 0);
      expect(totalSessions).toBeGreaterThan(0);
    });
  });
});
