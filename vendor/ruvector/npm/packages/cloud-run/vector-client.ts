/**
 * Vector Client - Optimized ruvector connection layer
 *
 * High-performance client with connection pooling, caching, and streaming support.
 */

import { EventEmitter } from 'events';
import { LRUCache } from 'lru-cache';
import { trace, SpanStatusCode } from '@opentelemetry/api';
import { Histogram, Counter, Gauge } from 'prom-client';

// Metrics
const metrics = {
  queryDuration: new Histogram({
    name: 'vector_query_duration_seconds',
    help: 'Vector query duration in seconds',
    labelNames: ['collection', 'operation', 'cached'],
    buckets: [0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1, 2],
  }),
  cacheHits: new Counter({
    name: 'vector_cache_hits_total',
    help: 'Total number of cache hits',
    labelNames: ['collection'],
  }),
  cacheMisses: new Counter({
    name: 'vector_cache_misses_total',
    help: 'Total number of cache misses',
    labelNames: ['collection'],
  }),
  poolConnections: new Gauge({
    name: 'vector_pool_connections',
    help: 'Number of connections in the pool',
    labelNames: ['state'],
  }),
  retries: new Counter({
    name: 'vector_retries_total',
    help: 'Total number of retry attempts',
    labelNames: ['collection', 'reason'],
  }),
};

const tracer = trace.getTracer('vector-client', '1.0.0');

// Configuration interface
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

// Query result interface
interface QueryResult {
  id: string;
  vector?: number[];
  metadata?: Record<string, any>;
  score?: number;
  distance?: number;
}

// Connection pool interface
interface PoolConnection {
  id: string;
  client: any; // Actual ruvector binding
  inUse: boolean;
  lastUsed: number;
  queryCount: number;
}

// Cache key generation
function getCacheKey(collection: string, query: any): string {
  const queryStr = JSON.stringify({
    collection,
    vector: query.vector?.slice(0, 5), // Use first 5 dimensions for caching
    filter: query.filter,
    limit: query.limit,
    type: query.type,
  });
  return Buffer.from(queryStr).toString('base64');
}

/**
 * Connection Pool Manager
 */
class ConnectionPool extends EventEmitter {
  private connections: PoolConnection[] = [];
  private waitQueue: Array<(conn: PoolConnection) => void> = [];
  private cleanupInterval: NodeJS.Timeout | null = null;

  constructor(private config: Required<VectorClientConfig>) {
    super();
    this.initializePool();
    this.startCleanup();
  }

  private async initializePool(): Promise<void> {
    for (let i = 0; i < this.config.minConnections; i++) {
      await this.createConnection();
    }
  }

  private async createConnection(): Promise<PoolConnection> {
    const span = tracer.startSpan('create-connection');

    try {
      // TODO: Replace with actual ruvector Node.js binding
      // const client = await ruvector.connect(this.config.host);
      const client = {
        // Mock client for now
        query: async (collection: string, params: any) => {
          return { results: [] };
        },
        close: async () => {},
      };

      const connection: PoolConnection = {
        id: `conn-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
        client,
        inUse: false,
        lastUsed: Date.now(),
        queryCount: 0,
      };

      this.connections.push(connection);
      metrics.poolConnections.inc({ state: 'idle' });
      span.setStatus({ code: SpanStatusCode.OK });

      return connection;
    } catch (error) {
      span.setStatus({ code: SpanStatusCode.ERROR, message: (error as Error).message });
      throw error;
    } finally {
      span.end();
    }
  }

  async acquire(): Promise<PoolConnection> {
    // Find available connection
    const available = this.connections.find(conn => !conn.inUse);

    if (available) {
      available.inUse = true;
      available.lastUsed = Date.now();
      metrics.poolConnections.dec({ state: 'idle' });
      metrics.poolConnections.inc({ state: 'active' });
      return available;
    }

    // Create new connection if under max
    if (this.connections.length < this.config.maxConnections) {
      const newConn = await this.createConnection();
      newConn.inUse = true;
      metrics.poolConnections.dec({ state: 'idle' });
      metrics.poolConnections.inc({ state: 'active' });
      return newConn;
    }

    // Wait for available connection
    return new Promise((resolve) => {
      this.waitQueue.push(resolve);
    });
  }

  release(connection: PoolConnection): void {
    connection.inUse = false;
    connection.lastUsed = Date.now();
    metrics.poolConnections.dec({ state: 'active' });
    metrics.poolConnections.inc({ state: 'idle' });

    // Process wait queue
    const waiter = this.waitQueue.shift();
    if (waiter) {
      connection.inUse = true;
      metrics.poolConnections.dec({ state: 'idle' });
      metrics.poolConnections.inc({ state: 'active' });
      waiter(connection);
    }
  }

  private startCleanup(): void {
    this.cleanupInterval = setInterval(() => {
      const now = Date.now();
      const toRemove: PoolConnection[] = [];

      // Find idle connections to remove
      for (const conn of this.connections) {
        if (
          !conn.inUse &&
          now - conn.lastUsed > this.config.idleTimeout &&
          this.connections.length > this.config.minConnections
        ) {
          toRemove.push(conn);
        }
      }

      // Remove idle connections
      for (const conn of toRemove) {
        const index = this.connections.indexOf(conn);
        if (index > -1) {
          this.connections.splice(index, 1);
          conn.client.close();
          metrics.poolConnections.dec({ state: 'idle' });
        }
      }
    }, 30000); // Run every 30 seconds
  }

  async close(): Promise<void> {
    if (this.cleanupInterval) {
      clearInterval(this.cleanupInterval);
    }

    await Promise.all(
      this.connections.map(async (conn) => {
        try {
          await conn.client.close();
        } catch (error) {
          console.error('Error closing connection:', error);
        }
      })
    );

    this.connections = [];
    metrics.poolConnections.set({ state: 'idle' }, 0);
    metrics.poolConnections.set({ state: 'active' }, 0);
  }

  getStats() {
    return {
      total: this.connections.length,
      active: this.connections.filter(c => c.inUse).length,
      idle: this.connections.filter(c => !c.inUse).length,
      waiting: this.waitQueue.length,
    };
  }
}

/**
 * Vector Client with connection pooling and caching
 */
export class VectorClient {
  private pool: ConnectionPool;
  private cache: LRUCache<string, any>;
  private config: Required<VectorClientConfig>;
  private initialized = false;

  constructor(config: VectorClientConfig) {
    this.config = {
      host: config.host,
      maxConnections: config.maxConnections || 100,
      minConnections: config.minConnections || 10,
      idleTimeout: config.idleTimeout || 60000,
      connectionTimeout: config.connectionTimeout || 5000,
      queryTimeout: config.queryTimeout || 30000,
      retryAttempts: config.retryAttempts || 3,
      retryDelay: config.retryDelay || 1000,
      cacheSize: config.cacheSize || 10000,
      cacheTTL: config.cacheTTL || 300000, // 5 minutes
      enableMetrics: config.enableMetrics !== false,
    };

    this.pool = new ConnectionPool(this.config);
    this.cache = new LRUCache({
      max: this.config.cacheSize,
      ttl: this.config.cacheTTL,
      updateAgeOnGet: true,
      updateAgeOnHas: false,
    });
  }

  async initialize(): Promise<void> {
    if (this.initialized) return;

    const span = tracer.startSpan('initialize-client');

    try {
      // Initialize connection pool
      await new Promise(resolve => setTimeout(resolve, 100)); // Wait for initial connections
      this.initialized = true;
      span.setStatus({ code: SpanStatusCode.OK });
      console.log('Vector client initialized', { config: this.config });
    } catch (error) {
      span.setStatus({ code: SpanStatusCode.ERROR, message: (error as Error).message });
      throw error;
    } finally {
      span.end();
    }
  }

  async query(collection: string, query: any): Promise<QueryResult[]> {
    if (!this.initialized) {
      throw new Error('Client not initialized');
    }

    const cacheKey = getCacheKey(collection, query);
    const cached = this.cache.get(cacheKey);

    if (cached) {
      metrics.cacheHits.inc({ collection });
      return cached;
    }

    metrics.cacheMisses.inc({ collection });

    const span = tracer.startSpan('vector-query', {
      attributes: { collection, cached: false },
    });

    const startTime = Date.now();
    let connection: PoolConnection | null = null;

    try {
      connection = await this.pool.acquire();
      const result = await this.executeWithRetry(
        () => connection!.client.query(collection, query),
        collection,
        'query'
      );

      connection.queryCount++;

      // Cache the result
      this.cache.set(cacheKey, result);

      const duration = (Date.now() - startTime) / 1000;
      metrics.queryDuration.observe({ collection, operation: 'query', cached: 'false' }, duration);
      span.setStatus({ code: SpanStatusCode.OK });

      return result;
    } catch (error) {
      span.setStatus({ code: SpanStatusCode.ERROR, message: (error as Error).message });
      throw error;
    } finally {
      if (connection) {
        this.pool.release(connection);
      }
      span.end();
    }
  }

  async streamQuery(
    collection: string,
    query: any,
    onChunk: (chunk: QueryResult) => void
  ): Promise<void> {
    if (!this.initialized) {
      throw new Error('Client not initialized');
    }

    const span = tracer.startSpan('vector-stream-query', {
      attributes: { collection },
    });

    const startTime = Date.now();
    let connection: PoolConnection | null = null;

    try {
      connection = await this.pool.acquire();

      // TODO: Replace with actual streaming from ruvector binding
      // For now, simulate streaming by chunking results
      const results = await this.executeWithRetry(
        () => connection!.client.query(collection, query),
        collection,
        'stream'
      );

      // Stream results in chunks
      const chunkSize = 10;
      for (let i = 0; i < results.results.length; i += chunkSize) {
        const chunk = results.results.slice(i, i + chunkSize);
        for (const item of chunk) {
          onChunk(item);
        }
        // Small delay to simulate streaming
        await new Promise(resolve => setTimeout(resolve, 10));
      }

      connection.queryCount++;

      const duration = (Date.now() - startTime) / 1000;
      metrics.queryDuration.observe({ collection, operation: 'stream', cached: 'false' }, duration);
      span.setStatus({ code: SpanStatusCode.OK });
    } catch (error) {
      span.setStatus({ code: SpanStatusCode.ERROR, message: (error as Error).message });
      throw error;
    } finally {
      if (connection) {
        this.pool.release(connection);
      }
      span.end();
    }
  }

  async batchQuery(queries: any[]): Promise<any[]> {
    if (!this.initialized) {
      throw new Error('Client not initialized');
    }

    const span = tracer.startSpan('vector-batch-query', {
      attributes: { queryCount: queries.length },
    });

    try {
      // Execute queries in parallel with connection pooling
      const results = await Promise.all(
        queries.map(q => this.query(q.collection, q))
      );

      span.setStatus({ code: SpanStatusCode.OK });
      return results;
    } catch (error) {
      span.setStatus({ code: SpanStatusCode.ERROR, message: (error as Error).message });
      throw error;
    } finally {
      span.end();
    }
  }

  private async executeWithRetry<T>(
    fn: () => Promise<T>,
    collection: string,
    operation: string
  ): Promise<T> {
    let lastError: Error | null = null;

    for (let attempt = 0; attempt <= this.config.retryAttempts; attempt++) {
      try {
        return await Promise.race([
          fn(),
          new Promise<T>((_, reject) =>
            setTimeout(() => reject(new Error('Query timeout')), this.config.queryTimeout)
          ),
        ]);
      } catch (error) {
        lastError = error as Error;

        if (attempt < this.config.retryAttempts) {
          metrics.retries.inc({ collection, reason: lastError.message });
          const delay = this.config.retryDelay * Math.pow(2, attempt); // Exponential backoff
          await new Promise(resolve => setTimeout(resolve, delay));
        }
      }
    }

    throw lastError || new Error('Unknown error during retry');
  }

  async healthCheck(): Promise<boolean> {
    try {
      const stats = this.pool.getStats();
      return stats.total > 0;
    } catch {
      return false;
    }
  }

  async close(): Promise<void> {
    await this.pool.close();
    this.cache.clear();
    this.initialized = false;
    console.log('Vector client closed');
  }

  getStats() {
    return {
      pool: this.pool.getStats(),
      cache: {
        size: this.cache.size,
        max: this.cache.max,
      },
    };
  }

  clearCache(): void {
    this.cache.clear();
  }
}
