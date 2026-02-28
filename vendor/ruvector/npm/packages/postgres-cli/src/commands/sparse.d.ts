/**
 * Sparse Vector Commands
 * CLI commands for sparse vector operations including BM25, sparsification, and distance calculations
 */
import type { RuVectorClient } from '../client.js';
export interface SparseCreateOptions {
    indices: string;
    values: string;
    dim: string;
}
export interface SparseDistanceOptions {
    a: string;
    b: string;
    metric: 'dot' | 'cosine' | 'euclidean' | 'manhattan';
}
export interface SparseBM25Options {
    query: string;
    doc: string;
    docLen: string;
    avgDocLen: string;
    k1?: string;
    b?: string;
}
export interface SparseTopKOptions {
    sparse: string;
    k: string;
}
export interface SparsePruneOptions {
    sparse: string;
    threshold: string;
}
export interface DenseToSparseOptions {
    dense: string;
}
export declare class SparseCommands {
    static create(client: RuVectorClient, options: SparseCreateOptions): Promise<void>;
    static distance(client: RuVectorClient, options: SparseDistanceOptions): Promise<void>;
    static bm25(client: RuVectorClient, options: SparseBM25Options): Promise<void>;
    static topK(client: RuVectorClient, options: SparseTopKOptions): Promise<void>;
    static prune(client: RuVectorClient, options: SparsePruneOptions): Promise<void>;
    static denseToSparse(client: RuVectorClient, options: DenseToSparseOptions): Promise<void>;
    static sparseToDense(client: RuVectorClient, sparse: string): Promise<void>;
    static info(client: RuVectorClient, sparse: string): Promise<void>;
    static showHelp(): void;
}
export default SparseCommands;
//# sourceMappingURL=sparse.d.ts.map