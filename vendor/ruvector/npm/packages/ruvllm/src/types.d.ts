/**
 * RuvLLM Type Definitions
 */
/**
 * Configuration for RuvLLM engine
 */
export interface RuvLLMConfig {
    /** Embedding dimension (default: 768) */
    embeddingDim?: number;
    /** Router hidden dimension (default: 128) */
    routerHiddenDim?: number;
    /** HNSW M parameter (default: 16) */
    hnswM?: number;
    /** HNSW ef_construction (default: 100) */
    hnswEfConstruction?: number;
    /** HNSW ef_search (default: 64) */
    hnswEfSearch?: number;
    /** Enable learning (default: true) */
    learningEnabled?: boolean;
    /** Quality threshold for learning (default: 0.7) */
    qualityThreshold?: number;
    /** EWC lambda (default: 2000) */
    ewcLambda?: number;
}
/**
 * Generation configuration
 */
export interface GenerationConfig {
    /** Maximum tokens to generate */
    maxTokens?: number;
    /** Temperature for sampling (0.0 - 2.0) */
    temperature?: number;
    /** Top-p nucleus sampling (0.0 - 1.0) */
    topP?: number;
    /** Top-k sampling */
    topK?: number;
    /** Repetition penalty */
    repetitionPenalty?: number;
}
/**
 * Query response from the LLM
 */
export interface QueryResponse {
    /** Generated text */
    text: string;
    /** Confidence score (0.0 - 1.0) */
    confidence: number;
    /** Selected model */
    model: string;
    /** Context size used */
    contextSize: number;
    /** Latency in milliseconds */
    latencyMs: number;
    /** Request ID for feedback */
    requestId: string;
}
/**
 * Routing decision
 */
export interface RoutingDecision {
    /** Selected model size */
    model: ModelSize;
    /** Recommended context size */
    contextSize: number;
    /** Temperature */
    temperature: number;
    /** Top-p */
    topP: number;
    /** Confidence */
    confidence: number;
}
/**
 * Memory search result
 */
export interface MemoryResult {
    /** Node ID */
    id: number;
    /** Similarity score */
    score: number;
    /** Content text */
    content: string;
    /** Metadata */
    metadata: Record<string, unknown>;
}
/**
 * Engine statistics
 */
export interface RuvLLMStats {
    /** Total queries processed */
    totalQueries: number;
    /** Memory nodes stored */
    memoryNodes: number;
    /** Patterns learned */
    patternsLearned: number;
    /** Average latency in ms */
    avgLatencyMs: number;
    /** Cache hit rate (0.0 - 1.0) */
    cacheHitRate: number;
    /** Router accuracy (0.0 - 1.0) */
    routerAccuracy: number;
}
/**
 * Model size options
 */
export type ModelSize = 'M350' | 'M700' | 'B1_2' | 'B2_6';
/**
 * Feedback for learning
 */
export interface Feedback {
    /** Request ID from query response */
    requestId: string;
    /** Rating 1-5 */
    rating: number;
    /** Optional correction text */
    correction?: string;
}
/**
 * Session for multi-turn conversations
 */
export interface Session {
    /** Session ID */
    id: string;
    /** Created timestamp */
    createdAt: Date;
    /** Messages in session */
    messageCount: number;
}
/**
 * SIMD capabilities
 */
export interface SimdCapabilities {
    /** Has any SIMD support */
    hasSimd: boolean;
    /** Available SIMD instructions */
    capabilities: string[];
}
/**
 * Embedding result
 */
export type Embedding = number[];
/**
 * Batch query request
 */
export interface BatchQueryRequest {
    /** Queries to process */
    queries: string[];
    /** Optional generation config */
    config?: GenerationConfig;
}
/**
 * Batch query response
 */
export interface BatchQueryResponse {
    /** Responses for each query */
    responses: QueryResponse[];
    /** Total processing time in ms */
    totalLatencyMs: number;
}
/**
 * SONA Configuration for adaptive learning
 */
export interface SonaConfig {
    /** Enable instant loop (real-time learning) */
    instantLoopEnabled?: boolean;
    /** Enable background loop (batch learning) */
    backgroundLoopEnabled?: boolean;
    /** Learning rate for LoRA adapters */
    loraLearningRate?: number;
    /** LoRA rank (lower = faster, higher = more capacity) */
    loraRank?: number;
    /** EWC lambda for memory protection */
    ewcLambda?: number;
    /** Max trajectory buffer size */
    maxTrajectorySize?: number;
    /** Pattern similarity threshold */
    patternThreshold?: number;
}
/**
 * Learning signal from user feedback
 */
export interface LearningSignal {
    /** Request ID */
    requestId: string;
    /** Quality score (0-1) */
    quality: number;
    /** Signal type */
    type: SignalType;
    /** Optional correction */
    correction?: string;
    /** Timestamp */
    timestamp: Date;
}
/**
 * Signal types for learning
 */
export type SignalType = 'positive' | 'negative' | 'correction' | 'implicit';
/**
 * Query trajectory for learning
 */
export interface QueryTrajectory {
    /** Trajectory ID */
    id: string;
    /** Steps in the trajectory */
    steps: TrajectoryStep[];
    /** Final outcome */
    outcome: TrajectoryOutcome;
    /** Total duration */
    durationMs: number;
}
/**
 * Single step in a trajectory
 */
export interface TrajectoryStep {
    /** Step type */
    type: 'query' | 'route' | 'generate' | 'memory' | 'feedback';
    /** Input data */
    input: string;
    /** Output data */
    output: string;
    /** Duration of this step */
    durationMs: number;
    /** Confidence at this step */
    confidence: number;
}
/**
 * Trajectory outcome
 */
export type TrajectoryOutcome = 'success' | 'partial' | 'failure' | 'unknown';
/**
 * Learned pattern from ReasoningBank
 */
export interface LearnedPattern {
    /** Pattern ID */
    id: string;
    /** Pattern type */
    type: PatternType;
    /** Pattern embedding */
    embedding: Embedding;
    /** Success rate (0-1) */
    successRate: number;
    /** Times used */
    useCount: number;
    /** Last used timestamp */
    lastUsed: Date;
}
/**
 * Types of learned patterns
 */
export type PatternType = 'query_response' | 'routing' | 'context_retrieval' | 'correction' | 'abstraction';
/**
 * LoRA adapter configuration
 */
export interface LoRAConfig {
    /** Adapter rank (4, 8, 16, 32) */
    rank: number;
    /** Alpha scaling factor */
    alpha: number;
    /** Dropout rate */
    dropout: number;
    /** Target modules to adapt */
    targetModules: string[];
}
/**
 * EWC (Elastic Weight Consolidation) stats
 */
export interface EwcStats {
    /** Number of tasks learned */
    tasksLearned: number;
    /** Fisher information computed */
    fisherComputed: boolean;
    /** Memory protection strength */
    protectionStrength: number;
    /** Estimated forgetting rate */
    forgettingRate: number;
}
/**
 * Extended session with conversation history
 */
export interface ConversationSession extends Session {
    /** Conversation messages */
    messages: ConversationMessage[];
    /** Session context (accumulated) */
    context: string[];
    /** Active memory IDs */
    activeMemoryIds: number[];
    /** Session metadata */
    metadata: Record<string, unknown>;
}
/**
 * Single message in conversation
 */
export interface ConversationMessage {
    /** Message role */
    role: 'user' | 'assistant' | 'system';
    /** Message content */
    content: string;
    /** Timestamp */
    timestamp: Date;
    /** Associated request ID (if assistant) */
    requestId?: string;
}
/**
 * Streaming response chunk
 */
export interface StreamChunk {
    /** Chunk text */
    text: string;
    /** Is final chunk */
    done: boolean;
    /** Token count so far */
    tokenCount: number;
    /** Cumulative latency */
    latencyMs: number;
}
/**
 * Stream options
 */
export interface StreamOptions extends GenerationConfig {
    /** Callback for each chunk */
    onChunk?: (chunk: StreamChunk) => void;
    /** Callback on completion */
    onComplete?: (response: QueryResponse) => void;
    /** Callback on error */
    onError?: (error: Error) => void;
}
/**
 * Memory compression result
 */
export interface CompressionResult {
    /** Nodes compressed */
    nodesCompressed: number;
    /** Nodes archived */
    nodesArchived: number;
    /** Concepts created */
    conceptsCreated: number;
    /** Memory saved (bytes) */
    memorySaved: number;
    /** Duration */
    durationMs: number;
}
/**
 * Archive query result
 */
export interface ArchiveResult {
    /** Archived node ID */
    id: number;
    /** Original content (if available) */
    content?: string;
    /** Concept it belongs to */
    conceptId?: string;
    /** Archive timestamp */
    archivedAt: Date;
}
/**
 * Attention weights for interpretability
 */
export interface AttentionWeights {
    /** Query-key attention scores */
    scores: number[][];
    /** Head index */
    headIndex: number;
    /** Layer index */
    layerIndex: number;
}
/**
 * Attention analysis result
 */
export interface AttentionAnalysis {
    /** Most attended tokens */
    topAttended: Array<{
        token: string;
        weight: number;
    }>;
    /** Attention entropy (uncertainty) */
    entropy: number;
    /** Focus score (0-1, higher = more focused) */
    focusScore: number;
}
/**
 * Federated learning configuration
 */
export interface FederatedConfig {
    /** Hidden dimension for embeddings */
    hiddenDim?: number;
    /** Embedding dimension */
    embeddingDim?: number;
    /** Micro-LoRA rank */
    microLoraRank?: number;
    /** Base LoRA rank */
    baseLoraRank?: number;
    /** Trajectory buffer capacity */
    trajectoryCapacity?: number;
    /** Pattern cluster count */
    patternClusters?: number;
    /** EWC lambda for regularization */
    ewcLambda?: number;
    /** Quality threshold for accepting trajectories */
    qualityThreshold?: number;
}
/**
 * Trajectory export for federation
 */
export interface TrajectoryExport {
    /** Query embedding */
    embedding: Embedding;
    /** Quality score */
    quality: number;
    /** Model route (if any) */
    route?: string;
    /** Context identifiers */
    context: string[];
    /** Timestamp */
    timestamp: number;
}
/**
 * Agent export statistics
 */
export interface AgentExportStats {
    /** Total trajectories processed */
    totalTrajectories: number;
    /** Average quality */
    avgQuality: number;
    /** Patterns learned locally */
    patternsLearned: number;
}
/**
 * Exported state from an ephemeral agent
 */
export interface AgentExport {
    /** Agent identifier */
    agentId: string;
    /** Exported trajectories */
    trajectories: TrajectoryExport[];
    /** Agent statistics */
    stats: AgentExportStats;
    /** Session duration in milliseconds */
    sessionDurationMs: number;
    /** Export timestamp */
    timestamp: number;
}
/**
 * Agent contribution record
 */
export interface AgentContribution {
    /** Number of trajectories contributed */
    trajectoryCount: number;
    /** Average quality of contributions */
    avgQuality: number;
    /** Contribution timestamp */
    timestamp: number;
    /** Session duration */
    sessionDurationMs: number;
}
/**
 * Result of aggregating an agent export
 */
export interface AggregationResult {
    /** Agent ID that was aggregated */
    agentId: string;
    /** Number of trajectories accepted */
    trajectoriesAccepted: number;
    /** Number of trajectories rejected (below quality threshold) */
    trajectoriesRejected: number;
    /** Whether consolidation was triggered */
    consolidated: boolean;
    /** Total number of contributing agents */
    totalAgents: number;
    /** Total trajectories in coordinator */
    totalTrajectories: number;
}
/**
 * Coordinator statistics
 */
export interface CoordinatorStats {
    /** Coordinator identifier */
    coordinatorId: string;
    /** Number of contributing agents */
    totalAgents: number;
    /** Total trajectories aggregated */
    totalTrajectories: number;
    /** Patterns learned */
    patternsLearned: number;
    /** Average quality across all contributions */
    avgQuality: number;
    /** Quality threshold */
    qualityThreshold: number;
}
/**
 * Federated learning topology
 */
export type FederatedTopology = 'star' | 'hierarchical' | 'peer-to-peer';
/**
 * Training configuration
 */
export interface TrainingConfig {
    /** Initial learning rate */
    learningRate?: number;
    /** Batch size */
    batchSize?: number;
    /** Number of epochs */
    epochs?: number;
    /** Learning rate scheduler */
    scheduler?: 'constant' | 'linear' | 'cosine' | 'warmup';
    /** Warmup steps (for warmup scheduler) */
    warmupSteps?: number;
    /** Weight decay */
    weightDecay?: number;
    /** Gradient clipping threshold */
    gradientClip?: number;
    /** Early stopping patience */
    earlyStoppingPatience?: number;
    /** Checkpoint interval (epochs) */
    checkpointInterval?: number;
    /** EWC lambda for continual learning */
    ewcLambda?: number;
    /** Validation split ratio */
    validationSplit?: number;
}
/**
 * Training metrics snapshot
 */
export interface TrainingMetricsSnapshot {
    /** Current epoch */
    epoch: number;
    /** Current step */
    step: number;
    /** Training loss */
    trainLoss: number;
    /** Validation loss */
    valLoss: number;
    /** Learning rate */
    learningRate: number;
    /** Gradient norm */
    gradNorm: number;
    /** Steps per second */
    stepsPerSecond: number;
    /** ETA in seconds */
    etaSeconds: number;
}
/**
 * Training result
 */
export interface TrainingResult {
    /** Total epochs completed */
    epochs: number;
    /** Total steps completed */
    steps: number;
    /** Final training loss */
    finalLoss: number;
    /** Best validation loss */
    bestValLoss: number;
    /** Training duration in ms */
    durationMs: number;
    /** Loss history */
    lossHistory: number[];
    /** Validation loss history */
    valLossHistory: number[];
    /** Early stopped */
    earlyStopped: boolean;
}
/**
 * Training checkpoint
 */
export interface TrainingCheckpoint {
    /** Epoch number */
    epoch: number;
    /** Step number */
    step: number;
    /** Training loss at checkpoint */
    loss: number;
    /** Model weights (serialized) */
    weights: string;
    /** Timestamp */
    timestamp: number;
}
/**
 * Export format options
 */
export type ExportFormat = 'safetensors' | 'json' | 'binary' | 'onnx';
/**
 * Model metadata for export
 */
export interface ModelMetadata {
    /** Model name */
    name: string;
    /** Model version */
    version: string;
    /** Architecture type */
    architecture: string;
    /** Training info */
    training?: {
        steps: number;
        loss: number;
        learningRate: number;
    };
    /** Custom metadata */
    custom?: Record<string, unknown>;
}
//# sourceMappingURL=types.d.ts.map