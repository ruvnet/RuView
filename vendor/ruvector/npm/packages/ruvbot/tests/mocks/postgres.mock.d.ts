/**
 * PostgreSQL Mock Module
 *
 * Mock implementations for Postgres database operations
 * Supports transaction testing and query validation
 */
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
export declare class MockPool {
    private config;
    private connected;
    private dataStore;
    private queryLog;
    private transactionActive;
    constructor(config: PoolConfig);
    connect(): Promise<PoolClient>;
    query<T = unknown>(text: string, values?: unknown[]): Promise<QueryResult<T>>;
    end(): Promise<void>;
    isConnected(): boolean;
    getQueryLog(): Array<{
        text: string;
        values?: unknown[];
        timestamp: Date;
    }>;
    clearQueryLog(): void;
    seedData(table: keyof MockDataStore, data: Array<{
        id: string;
        [key: string]: unknown;
    }>): void;
    getData(table: keyof MockDataStore): unknown[];
    private createClient;
    private logQuery;
    private executeQuery;
    private handleSelect;
    private handleInsert;
    private handleUpdate;
    private handleDelete;
    private extractTableName;
    private createResult;
}
/**
 * Create a mock pool instance
 */
export declare function createMockPool(config?: Partial<PoolConfig>): MockPool;
/**
 * Mock Pool factory for dependency injection
 */
export declare const mockPoolFactory: {
    create: import("vitest").Mock<[config: PoolConfig], MockPool>;
    createClient: import("vitest").Mock<[config: PoolConfig], Promise<PoolClient>>;
};
/**
 * Postgres query builder mock helpers
 */
export declare const queryBuilderHelpers: {
    expectQuery: (pool: MockPool, pattern: RegExp) => boolean;
    expectQueryCount: (pool: MockPool, pattern: RegExp) => number;
    expectTransaction: (pool: MockPool) => boolean;
};
declare const _default: {
    MockPool: typeof MockPool;
    createMockPool: typeof createMockPool;
    mockPoolFactory: {
        create: import("vitest").Mock<[config: PoolConfig], MockPool>;
        createClient: import("vitest").Mock<[config: PoolConfig], Promise<PoolClient>>;
    };
    queryBuilderHelpers: {
        expectQuery: (pool: MockPool, pattern: RegExp) => boolean;
        expectQueryCount: (pool: MockPool, pattern: RegExp) => number;
        expectTransaction: (pool: MockPool) => boolean;
    };
};
export default _default;
//# sourceMappingURL=postgres.mock.d.ts.map