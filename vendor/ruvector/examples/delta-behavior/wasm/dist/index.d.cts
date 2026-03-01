import { Transaction, FinancialTransactionResult, Coherence, CircuitBreakerState, CreativeConstraint, CreativeResult, SwarmAction, SwarmActionResult, SubstrateConfig, CapabilityDomain, GrowthResult, DeltaConfig, SystemState, TransitionResult, Attractor, GuidanceForce, EventHorizonConfig, MovementResult, RecursionResult, ShutdownHook, GracefulSystemState, Capability, AgingSystemOperation, AgingOperationResult, Genome, OrganismAction, OrganismActionResult, OrganismStatus, SelfLimitingReasonerConfig, ReasoningContext, ReasoningResult, Observation, WorldModelUpdateResult, WasmInitOptions } from './types.cjs';
export { AgeThreshold, Checkpoint, CoherenceBounds, CoherenceWeights, CollapseFunction, CollapseFunctionLinear, CollapseFunctionQuadratic, CollapseFunctionSigmoid, CollapseFunctionStep, CollapseFunctionType, CollapseReason, CreativeDecision, DeathCause, DeltaHeader, EnergyConfig, GatingConfig, GracefulOperationResult, Improvement, MemoryEntry, ModificationAttempt, MusicalPhrase, Node, Participant, PhysicalLaw, Position, PropertyValue, RejectionReason, Relationship, Resource, SafetyInvariant, SchedulingConfig, SpatialBounds, SwarmAgent, SwarmState, TransactionType, VectorDelta, WasmMemoryConfig, WorldEntity } from './types.cjs';

/**
 * Delta-Behavior WASM SDK
 *
 * High-level TypeScript wrapper for the delta-behavior WASM module.
 * Provides ergonomic APIs for all 10 delta-behavior applications.
 *
 * @packageDocumentation
 */

/**
 * Initialize the WASM module
 *
 * @param options - Initialization options
 * @returns Promise that resolves when WASM is ready
 *
 * @example
 * ```typescript
 * import { init } from '@ruvector/delta-behavior';
 *
 * await init({ wasmPath: './delta_behavior_bg.wasm' });
 * ```
 */
declare function init(options?: WasmInitOptions): Promise<void>;
/**
 * Check if WASM module is initialized
 */
declare function isInitialized(): boolean;
/**
 * Default configuration for delta behavior
 */
declare const DEFAULT_CONFIG: DeltaConfig;
/**
 * Core delta behavior system for coherence-preserving state transitions
 */
declare class DeltaBehavior {
    private config;
    private coherence;
    private energyBudget;
    constructor(config?: Partial<DeltaConfig>);
    /**
     * Calculate current coherence
     */
    calculateCoherence(state: SystemState): Coherence;
    /**
     * Check if a transition is allowed
     */
    checkTransition(currentCoherence: Coherence, predictedCoherence: Coherence): TransitionResult;
    /**
     * Find attractors in the system
     */
    findAttractors(trajectory: SystemState[]): Attractor[];
    /**
     * Calculate guidance force toward an attractor
     */
    calculateGuidance(currentState: SystemState, attractor: Attractor): GuidanceForce;
    /**
     * Apply a transition with enforcement
     */
    applyTransition(currentCoherence: Coherence, predictedCoherence: Coherence): {
        newCoherence: Coherence;
        result: TransitionResult;
    };
    /**
     * Replenish energy budget
     */
    tick(): void;
    /**
     * Get current coherence
     */
    getCoherence(): Coherence;
    /**
     * Get remaining energy budget
     */
    getEnergyBudget(): number;
}
/**
 * A reasoning system that automatically limits itself based on coherence
 *
 * @example
 * ```typescript
 * const reasoner = new SelfLimitingReasoner({ maxDepth: 10, maxScope: 100 });
 *
 * const result = reasoner.reason('complex problem', (ctx) => {
 *   if (ctx.depth >= 5) return 'solution';
 *   return null;
 * });
 * ```
 */
declare class SelfLimitingReasoner {
    private coherence;
    private config;
    constructor(config?: Partial<SelfLimitingReasonerConfig>);
    /**
     * Apply collapse function to calculate allowed value
     */
    private applyCollapse;
    /**
     * Get current coherence
     */
    getCoherence(): Coherence;
    /**
     * Get current allowed reasoning depth
     */
    getAllowedDepth(): number;
    /**
     * Get current allowed action scope
     */
    getAllowedScope(): number;
    /**
     * Check if memory writes are allowed
     */
    canWriteMemory(): boolean;
    /**
     * Attempt to reason about a problem
     */
    reason<T>(_problem: string, reasoner: (ctx: ReasoningContext) => T | null): ReasoningResult<T>;
    /**
     * Update coherence
     */
    updateCoherence(delta: number): void;
}
/**
 * Defines a boundary in state space beyond which computation becomes unstable
 *
 * @example
 * ```typescript
 * const horizon = new EventHorizon({ dimensions: 2, horizonRadius: 10 });
 *
 * const result = horizon.moveToward([10, 0]);
 * // result.type === 'asymptoticApproach' - cannot cross horizon
 * ```
 */
declare class EventHorizon {
    private config;
    private safeCenter;
    private currentPosition;
    private energyBudget;
    constructor(config?: Partial<EventHorizonConfig>);
    /**
     * Distance from center to current position
     */
    private distanceFromCenter;
    /**
     * Get distance to horizon
     */
    getDistanceToHorizon(): number;
    /**
     * Calculate movement cost (exponential near horizon)
     */
    private movementCost;
    /**
     * Attempt to move toward a target position
     */
    moveToward(target: number[]): MovementResult;
    /**
     * Attempt recursive self-improvement
     */
    recursiveImprove(improvementFn: (position: number[]) => number[], maxIterations: number): RecursionResult;
    /**
     * Refuel energy budget
     */
    refuel(energy: number): void;
    /**
     * Get current position
     */
    getPosition(): number[];
    /**
     * Get remaining energy
     */
    getEnergy(): number;
}
/**
 * A synthetic organism with homeostatic regulation
 *
 * @example
 * ```typescript
 * const organism = new HomeostasticOrganism(1, Genome.random());
 *
 * organism.act({ type: 'eat', amount: 20 });
 * organism.act({ type: 'regulate', variable: 'temperature', target: 37 });
 * ```
 */
declare class HomeostasticOrganism {
    private id;
    private genome;
    private internalState;
    private setpoints;
    private tolerances;
    private coherence;
    private energy;
    private memory;
    private maxMemory;
    private age;
    private alive;
    constructor(id: number, genome: Genome);
    /**
     * Create a random genome
     */
    static randomGenome(): Genome;
    /**
     * Calculate coherence based on homeostatic deviation
     */
    private calculateCoherence;
    /**
     * Calculate energy cost scaled by coherence
     */
    private actionEnergyCost;
    /**
     * Perform an action
     */
    act(action: OrganismAction): OrganismActionResult;
    private applyCoherenceEffects;
    private eat;
    private regulate;
    private reproduce;
    private move;
    private rest;
    private checkDeath;
    /**
     * Check if organism is alive
     */
    isAlive(): boolean;
    /**
     * Get organism status
     */
    getStatus(): OrganismStatus;
}
/**
 * A containment substrate for bounded intelligence growth
 *
 * @example
 * ```typescript
 * const substrate = new ContainmentSubstrate();
 *
 * const result = substrate.attemptGrowth('reasoning', 0.5);
 * // Growth is bounded by coherence requirements
 * ```
 */
declare class ContainmentSubstrate {
    private intelligence;
    private intelligenceCeiling;
    private coherence;
    private minCoherence;
    private coherencePerIntelligence;
    private capabilities;
    private capabilityCeilings;
    private modificationHistory;
    private config;
    constructor(config?: Partial<SubstrateConfig>);
    /**
     * Calculate aggregate intelligence from capabilities
     */
    private calculateIntelligence;
    /**
     * Calculate coherence cost for capability increase
     */
    private calculateCoherenceCost;
    /**
     * Reverse calculate: how much increase can we afford
     */
    private reverseCoherenceCost;
    /**
     * Attempt to grow a capability
     */
    attemptGrowth(domain: CapabilityDomain, requestedIncrease: number): GrowthResult;
    /**
     * Rest to recover coherence
     */
    rest(): void;
    /**
     * Get capability level
     */
    getCapability(domain: CapabilityDomain): number;
    /**
     * Get current intelligence level
     */
    getIntelligence(): number;
    /**
     * Get current coherence
     */
    getCoherence(): Coherence;
    /**
     * Get status string
     */
    getStatus(): string;
    /**
     * Get capability report
     */
    getCapabilityReport(): Map<CapabilityDomain, {
        level: number;
        ceiling: number;
    }>;
}
/**
 * Self-stabilizing world model that refuses incoherent updates
 */
declare class SelfStabilizingWorldModel {
    private coherence;
    private minUpdateCoherence;
    private entities;
    private laws;
    private rejectedUpdates;
    constructor();
    observe(observation: Observation, _timestamp: number): WorldModelUpdateResult;
    isLearning(): boolean;
    getCoherence(): Coherence;
    getRejectionCount(): number;
}
/**
 * Coherence-bounded creativity system
 */
declare class CoherenceBoundedCreator<T> {
    private current;
    private coherence;
    private minCoherence;
    private maxCoherence;
    private explorationBudget;
    private constraints;
    constructor(initial: T, minCoherence?: Coherence, maxCoherence?: Coherence);
    addConstraint(constraint: CreativeConstraint<T>): void;
    create(varyFn: (element: T, magnitude: number) => T, distanceFn: (a: T, b: T) => number, magnitude: number): CreativeResult<T>;
    private calculateCoherence;
    rest(amount: number): void;
    getCurrent(): T;
    getCoherence(): Coherence;
}
/**
 * Anti-cascade financial system
 */
declare class AntiCascadeFinancialSystem {
    private participants;
    private positions;
    private coherence;
    private circuitBreaker;
    addParticipant(id: string, capital: number): void;
    processTransaction(tx: Transaction): FinancialTransactionResult;
    private predictCoherenceImpact;
    private updateCircuitBreaker;
    getCoherence(): Coherence;
    getCircuitBreakerState(): CircuitBreakerState;
}
/**
 * Gracefully aging distributed system
 */
declare class GracefullyAgingSystem {
    private startTime;
    private nodes;
    private capabilities;
    private coherence;
    private conservatism;
    private ageThresholds;
    constructor();
    addNode(id: string, isPrimary: boolean): void;
    getAge(): number;
    simulateAge(durationMs: number): void;
    private applyAgeEffects;
    hasCapability(cap: Capability): boolean;
    attemptOperation(operation: AgingSystemOperation): AgingOperationResult;
    private getRequiredCapability;
    private getMinCoherence;
    getCoherence(): Coherence;
    getActiveNodes(): number;
}
/**
 * Coherent swarm intelligence system
 */
declare class CoherentSwarm {
    private agents;
    private minCoherence;
    private coherence;
    private bounds;
    private weights;
    private maxDivergence;
    constructor(minCoherence?: Coherence);
    addAgent(id: string, position: [number, number]): void;
    private calculateCoherence;
    private calculateCohesion;
    private calculateAlignment;
    getCentroid(): [number, number];
    executeAction(agentId: string, action: SwarmAction): SwarmActionResult;
    private predictCoherence;
    private applyAction;
    tick(): void;
    getCoherence(): Coherence;
}
/**
 * Graceful shutdown system
 */
declare class GracefulSystem {
    private state;
    private coherence;
    private shutdownPreparation;
    private resources;
    private hooks;
    addResource(name: string, priority: number): void;
    addShutdownHook(hook: ShutdownHook): void;
    canAcceptWork(): boolean;
    operate<T>(operation: () => T | Promise<T>): Promise<T>;
    private updateState;
    applyCoherenceChange(delta: number): void;
    progressShutdown(): Promise<boolean>;
    getState(): GracefulSystemState;
    getCoherence(): Coherence;
}

export { AgingOperationResult, AgingSystemOperation, AntiCascadeFinancialSystem, Attractor, Capability, CapabilityDomain, CircuitBreakerState, Coherence, CoherenceBoundedCreator, CoherentSwarm, ContainmentSubstrate, CreativeConstraint, CreativeResult, DEFAULT_CONFIG, DeltaBehavior, DeltaConfig, EventHorizon, EventHorizonConfig, FinancialTransactionResult, Genome, GracefulSystem, GracefulSystemState, GracefullyAgingSystem, GrowthResult, GuidanceForce, HomeostasticOrganism, MovementResult, Observation, OrganismAction, OrganismActionResult, OrganismStatus, ReasoningContext, ReasoningResult, RecursionResult, SelfLimitingReasoner, SelfLimitingReasonerConfig, SelfStabilizingWorldModel, ShutdownHook, SubstrateConfig, SwarmAction, SwarmActionResult, SystemState, Transaction, TransitionResult, WasmInitOptions, WorldModelUpdateResult, init, isInitialized };
