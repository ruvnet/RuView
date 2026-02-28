/**
 * RuvLLM WASM Types
 * Types for browser-based LLM inference
 */

/** WebGPU availability status */
export enum WebGPUStatus {
  Available = 'available',
  Unavailable = 'unavailable',
  NotSupported = 'not_supported',
}

/** Model loading status */
export enum LoadingStatus {
  Idle = 'idle',
  Downloading = 'downloading',
  Loading = 'loading',
  Ready = 'ready',
  Error = 'error',
}

/** Supported model architectures */
export enum ModelArchitecture {
  Llama = 'llama',
  Mistral = 'mistral',
  Phi = 'phi',
  Qwen = 'qwen',
  Gemma = 'gemma',
  StableLM = 'stablelm',
}

/** Model metadata */
export interface ModelMetadata {
  /** Model name */
  name: string;
  /** Model architecture */
  architecture: ModelArchitecture;
  /** Number of parameters */
  parameters: string;
  /** Context length */
  contextLength: number;
  /** Vocabulary size */
  vocabSize: number;
  /** Embedding dimension */
  embeddingDim: number;
  /** Number of layers */
  numLayers: number;
  /** Quantization type */
  quantization: string;
  /** File size in bytes */
  fileSize: number;
}

/** WASM module configuration */
export interface WASMConfig {
  /** WebGPU device (optional) */
  device?: GPUDevice;
  /** Number of threads (SharedArrayBuffer required) */
  threads?: number;
  /** SIMD enabled */
  simd?: boolean;
  /** Memory limit in MB */
  memoryLimit?: number;
  /** Cache models in IndexedDB */
  cacheModels?: boolean;
}

/** Generation configuration */
export interface GenerationConfig {
  /** Maximum tokens to generate */
  maxTokens?: number;
  /** Temperature (0-2) */
  temperature?: number;
  /** Top-p sampling */
  topP?: number;
  /** Top-k sampling */
  topK?: number;
  /** Repetition penalty */
  repetitionPenalty?: number;
  /** Stop sequences */
  stopSequences?: string[];
  /** Stream tokens as generated */
  stream?: boolean;
}

/** Token callback for streaming */
export type TokenCallback = (token: string, done: boolean) => void;

/** Progress callback for model loading */
export type ProgressCallback = (loaded: number, total: number) => void;

/** Inference statistics */
export interface InferenceStats {
  /** Tokens generated */
  tokensGenerated: number;
  /** Time to first token (ms) */
  timeToFirstToken: number;
  /** Total time (ms) */
  totalTime: number;
  /** Tokens per second */
  tokensPerSecond: number;
  /** Prompt tokens */
  promptTokens: number;
  /** Memory used (MB) */
  memoryUsed: number;
}

/** Chat message */
export interface ChatMessage {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

/** Completion result */
export interface CompletionResult {
  /** Generated text */
  text: string;
  /** Inference statistics */
  stats: InferenceStats;
  /** Finish reason */
  finishReason: 'stop' | 'length' | 'error';
}

/** Model download progress */
export interface DownloadProgress {
  /** Bytes downloaded */
  loaded: number;
  /** Total bytes */
  total: number;
  /** Download speed (bytes/sec) */
  speed: number;
  /** Estimated time remaining (seconds) */
  eta: number;
}
