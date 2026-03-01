/**
 * RuvLTRA Model Registry and Downloader
 *
 * Automatically downloads GGUF models from HuggingFace Hub.
 *
 * @example
 * ```typescript
 * import { ModelDownloader, RUVLTRA_MODELS } from '@ruvector/ruvllm';
 *
 * // Download the Claude Code optimized model
 * const downloader = new ModelDownloader();
 * const modelPath = await downloader.download('claude-code');
 *
 * // Or download all models
 * await downloader.downloadAll();
 * ```
 */
/** Model information from HuggingFace */
export interface ModelInfo {
    /** Model identifier */
    id: string;
    /** Display name */
    name: string;
    /** Model filename on HuggingFace */
    filename: string;
    /** Model size in bytes */
    sizeBytes: number;
    /** Model size (human readable) */
    size: string;
    /** Parameter count */
    parameters: string;
    /** Use case description */
    useCase: string;
    /** Quantization type */
    quantization: string;
    /** Context window size */
    contextLength: number;
    /** HuggingFace download URL */
    url: string;
}
/** Download progress callback */
export type ProgressCallback = (progress: DownloadProgress) => void;
/** Download progress information */
export interface DownloadProgress {
    /** Model being downloaded */
    modelId: string;
    /** Bytes downloaded so far */
    downloaded: number;
    /** Total bytes to download */
    total: number;
    /** Download percentage (0-100) */
    percent: number;
    /** Download speed in bytes per second */
    speedBps: number;
    /** Estimated time remaining in seconds */
    etaSeconds: number;
}
/** Download options */
export interface DownloadOptions {
    /** Directory to save models (default: ~/.ruvllm/models) */
    modelsDir?: string;
    /** Force re-download even if file exists */
    force?: boolean;
    /** Progress callback */
    onProgress?: ProgressCallback;
    /** Verify file integrity after download */
    verify?: boolean;
}
/** Available RuvLTRA models */
export declare const RUVLTRA_MODELS: Record<string, ModelInfo>;
/** Model aliases for convenience */
export declare const MODEL_ALIASES: Record<string, string>;
/**
 * Get the default models directory
 */
export declare function getDefaultModelsDir(): string;
/**
 * Resolve model ID from alias or direct ID
 */
export declare function resolveModelId(modelIdOrAlias: string): string | null;
/**
 * Get model info by ID or alias
 */
export declare function getModelInfo(modelIdOrAlias: string): ModelInfo | null;
/**
 * List all available models
 */
export declare function listModels(): ModelInfo[];
/**
 * Model downloader for RuvLTRA GGUF models
 */
export declare class ModelDownloader {
    private modelsDir;
    constructor(modelsDir?: string);
    /**
     * Get the path where a model would be saved
     */
    getModelPath(modelIdOrAlias: string): string | null;
    /**
     * Check if a model is already downloaded
     */
    isDownloaded(modelIdOrAlias: string): boolean;
    /**
     * Get download status for all models
     */
    getStatus(): {
        model: ModelInfo;
        downloaded: boolean;
        path: string;
    }[];
    /**
     * Download a model from HuggingFace
     */
    download(modelIdOrAlias: string, options?: DownloadOptions): Promise<string>;
    /**
     * Download all available models
     */
    downloadAll(options?: DownloadOptions): Promise<string[]>;
    /**
     * Delete a downloaded model
     */
    delete(modelIdOrAlias: string): boolean;
    /**
     * Delete all downloaded models
     */
    deleteAll(): number;
}
export default ModelDownloader;
//# sourceMappingURL=models.d.ts.map