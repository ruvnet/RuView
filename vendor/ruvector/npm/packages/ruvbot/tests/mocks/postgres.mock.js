"use strict";
/**
 * PostgreSQL Mock Module
 *
 * Mock implementations for Postgres database operations
 * Supports transaction testing and query validation
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.queryBuilderHelpers = exports.mockPoolFactory = exports.MockPool = void 0;
exports.createMockPool = createMockPool;
const vitest_1 = require("vitest");
/**
 * Mock PostgreSQL Pool
 */
class MockPool {
    constructor(config) {
        this.config = config;
        this.connected = false;
        this.dataStore = {
            agents: new Map(),
            sessions: new Map(),
            memories: new Map(),
            skills: new Map(),
            tenants: new Map(),
            tasks: new Map()
        };
        this.queryLog = [];
        this.transactionActive = false;
    }
    async connect() {
        this.connected = true;
        return this.createClient();
    }
    async query(text, values) {
        this.logQuery(text, values);
        return this.executeQuery(text, values);
    }
    async end() {
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
    isConnected() {
        return this.connected;
    }
    getQueryLog() {
        return [...this.queryLog];
    }
    clearQueryLog() {
        this.queryLog = [];
    }
    // Seed data for testing
    seedData(table, data) {
        for (const row of data) {
            this.dataStore[table].set(row.id, row);
        }
    }
    getData(table) {
        return Array.from(this.dataStore[table].values());
    }
    createClient() {
        return {
            query: async (text, values) => {
                return this.executeQuery(text, values);
            },
            release: () => {
                // No-op for mock
            }
        };
    }
    logQuery(text, values) {
        this.queryLog.push({ text, values, timestamp: new Date() });
    }
    async executeQuery(text, values) {
        const normalizedQuery = text.trim().toUpperCase();
        // Handle transaction commands
        if (normalizedQuery === 'BEGIN') {
            this.transactionActive = true;
            return this.createResult([], 'BEGIN');
        }
        if (normalizedQuery === 'COMMIT') {
            this.transactionActive = false;
            return this.createResult([], 'COMMIT');
        }
        if (normalizedQuery === 'ROLLBACK') {
            this.transactionActive = false;
            return this.createResult([], 'ROLLBACK');
        }
        // Parse and execute query
        if (normalizedQuery.startsWith('SELECT')) {
            return this.handleSelect(text, values);
        }
        if (normalizedQuery.startsWith('INSERT')) {
            return this.handleInsert(text, values);
        }
        if (normalizedQuery.startsWith('UPDATE')) {
            return this.handleUpdate(text, values);
        }
        if (normalizedQuery.startsWith('DELETE')) {
            return this.handleDelete(text, values);
        }
        // Default: return empty result
        return this.createResult([], 'UNKNOWN');
    }
    handleSelect(text, values) {
        const tableName = this.extractTableName(text);
        const store = this.dataStore[tableName];
        if (!store) {
            return this.createResult([], 'SELECT');
        }
        // Simple ID-based lookup
        const idMatch = text.match(/WHERE\s+id\s*=\s*\$1/i);
        if (idMatch && values?.[0]) {
            const row = store.get(values[0]);
            return this.createResult(row ? [row] : [], 'SELECT');
        }
        // Tenant-based lookup
        const tenantMatch = text.match(/WHERE\s+tenant_id\s*=\s*\$1/i);
        if (tenantMatch && values?.[0]) {
            const rows = Array.from(store.values())
                .filter((row) => row.tenantId === values[0] || row.tenant_id === values[0]);
            return this.createResult(rows, 'SELECT');
        }
        // Return all rows
        return this.createResult(Array.from(store.values()), 'SELECT');
    }
    handleInsert(text, values) {
        const tableName = this.extractTableName(text);
        const store = this.dataStore[tableName];
        if (!store || !values) {
            return this.createResult([], 'INSERT', 0);
        }
        // Extract column names from query
        const columnsMatch = text.match(/\(([^)]+)\)/);
        if (!columnsMatch) {
            return this.createResult([], 'INSERT', 0);
        }
        const columns = columnsMatch[1].split(',').map(c => c.trim());
        const row = {};
        columns.forEach((col, idx) => {
            row[col] = values[idx];
        });
        const id = row.id || `generated-${Date.now()}`;
        row.id = id;
        store.set(id, row);
        // Check for RETURNING clause
        if (text.includes('RETURNING')) {
            return this.createResult([row], 'INSERT', 1);
        }
        return this.createResult([], 'INSERT', 1);
    }
    handleUpdate(text, values) {
        const tableName = this.extractTableName(text);
        const store = this.dataStore[tableName];
        if (!store || !values) {
            return this.createResult([], 'UPDATE', 0);
        }
        // Simple ID-based update
        const idMatch = text.match(/WHERE\s+id\s*=\s*\$(\d+)/i);
        if (idMatch) {
            const idParamIndex = parseInt(idMatch[1]) - 1;
            const id = values[idParamIndex];
            const row = store.get(id);
            if (row) {
                // Update would happen here in real implementation
                return this.createResult([], 'UPDATE', 1);
            }
        }
        return this.createResult([], 'UPDATE', 0);
    }
    handleDelete(text, values) {
        const tableName = this.extractTableName(text);
        const store = this.dataStore[tableName];
        if (!store || !values) {
            return this.createResult([], 'DELETE', 0);
        }
        // Simple ID-based delete
        const idMatch = text.match(/WHERE\s+id\s*=\s*\$1/i);
        if (idMatch && values[0]) {
            const deleted = store.delete(values[0]);
            return this.createResult([], 'DELETE', deleted ? 1 : 0);
        }
        return this.createResult([], 'DELETE', 0);
    }
    extractTableName(query) {
        const fromMatch = query.match(/FROM\s+(\w+)/i);
        if (fromMatch)
            return fromMatch[1].toLowerCase();
        const intoMatch = query.match(/INTO\s+(\w+)/i);
        if (intoMatch)
            return intoMatch[1].toLowerCase();
        const updateMatch = query.match(/UPDATE\s+(\w+)/i);
        if (updateMatch)
            return updateMatch[1].toLowerCase();
        return 'unknown';
    }
    createResult(rows, command, rowCount) {
        return {
            rows,
            rowCount: rowCount ?? rows.length,
            command,
            fields: []
        };
    }
}
exports.MockPool = MockPool;
/**
 * Create a mock pool instance
 */
function createMockPool(config) {
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
exports.mockPoolFactory = {
    create: vitest_1.vi.fn((config) => createMockPool(config)),
    createClient: vitest_1.vi.fn(async (config) => {
        const pool = createMockPool(config);
        return pool.connect();
    })
};
/**
 * Postgres query builder mock helpers
 */
exports.queryBuilderHelpers = {
    expectQuery: (pool, pattern) => {
        return pool.getQueryLog().some(q => pattern.test(q.text));
    },
    expectQueryCount: (pool, pattern) => {
        return pool.getQueryLog().filter(q => pattern.test(q.text)).length;
    },
    expectTransaction: (pool) => {
        const log = pool.getQueryLog();
        const hasBegin = log.some(q => q.text.toUpperCase().includes('BEGIN'));
        const hasCommitOrRollback = log.some(q => q.text.toUpperCase().includes('COMMIT') ||
            q.text.toUpperCase().includes('ROLLBACK'));
        return hasBegin && hasCommitOrRollback;
    }
};
exports.default = {
    MockPool,
    createMockPool,
    mockPoolFactory: exports.mockPoolFactory,
    queryBuilderHelpers: exports.queryBuilderHelpers
};
//# sourceMappingURL=postgres.mock.js.map