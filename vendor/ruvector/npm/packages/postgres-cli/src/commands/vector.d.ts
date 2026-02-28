/**
 * Vector Commands
 * CLI commands for vector operations
 */
import type { RuVectorClient } from '../client.js';
export interface VectorCreateOptions {
    dim: string;
    index: 'hnsw' | 'ivfflat';
}
export interface VectorInsertOptions {
    file?: string;
    text?: string;
}
export interface VectorSearchOptions {
    query?: string;
    text?: string;
    topK: string;
    metric: 'cosine' | 'l2' | 'ip';
}
export interface VectorDistanceOptions {
    a: string;
    b: string;
    metric: 'cosine' | 'l2' | 'ip';
}
export interface VectorNormalizeOptions {
    vector: string;
}
export declare class VectorCommands {
    static distance(client: RuVectorClient, options: VectorDistanceOptions): Promise<void>;
    static normalize(client: RuVectorClient, options: VectorNormalizeOptions): Promise<void>;
    static create(client: RuVectorClient, name: string, options: VectorCreateOptions): Promise<void>;
    static insert(client: RuVectorClient, table: string, options: VectorInsertOptions): Promise<void>;
    static search(client: RuVectorClient, table: string, options: VectorSearchOptions): Promise<void>;
}
export default VectorCommands;
//# sourceMappingURL=vector.d.ts.map