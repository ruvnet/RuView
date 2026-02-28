/**
 * Context caching system for performance optimization
 */

import { CacheStrategy, CacheError } from '../types.js';

export interface CacheEntry<T = unknown> {
  key: string;
  value: T;
  timestamp: number;
  ttl: number;
  hits: number;
}

export interface CacheOptions {
  strategy: CacheStrategy;
  ttl: number;
  maxSize?: number;
  onEvict?: (key: string, value: unknown) => void;
}

export abstract class CacheStore {
  abstract get<T>(key: string): Promise<T | null>;
  abstract set<T>(key: string, value: T, ttl?: number): Promise<void>;
  abstract has(key: string): Promise<boolean>;
  abstract delete(key: string): Promise<boolean>;
  abstract clear(): Promise<void>;
  abstract size(): Promise<number>;
}

/**
 * In-memory cache implementation with LRU eviction
 */
export class MemoryCache extends CacheStore {
  private cache: Map<string, CacheEntry>;
  private maxSize: number;
  private defaultTTL: number;
  private onEvict?: (key: string, value: unknown) => void;

  constructor(options: Omit<CacheOptions, 'strategy'>) {
    super();
    this.cache = new Map();
    this.maxSize = options.maxSize || 1000;
    this.defaultTTL = options.ttl;
    this.onEvict = options.onEvict;
  }

  async get<T>(key: string): Promise<T | null> {
    const entry = this.cache.get(key);

    if (!entry) {
      return null;
    }

    // Check if expired
    if (Date.now() - entry.timestamp > entry.ttl * 1000) {
      await this.delete(key);
      return null;
    }

    // Update hits and move to end (LRU)
    entry.hits++;
    this.cache.delete(key);
    this.cache.set(key, entry);

    return entry.value as T;
  }

  async set<T>(key: string, value: T, ttl?: number): Promise<void> {
    // Evict if at max size
    if (this.cache.size >= this.maxSize && !this.cache.has(key)) {
      await this.evictLRU();
    }

    const entry: CacheEntry<T> = {
      key,
      value,
      timestamp: Date.now(),
      ttl: ttl || this.defaultTTL,
      hits: 0
    };

    this.cache.set(key, entry);
  }

  async has(key: string): Promise<boolean> {
    const value = await this.get(key);
    return value !== null;
  }

  async delete(key: string): Promise<boolean> {
    const entry = this.cache.get(key);
    const deleted = this.cache.delete(key);

    if (deleted && entry && this.onEvict) {
      this.onEvict(key, entry.value);
    }

    return deleted;
  }

  async clear(): Promise<void> {
    if (this.onEvict) {
      for (const [key, entry] of this.cache.entries()) {
        this.onEvict(key, entry.value);
      }
    }
    this.cache.clear();
  }

  async size(): Promise<number> {
    return this.cache.size;
  }

  private async evictLRU(): Promise<void> {
    // First entry is least recently used
    const firstKey = this.cache.keys().next().value;
    if (firstKey) {
      await this.delete(firstKey);
    }
  }

  /**
   * Get cache statistics
   */
  getStats() {
    let totalHits = 0;
    let expiredCount = 0;
    const now = Date.now();

    for (const entry of this.cache.values()) {
      totalHits += entry.hits;
      if (now - entry.timestamp > entry.ttl * 1000) {
        expiredCount++;
      }
    }

    return {
      size: this.cache.size,
      maxSize: this.maxSize,
      totalHits,
      expiredCount,
      hitRate: totalHits / (this.cache.size || 1)
    };
  }
}

/**
 * No-op cache for disabled caching
 */
export class NoCache extends CacheStore {
  async get<T>(): Promise<T | null> {
    return null;
  }

  async set<T>(): Promise<void> {
    // No-op
  }

  async has(): Promise<boolean> {
    return false;
  }

  async delete(): Promise<boolean> {
    return false;
  }

  async clear(): Promise<void> {
    // No-op
  }

  async size(): Promise<number> {
    return 0;
  }
}

/**
 * Cache manager factory
 */
export class CacheManager {
  private store: CacheStore;

  constructor(options: CacheOptions) {
    switch (options.strategy) {
      case 'memory':
        this.store = new MemoryCache(options);
        break;
      case 'none':
        this.store = new NoCache();
        break;
      case 'disk':
        // TODO: Implement disk cache
        throw new CacheError('Disk cache not yet implemented', { strategy: 'disk' });
      default:
        throw new CacheError(`Unknown cache strategy: ${options.strategy}`, {
          strategy: options.strategy
        });
    }
  }

  /**
   * Get value from cache
   */
  async get<T>(key: string): Promise<T | null> {
    try {
      return await this.store.get<T>(key);
    } catch (error) {
      throw new CacheError('Failed to get cache value', { key, error });
    }
  }

  /**
   * Set value in cache
   */
  async set<T>(key: string, value: T, ttl?: number): Promise<void> {
    try {
      await this.store.set(key, value, ttl);
    } catch (error) {
      throw new CacheError('Failed to set cache value', { key, error });
    }
  }

  /**
   * Check if key exists in cache
   */
  async has(key: string): Promise<boolean> {
    try {
      return await this.store.has(key);
    } catch (error) {
      throw new CacheError('Failed to check cache key', { key, error });
    }
  }

  /**
   * Delete key from cache
   */
  async delete(key: string): Promise<boolean> {
    try {
      return await this.store.delete(key);
    } catch (error) {
      throw new CacheError('Failed to delete cache key', { key, error });
    }
  }

  /**
   * Clear all cache entries
   */
  async clear(): Promise<void> {
    try {
      await this.store.clear();
    } catch (error) {
      throw new CacheError('Failed to clear cache', { error });
    }
  }

  /**
   * Get cache size
   */
  async size(): Promise<number> {
    try {
      return await this.store.size();
    } catch (error) {
      throw new CacheError('Failed to get cache size', { error });
    }
  }

  /**
   * Generate cache key from parameters
   */
  static generateKey(prefix: string, params: Record<string, unknown>): string {
    const sorted = Object.keys(params)
      .sort()
      .map(key => `${key}:${JSON.stringify(params[key])}`)
      .join('|');
    return `${prefix}:${sorted}`;
  }
}

export { CacheStrategy, CacheError };
