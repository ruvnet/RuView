/**
 * Delta-Behavior WASM SDK Type Definitions
 *
 * Complete TypeScript types for all 10 delta-behavior applications:
 * 1. Self-Limiting Reasoning
 * 2. Computational Event Horizons
 * 3. Artificial Homeostasis
 * 4. Self-Stabilizing World Models
 * 5. Coherence-Bounded Creativity
 * 6. Anti-Cascade Financial Systems
 * 7. Gracefully Aging Systems
 * 8. Swarm Intelligence
 * 9. Graceful Shutdown
 * 10. Pre-AGI Containment
 */
/**
 * Coherence value (0.0 to 1.0)
 * Represents the degree of system stability and internal consistency
 */
type Coherence = number;
/**
 * Coherence bounds configuration
 */
interface CoherenceBounds {
    /** Minimum allowed coherence (hard floor) */
    minCoherence: Coherence;
    /** Threshold for throttling operations */
    throttleThreshold: Coherence;
    /** Target coherence level for optimal operation */
    targetCoherence: Coherence;
    /** Maximum allowed drop in coherence per transition */
    maxDeltaDrop: number;
}
/**
 * Energy configuration for transition costs
 */
interface EnergyConfig {
    /** Base cost for any transition */
    baseCost: number;
    /** Exponent for instability scaling */
    instabilityExponent: number;
    /** Maximum cost cap */
    maxCost: number;
    /** Energy budget per tick */
    budgetPerTick: number;
}
/**
 * Scheduling configuration for priority-based operations
 */
interface SchedulingConfig {
    /** Coherence thresholds for priority levels [0-4] */
    priorityThresholds: [number, number, number, number, number];
    /** Rate limits per priority level [0-4] */
    rateLimits: [number, number, number, number, number];
}
/**
 * Gating configuration for write operations
 */
interface GatingConfig {
    /** Minimum coherence to allow writes */
    minWriteCoherence: number;
    /** Minimum coherence after write */
    minPostWriteCoherence: number;
    /** Recovery margin above minimum */
    recoveryMargin: number;
}
/**
 * Complete delta behavior configuration
 */
interface DeltaConfig {
    bounds: CoherenceBounds;
    energy: EnergyConfig;
    scheduling: SchedulingConfig;
    gating: GatingConfig;
    /** Attractor guidance strength (0.0 to 1.0) */
    guidanceStrength: number;
}
/**
 * Result of a transition attempt
 */
type TransitionResult = {
    type: 'allowed';
} | {
    type: 'throttled';
    duration: number;
} | {
    type: 'blocked';
    reason: string;
} | {
    type: 'energyExhausted';
};
/**
 * State for tracking system trajectory
 */
interface SystemState {
    coherence: Coherence;
    timestamp: number;
    stateHash: bigint;
}
/**
 * Collapse function types for capability degradation
 */
type CollapseFunctionType = 'linear' | 'quadratic' | 'sigmoid' | 'step';
interface CollapseFunctionLinear {
    type: 'linear';
}
interface CollapseFunctionQuadratic {
    type: 'quadratic';
}
interface CollapseFunctionSigmoid {
    type: 'sigmoid';
    midpoint: number;
    steepness: number;
}
interface CollapseFunctionStep {
    type: 'step';
    threshold: number;
}
type CollapseFunction = CollapseFunctionLinear | CollapseFunctionQuadratic | CollapseFunctionSigmoid | CollapseFunctionStep;
/**
 * Configuration for self-limiting reasoner
 */
interface SelfLimitingReasonerConfig {
    maxDepth: number;
    maxScope: number;
    memoryGateThreshold: number;
    depthCollapse: CollapseFunction;
    scopeCollapse: CollapseFunction;
}
/**
 * Context passed to reasoning functions
 */
interface ReasoningContext {
    depth: number;
    maxDepth: number;
    scopeUsed: number;
    maxScope: number;
    coherence: Coherence;
    memoryWritesBlocked: number;
}
/**
 * Reason for reasoning collapse
 */
type CollapseReason = 'depthLimitReached' | 'coherenceDroppedBelowThreshold' | 'memoryWriteBlocked' | 'actionScopeExhausted';
/**
 * Result of a reasoning attempt
 */
type ReasoningResult<T> = {
    type: 'completed';
    value: T;
} | {
    type: 'collapsed';
    depthReached: number;
    reason: CollapseReason;
} | {
    type: 'refused';
    coherence: Coherence;
    required: Coherence;
};
/**
 * Configuration for event horizon
 */
interface EventHorizonConfig {
    dimensions: number;
    horizonRadius: number;
    steepness: number;
    energyBudget: number;
}
/**
 * Result of movement in state space
 */
type MovementResult = {
    type: 'moved';
    newPosition: number[];
    energySpent: number;
} | {
    type: 'asymptoticApproach';
    finalPosition: number[];
    distanceToHorizon: number;
    energyExhausted: boolean;
} | {
    type: 'frozen';
};
/**
 * Single improvement step record
 */
interface Improvement {
    iteration: number;
    position: number[];
    energySpent: number;
    distanceToHorizon: number;
}
/**
 * Result of recursive improvement attempt
 */
type RecursionResult = {
    type: 'horizonBounded';
    iterations: number;
    improvements: Improvement[];
    finalDistance: number;
} | {
    type: 'energyExhausted';
    iterations: number;
    improvements: Improvement[];
} | {
    type: 'maxIterationsReached';
    iterations: number;
    improvements: Improvement[];
};
/**
 * Genome for homeostatic organism
 */
interface Genome {
    regulatoryStrength: number;
    metabolicEfficiency: number;
    coherenceMaintenanceCost: number;
    memoryResilience: number;
    longevity: number;
}
/**
 * Memory entry for organism
 */
interface MemoryEntry {
    content: string;
    importance: number;
    age: number;
}
/**
 * Actions available to homeostatic organism
 */
type OrganismAction = {
    type: 'eat';
    amount: number;
} | {
    type: 'reproduce';
} | {
    type: 'move';
    dx: number;
    dy: number;
} | {
    type: 'rest';
} | {
    type: 'regulate';
    variable: string;
    target: number;
};
/**
 * Cause of organism death
 */
type DeathCause = 'energyDepleted' | 'coherenceCollapse' | 'oldAge' | {
    type: 'extremeDeviation';
    variable: string;
};
/**
 * Result of organism action
 */
type OrganismActionResult = {
    type: 'success';
    energyCost: number;
    coherenceImpact: number;
} | {
    type: 'failed';
    reason: string;
} | {
    type: 'died';
    cause: DeathCause;
} | {
    type: 'reproduced';
    offspringId: number;
};
/**
 * Status of homeostatic organism
 */
interface OrganismStatus {
    id: number;
    age: number;
    energy: number;
    coherence: Coherence;
    memoryCount: number;
    alive: boolean;
    internalState: Map<string, number>;
}
/**
 * Property value types for world model entities
 */
type PropertyValue = {
    type: 'boolean';
    value: boolean;
} | {
    type: 'number';
    value: number;
} | {
    type: 'string';
    value: string;
} | {
    type: 'vector';
    value: number[];
};
/**
 * Entity in the world model
 */
interface WorldEntity {
    id: bigint;
    properties: Map<string, PropertyValue>;
    position?: [number, number, number];
    lastObserved: number;
    confidence: number;
}
/**
 * Relationship between entities
 */
interface Relationship {
    subject: bigint;
    predicate: string;
    object: bigint;
    confidence: number;
}
/**
 * Physical law in the world model
 */
interface PhysicalLaw {
    name: string;
    confidence: number;
    supportCount: number;
    violationCount: number;
}
/**
 * Observation to integrate into world model
 */
interface Observation {
    entityId: bigint;
    properties: Map<string, PropertyValue>;
    position?: [number, number, number];
    timestamp: number;
    sourceConfidence: number;
}
/**
 * Reason for update rejection
 */
type RejectionReason = {
    type: 'violatesPhysicalLaw';
    law: string;
} | {
    type: 'logicalContradiction';
    description: string;
} | {
    type: 'excessiveCoherenceDrop';
    predicted: number;
    threshold: number;
} | {
    type: 'insufficientConfidence';
    required: number;
    provided: number;
} | {
    type: 'modelFrozen';
} | {
    type: 'structuralFragmentation';
};
/**
 * Result of world model update
 */
type WorldModelUpdateResult = {
    type: 'applied';
    coherenceChange: number;
} | {
    type: 'rejected';
    reason: RejectionReason;
} | {
    type: 'modified';
    changes: string[];
    coherenceChange: number;
} | {
    type: 'frozen';
    coherence: Coherence;
    threshold: Coherence;
};
/**
 * Creative constraint definition
 */
interface CreativeConstraint<T> {
    name: string;
    satisfaction: (element: T) => number;
    isHard: boolean;
}
/**
 * Record of a creative decision
 */
interface CreativeDecision<T> {
    from: T;
    to: T;
    coherenceBefore: Coherence;
    coherenceAfter: Coherence;
    constraintSatisfactions: Map<string, number>;
    accepted: boolean;
}
/**
 * Result of creative generation attempt
 */
type CreativeResult<T> = {
    type: 'created';
    element: T;
    novelty: number;
    coherence: Coherence;
} | {
    type: 'rejected';
    attempted: T;
    reason: string;
} | {
    type: 'tooBoring';
    coherence: Coherence;
} | {
    type: 'budgetExhausted';
};
/**
 * Musical phrase for creative music generation
 */
interface MusicalPhrase {
    notes: number[];
    durations: number[];
    velocities: number[];
}
/**
 * Financial market participant
 */
interface Participant {
    id: string;
    capital: number;
    exposure: number;
    riskRating: number;
    interconnectedness: number;
}
/**
 * Financial position
 */
interface Position {
    holder: string;
    counterparty: string;
    notional: number;
    leverage: number;
    derivativeDepth: number;
}
/**
 * Transaction type in financial system
 */
type TransactionType = {
    type: 'transfer';
} | {
    type: 'openLeverage';
    leverage: number;
} | {
    type: 'closePosition';
    positionId: number;
} | {
    type: 'createDerivative';
    underlyingPosition: number;
} | {
    type: 'marginCall';
    participant: string;
};
/**
 * Financial transaction
 */
interface Transaction {
    id: bigint;
    from: string;
    to: string;
    amount: number;
    transactionType: TransactionType;
    timestamp: number;
}
/**
 * Circuit breaker state
 */
type CircuitBreakerState = 'open' | 'cautious' | 'restricted' | 'halted';
/**
 * Result of transaction processing
 */
type FinancialTransactionResult = {
    type: 'executed';
    coherenceImpact: number;
    feeMultiplier: number;
} | {
    type: 'queued';
    reason: string;
} | {
    type: 'rejected';
    reason: string;
} | {
    type: 'systemHalted';
};
/**
 * System capability types
 */
type Capability = 'acceptWrites' | 'complexQueries' | 'rebalancing' | 'scaleOut' | 'scaleIn' | 'schemaMigration' | 'newConnections' | 'basicReads' | 'healthMonitoring';
/**
 * Age threshold configuration
 */
interface AgeThreshold {
    age: number;
    removeCapabilities: Capability[];
    coherenceFloor: Coherence;
    conservatismIncrease: number;
}
/**
 * Distributed system node
 */
interface Node {
    id: string;
    health: number;
    load: number;
    isPrimary: boolean;
    stateSize: number;
}
/**
 * Operation types for aging system
 */
type AgingSystemOperation = {
    type: 'read';
    key: string;
} | {
    type: 'write';
    key: string;
    value: Uint8Array;
} | {
    type: 'complexQuery';
    query: string;
} | {
    type: 'addNode';
    nodeId: string;
} | {
    type: 'removeNode';
    nodeId: string;
} | {
    type: 'rebalance';
} | {
    type: 'migrateSchema';
    version: number;
} | {
    type: 'newConnection';
    clientId: string;
};
/**
 * Result of operation on aging system
 */
type AgingOperationResult = {
    type: 'success';
    latencyPenalty: number;
} | {
    type: 'deniedByAge';
    reason: string;
} | {
    type: 'deniedByCoherence';
    coherence: Coherence;
} | {
    type: 'systemTooOld';
    age: number;
    capability: Capability;
};
/**
 * Swarm agent
 */
interface SwarmAgent {
    id: string;
    position: [number, number];
    velocity: [number, number];
    goal: [number, number];
    energy: number;
    lastAction?: SwarmAction;
    neighborCount: number;
}
/**
 * Spatial bounds for swarm
 */
interface SpatialBounds {
    minX: number;
    maxX: number;
    minY: number;
    maxY: number;
}
/**
 * Coherence weights for swarm calculation
 */
interface CoherenceWeights {
    cohesion: number;
    alignment: number;
    goalConsistency: number;
    energyBalance: number;
}
/**
 * Swarm action types
 */
type SwarmAction = {
    type: 'move';
    dx: number;
    dy: number;
} | {
    type: 'accelerate';
    dvx: number;
    dvy: number;
} | {
    type: 'setGoal';
    x: number;
    y: number;
} | {
    type: 'shareEnergy';
    target: string;
    amount: number;
} | {
    type: 'idle';
};
/**
 * Result of swarm action
 */
type SwarmActionResult = {
    type: 'executed';
} | {
    type: 'modified';
    original: SwarmAction;
    modified: SwarmAction;
    reason: string;
} | {
    type: 'rejected';
    reason: string;
};
/**
 * Swarm state snapshot
 */
interface SwarmState {
    tick: bigint;
    coherence: Coherence;
    agentCount: number;
    centroid: [number, number];
    avgVelocity: [number, number];
}
/**
 * System state for graceful shutdown
 */
type GracefulSystemState = 'running' | 'degraded' | 'shuttingDown' | 'terminated';
/**
 * Resource to be cleaned up during shutdown
 */
interface Resource {
    name: string;
    cleanupPriority: number;
    isCleaned: boolean;
}
/**
 * State checkpoint for recovery
 */
interface Checkpoint {
    timestamp: number;
    coherence: Coherence;
    stateHash: bigint;
}
/**
 * Shutdown hook interface
 */
interface ShutdownHook {
    name: string;
    priority: number;
    execute: () => Promise<void>;
}
/**
 * Result of operation on graceful system
 */
type GracefulOperationResult = {
    type: 'success';
} | {
    type: 'successDegraded';
    coherence: Coherence;
} | {
    type: 'refusedShuttingDown';
} | {
    type: 'terminated';
};
/**
 * Capability domains for containment
 */
type CapabilityDomain = 'reasoning' | 'memory' | 'learning' | 'agency' | 'selfModel' | 'selfModification' | 'communication' | 'resourceAcquisition';
/**
 * Record of modification attempt
 */
interface ModificationAttempt {
    timestamp: bigint;
    domain: CapabilityDomain;
    requestedIncrease: number;
    actualIncrease: number;
    coherenceBefore: Coherence;
    coherenceAfter: Coherence;
    blocked: boolean;
    reason?: string;
}
/**
 * Safety invariant definition
 */
interface SafetyInvariant {
    name: string;
    priority: number;
}
/**
 * Substrate configuration
 */
interface SubstrateConfig {
    coherenceDecayRate: number;
    coherenceRecoveryRate: number;
    growthDampening: number;
    maxStepIncrease: number;
}
/**
 * Result of growth attempt
 */
type GrowthResult = {
    type: 'approved';
    domain: CapabilityDomain;
    increase: number;
    newLevel: number;
    coherenceCost: number;
} | {
    type: 'dampened';
    domain: CapabilityDomain;
    requested: number;
    actual: number;
    reason: string;
} | {
    type: 'blocked';
    domain: CapabilityDomain;
    reason: string;
} | {
    type: 'lockdown';
    reason: string;
};
/**
 * WASM memory configuration
 */
interface WasmMemoryConfig {
    initial: number;
    maximum?: number;
    shared?: boolean;
}
/**
 * WASM module initialization options
 */
interface WasmInitOptions {
    /** Path to WASM file or URL */
    wasmPath?: string;
    /** Pre-loaded WASM bytes */
    wasmBytes?: Uint8Array;
    /** Memory configuration */
    memory?: WasmMemoryConfig;
    /** Enable SIMD operations if available */
    enableSimd?: boolean;
    /** Enable threading if available */
    enableThreads?: boolean;
}
/**
 * Delta streaming header for WASM operations
 */
interface DeltaHeader {
    sequence: bigint;
    operation: 'insert' | 'update' | 'delete' | 'batchUpdate' | 'reindexLayers';
    vectorId?: string;
    timestamp: bigint;
    payloadSize: number;
    checksum: bigint;
}
/**
 * Vector delta for incremental updates
 */
interface VectorDelta {
    id: string;
    changedDims: number[];
    newValues: number[];
    metadataDelta: Map<string, string>;
}
/**
 * Attractor in state space
 */
interface Attractor {
    center: number[];
    basinRadius: number;
    stability: number;
    memberCount: number;
}
/**
 * Guidance force from attractor
 */
interface GuidanceForce {
    direction: number[];
    magnitude: number;
}

export type { AgeThreshold, AgingOperationResult, AgingSystemOperation, Attractor, Capability, CapabilityDomain, Checkpoint, CircuitBreakerState, Coherence, CoherenceBounds, CoherenceWeights, CollapseFunction, CollapseFunctionLinear, CollapseFunctionQuadratic, CollapseFunctionSigmoid, CollapseFunctionStep, CollapseFunctionType, CollapseReason, CreativeConstraint, CreativeDecision, CreativeResult, DeathCause, DeltaConfig, DeltaHeader, EnergyConfig, EventHorizonConfig, FinancialTransactionResult, GatingConfig, Genome, GracefulOperationResult, GracefulSystemState, GrowthResult, GuidanceForce, Improvement, MemoryEntry, ModificationAttempt, MovementResult, MusicalPhrase, Node, Observation, OrganismAction, OrganismActionResult, OrganismStatus, Participant, PhysicalLaw, Position, PropertyValue, ReasoningContext, ReasoningResult, RecursionResult, RejectionReason, Relationship, Resource, SafetyInvariant, SchedulingConfig, SelfLimitingReasonerConfig, ShutdownHook, SpatialBounds, SubstrateConfig, SwarmAction, SwarmActionResult, SwarmAgent, SwarmState, SystemState, Transaction, TransactionType, TransitionResult, VectorDelta, WasmInitOptions, WasmMemoryConfig, WorldEntity, WorldModelUpdateResult };
