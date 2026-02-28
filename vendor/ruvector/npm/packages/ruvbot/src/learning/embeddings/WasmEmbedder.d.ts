/**
 * WasmEmbedder - WASM-based Text Embedding
 *
 * Provides high-performance text embeddings using RuVector WASM bindings.
 * Supports batching, caching, and SIMD optimization.
 */
import type { Embedder } from '../memory/MemoryManager.js';
export interface WasmEmbedderConfig {
    dimensions: number;
    modelPath?: string;
    cacheSize?: number;
    useSIMD?: boolean;
    batchSize?: number;
}
export interface EmbeddingCache {
    get(key: string): Float32Array | undefined;
    set(key: string, value: Float32Array): void;
    clear(): void;
    size(): number;
}
export declare class WasmEmbedder implements Embedder {
    private readonly config;
    private readonly cache;
    private wasmModule;
    private initialized;
    constructor(config: WasmEmbedderConfig);
    /**
     * Initialize the WASM module
     */
    initialize(): Promise<void>;
    /**
     * Embed a single text string
     */
    embed(text: string): Promise<Float32Array>;
    /**
     * Embed multiple texts in batch
     */
    embedBatch(texts: string[]): Promise<Float32Array[]>;
    /**
     * Get embedding dimensions
     */
    dimension(): number;
    /**
     * Clear the embedding cache
     */
    clearCache(): void;
    /**
     * Get cache statistics
     */
    getCacheStats(): {
        size: number;
        maxSize: number;
    };
    private generateEmbedding;
    private generateEmbeddingBatch;
    private fallbackEmbed;
    private hashCode;
}
export declare function createWasmEmbedder(config: WasmEmbedderConfig): WasmEmbedder;
export default WasmEmbedder;
//# sourceMappingURL=WasmEmbedder.d.ts.map