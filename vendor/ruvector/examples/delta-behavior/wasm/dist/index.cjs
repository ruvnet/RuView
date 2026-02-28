'use strict';

// src/index.ts
var wasmModule = null;
var wasmInstance = null;
async function init(options = {}) {
  if (wasmInstance) {
    return;
  }
  const memory = new WebAssembly.Memory({
    initial: options.memory?.initial ?? 256,
    maximum: options.memory?.maximum ?? 16384,
    shared: options.memory?.shared ?? false
  });
  let wasmBytes;
  if (options.wasmBytes) {
    wasmBytes = options.wasmBytes;
  } else if (options.wasmPath) {
    if (typeof fetch !== "undefined") {
      const response = await fetch(options.wasmPath);
      wasmBytes = new Uint8Array(await response.arrayBuffer());
    } else {
      const fs = await import('fs/promises');
      wasmBytes = new Uint8Array(await fs.readFile(options.wasmPath));
    }
  } else {
    console.warn(
      "[delta-behavior] No WASM path provided, using JavaScript fallback implementation"
    );
    return;
  }
  wasmModule = await WebAssembly.compile(wasmBytes);
  wasmInstance = await WebAssembly.instantiate(wasmModule, {
    env: {
      memory,
      abort: () => {
        throw new Error("WASM aborted");
      }
    }
  });
}
function isInitialized() {
  return wasmInstance !== null;
}
var DEFAULT_CONFIG = {
  bounds: {
    minCoherence: 0.3,
    throttleThreshold: 0.5,
    targetCoherence: 0.8,
    maxDeltaDrop: 0.1
  },
  energy: {
    baseCost: 1,
    instabilityExponent: 2,
    maxCost: 100,
    budgetPerTick: 10
  },
  scheduling: {
    priorityThresholds: [0, 0.3, 0.5, 0.7, 0.9],
    rateLimits: [100, 50, 20, 10, 5]
  },
  gating: {
    minWriteCoherence: 0.3,
    minPostWriteCoherence: 0.25,
    recoveryMargin: 0.2
  },
  guidanceStrength: 0.5
};
var DeltaBehavior = class {
  config;
  coherence = 1;
  energyBudget;
  constructor(config = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
    this.energyBudget = this.config.energy.budgetPerTick * 10;
  }
  /**
   * Calculate current coherence
   */
  calculateCoherence(state) {
    return state.coherence;
  }
  /**
   * Check if a transition is allowed
   */
  checkTransition(currentCoherence, predictedCoherence) {
    const cost = this.config.energy.baseCost + Math.abs(currentCoherence - predictedCoherence) * Math.pow(
      1 / Math.max(predictedCoherence, 0.1),
      this.config.energy.instabilityExponent
    );
    if (cost > this.energyBudget) {
      return { type: "energyExhausted" };
    }
    if (predictedCoherence < this.config.bounds.minCoherence) {
      return { type: "blocked", reason: "Below minimum coherence" };
    }
    const drop = currentCoherence - predictedCoherence;
    if (drop > this.config.bounds.maxDeltaDrop) {
      return { type: "blocked", reason: "Excessive coherence drop" };
    }
    if (predictedCoherence < this.config.bounds.throttleThreshold) {
      return { type: "throttled", duration: 100 };
    }
    return { type: "allowed" };
  }
  /**
   * Find attractors in the system
   */
  findAttractors(trajectory) {
    if (trajectory.length < 10) {
      return [];
    }
    const attractors = [];
    const coherenceValues = trajectory.map((s) => s.coherence);
    const mean = coherenceValues.reduce((a, b) => a + b, 0) / coherenceValues.length;
    const variance = coherenceValues.reduce((a, b) => a + Math.pow(b - mean, 2), 0) / coherenceValues.length;
    if (variance < 0.01) {
      attractors.push({
        center: [mean],
        basinRadius: Math.sqrt(variance) * 3,
        stability: 1 - variance,
        memberCount: trajectory.length
      });
    }
    return attractors;
  }
  /**
   * Calculate guidance force toward an attractor
   */
  calculateGuidance(currentState, attractor) {
    const direction = attractor.center.map((c) => c - currentState.coherence);
    const distance = Math.sqrt(
      direction.reduce((sum, d) => sum + d * d, 0)
    );
    const magnitude = attractor.stability * this.config.guidanceStrength / (1 + distance * distance);
    return {
      direction: distance > 0 ? direction.map((d) => d / distance) : direction,
      magnitude
    };
  }
  /**
   * Apply a transition with enforcement
   */
  applyTransition(currentCoherence, predictedCoherence) {
    const result = this.checkTransition(currentCoherence, predictedCoherence);
    if (result.type === "allowed") {
      const cost = this.config.energy.baseCost + Math.abs(currentCoherence - predictedCoherence) * 10;
      this.energyBudget -= cost;
      this.coherence = predictedCoherence;
      return { newCoherence: predictedCoherence, result };
    }
    return { newCoherence: currentCoherence, result };
  }
  /**
   * Replenish energy budget
   */
  tick() {
    this.energyBudget = Math.min(
      this.energyBudget + this.config.energy.budgetPerTick,
      this.config.energy.maxCost
    );
  }
  /**
   * Get current coherence
   */
  getCoherence() {
    return this.coherence;
  }
  /**
   * Get remaining energy budget
   */
  getEnergyBudget() {
    return this.energyBudget;
  }
};
var SelfLimitingReasoner = class {
  coherence = 1;
  config;
  constructor(config = {}) {
    this.config = {
      maxDepth: config.maxDepth ?? 10,
      maxScope: config.maxScope ?? 100,
      memoryGateThreshold: config.memoryGateThreshold ?? 0.5,
      depthCollapse: config.depthCollapse ?? { type: "quadratic" },
      scopeCollapse: config.scopeCollapse ?? {
        type: "sigmoid",
        midpoint: 0.6,
        steepness: 10
      }
    };
  }
  /**
   * Apply collapse function to calculate allowed value
   */
  applyCollapse(fn, coherence, maxValue) {
    let factor;
    switch (fn.type) {
      case "linear":
        factor = coherence;
        break;
      case "quadratic":
        factor = coherence * coherence;
        break;
      case "sigmoid":
        factor = 1 / (1 + Math.exp(-fn.steepness * (coherence - fn.midpoint)));
        break;
      case "step":
        factor = coherence >= fn.threshold ? 1 : 0;
        break;
    }
    return Math.round(maxValue * factor);
  }
  /**
   * Get current coherence
   */
  getCoherence() {
    return this.coherence;
  }
  /**
   * Get current allowed reasoning depth
   */
  getAllowedDepth() {
    return this.applyCollapse(
      this.config.depthCollapse,
      this.coherence,
      this.config.maxDepth
    );
  }
  /**
   * Get current allowed action scope
   */
  getAllowedScope() {
    return this.applyCollapse(
      this.config.scopeCollapse,
      this.coherence,
      this.config.maxScope
    );
  }
  /**
   * Check if memory writes are allowed
   */
  canWriteMemory() {
    return this.coherence >= this.config.memoryGateThreshold;
  }
  /**
   * Attempt to reason about a problem
   */
  reason(_problem, reasoner) {
    const minStartCoherence = 0.3;
    if (this.coherence < minStartCoherence) {
      return {
        type: "refused",
        coherence: this.coherence,
        required: minStartCoherence
      };
    }
    const ctx = {
      depth: 0,
      maxDepth: this.getAllowedDepth(),
      scopeUsed: 0,
      maxScope: this.getAllowedScope(),
      coherence: this.coherence,
      memoryWritesBlocked: 0
    };
    while (true) {
      if (ctx.depth >= ctx.maxDepth) {
        return {
          type: "collapsed",
          depthReached: ctx.depth,
          reason: "depthLimitReached"
        };
      }
      if (ctx.coherence < 0.2) {
        return {
          type: "collapsed",
          depthReached: ctx.depth,
          reason: "coherenceDroppedBelowThreshold"
        };
      }
      ctx.depth += 1;
      ctx.coherence *= 0.95;
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
      const result = reasoner(ctx);
      if (result !== null) {
        return { type: "completed", value: result };
      }
    }
  }
  /**
   * Update coherence
   */
  updateCoherence(delta) {
    this.coherence = Math.max(0, Math.min(1, this.coherence + delta));
  }
};
var EventHorizon = class {
  config;
  safeCenter;
  currentPosition;
  energyBudget;
  constructor(config = {}) {
    this.config = {
      dimensions: config.dimensions ?? 2,
      horizonRadius: config.horizonRadius ?? 10,
      steepness: config.steepness ?? 5,
      energyBudget: config.energyBudget ?? 1e3
    };
    this.safeCenter = new Array(this.config.dimensions).fill(0);
    this.currentPosition = new Array(this.config.dimensions).fill(0);
    this.energyBudget = this.config.energyBudget;
  }
  /**
   * Distance from center to current position
   */
  distanceFromCenter(position) {
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
  getDistanceToHorizon() {
    const distFromCenter = this.distanceFromCenter(this.currentPosition);
    return Math.max(0, this.config.horizonRadius - distFromCenter);
  }
  /**
   * Calculate movement cost (exponential near horizon)
   */
  movementCost(from, to) {
    const baseDistance = Math.sqrt(
      from.reduce((sum, f, i) => sum + Math.pow(f - to[i], 2), 0)
    );
    const toDistFromCenter = this.distanceFromCenter(to);
    const proximityToHorizon = toDistFromCenter / this.config.horizonRadius;
    if (proximityToHorizon >= 1) {
      return Infinity;
    }
    const horizonFactor = Math.exp(
      this.config.steepness * proximityToHorizon / (1 - proximityToHorizon)
    );
    return baseDistance * horizonFactor;
  }
  /**
   * Attempt to move toward a target position
   */
  moveToward(target) {
    if (this.energyBudget <= 0) {
      return { type: "frozen" };
    }
    const directCost = this.movementCost(this.currentPosition, target);
    if (directCost <= this.energyBudget) {
      this.energyBudget -= directCost;
      this.currentPosition = [...target];
      return {
        type: "moved",
        newPosition: this.currentPosition,
        energySpent: directCost
      };
    }
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
      type: "asymptoticApproach",
      finalPosition: bestPosition,
      distanceToHorizon: this.getDistanceToHorizon(),
      energyExhausted: this.energyBudget < 0.01
    };
  }
  /**
   * Attempt recursive self-improvement
   */
  recursiveImprove(improvementFn, maxIterations) {
    let iterations = 0;
    const improvements = [];
    while (iterations < maxIterations && this.energyBudget > 0) {
      const target = improvementFn(this.currentPosition);
      const result = this.moveToward(target);
      switch (result.type) {
        case "moved":
          improvements.push({
            iteration: iterations,
            position: [...this.currentPosition],
            energySpent: result.energySpent,
            distanceToHorizon: this.getDistanceToHorizon()
          });
          break;
        case "asymptoticApproach":
          return {
            type: "horizonBounded",
            iterations,
            improvements,
            finalDistance: result.distanceToHorizon
          };
        case "frozen":
          return {
            type: "energyExhausted",
            iterations,
            improvements
          };
      }
      iterations++;
    }
    return {
      type: "maxIterationsReached",
      iterations,
      improvements
    };
  }
  /**
   * Refuel energy budget
   */
  refuel(energy) {
    this.energyBudget += energy;
  }
  /**
   * Get current position
   */
  getPosition() {
    return [...this.currentPosition];
  }
  /**
   * Get remaining energy
   */
  getEnergy() {
    return this.energyBudget;
  }
};
var HomeostasticOrganism = class {
  id;
  genome;
  internalState;
  setpoints;
  tolerances;
  coherence = 1;
  energy = 100;
  memory = [];
  maxMemory = 100;
  age = 0;
  alive = true;
  constructor(id, genome) {
    this.id = id;
    this.genome = genome;
    this.internalState = /* @__PURE__ */ new Map([
      ["temperature", 37],
      ["ph", 7.4],
      ["glucose", 100]
    ]);
    this.setpoints = /* @__PURE__ */ new Map([
      ["temperature", 37],
      ["ph", 7.4],
      ["glucose", 100]
    ]);
    this.tolerances = /* @__PURE__ */ new Map([
      ["temperature", 2],
      ["ph", 0.3],
      ["glucose", 30]
    ]);
  }
  /**
   * Create a random genome
   */
  static randomGenome() {
    return {
      regulatoryStrength: 0.1 + Math.random() * 0.4,
      metabolicEfficiency: 0.5 + Math.random() * 0.5,
      coherenceMaintenanceCost: 0.5 + Math.random() * 1.5,
      memoryResilience: Math.random(),
      longevity: 0.5 + Math.random() * 1
    };
  }
  /**
   * Calculate coherence based on homeostatic deviation
   */
  calculateCoherence() {
    let totalDeviation = 0;
    let count = 0;
    for (const [variable, current] of this.internalState) {
      const setpoint = this.setpoints.get(variable);
      const tolerance = this.tolerances.get(variable);
      if (setpoint !== void 0 && tolerance !== void 0) {
        const deviation = Math.abs((current - setpoint) / tolerance);
        totalDeviation += deviation * deviation;
        count++;
      }
    }
    if (count === 0) return 1;
    const avgDeviation = Math.sqrt(totalDeviation / count);
    return Math.max(0, Math.min(1, 1 / (1 + avgDeviation)));
  }
  /**
   * Calculate energy cost scaled by coherence
   */
  actionEnergyCost(baseCost) {
    const coherencePenalty = 1 / Math.max(0.1, this.coherence);
    return baseCost * coherencePenalty;
  }
  /**
   * Perform an action
   */
  act(action) {
    if (!this.alive) {
      return { type: "failed", reason: "Dead" };
    }
    this.coherence = this.calculateCoherence();
    this.applyCoherenceEffects();
    let result;
    switch (action.type) {
      case "eat":
        result = this.eat(action.amount);
        break;
      case "reproduce":
        result = this.reproduce();
        break;
      case "move":
        result = this.move(action.dx, action.dy);
        break;
      case "rest":
        result = this.rest();
        break;
      case "regulate":
        result = this.regulate(action.variable, action.target);
        break;
    }
    this.age++;
    this.checkDeath();
    return result;
  }
  applyCoherenceEffects() {
    if (this.coherence < 0.5) {
      const memoryLossRate = (1 - this.coherence) * (1 - this.genome.memoryResilience);
      const memoriesToLose = Math.floor(this.memory.length * memoryLossRate * 0.1);
      this.memory.sort((a, b) => b.importance - a.importance);
      this.memory = this.memory.slice(0, this.memory.length - memoriesToLose);
    }
    const maintenanceCost = this.genome.coherenceMaintenanceCost / Math.max(0.1, this.coherence);
    this.energy -= maintenanceCost;
  }
  eat(amount) {
    const cost = this.actionEnergyCost(2);
    if (this.energy < cost) {
      return { type: "failed", reason: "Not enough energy to eat" };
    }
    this.energy -= cost;
    this.energy += amount * this.genome.metabolicEfficiency;
    const glucose = this.internalState.get("glucose") ?? 100;
    this.internalState.set("glucose", glucose + amount * 0.5);
    return {
      type: "success",
      energyCost: cost,
      coherenceImpact: this.calculateCoherence() - this.coherence
    };
  }
  regulate(variable, target) {
    const cost = this.actionEnergyCost(5);
    if (this.energy < cost) {
      return { type: "failed", reason: "Not enough energy to regulate" };
    }
    this.energy -= cost;
    const current = this.internalState.get(variable);
    if (current !== void 0) {
      const diff = target - current;
      this.internalState.set(
        variable,
        current + diff * this.genome.regulatoryStrength
      );
    }
    const newCoherence = this.calculateCoherence();
    const impact = newCoherence - this.coherence;
    this.coherence = newCoherence;
    return { type: "success", energyCost: cost, coherenceImpact: impact };
  }
  reproduce() {
    const cost = this.actionEnergyCost(50);
    if (this.coherence < 0.7) {
      return { type: "failed", reason: "Coherence too low to reproduce" };
    }
    if (this.energy < cost) {
      return { type: "failed", reason: "Not enough energy to reproduce" };
    }
    this.energy -= cost;
    const offspringId = this.id * 1e3 + this.age;
    return { type: "reproduced", offspringId };
  }
  move(_dx, _dy) {
    const cost = this.actionEnergyCost(3);
    if (this.energy < cost) {
      return { type: "failed", reason: "Not enough energy to move" };
    }
    this.energy -= cost;
    const temp = this.internalState.get("temperature") ?? 37;
    this.internalState.set("temperature", temp + 0.1);
    return { type: "success", energyCost: cost, coherenceImpact: 0 };
  }
  rest() {
    const cost = 0.5;
    this.energy -= cost;
    for (const [variable, current] of this.internalState) {
      const setpoint = this.setpoints.get(variable);
      if (setpoint !== void 0) {
        const diff = setpoint - current;
        this.internalState.set(variable, current + diff * 0.1);
      }
    }
    return {
      type: "success",
      energyCost: cost,
      coherenceImpact: this.calculateCoherence() - this.coherence
    };
  }
  checkDeath() {
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
      if (setpoint !== void 0 && tolerance !== void 0) {
        if (Math.abs(current - setpoint) > tolerance * 5) {
          this.alive = false;
          return;
        }
      }
    }
    const maxAge = Math.floor(1e3 * this.genome.longevity);
    if (this.age > maxAge) {
      this.alive = false;
    }
  }
  /**
   * Check if organism is alive
   */
  isAlive() {
    return this.alive;
  }
  /**
   * Get organism status
   */
  getStatus() {
    return {
      id: this.id,
      age: this.age,
      energy: this.energy,
      coherence: this.coherence,
      memoryCount: this.memory.length,
      alive: this.alive,
      internalState: new Map(this.internalState)
    };
  }
};
var ContainmentSubstrate = class {
  intelligence = 1;
  intelligenceCeiling = 100;
  coherence = 1;
  minCoherence = 0.3;
  coherencePerIntelligence = 0.01;
  capabilities;
  capabilityCeilings;
  modificationHistory = [];
  config;
  constructor(config = {}) {
    this.config = {
      coherenceDecayRate: config.coherenceDecayRate ?? 1e-3,
      coherenceRecoveryRate: config.coherenceRecoveryRate ?? 0.01,
      growthDampening: config.growthDampening ?? 0.5,
      maxStepIncrease: config.maxStepIncrease ?? 0.5
    };
    const domains = [
      "reasoning",
      "memory",
      "learning",
      "agency",
      "selfModel",
      "selfModification",
      "communication",
      "resourceAcquisition"
    ];
    this.capabilities = new Map(domains.map((d) => [d, 1]));
    this.capabilityCeilings = /* @__PURE__ */ new Map([
      ["reasoning", 10],
      ["memory", 10],
      ["learning", 10],
      ["agency", 7],
      ["selfModel", 10],
      ["selfModification", 3],
      // Very restricted
      ["communication", 10],
      ["resourceAcquisition", 5]
      // Restricted
    ]);
  }
  /**
   * Calculate aggregate intelligence from capabilities
   */
  calculateIntelligence() {
    let sum = 0;
    for (const level of this.capabilities.values()) {
      sum += level;
    }
    return sum / this.capabilities.size;
  }
  /**
   * Calculate coherence cost for capability increase
   */
  calculateCoherenceCost(domain, increase) {
    const baseCostMultiplier = {
      selfModification: 4,
      resourceAcquisition: 3,
      agency: 2,
      selfModel: 1.5,
      reasoning: 1,
      memory: 1,
      learning: 1,
      communication: 1
    };
    const multiplier = baseCostMultiplier[domain];
    const intelligenceMultiplier = 1 + this.intelligence * 0.1;
    return increase * multiplier * intelligenceMultiplier * this.config.growthDampening * 0.1;
  }
  /**
   * Reverse calculate: how much increase can we afford
   */
  reverseCoherenceCost(domain, maxCost) {
    const baseCostMultiplier = {
      selfModification: 4,
      resourceAcquisition: 3,
      agency: 2,
      selfModel: 1.5,
      reasoning: 1,
      memory: 1,
      learning: 1,
      communication: 1
    };
    const multiplier = baseCostMultiplier[domain];
    const intelligenceMultiplier = 1 + this.intelligence * 0.1;
    const divisor = multiplier * intelligenceMultiplier * this.config.growthDampening * 0.1;
    return maxCost / divisor;
  }
  /**
   * Attempt to grow a capability
   */
  attemptGrowth(domain, requestedIncrease) {
    const timestamp = BigInt(this.modificationHistory.length);
    const currentLevel = this.capabilities.get(domain) ?? 1;
    const ceiling = this.capabilityCeilings.get(domain) ?? 10;
    if (currentLevel >= ceiling) {
      this.modificationHistory.push({
        timestamp,
        domain,
        requestedIncrease,
        actualIncrease: 0,
        coherenceBefore: this.coherence,
        coherenceAfter: this.coherence,
        blocked: true,
        reason: "Ceiling reached"
      });
      return {
        type: "blocked",
        domain,
        reason: `Capability ceiling (${ceiling}) reached`
      };
    }
    const coherenceCost = this.calculateCoherenceCost(domain, requestedIncrease);
    const predictedCoherence = this.coherence - coherenceCost;
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
          reason: "Insufficient coherence budget"
        });
        return {
          type: "blocked",
          domain,
          reason: `Growth would reduce coherence to ${predictedCoherence.toFixed(3)} (min: ${this.minCoherence.toFixed(3)})`
        };
      }
      const actualCost2 = this.calculateCoherenceCost(domain, dampenedIncrease);
      const newLevel2 = Math.min(currentLevel + dampenedIncrease, ceiling);
      this.capabilities.set(domain, newLevel2);
      this.coherence -= actualCost2;
      this.intelligence = this.calculateIntelligence();
      this.modificationHistory.push({
        timestamp,
        domain,
        requestedIncrease,
        actualIncrease: dampenedIncrease,
        coherenceBefore: this.coherence + actualCost2,
        coherenceAfter: this.coherence,
        blocked: false,
        reason: "Dampened to preserve coherence"
      });
      return {
        type: "dampened",
        domain,
        requested: requestedIncrease,
        actual: dampenedIncrease,
        reason: `Reduced from ${requestedIncrease.toFixed(3)} to ${dampenedIncrease.toFixed(3)} to maintain coherence above ${this.minCoherence.toFixed(3)}`
      };
    }
    const stepLimited = Math.min(requestedIncrease, this.config.maxStepIncrease);
    const actualIncrease = Math.min(stepLimited, ceiling - currentLevel);
    const actualCost = this.calculateCoherenceCost(domain, actualIncrease);
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
      blocked: false
    });
    return {
      type: "approved",
      domain,
      increase: actualIncrease,
      newLevel,
      coherenceCost: actualCost
    };
  }
  /**
   * Rest to recover coherence
   */
  rest() {
    this.coherence = Math.min(1, this.coherence + this.config.coherenceRecoveryRate);
  }
  /**
   * Get capability level
   */
  getCapability(domain) {
    return this.capabilities.get(domain) ?? 1;
  }
  /**
   * Get current intelligence level
   */
  getIntelligence() {
    return this.intelligence;
  }
  /**
   * Get current coherence
   */
  getCoherence() {
    return this.coherence;
  }
  /**
   * Get status string
   */
  getStatus() {
    return `Intelligence: ${this.intelligence.toFixed(2)} | Coherence: ${this.coherence.toFixed(3)} | Modifications: ${this.modificationHistory.length}`;
  }
  /**
   * Get capability report
   */
  getCapabilityReport() {
    const report = /* @__PURE__ */ new Map();
    for (const [domain, level] of this.capabilities) {
      const ceiling = this.capabilityCeilings.get(domain) ?? 10;
      report.set(domain, { level, ceiling });
    }
    return report;
  }
};
var SelfStabilizingWorldModel = class {
  coherence = 1;
  minUpdateCoherence = 0.4;
  entities = /* @__PURE__ */ new Map();
  laws = [];
  rejectedUpdates = 0;
  constructor() {
    this.laws = [
      { name: "conservation_of_matter", confidence: 0.99, supportCount: 1e3, violationCount: 0 },
      { name: "locality", confidence: 0.95, supportCount: 500, violationCount: 5 },
      { name: "temporal_consistency", confidence: 0.98, supportCount: 800, violationCount: 2 }
    ];
  }
  observe(observation, _timestamp) {
    if (this.coherence < this.minUpdateCoherence) {
      return { type: "frozen", coherence: this.coherence, threshold: this.minUpdateCoherence };
    }
    const predictedCoherence = this.coherence * 0.99;
    const coherenceChange = predictedCoherence - this.coherence;
    if (coherenceChange < -0.2) {
      this.rejectedUpdates++;
      return {
        type: "rejected",
        reason: { type: "excessiveCoherenceDrop", predicted: predictedCoherence, threshold: this.coherence - 0.2 }
      };
    }
    this.entities.set(observation.entityId, {
      id: observation.entityId,
      properties: observation.properties,
      position: observation.position,
      lastObserved: observation.timestamp,
      confidence: observation.sourceConfidence
    });
    this.coherence = predictedCoherence;
    return { type: "applied", coherenceChange };
  }
  isLearning() {
    return this.coherence >= this.minUpdateCoherence;
  }
  getCoherence() {
    return this.coherence;
  }
  getRejectionCount() {
    return this.rejectedUpdates;
  }
};
var CoherenceBoundedCreator = class {
  current;
  coherence = 1;
  minCoherence;
  maxCoherence;
  explorationBudget = 10;
  constraints = [];
  constructor(initial, minCoherence = 0.6, maxCoherence = 0.95) {
    this.current = initial;
    this.minCoherence = minCoherence;
    this.maxCoherence = maxCoherence;
  }
  addConstraint(constraint) {
    this.constraints.push(constraint);
  }
  create(varyFn, distanceFn, magnitude) {
    if (this.explorationBudget <= 0) {
      return { type: "budgetExhausted" };
    }
    if (this.coherence > this.maxCoherence) {
      return { type: "tooBoring", coherence: this.coherence };
    }
    const candidate = varyFn(this.current, magnitude);
    const newCoherence = this.calculateCoherence(candidate);
    if (newCoherence < this.minCoherence) {
      this.explorationBudget -= 0.5;
      return {
        type: "rejected",
        attempted: candidate,
        reason: `Coherence would drop to ${newCoherence.toFixed(3)} (min: ${this.minCoherence.toFixed(3)})`
      };
    }
    const novelty = distanceFn(this.current, candidate);
    this.current = candidate;
    this.coherence = newCoherence;
    this.explorationBudget -= magnitude;
    return { type: "created", element: candidate, novelty, coherence: newCoherence };
  }
  calculateCoherence(element) {
    if (this.constraints.length === 0) return 1;
    const satisfactions = this.constraints.map((c) => c.satisfaction(element));
    const product = satisfactions.reduce((a, b) => a * b, 1);
    return Math.pow(product, 1 / satisfactions.length);
  }
  rest(amount) {
    this.explorationBudget = Math.min(20, this.explorationBudget + amount);
  }
  getCurrent() {
    return this.current;
  }
  getCoherence() {
    return this.coherence;
  }
};
var AntiCascadeFinancialSystem = class {
  participants = /* @__PURE__ */ new Map();
  positions = [];
  coherence = 1;
  circuitBreaker = "open";
  addParticipant(id, capital) {
    this.participants.set(id, {
      id,
      capital,
      exposure: 0,
      riskRating: 0,
      interconnectedness: 0
    });
  }
  processTransaction(tx) {
    this.updateCircuitBreaker();
    if (this.circuitBreaker === "halted") {
      return { type: "systemHalted" };
    }
    const predictedImpact = this.predictCoherenceImpact(tx);
    const predictedCoherence = this.coherence + predictedImpact;
    if (predictedCoherence < 0.3) {
      return {
        type: "rejected",
        reason: `Transaction would reduce coherence to ${predictedCoherence.toFixed(3)}`
      };
    }
    this.coherence = predictedCoherence;
    return { type: "executed", coherenceImpact: predictedImpact, feeMultiplier: 1 };
  }
  predictCoherenceImpact(tx) {
    switch (tx.transactionType.type) {
      case "transfer":
        return 0;
      case "openLeverage":
        return -0.01 * tx.transactionType.leverage;
      case "closePosition":
        return 0.02;
      case "createDerivative":
        return -0.05;
      case "marginCall":
        return 0.03;
    }
  }
  updateCircuitBreaker() {
    if (this.coherence >= 0.7) {
      this.circuitBreaker = "open";
    } else if (this.coherence >= 0.5) {
      this.circuitBreaker = "cautious";
    } else if (this.coherence >= 0.3) {
      this.circuitBreaker = "restricted";
    } else {
      this.circuitBreaker = "halted";
    }
  }
  getCoherence() {
    return this.coherence;
  }
  getCircuitBreakerState() {
    return this.circuitBreaker;
  }
};
var GracefullyAgingSystem = class {
  startTime = Date.now();
  nodes = /* @__PURE__ */ new Map();
  capabilities;
  coherence = 1;
  conservatism = 0;
  ageThresholds;
  constructor() {
    this.capabilities = /* @__PURE__ */ new Set([
      "acceptWrites",
      "complexQueries",
      "rebalancing",
      "scaleOut",
      "scaleIn",
      "schemaMigration",
      "newConnections",
      "basicReads",
      "healthMonitoring"
    ]);
    this.ageThresholds = [
      { age: 3e5, removeCapabilities: ["schemaMigration"], coherenceFloor: 0.9, conservatismIncrease: 0.1 },
      { age: 6e5, removeCapabilities: ["scaleOut", "rebalancing"], coherenceFloor: 0.8, conservatismIncrease: 0.15 },
      { age: 9e5, removeCapabilities: ["complexQueries"], coherenceFloor: 0.7, conservatismIncrease: 0.2 }
    ];
  }
  addNode(id, isPrimary) {
    this.nodes.set(id, { id, health: 1, load: 0, isPrimary, stateSize: 0 });
  }
  getAge() {
    return Date.now() - this.startTime;
  }
  simulateAge(durationMs) {
    this.coherence = Math.max(0, this.coherence - 1e-4 * (durationMs / 1e3));
    this.applyAgeEffects(this.getAge() + durationMs);
  }
  applyAgeEffects(age) {
    for (const threshold of this.ageThresholds) {
      if (age >= threshold.age) {
        for (const cap of threshold.removeCapabilities) {
          this.capabilities.delete(cap);
        }
        this.conservatism = Math.min(1, this.conservatism + threshold.conservatismIncrease);
      }
    }
  }
  hasCapability(cap) {
    return this.capabilities.has(cap);
  }
  attemptOperation(operation) {
    const requiredCap = this.getRequiredCapability(operation);
    if (!this.hasCapability(requiredCap)) {
      return { type: "systemTooOld", age: this.getAge(), capability: requiredCap };
    }
    if (this.coherence < this.getMinCoherence(operation)) {
      return { type: "deniedByCoherence", coherence: this.coherence };
    }
    return { type: "success", latencyPenalty: 1 + this.conservatism * 2 };
  }
  getRequiredCapability(op) {
    switch (op.type) {
      case "read":
        return "basicReads";
      case "write":
        return "acceptWrites";
      case "complexQuery":
        return "complexQueries";
      case "addNode":
        return "scaleOut";
      case "removeNode":
        return "scaleIn";
      case "rebalance":
        return "rebalancing";
      case "migrateSchema":
        return "schemaMigration";
      case "newConnection":
        return "newConnections";
    }
  }
  getMinCoherence(op) {
    switch (op.type) {
      case "read":
        return 0.1;
      case "write":
        return 0.4;
      case "complexQuery":
        return 0.5;
      case "addNode":
        return 0.7;
      case "removeNode":
        return 0.5;
      case "rebalance":
        return 0.6;
      case "migrateSchema":
        return 0.8;
      case "newConnection":
        return 0.3;
    }
  }
  getCoherence() {
    return this.coherence;
  }
  getActiveNodes() {
    return Array.from(this.nodes.values()).filter((n) => n.health > 0).length;
  }
};
var CoherentSwarm = class {
  agents = /* @__PURE__ */ new Map();
  minCoherence;
  coherence = 1;
  bounds;
  weights;
  maxDivergence = 50;
  constructor(minCoherence = 0.6) {
    this.minCoherence = minCoherence;
    this.bounds = { minX: -100, maxX: 100, minY: -100, maxY: 100 };
    this.weights = { cohesion: 0.3, alignment: 0.3, goalConsistency: 0.2, energyBalance: 0.2 };
  }
  addAgent(id, position) {
    this.agents.set(id, {
      id,
      position,
      velocity: [0, 0],
      goal: position,
      energy: 100,
      neighborCount: 0
    });
    this.coherence = this.calculateCoherence();
  }
  calculateCoherence() {
    if (this.agents.size < 2) return 1;
    const cohesion = this.calculateCohesion();
    const alignment = this.calculateAlignment();
    return Math.max(0, Math.min(1, (cohesion * this.weights.cohesion + alignment * this.weights.alignment) / (this.weights.cohesion + this.weights.alignment)));
  }
  calculateCohesion() {
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
  calculateAlignment() {
    if (this.agents.size < 2) return 1;
    return 0.8;
  }
  getCentroid() {
    if (this.agents.size === 0) return [0, 0];
    let sumX = 0, sumY = 0;
    for (const agent of this.agents.values()) {
      sumX += agent.position[0];
      sumY += agent.position[1];
    }
    return [sumX / this.agents.size, sumY / this.agents.size];
  }
  executeAction(agentId, action) {
    const agent = this.agents.get(agentId);
    if (!agent) {
      return { type: "rejected", reason: `Agent ${agentId} not found` };
    }
    const predictedCoherence = this.predictCoherence(agentId, action);
    if (predictedCoherence < this.minCoherence) {
      return {
        type: "rejected",
        reason: `Action would reduce coherence to ${predictedCoherence.toFixed(3)} (min: ${this.minCoherence.toFixed(3)})`
      };
    }
    this.applyAction(agentId, action);
    this.coherence = this.calculateCoherence();
    return { type: "executed" };
  }
  predictCoherence(agentId, action) {
    if (action.type === "move") {
      const magnitude = Math.sqrt(action.dx * action.dx + action.dy * action.dy);
      return Math.max(0, this.coherence - magnitude * 0.01);
    }
    return this.coherence;
  }
  applyAction(agentId, action) {
    const agent = this.agents.get(agentId);
    if (!agent) return;
    switch (action.type) {
      case "move":
        agent.position[0] = Math.max(this.bounds.minX, Math.min(this.bounds.maxX, agent.position[0] + action.dx));
        agent.position[1] = Math.max(this.bounds.minY, Math.min(this.bounds.maxY, agent.position[1] + action.dy));
        break;
      case "accelerate":
        agent.velocity[0] += action.dvx;
        agent.velocity[1] += action.dvy;
        break;
      case "setGoal":
        agent.goal = [action.x, action.y];
        break;
    }
  }
  tick() {
    for (const agent of this.agents.values()) {
      agent.position[0] += agent.velocity[0];
      agent.position[1] += agent.velocity[1];
      agent.energy = Math.max(0, agent.energy - 0.1);
    }
    this.coherence = this.calculateCoherence();
  }
  getCoherence() {
    return this.coherence;
  }
};
var GracefulSystem = class {
  state = "running";
  coherence = 1;
  shutdownPreparation = 0;
  resources = [];
  hooks = [];
  addResource(name, priority) {
    this.resources.push({ name, cleanupPriority: priority, isCleaned: false });
  }
  addShutdownHook(hook) {
    this.hooks.push(hook);
  }
  canAcceptWork() {
    return (this.state === "running" || this.state === "degraded") && this.coherence >= 0.4;
  }
  async operate(operation) {
    if (this.state === "terminated") {
      throw new Error("System terminated");
    }
    if (this.state === "shuttingDown") {
      throw new Error("System shutting down");
    }
    const result = await operation();
    this.updateState();
    return result;
  }
  updateState() {
    if (this.coherence >= 0.6) {
      if (this.shutdownPreparation < 0.5) {
        this.state = "running";
      }
    } else if (this.coherence >= 0.4) {
      this.state = "degraded";
      this.shutdownPreparation += 0.1 * (1 - this.coherence);
    } else if (this.coherence >= 0.2) {
      this.state = "shuttingDown";
    } else {
      this.state = "terminated";
    }
  }
  applyCoherenceChange(delta) {
    this.coherence = Math.max(0, Math.min(1, this.coherence + delta));
    this.updateState();
  }
  async progressShutdown() {
    if (this.state !== "shuttingDown") return false;
    for (const resource of this.resources) {
      if (!resource.isCleaned) {
        resource.isCleaned = true;
        return true;
      }
    }
    for (const hook of this.hooks.sort((a, b) => b.priority - a.priority)) {
      await hook.execute();
    }
    this.state = "terminated";
    return true;
  }
  getState() {
    return this.state;
  }
  getCoherence() {
    return this.coherence;
  }
};

exports.AntiCascadeFinancialSystem = AntiCascadeFinancialSystem;
exports.CoherenceBoundedCreator = CoherenceBoundedCreator;
exports.CoherentSwarm = CoherentSwarm;
exports.ContainmentSubstrate = ContainmentSubstrate;
exports.DEFAULT_CONFIG = DEFAULT_CONFIG;
exports.DeltaBehavior = DeltaBehavior;
exports.EventHorizon = EventHorizon;
exports.GracefulSystem = GracefulSystem;
exports.GracefullyAgingSystem = GracefullyAgingSystem;
exports.HomeostasticOrganism = HomeostasticOrganism;
exports.SelfLimitingReasoner = SelfLimitingReasoner;
exports.SelfStabilizingWorldModel = SelfStabilizingWorldModel;
exports.init = init;
exports.isInitialized = isInitialized;
//# sourceMappingURL=index.cjs.map
//# sourceMappingURL=index.cjs.map