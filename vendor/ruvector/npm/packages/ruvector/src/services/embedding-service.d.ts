/**
 * Embedding Service - Unified embedding generation and management
 *
 * This service provides a unified interface for generating, caching, and
 * managing embeddings from various sources (local models, APIs, etc.)
 */
/**
 * Embedding provider interface
 */
export interface EmbeddingProvider {
    /** Provider name */
    name: string;
    /** Generate embeddings for texts */
    embed(texts: string[]): Promise<number[][]>;
    /** Get embedding dimensions */
    getDimensions(): number;
}
/**
 * Embedding service configuration
 */
export interface EmbeddingServiceConfig {
    /** Default provider to use */
    defaultProvider?: string;
    /** Maximum cache size */
    maxCacheSize?: number;
    /** Cache TTL in milliseconds */
    cacheTtl?: number;
    /** Batch size for embedding generation */
    batchSize?: number;
}
/**
 * Mock embedding provider for testing
 */
export declare class MockEmbeddingProvider implements EmbeddingProvider {
    name: string;
    private dimensions;
    constructor(dimensions?: number);
    embed(texts: string[]): Promise<number[][]>;
    getDimensions(): number;
}
/**
 * Simple local embedding using character n-grams
 * This is a fallback when no external provider is available
 */
export declare class LocalNGramProvider implements EmbeddingProvider {
    name: string;
    private dimensions;
    private ngramSize;
    constructor(dimensions?: number, ngramSize?: number);
    embed(texts: string[]): Promise<number[][]>;
    private embedSingle;
    private hashNgram;
    getDimensions(): number;
}
/**
 * Embedding service with caching and batching
 */
export declare class EmbeddingService {
    private providers;
    private cache;
    private config;
    constructor(config?: EmbeddingServiceConfig);
    /**
     * Register an embedding provider
     */
    registerProvider(provider: EmbeddingProvider): void;
    /**
     * Get a registered provider
     */
    getProvider(name?: string): EmbeddingProvider;
    /**
     * Generate embeddings for texts with caching
     *
     * @param texts - Texts to embed
     * @param provider - Provider name (uses default if not specified)
     * @returns Array of embeddings
     */
    embed(texts: string[], provider?: string): Promise<number[][]>;
    /**
     * Generate a single embedding
     */
    embedOne(text: string, provider?: string): Promise<number[]>;
    /**
     * Add entry to cache with LRU eviction
     */
    private addToCache;
    /**
     * Compute cosine similarity between two embeddings
     */
    cosineSimilarity(a: number[], b: number[]): number;
    /**
     * Find most similar texts from a corpus
     */
    findSimilar(query: string, corpus: string[], k?: number, provider?: string): Promise<{
        text: string;
        similarity: number;
        index: number;
    }[]>;
    /**
     * Get cache statistics
     */
    getCacheStats(): {
        size: number;
        maxSize: number;
        hitRate: number;
    };
    /**
     * Clear the cache
     */
    clearCache(): void;
    /**
     * Get embedding dimensions for a provider
     */
    getDimensions(provider?: string): number;
    /**
     * List available providers
     */
    listProviders(): string[];
}
/**
 * Create an embedding service instance
 */
export declare function createEmbeddingService(config?: EmbeddingServiceConfig): EmbeddingService;
/**
 * Get the default embedding service instance
 */
export declare function getDefaultEmbeddingService(): EmbeddingService;
declare const _default: {
    EmbeddingService: typeof EmbeddingService;
    LocalNGramProvider: typeof LocalNGramProvider;
    MockEmbeddingProvider: typeof MockEmbeddingProvider;
    createEmbeddingService: typeof createEmbeddingService;
    getDefaultEmbeddingService: typeof getDefaultEmbeddingService;
};
export default _default;
//# sourceMappingURL=embedding-service.d.ts.map