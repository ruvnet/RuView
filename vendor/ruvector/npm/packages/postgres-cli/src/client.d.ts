/**
 * RuVector PostgreSQL Client
 * Comprehensive wrapper for PostgreSQL connections with RuVector extension
 *
 * Features:
 * - Connection pooling with configurable limits
 * - Automatic retry with exponential backoff
 * - Batch operations for bulk inserts
 * - SQL injection protection
 * - Input validation
 */
import pg from 'pg';
export interface PoolConfig {
    maxConnections?: number;
    idleTimeoutMs?: number;
    connectionTimeoutMs?: number;
    statementTimeoutMs?: number;
}
export interface RetryConfig {
    maxRetries?: number;
    baseDelayMs?: number;
    maxDelayMs?: number;
}
export interface RuVectorInfo {
    version: string;
    features: string[];
    simd_info?: string;
}
export interface VectorSearchResult {
    id: number | string;
    distance: number;
    metadata?: Record<string, unknown>;
    vector?: number[];
}
export interface AttentionResult {
    output: number[];
    weights?: number[][];
}
export interface GnnResult {
    embeddings: number[][];
    layer_output?: number[][];
}
export interface GraphNode {
    id: string;
    labels: string[];
    properties: Record<string, unknown>;
}
export interface GraphEdge {
    id: string;
    type: string;
    from: string;
    to: string;
    properties: Record<string, unknown>;
}
export interface TraversalResult {
    nodes: GraphNode[];
    edges: GraphEdge[];
    path?: string[];
}
export interface SparseInfo {
    dim: number;
    nnz: number;
    sparsity: number;
    norm: number;
}
export interface SparseResult {
    vector: string;
    nnz: number;
    originalNnz?: number;
    newNnz?: number;
}
export interface ScalarQuantizeResult {
    data: number[];
    scale: number;
    offset: number;
}
export interface Agent {
    name: string;
    agent_type: string;
    capabilities: string[];
    is_active: boolean;
    cost_model: {
        per_request: number;
        per_token?: number;
    };
    performance: {
        avg_latency_ms: number;
        quality_score: number;
        success_rate: number;
        total_requests: number;
    };
}
export interface AgentSummary {
    name: string;
    agent_type: string;
    capabilities: string[];
    cost_per_request: number;
    avg_latency_ms: number;
    quality_score: number;
    success_rate: number;
    total_requests: number;
    is_active: boolean;
}
export interface RoutingDecision {
    agent_name: string;
    confidence: number;
    estimated_cost: number;
    estimated_latency_ms: number;
    expected_quality: number;
    similarity_score: number;
    reasoning?: string;
    alternatives?: Array<{
        name: string;
        score?: number;
    }>;
}
export interface RoutingStats {
    total_agents: number;
    active_agents: number;
    total_requests: number;
    average_quality: number;
}
export interface LearningStats {
    trajectories: {
        total: number;
        with_feedback: number;
        avg_latency_us: number;
        avg_precision: number;
        avg_recall: number;
    };
    patterns: {
        total: number;
        total_samples: number;
        avg_confidence: number;
        total_usage: number;
    };
}
export interface GraphStats {
    name: string;
    node_count: number;
    edge_count: number;
    labels: string[];
    edge_types: string[];
}
export interface MemoryStats {
    index_memory_mb: number;
    vector_cache_mb: number;
    quantization_tables_mb: number;
    total_extension_mb: number;
}
export declare class RuVectorClient {
    private pool;
    private connectionString;
    private poolConfig;
    private retryConfig;
    constructor(connectionString: string, poolConfig?: PoolConfig, retryConfig?: RetryConfig);
    connect(): Promise<void>;
    disconnect(): Promise<void>;
    /**
     * Execute query with automatic retry on transient errors
     */
    private queryWithRetry;
    query<T extends pg.QueryResultRow = pg.QueryResultRow>(sql: string, params?: unknown[]): Promise<T[]>;
    execute(sql: string, params?: unknown[]): Promise<void>;
    /**
     * Execute multiple statements in a transaction
     */
    transaction<T>(fn: (client: pg.PoolClient) => Promise<T>): Promise<T>;
    getExtensionInfo(): Promise<RuVectorInfo>;
    installExtension(upgrade?: boolean): Promise<void>;
    getMemoryStats(): Promise<MemoryStats>;
    createVectorTable(name: string, dimensions: number, indexType?: 'hnsw' | 'ivfflat'): Promise<void>;
    insertVector(table: string, vector: number[], metadata?: Record<string, unknown>): Promise<number>;
    /**
     * Batch insert vectors (10-100x faster than individual inserts)
     */
    insertVectorsBatch(table: string, vectors: Array<{
        vector: number[];
        metadata?: Record<string, unknown>;
    }>, batchSize?: number): Promise<number[]>;
    searchVectors(table: string, query: number[], topK?: number, metric?: 'cosine' | 'l2' | 'ip'): Promise<VectorSearchResult[]>;
    /**
     * Compute cosine distance using array-based function (available in current SQL)
     */
    cosineDistanceArr(a: number[], b: number[]): Promise<number>;
    /**
     * Compute L2 distance using array-based function (available in current SQL)
     */
    l2DistanceArr(a: number[], b: number[]): Promise<number>;
    /**
     * Compute inner product using array-based function (available in current SQL)
     */
    innerProductArr(a: number[], b: number[]): Promise<number>;
    /**
     * Normalize a vector using array-based function (available in current SQL)
     */
    vectorNormalize(v: number[]): Promise<number[]>;
    createSparseVector(indices: number[], values: number[], dim: number): Promise<string>;
    sparseDistance(a: string, b: string, metric: 'dot' | 'cosine' | 'euclidean' | 'manhattan'): Promise<number>;
    sparseBM25(query: string, doc: string, docLen: number, avgDocLen: number, k1?: number, b?: number): Promise<number>;
    sparseTopK(sparse: string, k: number): Promise<SparseResult>;
    sparsePrune(sparse: string, threshold: number): Promise<SparseResult>;
    denseToSparse(dense: number[]): Promise<SparseResult>;
    sparseToDense(sparse: string): Promise<number[]>;
    sparseInfo(sparse: string): Promise<SparseInfo>;
    poincareDistance(a: number[], b: number[], curvature?: number): Promise<number>;
    lorentzDistance(a: number[], b: number[], curvature?: number): Promise<number>;
    mobiusAdd(a: number[], b: number[], curvature?: number): Promise<number[]>;
    expMap(base: number[], tangent: number[], curvature?: number): Promise<number[]>;
    logMap(base: number[], target: number[], curvature?: number): Promise<number[]>;
    poincareToLorentz(poincare: number[], curvature?: number): Promise<number[]>;
    lorentzToPoincare(lorentz: number[], curvature?: number): Promise<number[]>;
    minkowskiDot(a: number[], b: number[]): Promise<number>;
    binaryQuantize(vector: number[]): Promise<number[]>;
    scalarQuantize(vector: number[]): Promise<ScalarQuantizeResult>;
    quantizationStats(): Promise<MemoryStats>;
    computeAttention(query: number[], keys: number[][], values: number[][], _type?: 'scaled_dot' | 'multi_head' | 'flash'): Promise<AttentionResult>;
    listAttentionTypes(): Promise<string[]>;
    createGnnLayer(name: string, type: 'gcn' | 'graphsage' | 'gat' | 'gin', inputDim: number, outputDim: number): Promise<void>;
    gnnForward(layerType: 'gcn' | 'sage', features: number[][], src: number[], dst: number[], outDim: number): Promise<number[][]>;
    createGraph(name: string): Promise<boolean>;
    cypherQuery(graphName: string, query: string, params?: Record<string, unknown>): Promise<unknown[]>;
    addNode(graphName: string, labels: string[], properties: Record<string, unknown>): Promise<number>;
    addEdge(graphName: string, sourceId: number, targetId: number, edgeType: string, properties: Record<string, unknown>): Promise<number>;
    shortestPath(graphName: string, startId: number, endId: number, maxHops: number): Promise<{
        nodes: number[];
        edges: number[];
        length: number;
        cost: number;
    }>;
    graphStats(graphName: string): Promise<GraphStats>;
    listGraphs(): Promise<string[]>;
    deleteGraph(graphName: string): Promise<boolean>;
    registerAgent(name: string, agentType: string, capabilities: string[], costPerRequest: number, avgLatencyMs: number, qualityScore: number): Promise<boolean>;
    registerAgentFull(config: Record<string, unknown>): Promise<boolean>;
    updateAgentMetrics(name: string, latencyMs: number, success: boolean, quality?: number): Promise<boolean>;
    removeAgent(name: string): Promise<boolean>;
    setAgentActive(name: string, isActive: boolean): Promise<boolean>;
    route(embedding: number[], optimizeFor?: string, constraints?: Record<string, unknown>): Promise<RoutingDecision>;
    listAgents(): Promise<AgentSummary[]>;
    getAgent(name: string): Promise<Agent>;
    findAgentsByCapability(capability: string, limit?: number): Promise<AgentSummary[]>;
    routingStats(): Promise<RoutingStats>;
    clearAgents(): Promise<boolean>;
    enableLearning(tableName: string, config?: Record<string, unknown>): Promise<string>;
    recordFeedback(tableName: string, queryVector: number[], relevantIds: number[], irrelevantIds: number[]): Promise<string>;
    learningStats(tableName: string): Promise<LearningStats>;
    autoTune(tableName: string, optimizeFor?: string, sampleQueries?: number[][]): Promise<Record<string, unknown>>;
    extractPatterns(tableName: string, numClusters?: number): Promise<string>;
    getSearchParams(tableName: string, queryVector: number[]): Promise<{
        ef_search: number;
        probes: number;
        confidence: number;
    }>;
    clearLearning(tableName: string): Promise<string>;
    trainFromTrajectories(data: Record<string, unknown>[], epochs?: number): Promise<{
        loss: number;
        accuracy: number;
    }>;
    predict(input: number[]): Promise<number[]>;
    runBenchmark(type: 'vector' | 'attention' | 'gnn' | 'all', size: number, dimensions: number): Promise<Record<string, unknown>>;
}
export default RuVectorClient;
//# sourceMappingURL=client.d.ts.map