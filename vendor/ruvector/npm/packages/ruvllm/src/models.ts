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

import { createWriteStream, existsSync, mkdirSync, statSync, unlinkSync, renameSync } from 'fs';
import { join, dirname } from 'path';
import { homedir } from 'os';
import { pipeline } from 'stream/promises';
import { createHash } from 'crypto';

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

/** HuggingFace repository */
const HF_REPO = 'ruv/ruvltra';
const HF_BASE_URL = `https://huggingface.co/${HF_REPO}/resolve/main`;

/** Available RuvLTRA models */
export const RUVLTRA_MODELS: Record<string, ModelInfo> = {
  'claude-code': {
    id: 'claude-code',
    name: 'RuvLTRA Claude Code',
    filename: 'ruvltra-claude-code-0.5b-q4_k_m.gguf',
    sizeBytes: 398_000_000,
    size: '398 MB',
    parameters: '0.5B',
    useCase: 'Claude Code workflows, agentic coding',
    quantization: 'Q4_K_M',
    contextLength: 4096,
    url: `${HF_BASE_URL}/ruvltra-claude-code-0.5b-q4_k_m.gguf`,
  },
  'small': {
    id: 'small',
    name: 'RuvLTRA Small',
    filename: 'ruvltra-small-0.5b-q4_k_m.gguf',
    sizeBytes: 398_000_000,
    size: '398 MB',
    parameters: '0.5B',
    useCase: 'Edge devices, IoT, resource-constrained environments',
    quantization: 'Q4_K_M',
    contextLength: 4096,
    url: `${HF_BASE_URL}/ruvltra-small-0.5b-q4_k_m.gguf`,
  },
  'medium': {
    id: 'medium',
    name: 'RuvLTRA Medium',
    filename: 'ruvltra-medium-1.1b-q4_k_m.gguf',
    sizeBytes: 669_000_000,
    size: '669 MB',
    parameters: '1.1B',
    useCase: 'General purpose, balanced performance',
    quantization: 'Q4_K_M',
    contextLength: 8192,
    url: `${HF_BASE_URL}/ruvltra-medium-1.1b-q4_k_m.gguf`,
  },
};

/** Model aliases for convenience */
export const MODEL_ALIASES: Record<string, string> = {
  'cc': 'claude-code',
  'claudecode': 'claude-code',
  'claude': 'claude-code',
  's': 'small',
  'sm': 'small',
  'm': 'medium',
  'med': 'medium',
  'default': 'claude-code',
};

/**
 * Get the default models directory
 */
export function getDefaultModelsDir(): string {
  return join(homedir(), '.ruvllm', 'models');
}

/**
 * Resolve model ID from alias or direct ID
 */
export function resolveModelId(modelIdOrAlias: string): string | null {
  const normalized = modelIdOrAlias.toLowerCase().trim();

  // Direct match
  if (RUVLTRA_MODELS[normalized]) {
    return normalized;
  }

  // Alias match
  if (MODEL_ALIASES[normalized]) {
    return MODEL_ALIASES[normalized];
  }

  return null;
}

/**
 * Get model info by ID or alias
 */
export function getModelInfo(modelIdOrAlias: string): ModelInfo | null {
  const id = resolveModelId(modelIdOrAlias);
  return id ? RUVLTRA_MODELS[id] : null;
}

/**
 * List all available models
 */
export function listModels(): ModelInfo[] {
  return Object.values(RUVLTRA_MODELS);
}

/**
 * Model downloader for RuvLTRA GGUF models
 */
export class ModelDownloader {
  private modelsDir: string;

  constructor(modelsDir?: string) {
    this.modelsDir = modelsDir || getDefaultModelsDir();
  }

  /**
   * Get the path where a model would be saved
   */
  getModelPath(modelIdOrAlias: string): string | null {
    const model = getModelInfo(modelIdOrAlias);
    if (!model) return null;
    return join(this.modelsDir, model.filename);
  }

  /**
   * Check if a model is already downloaded
   */
  isDownloaded(modelIdOrAlias: string): boolean {
    const path = this.getModelPath(modelIdOrAlias);
    if (!path) return false;

    if (!existsSync(path)) return false;

    // Verify size matches expected
    const model = getModelInfo(modelIdOrAlias);
    if (!model) return false;

    const stats = statSync(path);
    // Allow 5% variance for size check
    const minSize = model.sizeBytes * 0.95;
    return stats.size >= minSize;
  }

  /**
   * Get download status for all models
   */
  getStatus(): { model: ModelInfo; downloaded: boolean; path: string }[] {
    return listModels().map(model => ({
      model,
      downloaded: this.isDownloaded(model.id),
      path: this.getModelPath(model.id)!,
    }));
  }

  /**
   * Download a model from HuggingFace
   */
  async download(
    modelIdOrAlias: string,
    options: DownloadOptions = {}
  ): Promise<string> {
    const model = getModelInfo(modelIdOrAlias);
    if (!model) {
      const available = listModels().map(m => m.id).join(', ');
      throw new Error(
        `Unknown model: ${modelIdOrAlias}. Available models: ${available}`
      );
    }

    const destDir = options.modelsDir || this.modelsDir;
    const destPath = join(destDir, model.filename);

    // Check if already downloaded
    if (!options.force && this.isDownloaded(model.id)) {
      return destPath;
    }

    // Ensure directory exists
    if (!existsSync(destDir)) {
      mkdirSync(destDir, { recursive: true });
    }

    // Download with progress tracking
    const tempPath = `${destPath}.tmp`;
    let startTime = Date.now();
    let lastProgressTime = startTime;
    let lastDownloaded = 0;

    try {
      // Use dynamic import for node-fetch if native fetch not available
      const fetchFn = globalThis.fetch || (await import('node:https')).default;

      const response = await fetch(model.url, {
        headers: {
          'User-Agent': 'RuvLLM/2.3.0',
        },
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const contentLength = parseInt(
        response.headers.get('content-length') || String(model.sizeBytes)
      );

      // Create write stream
      const fileStream = createWriteStream(tempPath);
      let downloaded = 0;

      // Stream with progress
      const reader = response.body?.getReader();
      if (!reader) {
        throw new Error('Response body is not readable');
      }

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        downloaded += value.length;
        fileStream.write(value);

        // Report progress
        if (options.onProgress) {
          const now = Date.now();
          const elapsed = (now - lastProgressTime) / 1000;
          const bytesThisInterval = downloaded - lastDownloaded;
          const speedBps = elapsed > 0 ? bytesThisInterval / elapsed : 0;
          const remaining = contentLength - downloaded;
          const etaSeconds = speedBps > 0 ? remaining / speedBps : 0;

          options.onProgress({
            modelId: model.id,
            downloaded,
            total: contentLength,
            percent: Math.round((downloaded / contentLength) * 100),
            speedBps,
            etaSeconds,
          });

          lastProgressTime = now;
          lastDownloaded = downloaded;
        }
      }

      fileStream.end();

      // Wait for file to be fully written
      await new Promise<void>((resolve, reject) => {
        fileStream.on('finish', resolve);
        fileStream.on('error', reject);
      });

      // Move temp file to final destination
      if (existsSync(destPath)) {
        unlinkSync(destPath);
      }
      renameSync(tempPath, destPath);

      return destPath;
    } catch (error) {
      // Clean up temp file on error
      if (existsSync(tempPath)) {
        try { unlinkSync(tempPath); } catch {}
      }
      throw error;
    }
  }

  /**
   * Download all available models
   */
  async downloadAll(options: DownloadOptions = {}): Promise<string[]> {
    const paths: string[] = [];
    for (const model of listModels()) {
      const path = await this.download(model.id, options);
      paths.push(path);
    }
    return paths;
  }

  /**
   * Delete a downloaded model
   */
  delete(modelIdOrAlias: string): boolean {
    const path = this.getModelPath(modelIdOrAlias);
    if (!path || !existsSync(path)) {
      return false;
    }
    unlinkSync(path);
    return true;
  }

  /**
   * Delete all downloaded models
   */
  deleteAll(): number {
    let count = 0;
    for (const model of listModels()) {
      if (this.delete(model.id)) {
        count++;
      }
    }
    return count;
  }
}

export default ModelDownloader;
