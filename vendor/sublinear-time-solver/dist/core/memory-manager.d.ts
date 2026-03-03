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
    ttl: number;
    evictionPolicy: 'lru' | 'lfu' | 'fifo';
}
export declare class MemoryStreamManager {
    private cache;
    private arrayPool;
    private gcCount;
    private streamingThreshold;
    constructor(cacheConfig?: CacheConfig, streamingThreshold?: number);
    streamMatrixChunks<T>(data: T[], chunkSize: number, processor: (chunk: T[]) => Promise<any>): AsyncGenerator<any, void, unknown>;
    scheduleOperation<T>(operation: () => Promise<T>, estimatedMemory: number): Promise<T>;
    private freeMemory;
    private getCurrentMemoryUsage;
    acquireTypedArray(type: 'float64' | 'uint32' | 'uint8', length: number): any;
    releaseTypedArray(array: Float64Array | Uint32Array | Uint8Array): void;
    getMemoryStats(): MemoryStats;
    profileOperation<T>(name: string, operation: () => Promise<T>): Promise<{
        result: T;
        profile: MemoryProfile;
    }>;
    optimizeCache(): void;
    cleanup(): void;
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
export declare class SIMDMemoryOptimizer {
    private static readonly SIMD_WIDTH;
    private static readonly CACHE_LINE_SIZE;
    static alignForSIMD(length: number): number;
    static optimizeLayout<T>(arrays: T[][], accessPattern: 'row' | 'column'): T[][];
    static padForCacheLines<T>(array: T[], padValue: T): T[];
    static blockMatrixMultiply(a: number[][], b: number[][], result: number[][], blockSize?: number): void;
}
export declare const globalMemoryManager: MemoryStreamManager;
