/**
 * ONNX WASM Embedder - Semantic embeddings for hooks
 *
 * Provides real transformer-based embeddings using all-MiniLM-L6-v2
 * running in pure WASM (no native dependencies).
 *
 * Uses bundled ONNX WASM files from src/core/onnx/
 *
 * Features:
 * - 384-dimensional semantic embeddings
 * - Real semantic understanding (not hash-based)
 * - Cached model loading (downloads from HuggingFace on first use)
 * - Batch embedding support
 * - Optional parallel workers for 3.8x batch speedup
 */
declare global {
    var __ruvector_require: NodeRequire | undefined;
}
export interface OnnxEmbedderConfig {
    modelId?: string;
    maxLength?: number;
    normalize?: boolean;
    cacheDir?: string;
    /**
     * Enable parallel workers for batch operations
     * - 'auto' (default): Enable for long-running processes, skip for CLI
     * - true: Always enable workers
     * - false: Never use workers
     */
    enableParallel?: boolean | 'auto';
    /** Number of worker threads (default: CPU cores - 1) */
    numWorkers?: number;
    /** Minimum batch size to use parallel processing (default: 4) */
    parallelThreshold?: number;
}
export interface EmbeddingResult {
    embedding: number[];
    dimension: number;
    timeMs: number;
}
export interface SimilarityResult {
    similarity: number;
    timeMs: number;
}
/**
 * Check if ONNX embedder is available (bundled files exist)
 */
export declare function isOnnxAvailable(): boolean;
/**
 * Initialize the ONNX embedder (downloads model if needed)
 */
export declare function initOnnxEmbedder(config?: OnnxEmbedderConfig): Promise<boolean>;
/**
 * Generate embedding for text
 */
export declare function embed(text: string): Promise<EmbeddingResult>;
/**
 * Generate embeddings for multiple texts
 * Uses parallel workers automatically for batches >= parallelThreshold
 */
export declare function embedBatch(texts: string[]): Promise<EmbeddingResult[]>;
/**
 * Calculate cosine similarity between two texts
 */
export declare function similarity(text1: string, text2: string): Promise<SimilarityResult>;
/**
 * Calculate cosine similarity between two embeddings
 */
export declare function cosineSimilarity(a: number[], b: number[]): number;
/**
 * Get embedding dimension
 */
export declare function getDimension(): number;
/**
 * Check if embedder is ready
 */
export declare function isReady(): boolean;
/**
 * Get embedder stats including SIMD and parallel capabilities
 */
export declare function getStats(): {
    ready: boolean;
    dimension: number;
    model: string;
    simd: boolean;
    parallel: boolean;
    parallelWorkers: number;
    parallelThreshold: number;
};
/**
 * Shutdown parallel workers (call on exit)
 */
export declare function shutdown(): Promise<void>;
export declare class OnnxEmbedder {
    private config;
    constructor(config?: OnnxEmbedderConfig);
    init(): Promise<boolean>;
    embed(text: string): Promise<number[]>;
    embedBatch(texts: string[]): Promise<number[][]>;
    similarity(text1: string, text2: string): Promise<number>;
    get dimension(): number;
    get ready(): boolean;
}
export default OnnxEmbedder;
//# sourceMappingURL=onnx-embedder.d.ts.map