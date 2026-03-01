/**
 * RuvLLM CLI Types
 * Types for CLI configuration and inference options
 */

/** Supported model formats */
export enum ModelFormat {
  GGUF = 'gguf',
  SafeTensors = 'safetensors',
  ONNX = 'onnx',
}

/** Hardware acceleration backends */
export enum AccelerationBackend {
  /** Apple Metal (macOS) */
  Metal = 'metal',
  /** NVIDIA CUDA */
  CUDA = 'cuda',
  /** CPU only */
  CPU = 'cpu',
  /** Apple Neural Engine */
  ANE = 'ane',
  /** Vulkan (cross-platform GPU) */
  Vulkan = 'vulkan',
}

/** Quantization levels */
export enum QuantizationType {
  F32 = 'f32',
  F16 = 'f16',
  Q8_0 = 'q8_0',
  Q4_K_M = 'q4_k_m',
  Q4_K_S = 'q4_k_s',
  Q5_K_M = 'q5_k_m',
  Q5_K_S = 'q5_k_s',
  Q6_K = 'q6_k',
  Q2_K = 'q2_k',
  Q3_K_M = 'q3_k_m',
}

/** Model configuration */
export interface ModelConfig {
  /** Path to model file */
  modelPath: string;
  /** Model format */
  format?: ModelFormat;
  /** Quantization type */
  quantization?: QuantizationType;
  /** Context window size */
  contextSize?: number;
  /** Number of GPU layers to offload */
  gpuLayers?: number;
  /** Batch size for inference */
  batchSize?: number;
  /** Number of threads for CPU inference */
  threads?: number;
}

/** Generation parameters */
export interface GenerationParams {
  /** Maximum tokens to generate */
  maxTokens?: number;
  /** Temperature for sampling */
  temperature?: number;
  /** Top-p (nucleus) sampling */
  topP?: number;
  /** Top-k sampling */
  topK?: number;
  /** Repetition penalty */
  repetitionPenalty?: number;
  /** Stop sequences */
  stopSequences?: string[];
  /** Seed for reproducibility */
  seed?: number;
}

/** Inference result */
export interface InferenceResult {
  /** Generated text */
  text: string;
  /** Number of tokens generated */
  tokensGenerated: number;
  /** Time to first token (ms) */
  timeToFirstToken: number;
  /** Total generation time (ms) */
  totalTime: number;
  /** Tokens per second */
  tokensPerSecond: number;
  /** Finish reason */
  finishReason: 'stop' | 'length' | 'error';
}

/** Benchmark result */
export interface BenchmarkResult {
  /** Model name */
  model: string;
  /** Backend used */
  backend: AccelerationBackend;
  /** Prompt tokens */
  promptTokens: number;
  /** Generated tokens */
  generatedTokens: number;
  /** Prompt processing time (ms) */
  promptTime: number;
  /** Generation time (ms) */
  generationTime: number;
  /** Tokens per second (prompt) */
  promptTPS: number;
  /** Tokens per second (generation) */
  generationTPS: number;
  /** Memory usage (MB) */
  memoryUsage: number;
  /** Peak memory (MB) */
  peakMemory: number;
}

/** CLI configuration */
export interface CLIConfig {
  /** Default model path */
  defaultModel?: string;
  /** Default backend */
  defaultBackend?: AccelerationBackend;
  /** Models directory */
  modelsDir?: string;
  /** Cache directory */
  cacheDir?: string;
  /** Log level */
  logLevel?: 'debug' | 'info' | 'warn' | 'error';
  /** Enable streaming output */
  streaming?: boolean;
}

/** Chat message */
export interface ChatMessage {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

/** Chat completion options */
export interface ChatCompletionOptions extends GenerationParams {
  /** System prompt */
  systemPrompt?: string;
  /** Chat history */
  messages?: ChatMessage[];
}
