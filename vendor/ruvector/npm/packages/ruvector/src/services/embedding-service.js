"use strict";
/**
 * Embedding Service - Unified embedding generation and management
 *
 * This service provides a unified interface for generating, caching, and
 * managing embeddings from various sources (local models, APIs, etc.)
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.EmbeddingService = exports.LocalNGramProvider = exports.MockEmbeddingProvider = void 0;
exports.createEmbeddingService = createEmbeddingService;
exports.getDefaultEmbeddingService = getDefaultEmbeddingService;
/**
 * Simple hash function for cache keys
 */
function hashText(text) {
    let hash = 0;
    for (let i = 0; i < text.length; i++) {
        const char = text.charCodeAt(i);
        hash = ((hash << 5) - hash) + char;
        hash = hash & hash;
    }
    return `h${hash.toString(36)}`;
}
/**
 * Mock embedding provider for testing
 */
class MockEmbeddingProvider {
    constructor(dimensions = 384) {
        this.name = 'mock';
        this.dimensions = dimensions;
    }
    async embed(texts) {
        return texts.map(text => {
            // Generate deterministic pseudo-random embeddings based on text
            const embedding = [];
            let seed = 0;
            for (let i = 0; i < text.length; i++) {
                seed = ((seed << 5) - seed + text.charCodeAt(i)) | 0;
            }
            for (let i = 0; i < this.dimensions; i++) {
                seed = (seed * 1103515245 + 12345) | 0;
                embedding.push((seed % 1000) / 1000 - 0.5);
            }
            // Normalize
            const norm = Math.sqrt(embedding.reduce((s, v) => s + v * v, 0));
            return embedding.map(v => v / (norm || 1));
        });
    }
    getDimensions() {
        return this.dimensions;
    }
}
exports.MockEmbeddingProvider = MockEmbeddingProvider;
/**
 * Simple local embedding using character n-grams
 * This is a fallback when no external provider is available
 */
class LocalNGramProvider {
    constructor(dimensions = 256, ngramSize = 3) {
        this.name = 'local-ngram';
        this.dimensions = dimensions;
        this.ngramSize = ngramSize;
    }
    async embed(texts) {
        return texts.map(text => this.embedSingle(text));
    }
    embedSingle(text) {
        const embedding = new Array(this.dimensions).fill(0);
        const normalized = text.toLowerCase().replace(/[^a-z0-9]/g, ' ');
        // Generate n-grams and hash them into embedding dimensions
        for (let i = 0; i <= normalized.length - this.ngramSize; i++) {
            const ngram = normalized.slice(i, i + this.ngramSize);
            const hash = this.hashNgram(ngram);
            const idx = Math.abs(hash) % this.dimensions;
            embedding[idx] += hash > 0 ? 1 : -1;
        }
        // Normalize
        const norm = Math.sqrt(embedding.reduce((s, v) => s + v * v, 0));
        return embedding.map(v => v / (norm || 1));
    }
    hashNgram(ngram) {
        let hash = 0;
        for (let i = 0; i < ngram.length; i++) {
            hash = ((hash << 5) - hash + ngram.charCodeAt(i)) | 0;
        }
        return hash;
    }
    getDimensions() {
        return this.dimensions;
    }
}
exports.LocalNGramProvider = LocalNGramProvider;
/**
 * Embedding service with caching and batching
 */
class EmbeddingService {
    constructor(config = {}) {
        this.providers = new Map();
        this.cache = new Map();
        this.config = {
            defaultProvider: config.defaultProvider ?? 'local-ngram',
            maxCacheSize: config.maxCacheSize ?? 10000,
            cacheTtl: config.cacheTtl ?? 3600000, // 1 hour
            batchSize: config.batchSize ?? 32,
        };
        // Register default providers
        this.registerProvider(new LocalNGramProvider());
        this.registerProvider(new MockEmbeddingProvider());
    }
    /**
     * Register an embedding provider
     */
    registerProvider(provider) {
        this.providers.set(provider.name, provider);
    }
    /**
     * Get a registered provider
     */
    getProvider(name) {
        const providerName = name ?? this.config.defaultProvider;
        const provider = this.providers.get(providerName);
        if (!provider) {
            throw new Error(`Provider not found: ${providerName}`);
        }
        return provider;
    }
    /**
     * Generate embeddings for texts with caching
     *
     * @param texts - Texts to embed
     * @param provider - Provider name (uses default if not specified)
     * @returns Array of embeddings
     */
    async embed(texts, provider) {
        const providerInstance = this.getProvider(provider);
        const providerName = providerInstance.name;
        const now = Date.now();
        // Check cache and collect texts that need embedding
        const results = new Array(texts.length).fill(null);
        const uncachedIndices = [];
        const uncachedTexts = [];
        for (let i = 0; i < texts.length; i++) {
            const cacheKey = `${providerName}:${hashText(texts[i])}`;
            const cached = this.cache.get(cacheKey);
            if (cached && now - cached.timestamp < this.config.cacheTtl) {
                results[i] = cached.embedding;
                cached.hits++;
            }
            else {
                uncachedIndices.push(i);
                uncachedTexts.push(texts[i]);
            }
        }
        // Generate embeddings for uncached texts in batches
        if (uncachedTexts.length > 0) {
            const batches = [];
            for (let i = 0; i < uncachedTexts.length; i += this.config.batchSize) {
                batches.push(uncachedTexts.slice(i, i + this.config.batchSize));
            }
            let batchOffset = 0;
            for (const batch of batches) {
                const embeddings = await providerInstance.embed(batch);
                for (let j = 0; j < embeddings.length; j++) {
                    const originalIndex = uncachedIndices[batchOffset + j];
                    results[originalIndex] = embeddings[j];
                    // Cache the result
                    const cacheKey = `${providerName}:${hashText(texts[originalIndex])}`;
                    this.addToCache(cacheKey, embeddings[j], now);
                }
                batchOffset += batch.length;
            }
        }
        return results;
    }
    /**
     * Generate a single embedding
     */
    async embedOne(text, provider) {
        const results = await this.embed([text], provider);
        return results[0];
    }
    /**
     * Add entry to cache with LRU eviction
     */
    addToCache(key, embedding, timestamp) {
        // Evict old entries if cache is full
        if (this.cache.size >= this.config.maxCacheSize) {
            // Find and remove least recently used entry
            let oldestKey = '';
            let oldestTime = Infinity;
            let lowestHits = Infinity;
            for (const [k, v] of this.cache.entries()) {
                if (v.hits < lowestHits || (v.hits === lowestHits && v.timestamp < oldestTime)) {
                    oldestKey = k;
                    oldestTime = v.timestamp;
                    lowestHits = v.hits;
                }
            }
            if (oldestKey) {
                this.cache.delete(oldestKey);
            }
        }
        this.cache.set(key, { embedding, timestamp, hits: 0 });
    }
    /**
     * Compute cosine similarity between two embeddings
     */
    cosineSimilarity(a, b) {
        if (a.length !== b.length) {
            throw new Error('Embeddings must have same dimensions');
        }
        let dotProduct = 0;
        let normA = 0;
        let normB = 0;
        for (let i = 0; i < a.length; i++) {
            dotProduct += a[i] * b[i];
            normA += a[i] * a[i];
            normB += b[i] * b[i];
        }
        const denom = Math.sqrt(normA) * Math.sqrt(normB);
        return denom === 0 ? 0 : dotProduct / denom;
    }
    /**
     * Find most similar texts from a corpus
     */
    async findSimilar(query, corpus, k = 5, provider) {
        const [queryEmbed, ...corpusEmbeds] = await this.embed([query, ...corpus], provider);
        const results = corpusEmbeds.map((embed, i) => ({
            text: corpus[i],
            similarity: this.cosineSimilarity(queryEmbed, embed),
            index: i,
        }));
        return results
            .sort((a, b) => b.similarity - a.similarity)
            .slice(0, k);
    }
    /**
     * Get cache statistics
     */
    getCacheStats() {
        let totalHits = 0;
        for (const entry of this.cache.values()) {
            totalHits += entry.hits;
        }
        return {
            size: this.cache.size,
            maxSize: this.config.maxCacheSize,
            hitRate: this.cache.size > 0 ? totalHits / this.cache.size : 0,
        };
    }
    /**
     * Clear the cache
     */
    clearCache() {
        this.cache.clear();
    }
    /**
     * Get embedding dimensions for a provider
     */
    getDimensions(provider) {
        return this.getProvider(provider).getDimensions();
    }
    /**
     * List available providers
     */
    listProviders() {
        return Array.from(this.providers.keys());
    }
}
exports.EmbeddingService = EmbeddingService;
/**
 * Create an embedding service instance
 */
function createEmbeddingService(config) {
    return new EmbeddingService(config);
}
// Singleton instance
let defaultService = null;
/**
 * Get the default embedding service instance
 */
function getDefaultEmbeddingService() {
    if (!defaultService) {
        defaultService = new EmbeddingService();
    }
    return defaultService;
}
exports.default = {
    EmbeddingService,
    LocalNGramProvider,
    MockEmbeddingProvider,
    createEmbeddingService,
    getDefaultEmbeddingService,
};
//# sourceMappingURL=embedding-service.js.map