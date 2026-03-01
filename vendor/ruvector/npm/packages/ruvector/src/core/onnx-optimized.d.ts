/**
 * Optimized ONNX Embedder for RuVector
 *
 * Performance optimizations:
 * 1. TOKENIZER CACHING - Cache tokenization results (~10-20ms savings per repeat)
 * 2. EMBEDDING LRU CACHE - Full embedding cache with configurable size
 * 3. QUANTIZED MODELS - INT8/FP16 models for 2-4x speedup
 * 4. LAZY INITIALIZATION - Defer model loading until first use
 * 5. DYNAMIC BATCHING - Optimize batch sizes based on input
 * 6. MEMORY OPTIMIZATION - Float32Array for all operations
 *
 * Usage:
 *   const embedder = new OptimizedOnnxEmbedder({ cacheSize: 1000 });
 *   await embedder.init();
 *   const embedding = await embedder.embed("Hello world");
 */
export interface OptimizedOnnxConfig {
    /** Model to use (default: 'all-MiniLM-L6-v2') */
    modelId?: string;
    /** Use quantized model if available (default: true) */
    useQuantized?: boolean;
    /** Quantization type: 'fp16' | 'int8' | 'dynamic' */
    quantization?: 'fp16' | 'int8' | 'dynamic' | 'none';
    /** Max input length (default: 256) */
    maxLength?: number;
    /** Embedding cache size (default: 512) */
    cacheSize?: number;
    /** Tokenizer cache size (default: 256) */
    tokenizerCacheSize?: number;
    /** Enable lazy initialization (default: true) */
    lazyInit?: boolean;
    /** Batch size for dynamic batching (default: 32) */
    batchSize?: number;
    /** Minimum texts to trigger batching (default: 4) */
    batchThreshold?: number;
}
export declare class OptimizedOnnxEmbedder {
    private config;
    private wasmModule;
    private embedder;
    private initialized;
    private initPromise;
    private embeddingCache;
    private tokenizerCache;
    private totalEmbeds;
    private totalTimeMs;
    private dimension;
    constructor(config?: OptimizedOnnxConfig);
    /**
     * Initialize the embedder (loads model)
     */
    init(): Promise<void>;
    private doInit;
    /**
     * Embed a single text with caching
     */
    embed(text: string): Promise<Float32Array>;
    /**
     * Embed multiple texts with batching and caching
     */
    embedBatch(texts: string[]): Promise<Float32Array[]>;
    /**
     * Calculate similarity between two texts
     */
    similarity(text1: string, text2: string): Promise<number>;
    /**
     * Fast cosine similarity with loop unrolling
     */
    cosineSimilarity(a: Float32Array, b: Float32Array): number;
    /**
     * Get cache statistics
     */
    getCacheStats(): {
        embedding: {
            hits: number;
            misses: number;
            hitRate: number;
            size: number;
        };
        tokenizer: {
            hits: number;
            misses: number;
            hitRate: number;
            size: number;
        };
        avgTimeMs: number;
        totalEmbeds: number;
    };
    /**
     * Clear all caches
     */
    clearCache(): void;
    /**
     * Get embedding dimension
     */
    getDimension(): number;
    /**
     * Check if initialized
     */
    isReady(): boolean;
    /**
     * Get configuration
     */
    getConfig(): Required<OptimizedOnnxConfig>;
}
export declare function getOptimizedOnnxEmbedder(config?: OptimizedOnnxConfig): OptimizedOnnxEmbedder;
export declare function initOptimizedOnnx(config?: OptimizedOnnxConfig): Promise<OptimizedOnnxEmbedder>;
export default OptimizedOnnxEmbedder;
//# sourceMappingURL=onnx-optimized.d.ts.map