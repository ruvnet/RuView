/**
 * RuVector Hooks Storage Layer
 *
 * Supports PostgreSQL (preferred) with JSON fallback
 * Uses ruvector extension for vector operations and pgvector-compatible storage
 */
export interface QPattern {
    state: string;
    action: string;
    q_value: number;
    visits: number;
    last_update: number;
}
export interface MemoryEntry {
    id: string;
    memory_type: string;
    content: string;
    embedding: number[];
    metadata: Record<string, string>;
    timestamp: number;
}
export interface Trajectory {
    id: string;
    state: string;
    action: string;
    outcome: string;
    reward: number;
    timestamp: number;
}
export interface ErrorPattern {
    code: string;
    error_type: string;
    message: string;
    fixes: string[];
    occurrences: number;
}
export interface SwarmAgent {
    id: string;
    agent_type: string;
    capabilities: string[];
    success_rate: number;
    task_count: number;
    status: string;
}
export interface SwarmEdge {
    source: string;
    target: string;
    weight: number;
    coordination_count: number;
}
export interface IntelligenceStats {
    total_patterns: number;
    total_memories: number;
    total_trajectories: number;
    total_errors: number;
    session_count: number;
    last_session: number;
}
export interface StorageBackend {
    connect(): Promise<void>;
    disconnect(): Promise<void>;
    isConnected(): boolean;
    updateQ(state: string, action: string, reward: number): Promise<void>;
    getQ(state: string, action: string): Promise<number>;
    getBestAction(state: string, actions: string[]): Promise<{
        action: string;
        confidence: number;
    }>;
    remember(type: string, content: string, embedding: number[], metadata: Record<string, string>): Promise<string>;
    recall(queryEmbedding: number[], topK: number): Promise<MemoryEntry[]>;
    recordTrajectory(state: string, action: string, outcome: string, reward: number): Promise<string>;
    recordError(code: string, errorType: string, message: string): Promise<void>;
    getErrorFixes(code: string): Promise<ErrorPattern | null>;
    recordSequence(fromFile: string, toFile: string): Promise<void>;
    getNextFiles(file: string, limit: number): Promise<Array<{
        file: string;
        count: number;
    }>>;
    registerAgent(id: string, type: string, capabilities: string[]): Promise<void>;
    coordinateAgents(source: string, target: string, weight: number): Promise<void>;
    getSwarmStats(): Promise<{
        agents: number;
        edges: number;
        avgSuccess: number;
    }>;
    sessionStart(): Promise<void>;
    getStats(): Promise<IntelligenceStats>;
}
export declare class JsonStorage implements StorageBackend {
    private data;
    private alpha;
    constructor();
    private load;
    private save;
    private now;
    connect(): Promise<void>;
    disconnect(): Promise<void>;
    isConnected(): boolean;
    updateQ(state: string, action: string, reward: number): Promise<void>;
    getQ(state: string, action: string): Promise<number>;
    getBestAction(state: string, actions: string[]): Promise<{
        action: string;
        confidence: number;
    }>;
    remember(type: string, content: string, embedding: number[], metadata: Record<string, string>): Promise<string>;
    recall(queryEmbedding: number[], topK: number): Promise<MemoryEntry[]>;
    recordTrajectory(state: string, action: string, outcome: string, reward: number): Promise<string>;
    recordError(code: string, errorType: string, message: string): Promise<void>;
    getErrorFixes(code: string): Promise<ErrorPattern | null>;
    recordSequence(fromFile: string, toFile: string): Promise<void>;
    getNextFiles(file: string, limit: number): Promise<Array<{
        file: string;
        count: number;
    }>>;
    registerAgent(id: string, type: string, capabilities: string[]): Promise<void>;
    coordinateAgents(source: string, target: string, weight: number): Promise<void>;
    getSwarmStats(): Promise<{
        agents: number;
        edges: number;
        avgSuccess: number;
    }>;
    sessionStart(): Promise<void>;
    getStats(): Promise<IntelligenceStats>;
}
export declare class PostgresStorage implements StorageBackend {
    private pool;
    private connectionString;
    private connected;
    constructor(connectionString?: string);
    connect(): Promise<void>;
    disconnect(): Promise<void>;
    isConnected(): boolean;
    private query;
    private initSchema;
    updateQ(state: string, action: string, reward: number): Promise<void>;
    getQ(state: string, action: string): Promise<number>;
    getBestAction(state: string, actions: string[]): Promise<{
        action: string;
        confidence: number;
    }>;
    remember(type: string, content: string, embedding: number[], metadata: Record<string, string>): Promise<string>;
    recall(queryEmbedding: number[], topK: number): Promise<MemoryEntry[]>;
    recordTrajectory(state: string, action: string, outcome: string, reward: number): Promise<string>;
    recordError(code: string, errorType: string, message: string): Promise<void>;
    getErrorFixes(code: string): Promise<ErrorPattern | null>;
    recordSequence(fromFile: string, toFile: string): Promise<void>;
    getNextFiles(file: string, limit: number): Promise<Array<{
        file: string;
        count: number;
    }>>;
    registerAgent(id: string, type: string, capabilities: string[]): Promise<void>;
    coordinateAgents(source: string, target: string, weight: number): Promise<void>;
    getSwarmStats(): Promise<{
        agents: number;
        edges: number;
        avgSuccess: number;
    }>;
    sessionStart(): Promise<void>;
    getStats(): Promise<IntelligenceStats>;
}
export declare function createStorage(): Promise<StorageBackend>;
export declare function createStorageSync(): StorageBackend;
//# sourceMappingURL=storage.d.ts.map