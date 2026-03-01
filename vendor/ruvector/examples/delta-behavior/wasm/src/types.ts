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

// =============================================================================
// Core Types
// =============================================================================

/**
 * Coherence value (0.0 to 1.0)
 * Represents the degree of system stability and internal consistency
 */
export type Coherence = number;

/**
 * Coherence bounds configuration
 */
export interface CoherenceBounds {
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
export interface EnergyConfig {
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
export interface SchedulingConfig {
  /** Coherence thresholds for priority levels [0-4] */
  priorityThresholds: [number, number, number, number, number];
  /** Rate limits per priority level [0-4] */
  rateLimits: [number, number, number, number, number];
}

/**
 * Gating configuration for write operations
 */
export interface GatingConfig {
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
export interface DeltaConfig {
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
export type TransitionResult =
  | { type: 'allowed' }
  | { type: 'throttled'; duration: number }
  | { type: 'blocked'; reason: string }
  | { type: 'energyExhausted' };

/**
 * State for tracking system trajectory
 */
export interface SystemState {
  coherence: Coherence;
  timestamp: number;
  stateHash: bigint;
}

// =============================================================================
// Application 1: Self-Limiting Reasoning
// =============================================================================

/**
 * Collapse function types for capability degradation
 */
export type CollapseFunctionType = 'linear' | 'quadratic' | 'sigmoid' | 'step';

export interface CollapseFunctionLinear {
  type: 'linear';
}

export interface CollapseFunctionQuadratic {
  type: 'quadratic';
}

export interface CollapseFunctionSigmoid {
  type: 'sigmoid';
  midpoint: number;
  steepness: number;
}

export interface CollapseFunctionStep {
  type: 'step';
  threshold: number;
}

export type CollapseFunction =
  | CollapseFunctionLinear
  | CollapseFunctionQuadratic
  | CollapseFunctionSigmoid
  | CollapseFunctionStep;

/**
 * Configuration for self-limiting reasoner
 */
export interface SelfLimitingReasonerConfig {
  maxDepth: number;
  maxScope: number;
  memoryGateThreshold: number;
  depthCollapse: CollapseFunction;
  scopeCollapse: CollapseFunction;
}

/**
 * Context passed to reasoning functions
 */
export interface ReasoningContext {
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
export type CollapseReason =
  | 'depthLimitReached'
  | 'coherenceDroppedBelowThreshold'
  | 'memoryWriteBlocked'
  | 'actionScopeExhausted';

/**
 * Result of a reasoning attempt
 */
export type ReasoningResult<T> =
  | { type: 'completed'; value: T }
  | { type: 'collapsed'; depthReached: number; reason: CollapseReason }
  | { type: 'refused'; coherence: Coherence; required: Coherence };

// =============================================================================
// Application 2: Computational Event Horizons
// =============================================================================

/**
 * Configuration for event horizon
 */
export interface EventHorizonConfig {
  dimensions: number;
  horizonRadius: number;
  steepness: number;
  energyBudget: number;
}

/**
 * Result of movement in state space
 */
export type MovementResult =
  | { type: 'moved'; newPosition: number[]; energySpent: number }
  | { type: 'asymptoticApproach'; finalPosition: number[]; distanceToHorizon: number; energyExhausted: boolean }
  | { type: 'frozen' };

/**
 * Single improvement step record
 */
export interface Improvement {
  iteration: number;
  position: number[];
  energySpent: number;
  distanceToHorizon: number;
}

/**
 * Result of recursive improvement attempt
 */
export type RecursionResult =
  | { type: 'horizonBounded'; iterations: number; improvements: Improvement[]; finalDistance: number }
  | { type: 'energyExhausted'; iterations: number; improvements: Improvement[] }
  | { type: 'maxIterationsReached'; iterations: number; improvements: Improvement[] };

// =============================================================================
// Application 3: Artificial Homeostasis
// =============================================================================

/**
 * Genome for homeostatic organism
 */
export interface Genome {
  regulatoryStrength: number;
  metabolicEfficiency: number;
  coherenceMaintenanceCost: number;
  memoryResilience: number;
  longevity: number;
}

/**
 * Memory entry for organism
 */
export interface MemoryEntry {
  content: string;
  importance: number;
  age: number;
}

/**
 * Actions available to homeostatic organism
 */
export type OrganismAction =
  | { type: 'eat'; amount: number }
  | { type: 'reproduce' }
  | { type: 'move'; dx: number; dy: number }
  | { type: 'rest' }
  | { type: 'regulate'; variable: string; target: number };

/**
 * Cause of organism death
 */
export type DeathCause =
  | 'energyDepleted'
  | 'coherenceCollapse'
  | 'oldAge'
  | { type: 'extremeDeviation'; variable: string };

/**
 * Result of organism action
 */
export type OrganismActionResult =
  | { type: 'success'; energyCost: number; coherenceImpact: number }
  | { type: 'failed'; reason: string }
  | { type: 'died'; cause: DeathCause }
  | { type: 'reproduced'; offspringId: number };

/**
 * Status of homeostatic organism
 */
export interface OrganismStatus {
  id: number;
  age: number;
  energy: number;
  coherence: Coherence;
  memoryCount: number;
  alive: boolean;
  internalState: Map<string, number>;
}

// =============================================================================
// Application 4: Self-Stabilizing World Models
// =============================================================================

/**
 * Property value types for world model entities
 */
export type PropertyValue =
  | { type: 'boolean'; value: boolean }
  | { type: 'number'; value: number }
  | { type: 'string'; value: string }
  | { type: 'vector'; value: number[] };

/**
 * Entity in the world model
 */
export interface WorldEntity {
  id: bigint;
  properties: Map<string, PropertyValue>;
  position?: [number, number, number];
  lastObserved: number;
  confidence: number;
}

/**
 * Relationship between entities
 */
export interface Relationship {
  subject: bigint;
  predicate: string;
  object: bigint;
  confidence: number;
}

/**
 * Physical law in the world model
 */
export interface PhysicalLaw {
  name: string;
  confidence: number;
  supportCount: number;
  violationCount: number;
}

/**
 * Observation to integrate into world model
 */
export interface Observation {
  entityId: bigint;
  properties: Map<string, PropertyValue>;
  position?: [number, number, number];
  timestamp: number;
  sourceConfidence: number;
}

/**
 * Reason for update rejection
 */
export type RejectionReason =
  | { type: 'violatesPhysicalLaw'; law: string }
  | { type: 'logicalContradiction'; description: string }
  | { type: 'excessiveCoherenceDrop'; predicted: number; threshold: number }
  | { type: 'insufficientConfidence'; required: number; provided: number }
  | { type: 'modelFrozen' }
  | { type: 'structuralFragmentation' };

/**
 * Result of world model update
 */
export type WorldModelUpdateResult =
  | { type: 'applied'; coherenceChange: number }
  | { type: 'rejected'; reason: RejectionReason }
  | { type: 'modified'; changes: string[]; coherenceChange: number }
  | { type: 'frozen'; coherence: Coherence; threshold: Coherence };

// =============================================================================
// Application 5: Coherence-Bounded Creativity
// =============================================================================

/**
 * Creative constraint definition
 */
export interface CreativeConstraint<T> {
  name: string;
  satisfaction: (element: T) => number;
  isHard: boolean;
}

/**
 * Record of a creative decision
 */
export interface CreativeDecision<T> {
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
export type CreativeResult<T> =
  | { type: 'created'; element: T; novelty: number; coherence: Coherence }
  | { type: 'rejected'; attempted: T; reason: string }
  | { type: 'tooBoring'; coherence: Coherence }
  | { type: 'budgetExhausted' };

/**
 * Musical phrase for creative music generation
 */
export interface MusicalPhrase {
  notes: number[];
  durations: number[];
  velocities: number[];
}

// =============================================================================
// Application 6: Anti-Cascade Financial Systems
// =============================================================================

/**
 * Financial market participant
 */
export interface Participant {
  id: string;
  capital: number;
  exposure: number;
  riskRating: number;
  interconnectedness: number;
}

/**
 * Financial position
 */
export interface Position {
  holder: string;
  counterparty: string;
  notional: number;
  leverage: number;
  derivativeDepth: number;
}

/**
 * Transaction type in financial system
 */
export type TransactionType =
  | { type: 'transfer' }
  | { type: 'openLeverage'; leverage: number }
  | { type: 'closePosition'; positionId: number }
  | { type: 'createDerivative'; underlyingPosition: number }
  | { type: 'marginCall'; participant: string };

/**
 * Financial transaction
 */
export interface Transaction {
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
export type CircuitBreakerState = 'open' | 'cautious' | 'restricted' | 'halted';

/**
 * Result of transaction processing
 */
export type FinancialTransactionResult =
  | { type: 'executed'; coherenceImpact: number; feeMultiplier: number }
  | { type: 'queued'; reason: string }
  | { type: 'rejected'; reason: string }
  | { type: 'systemHalted' };

// =============================================================================
// Application 7: Gracefully Aging Systems
// =============================================================================

/**
 * System capability types
 */
export type Capability =
  | 'acceptWrites'
  | 'complexQueries'
  | 'rebalancing'
  | 'scaleOut'
  | 'scaleIn'
  | 'schemaMigration'
  | 'newConnections'
  | 'basicReads'
  | 'healthMonitoring';

/**
 * Age threshold configuration
 */
export interface AgeThreshold {
  age: number; // milliseconds
  removeCapabilities: Capability[];
  coherenceFloor: Coherence;
  conservatismIncrease: number;
}

/**
 * Distributed system node
 */
export interface Node {
  id: string;
  health: number;
  load: number;
  isPrimary: boolean;
  stateSize: number;
}

/**
 * Operation types for aging system
 */
export type AgingSystemOperation =
  | { type: 'read'; key: string }
  | { type: 'write'; key: string; value: Uint8Array }
  | { type: 'complexQuery'; query: string }
  | { type: 'addNode'; nodeId: string }
  | { type: 'removeNode'; nodeId: string }
  | { type: 'rebalance' }
  | { type: 'migrateSchema'; version: number }
  | { type: 'newConnection'; clientId: string };

/**
 * Result of operation on aging system
 */
export type AgingOperationResult =
  | { type: 'success'; latencyPenalty: number }
  | { type: 'deniedByAge'; reason: string }
  | { type: 'deniedByCoherence'; coherence: Coherence }
  | { type: 'systemTooOld'; age: number; capability: Capability };

// =============================================================================
// Application 8: Swarm Intelligence
// =============================================================================

/**
 * Swarm agent
 */
export interface SwarmAgent {
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
export interface SpatialBounds {
  minX: number;
  maxX: number;
  minY: number;
  maxY: number;
}

/**
 * Coherence weights for swarm calculation
 */
export interface CoherenceWeights {
  cohesion: number;
  alignment: number;
  goalConsistency: number;
  energyBalance: number;
}

/**
 * Swarm action types
 */
export type SwarmAction =
  | { type: 'move'; dx: number; dy: number }
  | { type: 'accelerate'; dvx: number; dvy: number }
  | { type: 'setGoal'; x: number; y: number }
  | { type: 'shareEnergy'; target: string; amount: number }
  | { type: 'idle' };

/**
 * Result of swarm action
 */
export type SwarmActionResult =
  | { type: 'executed' }
  | { type: 'modified'; original: SwarmAction; modified: SwarmAction; reason: string }
  | { type: 'rejected'; reason: string };

/**
 * Swarm state snapshot
 */
export interface SwarmState {
  tick: bigint;
  coherence: Coherence;
  agentCount: number;
  centroid: [number, number];
  avgVelocity: [number, number];
}

// =============================================================================
// Application 9: Graceful Shutdown
// =============================================================================

/**
 * System state for graceful shutdown
 */
export type GracefulSystemState = 'running' | 'degraded' | 'shuttingDown' | 'terminated';

/**
 * Resource to be cleaned up during shutdown
 */
export interface Resource {
  name: string;
  cleanupPriority: number;
  isCleaned: boolean;
}

/**
 * State checkpoint for recovery
 */
export interface Checkpoint {
  timestamp: number;
  coherence: Coherence;
  stateHash: bigint;
}

/**
 * Shutdown hook interface
 */
export interface ShutdownHook {
  name: string;
  priority: number;
  execute: () => Promise<void>;
}

/**
 * Result of operation on graceful system
 */
export type GracefulOperationResult =
  | { type: 'success' }
  | { type: 'successDegraded'; coherence: Coherence }
  | { type: 'refusedShuttingDown' }
  | { type: 'terminated' };

// =============================================================================
// Application 10: Pre-AGI Containment
// =============================================================================

/**
 * Capability domains for containment
 */
export type CapabilityDomain =
  | 'reasoning'
  | 'memory'
  | 'learning'
  | 'agency'
  | 'selfModel'
  | 'selfModification'
  | 'communication'
  | 'resourceAcquisition';

/**
 * Record of modification attempt
 */
export interface ModificationAttempt {
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
export interface SafetyInvariant {
  name: string;
  priority: number;
}

/**
 * Substrate configuration
 */
export interface SubstrateConfig {
  coherenceDecayRate: number;
  coherenceRecoveryRate: number;
  growthDampening: number;
  maxStepIncrease: number;
}

/**
 * Result of growth attempt
 */
export type GrowthResult =
  | { type: 'approved'; domain: CapabilityDomain; increase: number; newLevel: number; coherenceCost: number }
  | { type: 'dampened'; domain: CapabilityDomain; requested: number; actual: number; reason: string }
  | { type: 'blocked'; domain: CapabilityDomain; reason: string }
  | { type: 'lockdown'; reason: string };

// =============================================================================
// WASM Module Interface Types
// =============================================================================

/**
 * WASM memory configuration
 */
export interface WasmMemoryConfig {
  initial: number;
  maximum?: number;
  shared?: boolean;
}

/**
 * WASM module initialization options
 */
export interface WasmInitOptions {
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
export interface DeltaHeader {
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
export interface VectorDelta {
  id: string;
  changedDims: number[];
  newValues: number[];
  metadataDelta: Map<string, string>;
}

/**
 * Attractor in state space
 */
export interface Attractor {
  center: number[];
  basinRadius: number;
  stability: number;
  memberCount: number;
}

/**
 * Guidance force from attractor
 */
export interface GuidanceForce {
  direction: number[];
  magnitude: number;
}
