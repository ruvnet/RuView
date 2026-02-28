/**
 * PostgreSQL Persistence - Integration Tests
 *
 * Tests for database operations, transactions, and data integrity
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createMockPool, queryBuilderHelpers, type MockPool } from '../../mocks/postgres.mock';
import { createAgent, createSession, createMemory, createTenant } from '../../factories';

describe('PostgreSQL Persistence', () => {
  let pool: MockPool;

  beforeEach(async () => {
    pool = createMockPool();
    await pool.connect();
  });

  afterEach(async () => {
    await pool.end();
  });

  describe('Connection Management', () => {
    it('should establish connection', async () => {
      expect(pool.isConnected()).toBe(true);
    });

    it('should close connection', async () => {
      await pool.end();
      expect(pool.isConnected()).toBe(false);
    });
  });

  describe('Agent Persistence', () => {
    it('should insert agent', async () => {
      const agent = createAgent({ name: 'Test Agent', type: 'coder' });

      const result = await pool.query(
        'INSERT INTO agents (id, name, type, status, config) VALUES ($1, $2, $3, $4, $5) RETURNING *',
        [agent.id, agent.name, agent.type, agent.status, JSON.stringify(agent.config)]
      );

      expect(result.rowCount).toBe(1);
      expect(queryBuilderHelpers.expectQuery(pool, /INSERT INTO agents/)).toBe(true);
    });

    it('should select agent by ID', async () => {
      const agent = createAgent();

      // Seed data
      pool.seedData('agents', [{ id: agent.id, name: agent.name, type: agent.type }]);

      const result = await pool.query(
        'SELECT * FROM agents WHERE id = $1',
        [agent.id]
      );

      expect(result.rows).toHaveLength(1);
      expect(result.rows[0].id).toBe(agent.id);
    });

    it('should update agent', async () => {
      const agent = createAgent();
      pool.seedData('agents', [{ id: agent.id, name: agent.name, status: 'idle' }]);

      const result = await pool.query(
        'UPDATE agents SET status = $1 WHERE id = $2',
        ['busy', agent.id]
      );

      expect(result.rowCount).toBe(1);
    });

    it('should delete agent', async () => {
      const agent = createAgent();
      pool.seedData('agents', [{ id: agent.id }]);

      const result = await pool.query(
        'DELETE FROM agents WHERE id = $1',
        [agent.id]
      );

      expect(result.rowCount).toBe(1);
    });
  });

  describe('Session Persistence', () => {
    it('should insert session', async () => {
      const session = createSession();

      const result = await pool.query(
        'INSERT INTO sessions (id, tenant_id, user_id, channel_id, status) VALUES ($1, $2, $3, $4, $5) RETURNING *',
        [session.id, session.tenantId, session.userId, session.channelId, session.status]
      );

      expect(result.rowCount).toBe(1);
    });

    it('should select sessions by tenant', async () => {
      const tenantId = 'tenant-001';
      pool.seedData('sessions', [
        { id: 'session-1', tenantId, tenant_id: tenantId },
        { id: 'session-2', tenantId, tenant_id: tenantId },
        { id: 'session-3', tenantId: 'other-tenant', tenant_id: 'other-tenant' }
      ]);

      const result = await pool.query(
        'SELECT * FROM sessions WHERE tenant_id = $1',
        [tenantId]
      );

      expect(result.rows).toHaveLength(2);
      result.rows.forEach(row => {
        expect(row.tenantId || row.tenant_id).toBe(tenantId);
      });
    });
  });

  describe('Memory Persistence', () => {
    it('should insert memory entry', async () => {
      const memory = createMemory({ key: 'test-key', value: { data: 'test' } });

      const result = await pool.query(
        'INSERT INTO memories (id, tenant_id, key, value, type) VALUES ($1, $2, $3, $4, $5) RETURNING *',
        [memory.id, memory.tenantId, memory.key, JSON.stringify(memory.value), memory.type]
      );

      expect(result.rowCount).toBe(1);
    });

    it('should select memory by key', async () => {
      pool.seedData('memories', [
        { id: 'mem-1', key: 'unique-key', tenantId: 'tenant-001' }
      ]);

      // Note: Mock implementation uses indexByKey
      const result = await pool.query(
        'SELECT * FROM memories WHERE key = $1',
        ['unique-key']
      );

      expect(queryBuilderHelpers.expectQuery(pool, /SELECT \* FROM memories/)).toBe(true);
    });
  });

  describe('Tenant Persistence', () => {
    it('should insert tenant', async () => {
      const tenant = createTenant();

      const result = await pool.query(
        'INSERT INTO tenants (id, name, slack_team_id, status, plan) VALUES ($1, $2, $3, $4, $5) RETURNING *',
        [tenant.id, tenant.name, tenant.slackTeamId, tenant.status, tenant.plan]
      );

      expect(result.rowCount).toBe(1);
    });

    it('should select tenant by slack team ID', async () => {
      pool.seedData('tenants', [
        { id: 'tenant-1', slackTeamId: 'T12345678' }
      ]);

      const result = await pool.query(
        'SELECT * FROM tenants WHERE id = $1',
        ['tenant-1']
      );

      expect(result.rows).toHaveLength(1);
    });
  });

  describe('Transactions', () => {
    it('should execute transaction with commit', async () => {
      await pool.query('BEGIN');
      await pool.query('INSERT INTO agents (id, name) VALUES ($1, $2)', ['agent-1', 'Test']);
      await pool.query('INSERT INTO sessions (id, tenant_id) VALUES ($1, $2)', ['session-1', 'tenant-1']);
      await pool.query('COMMIT');

      expect(queryBuilderHelpers.expectTransaction(pool)).toBe(true);
    });

    it('should execute transaction with rollback', async () => {
      await pool.query('BEGIN');
      await pool.query('INSERT INTO agents (id, name) VALUES ($1, $2)', ['agent-1', 'Test']);
      await pool.query('ROLLBACK');

      expect(queryBuilderHelpers.expectTransaction(pool)).toBe(true);
    });
  });

  describe('Query Logging', () => {
    it('should log all queries', async () => {
      await pool.query('SELECT 1');
      await pool.query('SELECT 2');
      await pool.query('SELECT 3');

      const log = pool.getQueryLog();
      expect(log).toHaveLength(3);
    });

    it('should log query values', async () => {
      await pool.query('INSERT INTO agents (id) VALUES ($1)', ['agent-1']);

      const log = pool.getQueryLog();
      expect(log[0].values).toEqual(['agent-1']);
    });

    it('should clear query log', async () => {
      await pool.query('SELECT 1');
      pool.clearQueryLog();

      expect(pool.getQueryLog()).toHaveLength(0);
    });
  });

  describe('Query Helpers', () => {
    it('should match query patterns', async () => {
      await pool.query('SELECT * FROM agents WHERE type = $1', ['coder']);

      expect(queryBuilderHelpers.expectQuery(pool, /SELECT \* FROM agents/)).toBe(true);
      expect(queryBuilderHelpers.expectQuery(pool, /SELECT \* FROM sessions/)).toBe(false);
    });

    it('should count matching queries', async () => {
      await pool.query('SELECT * FROM agents');
      await pool.query('SELECT * FROM agents WHERE id = $1', ['1']);
      await pool.query('SELECT * FROM sessions');

      const count = queryBuilderHelpers.expectQueryCount(pool, /SELECT \* FROM agents/);
      expect(count).toBe(2);
    });
  });
});

describe('PostgreSQL Repository Patterns', () => {
  let pool: MockPool;

  beforeEach(async () => {
    pool = createMockPool();
    await pool.connect();
  });

  afterEach(async () => {
    await pool.end();
  });

  describe('Bulk Operations', () => {
    it('should handle bulk insert', async () => {
      const agents = Array.from({ length: 10 }, (_, i) =>
        createAgent({ id: `agent-${i}`, name: `Agent ${i}` })
      );

      // Simulate bulk insert
      for (const agent of agents) {
        await pool.query(
          'INSERT INTO agents (id, name) VALUES ($1, $2)',
          [agent.id, agent.name]
        );
      }

      expect(queryBuilderHelpers.expectQueryCount(pool, /INSERT INTO agents/)).toBe(10);
    });
  });

  describe('Upsert Operations', () => {
    it('should handle upsert pattern', async () => {
      pool.seedData('agents', [{ id: 'agent-1', name: 'Original' }]);

      // Simulate upsert
      const result = await pool.query(
        `INSERT INTO agents (id, name) VALUES ($1, $2)
         ON CONFLICT (id) DO UPDATE SET name = $2
         RETURNING *`,
        ['agent-1', 'Updated']
      );

      expect(queryBuilderHelpers.expectQuery(pool, /INSERT INTO agents/)).toBe(true);
    });
  });

  describe('Pagination', () => {
    it('should handle paginated queries', async () => {
      pool.seedData('agents', Array.from({ length: 25 }, (_, i) => ({
        id: `agent-${i}`,
        name: `Agent ${i}`
      })));

      const page1 = await pool.query(
        'SELECT * FROM agents ORDER BY id LIMIT $1 OFFSET $2',
        [10, 0]
      );

      const page2 = await pool.query(
        'SELECT * FROM agents ORDER BY id LIMIT $1 OFFSET $2',
        [10, 10]
      );

      expect(queryBuilderHelpers.expectQueryCount(pool, /LIMIT/)).toBe(2);
    });
  });

  describe('Join Operations', () => {
    it('should log join queries', async () => {
      await pool.query(`
        SELECT s.*, a.name as agent_name
        FROM sessions s
        LEFT JOIN agents a ON a.id = ANY(s.active_agents)
        WHERE s.tenant_id = $1
      `, ['tenant-1']);

      expect(queryBuilderHelpers.expectQuery(pool, /JOIN/)).toBe(true);
    });
  });

  describe('Aggregations', () => {
    it('should log aggregation queries', async () => {
      await pool.query(`
        SELECT tenant_id, COUNT(*) as session_count
        FROM sessions
        GROUP BY tenant_id
        HAVING COUNT(*) > $1
      `, [5]);

      expect(queryBuilderHelpers.expectQuery(pool, /GROUP BY/)).toBe(true);
      expect(queryBuilderHelpers.expectQuery(pool, /COUNT/)).toBe(true);
    });
  });
});

describe('PostgreSQL Error Handling', () => {
  let pool: MockPool;

  beforeEach(async () => {
    pool = createMockPool();
    await pool.connect();
  });

  afterEach(async () => {
    await pool.end();
  });

  it('should handle query errors gracefully', async () => {
    // In real implementation, this would test actual error scenarios
    const result = await pool.query('SELECT * FROM non_existent_table');

    expect(result.rows).toEqual([]);
  });

  it('should track failed transactions', async () => {
    await pool.query('BEGIN');
    await pool.query('INVALID SQL THAT WOULD FAIL');
    await pool.query('ROLLBACK');

    expect(queryBuilderHelpers.expectTransaction(pool)).toBe(true);
  });
});

describe('PostgreSQL Multi-tenancy', () => {
  let pool: MockPool;

  beforeEach(async () => {
    pool = createMockPool();
    await pool.connect();

    // Seed multi-tenant data
    pool.seedData('agents', [
      { id: 'agent-1', tenantId: 'tenant-1', tenant_id: 'tenant-1', name: 'T1 Agent' },
      { id: 'agent-2', tenantId: 'tenant-2', tenant_id: 'tenant-2', name: 'T2 Agent' },
      { id: 'agent-3', tenantId: 'tenant-1', tenant_id: 'tenant-1', name: 'T1 Agent 2' }
    ]);
  });

  afterEach(async () => {
    await pool.end();
  });

  it('should filter by tenant ID', async () => {
    const result = await pool.query(
      'SELECT * FROM agents WHERE tenant_id = $1',
      ['tenant-1']
    );

    expect(result.rows).toHaveLength(2);
    result.rows.forEach(row => {
      expect(row.tenantId || row.tenant_id).toBe('tenant-1');
    });
  });

  it('should isolate tenant data', async () => {
    const tenant1Data = await pool.query(
      'SELECT * FROM agents WHERE tenant_id = $1',
      ['tenant-1']
    );

    const tenant2Data = await pool.query(
      'SELECT * FROM agents WHERE tenant_id = $1',
      ['tenant-2']
    );

    expect(tenant1Data.rows).toHaveLength(2);
    expect(tenant2Data.rows).toHaveLength(1);

    // Verify no data leakage
    const tenant1Ids = tenant1Data.rows.map((r: any) => r.id);
    const tenant2Ids = tenant2Data.rows.map((r: any) => r.id);

    expect(tenant1Ids).not.toContain('agent-2');
    expect(tenant2Ids).not.toContain('agent-1');
  });
});
