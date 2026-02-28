/**
 * Native bindings loader for RuvLLM
 *
 * Automatically loads the correct native binary for the current platform.
 */
interface NativeRuvLLM {
    RuvLLMEngine: new (config?: NativeConfig) => NativeEngine;
    SimdOperations: new () => NativeSimdOps;
    version: () => string;
    hasSimdSupport: () => boolean;
}
interface NativeConfig {
    embedding_dim?: number;
    router_hidden_dim?: number;
    hnsw_m?: number;
    hnsw_ef_construction?: number;
    hnsw_ef_search?: number;
    learning_enabled?: boolean;
    quality_threshold?: number;
    ewc_lambda?: number;
}
interface NativeEngine {
    query(text: string, config?: NativeGenConfig): NativeQueryResponse;
    generate(prompt: string, config?: NativeGenConfig): string;
    route(text: string): NativeRoutingDecision;
    searchMemory(text: string, k?: number): NativeMemoryResult[];
    addMemory(content: string, metadata?: string): number;
    feedback(requestId: string, rating: number, correction?: string): boolean;
    stats(): NativeStats;
    forceLearn(): string;
    embed(text: string): number[];
    similarity(text1: string, text2: string): number;
    hasSimd(): boolean;
    simdCapabilities(): string[];
}
interface NativeGenConfig {
    max_tokens?: number;
    temperature?: number;
    top_p?: number;
    top_k?: number;
    repetition_penalty?: number;
}
interface NativeQueryResponse {
    text: string;
    confidence: number;
    model: string;
    context_size: number;
    latency_ms: number;
    request_id: string;
}
interface NativeRoutingDecision {
    model: string;
    context_size: number;
    temperature: number;
    top_p: number;
    confidence: number;
}
interface NativeMemoryResult {
    id: number;
    score: number;
    content: string;
    metadata: string;
}
interface NativeStats {
    total_queries: number;
    memory_nodes: number;
    patterns_learned: number;
    avg_latency_ms: number;
    cache_hit_rate: number;
    router_accuracy: number;
}
interface NativeSimdOps {
    dotProduct(a: number[], b: number[]): number;
    cosineSimilarity(a: number[], b: number[]): number;
    l2Distance(a: number[], b: number[]): number;
    matvec(matrix: number[][], vector: number[]): number[];
    softmax(input: number[]): number[];
}
export declare function getNativeModule(): NativeRuvLLM | null;
export declare function version(): string;
export declare function hasSimdSupport(): boolean;
export type { NativeRuvLLM, NativeConfig, NativeEngine, NativeGenConfig, NativeQueryResponse, NativeRoutingDecision, NativeMemoryResult, NativeStats, NativeSimdOps, };
//# sourceMappingURL=native.d.ts.map