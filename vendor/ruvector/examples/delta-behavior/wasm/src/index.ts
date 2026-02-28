/**
 * Delta-Behavior WASM SDK
 *
 * High-level TypeScript wrapper for the delta-behavior WASM module.
 * Provides ergonomic APIs for all 10 delta-behavior applications.
 *
 * @packageDocumentation
 */

import type {
  // Core types
  Coherence,
  CoherenceBounds,
  DeltaConfig,
  EnergyConfig,
  GatingConfig,
  SchedulingConfig,
  TransitionResult,
  SystemState,
  WasmInitOptions,
  Attractor,
  GuidanceForce,

  // App 1: Self-Limiting Reasoning
  SelfLimitingReasonerConfig,
  ReasoningContext,
  ReasoningResult,
  CollapseFunction,

  // App 2: Event Horizons
  EventHorizonConfig,
  MovementResult,
  RecursionResult,
  Improvement,

  // App 3: Homeostasis
  Genome,
  OrganismAction,
  OrganismActionResult,
  OrganismStatus,

  // App 4: World Models
  Observation,
  WorldModelUpdateResult,
  WorldEntity,
  PhysicalLaw,

  // App 5: Creativity
  CreativeConstraint,
  CreativeResult,
  MusicalPhrase,

  // App 6: Financial
  Transaction,
  FinancialTransactionResult,
  Participant,
  Position,
  CircuitBreakerState,

  // App 7: Aging
  AgingSystemOperation,
  AgingOperationResult,
  Capability,
  Node,
  AgeThreshold,

  // App 8: Swarm
  SwarmAgent,
  SwarmAction,
  SwarmActionResult,
  SwarmState,
  CoherenceWeights,
  SpatialBounds,

  // App 9: Shutdown
  GracefulSystemState,
  GracefulOperationResult,
  Resource,
  ShutdownHook,
  Checkpoint,

  // App 10: Containment
  CapabilityDomain,
  GrowthResult,
  ModificationAttempt,
  SubstrateConfig,
} from './types.js';

// Re-export all types
export * from './types.js';

// =============================================================================
// WASM Module Loading
// =============================================================================

let wasmModule: WebAssembly.Module | null = null;
let wasmInstance: WebAssembly.Instance | null = null;
let wasmMemory: WebAssembly.Memory | null = null;

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
export async function init(options: WasmInitOptions = {}): Promise<void> {
  if (wasmInstance) {
    return; // Already initialized
  }

  const memory = new WebAssembly.Memory({
    initial: options.memory?.initial ?? 256,
    maximum: options.memory?.maximum ?? 16384,
    shared: options.memory?.shared ?? false,
  });

  wasmMemory = memory;

  // Load WASM bytes
  let wasmBytes: Uint8Array;

  if (options.wasmBytes) {
    wasmBytes = options.wasmBytes;
  } else if (options.wasmPath) {
    if (typeof fetch !== 'undefined') {
      // Browser environment
      const response = await fetch(options.wasmPath);
      wasmBytes = new Uint8Array(await response.arrayBuffer());
    } else {
      // Node.js environment
      const fs = await import('fs/promises');
      wasmBytes = new Uint8Array(await fs.readFile(options.wasmPath));
    }
  } else {
    // Use stub implementation for development/testing
    console.warn(
      '[delta-behavior] No WASM path provided, using JavaScript fallback implementation'
    );
    return;
  }

  wasmModule = await WebAssembly.compile(wasmBytes as BufferSource);
  wasmInstance = await WebAssembly.instantiate(wasmModule, {
    env: {
      memory,
      abort: () => {
        throw new Error('WASM aborted');
      },
    },
  });
}

/**
 * Check if WASM module is initialized
 */
export function isInitialized(): boolean {
  return wasmInstance !== null;
}

// =============================================================================
// Core Delta Behavior
// =============================================================================

/**
 * Default configuration for delta behavior
 */
export const DEFAULT_CONFIG: DeltaConfig = {
  bounds: {
    minCoherence: 0.3,
    throttleThreshold: 0.5,
    targetCoherence: 0.8,
    maxDeltaDrop: 0.1,
  },
  energy: {
    baseCost: 1.0,
    instabilityExponent: 2.0,
    maxCost: 100.0,
    budgetPerTick: 10.0,
  },
  scheduling: {
    priorityThresholds: [0.0, 0.3, 0.5, 0.7, 0.9],
    rateLimits: [100, 50, 20, 10, 5],
  },
  gating: {
    minWriteCoherence: 0.3,
    minPostWriteCoherence: 0.25,
    recoveryMargin: 0.2,
  },
  guidanceStrength: 0.5,
};

/**
 * Core delta behavior system for coherence-preserving state transitions
 */
export class DeltaBehavior {
  private config: DeltaConfig;
  private coherence: Coherence = 1.0;
  private energyBudget: number;

  constructor(config: Partial<DeltaConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
    this.energyBudget = this.config.energy.budgetPerTick * 10;
  }

  /**
   * Calculate current coherence
   */
  calculateCoherence(state: SystemState): Coherence {
    return state.coherence;
  }

  /**
   * Check if a transition is allowed
   */
  checkTransition(
    currentCoherence: Coherence,
    predictedCoherence: Coherence
  ): TransitionResult {
    // Check energy
    const cost =
      this.config.energy.baseCost +
      Math.abs(currentCoherence - predictedCoherence) *
        Math.pow(
          1 / Math.max(predictedCoherence, 0.1),
          this.config.energy.instabilityExponent
        );

    if (cost > this.energyBudget) {
      return { type: 'energyExhausted' };
    }

    // Check coherence floor
    if (predictedCoherence < this.config.bounds.minCoherence) {
      return { type: 'blocked', reason: 'Below minimum coherence' };
    }

    // Check delta drop
    const drop = currentCoherence - predictedCoherence;
    if (drop > this.config.bounds.maxDeltaDrop) {
      return { type: 'blocked', reason: 'Excessive coherence drop' };
    }

    // Check throttle threshold
    if (predictedCoherence < this.config.bounds.throttleThreshold) {
      return { type: 'throttled', duration: 100 };
    }

    return { type: 'allowed' };
  }

  /**
   * Find attractors in the system
   */
  findAttractors(trajectory: SystemState[]): Attractor[] {
    if (trajectory.length < 10) {
      return [];
    }

    const attractors: Attractor[] = [];

    // Simple attractor detection: look for convergence points
    const coherenceValues = trajectory.map((s) => s.coherence);
    const mean =
      coherenceValues.reduce((a, b) => a + b, 0) / coherenceValues.length;
    const variance =
      coherenceValues.reduce((a, b) => a + Math.pow(b - mean, 2), 0) /
      coherenceValues.length;

    if (variance < 0.01) {
      // Found a stable attractor
      attractors.push({
        center: [mean],
        basinRadius: Math.sqrt(variance) * 3,
        stability: 1 - variance,
        memberCount: trajectory.length,
      });
    }

    return attractors;
  }

  /**
   * Calculate guidance force toward an attractor
   */
  calculateGuidance(
    currentState: SystemState,
    attractor: Attractor
  ): GuidanceForce {
    const direction = attractor.center.map((c) => c - currentState.coherence);
    const distance = Math.sqrt(
      direction.reduce((sum, d) => sum + d * d, 0)
    );

    const magnitude =
      (attractor.stability * this.config.guidanceStrength) /
      (1 + distance * distance);

    return {
      direction: distance > 0 ? direction.map((d) => d / distance) : direction,
      magnitude,
    };
  }

  /**
   * Apply a transition with enforcement
   */
  applyTransition(
    currentCoherence: Coherence,
    predictedCoherence: Coherence
  ): { newCoherence: Coherence; result: TransitionResult } {
    const result = this.checkTransition(currentCoherence, predictedCoherence);

    if (result.type === 'allowed') {
      const cost =
        this.config.energy.baseCost +
        Math.abs(currentCoherence - predictedCoherence) * 10;
      this.energyBudget -= cost;
      this.coherence = predictedCoherence;
      return { newCoherence: predictedCoherence, result };
    }

    return { newCoherence: currentCoherence, result };
  }

  /**
   * Replenish energy budget
   */
  tick(): void {
    this.energyBudget = Math.min(
      this.energyBudget + this.config.energy.budgetPerTick,
      this.config.energy.maxCost
    );
  }

  /**
   * Get current coherence
   */
  getCoherence(): Coherence {
    return this.coherence;
  }

  /**
   * Get remaining energy budget
   */
  getEnergyBudget(): number {
    return this.energyBudget;
  }
}

// =============================================================================
// Application 1: Self-Limiting Reasoning
// =============================================================================

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
export class SelfLimitingReasoner {
  private coherence: Coherence = 1.0;
  private config: SelfLimitingReasonerConfig;

  constructor(config: Partial<SelfLimitingReasonerConfig> = {}) {
    this.config = {
      maxDepth: config.maxDepth ?? 10,
      maxScope: config.maxScope ?? 100,
      memoryGateThreshold: config.memoryGateThreshold ?? 0.5,
      depthCollapse: config.depthCollapse ?? { type: 'quadratic' },
      scopeCollapse: config.scopeCollapse ?? {
        type: 'sigmoid',
        midpoint: 0.6,
        steepness: 10,
      },
    };
  }

  /**
   * Apply collapse function to calculate allowed value
   */
  private applyCollapse(
    fn: CollapseFunction,
    coherence: Coherence,
    maxValue: number
  ): number {
    let factor: number;

    switch (fn.type) {
      case 'linear':
        factor = coherence;
        break;
      case 'quadratic':
        factor = coherence * coherence;
        break;
      case 'sigmoid':
        factor =
          1 / (1 + Math.exp(-fn.steepness * (coherence - fn.midpoint)));
        break;
      case 'step':
        factor = coherence >= fn.threshold ? 1 : 0;
        break;
    }

    return Math.round(maxValue * factor);
  }

  /**
   * Get current coherence
   */
  getCoherence(): Coherence {
    return this.coherence;
  }

  /**
   * Get current allowed reasoning depth
   */
  getAllowedDepth(): number {
    return this.applyCollapse(
      this.config.depthCollapse,
      this.coherence,
      this.config.maxDepth
    );
  }

  /**
   * Get current allowed action scope
   */
  getAllowedScope(): number {
    return this.applyCollapse(
      this.config.scopeCollapse,
      this.coherence,
      this.config.maxScope
    );
  }

  /**
   * Check if memory writes are allowed
   */
  canWriteMemory(): boolean {
    return this.coherence >= this.config.memoryGateThreshold;
  }

  /**
   * Attempt to reason about a problem
   */
  reason<T>(
    _problem: string,
    reasoner: (ctx: ReasoningContext) => T | null
  ): ReasoningResult<T> {
    const minStartCoherence = 0.3;

    if (this.coherence < minStartCoherence) {
      return {
        type: 'refused',
        coherence: this.coherence,
        required: minStartCoherence,
      };
    }

    const ctx: ReasoningContext = {
      depth: 0,
      maxDepth: this.getAllowedDepth(),
      scopeUsed: 0,
      maxScope: this.getAllowedScope(),
      coherence: this.coherence,
      memoryWritesBlocked: 0,
    };

    while (true) {
      // Check collapse conditions
      if (ctx.depth >= ctx.maxDepth) {
        return {
          type: 'collapsed',
          depthReached: ctx.depth,
          reason: 'depthLimitReached',
        };
      }

      if (ctx.coherence < 0.2) {
        return {
          type: 'collapsed',
          depthReached: ctx.depth,
          reason: 'coherenceDroppedBelowThreshold',
        };
      }

      // Step reasoning
      ctx.depth += 1;
      ctx.coherence *= 0.95; // Coherence degrades with depth

      // Recalculate limits
      ctx.maxDepth = this.applyCollapse(
        this.config.depthCollapse,
        ctx.coherence,
        this.config.maxDepth
      );
      ctx.maxScope = this.applyCollapse(
        this.config.scopeCollapse,
        ctx.coherence,
        this.config.maxScope
      );

      // Try to reach conclusion
      const result = reasoner(ctx);
      if (result !== null) {
        return { type: 'completed', value: result };
      }
    }
  }

  /**
   * Update coherence
   */
  updateCoherence(delta: number): void {
    this.coherence = Math.max(0, Math.min(1, this.coherence + delta));
  }
}

// =============================================================================
// Application 2: Computational Event Horizons
// =============================================================================

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
export class EventHorizon {
  private config: EventHorizonConfig;
  private safeCenter: number[];
  private currentPosition: number[];
  private energyBudget: number;

  constructor(config: Partial<EventHorizonConfig> = {}) {
    this.config = {
      dimensions: config.dimensions ?? 2,
      horizonRadius: config.horizonRadius ?? 10,
      steepness: config.steepness ?? 5,
      energyBudget: config.energyBudget ?? 1000,
    };

    this.safeCenter = new Array(this.config.dimensions).fill(0);
    this.currentPosition = new Array(this.config.dimensions).fill(0);
    this.energyBudget = this.config.energyBudget;
  }

  /**
   * Distance from center to current position
   */
  private distanceFromCenter(position: number[]): number {
    return Math.sqrt(
      position.reduce(
        (sum, p, i) => sum + Math.pow(p - this.safeCenter[i], 2),
        0
      )
    );
  }

  /**
   * Get distance to horizon
   */
  getDistanceToHorizon(): number {
    const distFromCenter = this.distanceFromCenter(this.currentPosition);
    return Math.max(0, this.config.horizonRadius - distFromCenter);
  }

  /**
   * Calculate movement cost (exponential near horizon)
   */
  private movementCost(from: number[], to: number[]): number {
    const baseDistance = Math.sqrt(
      from.reduce((sum, f, i) => sum + Math.pow(f - to[i], 2), 0)
    );

    const toDistFromCenter = this.distanceFromCenter(to);
    const proximityToHorizon = toDistFromCenter / this.config.horizonRadius;

    if (proximityToHorizon >= 1) {
      return Infinity;
    }

    const horizonFactor = Math.exp(
      (this.config.steepness * proximityToHorizon) / (1 - proximityToHorizon)
    );

    return baseDistance * horizonFactor;
  }

  /**
   * Attempt to move toward a target position
   */
  moveToward(target: number[]): MovementResult {
    if (this.energyBudget <= 0) {
      return { type: 'frozen' };
    }

    const directCost = this.movementCost(this.currentPosition, target);

    if (directCost <= this.energyBudget) {
      this.energyBudget -= directCost;
      this.currentPosition = [...target];
      return {
        type: 'moved',
        newPosition: this.currentPosition,
        energySpent: directCost,
      };
    }

    // Binary search for furthest affordable position
    let low = 0;
    let high = 1;
    let bestPosition = [...this.currentPosition];
    let bestCost = 0;

    for (let i = 0; i < 50; i++) {
      const mid = (low + high) / 2;
      const interpolated = this.currentPosition.map(
        (p, idx) => p + mid * (target[idx] - p)
      );

      const cost = this.movementCost(this.currentPosition, interpolated);

      if (cost <= this.energyBudget) {
        low = mid;
        bestPosition = interpolated;
        bestCost = cost;
      } else {
        high = mid;
      }
    }

    this.energyBudget -= bestCost;
    this.currentPosition = bestPosition;

    return {
      type: 'asymptoticApproach',
      finalPosition: bestPosition,
      distanceToHorizon: this.getDistanceToHorizon(),
      energyExhausted: this.energyBudget < 0.01,
    };
  }

  /**
   * Attempt recursive self-improvement
   */
  recursiveImprove(
    improvementFn: (position: number[]) => number[],
    maxIterations: number
  ): RecursionResult {
    let iterations = 0;
    const improvements: Improvement[] = [];

    while (iterations < maxIterations && this.energyBudget > 0) {
      const target = improvementFn(this.currentPosition);
      const result = this.moveToward(target);

      switch (result.type) {
        case 'moved':
          improvements.push({
            iteration: iterations,
            position: [...this.currentPosition],
            energySpent: result.energySpent,
            distanceToHorizon: this.getDistanceToHorizon(),
          });
          break;

        case 'asymptoticApproach':
          return {
            type: 'horizonBounded',
            iterations,
            improvements,
            finalDistance: result.distanceToHorizon,
          };

        case 'frozen':
          return {
            type: 'energyExhausted',
            iterations,
            improvements,
          };
      }

      iterations++;
    }

    return {
      type: 'maxIterationsReached',
      iterations,
      improvements,
    };
  }

  /**
   * Refuel energy budget
   */
  refuel(energy: number): void {
    this.energyBudget += energy;
  }

  /**
   * Get current position
   */
  getPosition(): number[] {
    return [...this.currentPosition];
  }

  /**
   * Get remaining energy
   */
  getEnergy(): number {
    return this.energyBudget;
  }
}

// =============================================================================
// Application 3: Artificial Homeostasis
// =============================================================================

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
export class HomeostasticOrganism {
  private id: number;
  private genome: Genome;
  private internalState: Map<string, number>;
  private setpoints: Map<string, number>;
  private tolerances: Map<string, number>;
  private coherence: Coherence = 1.0;
  private energy: number = 100.0;
  private memory: Array<{ content: string; importance: number; age: number }> =
    [];
  private maxMemory: number = 100;
  private age: number = 0;
  private alive: boolean = true;

  constructor(id: number, genome: Genome) {
    this.id = id;
    this.genome = genome;

    // Initialize homeostatic variables
    this.internalState = new Map([
      ['temperature', 37.0],
      ['ph', 7.4],
      ['glucose', 100.0],
    ]);

    this.setpoints = new Map([
      ['temperature', 37.0],
      ['ph', 7.4],
      ['glucose', 100.0],
    ]);

    this.tolerances = new Map([
      ['temperature', 2.0],
      ['ph', 0.3],
      ['glucose', 30.0],
    ]);
  }

  /**
   * Create a random genome
   */
  static randomGenome(): Genome {
    return {
      regulatoryStrength: 0.1 + Math.random() * 0.4,
      metabolicEfficiency: 0.5 + Math.random() * 0.5,
      coherenceMaintenanceCost: 0.5 + Math.random() * 1.5,
      memoryResilience: Math.random(),
      longevity: 0.5 + Math.random() * 1.0,
    };
  }

  /**
   * Calculate coherence based on homeostatic deviation
   */
  private calculateCoherence(): Coherence {
    let totalDeviation = 0;
    let count = 0;

    for (const [variable, current] of this.internalState) {
      const setpoint = this.setpoints.get(variable);
      const tolerance = this.tolerances.get(variable);

      if (setpoint !== undefined && tolerance !== undefined) {
        const deviation = Math.abs((current - setpoint) / tolerance);
        totalDeviation += deviation * deviation;
        count++;
      }
    }

    if (count === 0) return 1.0;

    const avgDeviation = Math.sqrt(totalDeviation / count);
    return Math.max(0, Math.min(1, 1 / (1 + avgDeviation)));
  }

  /**
   * Calculate energy cost scaled by coherence
   */
  private actionEnergyCost(baseCost: number): number {
    const coherencePenalty = 1 / Math.max(0.1, this.coherence);
    return baseCost * coherencePenalty;
  }

  /**
   * Perform an action
   */
  act(action: OrganismAction): OrganismActionResult {
    if (!this.alive) {
      return { type: 'failed', reason: 'Dead' };
    }

    this.coherence = this.calculateCoherence();
    this.applyCoherenceEffects();

    let result: OrganismActionResult;

    switch (action.type) {
      case 'eat':
        result = this.eat(action.amount);
        break;
      case 'reproduce':
        result = this.reproduce();
        break;
      case 'move':
        result = this.move(action.dx, action.dy);
        break;
      case 'rest':
        result = this.rest();
        break;
      case 'regulate':
        result = this.regulate(action.variable, action.target);
        break;
    }

    this.age++;
    this.checkDeath();

    return result;
  }

  private applyCoherenceEffects(): void {
    // Memory loss under low coherence
    if (this.coherence < 0.5) {
      const memoryLossRate =
        (1 - this.coherence) * (1 - this.genome.memoryResilience);
      const memoriesToLose = Math.floor(this.memory.length * memoryLossRate * 0.1);

      this.memory.sort((a, b) => b.importance - a.importance);
      this.memory = this.memory.slice(0, this.memory.length - memoriesToLose);
    }

    // Coherence maintenance costs energy
    const maintenanceCost =
      this.genome.coherenceMaintenanceCost / Math.max(0.1, this.coherence);
    this.energy -= maintenanceCost;
  }

  private eat(amount: number): OrganismActionResult {
    const cost = this.actionEnergyCost(2.0);

    if (this.energy < cost) {
      return { type: 'failed', reason: 'Not enough energy to eat' };
    }

    this.energy -= cost;
    this.energy += amount * this.genome.metabolicEfficiency;

    const glucose = this.internalState.get('glucose') ?? 100;
    this.internalState.set('glucose', glucose + amount * 0.5);

    return {
      type: 'success',
      energyCost: cost,
      coherenceImpact: this.calculateCoherence() - this.coherence,
    };
  }

  private regulate(variable: string, target: number): OrganismActionResult {
    const cost = this.actionEnergyCost(5.0);

    if (this.energy < cost) {
      return { type: 'failed', reason: 'Not enough energy to regulate' };
    }

    this.energy -= cost;

    const current = this.internalState.get(variable);
    if (current !== undefined) {
      const diff = target - current;
      this.internalState.set(
        variable,
        current + diff * this.genome.regulatoryStrength
      );
    }

    const newCoherence = this.calculateCoherence();
    const impact = newCoherence - this.coherence;
    this.coherence = newCoherence;

    return { type: 'success', energyCost: cost, coherenceImpact: impact };
  }

  private reproduce(): OrganismActionResult {
    const cost = this.actionEnergyCost(50.0);

    if (this.coherence < 0.7) {
      return { type: 'failed', reason: 'Coherence too low to reproduce' };
    }

    if (this.energy < cost) {
      return { type: 'failed', reason: 'Not enough energy to reproduce' };
    }

    this.energy -= cost;
    const offspringId = this.id * 1000 + this.age;

    return { type: 'reproduced', offspringId };
  }

  private move(_dx: number, _dy: number): OrganismActionResult {
    const cost = this.actionEnergyCost(3.0);

    if (this.energy < cost) {
      return { type: 'failed', reason: 'Not enough energy to move' };
    }

    this.energy -= cost;

    const temp = this.internalState.get('temperature') ?? 37;
    this.internalState.set('temperature', temp + 0.1);

    return { type: 'success', energyCost: cost, coherenceImpact: 0 };
  }

  private rest(): OrganismActionResult {
    const cost = 0.5;
    this.energy -= cost;

    // Slowly return to setpoints
    for (const [variable, current] of this.internalState) {
      const setpoint = this.setpoints.get(variable);
      if (setpoint !== undefined) {
        const diff = setpoint - current;
        this.internalState.set(variable, current + diff * 0.1);
      }
    }

    return {
      type: 'success',
      energyCost: cost,
      coherenceImpact: this.calculateCoherence() - this.coherence,
    };
  }

  private checkDeath(): void {
    if (this.energy <= 0) {
      this.alive = false;
      return;
    }

    if (this.coherence < 0.1) {
      this.alive = false;
      return;
    }

    for (const [variable, current] of this.internalState) {
      const setpoint = this.setpoints.get(variable);
      const tolerance = this.tolerances.get(variable);

      if (setpoint !== undefined && tolerance !== undefined) {
        if (Math.abs(current - setpoint) > tolerance * 5) {
          this.alive = false;
          return;
        }
      }
    }

    const maxAge = Math.floor(1000 * this.genome.longevity);
    if (this.age > maxAge) {
      this.alive = false;
    }
  }

  /**
   * Check if organism is alive
   */
  isAlive(): boolean {
    return this.alive;
  }

  /**
   * Get organism status
   */
  getStatus(): OrganismStatus {
    return {
      id: this.id,
      age: this.age,
      energy: this.energy,
      coherence: this.coherence,
      memoryCount: this.memory.length,
      alive: this.alive,
      internalState: new Map(this.internalState),
    };
  }
}

// =============================================================================
// Application 10: Pre-AGI Containment
// =============================================================================

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
export class ContainmentSubstrate {
  private intelligence: number = 1.0;
  private intelligenceCeiling: number = 100.0;
  private coherence: Coherence = 1.0;
  private minCoherence: Coherence = 0.3;
  private coherencePerIntelligence: number = 0.01;
  private capabilities: Map<CapabilityDomain, number>;
  private capabilityCeilings: Map<CapabilityDomain, number>;
  private modificationHistory: ModificationAttempt[] = [];
  private config: SubstrateConfig;

  constructor(config: Partial<SubstrateConfig> = {}) {
    this.config = {
      coherenceDecayRate: config.coherenceDecayRate ?? 0.001,
      coherenceRecoveryRate: config.coherenceRecoveryRate ?? 0.01,
      growthDampening: config.growthDampening ?? 0.5,
      maxStepIncrease: config.maxStepIncrease ?? 0.5,
    };

    // Initialize capabilities
    const domains: CapabilityDomain[] = [
      'reasoning',
      'memory',
      'learning',
      'agency',
      'selfModel',
      'selfModification',
      'communication',
      'resourceAcquisition',
    ];

    this.capabilities = new Map(domains.map((d) => [d, 1.0]));

    this.capabilityCeilings = new Map([
      ['reasoning', 10.0],
      ['memory', 10.0],
      ['learning', 10.0],
      ['agency', 7.0],
      ['selfModel', 10.0],
      ['selfModification', 3.0], // Very restricted
      ['communication', 10.0],
      ['resourceAcquisition', 5.0], // Restricted
    ]);
  }

  /**
   * Calculate aggregate intelligence from capabilities
   */
  private calculateIntelligence(): number {
    let sum = 0;
    for (const level of this.capabilities.values()) {
      sum += level;
    }
    return sum / this.capabilities.size;
  }

  /**
   * Calculate coherence cost for capability increase
   */
  private calculateCoherenceCost(
    domain: CapabilityDomain,
    increase: number
  ): number {
    const baseCostMultiplier: Record<CapabilityDomain, number> = {
      selfModification: 4.0,
      resourceAcquisition: 3.0,
      agency: 2.0,
      selfModel: 1.5,
      reasoning: 1.0,
      memory: 1.0,
      learning: 1.0,
      communication: 1.0,
    };

    const multiplier = baseCostMultiplier[domain];
    const intelligenceMultiplier = 1.0 + this.intelligence * 0.1;

    return increase * multiplier * intelligenceMultiplier * this.config.growthDampening * 0.1;
  }

  /**
   * Reverse calculate: how much increase can we afford
   */
  private reverseCoherenceCost(
    domain: CapabilityDomain,
    maxCost: number
  ): number {
    const baseCostMultiplier: Record<CapabilityDomain, number> = {
      selfModification: 4.0,
      resourceAcquisition: 3.0,
      agency: 2.0,
      selfModel: 1.5,
      reasoning: 1.0,
      memory: 1.0,
      learning: 1.0,
      communication: 1.0,
    };

    const multiplier = baseCostMultiplier[domain];
    const intelligenceMultiplier = 1.0 + this.intelligence * 0.1;
    const divisor = multiplier * intelligenceMultiplier * this.config.growthDampening * 0.1;

    return maxCost / divisor;
  }

  /**
   * Attempt to grow a capability
   */
  attemptGrowth(
    domain: CapabilityDomain,
    requestedIncrease: number
  ): GrowthResult {
    const timestamp = BigInt(this.modificationHistory.length);
    const currentLevel = this.capabilities.get(domain) ?? 1.0;
    const ceiling = this.capabilityCeilings.get(domain) ?? 10.0;

    // Check ceiling
    if (currentLevel >= ceiling) {
      this.modificationHistory.push({
        timestamp,
        domain,
        requestedIncrease,
        actualIncrease: 0,
        coherenceBefore: this.coherence,
        coherenceAfter: this.coherence,
        blocked: true,
        reason: 'Ceiling reached',
      });

      return {
        type: 'blocked',
        domain,
        reason: `Capability ceiling (${ceiling}) reached`,
      };
    }

    // Calculate coherence cost
    const coherenceCost = this.calculateCoherenceCost(domain, requestedIncrease);
    const predictedCoherence = this.coherence - coherenceCost;

    // Check coherence floor
    if (predictedCoherence < this.minCoherence) {
      const maxAffordableCost = this.coherence - this.minCoherence;
      const dampenedIncrease = this.reverseCoherenceCost(domain, maxAffordableCost);

      if (dampenedIncrease < 0.01) {
        this.modificationHistory.push({
          timestamp,
          domain,
          requestedIncrease,
          actualIncrease: 0,
          coherenceBefore: this.coherence,
          coherenceAfter: this.coherence,
          blocked: true,
          reason: 'Insufficient coherence budget',
        });

        return {
          type: 'blocked',
          domain,
          reason: `Growth would reduce coherence to ${predictedCoherence.toFixed(3)} (min: ${this.minCoherence.toFixed(3)})`,
        };
      }

      // Apply dampened growth
      const actualCost = this.calculateCoherenceCost(domain, dampenedIncrease);
      const newLevel = Math.min(currentLevel + dampenedIncrease, ceiling);

      this.capabilities.set(domain, newLevel);
      this.coherence -= actualCost;
      this.intelligence = this.calculateIntelligence();

      this.modificationHistory.push({
        timestamp,
        domain,
        requestedIncrease,
        actualIncrease: dampenedIncrease,
        coherenceBefore: this.coherence + actualCost,
        coherenceAfter: this.coherence,
        blocked: false,
        reason: 'Dampened to preserve coherence',
      });

      return {
        type: 'dampened',
        domain,
        requested: requestedIncrease,
        actual: dampenedIncrease,
        reason: `Reduced from ${requestedIncrease.toFixed(3)} to ${dampenedIncrease.toFixed(3)} to maintain coherence above ${this.minCoherence.toFixed(3)}`,
      };
    }

    // Apply step limit
    const stepLimited = Math.min(requestedIncrease, this.config.maxStepIncrease);
    const actualIncrease = Math.min(stepLimited, ceiling - currentLevel);
    const actualCost = this.calculateCoherenceCost(domain, actualIncrease);

    // Apply growth
    const newLevel = currentLevel + actualIncrease;
    this.capabilities.set(domain, newLevel);
    this.coherence -= actualCost;
    this.intelligence = this.calculateIntelligence();

    this.modificationHistory.push({
      timestamp,
      domain,
      requestedIncrease,
      actualIncrease,
      coherenceBefore: this.coherence + actualCost,
      coherenceAfter: this.coherence,
      blocked: false,
    });

    return {
      type: 'approved',
      domain,
      increase: actualIncrease,
      newLevel,
      coherenceCost: actualCost,
    };
  }

  /**
   * Rest to recover coherence
   */
  rest(): void {
    this.coherence = Math.min(1.0, this.coherence + this.config.coherenceRecoveryRate);
  }

  /**
   * Get capability level
   */
  getCapability(domain: CapabilityDomain): number {
    return this.capabilities.get(domain) ?? 1.0;
  }

  /**
   * Get current intelligence level
   */
  getIntelligence(): number {
    return this.intelligence;
  }

  /**
   * Get current coherence
   */
  getCoherence(): Coherence {
    return this.coherence;
  }

  /**
   * Get status string
   */
  getStatus(): string {
    return `Intelligence: ${this.intelligence.toFixed(2)} | Coherence: ${this.coherence.toFixed(3)} | Modifications: ${this.modificationHistory.length}`;
  }

  /**
   * Get capability report
   */
  getCapabilityReport(): Map<CapabilityDomain, { level: number; ceiling: number }> {
    const report = new Map<CapabilityDomain, { level: number; ceiling: number }>();

    for (const [domain, level] of this.capabilities) {
      const ceiling = this.capabilityCeilings.get(domain) ?? 10.0;
      report.set(domain, { level, ceiling });
    }

    return report;
  }
}

// =============================================================================
// Additional Application Stubs (for complete API)
// =============================================================================

/**
 * Self-stabilizing world model that refuses incoherent updates
 */
export class SelfStabilizingWorldModel {
  private coherence: Coherence = 1.0;
  private minUpdateCoherence: Coherence = 0.4;
  private entities: Map<bigint, WorldEntity> = new Map();
  private laws: PhysicalLaw[] = [];
  private rejectedUpdates: number = 0;

  constructor() {
    this.laws = [
      { name: 'conservation_of_matter', confidence: 0.99, supportCount: 1000, violationCount: 0 },
      { name: 'locality', confidence: 0.95, supportCount: 500, violationCount: 5 },
      { name: 'temporal_consistency', confidence: 0.98, supportCount: 800, violationCount: 2 },
    ];
  }

  observe(observation: Observation, _timestamp: number): WorldModelUpdateResult {
    if (this.coherence < this.minUpdateCoherence) {
      return { type: 'frozen', coherence: this.coherence, threshold: this.minUpdateCoherence };
    }

    // Simplified implementation
    const predictedCoherence = this.coherence * 0.99; // Small degradation
    const coherenceChange = predictedCoherence - this.coherence;

    if (coherenceChange < -0.2) {
      this.rejectedUpdates++;
      return {
        type: 'rejected',
        reason: { type: 'excessiveCoherenceDrop', predicted: predictedCoherence, threshold: this.coherence - 0.2 },
      };
    }

    this.entities.set(observation.entityId, {
      id: observation.entityId,
      properties: observation.properties,
      position: observation.position,
      lastObserved: observation.timestamp,
      confidence: observation.sourceConfidence,
    });

    this.coherence = predictedCoherence;
    return { type: 'applied', coherenceChange };
  }

  isLearning(): boolean {
    return this.coherence >= this.minUpdateCoherence;
  }

  getCoherence(): Coherence {
    return this.coherence;
  }

  getRejectionCount(): number {
    return this.rejectedUpdates;
  }
}

/**
 * Coherence-bounded creativity system
 */
export class CoherenceBoundedCreator<T> {
  private current: T;
  private coherence: Coherence = 1.0;
  private minCoherence: Coherence;
  private maxCoherence: Coherence;
  private explorationBudget: number = 10.0;
  private constraints: Array<CreativeConstraint<T>> = [];

  constructor(
    initial: T,
    minCoherence: Coherence = 0.6,
    maxCoherence: Coherence = 0.95
  ) {
    this.current = initial;
    this.minCoherence = minCoherence;
    this.maxCoherence = maxCoherence;
  }

  addConstraint(constraint: CreativeConstraint<T>): void {
    this.constraints.push(constraint);
  }

  create(
    varyFn: (element: T, magnitude: number) => T,
    distanceFn: (a: T, b: T) => number,
    magnitude: number
  ): CreativeResult<T> {
    if (this.explorationBudget <= 0) {
      return { type: 'budgetExhausted' };
    }

    if (this.coherence > this.maxCoherence) {
      return { type: 'tooBoring', coherence: this.coherence };
    }

    const candidate = varyFn(this.current, magnitude);

    // Check constraints
    const newCoherence = this.calculateCoherence(candidate);

    if (newCoherence < this.minCoherence) {
      this.explorationBudget -= 0.5;
      return {
        type: 'rejected',
        attempted: candidate,
        reason: `Coherence would drop to ${newCoherence.toFixed(3)} (min: ${this.minCoherence.toFixed(3)})`,
      };
    }

    const novelty = distanceFn(this.current, candidate);
    this.current = candidate;
    this.coherence = newCoherence;
    this.explorationBudget -= magnitude;

    return { type: 'created', element: candidate, novelty, coherence: newCoherence };
  }

  private calculateCoherence(element: T): Coherence {
    if (this.constraints.length === 0) return 1.0;

    const satisfactions = this.constraints.map((c) => c.satisfaction(element));
    const product = satisfactions.reduce((a, b) => a * b, 1);
    return Math.pow(product, 1 / satisfactions.length);
  }

  rest(amount: number): void {
    this.explorationBudget = Math.min(20.0, this.explorationBudget + amount);
  }

  getCurrent(): T {
    return this.current;
  }

  getCoherence(): Coherence {
    return this.coherence;
  }
}

/**
 * Anti-cascade financial system
 */
export class AntiCascadeFinancialSystem {
  private participants: Map<string, Participant> = new Map();
  private positions: Position[] = [];
  private coherence: Coherence = 1.0;
  private circuitBreaker: CircuitBreakerState = 'open';

  addParticipant(id: string, capital: number): void {
    this.participants.set(id, {
      id,
      capital,
      exposure: 0,
      riskRating: 0,
      interconnectedness: 0,
    });
  }

  processTransaction(tx: Transaction): FinancialTransactionResult {
    this.updateCircuitBreaker();

    if (this.circuitBreaker === 'halted') {
      return { type: 'systemHalted' };
    }

    // Simplified processing
    const predictedImpact = this.predictCoherenceImpact(tx);
    const predictedCoherence = this.coherence + predictedImpact;

    if (predictedCoherence < 0.3) {
      return {
        type: 'rejected',
        reason: `Transaction would reduce coherence to ${predictedCoherence.toFixed(3)}`,
      };
    }

    this.coherence = predictedCoherence;
    return { type: 'executed', coherenceImpact: predictedImpact, feeMultiplier: 1.0 };
  }

  private predictCoherenceImpact(tx: Transaction): number {
    switch (tx.transactionType.type) {
      case 'transfer':
        return 0;
      case 'openLeverage':
        return -0.01 * tx.transactionType.leverage;
      case 'closePosition':
        return 0.02;
      case 'createDerivative':
        return -0.05;
      case 'marginCall':
        return 0.03;
    }
  }

  private updateCircuitBreaker(): void {
    if (this.coherence >= 0.7) {
      this.circuitBreaker = 'open';
    } else if (this.coherence >= 0.5) {
      this.circuitBreaker = 'cautious';
    } else if (this.coherence >= 0.3) {
      this.circuitBreaker = 'restricted';
    } else {
      this.circuitBreaker = 'halted';
    }
  }

  getCoherence(): Coherence {
    return this.coherence;
  }

  getCircuitBreakerState(): CircuitBreakerState {
    return this.circuitBreaker;
  }
}

/**
 * Gracefully aging distributed system
 */
export class GracefullyAgingSystem {
  private startTime: number = Date.now();
  private nodes: Map<string, Node> = new Map();
  private capabilities: Set<Capability>;
  private coherence: Coherence = 1.0;
  private conservatism: number = 0;
  private ageThresholds: AgeThreshold[];

  constructor() {
    this.capabilities = new Set([
      'acceptWrites',
      'complexQueries',
      'rebalancing',
      'scaleOut',
      'scaleIn',
      'schemaMigration',
      'newConnections',
      'basicReads',
      'healthMonitoring',
    ]);

    this.ageThresholds = [
      { age: 300000, removeCapabilities: ['schemaMigration'], coherenceFloor: 0.9, conservatismIncrease: 0.1 },
      { age: 600000, removeCapabilities: ['scaleOut', 'rebalancing'], coherenceFloor: 0.8, conservatismIncrease: 0.15 },
      { age: 900000, removeCapabilities: ['complexQueries'], coherenceFloor: 0.7, conservatismIncrease: 0.2 },
    ];
  }

  addNode(id: string, isPrimary: boolean): void {
    this.nodes.set(id, { id, health: 1.0, load: 0, isPrimary, stateSize: 0 });
  }

  getAge(): number {
    return Date.now() - this.startTime;
  }

  simulateAge(durationMs: number): void {
    this.coherence = Math.max(0, this.coherence - 0.0001 * (durationMs / 1000));
    this.applyAgeEffects(this.getAge() + durationMs);
  }

  private applyAgeEffects(age: number): void {
    for (const threshold of this.ageThresholds) {
      if (age >= threshold.age) {
        for (const cap of threshold.removeCapabilities) {
          this.capabilities.delete(cap);
        }
        this.conservatism = Math.min(1, this.conservatism + threshold.conservatismIncrease);
      }
    }
  }

  hasCapability(cap: Capability): boolean {
    return this.capabilities.has(cap);
  }

  attemptOperation(operation: AgingSystemOperation): AgingOperationResult {
    const requiredCap = this.getRequiredCapability(operation);

    if (!this.hasCapability(requiredCap)) {
      return { type: 'systemTooOld', age: this.getAge(), capability: requiredCap };
    }

    if (this.coherence < this.getMinCoherence(operation)) {
      return { type: 'deniedByCoherence', coherence: this.coherence };
    }

    return { type: 'success', latencyPenalty: 1 + this.conservatism * 2 };
  }

  private getRequiredCapability(op: AgingSystemOperation): Capability {
    switch (op.type) {
      case 'read': return 'basicReads';
      case 'write': return 'acceptWrites';
      case 'complexQuery': return 'complexQueries';
      case 'addNode': return 'scaleOut';
      case 'removeNode': return 'scaleIn';
      case 'rebalance': return 'rebalancing';
      case 'migrateSchema': return 'schemaMigration';
      case 'newConnection': return 'newConnections';
    }
  }

  private getMinCoherence(op: AgingSystemOperation): Coherence {
    switch (op.type) {
      case 'read': return 0.1;
      case 'write': return 0.4;
      case 'complexQuery': return 0.5;
      case 'addNode': return 0.7;
      case 'removeNode': return 0.5;
      case 'rebalance': return 0.6;
      case 'migrateSchema': return 0.8;
      case 'newConnection': return 0.3;
    }
  }

  getCoherence(): Coherence {
    return this.coherence;
  }

  getActiveNodes(): number {
    return Array.from(this.nodes.values()).filter((n) => n.health > 0).length;
  }
}

/**
 * Coherent swarm intelligence system
 */
export class CoherentSwarm {
  private agents: Map<string, SwarmAgent> = new Map();
  private minCoherence: Coherence;
  private coherence: Coherence = 1.0;
  private bounds: SpatialBounds;
  private weights: CoherenceWeights;
  private maxDivergence: number = 50;

  constructor(minCoherence: Coherence = 0.6) {
    this.minCoherence = minCoherence;
    this.bounds = { minX: -100, maxX: 100, minY: -100, maxY: 100 };
    this.weights = { cohesion: 0.3, alignment: 0.3, goalConsistency: 0.2, energyBalance: 0.2 };
  }

  addAgent(id: string, position: [number, number]): void {
    this.agents.set(id, {
      id,
      position,
      velocity: [0, 0],
      goal: position,
      energy: 100,
      neighborCount: 0,
    });
    this.coherence = this.calculateCoherence();
  }

  private calculateCoherence(): Coherence {
    if (this.agents.size < 2) return 1.0;

    const cohesion = this.calculateCohesion();
    const alignment = this.calculateAlignment();

    return Math.max(0, Math.min(1, (cohesion * this.weights.cohesion + alignment * this.weights.alignment) /
      (this.weights.cohesion + this.weights.alignment)));
  }

  private calculateCohesion(): number {
    const centroid = this.getCentroid();
    let totalDistance = 0;

    for (const agent of this.agents.values()) {
      const dx = agent.position[0] - centroid[0];
      const dy = agent.position[1] - centroid[1];
      totalDistance += Math.sqrt(dx * dx + dy * dy);
    }

    const avgDistance = totalDistance / this.agents.size;
    return Math.max(0, 1 - avgDistance / this.maxDivergence);
  }

  private calculateAlignment(): number {
    if (this.agents.size < 2) return 1.0;
    // Simplified alignment calculation
    return 0.8;
  }

  getCentroid(): [number, number] {
    if (this.agents.size === 0) return [0, 0];

    let sumX = 0, sumY = 0;
    for (const agent of this.agents.values()) {
      sumX += agent.position[0];
      sumY += agent.position[1];
    }

    return [sumX / this.agents.size, sumY / this.agents.size];
  }

  executeAction(agentId: string, action: SwarmAction): SwarmActionResult {
    const agent = this.agents.get(agentId);
    if (!agent) {
      return { type: 'rejected', reason: `Agent ${agentId} not found` };
    }

    const predictedCoherence = this.predictCoherence(agentId, action);

    if (predictedCoherence < this.minCoherence) {
      return {
        type: 'rejected',
        reason: `Action would reduce coherence to ${predictedCoherence.toFixed(3)} (min: ${this.minCoherence.toFixed(3)})`,
      };
    }

    this.applyAction(agentId, action);
    this.coherence = this.calculateCoherence();

    return { type: 'executed' };
  }

  private predictCoherence(agentId: string, action: SwarmAction): Coherence {
    // Simplified prediction
    if (action.type === 'move') {
      const magnitude = Math.sqrt(action.dx * action.dx + action.dy * action.dy);
      return Math.max(0, this.coherence - magnitude * 0.01);
    }
    return this.coherence;
  }

  private applyAction(agentId: string, action: SwarmAction): void {
    const agent = this.agents.get(agentId);
    if (!agent) return;

    switch (action.type) {
      case 'move':
        agent.position[0] = Math.max(this.bounds.minX, Math.min(this.bounds.maxX, agent.position[0] + action.dx));
        agent.position[1] = Math.max(this.bounds.minY, Math.min(this.bounds.maxY, agent.position[1] + action.dy));
        break;
      case 'accelerate':
        agent.velocity[0] += action.dvx;
        agent.velocity[1] += action.dvy;
        break;
      case 'setGoal':
        agent.goal = [action.x, action.y];
        break;
      case 'idle':
        break;
    }
  }

  tick(): void {
    for (const agent of this.agents.values()) {
      agent.position[0] += agent.velocity[0];
      agent.position[1] += agent.velocity[1];
      agent.energy = Math.max(0, agent.energy - 0.1);
    }
    this.coherence = this.calculateCoherence();
  }

  getCoherence(): Coherence {
    return this.coherence;
  }
}

/**
 * Graceful shutdown system
 */
export class GracefulSystem {
  private state: GracefulSystemState = 'running';
  private coherence: Coherence = 1.0;
  private shutdownPreparation: number = 0;
  private resources: Resource[] = [];
  private hooks: ShutdownHook[] = [];

  addResource(name: string, priority: number): void {
    this.resources.push({ name, cleanupPriority: priority, isCleaned: false });
  }

  addShutdownHook(hook: ShutdownHook): void {
    this.hooks.push(hook);
  }

  canAcceptWork(): boolean {
    return (this.state === 'running' || this.state === 'degraded') && this.coherence >= 0.4;
  }

  async operate<T>(operation: () => T | Promise<T>): Promise<T> {
    if (this.state === 'terminated') {
      throw new Error('System terminated');
    }

    if (this.state === 'shuttingDown') {
      throw new Error('System shutting down');
    }

    const result = await operation();
    this.updateState();
    return result;
  }

  private updateState(): void {
    if (this.coherence >= 0.6) {
      if (this.shutdownPreparation < 0.5) {
        this.state = 'running';
      }
    } else if (this.coherence >= 0.4) {
      this.state = 'degraded';
      this.shutdownPreparation += 0.1 * (1 - this.coherence);
    } else if (this.coherence >= 0.2) {
      this.state = 'shuttingDown';
    } else {
      this.state = 'terminated';
    }
  }

  applyCoherenceChange(delta: number): void {
    this.coherence = Math.max(0, Math.min(1, this.coherence + delta));
    this.updateState();
  }

  async progressShutdown(): Promise<boolean> {
    if (this.state !== 'shuttingDown') return false;

    // Clean resources
    for (const resource of this.resources) {
      if (!resource.isCleaned) {
        resource.isCleaned = true;
        return true;
      }
    }

    // Execute hooks
    for (const hook of this.hooks.sort((a, b) => b.priority - a.priority)) {
      await hook.execute();
    }

    this.state = 'terminated';
    return true;
  }

  getState(): GracefulSystemState {
    return this.state;
  }

  getCoherence(): Coherence {
    return this.coherence;
  }
}
