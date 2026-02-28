/**
 * TensorCompress - Adaptive tensor compression for intelligence storage
 * Provides 10x memory savings with access-frequency based compression
 */
export type CompressionLevel = 'none' | 'half' | 'pq8' | 'pq4' | 'binary';
export interface CompressedTensor {
    data: number[] | Uint8Array | Uint16Array | Float32Array;
    level: CompressionLevel;
    originalDim: number;
    accessCount: number;
    lastAccess: number;
    created: number;
}
export interface CompressionStats {
    totalTensors: number;
    byLevel: Record<CompressionLevel, number>;
    originalBytes: number;
    compressedBytes: number;
    savingsPercent: number;
}
export interface CompressionConfig {
    hotThreshold: number;
    warmThreshold: number;
    coolThreshold: number;
    coldThreshold: number;
    autoCompress: boolean;
    compressIntervalMs: number;
}
export declare class TensorCompress {
    private config;
    private tensors;
    private totalAccesses;
    private compressTimer;
    constructor(config?: Partial<CompressionConfig>);
    /**
     * Store a tensor with automatic compression based on access patterns
     */
    store(id: string, tensor: Float32Array | number[], level?: CompressionLevel): void;
    /**
     * Retrieve and decompress a tensor
     */
    get(id: string): Float32Array | null;
    /**
     * Check if tensor exists
     */
    has(id: string): boolean;
    /**
     * Delete a tensor
     */
    delete(id: string): boolean;
    /**
     * Get all tensor IDs
     */
    keys(): string[];
    /**
     * Compress tensor to specified level
     */
    private compress;
    /**
     * Decompress tensor back to Float32Array
     */
    private decompress;
    /**
     * Float16 conversion (approximate)
     */
    private toFloat16;
    private fromFloat16;
    private floatToHalf;
    private halfToFloat;
    /**
     * Product Quantization 8-bit
     */
    private toPQ8;
    private fromPQ8;
    /**
     * Product Quantization 4-bit (packed)
     */
    private toPQ4;
    private fromPQ4;
    /**
     * Binary quantization (1-bit per value)
     */
    private toBinary;
    private fromBinary;
    /**
     * Calculate access frequency for a tensor
     */
    private getAccessFrequency;
    /**
     * Determine optimal compression level based on access frequency
     */
    getOptimalLevel(id: string): CompressionLevel;
    /**
     * Recompress all tensors based on current access patterns
     */
    recompressAll(): CompressionStats;
    /**
     * Get compressed size in bytes
     */
    private getCompressedSize;
    /**
     * Get compression statistics
     */
    getStats(): CompressionStats;
    /**
     * Start auto-compression timer
     */
    private startAutoCompress;
    /**
     * Stop auto-compression
     */
    stopAutoCompress(): void;
    /**
     * Export all tensors for persistence
     */
    export(): {
        tensors: Record<string, any>;
        totalAccesses: number;
    };
    /**
     * Import tensors from persistence
     */
    import(data: {
        tensors: Record<string, any>;
        totalAccesses: number;
    }): void;
    private restoreTypedArray;
    /**
     * Clear all tensors
     */
    clear(): void;
}
export default TensorCompress;
//# sourceMappingURL=tensor-compress.d.ts.map