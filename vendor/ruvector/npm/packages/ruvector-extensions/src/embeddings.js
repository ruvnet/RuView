"use strict";
/**
 * @fileoverview Comprehensive embeddings integration module for ruvector-extensions
 * Supports multiple providers: OpenAI, Cohere, Anthropic, and local HuggingFace models
 *
 * @module embeddings
 * @author ruv.io Team <info@ruv.io>
 * @license MIT
 *
 * @example
 * ```typescript
 * // OpenAI embeddings
 * const openai = new OpenAIEmbeddings({ apiKey: 'sk-...' });
 * const embeddings = await openai.embedTexts(['Hello world', 'Test']);
 *
 * // Auto-insert into VectorDB
 * await embedAndInsert(db, openai, [
 *   { id: '1', text: 'Hello world', metadata: { source: 'test' } }
 * ]);
 * ```
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
exports.HuggingFaceEmbeddings = exports.AnthropicEmbeddings = exports.CohereEmbeddings = exports.OpenAIEmbeddings = exports.EmbeddingProvider = void 0;
exports.embedAndInsert = embedAndInsert;
exports.embedAndSearch = embedAndSearch;
// ============================================================================
// Abstract Base Class
// ============================================================================
/**
 * Abstract base class for embedding providers
 * All embedding providers must extend this class and implement its methods
 */
class EmbeddingProvider {
    /**
     * Creates a new embedding provider instance
     * @param retryConfig - Configuration for retry logic
     */
    constructor(retryConfig) {
        this.retryConfig = {
            maxRetries: 3,
            initialDelay: 1000,
            maxDelay: 10000,
            backoffMultiplier: 2,
            ...retryConfig,
        };
    }
    /**
     * Embed a single text string
     * @param text - Text to embed
     * @returns Promise resolving to the embedding vector
     */
    async embedText(text) {
        const result = await this.embedTexts([text]);
        return result.embeddings[0].embedding;
    }
    /**
     * Execute a function with retry logic
     * @param fn - Function to execute
     * @param context - Context description for error messages
     * @returns Promise resolving to the function result
     */
    async withRetry(fn, context) {
        let lastError;
        let delay = this.retryConfig.initialDelay;
        for (let attempt = 0; attempt <= this.retryConfig.maxRetries; attempt++) {
            try {
                return await fn();
            }
            catch (error) {
                lastError = error;
                // Check if error is retryable
                if (!this.isRetryableError(error)) {
                    throw this.createEmbeddingError(error, context, false);
                }
                if (attempt < this.retryConfig.maxRetries) {
                    await this.sleep(delay);
                    delay = Math.min(delay * this.retryConfig.backoffMultiplier, this.retryConfig.maxDelay);
                }
            }
        }
        throw this.createEmbeddingError(lastError, `${context} (after ${this.retryConfig.maxRetries} retries)`, false);
    }
    /**
     * Determine if an error is retryable
     * @param error - Error to check
     * @returns True if the error should trigger a retry
     */
    isRetryableError(error) {
        if (error instanceof Error) {
            const message = error.message.toLowerCase();
            // Rate limits, timeouts, and temporary server errors are retryable
            return (message.includes('rate limit') ||
                message.includes('timeout') ||
                message.includes('503') ||
                message.includes('429') ||
                message.includes('connection'));
        }
        return false;
    }
    /**
     * Create a standardized embedding error
     * @param error - Original error
     * @param context - Context description
     * @param retryable - Whether the error is retryable
     * @returns Formatted error object
     */
    createEmbeddingError(error, context, retryable) {
        const message = error instanceof Error ? error.message : String(error);
        return {
            message: `${context}: ${message}`,
            error,
            retryable,
        };
    }
    /**
     * Sleep for a specified duration
     * @param ms - Milliseconds to sleep
     */
    sleep(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }
    /**
     * Split texts into batches based on max batch size
     * @param texts - Texts to batch
     * @returns Array of text batches
     */
    createBatches(texts) {
        const batches = [];
        const batchSize = this.getMaxBatchSize();
        for (let i = 0; i < texts.length; i += batchSize) {
            batches.push(texts.slice(i, i + batchSize));
        }
        return batches;
    }
}
exports.EmbeddingProvider = EmbeddingProvider;
/**
 * OpenAI embeddings provider
 * Supports text-embedding-3-small, text-embedding-3-large, and text-embedding-ada-002
 */
class OpenAIEmbeddings extends EmbeddingProvider {
    /**
     * Creates a new OpenAI embeddings provider
     * @param config - Configuration options
     * @throws Error if OpenAI SDK is not installed
     */
    constructor(config) {
        super(config.retryConfig);
        this.config = {
            apiKey: config.apiKey,
            model: config.model || 'text-embedding-3-small',
            organization: config.organization,
            baseURL: config.baseURL,
            dimensions: config.dimensions,
        };
        try {
            // Dynamic import to support optional peer dependency
            const OpenAI = require('openai');
            this.openai = new OpenAI({
                apiKey: this.config.apiKey,
                organization: this.config.organization,
                baseURL: this.config.baseURL,
            });
        }
        catch (error) {
            throw new Error('OpenAI SDK not found. Install it with: npm install openai');
        }
    }
    getMaxBatchSize() {
        // OpenAI supports up to 2048 inputs per request
        return 2048;
    }
    getDimension() {
        // Return configured dimensions or default based on model
        if (this.config.dimensions) {
            return this.config.dimensions;
        }
        switch (this.config.model) {
            case 'text-embedding-3-small':
                return 1536;
            case 'text-embedding-3-large':
                return 3072;
            case 'text-embedding-ada-002':
                return 1536;
            default:
                return 1536;
        }
    }
    async embedTexts(texts) {
        if (texts.length === 0) {
            return { embeddings: [] };
        }
        const batches = this.createBatches(texts);
        const allResults = [];
        let totalTokens = 0;
        for (let batchIndex = 0; batchIndex < batches.length; batchIndex++) {
            const batch = batches[batchIndex];
            const baseIndex = batchIndex * this.getMaxBatchSize();
            const response = await this.withRetry(async () => {
                const params = {
                    model: this.config.model,
                    input: batch,
                };
                if (this.config.dimensions) {
                    params.dimensions = this.config.dimensions;
                }
                return await this.openai.embeddings.create(params);
            }, `OpenAI embeddings for batch ${batchIndex + 1}/${batches.length}`);
            totalTokens += response.usage?.total_tokens || 0;
            for (const item of response.data) {
                allResults.push({
                    embedding: item.embedding,
                    index: baseIndex + item.index,
                    tokens: response.usage?.total_tokens,
                });
            }
        }
        return {
            embeddings: allResults,
            totalTokens,
            metadata: {
                model: this.config.model,
                provider: 'openai',
            },
        };
    }
}
exports.OpenAIEmbeddings = OpenAIEmbeddings;
/**
 * Cohere embeddings provider
 * Supports embed-english-v3.0, embed-multilingual-v3.0, and other Cohere models
 */
class CohereEmbeddings extends EmbeddingProvider {
    /**
     * Creates a new Cohere embeddings provider
     * @param config - Configuration options
     * @throws Error if Cohere SDK is not installed
     */
    constructor(config) {
        super(config.retryConfig);
        this.config = {
            apiKey: config.apiKey,
            model: config.model || 'embed-english-v3.0',
            inputType: config.inputType,
            truncate: config.truncate,
        };
        try {
            // Dynamic import to support optional peer dependency
            const { CohereClient } = require('cohere-ai');
            this.cohere = new CohereClient({
                token: this.config.apiKey,
            });
        }
        catch (error) {
            throw new Error('Cohere SDK not found. Install it with: npm install cohere-ai');
        }
    }
    getMaxBatchSize() {
        // Cohere supports up to 96 texts per request
        return 96;
    }
    getDimension() {
        // Cohere v3 models produce 1024-dimensional embeddings
        if (this.config.model.includes('v3')) {
            return 1024;
        }
        // Earlier models use different dimensions
        return 4096;
    }
    async embedTexts(texts) {
        if (texts.length === 0) {
            return { embeddings: [] };
        }
        const batches = this.createBatches(texts);
        const allResults = [];
        for (let batchIndex = 0; batchIndex < batches.length; batchIndex++) {
            const batch = batches[batchIndex];
            const baseIndex = batchIndex * this.getMaxBatchSize();
            const response = await this.withRetry(async () => {
                const params = {
                    model: this.config.model,
                    texts: batch,
                };
                if (this.config.inputType) {
                    params.inputType = this.config.inputType;
                }
                if (this.config.truncate) {
                    params.truncate = this.config.truncate;
                }
                return await this.cohere.embed(params);
            }, `Cohere embeddings for batch ${batchIndex + 1}/${batches.length}`);
            for (let i = 0; i < response.embeddings.length; i++) {
                allResults.push({
                    embedding: response.embeddings[i],
                    index: baseIndex + i,
                });
            }
        }
        return {
            embeddings: allResults,
            metadata: {
                model: this.config.model,
                provider: 'cohere',
            },
        };
    }
}
exports.CohereEmbeddings = CohereEmbeddings;
/**
 * Anthropic embeddings provider using Voyage AI
 * Anthropic partners with Voyage AI for embeddings
 */
class AnthropicEmbeddings extends EmbeddingProvider {
    /**
     * Creates a new Anthropic embeddings provider
     * @param config - Configuration options
     * @throws Error if Anthropic SDK is not installed
     */
    constructor(config) {
        super(config.retryConfig);
        this.config = {
            apiKey: config.apiKey,
            model: config.model || 'voyage-2',
            inputType: config.inputType,
        };
        try {
            const Anthropic = require('@anthropic-ai/sdk');
            this.anthropic = new Anthropic({
                apiKey: this.config.apiKey,
            });
        }
        catch (error) {
            throw new Error('Anthropic SDK not found. Install it with: npm install @anthropic-ai/sdk');
        }
    }
    getMaxBatchSize() {
        // Process in smaller batches for Voyage API
        return 128;
    }
    getDimension() {
        // Voyage-2 produces 1024-dimensional embeddings
        return 1024;
    }
    async embedTexts(texts) {
        if (texts.length === 0) {
            return { embeddings: [] };
        }
        const batches = this.createBatches(texts);
        const allResults = [];
        for (let batchIndex = 0; batchIndex < batches.length; batchIndex++) {
            const batch = batches[batchIndex];
            const baseIndex = batchIndex * this.getMaxBatchSize();
            // Note: As of early 2025, Anthropic uses Voyage AI for embeddings
            // This is a placeholder for when official API is available
            const response = await this.withRetry(async () => {
                // Use Voyage AI API through Anthropic's recommended integration
                const httpResponse = await fetch('https://api.voyageai.com/v1/embeddings', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                        'Authorization': `Bearer ${this.config.apiKey}`,
                    },
                    body: JSON.stringify({
                        input: batch,
                        model: this.config.model,
                        input_type: this.config.inputType || 'document',
                    }),
                });
                if (!httpResponse.ok) {
                    const error = await httpResponse.text();
                    throw new Error(`Voyage API error: ${error}`);
                }
                return await httpResponse.json();
            }, `Anthropic/Voyage embeddings for batch ${batchIndex + 1}/${batches.length}`);
            for (let i = 0; i < response.data.length; i++) {
                allResults.push({
                    embedding: response.data[i].embedding,
                    index: baseIndex + i,
                });
            }
        }
        return {
            embeddings: allResults,
            metadata: {
                model: this.config.model,
                provider: 'anthropic-voyage',
            },
        };
    }
}
exports.AnthropicEmbeddings = AnthropicEmbeddings;
/**
 * HuggingFace local embeddings provider
 * Runs embedding models locally using transformers.js
 */
class HuggingFaceEmbeddings extends EmbeddingProvider {
    /**
     * Creates a new HuggingFace local embeddings provider
     * @param config - Configuration options
     */
    constructor(config = {}) {
        super(config.retryConfig);
        this.initialized = false;
        this.config = {
            model: config.model || 'Xenova/all-MiniLM-L6-v2',
            normalize: config.normalize !== false,
            batchSize: config.batchSize || 32,
        };
    }
    getMaxBatchSize() {
        return this.config.batchSize;
    }
    getDimension() {
        // all-MiniLM-L6-v2 produces 384-dimensional embeddings
        // This should be determined dynamically based on model
        return 384;
    }
    /**
     * Initialize the embedding pipeline
     */
    async initialize() {
        if (this.initialized)
            return;
        try {
            // Dynamic import of transformers.js
            const { pipeline } = await Promise.resolve().then(() => __importStar(require('@xenova/transformers')));
            this.pipeline = await pipeline('feature-extraction', this.config.model);
            this.initialized = true;
        }
        catch (error) {
            throw new Error('Transformers.js not found or failed to load. Install it with: npm install @xenova/transformers');
        }
    }
    async embedTexts(texts) {
        if (texts.length === 0) {
            return { embeddings: [] };
        }
        await this.initialize();
        const batches = this.createBatches(texts);
        const allResults = [];
        for (let batchIndex = 0; batchIndex < batches.length; batchIndex++) {
            const batch = batches[batchIndex];
            const baseIndex = batchIndex * this.getMaxBatchSize();
            const embeddings = await this.withRetry(async () => {
                const output = await this.pipeline(batch, {
                    pooling: 'mean',
                    normalize: this.config.normalize,
                });
                // Convert tensor to array
                return output.tolist();
            }, `HuggingFace embeddings for batch ${batchIndex + 1}/${batches.length}`);
            for (let i = 0; i < embeddings.length; i++) {
                allResults.push({
                    embedding: embeddings[i],
                    index: baseIndex + i,
                });
            }
        }
        return {
            embeddings: allResults,
            metadata: {
                model: this.config.model,
                provider: 'huggingface-local',
            },
        };
    }
}
exports.HuggingFaceEmbeddings = HuggingFaceEmbeddings;
// ============================================================================
// Helper Functions
// ============================================================================
/**
 * Embed texts and automatically insert them into a VectorDB
 *
 * @param db - VectorDB instance to insert into
 * @param provider - Embedding provider to use
 * @param documents - Documents to embed and insert
 * @param options - Additional options
 * @returns Promise resolving to array of inserted vector IDs
 *
 * @example
 * ```typescript
 * const openai = new OpenAIEmbeddings({ apiKey: 'sk-...' });
 * const db = new VectorDB({ dimension: 1536 });
 *
 * const ids = await embedAndInsert(db, openai, [
 *   { id: '1', text: 'Hello world', metadata: { source: 'test' } },
 *   { id: '2', text: 'Another document', metadata: { source: 'test' } }
 * ]);
 *
 * console.log('Inserted vector IDs:', ids);
 * ```
 */
async function embedAndInsert(db, provider, documents, options = {}) {
    if (documents.length === 0) {
        return [];
    }
    // Verify dimension compatibility
    const dbDimension = db.dimension || db.getDimension?.();
    const providerDimension = provider.getDimension();
    if (dbDimension && dbDimension !== providerDimension) {
        throw new Error(`Dimension mismatch: VectorDB expects ${dbDimension} but provider produces ${providerDimension}`);
    }
    // Extract texts
    const texts = documents.map(doc => doc.text);
    // Generate embeddings
    const result = await provider.embedTexts(texts);
    // Insert vectors
    const insertedIds = [];
    for (let i = 0; i < documents.length; i++) {
        const doc = documents[i];
        const embedding = result.embeddings.find(e => e.index === i);
        if (!embedding) {
            throw new Error(`Missing embedding for document at index ${i}`);
        }
        // Insert or update vector
        if (options.overwrite) {
            await db.upsert({
                id: doc.id,
                values: embedding.embedding,
                metadata: doc.metadata,
            });
        }
        else {
            await db.insert({
                id: doc.id,
                values: embedding.embedding,
                metadata: doc.metadata,
            });
        }
        insertedIds.push(doc.id);
        // Call progress callback
        if (options.onProgress) {
            options.onProgress(i + 1, documents.length);
        }
    }
    return insertedIds;
}
/**
 * Embed a query and search for similar documents in VectorDB
 *
 * @param db - VectorDB instance to search
 * @param provider - Embedding provider to use
 * @param query - Query text to search for
 * @param options - Search options
 * @returns Promise resolving to search results
 *
 * @example
 * ```typescript
 * const openai = new OpenAIEmbeddings({ apiKey: 'sk-...' });
 * const db = new VectorDB({ dimension: 1536 });
 *
 * const results = await embedAndSearch(db, openai, 'machine learning', {
 *   topK: 5,
 *   threshold: 0.7
 * });
 *
 * console.log('Found documents:', results);
 * ```
 */
async function embedAndSearch(db, provider, query, options = {}) {
    // Generate query embedding
    const queryEmbedding = await provider.embedText(query);
    // Search VectorDB
    const results = await db.search({
        vector: queryEmbedding,
        topK: options.topK || 10,
        threshold: options.threshold,
        filter: options.filter,
    });
    return results;
}
// ============================================================================
// Exports
// ============================================================================
exports.default = {
    // Base class
    EmbeddingProvider,
    // Providers
    OpenAIEmbeddings,
    CohereEmbeddings,
    AnthropicEmbeddings,
    HuggingFaceEmbeddings,
    // Helper functions
    embedAndInsert,
    embedAndSearch,
};
//# sourceMappingURL=embeddings.js.map