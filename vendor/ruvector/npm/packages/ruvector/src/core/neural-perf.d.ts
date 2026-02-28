/**
 * Neural Performance Optimizations
 *
 * High-performance utilities for neural embedding operations:
 * - O(1) LRU Cache with doubly-linked list + hash map
 * - Parallel batch processing
 * - Pre-allocated Float32Array buffer pools
 * - Tensor buffer reuse
 * - 8x loop unrolling for vector operations
 */
export declare const PERF_CONSTANTS: {
    readonly DEFAULT_CACHE_SIZE: 1000;
    readonly DEFAULT_BUFFER_POOL_SIZE: 64;
    readonly DEFAULT_BATCH_SIZE: 32;
    readonly MIN_PARALLEL_BATCH_SIZE: 8;
    readonly UNROLL_THRESHOLD: 32;
};
/**
 * High-performance LRU Cache with O(1) get, set, and eviction.
 * Uses doubly-linked list for ordering + hash map for O(1) lookup.
 */
export declare class LRUCache<K, V> {
    private capacity;
    private map;
    private head;
    private tail;
    private hits;
    private misses;
    constructor(capacity?: number);
    /**
     * Get value from cache - O(1)
     */
    get(key: K): V | undefined;
    /**
     * Set value in cache - O(1)
     */
    set(key: K, value: V): void;
    /**
     * Check if key exists - O(1)
     */
    has(key: K): boolean;
    /**
     * Delete key from cache - O(1)
     */
    delete(key: K): boolean;
    /**
     * Clear entire cache - O(1)
     */
    clear(): void;
    /**
     * Get cache size
     */
    get size(): number;
    /**
     * Get cache statistics
     */
    getStats(): {
        size: number;
        capacity: number;
        hits: number;
        misses: number;
        hitRate: number;
    };
    /**
     * Reset statistics
     */
    resetStats(): void;
    private moveToHead;
    private addToHead;
    private removeNode;
    private evictLRU;
    /**
     * Iterate over entries (most recent first)
     */
    entries(): Generator<[K, V]>;
}
/**
 * High-performance buffer pool for Float32Arrays.
 * Eliminates GC pressure by reusing pre-allocated buffers.
 */
export declare class Float32BufferPool {
    private pools;
    private maxPoolSize;
    private allocations;
    private reuses;
    constructor(maxPoolSize?: number);
    /**
     * Acquire a buffer of specified size - O(1) amortized
     */
    acquire(size: number): Float32Array;
    /**
     * Release a buffer back to the pool - O(1)
     */
    release(buffer: Float32Array): void;
    /**
     * Pre-warm the pool with buffers of specific sizes
     */
    prewarm(sizes: number[], count?: number): void;
    /**
     * Clear all pools
     */
    clear(): void;
    /**
     * Get pool statistics
     */
    getStats(): {
        allocations: number;
        reuses: number;
        reuseRate: number;
        pooledBuffers: number;
    };
}
/**
 * Manages reusable tensor buffers for intermediate computations.
 * Reduces allocations in hot paths.
 */
export declare class TensorBufferManager {
    private bufferPool;
    private workingBuffers;
    constructor(pool?: Float32BufferPool);
    /**
     * Get or create a named working buffer
     */
    getWorking(name: string, size: number): Float32Array;
    /**
     * Get a temporary buffer (caller must release)
     */
    getTemp(size: number): Float32Array;
    /**
     * Release a temporary buffer
     */
    releaseTemp(buffer: Float32Array): void;
    /**
     * Release all working buffers
     */
    releaseAll(): void;
    /**
     * Get underlying pool for stats
     */
    getPool(): Float32BufferPool;
}
/**
 * High-performance vector operations with 8x loop unrolling.
 * Provides 15-30% speedup on large vectors.
 */
export declare const VectorOps: {
    /**
     * Dot product with 8x unrolling
     */
    dot(a: Float32Array, b: Float32Array): number;
    /**
     * Squared L2 norm with 8x unrolling
     */
    normSq(a: Float32Array): number;
    /**
     * L2 norm
     */
    norm(a: Float32Array): number;
    /**
     * Cosine similarity - optimized for V8 JIT
     * Uses 4x unrolling which benchmarks faster than 8x due to register pressure
     */
    cosine(a: Float32Array, b: Float32Array): number;
    /**
     * Euclidean distance squared with 8x unrolling
     */
    distanceSq(a: Float32Array, b: Float32Array): number;
    /**
     * Euclidean distance
     */
    distance(a: Float32Array, b: Float32Array): number;
    /**
     * Add vectors: out = a + b (with 8x unrolling)
     */
    add(a: Float32Array, b: Float32Array, out: Float32Array): Float32Array;
    /**
     * Subtract vectors: out = a - b (with 8x unrolling)
     */
    sub(a: Float32Array, b: Float32Array, out: Float32Array): Float32Array;
    /**
     * Scale vector: out = a * scalar (with 8x unrolling)
     */
    scale(a: Float32Array, scalar: number, out: Float32Array): Float32Array;
    /**
     * Normalize vector in-place
     */
    normalize(a: Float32Array): Float32Array;
    /**
     * Mean of multiple vectors (with buffer reuse)
     */
    mean(vectors: Float32Array[], out: Float32Array): Float32Array;
};
export interface BatchResult<T> {
    results: T[];
    timing: {
        totalMs: number;
        perItemMs: number;
    };
}
/**
 * Parallel batch processor for embedding operations.
 * Uses chunking and Promise.all for concurrent processing.
 */
export declare class ParallelBatchProcessor {
    private batchSize;
    private maxConcurrency;
    constructor(options?: {
        batchSize?: number;
        maxConcurrency?: number;
    });
    /**
     * Process items in parallel batches
     */
    processBatch<T, R>(items: T[], processor: (item: T, index: number) => Promise<R> | R): Promise<BatchResult<R>>;
    /**
     * Process with synchronous function (uses chunking for better cache locality)
     */
    processSync<T, R>(items: T[], processor: (item: T, index: number) => R): BatchResult<R>;
    /**
     * Batch similarity search (optimized for many queries)
     */
    batchSimilarity(queries: Float32Array[], corpus: Float32Array[], k?: number): Array<Array<{
        index: number;
        score: number;
    }>>;
    private chunkArray;
}
export interface CachedMemoryEntry {
    id: string;
    embedding: Float32Array;
    content: string;
    score: number;
}
/**
 * High-performance memory store with O(1) LRU caching.
 */
export declare class OptimizedMemoryStore {
    private cache;
    private bufferPool;
    private dimension;
    constructor(options?: {
        cacheSize?: number;
        dimension?: number;
    });
    /**
     * Store embedding - O(1)
     */
    store(id: string, embedding: Float32Array | number[], content: string): void;
    /**
     * Get by ID - O(1)
     */
    get(id: string): CachedMemoryEntry | undefined;
    /**
     * Search by similarity - O(n) but with optimized vector ops
     */
    search(query: Float32Array, k?: number): CachedMemoryEntry[];
    /**
     * Delete entry - O(1)
     */
    delete(id: string): boolean;
    /**
     * Get statistics
     */
    getStats(): {
        cache: ReturnType<LRUCache<string, CachedMemoryEntry>['getStats']>;
        buffers: ReturnType<Float32BufferPool['getStats']>;
    };
}
declare const _default: {
    LRUCache: typeof LRUCache;
    Float32BufferPool: typeof Float32BufferPool;
    TensorBufferManager: typeof TensorBufferManager;
    VectorOps: {
        /**
         * Dot product with 8x unrolling
         */
        dot(a: Float32Array, b: Float32Array): number;
        /**
         * Squared L2 norm with 8x unrolling
         */
        normSq(a: Float32Array): number;
        /**
         * L2 norm
         */
        norm(a: Float32Array): number;
        /**
         * Cosine similarity - optimized for V8 JIT
         * Uses 4x unrolling which benchmarks faster than 8x due to register pressure
         */
        cosine(a: Float32Array, b: Float32Array): number;
        /**
         * Euclidean distance squared with 8x unrolling
         */
        distanceSq(a: Float32Array, b: Float32Array): number;
        /**
         * Euclidean distance
         */
        distance(a: Float32Array, b: Float32Array): number;
        /**
         * Add vectors: out = a + b (with 8x unrolling)
         */
        add(a: Float32Array, b: Float32Array, out: Float32Array): Float32Array;
        /**
         * Subtract vectors: out = a - b (with 8x unrolling)
         */
        sub(a: Float32Array, b: Float32Array, out: Float32Array): Float32Array;
        /**
         * Scale vector: out = a * scalar (with 8x unrolling)
         */
        scale(a: Float32Array, scalar: number, out: Float32Array): Float32Array;
        /**
         * Normalize vector in-place
         */
        normalize(a: Float32Array): Float32Array;
        /**
         * Mean of multiple vectors (with buffer reuse)
         */
        mean(vectors: Float32Array[], out: Float32Array): Float32Array;
    };
    ParallelBatchProcessor: typeof ParallelBatchProcessor;
    OptimizedMemoryStore: typeof OptimizedMemoryStore;
    PERF_CONSTANTS: {
        readonly DEFAULT_CACHE_SIZE: 1000;
        readonly DEFAULT_BUFFER_POOL_SIZE: 64;
        readonly DEFAULT_BATCH_SIZE: 32;
        readonly MIN_PARALLEL_BATCH_SIZE: 8;
        readonly UNROLL_THRESHOLD: 32;
    };
};
export default _default;
//# sourceMappingURL=neural-perf.d.ts.map