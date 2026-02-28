/* tslint:disable */
/* eslint-disable */

export class DagAttentionFactory {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get available DAG attention types
   */
  static availableTypes(): any;
  /**
   * Get description for a DAG attention type
   */
  static getDescription(attention_type: string): string;
}

export class GraphAttentionFactory {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get recommended use cases for a graph attention type
   */
  static getUseCases(attention_type: string): any;
  /**
   * Get available graph attention types
   */
  static availableTypes(): any;
  /**
   * Get description for a graph attention type
   */
  static getDescription(attention_type: string): string;
}

/**
 * Graph attention mechanism types
 */
export enum GraphAttentionType {
  /**
   * Graph Attention Networks (Velickovic et al., 2018)
   */
  GAT = 0,
  /**
   * Graph Convolutional Networks (Kipf & Welling, 2017)
   */
  GCN = 1,
  /**
   * GraphSAGE (Hamilton et al., 2017)
   */
  GraphSAGE = 2,
}

export class HybridMambaAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new hybrid Mamba-Attention layer
   */
  constructor(config: MambaConfig, local_window: number);
  /**
   * Forward pass
   */
  forward(input: Float32Array, seq_len: number): Float32Array;
  /**
   * Get local window size
   */
  readonly localWindow: number;
}

export class MambaConfig {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Set state space dimension
   */
  withStateDim(state_dim: number): MambaConfig;
  /**
   * Set expansion factor
   */
  withExpandFactor(factor: number): MambaConfig;
  /**
   * Set convolution kernel size
   */
  withConvKernelSize(size: number): MambaConfig;
  /**
   * Create a new Mamba configuration
   */
  constructor(dim: number);
  /**
   * Model dimension (d_model)
   */
  dim: number;
  /**
   * State space dimension (n)
   */
  state_dim: number;
  /**
   * Expansion factor for inner dimension
   */
  expand_factor: number;
  /**
   * Convolution kernel size
   */
  conv_kernel_size: number;
  /**
   * Delta (discretization step) range minimum
   */
  dt_min: number;
  /**
   * Delta range maximum
   */
  dt_max: number;
  /**
   * Whether to use learnable D skip connection
   */
  use_d_skip: boolean;
}

export class MambaSSMAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create with default configuration
   */
  static withDefaults(dim: number): MambaSSMAttention;
  /**
   * Compute attention-like scores (for visualization/analysis)
   *
   * Returns pseudo-attention scores showing which positions influence output
   */
  getAttentionScores(input: Float32Array, seq_len: number): Float32Array;
  /**
   * Create a new Mamba SSM attention layer
   */
  constructor(config: MambaConfig);
  /**
   * Forward pass through Mamba SSM
   *
   * # Arguments
   * * `input` - Input sequence (seq_len, dim) flattened to 1D
   * * `seq_len` - Sequence length
   *
   * # Returns
   * Output sequence (seq_len, dim) flattened to 1D
   */
  forward(input: Float32Array, seq_len: number): Float32Array;
  /**
   * Get the configuration
   */
  readonly config: MambaConfig;
  /**
   * Get the inner dimension
   */
  readonly innerDim: number;
}

export class UnifiedAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Check if this mechanism supports graph/DAG structures
   */
  supportsGraphs(): boolean;
  /**
   * Check if this mechanism supports sequence processing
   */
  supportsSequences(): boolean;
  /**
   * Check if this mechanism supports hyperbolic geometry
   */
  supportsHyperbolic(): boolean;
  /**
   * Create a new unified attention selector
   */
  constructor(mechanism: string);
  /**
   * Get the category of the selected mechanism
   */
  readonly category: string;
  /**
   * Get the currently selected mechanism type
   */
  readonly mechanism: string;
}

export class WasmCausalConeAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new causal cone attention instance
   *
   * # Arguments
   * * `future_discount` - Discount for future nodes
   * * `ancestor_weight` - Weight for ancestor influence
   */
  constructor(future_discount: number, ancestor_weight: number);
  /**
   * Compute attention scores for the DAG
   */
  forward(dag: WasmQueryDag): Float32Array;
}

export class WasmCriticalPathAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new critical path attention instance
   *
   * # Arguments
   * * `path_weight` - Weight for critical path membership
   * * `branch_penalty` - Penalty for branching nodes
   */
  constructor(path_weight: number, branch_penalty: number);
  /**
   * Compute attention scores for the DAG
   */
  forward(dag: WasmQueryDag): Float32Array;
}

export class WasmFlashAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new flash attention instance
   *
   * # Arguments
   * * `dim` - Embedding dimension
   * * `block_size` - Block size for tiled computation
   */
  constructor(dim: number, block_size: number);
  /**
   * Compute flash attention
   */
  compute(query: Float32Array, keys: any, values: any): Float32Array;
}

export class WasmGNNLayer {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new GNN layer with attention
   *
   * # Arguments
   * * `input_dim` - Dimension of input node embeddings
   * * `hidden_dim` - Dimension of hidden representations
   * * `heads` - Number of attention heads
   * * `dropout` - Dropout rate (0.0 to 1.0)
   */
  constructor(input_dim: number, hidden_dim: number, heads: number, dropout: number);
  /**
   * Forward pass through the GNN layer
   *
   * # Arguments
   * * `node_embedding` - Current node's embedding (Float32Array)
   * * `neighbor_embeddings` - Embeddings of neighbor nodes (array of Float32Arrays)
   * * `edge_weights` - Weights of edges to neighbors (Float32Array)
   *
   * # Returns
   * Updated node embedding (Float32Array)
   */
  forward(node_embedding: Float32Array, neighbor_embeddings: any, edge_weights: Float32Array): Float32Array;
  /**
   * Get the output dimension
   */
  readonly outputDim: number;
}

export class WasmHierarchicalLorentzAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new hierarchical Lorentz attention instance
   *
   * # Arguments
   * * `curvature` - Hyperbolic curvature parameter
   * * `temperature` - Temperature for softmax
   */
  constructor(curvature: number, temperature: number);
  /**
   * Compute attention scores for the DAG
   */
  forward(dag: WasmQueryDag): Float32Array;
}

export class WasmHyperbolicAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new hyperbolic attention instance
   *
   * # Arguments
   * * `dim` - Embedding dimension
   * * `curvature` - Hyperbolic curvature parameter (negative for hyperbolic space)
   */
  constructor(dim: number, curvature: number);
  /**
   * Compute hyperbolic attention
   */
  compute(query: Float32Array, keys: any, values: any): Float32Array;
  /**
   * Get the curvature parameter
   */
  readonly curvature: number;
}

export class WasmLinearAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new linear attention instance
   *
   * # Arguments
   * * `dim` - Embedding dimension
   * * `num_features` - Number of random features for kernel approximation
   */
  constructor(dim: number, num_features: number);
  /**
   * Compute linear attention
   */
  compute(query: Float32Array, keys: any, values: any): Float32Array;
}

export class WasmLocalGlobalAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new local-global attention instance
   *
   * # Arguments
   * * `dim` - Embedding dimension
   * * `local_window` - Size of local attention window
   * * `global_tokens` - Number of global attention tokens
   */
  constructor(dim: number, local_window: number, global_tokens: number);
  /**
   * Compute local-global attention
   */
  compute(query: Float32Array, keys: any, values: any): Float32Array;
}

export class WasmMinCutGatedAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new MinCut-gated attention instance
   *
   * # Arguments
   * * `gate_threshold` - Threshold for gating (0.0-1.0)
   */
  constructor(gate_threshold: number);
  /**
   * Compute attention scores for the DAG
   */
  forward(dag: WasmQueryDag): Float32Array;
}

export class WasmMoEAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new MoE attention instance
   *
   * # Arguments
   * * `dim` - Embedding dimension
   * * `num_experts` - Number of expert attention mechanisms
   * * `top_k` - Number of experts to activate per query
   */
  constructor(dim: number, num_experts: number, top_k: number);
  /**
   * Compute MoE attention
   */
  compute(query: Float32Array, keys: any, values: any): Float32Array;
}

export class WasmMultiHeadAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new multi-head attention instance
   *
   * # Arguments
   * * `dim` - Embedding dimension (must be divisible by num_heads)
   * * `num_heads` - Number of parallel attention heads
   */
  constructor(dim: number, num_heads: number);
  /**
   * Compute multi-head attention
   *
   * # Arguments
   * * `query` - Query vector
   * * `keys` - Array of key vectors
   * * `values` - Array of value vectors
   */
  compute(query: Float32Array, keys: any, values: any): Float32Array;
  /**
   * Get the embedding dimension
   */
  readonly dim: number;
  /**
   * Get the dimension per head
   */
  readonly headDim: number;
  /**
   * Get the number of attention heads
   */
  readonly numHeads: number;
}

export class WasmParallelBranchAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new parallel branch attention instance
   *
   * # Arguments
   * * `max_branches` - Maximum number of branches to consider
   * * `sync_penalty` - Penalty for synchronization between branches
   */
  constructor(max_branches: number, sync_penalty: number);
  /**
   * Compute attention scores for the DAG
   */
  forward(dag: WasmQueryDag): Float32Array;
}

export class WasmQueryDag {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new empty DAG
   */
  constructor();
  /**
   * Serialize to JSON
   */
  toJson(): string;
  /**
   * Add an edge between nodes
   *
   * # Arguments
   * * `from` - Source node ID
   * * `to` - Target node ID
   *
   * # Returns
   * True if edge was added successfully
   */
  addEdge(from: number, to: number): boolean;
  /**
   * Add a node with operator type and cost
   *
   * # Arguments
   * * `op_type` - Operator type: "scan", "filter", "join", "aggregate", "project", "sort"
   * * `cost` - Estimated execution cost
   *
   * # Returns
   * Node ID
   */
  addNode(op_type: string, cost: number): number;
  /**
   * Get the number of edges
   */
  readonly edgeCount: number;
  /**
   * Get the number of nodes
   */
  readonly nodeCount: number;
}

export class WasmSearchConfig {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new search configuration
   */
  constructor(k: number, temperature: number);
  /**
   * Number of top results to return
   */
  k: number;
  /**
   * Temperature for softmax
   */
  temperature: number;
}

export class WasmTemporalBTSPAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new temporal BTSP attention instance
   *
   * # Arguments
   * * `eligibility_decay` - Decay rate for eligibility traces (0.0-1.0)
   * * `baseline_attention` - Baseline attention for nodes without history
   */
  constructor(eligibility_decay: number, baseline_attention: number);
  /**
   * Compute attention scores for the DAG
   */
  forward(dag: WasmQueryDag): Float32Array;
}

export class WasmTensorCompress {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Decompress a compressed tensor
   */
  decompress(compressed: any): Float32Array;
  /**
   * Compress with explicit compression level
   *
   * # Arguments
   * * `embedding` - The input embedding vector
   * * `level` - Compression level: "none", "half", "pq8", "pq4", "binary"
   */
  compressWithLevel(embedding: Float32Array, level: string): any;
  /**
   * Get compression ratio estimate for a given access frequency
   */
  getCompressionRatio(access_freq: number): number;
  /**
   * Create a new tensor compressor
   */
  constructor();
  /**
   * Compress an embedding based on access frequency
   *
   * # Arguments
   * * `embedding` - The input embedding vector
   * * `access_freq` - Access frequency in range [0.0, 1.0]
   *   - f > 0.8: Full precision (hot data)
   *   - f > 0.4: Half precision (warm data)
   *   - f > 0.1: 8-bit PQ (cool data)
   *   - f > 0.01: 4-bit PQ (cold data)
   *   - f <= 0.01: Binary (archive)
   */
  compress(embedding: Float32Array, access_freq: number): any;
}

export class WasmTopologicalAttention {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new topological attention instance
   *
   * # Arguments
   * * `decay_factor` - Decay factor for position-based attention (0.0-1.0)
   */
  constructor(decay_factor: number);
  /**
   * Compute attention scores for the DAG
   *
   * # Returns
   * Attention scores for each node
   */
  forward(dag: WasmQueryDag): Float32Array;
}

/**
 * Get information about all available attention mechanisms
 */
export function availableMechanisms(): any;

/**
 * Compute cosine similarity between two vectors
 */
export function cosineSimilarity(a: Float32Array, b: Float32Array): number;

/**
 * Get summary statistics about the unified attention library
 */
export function getStats(): any;

/**
 * Differentiable search using soft attention mechanism
 *
 * # Arguments
 * * `query` - The query vector
 * * `candidate_embeddings` - List of candidate embedding vectors
 * * `config` - Search configuration
 *
 * # Returns
 * Object with indices and weights for top-k candidates
 */
export function graphDifferentiableSearch(query: Float32Array, candidate_embeddings: any, config: WasmSearchConfig): any;

/**
 * Hierarchical forward pass through multiple GNN layers
 *
 * # Arguments
 * * `query` - The query vector
 * * `layer_embeddings` - Embeddings organized by layer
 * * `gnn_layers` - Array of GNN layers
 *
 * # Returns
 * Final embedding after hierarchical processing
 */
export function graphHierarchicalForward(query: Float32Array, layer_embeddings: any, gnn_layers: WasmGNNLayer[]): Float32Array;

/**
 * Initialize the WASM module with panic hook for better error messages
 */
export function init(): void;

/**
 * Compute scaled dot-product attention
 *
 * Standard transformer attention: softmax(QK^T / sqrt(d)) * V
 *
 * # Arguments
 * * `query` - Query vector (Float32Array)
 * * `keys` - Array of key vectors (JsValue - array of Float32Arrays)
 * * `values` - Array of value vectors (JsValue - array of Float32Arrays)
 * * `scale` - Optional scaling factor (defaults to 1/sqrt(dim))
 *
 * # Returns
 * Attention-weighted output vector
 */
export function scaledDotAttention(query: Float32Array, keys: any, values: any, scale?: number | null): Float32Array;

/**
 * Softmax normalization
 */
export function softmax(values: Float32Array): Float32Array;

/**
 * Temperature-scaled softmax
 */
export function temperatureSoftmax(values: Float32Array, temperature: number): Float32Array;

/**
 * Get the version of the unified attention WASM crate
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_dagattentionfactory_free: (a: number, b: number) => void;
  readonly __wbg_get_mambaconfig_conv_kernel_size: (a: number) => number;
  readonly __wbg_get_mambaconfig_dim: (a: number) => number;
  readonly __wbg_get_mambaconfig_dt_max: (a: number) => number;
  readonly __wbg_get_mambaconfig_dt_min: (a: number) => number;
  readonly __wbg_get_mambaconfig_expand_factor: (a: number) => number;
  readonly __wbg_get_mambaconfig_state_dim: (a: number) => number;
  readonly __wbg_get_mambaconfig_use_d_skip: (a: number) => number;
  readonly __wbg_get_wasmsearchconfig_temperature: (a: number) => number;
  readonly __wbg_hybridmambaattention_free: (a: number, b: number) => void;
  readonly __wbg_mambaconfig_free: (a: number, b: number) => void;
  readonly __wbg_mambassmattention_free: (a: number, b: number) => void;
  readonly __wbg_set_mambaconfig_conv_kernel_size: (a: number, b: number) => void;
  readonly __wbg_set_mambaconfig_dim: (a: number, b: number) => void;
  readonly __wbg_set_mambaconfig_dt_max: (a: number, b: number) => void;
  readonly __wbg_set_mambaconfig_dt_min: (a: number, b: number) => void;
  readonly __wbg_set_mambaconfig_expand_factor: (a: number, b: number) => void;
  readonly __wbg_set_mambaconfig_state_dim: (a: number, b: number) => void;
  readonly __wbg_set_mambaconfig_use_d_skip: (a: number, b: number) => void;
  readonly __wbg_set_wasmsearchconfig_temperature: (a: number, b: number) => void;
  readonly __wbg_unifiedattention_free: (a: number, b: number) => void;
  readonly __wbg_wasmcausalconeattention_free: (a: number, b: number) => void;
  readonly __wbg_wasmflashattention_free: (a: number, b: number) => void;
  readonly __wbg_wasmgnnlayer_free: (a: number, b: number) => void;
  readonly __wbg_wasmhyperbolicattention_free: (a: number, b: number) => void;
  readonly __wbg_wasmlinearattention_free: (a: number, b: number) => void;
  readonly __wbg_wasmmincutgatedattention_free: (a: number, b: number) => void;
  readonly __wbg_wasmmoeattention_free: (a: number, b: number) => void;
  readonly __wbg_wasmmultiheadattention_free: (a: number, b: number) => void;
  readonly __wbg_wasmquerydag_free: (a: number, b: number) => void;
  readonly __wbg_wasmtensorcompress_free: (a: number, b: number) => void;
  readonly availableMechanisms: () => number;
  readonly cosineSimilarity: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly dagattentionfactory_availableTypes: () => number;
  readonly dagattentionfactory_getDescription: (a: number, b: number, c: number) => void;
  readonly getStats: () => number;
  readonly graphDifferentiableSearch: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly graphHierarchicalForward: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly graphattentionfactory_availableTypes: () => number;
  readonly graphattentionfactory_getDescription: (a: number, b: number, c: number) => void;
  readonly graphattentionfactory_getUseCases: (a: number, b: number) => number;
  readonly hybridmambaattention_forward: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly hybridmambaattention_localWindow: (a: number) => number;
  readonly hybridmambaattention_new: (a: number, b: number) => number;
  readonly mambaconfig_new: (a: number) => number;
  readonly mambaconfig_withConvKernelSize: (a: number, b: number) => number;
  readonly mambaconfig_withExpandFactor: (a: number, b: number) => number;
  readonly mambaconfig_withStateDim: (a: number, b: number) => number;
  readonly mambassmattention_config: (a: number) => number;
  readonly mambassmattention_forward: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly mambassmattention_getAttentionScores: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly mambassmattention_innerDim: (a: number) => number;
  readonly mambassmattention_new: (a: number) => number;
  readonly mambassmattention_withDefaults: (a: number) => number;
  readonly scaledDotAttention: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly softmax: (a: number, b: number, c: number) => void;
  readonly temperatureSoftmax: (a: number, b: number, c: number, d: number) => void;
  readonly unifiedattention_category: (a: number, b: number) => void;
  readonly unifiedattention_mechanism: (a: number, b: number) => void;
  readonly unifiedattention_new: (a: number, b: number, c: number) => void;
  readonly unifiedattention_supportsGraphs: (a: number) => number;
  readonly unifiedattention_supportsHyperbolic: (a: number) => number;
  readonly unifiedattention_supportsSequences: (a: number) => number;
  readonly version: (a: number) => void;
  readonly wasmcausalconeattention_forward: (a: number, b: number, c: number) => void;
  readonly wasmcriticalpathattention_forward: (a: number, b: number, c: number) => void;
  readonly wasmflashattention_compute: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly wasmflashattention_new: (a: number, b: number) => number;
  readonly wasmgnnlayer_forward: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
  readonly wasmgnnlayer_new: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly wasmgnnlayer_outputDim: (a: number) => number;
  readonly wasmhierarchicallorentzattention_forward: (a: number, b: number, c: number) => void;
  readonly wasmhyperbolicattention_compute: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly wasmhyperbolicattention_curvature: (a: number) => number;
  readonly wasmhyperbolicattention_new: (a: number, b: number) => number;
  readonly wasmlinearattention_compute: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly wasmlinearattention_new: (a: number, b: number) => number;
  readonly wasmlocalglobalattention_compute: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly wasmlocalglobalattention_new: (a: number, b: number, c: number) => number;
  readonly wasmmincutgatedattention_forward: (a: number, b: number, c: number) => void;
  readonly wasmmoeattention_compute: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly wasmmoeattention_new: (a: number, b: number, c: number) => number;
  readonly wasmmultiheadattention_compute: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly wasmmultiheadattention_dim: (a: number) => number;
  readonly wasmmultiheadattention_headDim: (a: number) => number;
  readonly wasmmultiheadattention_new: (a: number, b: number, c: number) => void;
  readonly wasmmultiheadattention_numHeads: (a: number) => number;
  readonly wasmparallelbranchattention_forward: (a: number, b: number, c: number) => void;
  readonly wasmquerydag_addEdge: (a: number, b: number, c: number) => number;
  readonly wasmquerydag_addNode: (a: number, b: number, c: number, d: number) => number;
  readonly wasmquerydag_edgeCount: (a: number) => number;
  readonly wasmquerydag_new: () => number;
  readonly wasmquerydag_nodeCount: (a: number) => number;
  readonly wasmquerydag_toJson: (a: number, b: number) => void;
  readonly wasmtemporalbtspattention_forward: (a: number, b: number, c: number) => void;
  readonly wasmtensorcompress_compress: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly wasmtensorcompress_compressWithLevel: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly wasmtensorcompress_decompress: (a: number, b: number, c: number) => void;
  readonly wasmtensorcompress_getCompressionRatio: (a: number, b: number) => number;
  readonly wasmtensorcompress_new: () => number;
  readonly wasmtopologicalattention_forward: (a: number, b: number, c: number) => void;
  readonly init: () => void;
  readonly wasmmincutgatedattention_new: (a: number) => number;
  readonly wasmtopologicalattention_new: (a: number) => number;
  readonly __wbg_set_wasmsearchconfig_k: (a: number, b: number) => void;
  readonly wasmcausalconeattention_new: (a: number, b: number) => number;
  readonly wasmcriticalpathattention_new: (a: number, b: number) => number;
  readonly wasmhierarchicallorentzattention_new: (a: number, b: number) => number;
  readonly wasmparallelbranchattention_new: (a: number, b: number) => number;
  readonly wasmsearchconfig_new: (a: number, b: number) => number;
  readonly wasmtemporalbtspattention_new: (a: number, b: number) => number;
  readonly __wbg_get_wasmsearchconfig_k: (a: number) => number;
  readonly __wbg_graphattentionfactory_free: (a: number, b: number) => void;
  readonly __wbg_wasmcriticalpathattention_free: (a: number, b: number) => void;
  readonly __wbg_wasmhierarchicallorentzattention_free: (a: number, b: number) => void;
  readonly __wbg_wasmlocalglobalattention_free: (a: number, b: number) => void;
  readonly __wbg_wasmparallelbranchattention_free: (a: number, b: number) => void;
  readonly __wbg_wasmsearchconfig_free: (a: number, b: number) => void;
  readonly __wbg_wasmtemporalbtspattention_free: (a: number, b: number) => void;
  readonly __wbg_wasmtopologicalattention_free: (a: number, b: number) => void;
  readonly __wbindgen_export: (a: number, b: number) => number;
  readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_export3: (a: number) => void;
  readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
