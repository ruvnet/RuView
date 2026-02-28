/**
 * PostgreSQL Mock Module
 *
 * Mock implementations for Postgres database operations
 * Supports transaction testing and query validation
 */

import { vi } from 'vitest';

// Types
export interface QueryResult<T = unknown> {
  rows: T[];
  rowCount: number;
  command: string;
  fields: FieldInfo[];
}

export interface FieldInfo {
  name: string;
  dataTypeID: number;
}

export interface PoolConfig {
  host: string;
  port: number;
  database: string;
  user: string;
  password: string;
  max?: number;
  idleTimeoutMillis?: number;
}

export interface PoolClient {
  query<T = unknown>(text: string, values?: unknown[]): Promise<QueryResult<T>>;
  release(): void;
}

// In-memory data store for mock
interface MockDataStore {
  agents: Map<string, unknown>;
  sessions: Map<string, unknown>;
  memories: Map<string, unknown>;
  skills: Map<string, unknown>;
  tenants: Map<string, unknown>;
  tasks: Map<string, unknown>;
}

/**
 * Mock PostgreSQL Pool
 */
export class MockPool {
  private connected: boolean = false;
  private dataStore: MockDataStore = {
    agents: new Map(),
    sessions: new Map(),
    memories: new Map(),
    skills: new Map(),
    tenants: new Map(),
    tasks: new Map()
  };
  private queryLog: Array<{ text: string; values?: unknown[]; timestamp: Date }> = [];
  private transactionActive: boolean = false;

  constructor(private config: PoolConfig) {}

  async connect(): Promise<PoolClient> {
    this.connected = true;
    return this.createClient();
  }

  async query<T = unknown>(text: string, values?: unknown[]): Promise<QueryResult<T>> {
    this.logQuery(text, values);
    return this.executeQuery<T>(text, values);
  }

  async end(): Promise<void> {
    this.connected = false;
    this.dataStore = {
      agents: new Map(),
      sessions: new Map(),
      memories: new Map(),
      skills: new Map(),
      tenants: new Map(),
      tasks: new Map()
    };
  }

  isConnected(): boolean {
    return this.connected;
  }

  getQueryLog(): Array<{ text: string; values?: unknown[]; timestamp: Date }> {
    return [...this.queryLog];
  }

  clearQueryLog(): void {
    this.queryLog = [];
  }

  // Seed data for testing
  seedData(table: keyof MockDataStore, data: Array<{ id: string; [key: string]: unknown }>): void {
    for (const row of data) {
      this.dataStore[table].set(row.id, row);
    }
  }

  getData(table: keyof MockDataStore): unknown[] {
    return Array.from(this.dataStore[table].values());
  }

  private createClient(): PoolClient {
    return {
      query: async <T = unknown>(text: string, values?: unknown[]): Promise<QueryResult<T>> => {
        return this.executeQuery<T>(text, values);
      },
      release: () => {
        // No-op for mock
      }
    };
  }

  private logQuery(text: string, values?: unknown[]): void {
    this.queryLog.push({ text, values, timestamp: new Date() });
  }

  private async executeQuery<T>(text: string, values?: unknown[]): Promise<QueryResult<T>> {
    const normalizedQuery = text.trim().toUpperCase();

    // Handle transaction commands
    if (normalizedQuery === 'BEGIN') {
      this.transactionActive = true;
      return this.createResult<T>([], 'BEGIN');
    }

    if (normalizedQuery === 'COMMIT') {
      this.transactionActive = false;
      return this.createResult<T>([], 'COMMIT');
    }

    if (normalizedQuery === 'ROLLBACK') {
      this.transactionActive = false;
      return this.createResult<T>([], 'ROLLBACK');
    }

    // Parse and execute query
    if (normalizedQuery.startsWith('SELECT')) {
      return this.handleSelect<T>(text, values);
    }

    if (normalizedQuery.startsWith('INSERT')) {
      return this.handleInsert<T>(text, values);
    }

    if (normalizedQuery.startsWith('UPDATE')) {
      return this.handleUpdate<T>(text, values);
    }

    if (normalizedQuery.startsWith('DELETE')) {
      return this.handleDelete<T>(text, values);
    }

    // Default: return empty result
    return this.createResult<T>([], 'UNKNOWN');
  }

  private handleSelect<T>(text: string, values?: unknown[]): QueryResult<T> {
    const tableName = this.extractTableName(text);
    const store = this.dataStore[tableName as keyof MockDataStore];

    if (!store) {
      return this.createResult<T>([], 'SELECT');
    }

    // Simple ID-based lookup
    const idMatch = text.match(/WHERE\s+id\s*=\s*\$1/i);
    if (idMatch && values?.[0]) {
      const row = store.get(values[0] as string);
      return this.createResult<T>(row ? [row as T] : [], 'SELECT');
    }

    // Tenant-based lookup
    const tenantMatch = text.match(/WHERE\s+tenant_id\s*=\s*\$1/i);
    if (tenantMatch && values?.[0]) {
      const rows = Array.from(store.values())
        .filter((row: any) => row.tenantId === values[0] || row.tenant_id === values[0]);
      return this.createResult<T>(rows as T[], 'SELECT');
    }

    // Return all rows
    return this.createResult<T>(Array.from(store.values()) as T[], 'SELECT');
  }

  private handleInsert<T>(text: string, values?: unknown[]): QueryResult<T> {
    const tableName = this.extractTableName(text);
    const store = this.dataStore[tableName as keyof MockDataStore];

    if (!store || !values) {
      return this.createResult<T>([], 'INSERT', 0);
    }

    // Extract column names from query
    const columnsMatch = text.match(/\(([^)]+)\)/);
    if (!columnsMatch) {
      return this.createResult<T>([], 'INSERT', 0);
    }

    const columns = columnsMatch[1].split(',').map(c => c.trim());
    const row: Record<string, unknown> = {};

    columns.forEach((col, idx) => {
      row[col] = values[idx];
    });

    const id = row.id as string || `generated-${Date.now()}`;
    row.id = id;
    store.set(id, row);

    // Check for RETURNING clause
    if (text.includes('RETURNING')) {
      return this.createResult<T>([row as T], 'INSERT', 1);
    }

    return this.createResult<T>([], 'INSERT', 1);
  }

  private handleUpdate<T>(text: string, values?: unknown[]): QueryResult<T> {
    const tableName = this.extractTableName(text);
    const store = this.dataStore[tableName as keyof MockDataStore];

    if (!store || !values) {
      return this.createResult<T>([], 'UPDATE', 0);
    }

    // Simple ID-based update
    const idMatch = text.match(/WHERE\s+id\s*=\s*\$(\d+)/i);
    if (idMatch) {
      const idParamIndex = parseInt(idMatch[1]) - 1;
      const id = values[idParamIndex] as string;
      const row = store.get(id);

      if (row) {
        // Update would happen here in real implementation
        return this.createResult<T>([], 'UPDATE', 1);
      }
    }

    return this.createResult<T>([], 'UPDATE', 0);
  }

  private handleDelete<T>(text: string, values?: unknown[]): QueryResult<T> {
    const tableName = this.extractTableName(text);
    const store = this.dataStore[tableName as keyof MockDataStore];

    if (!store || !values) {
      return this.createResult<T>([], 'DELETE', 0);
    }

    // Simple ID-based delete
    const idMatch = text.match(/WHERE\s+id\s*=\s*\$1/i);
    if (idMatch && values[0]) {
      const deleted = store.delete(values[0] as string);
      return this.createResult<T>([], 'DELETE', deleted ? 1 : 0);
    }

    return this.createResult<T>([], 'DELETE', 0);
  }

  private extractTableName(query: string): string {
    const fromMatch = query.match(/FROM\s+(\w+)/i);
    if (fromMatch) return fromMatch[1].toLowerCase();

    const intoMatch = query.match(/INTO\s+(\w+)/i);
    if (intoMatch) return intoMatch[1].toLowerCase();

    const updateMatch = query.match(/UPDATE\s+(\w+)/i);
    if (updateMatch) return updateMatch[1].toLowerCase();

    return 'unknown';
  }

  private createResult<T>(rows: T[], command: string, rowCount?: number): QueryResult<T> {
    return {
      rows,
      rowCount: rowCount ?? rows.length,
      command,
      fields: []
    };
  }
}

/**
 * Create a mock pool instance
 */
export function createMockPool(config?: Partial<PoolConfig>): MockPool {
  return new MockPool({
    host: 'localhost',
    port: 5432,
    database: 'ruvbot_test',
    user: 'test',
    password: 'test',
    ...config
  });
}

/**
 * Mock Pool factory for dependency injection
 */
export const mockPoolFactory = {
  create: vi.fn((config: PoolConfig) => createMockPool(config)),
  createClient: vi.fn(async (config: PoolConfig) => {
    const pool = createMockPool(config);
    return pool.connect();
  })
};

/**
 * Postgres query builder mock helpers
 */
export const queryBuilderHelpers = {
  expectQuery: (pool: MockPool, pattern: RegExp): boolean => {
    return pool.getQueryLog().some(q => pattern.test(q.text));
  },

  expectQueryCount: (pool: MockPool, pattern: RegExp): number => {
    return pool.getQueryLog().filter(q => pattern.test(q.text)).length;
  },

  expectTransaction: (pool: MockPool): boolean => {
    const log = pool.getQueryLog();
    const hasBegin = log.some(q => q.text.toUpperCase().includes('BEGIN'));
    const hasCommitOrRollback = log.some(q =>
      q.text.toUpperCase().includes('COMMIT') ||
      q.text.toUpperCase().includes('ROLLBACK')
    );
    return hasBegin && hasCommitOrRollback;
  }
};

export default {
  MockPool,
  createMockPool,
  mockPoolFactory,
  queryBuilderHelpers
};
