"use strict";
/**
 * WasmEmbedder - WASM-based Text Embedding
 *
 * Provides high-performance text embeddings using RuVector WASM bindings.
 * Supports batching, caching, and SIMD optimization.
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.WasmEmbedder = void 0;
exports.createWasmEmbedder = createWasmEmbedder;
const errors_js_1 = require("../../core/errors.js");
// ============================================================================
// Simple LRU Cache Implementation
// ============================================================================
class LRUCache {
    constructor(maxSize = 10000) {
        this.cache = new Map();
        this.maxSize = maxSize;
    }
    get(key) {
        const value = this.cache.get(key);
        if (value) {
            // Move to end (most recently used)
            this.cache.delete(key);
            this.cache.set(key, value);
        }
        return value;
    }
    set(key, value) {
        if (this.cache.has(key)) {
            this.cache.delete(key);
        }
        else if (this.cache.size >= this.maxSize) {
            // Remove oldest entry
            const firstKey = this.cache.keys().next().value;
            if (firstKey) {
                this.cache.delete(firstKey);
            }
        }
        this.cache.set(key, value);
    }
    clear() {
        this.cache.clear();
    }
    size() {
        return this.cache.size;
    }
}
// ============================================================================
// WasmEmbedder Implementation
// ============================================================================
class WasmEmbedder {
    constructor(config) {
        this.wasmModule = null;
        this.initialized = false;
        this.config = {
            dimensions: config.dimensions,
            modelPath: config.modelPath,
            cacheSize: config.cacheSize ?? 10000,
            useSIMD: config.useSIMD ?? true,
            batchSize: config.batchSize ?? 32,
        };
        this.cache = new LRUCache(this.config.cacheSize);
    }
    /**
     * Initialize the WASM module
     */
    async initialize() {
        if (this.initialized)
            return;
        try {
            // Try to load @ruvector/ruvllm (WASM module)
            try {
                // Dynamic import - may not be available
                const ruvllm = await Promise.resolve().then(() => __importStar(require('@ruvector/ruvllm')));
                this.wasmModule = ruvllm;
            }
            catch {
                // Use fallback embedder if no WASM available
                console.warn('No WASM module available, using fallback embedder');
            }
            this.initialized = true;
        }
        catch (error) {
            throw new errors_js_1.WasmError(`Failed to initialize WASM embedder: ${error instanceof Error ? error.message : 'Unknown error'}`, { config: this.config });
        }
    }
    /**
     * Embed a single text string
     */
    async embed(text) {
        if (!this.initialized) {
            await this.initialize();
        }
        // Check cache
        const cached = this.cache.get(text);
        if (cached) {
            return cached;
        }
        // Generate embedding
        const embedding = await this.generateEmbedding(text);
        // Cache result
        this.cache.set(text, embedding);
        return embedding;
    }
    /**
     * Embed multiple texts in batch
     */
    async embedBatch(texts) {
        if (!this.initialized) {
            await this.initialize();
        }
        const results = [];
        const uncached = [];
        // Check cache for each text
        for (let i = 0; i < texts.length; i++) {
            const cached = this.cache.get(texts[i]);
            if (cached) {
                results[i] = cached;
            }
            else {
                uncached.push({ index: i, text: texts[i] });
            }
        }
        // Generate embeddings for uncached texts in batches
        const batchSize = this.config.batchSize;
        for (let i = 0; i < uncached.length; i += batchSize) {
            const batch = uncached.slice(i, i + batchSize);
            const batchTexts = batch.map(item => item.text);
            const embeddings = await this.generateEmbeddingBatch(batchTexts);
            for (let j = 0; j < batch.length; j++) {
                const embedding = embeddings[j];
                results[batch[j].index] = embedding;
                this.cache.set(batch[j].text, embedding);
            }
        }
        return results;
    }
    /**
     * Get embedding dimensions
     */
    dimension() {
        return this.config.dimensions;
    }
    /**
     * Clear the embedding cache
     */
    clearCache() {
        this.cache.clear();
    }
    /**
     * Get cache statistics
     */
    getCacheStats() {
        return {
            size: this.cache.size(),
            maxSize: this.config.cacheSize,
        };
    }
    // ==========================================================================
    // Private Methods
    // ==========================================================================
    async generateEmbedding(text) {
        if (this.wasmModule) {
            // Use WASM module if available
            const module = this.wasmModule;
            if (module.embed) {
                return module.embed(text);
            }
            if (module.RuvLLM) {
                return module.RuvLLM.embed(text);
            }
        }
        // Fallback: Generate deterministic pseudo-random embedding
        return this.fallbackEmbed(text);
    }
    async generateEmbeddingBatch(texts) {
        if (this.wasmModule) {
            const module = this.wasmModule;
            if (module.embedBatch) {
                return module.embedBatch(texts);
            }
        }
        // Fallback: Generate individually
        return Promise.all(texts.map(text => this.generateEmbedding(text)));
    }
    fallbackEmbed(text) {
        // Generate deterministic embedding based on text hash
        // This is for testing/development when WASM is not available
        const embedding = new Float32Array(this.config.dimensions);
        let hash = this.hashCode(text);
        for (let i = 0; i < this.config.dimensions; i++) {
            hash = ((hash * 1103515245) + 12345) & 0x7fffffff;
            embedding[i] = (hash / 0x7fffffff) * 2 - 1;
        }
        // Normalize
        const norm = Math.sqrt(embedding.reduce((sum, val) => sum + val * val, 0));
        for (let i = 0; i < this.config.dimensions; i++) {
            embedding[i] /= norm;
        }
        return embedding;
    }
    hashCode(str) {
        let hash = 0;
        for (let i = 0; i < str.length; i++) {
            const char = str.charCodeAt(i);
            hash = ((hash << 5) - hash) + char;
            hash = hash & hash;
        }
        return Math.abs(hash);
    }
}
exports.WasmEmbedder = WasmEmbedder;
// ============================================================================
// Factory Function
// ============================================================================
function createWasmEmbedder(config) {
    return new WasmEmbedder(config);
}
exports.default = WasmEmbedder;
//# sourceMappingURL=WasmEmbedder.js.map