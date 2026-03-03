/**
 * Advanced memory management and profiling for matrix operations
 * Implements memory streaming, pooling, and cache optimization
 */

export interface MemoryStats {
  totalAllocated: number;
  totalReleased: number;
  currentUsage: number;
  peakUsage: number;
  poolStats: Record<string, any>;
  gcCount: number;
  cacheHitRate: number;
}

export interface CacheConfig {
  maxSize: number;
  ttl: number; // Time to live in milliseconds
  evictionPolicy: 'lru' | 'lfu' | 'fifo';
}

// LRU Cache implementation for matrix chunks
class LRUCache<K, V> {
  private cache = new Map<K, { value: V; lastUsed: number; useCount: number }>();
  private maxSize: number;
  private ttl: number;
  private hits = 0;
  private misses = 0;

  constructor(config: CacheConfig) {
    this.maxSize = config.maxSize;
    this.ttl = config.ttl;
  }

  get(key: K): V | undefined {
    const entry = this.cache.get(key);

    if (!entry) {
      this.misses++;
      return undefined;
    }

    // Check TTL
    if (Date.now() - entry.lastUsed > this.ttl) {
      this.cache.delete(key);
      this.misses++;
      return undefined;
    }

    entry.lastUsed = Date.now();
    entry.useCount++;
    this.hits++;
    return entry.value;
  }

  set(key: K, value: V): void {
    if (this.cache.size >= this.maxSize) {
      this.evict();
    }

    this.cache.set(key, {
      value,
      lastUsed: Date.now(),
      useCount: 1
    });
  }

  private evict(): void {
    let oldestKey: K | undefined;
    let oldestTime = Infinity;

    for (const [key, entry] of this.cache) {
      if (entry.lastUsed < oldestTime) {
        oldestTime = entry.lastUsed;
        oldestKey = key;
      }
    }

    if (oldestKey !== undefined) {
      this.cache.delete(oldestKey);
    }
  }

  getHitRate(): number {
    const total = this.hits + this.misses;
    return total > 0 ? this.hits / total : 0;
  }

  clear(): void {
    this.cache.clear();
    this.hits = 0;
    this.misses = 0;
  }

  size(): number {
    return this.cache.size;
  }
}

// Memory pool for typed arrays
class TypedArrayPool {
  private pools = new Map<string, Array<ArrayBuffer>>();
  private allocatedBytes = 0;
  private releasedBytes = 0;
  private peakBytes = 0;
  private maxPoolSize = 50;

  acquire(type: 'float64' | 'uint32' | 'uint8', length: number): ArrayBuffer {
    const bytesPerElement = this.getBytesPerElement(type);
    const totalBytes = length * bytesPerElement;
    const key = `${type}_${length}`;

    const pool = this.pools.get(key);
    if (pool && pool.length > 0) {
      const buffer = pool.pop()!;
      this.allocatedBytes += totalBytes;
      this.peakBytes = Math.max(this.peakBytes, this.allocatedBytes - this.releasedBytes);
      return buffer;
    }

    const buffer = new ArrayBuffer(totalBytes);
    this.allocatedBytes += totalBytes;
    this.peakBytes = Math.max(this.peakBytes, this.allocatedBytes - this.releasedBytes);
    return buffer;
  }

  release(type: 'float64' | 'uint32' | 'uint8', buffer: ArrayBuffer): void {
    const length = buffer.byteLength / this.getBytesPerElement(type);
    const key = `${type}_${length}`;

    let pool = this.pools.get(key);
    if (!pool) {
      pool = [];
      this.pools.set(key, pool);
    }

    if (pool.length < this.maxPoolSize) {
      pool.push(buffer);
    }

    this.releasedBytes += buffer.byteLength;
  }

  private getBytesPerElement(type: 'float64' | 'uint32' | 'uint8'): number {
    switch (type) {
      case 'float64': return 8;
      case 'uint32': return 4;
      case 'uint8': return 1;
    }
  }

  getStats(): {
    allocated: number;
    released: number;
    current: number;
    peak: number;
    poolSizes: Record<string, number>;
  } {
    const poolSizes: Record<string, number> = {};
    for (const [key, pool] of this.pools) {
      poolSizes[key] = pool.length;
    }

    return {
      allocated: this.allocatedBytes,
      released: this.releasedBytes,
      current: this.allocatedBytes - this.releasedBytes,
      peak: this.peakBytes,
      poolSizes
    };
  }

  clear(): void {
    this.pools.clear();
    this.allocatedBytes = 0;
    this.releasedBytes = 0;
    this.peakBytes = 0;
  }
}

// Memory streaming manager for large matrix operations
export class MemoryStreamManager {
  private cache: LRUCache<string, any>;
  private arrayPool: TypedArrayPool;
  private gcCount = 0;
  private streamingThreshold: number;

  constructor(
    cacheConfig: CacheConfig = { maxSize: 100, ttl: 300000, evictionPolicy: 'lru' },
    streamingThreshold = 1024 * 1024 * 100 // 100MB threshold
  ) {
    this.cache = new LRUCache(cacheConfig);
    this.arrayPool = new TypedArrayPool();
    this.streamingThreshold = streamingThreshold;

    // Monitor garbage collection
    if (typeof globalThis !== 'undefined' && 'performance' in globalThis) {
      (performance as any).onGC?.(() => this.gcCount++);
    }
  }

  // Stream large matrix data in chunks
  async *streamMatrixChunks<T>(
    data: T[],
    chunkSize: number,
    processor: (chunk: T[]) => Promise<any>
  ): AsyncGenerator<any, void, unknown> {
    for (let i = 0; i < data.length; i += chunkSize) {
      const chunk = data.slice(i, i + chunkSize);
      const cacheKey = `chunk_${i}_${chunkSize}`;

      let result = this.cache.get(cacheKey);
      if (!result) {
        result = await processor(chunk);
        this.cache.set(cacheKey, result);
      }

      yield result;

      // Yield control to prevent blocking
      if (i % (chunkSize * 10) === 0) {
        await new Promise(resolve => setTimeout(resolve, 0));
      }
    }
  }

  // Memory-aware matrix operation scheduling
  async scheduleOperation<T>(
    operation: () => Promise<T>,
    estimatedMemory: number
  ): Promise<T> {
    const currentUsage = this.getCurrentMemoryUsage();

    // If operation would exceed threshold, wait for GC or free cache
    if (currentUsage + estimatedMemory > this.streamingThreshold) {
      await this.freeMemory();
    }

    return operation();
  }

  private async freeMemory(): Promise<void> {
    // Clear oldest cache entries
    this.cache.clear();
    this.arrayPool.clear();

    // Force garbage collection if available
    if (typeof globalThis !== 'undefined' && (globalThis as any).gc) {
      (globalThis as any).gc();
    }

    // Wait a bit for GC to complete
    await new Promise(resolve => setTimeout(resolve, 100));
  }

  private getCurrentMemoryUsage(): number {
    if (typeof globalThis !== 'undefined' && 'performance' in globalThis && 'memory' in performance) {
      return (performance as any).memory.usedJSHeapSize;
    }

    // Fallback to estimated usage from pool
    return this.arrayPool.getStats().current;
  }

  // Acquire optimized typed array
  acquireTypedArray(type: 'float64' | 'uint32' | 'uint8', length: number): any {
    const buffer = this.arrayPool.acquire(type, length);

    switch (type) {
      case 'float64': return new Float64Array(buffer);
      case 'uint32': return new Uint32Array(buffer);
      case 'uint8': return new Uint8Array(buffer);
    }
  }

  // Release typed array back to pool
  releaseTypedArray(array: Float64Array | Uint32Array | Uint8Array): void {
    let type: 'float64' | 'uint32' | 'uint8';

    if (array instanceof Float64Array) type = 'float64';
    else if (array instanceof Uint32Array) type = 'uint32';
    else type = 'uint8';

    this.arrayPool.release(type, array.buffer as ArrayBuffer);
  }

  // Get comprehensive memory statistics
  getMemoryStats(): MemoryStats {
    const poolStats = this.arrayPool.getStats();

    return {
      totalAllocated: poolStats.allocated,
      totalReleased: poolStats.released,
      currentUsage: poolStats.current,
      peakUsage: poolStats.peak,
      poolStats: {
        arrayPool: poolStats.poolSizes,
        cacheSize: this.cache.size(),
        cacheHitRate: this.cache.getHitRate()
      },
      gcCount: this.gcCount,
      cacheHitRate: this.cache.getHitRate()
    };
  }

  // Memory profiler for operations
  async profileOperation<T>(
    name: string,
    operation: () => Promise<T>
  ): Promise<{ result: T; profile: MemoryProfile }> {
    const startStats = this.getMemoryStats();
    const startTime = performance.now();

    const result = await operation();

    const endTime = performance.now();
    const endStats = this.getMemoryStats();

    const profile: MemoryProfile = {
      name,
      duration: endTime - startTime,
      memoryDelta: endStats.currentUsage - startStats.currentUsage,
      peakMemory: endStats.peakUsage,
      allocations: endStats.totalAllocated - startStats.totalAllocated,
      deallocations: endStats.totalReleased - startStats.totalReleased,
      cacheHitRate: endStats.cacheHitRate
    };

    return { result, profile };
  }

  // Optimize cache based on access patterns
  optimizeCache(): void {
    // This could analyze access patterns and adjust cache size/TTL
    const hitRate = this.cache.getHitRate();

    if (hitRate < 0.5) {
      // Low hit rate, might need larger cache or different eviction policy
      console.warn(`Low cache hit rate: ${hitRate.toFixed(2)}`);
    }
  }

  cleanup(): void {
    this.cache.clear();
    this.arrayPool.clear();
  }
}

export interface MemoryProfile {
  name: string;
  duration: number;
  memoryDelta: number;
  peakMemory: number;
  allocations: number;
  deallocations: number;
  cacheHitRate: number;
}

// SIMD-aware memory layout optimizer
export class SIMDMemoryOptimizer {
  private static readonly SIMD_WIDTH = 4; // 4 doubles for AVX
  private static readonly CACHE_LINE_SIZE = 64; // bytes

  // Align arrays for SIMD operations
  static alignForSIMD(length: number): number {
    return Math.ceil(length / this.SIMD_WIDTH) * this.SIMD_WIDTH;
  }

  // Optimize array layout for cache performance
  static optimizeLayout<T>(arrays: T[][], accessPattern: 'row' | 'column'): T[][] {
    if (accessPattern === 'row') {
      // Keep arrays as-is for row-major access
      return arrays;
    } else {
      // Transpose for column-major access
      const rows = arrays.length;
      const cols = arrays[0]?.length || 0;
      const transposed: T[][] = Array(cols).fill(null).map(() => Array(rows));

      for (let i = 0; i < rows; i++) {
        for (let j = 0; j < cols; j++) {
          transposed[j][i] = arrays[i][j];
        }
      }

      return transposed;
    }
  }

  // Pad arrays to avoid false sharing
  static padForCacheLines<T>(array: T[], padValue: T): T[] {
    const elementSize = 8; // Assume 8 bytes per element
    const elementsPerCacheLine = this.CACHE_LINE_SIZE / elementSize;
    const padding = elementsPerCacheLine - (array.length % elementsPerCacheLine);

    if (padding === elementsPerCacheLine) {
      return array;
    }

    return [...array, ...Array(padding).fill(padValue)];
  }

  // Block matrix operations for better cache locality
  static blockMatrixMultiply(
    a: number[][],
    b: number[][],
    result: number[][],
    blockSize = 64
  ): void {
    const n = a.length;
    const m = b[0].length;
    const p = b.length;

    for (let ii = 0; ii < n; ii += blockSize) {
      for (let jj = 0; jj < m; jj += blockSize) {
        for (let kk = 0; kk < p; kk += blockSize) {
          const iEnd = Math.min(ii + blockSize, n);
          const jEnd = Math.min(jj + blockSize, m);
          const kEnd = Math.min(kk + blockSize, p);

          for (let i = ii; i < iEnd; i++) {
            for (let j = jj; j < jEnd; j++) {
              let sum = result[i][j];
              for (let k = kk; k < kEnd; k++) {
                sum += a[i][k] * b[k][j];
              }
              result[i][j] = sum;
            }
          }
        }
      }
    }
  }
}

// Global memory manager instance
export const globalMemoryManager = new MemoryStreamManager();