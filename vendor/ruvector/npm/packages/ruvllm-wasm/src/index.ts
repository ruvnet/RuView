/**
 * @ruvector/ruvllm-wasm - Browser LLM Inference with WebAssembly
 *
 * Run large language models directly in the browser using WebAssembly
 * with optional WebGPU acceleration for faster inference.
 *
 * @example
 * ```typescript
 * import { RuvLLMWasm } from '@ruvector/ruvllm-wasm';
 *
 * // Initialize with WebGPU (if available)
 * const llm = await RuvLLMWasm.create({ useWebGPU: true });
 *
 * // Load a model
 * await llm.loadModel('https://example.com/model.gguf', {
 *   onProgress: (loaded, total) => console.log(`${loaded}/${total}`)
 * });
 *
 * // Generate text
 * const result = await llm.generate('Hello, world!', {
 *   maxTokens: 100,
 *   temperature: 0.7,
 * });
 *
 * console.log(result.text);
 * ```
 *
 * @packageDocumentation
 */

export {
  WebGPUStatus,
  LoadingStatus,
  ModelArchitecture,
  ModelMetadata,
  WASMConfig,
  GenerationConfig,
  TokenCallback,
  ProgressCallback,
  InferenceStats,
  ChatMessage,
  CompletionResult,
  DownloadProgress,
} from './types.js';

/** Package version */
export const VERSION = '0.1.0';

/**
 * Check WebGPU availability
 */
export async function checkWebGPU(): Promise<import('./types.js').WebGPUStatus> {
  if (typeof navigator === 'undefined') {
    return 'not_supported' as import('./types.js').WebGPUStatus;
  }

  if (!('gpu' in navigator)) {
    return 'not_supported' as import('./types.js').WebGPUStatus;
  }

  try {
    const adapter = await (navigator as any).gpu.requestAdapter();
    if (adapter) {
      return 'available' as import('./types.js').WebGPUStatus;
    }
    return 'unavailable' as import('./types.js').WebGPUStatus;
  } catch {
    return 'unavailable' as import('./types.js').WebGPUStatus;
  }
}

/**
 * Check SharedArrayBuffer support (required for threading)
 */
export function checkSharedArrayBuffer(): boolean {
  return typeof SharedArrayBuffer !== 'undefined';
}

/**
 * Check SIMD support
 */
export async function checkSIMD(): Promise<boolean> {
  try {
    // Check for WASM SIMD support
    const simdTest = new Uint8Array([
      0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
      0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7b, 0x03,
      0x02, 0x01, 0x00, 0x0a, 0x0a, 0x01, 0x08, 0x00,
      0x41, 0x00, 0xfd, 0x0f, 0x00, 0x0b,
    ]);
    await WebAssembly.compile(simdTest);
    return true;
  } catch {
    return false;
  }
}

/**
 * Get browser capabilities for LLM inference
 */
export async function getCapabilities(): Promise<{
  webgpu: import('./types.js').WebGPUStatus;
  sharedArrayBuffer: boolean;
  simd: boolean;
  crossOriginIsolated: boolean;
}> {
  const [webgpu, simd] = await Promise.all([
    checkWebGPU(),
    checkSIMD(),
  ]);

  return {
    webgpu,
    sharedArrayBuffer: checkSharedArrayBuffer(),
    simd,
    crossOriginIsolated: typeof crossOriginIsolated !== 'undefined' && crossOriginIsolated,
  };
}

/**
 * Format file size for display
 */
export function formatFileSize(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB'];
  let size = bytes;
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }

  return `${size.toFixed(1)} ${units[unitIndex]}`;
}

/**
 * Estimate memory requirements for a model
 */
export function estimateMemory(fileSizeBytes: number): {
  minimum: number;
  recommended: number;
} {
  // Rough estimates based on model size
  const fileSizeMB = fileSizeBytes / (1024 * 1024);

  return {
    minimum: Math.ceil(fileSizeMB * 1.2), // 20% overhead
    recommended: Math.ceil(fileSizeMB * 1.5), // 50% overhead for KV cache
  };
}

/**
 * RuvLLM WASM class placeholder
 * Full implementation requires WASM binary from ruvllm-wasm crate
 */
export class RuvLLMWasm {
  private config: import('./types.js').WASMConfig;
  private status: import('./types.js').LoadingStatus = 'idle' as import('./types.js').LoadingStatus;

  private constructor(config: import('./types.js').WASMConfig) {
    this.config = config;
  }

  /**
   * Create a new RuvLLMWasm instance
   */
  static async create(options?: {
    useWebGPU?: boolean;
    threads?: number;
    memoryLimit?: number;
  }): Promise<RuvLLMWasm> {
    const config: import('./types.js').WASMConfig = {
      threads: options?.threads,
      memoryLimit: options?.memoryLimit,
      simd: await checkSIMD(),
      cacheModels: true,
    };

    if (options?.useWebGPU) {
      const webgpuStatus = await checkWebGPU();
      if (webgpuStatus === 'available') {
        const adapter = await (navigator as any).gpu.requestAdapter();
        if (adapter) {
          config.device = await adapter.requestDevice();
        }
      }
    }

    return new RuvLLMWasm(config);
  }

  /**
   * Get current loading status
   */
  getStatus(): import('./types.js').LoadingStatus {
    return this.status;
  }

  /**
   * Load a model from URL or ArrayBuffer
   */
  async loadModel(
    source: string | ArrayBuffer,
    options?: {
      onProgress?: import('./types.js').ProgressCallback;
    }
  ): Promise<import('./types.js').ModelMetadata> {
    this.status = 'loading' as import('./types.js').LoadingStatus;

    // Placeholder - actual implementation requires WASM binary
    console.log('Loading model from:', typeof source === 'string' ? source : 'ArrayBuffer');
    console.log('Note: Full model loading requires the ruvllm-wasm binary.');
    console.log('Build from: crates/ruvllm-wasm');

    this.status = 'ready' as import('./types.js').LoadingStatus;

    return {
      name: 'placeholder',
      architecture: 'llama' as import('./types.js').ModelArchitecture,
      parameters: '0B',
      contextLength: 2048,
      vocabSize: 32000,
      embeddingDim: 2048,
      numLayers: 22,
      quantization: 'q4_k_m',
      fileSize: 0,
    };
  }

  /**
   * Generate text completion
   */
  async generate(
    prompt: string,
    config?: import('./types.js').GenerationConfig,
    onToken?: import('./types.js').TokenCallback
  ): Promise<import('./types.js').CompletionResult> {
    console.log('Generating with prompt:', prompt.substring(0, 50) + '...');
    console.log('Note: Full generation requires the ruvllm-wasm binary.');

    return {
      text: '[Placeholder - build ruvllm-wasm crate for actual inference]',
      stats: {
        tokensGenerated: 0,
        timeToFirstToken: 0,
        totalTime: 0,
        tokensPerSecond: 0,
        promptTokens: 0,
        memoryUsed: 0,
      },
      finishReason: 'stop',
    };
  }

  /**
   * Chat completion with message history
   */
  async chat(
    messages: import('./types.js').ChatMessage[],
    config?: import('./types.js').GenerationConfig,
    onToken?: import('./types.js').TokenCallback
  ): Promise<import('./types.js').CompletionResult> {
    const prompt = messages
      .map(m => `${m.role}: ${m.content}`)
      .join('\n');

    return this.generate(prompt, config, onToken);
  }

  /**
   * Unload model and free memory
   */
  unload(): void {
    this.status = 'idle' as import('./types.js').LoadingStatus;
  }
}
