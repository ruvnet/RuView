/**
 * RuVector WASM Unified - Attention Engine
 *
 * Provides 14+ attention mechanisms including:
 * - 7 Neural attention mechanisms (scaled-dot, multi-head, hyperbolic, linear, flash, local-global, MoE, Mamba)
 * - 7 DAG attention mechanisms (topological, mincut-gated, hierarchical, spectral, flow, causal, sparse)
 */
import type { MultiHeadConfig, MoEResult, MambaResult, AttentionScores, QueryDag, GatePacket, AttentionConfig } from './types';
/**
 * Core attention engine providing all neural and DAG attention mechanisms
 */
export interface AttentionEngine {
    /**
     * Scaled dot-product attention: softmax(QK^T / sqrt(d_k)) * V
     * @param Q Query tensor [batch, seq_len, d_k]
     * @param K Key tensor [batch, seq_len, d_k]
     * @param V Value tensor [batch, seq_len, d_v]
     * @param mask Optional attention mask
     * @returns Attention output [batch, seq_len, d_v]
     */
    scaledDot(Q: Float32Array, K: Float32Array, V: Float32Array, mask?: Float32Array): Float32Array;
    /**
     * Multi-head attention with configurable heads
     * @param query Query tensor
     * @param keys Array of key tensors for each head
     * @param values Array of value tensors for each head
     * @param config Multi-head configuration
     * @returns Concatenated and projected attention output
     */
    multiHead(query: Float32Array, keys: Float32Array[], values: Float32Array[], config: MultiHeadConfig): Float32Array;
    /**
     * Hyperbolic attention in Poincare ball model
     * Uses Mobius operations for attention in hyperbolic space
     * @param query Query in hyperbolic space
     * @param keys Keys in hyperbolic space
     * @param values Values in hyperbolic space
     * @param curvature Negative curvature of the manifold (default: -1)
     * @returns Attention output in hyperbolic space
     */
    hyperbolic(query: Float32Array, keys: Float32Array[], values: Float32Array[], curvature?: number): Float32Array;
    /**
     * Linear attention with kernel feature maps
     * O(n) complexity instead of O(n^2)
     * @param query Query tensor
     * @param keys Key tensors
     * @param values Value tensors
     * @param kernel Kernel function: 'elu' | 'relu' | 'softmax' (default: 'elu')
     * @returns Linear attention output
     */
    linear(query: Float32Array, keys: Float32Array[], values: Float32Array[], kernel?: 'elu' | 'relu' | 'softmax'): Float32Array;
    /**
     * Flash attention with memory-efficient tiling
     * Reduces memory from O(n^2) to O(n)
     * @param query Query tensor
     * @param keys Key tensors
     * @param values Value tensors
     * @param blockSize Tile size for chunked computation (default: 256)
     * @returns Flash attention output
     */
    flash(query: Float32Array, keys: Float32Array[], values: Float32Array[], blockSize?: number): Float32Array;
    /**
     * Local-global attention combining sliding window with global tokens
     * @param query Query tensor
     * @param keys Key tensors
     * @param values Value tensors
     * @param windowSize Local attention window size
     * @param globalIndices Indices of global attention tokens
     * @returns Local-global attention output
     */
    localGlobal(query: Float32Array, keys: Float32Array[], values: Float32Array[], windowSize: number, globalIndices?: number[]): Float32Array;
    /**
     * Mixture of Experts attention with top-k routing
     * @param query Query tensor
     * @param keys Key tensors
     * @param values Value tensors
     * @param numExperts Number of expert heads
     * @param topK Number of experts to route to per token
     * @param balanceLoss Whether to compute load balancing loss
     * @returns MoE result with output and routing info
     */
    moe(query: Float32Array, keys: Float32Array[], values: Float32Array[], numExperts: number, topK: number, balanceLoss?: boolean): MoEResult;
    /**
     * Mamba selective state space attention
     * Linear-time sequence modeling with selective state spaces
     * @param input Input sequence
     * @param state Previous hidden state
     * @param config Mamba configuration
     * @returns Mamba result with output and new state
     */
    mamba(input: Float32Array, state: Float32Array, config?: MambaConfig): MambaResult;
    /**
     * Topological attention following DAG structure
     * Respects topological ordering for information flow
     * @param dag Query DAG with nodes and edges
     * @returns Attention scores following topological order
     */
    dagTopological(dag: QueryDag): AttentionScores;
    /**
     * Mincut-gated attention with selective information flow
     * Uses graph cuts for attention gating
     * @param dag Query DAG
     * @param gatePacket Gating configuration
     * @returns Gated attention scores
     */
    dagMincutGated(dag: QueryDag, gatePacket: GatePacket): AttentionScores;
    /**
     * Hierarchical DAG attention with multi-scale aggregation
     * @param dag Query DAG
     * @param levels Number of hierarchy levels
     * @returns Hierarchical attention scores
     */
    dagHierarchical(dag: QueryDag, levels?: number): AttentionScores;
    /**
     * Spectral attention using graph Laplacian eigenvectors
     * @param dag Query DAG
     * @param numEigenvectors Number of spectral components
     * @returns Spectral attention scores
     */
    dagSpectral(dag: QueryDag, numEigenvectors?: number): AttentionScores;
    /**
     * Flow-based attention using max-flow algorithms
     * @param dag Query DAG
     * @param sourceIds Source node IDs
     * @param sinkIds Sink node IDs
     * @returns Flow-based attention scores
     */
    dagFlow(dag: QueryDag, sourceIds: string[], sinkIds: string[]): AttentionScores;
    /**
     * Causal DAG attention respecting temporal ordering
     * @param dag Query DAG with temporal annotations
     * @returns Causally-masked attention scores
     */
    dagCausal(dag: QueryDag): AttentionScores;
    /**
     * Sparse DAG attention with adaptive sparsity
     * @param dag Query DAG
     * @param sparsityRatio Target sparsity ratio (0-1)
     * @returns Sparse attention scores
     */
    dagSparse(dag: QueryDag, sparsityRatio?: number): AttentionScores;
}
/** Mamba configuration */
export interface MambaConfig {
    dState: number;
    dConv: number;
    expand: number;
    dt_rank: 'auto' | number;
    dt_min: number;
    dt_max: number;
    dt_init: 'constant' | 'random';
    dt_scale: number;
    conv_bias: boolean;
    bias: boolean;
}
/** Attention mechanism type */
export type AttentionMechanism = 'scaled-dot' | 'multi-head' | 'hyperbolic' | 'linear' | 'flash' | 'local-global' | 'moe' | 'mamba' | 'dag-topological' | 'dag-mincut' | 'dag-hierarchical' | 'dag-spectral' | 'dag-flow' | 'dag-causal' | 'dag-sparse';
/**
 * Create an attention engine instance
 * @param config Optional configuration
 * @returns Initialized attention engine
 */
export declare function createAttentionEngine(config?: AttentionConfig): AttentionEngine;
/**
 * Get list of available attention mechanisms
 */
export declare function listAttentionMechanisms(): AttentionMechanism[];
/**
 * Benchmark attention mechanism performance
 * @param mechanism Mechanism to benchmark
 * @param inputSize Input tensor size
 * @param iterations Number of iterations
 * @returns Benchmark results
 */
export declare function benchmarkAttention(mechanism: AttentionMechanism, inputSize: number, iterations?: number): Promise<{
    mechanism: AttentionMechanism;
    avgTimeMs: number;
    throughputOpsPerSec: number;
    memoryPeakBytes: number;
}>;
//# sourceMappingURL=attention.d.ts.map