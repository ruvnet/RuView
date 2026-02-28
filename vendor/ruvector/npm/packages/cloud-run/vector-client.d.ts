/**
 * Vector Client - Optimized ruvector connection layer
 *
 * High-performance client with connection pooling, caching, and streaming support.
 */
export interface VectorClientConfig {
    host: string;
    maxConnections?: number;
    minConnections?: number;
    idleTimeout?: number;
    connectionTimeout?: number;
    queryTimeout?: number;
    retryAttempts?: number;
    retryDelay?: number;
    cacheSize?: number;
    cacheTTL?: number;
    enableMetrics?: boolean;
}
interface QueryResult {
    id: string;
    vector?: number[];
    metadata?: Record<string, any>;
    score?: number;
    distance?: number;
}
/**
 * Vector Client with connection pooling and caching
 */
export declare class VectorClient {
    private pool;
    private cache;
    private config;
    private initialized;
    constructor(config: VectorClientConfig);
    initialize(): Promise<void>;
    query(collection: string, query: any): Promise<QueryResult[]>;
    streamQuery(collection: string, query: any, onChunk: (chunk: QueryResult) => void): Promise<void>;
    batchQuery(queries: any[]): Promise<any[]>;
    private executeWithRetry;
    healthCheck(): Promise<boolean>;
    close(): Promise<void>;
    getStats(): {
        pool: {
            total: number;
            active: number;
            idle: number;
            waiting: number;
        };
        cache: {
            size: number;
            max: number;
        };
    };
    clearCache(): void;
}
export {};
//# sourceMappingURL=vector-client.d.ts.map