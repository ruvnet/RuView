/**
 * Persistence Layer - PostgreSQL, RuVector, Redis
 */

export interface PersistenceLayer {
  postgres: PostgresAdapter;
  vectorStore: VectorStoreAdapter;
  cache: CacheAdapter;
}

export interface PostgresAdapter {
  query<T>(sql: string, params?: unknown[]): Promise<T[]>;
  transaction<T>(fn: (tx: Transaction) => Promise<T>): Promise<T>;
  migrate(direction: 'up' | 'down'): Promise<void>;
}

export interface Transaction {
  query<T>(sql: string, params?: unknown[]): Promise<T[]>;
  commit(): Promise<void>;
  rollback(): Promise<void>;
}

export interface VectorStoreAdapter {
  createIndex(config: IndexConfig): Promise<IndexHandle>;
  deleteIndex(handle: IndexHandle): Promise<void>;
  getIndex(namespace: string): Promise<IndexHandle | null>;
  insert(handle: IndexHandle, entries: VectorEntry[]): Promise<void>;
  search(handle: IndexHandle, query: Float32Array, options: SearchOptions): Promise<SearchResult[]>;
  delete(handle: IndexHandle, ids: string[]): Promise<void>;
}

export interface IndexConfig {
  namespace: string;
  dimensions: number;
  distanceMetric: 'cosine' | 'euclidean' | 'dot_product';
  hnsw: {
    m: number;
    efConstruction: number;
    efSearch: number;
  };
}

export interface IndexHandle {
  namespace: string;
  dimensions: number;
}

export interface VectorEntry {
  id: string;
  vector: Float32Array;
  metadata?: Record<string, unknown>;
}

export interface SearchOptions {
  k: number;
  threshold?: number;
  filter?: Record<string, unknown>;
}

export interface SearchResult {
  id: string;
  score: number;
  metadata?: Record<string, unknown>;
}

export interface CacheAdapter {
  get<T>(key: string): Promise<T | null>;
  set<T>(key: string, value: T, ttl?: number): Promise<void>;
  delete(key: string): Promise<void>;
  invalidate(pattern: string): Promise<void>;
}
