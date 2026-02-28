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

import * as path from 'path';
import * as fs from 'fs';
import { pathToFileURL } from 'url';
import { createRequire } from 'module';

// Extend globalThis type for ESM require compatibility
declare global {
  // eslint-disable-next-line no-var
  var __ruvector_require: NodeRequire | undefined;
}

// Set up ESM-compatible require for WASM module (fixes Windows/ESM compatibility)
// The WASM bindings use module.require for Node.js crypto, this provides a fallback
if (typeof globalThis !== 'undefined' && !globalThis.__ruvector_require) {
  try {
    // In ESM context, use createRequire with __filename
    globalThis.__ruvector_require = createRequire(__filename);
  } catch {
    // Fallback: require should be available in CommonJS
    try {
      globalThis.__ruvector_require = require;
    } catch {
      // Neither available - WASM will fall back to crypto.getRandomValues
    }
  }
}

// Force native dynamic import (avoids TypeScript transpiling to require)
// eslint-disable-next-line @typescript-eslint/no-implied-eval
const dynamicImport = new Function('specifier', 'return import(specifier)') as (specifier: string) => Promise<any>;

// Types
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

// Capability detection
let simdAvailable = false;
let parallelAvailable = false;

export interface EmbeddingResult {
  embedding: number[];
  dimension: number;
  timeMs: number;
}

export interface SimilarityResult {
  similarity: number;
  timeMs: number;
}

// Lazy-loaded module state
let wasmModule: any = null;
let embedder: any = null;
let parallelEmbedder: any = null;
let loadError: Error | null = null;
let loadPromise: Promise<void> | null = null;
let isInitialized = false;
let parallelEnabled = false;
let parallelThreshold = 4;

// Default model
const DEFAULT_MODEL = 'all-MiniLM-L6-v2';

/**
 * Check if ONNX embedder is available (bundled files exist)
 */
export function isOnnxAvailable(): boolean {
  try {
    const pkgPath = path.join(__dirname, 'onnx', 'pkg', 'ruvector_onnx_embeddings_wasm.js');
    return fs.existsSync(pkgPath);
  } catch {
    return false;
  }
}

/**
 * Check if parallel workers are available (npm package installed)
 */
async function detectParallelAvailable(): Promise<boolean> {
  try {
    await dynamicImport('ruvector-onnx-embeddings-wasm/parallel');
    parallelAvailable = true;
    return true;
  } catch {
    parallelAvailable = false;
    return false;
  }
}

/**
 * Check if SIMD is available (from WASM module)
 */
function detectSimd(): boolean {
  try {
    if (wasmModule && typeof wasmModule.simd_available === 'function') {
      simdAvailable = wasmModule.simd_available();
      return simdAvailable;
    }
  } catch {}
  return false;
}

/**
 * Try to load ParallelEmbedder from npm package (optional)
 */
async function tryInitParallel(config: OnnxEmbedderConfig): Promise<boolean> {
  // Skip if explicitly disabled
  if (config.enableParallel === false) return false;

  // For 'auto' or true, try to initialize
  try {
    const parallelModule = await dynamicImport('ruvector-onnx-embeddings-wasm/parallel');
    const { ParallelEmbedder } = parallelModule;

    parallelEmbedder = new ParallelEmbedder({
      numWorkers: config.numWorkers,
    });
    await parallelEmbedder.init(config.modelId || DEFAULT_MODEL);

    parallelThreshold = config.parallelThreshold || 4;
    parallelEnabled = true;
    parallelAvailable = true;
    console.error(`Parallel embedder ready: ${parallelEmbedder.numWorkers} workers, SIMD: ${simdAvailable}`);
    return true;
  } catch (e: any) {
    parallelAvailable = false;
    if (config.enableParallel === true) {
      // Only warn if explicitly requested
      console.error(`Parallel embedder not available: ${e.message}`);
    }
    return false;
  }
}

/**
 * Initialize the ONNX embedder (downloads model if needed)
 */
export async function initOnnxEmbedder(config: OnnxEmbedderConfig = {}): Promise<boolean> {
  if (isInitialized) return true;
  if (loadError) throw loadError;
  if (loadPromise) {
    await loadPromise;
    return isInitialized;
  }

  loadPromise = (async () => {
    try {
      // Paths to bundled ONNX files
      const pkgPath = path.join(__dirname, 'onnx', 'pkg', 'ruvector_onnx_embeddings_wasm.js');
      const loaderPath = path.join(__dirname, 'onnx', 'loader.js');

      if (!fs.existsSync(pkgPath)) {
        throw new Error('ONNX WASM files not bundled. The onnx/ directory is missing.');
      }

      // Convert paths to file:// URLs for cross-platform ESM compatibility (Windows fix)
      const pkgUrl = pathToFileURL(pkgPath).href;
      const loaderUrl = pathToFileURL(loaderPath).href;

      // Dynamic import of bundled modules using file:// URLs
      wasmModule = await dynamicImport(pkgUrl);

      // Initialize WASM module (loads the .wasm file)
      const wasmPath = path.join(__dirname, 'onnx', 'pkg', 'ruvector_onnx_embeddings_wasm_bg.wasm');
      if (wasmModule.default && typeof wasmModule.default === 'function') {
        // For bundler-style initialization, pass the wasm buffer
        const wasmBytes = fs.readFileSync(wasmPath);
        await wasmModule.default(wasmBytes);
      }

      const loaderModule = await dynamicImport(loaderUrl);
      const { ModelLoader } = loaderModule;

      // Create model loader with caching
      const modelLoader = new ModelLoader({
        cache: true,
        cacheDir: config.cacheDir || path.join(process.env.HOME || '/tmp', '.ruvector', 'models'),
      });

      // Load model (downloads from HuggingFace on first use)
      const modelId = config.modelId || DEFAULT_MODEL;
      console.error(`Loading ONNX model: ${modelId}...`);

      const { modelBytes, tokenizerJson, config: modelConfig } = await modelLoader.loadModel(modelId);

      // Create embedder with config
      const embedderConfig = new wasmModule.WasmEmbedderConfig()
        .setMaxLength(config.maxLength || modelConfig.maxLength || 256)
        .setNormalize(config.normalize !== false)
        .setPooling(0); // Mean pooling

      embedder = wasmModule.WasmEmbedder.withConfig(modelBytes, tokenizerJson, embedderConfig);

      // Detect SIMD capability
      detectSimd();
      console.error(`ONNX embedder ready: ${embedder.dimension()}d, SIMD: ${simdAvailable}`);

      isInitialized = true;

      // Determine if we should use parallel workers
      // - true: always enable
      // - false: never enable
      // - 'auto'/undefined: enable for long-running processes (MCP, servers), skip for CLI
      let shouldTryParallel = false;
      if (config.enableParallel === true) {
        shouldTryParallel = true;
      } else if (config.enableParallel === false) {
        shouldTryParallel = false;
      } else {
        // Auto-detect: check if running as CLI hook or long-running process
        const isCLI = process.argv[1]?.includes('cli.js') ||
                      process.argv[1]?.includes('bin/ruvector') ||
                      process.env.RUVECTOR_CLI === '1';
        const isMCP = process.env.MCP_SERVER === '1' ||
                      process.argv.some(a => a.includes('mcp'));
        const forceParallel = process.env.RUVECTOR_PARALLEL === '1';

        // Enable parallel for MCP/servers or if explicitly requested, skip for CLI
        shouldTryParallel = forceParallel || (isMCP && !isCLI);
      }

      if (shouldTryParallel) {
        await tryInitParallel(config);
      }
    } catch (e: any) {
      loadError = new Error(`Failed to initialize ONNX embedder: ${e.message}`);
      throw loadError;
    }
  })();

  await loadPromise;
  return isInitialized;
}

/**
 * Generate embedding for text
 */
export async function embed(text: string): Promise<EmbeddingResult> {
  if (!isInitialized) {
    await initOnnxEmbedder();
  }
  if (!embedder) {
    throw new Error('ONNX embedder not initialized');
  }

  const start = performance.now();
  const embedding = embedder.embedOne(text);
  const timeMs = performance.now() - start;

  return {
    embedding: Array.from(embedding),
    dimension: embedding.length,
    timeMs,
  };
}

/**
 * Generate embeddings for multiple texts
 * Uses parallel workers automatically for batches >= parallelThreshold
 */
export async function embedBatch(texts: string[]): Promise<EmbeddingResult[]> {
  if (!isInitialized) {
    await initOnnxEmbedder();
  }
  if (!embedder) {
    throw new Error('ONNX embedder not initialized');
  }

  const start = performance.now();

  // Use parallel workers for large batches
  if (parallelEnabled && parallelEmbedder && texts.length >= parallelThreshold) {
    const batchResults = await parallelEmbedder.embedBatch(texts);
    const totalTime = performance.now() - start;
    const dimension = parallelEmbedder.dimension || 384;

    return batchResults.map((emb: number[]) => ({
      embedding: Array.from(emb),
      dimension,
      timeMs: totalTime / texts.length,
    }));
  }

  // Sequential fallback
  const batchEmbeddings = embedder.embedBatch(texts);
  const totalTime = performance.now() - start;

  const dimension = embedder.dimension();
  const results: EmbeddingResult[] = [];

  for (let i = 0; i < texts.length; i++) {
    const embedding = batchEmbeddings.slice(i * dimension, (i + 1) * dimension);
    results.push({
      embedding: Array.from(embedding),
      dimension,
      timeMs: totalTime / texts.length,
    });
  }

  return results;
}

/**
 * Calculate cosine similarity between two texts
 */
export async function similarity(text1: string, text2: string): Promise<SimilarityResult> {
  if (!isInitialized) {
    await initOnnxEmbedder();
  }
  if (!embedder) {
    throw new Error('ONNX embedder not initialized');
  }

  const start = performance.now();
  const sim = embedder.similarity(text1, text2);
  const timeMs = performance.now() - start;

  return { similarity: sim, timeMs };
}

/**
 * Calculate cosine similarity between two embeddings
 */
export function cosineSimilarity(a: number[], b: number[]): number {
  if (a.length !== b.length) {
    throw new Error('Embeddings must have same dimension');
  }

  let dotProduct = 0;
  let normA = 0;
  let normB = 0;

  for (let i = 0; i < a.length; i++) {
    dotProduct += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }

  const magnitude = Math.sqrt(normA) * Math.sqrt(normB);
  return magnitude === 0 ? 0 : dotProduct / magnitude;
}

/**
 * Get embedding dimension
 */
export function getDimension(): number {
  return embedder ? embedder.dimension() : 384;
}

/**
 * Check if embedder is ready
 */
export function isReady(): boolean {
  return isInitialized;
}

/**
 * Get embedder stats including SIMD and parallel capabilities
 */
export function getStats(): {
  ready: boolean;
  dimension: number;
  model: string;
  simd: boolean;
  parallel: boolean;
  parallelWorkers: number;
  parallelThreshold: number;
} {
  return {
    ready: isInitialized,
    dimension: embedder ? embedder.dimension() : 384,
    model: DEFAULT_MODEL,
    simd: simdAvailable,
    parallel: parallelEnabled,
    parallelWorkers: parallelEmbedder?.numWorkers || 0,
    parallelThreshold,
  };
}

/**
 * Shutdown parallel workers (call on exit)
 */
export async function shutdown(): Promise<void> {
  if (parallelEmbedder) {
    await parallelEmbedder.shutdown();
    parallelEmbedder = null;
    parallelEnabled = false;
  }
}

// Export class wrapper for compatibility
export class OnnxEmbedder {
  private config: OnnxEmbedderConfig;

  constructor(config: OnnxEmbedderConfig = {}) {
    this.config = config;
  }

  async init(): Promise<boolean> {
    return initOnnxEmbedder(this.config);
  }

  async embed(text: string): Promise<number[]> {
    const result = await embed(text);
    return result.embedding;
  }

  async embedBatch(texts: string[]): Promise<number[][]> {
    const results = await embedBatch(texts);
    return results.map(r => r.embedding);
  }

  async similarity(text1: string, text2: string): Promise<number> {
    const result = await similarity(text1, text2);
    return result.similarity;
  }

  get dimension(): number {
    return getDimension();
  }

  get ready(): boolean {
    return isReady();
  }
}

export default OnnxEmbedder;
