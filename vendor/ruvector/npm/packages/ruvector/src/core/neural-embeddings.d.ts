/**
 * Neural Embedding System - Frontier Embedding Intelligence
 *
 * Implements late-2025 research concepts treating embeddings as:
 * 1. CONTROL SIGNALS - Semantic drift detection, reflex triggers
 * 2. MEMORY PHYSICS - Forgetting curves, interference, consolidation
 * 3. PROGRAM STATE - Agent state management via geometry
 * 4. COORDINATION PRIMITIVES - Multi-agent swarm alignment
 * 5. SAFETY MONITORS - Coherence detection, misalignment alerts
 * 6. NEURAL SUBSTRATE - Synthetic nervous system layer
 *
 * Based on:
 * - TinyTE (EMNLP 2025): Embedding-layer steering
 * - DoRA (ICML 2024): Magnitude-direction decomposition
 * - S-LoRA/Punica: Multi-adapter serving patterns
 * - MMTEB: Multilingual embedding benchmarks
 */
export declare const NEURAL_CONSTANTS: {
    readonly MAX_DRIFT_EVENTS: 1000;
    readonly MAX_HISTORY_SIZE: 500;
    readonly DEFAULT_DRIFT_THRESHOLD: 0.15;
    readonly DEFAULT_DRIFT_WINDOW_MS: 60000;
    readonly DRIFT_CRITICAL_MULTIPLIER: 2;
    readonly VELOCITY_WINDOW_SIZE: 10;
    readonly MAX_MEMORIES: 10000;
    readonly MAX_CONTENT_LENGTH: 10000;
    readonly MAX_ID_LENGTH: 256;
    readonly DEFAULT_MEMORY_DECAY_RATE: 0.01;
    readonly DEFAULT_INTERFERENCE_THRESHOLD: 0.8;
    readonly DEFAULT_CONSOLIDATION_RATE: 0.1;
    readonly MEMORY_FORGET_THRESHOLD: 0.01;
    readonly CONSOLIDATION_SCORE_THRESHOLD: 0.5;
    readonly MEMORY_CLEANUP_PERCENT: 0.1;
    readonly RECALL_STRENGTH_BOOST: 0.1;
    readonly MAX_TIME_JUMP_MINUTES: 1440;
    readonly MAX_AGENTS: 1000;
    readonly MAX_SPECIALTY_LENGTH: 100;
    readonly AGENT_TIMEOUT_MS: 3600000;
    readonly DEFAULT_AGENT_ENERGY: 1;
    readonly TRAJECTORY_DAMPING: 0.1;
    readonly MAX_TRAJECTORY_STEPS: 100;
    readonly MAX_CLUSTER_AGENTS: 500;
    readonly DEFAULT_CLUSTER_THRESHOLD: 0.7;
    readonly DEFAULT_WINDOW_SIZE: 100;
    readonly MIN_CALIBRATION_OBSERVATIONS: 10;
    readonly STABILITY_WINDOW_SIZE: 10;
    readonly ALIGNMENT_WINDOW_SIZE: 50;
    readonly RECENT_OBSERVATIONS_SIZE: 20;
    readonly DRIFT_WARNING_THRESHOLD: 0.3;
    readonly STABILITY_WARNING_THRESHOLD: 0.5;
    readonly ALIGNMENT_WARNING_THRESHOLD: 0.6;
    readonly COHERENCE_WARNING_THRESHOLD: 0.5;
    readonly EPSILON: 1e-8;
    readonly ZERO_VECTOR_THRESHOLD: 1e-10;
    readonly DEFAULT_DIMENSION: 384;
    readonly DEFAULT_REFLEX_LATENCY_MS: 10;
};
export type LogLevel = 'debug' | 'info' | 'warn' | 'error';
export interface NeuralLogger {
    log(level: LogLevel, message: string, data?: Record<string, unknown>): void;
}
/** Default console logger */
export declare const defaultLogger: NeuralLogger;
/** Silent logger for suppressing output */
export declare const silentLogger: NeuralLogger;
export interface DriftEvent {
    readonly timestamp: number;
    readonly magnitude: number;
    readonly direction: Float32Array;
    readonly category: 'normal' | 'warning' | 'critical';
    readonly source?: string;
}
export interface NeuralMemoryEntry {
    readonly id: string;
    readonly embedding: Float32Array;
    readonly content: string;
    strength: number;
    lastAccess: number;
    accessCount: number;
    consolidationLevel: number;
    interference: number;
}
export interface AgentState {
    readonly id: string;
    position: Float32Array;
    velocity: Float32Array;
    attention: Float32Array;
    energy: number;
    mode: string;
    lastUpdate: number;
}
export interface CoherenceReport {
    readonly timestamp: number;
    readonly overallScore: number;
    readonly driftScore: number;
    readonly stabilityScore: number;
    readonly alignmentScore: number;
    readonly anomalies: ReadonlyArray<{
        readonly type: string;
        readonly severity: number;
        readonly description: string;
    }>;
}
export interface NeuralConfig {
    readonly dimension?: number;
    readonly driftThreshold?: number;
    readonly driftWindowMs?: number;
    readonly memoryDecayRate?: number;
    readonly interferenceThreshold?: number;
    readonly consolidationRate?: number;
    readonly reflexLatencyMs?: number;
    readonly logger?: NeuralLogger;
}
/**
 * Detects semantic drift and triggers reflexes based on embedding movement.
 * Instead of asking "what is similar", asks "how far did we move".
 */
export declare class SemanticDriftDetector {
    private baseline;
    private history;
    private driftEvents;
    private config;
    private logger;
    private reflexes;
    constructor(config?: NeuralConfig);
    /**
     * Set the baseline embedding (reference point)
     */
    setBaseline(embedding: number[] | Float32Array): void;
    /**
     * Observe a new embedding and detect drift
     */
    observe(embedding: number[] | Float32Array, source?: string): DriftEvent | null;
    /**
     * Calculate drift between two embeddings
     */
    private calculateDrift;
    /**
     * Register a reflex callback for drift events
     */
    registerReflex(name: string, callback: (event: DriftEvent) => void): void;
    /**
     * Trigger registered reflexes
     */
    private triggerReflexes;
    /**
     * Get recent drift velocity (rate of change)
     */
    getVelocity(): number;
    /**
     * Get drift statistics
     */
    getStats(): {
        currentDrift: number;
        velocity: number;
        criticalEvents: number;
        warningEvents: number;
        historySize: number;
    };
    /**
     * Reset baseline to current position
     */
    recenter(): void;
}
/**
 * Implements hippocampal-like memory dynamics in embedding space.
 * Memory strength decays, similar memories interfere, consolidation strengthens.
 */
export declare class MemoryPhysics {
    private memories;
    private config;
    private lastUpdate;
    private logger;
    constructor(config?: NeuralConfig);
    /**
     * Encode a new memory
     */
    encode(id: string, embedding: number[] | Float32Array, content: string): NeuralMemoryEntry;
    /**
     * Recall memories similar to a query (strengthens accessed memories)
     */
    recall(query: number[] | Float32Array, k?: number): NeuralMemoryEntry[];
    /**
     * Apply time-based decay to all memories
     */
    private applyDecay;
    /**
     * Consolidate memories (like sleep consolidation)
     * Strengthens frequently accessed, weakly interfered memories
     */
    consolidate(): {
        consolidated: number;
        forgotten: number;
    };
    /**
     * Get memory statistics
     */
    getStats(): {
        totalMemories: number;
        avgStrength: number;
        avgConsolidation: number;
        avgInterference: number;
    };
    private cosineSimilarity;
    /**
     * Force cleanup of weak memories when limit reached
     */
    private forceCleanup;
}
/**
 * Manages agent state as movement through embedding space.
 * Decisions become geometric - no explicit state machine.
 */
export declare class EmbeddingStateMachine {
    private agents;
    private modeRegions;
    private config;
    private logger;
    private lastCleanup;
    constructor(config?: NeuralConfig);
    /**
     * Create or update an agent
     */
    updateAgent(id: string, embedding: number[] | Float32Array): AgentState;
    /**
     * Remove stale agents that haven't been updated recently
     */
    private cleanupStaleAgents;
    /**
     * Manually remove an agent
     */
    removeAgent(id: string): boolean;
    /**
     * Define a mode region in embedding space
     */
    defineMode(name: string, centroid: number[] | Float32Array, radius?: number): void;
    /**
     * Determine which mode an agent is in based on position
     */
    private determineMode;
    /**
     * Get agent trajectory prediction
     */
    predictTrajectory(id: string, steps?: number): Float32Array[];
    /**
     * Apply attention to agent state
     */
    attendTo(agentId: string, focusEmbedding: number[] | Float32Array): void;
    /**
     * Get all agents in a specific mode
     */
    getAgentsInMode(mode: string): AgentState[];
    private euclideanDistance;
}
/**
 * Enables multi-agent coordination through shared embedding space.
 * Swarm behavior emerges from geometry, not protocol.
 */
export declare class SwarmCoordinator {
    private agents;
    private sharedContext;
    private config;
    private logger;
    constructor(config?: NeuralConfig);
    /**
     * Register an agent with the swarm
     */
    register(id: string, embedding: number[] | Float32Array, specialty?: string): void;
    /**
     * Update agent position (from their work/observations)
     */
    update(id: string, embedding: number[] | Float32Array): void;
    /**
     * Update shared context (centroid of all agents)
     */
    private updateSharedContext;
    /**
     * Get coordination signal for an agent (how to align with swarm)
     */
    getCoordinationSignal(id: string): Float32Array;
    /**
     * Find agents working on similar things (for collaboration)
     */
    findCollaborators(id: string, k?: number): Array<{
        id: string;
        similarity: number;
        specialty: string;
    }>;
    /**
     * Detect emergent clusters (specialization)
     */
    detectClusters(threshold?: number): Map<string, string[]>;
    /**
     * Get swarm coherence (how aligned are agents)
     */
    getCoherence(): number;
    private cosineSimilarity;
    /**
     * Remove an agent from the swarm
     */
    removeAgent(id: string): boolean;
}
/**
 * Monitors system coherence via embedding patterns.
 * Detects degradation, poisoning, misalignment before explicit failures.
 */
export declare class CoherenceMonitor {
    private history;
    private baselineDistribution;
    private config;
    private logger;
    constructor(config?: NeuralConfig & {
        windowSize?: number;
    });
    /**
     * Record an observation
     */
    observe(embedding: number[] | Float32Array, source?: string): void;
    /**
     * Establish baseline distribution
     */
    calibrate(): void;
    /**
     * Generate coherence report
     */
    report(): CoherenceReport;
    private calculateDriftScore;
    private calculateStabilityScore;
    private calculateAlignmentScore;
    private cosineSimilarity;
}
/**
 * Unified neural embedding substrate combining all components.
 * Acts like a synthetic nervous system with reflexes, memory, and coordination.
 */
export declare class NeuralSubstrate {
    readonly drift: SemanticDriftDetector;
    readonly memory: MemoryPhysics;
    readonly state: EmbeddingStateMachine;
    readonly swarm: SwarmCoordinator;
    readonly coherence: CoherenceMonitor;
    private config;
    private logger;
    private reflexLatency;
    constructor(config?: NeuralConfig);
    /**
     * Process an embedding through the entire substrate
     */
    process(embedding: number[] | Float32Array, options?: {
        agentId?: string;
        memoryId?: string;
        content?: string;
        source?: string;
    }): {
        drift: DriftEvent | null;
        memory: NeuralMemoryEntry | null;
        state: AgentState | null;
    };
    /**
     * Query the substrate
     */
    query(embedding: number[] | Float32Array, k?: number): {
        memories: NeuralMemoryEntry[];
        collaborators: Array<{
            id: string;
            similarity: number;
            specialty: string;
        }>;
        coherence: CoherenceReport;
    };
    /**
     * Get overall system health
     */
    health(): {
        driftStats: ReturnType<SemanticDriftDetector['getStats']>;
        memoryStats: ReturnType<MemoryPhysics['getStats']>;
        swarmCoherence: number;
        coherenceReport: CoherenceReport;
    };
    /**
     * Run consolidation (like "sleep")
     */
    consolidate(): {
        consolidated: number;
        forgotten: number;
    };
    /**
     * Calibrate coherence baseline
     */
    calibrate(): void;
}
export default NeuralSubstrate;
//# sourceMappingURL=neural-embeddings.d.ts.map