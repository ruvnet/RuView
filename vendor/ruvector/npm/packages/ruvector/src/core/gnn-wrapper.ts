/**
 * GNN Wrapper - Safe wrapper around @ruvector/gnn with automatic array conversion
 *
 * This wrapper handles the array type conversion automatically, allowing users
 * to pass either regular arrays or Float32Arrays.
 *
 * The native @ruvector/gnn requires Float32Array for maximum performance.
 * This wrapper converts any input type to Float32Array automatically.
 *
 * Performance Tips:
 * - Pass Float32Array directly for zero-copy performance
 * - Use toFloat32Array/toFloat32ArrayBatch for pre-conversion
 * - Avoid repeated conversions in hot paths
 */

// Lazy load to avoid import errors if not installed
let gnnModule: any = null;
let loadError: Error | null = null;

function getGnnModule() {
  if (gnnModule) return gnnModule;
  if (loadError) throw loadError;

  try {
    gnnModule = require('@ruvector/gnn');
    return gnnModule;
  } catch (e: any) {
    loadError = new Error(
      `@ruvector/gnn is not installed or failed to load: ${e.message}\n` +
      `Install with: npm install @ruvector/gnn`
    );
    throw loadError;
  }
}

/**
 * Convert any array-like input to Float32Array (native requires Float32Array)
 * Optimized paths:
 * - Float32Array: zero-copy return
 * - Float64Array: efficient typed array copy
 * - Array: direct Float32Array construction
 */
export function toFloat32Array(input: number[] | Float32Array | Float64Array): Float32Array {
  if (input instanceof Float32Array) return input;
  if (input instanceof Float64Array) return new Float32Array(input);
  if (Array.isArray(input)) return new Float32Array(input);
  return new Float32Array(Array.from(input));
}

/**
 * Convert array of arrays to array of Float32Arrays
 */
export function toFloat32ArrayBatch(input: (number[] | Float32Array | Float64Array)[]): Float32Array[] {
  const result = new Array(input.length);
  for (let i = 0; i < input.length; i++) {
    result[i] = toFloat32Array(input[i]);
  }
  return result;
}

/**
 * Search result from differentiable search
 */
export interface DifferentiableSearchResult {
  /** Indices of top-k candidates */
  indices: number[];
  /** Soft weights for top-k candidates */
  weights: number[];
}

/**
 * Differentiable search using soft attention mechanism
 *
 * This wrapper automatically converts Float32Array inputs to regular arrays.
 *
 * @param query - Query vector (array or Float32Array)
 * @param candidates - List of candidate vectors (arrays or Float32Arrays)
 * @param k - Number of top results to return
 * @param temperature - Temperature for softmax (lower = sharper, higher = smoother)
 * @returns Search result with indices and soft weights
 *
 * @example
 * ```typescript
 * import { differentiableSearch } from 'ruvector/core/gnn-wrapper';
 *
 * // Works with regular arrays (auto-converted to Float32Array)
 * const result1 = differentiableSearch([1, 0, 0], [[1, 0, 0], [0, 1, 0]], 2, 1.0);
 *
 * // For best performance, use Float32Array directly (zero-copy)
 * const query = new Float32Array([1, 0, 0]);
 * const candidates = [new Float32Array([1, 0, 0]), new Float32Array([0, 1, 0])];
 * const result2 = differentiableSearch(query, candidates, 2, 1.0);
 * ```
 */
export function differentiableSearch(
  query: number[] | Float32Array | Float64Array,
  candidates: (number[] | Float32Array | Float64Array)[],
  k: number,
  temperature: number = 1.0
): DifferentiableSearchResult {
  const gnn = getGnnModule();

  // Convert to Float32Array (native Rust expects Float32Array for performance)
  const queryFloat32 = toFloat32Array(query);
  const candidatesFloat32 = toFloat32ArrayBatch(candidates);

  return gnn.differentiableSearch(queryFloat32, candidatesFloat32, k, temperature);
}

/**
 * GNN Layer for HNSW topology
 */
export class RuvectorLayer {
  private inner: any;

  /**
   * Create a new Ruvector GNN layer
   *
   * @param inputDim - Dimension of input node embeddings
   * @param hiddenDim - Dimension of hidden representations
   * @param heads - Number of attention heads
   * @param dropout - Dropout rate (0.0 to 1.0)
   */
  constructor(inputDim: number, hiddenDim: number, heads: number, dropout: number = 0.1) {
    const gnn = getGnnModule();
    this.inner = new gnn.RuvectorLayer(inputDim, hiddenDim, heads, dropout);
  }

  /**
   * Forward pass through the GNN layer
   *
   * @param nodeEmbedding - Current node's embedding
   * @param neighborEmbeddings - Embeddings of neighbor nodes
   * @param edgeWeights - Weights of edges to neighbors
   * @returns Updated node embedding as Float32Array
   */
  forward(
    nodeEmbedding: number[] | Float32Array,
    neighborEmbeddings: (number[] | Float32Array)[],
    edgeWeights: number[] | Float32Array
  ): Float32Array {
    return this.inner.forward(
      toFloat32Array(nodeEmbedding),
      toFloat32ArrayBatch(neighborEmbeddings),
      toFloat32Array(edgeWeights)
    );
  }

  /**
   * Serialize the layer to JSON
   */
  toJson(): string {
    return this.inner.toJson();
  }

  /**
   * Deserialize the layer from JSON
   */
  static fromJson(json: string): RuvectorLayer {
    const gnn = getGnnModule();
    const layer = new RuvectorLayer(1, 1, 1, 0); // Dummy constructor
    layer.inner = gnn.RuvectorLayer.fromJson(json);
    return layer;
  }
}

/**
 * Tensor compressor with adaptive level selection
 */
export class TensorCompress {
  private inner: any;

  constructor() {
    const gnn = getGnnModule();
    this.inner = new gnn.TensorCompress();
  }

  /**
   * Compress an embedding based on access frequency
   *
   * @param embedding - Input embedding vector
   * @param accessFreq - Access frequency (0.0 to 1.0)
   * @returns Compressed tensor as JSON string
   */
  compress(embedding: number[] | Float32Array, accessFreq: number): string {
    return this.inner.compress(toFloat32Array(embedding), accessFreq);
  }

  /**
   * Decompress a compressed tensor
   *
   * @param compressedJson - Compressed tensor JSON
   * @returns Decompressed embedding
   */
  decompress(compressedJson: string): number[] {
    return this.inner.decompress(compressedJson);
  }
}

/**
 * Hierarchical forward pass through GNN layers
 *
 * @param query - Query vector
 * @param layerEmbeddings - Embeddings organized by layer
 * @param gnnLayersJson - JSON array of serialized GNN layers
 * @returns Final embedding after hierarchical processing as Float32Array
 */
export function hierarchicalForward(
  query: number[] | Float32Array,
  layerEmbeddings: (number[] | Float32Array)[][],
  gnnLayersJson: string[]
): Float32Array {
  const gnn = getGnnModule();
  return gnn.hierarchicalForward(
    toFloat32Array(query),
    layerEmbeddings.map(layer => toFloat32ArrayBatch(layer)),
    gnnLayersJson
  );
}

/**
 * Get compression level for a given access frequency
 */
export function getCompressionLevel(accessFreq: number): string {
  const gnn = getGnnModule();
  return gnn.getCompressionLevel(accessFreq);
}

/**
 * Check if GNN module is available
 */
export function isGnnAvailable(): boolean {
  try {
    getGnnModule();
    return true;
  } catch {
    return false;
  }
}

export default {
  differentiableSearch,
  RuvectorLayer,
  TensorCompress,
  hierarchicalForward,
  getCompressionLevel,
  isGnnAvailable,
  // Export conversion helpers for performance optimization
  toFloat32Array,
  toFloat32ArrayBatch,
};
