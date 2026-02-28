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
type VectorDB = any;
/**
 * Configuration for retry logic
 */
export interface RetryConfig {
    /** Maximum number of retry attempts */
    maxRetries: number;
    /** Initial delay in milliseconds before first retry */
    initialDelay: number;
    /** Maximum delay in milliseconds between retries */
    maxDelay: number;
    /** Multiplier for exponential backoff */
    backoffMultiplier: number;
}
/**
 * Result of an embedding operation
 */
export interface EmbeddingResult {
    /** The generated embedding vector */
    embedding: number[];
    /** Index of the text in the original batch */
    index: number;
    /** Optional token count used */
    tokens?: number;
}
/**
 * Batch result with embeddings and metadata
 */
export interface BatchEmbeddingResult {
    /** Array of embedding results */
    embeddings: EmbeddingResult[];
    /** Total tokens used (if available) */
    totalTokens?: number;
    /** Provider-specific metadata */
    metadata?: Record<string, unknown>;
}
/**
 * Error details for failed embedding operations
 */
export interface EmbeddingError {
    /** Error message */
    message: string;
    /** Original error object */
    error: unknown;
    /** Index of the text that failed (if applicable) */
    index?: number;
    /** Whether the error is retryable */
    retryable: boolean;
}
/**
 * Document to embed and insert into VectorDB
 */
export interface DocumentToEmbed {
    /** Unique identifier for the document */
    id: string;
    /** Text content to embed */
    text: string;
    /** Optional metadata to store with the vector */
    metadata?: Record<string, unknown>;
}
/**
 * Abstract base class for embedding providers
 * All embedding providers must extend this class and implement its methods
 */
export declare abstract class EmbeddingProvider {
    protected retryConfig: RetryConfig;
    /**
     * Creates a new embedding provider instance
     * @param retryConfig - Configuration for retry logic
     */
    constructor(retryConfig?: Partial<RetryConfig>);
    /**
     * Get the maximum batch size supported by this provider
     */
    abstract getMaxBatchSize(): number;
    /**
     * Get the dimension of embeddings produced by this provider
     */
    abstract getDimension(): number;
    /**
     * Embed a single text string
     * @param text - Text to embed
     * @returns Promise resolving to the embedding vector
     */
    embedText(text: string): Promise<number[]>;
    /**
     * Embed multiple texts with automatic batching
     * @param texts - Array of texts to embed
     * @returns Promise resolving to batch embedding results
     */
    abstract embedTexts(texts: string[]): Promise<BatchEmbeddingResult>;
    /**
     * Execute a function with retry logic
     * @param fn - Function to execute
     * @param context - Context description for error messages
     * @returns Promise resolving to the function result
     */
    protected withRetry<T>(fn: () => Promise<T>, context: string): Promise<T>;
    /**
     * Determine if an error is retryable
     * @param error - Error to check
     * @returns True if the error should trigger a retry
     */
    protected isRetryableError(error: unknown): boolean;
    /**
     * Create a standardized embedding error
     * @param error - Original error
     * @param context - Context description
     * @param retryable - Whether the error is retryable
     * @returns Formatted error object
     */
    protected createEmbeddingError(error: unknown, context: string, retryable: boolean): EmbeddingError;
    /**
     * Sleep for a specified duration
     * @param ms - Milliseconds to sleep
     */
    protected sleep(ms: number): Promise<void>;
    /**
     * Split texts into batches based on max batch size
     * @param texts - Texts to batch
     * @returns Array of text batches
     */
    protected createBatches(texts: string[]): string[][];
}
/**
 * Configuration for OpenAI embeddings
 */
export interface OpenAIEmbeddingsConfig {
    /** OpenAI API key */
    apiKey: string;
    /** Model name (default: 'text-embedding-3-small') */
    model?: string;
    /** Embedding dimensions (only for text-embedding-3-* models) */
    dimensions?: number;
    /** Organization ID (optional) */
    organization?: string;
    /** Custom base URL (optional) */
    baseURL?: string;
    /** Retry configuration */
    retryConfig?: Partial<RetryConfig>;
}
/**
 * OpenAI embeddings provider
 * Supports text-embedding-3-small, text-embedding-3-large, and text-embedding-ada-002
 */
export declare class OpenAIEmbeddings extends EmbeddingProvider {
    private config;
    private openai;
    /**
     * Creates a new OpenAI embeddings provider
     * @param config - Configuration options
     * @throws Error if OpenAI SDK is not installed
     */
    constructor(config: OpenAIEmbeddingsConfig);
    getMaxBatchSize(): number;
    getDimension(): number;
    embedTexts(texts: string[]): Promise<BatchEmbeddingResult>;
}
/**
 * Configuration for Cohere embeddings
 */
export interface CohereEmbeddingsConfig {
    /** Cohere API key */
    apiKey: string;
    /** Model name (default: 'embed-english-v3.0') */
    model?: string;
    /** Input type: 'search_document', 'search_query', 'classification', or 'clustering' */
    inputType?: 'search_document' | 'search_query' | 'classification' | 'clustering';
    /** Truncate input text if it exceeds model limits */
    truncate?: 'NONE' | 'START' | 'END';
    /** Retry configuration */
    retryConfig?: Partial<RetryConfig>;
}
/**
 * Cohere embeddings provider
 * Supports embed-english-v3.0, embed-multilingual-v3.0, and other Cohere models
 */
export declare class CohereEmbeddings extends EmbeddingProvider {
    private config;
    private cohere;
    /**
     * Creates a new Cohere embeddings provider
     * @param config - Configuration options
     * @throws Error if Cohere SDK is not installed
     */
    constructor(config: CohereEmbeddingsConfig);
    getMaxBatchSize(): number;
    getDimension(): number;
    embedTexts(texts: string[]): Promise<BatchEmbeddingResult>;
}
/**
 * Configuration for Anthropic embeddings via Voyage AI
 */
export interface AnthropicEmbeddingsConfig {
    /** Anthropic API key */
    apiKey: string;
    /** Model name (default: 'voyage-2') */
    model?: string;
    /** Input type for embeddings */
    inputType?: 'document' | 'query';
    /** Retry configuration */
    retryConfig?: Partial<RetryConfig>;
}
/**
 * Anthropic embeddings provider using Voyage AI
 * Anthropic partners with Voyage AI for embeddings
 */
export declare class AnthropicEmbeddings extends EmbeddingProvider {
    private config;
    private anthropic;
    /**
     * Creates a new Anthropic embeddings provider
     * @param config - Configuration options
     * @throws Error if Anthropic SDK is not installed
     */
    constructor(config: AnthropicEmbeddingsConfig);
    getMaxBatchSize(): number;
    getDimension(): number;
    embedTexts(texts: string[]): Promise<BatchEmbeddingResult>;
}
/**
 * Configuration for HuggingFace local embeddings
 */
export interface HuggingFaceEmbeddingsConfig {
    /** Model name or path (default: 'sentence-transformers/all-MiniLM-L6-v2') */
    model?: string;
    /** Device to run on: 'cpu' or 'cuda' */
    device?: 'cpu' | 'cuda';
    /** Normalize embeddings to unit length */
    normalize?: boolean;
    /** Batch size for processing */
    batchSize?: number;
    /** Retry configuration */
    retryConfig?: Partial<RetryConfig>;
}
/**
 * HuggingFace local embeddings provider
 * Runs embedding models locally using transformers.js
 */
export declare class HuggingFaceEmbeddings extends EmbeddingProvider {
    private config;
    private pipeline;
    private initialized;
    /**
     * Creates a new HuggingFace local embeddings provider
     * @param config - Configuration options
     */
    constructor(config?: HuggingFaceEmbeddingsConfig);
    getMaxBatchSize(): number;
    getDimension(): number;
    /**
     * Initialize the embedding pipeline
     */
    private initialize;
    embedTexts(texts: string[]): Promise<BatchEmbeddingResult>;
}
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
export declare function embedAndInsert(db: VectorDB, provider: EmbeddingProvider, documents: DocumentToEmbed[], options?: {
    /** Whether to overwrite existing vectors with same ID */
    overwrite?: boolean;
    /** Progress callback */
    onProgress?: (current: number, total: number) => void;
}): Promise<string[]>;
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
export declare function embedAndSearch(db: VectorDB, provider: EmbeddingProvider, query: string, options?: {
    /** Number of results to return */
    topK?: number;
    /** Minimum similarity threshold (0-1) */
    threshold?: number;
    /** Metadata filter */
    filter?: Record<string, unknown>;
}): Promise<any[]>;
declare const _default: {
    EmbeddingProvider: typeof EmbeddingProvider;
    OpenAIEmbeddings: typeof OpenAIEmbeddings;
    CohereEmbeddings: typeof CohereEmbeddings;
    AnthropicEmbeddings: typeof AnthropicEmbeddings;
    HuggingFaceEmbeddings: typeof HuggingFaceEmbeddings;
    embedAndInsert: typeof embedAndInsert;
    embedAndSearch: typeof embedAndSearch;
};
export default _default;
//# sourceMappingURL=embeddings.d.ts.map