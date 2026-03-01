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

// ============================================================================
// Constants - Replace magic numbers with named constants
// ============================================================================

export const NEURAL_CONSTANTS = {
  // Drift Detection
  MAX_DRIFT_EVENTS: 1000,
  MAX_HISTORY_SIZE: 500,
  DEFAULT_DRIFT_THRESHOLD: 0.15,
  DEFAULT_DRIFT_WINDOW_MS: 60000,
  DRIFT_CRITICAL_MULTIPLIER: 2,
  VELOCITY_WINDOW_SIZE: 10,

  // Memory Physics
  MAX_MEMORIES: 10000,
  MAX_CONTENT_LENGTH: 10000,
  MAX_ID_LENGTH: 256,
  DEFAULT_MEMORY_DECAY_RATE: 0.01,
  DEFAULT_INTERFERENCE_THRESHOLD: 0.8,
  DEFAULT_CONSOLIDATION_RATE: 0.1,
  MEMORY_FORGET_THRESHOLD: 0.01,
  CONSOLIDATION_SCORE_THRESHOLD: 0.5,
  MEMORY_CLEANUP_PERCENT: 0.1,
  RECALL_STRENGTH_BOOST: 0.1,
  MAX_TIME_JUMP_MINUTES: 1440,

  // Agent State
  MAX_AGENTS: 1000,
  MAX_SPECIALTY_LENGTH: 100,
  AGENT_TIMEOUT_MS: 3600000, // 1 hour
  DEFAULT_AGENT_ENERGY: 1.0,
  TRAJECTORY_DAMPING: 0.1,
  MAX_TRAJECTORY_STEPS: 100,

  // Swarm Coordination
  MAX_CLUSTER_AGENTS: 500,
  DEFAULT_CLUSTER_THRESHOLD: 0.7,

  // Coherence Monitoring
  DEFAULT_WINDOW_SIZE: 100,
  MIN_CALIBRATION_OBSERVATIONS: 10,
  STABILITY_WINDOW_SIZE: 10,
  ALIGNMENT_WINDOW_SIZE: 50,
  RECENT_OBSERVATIONS_SIZE: 20,
  DRIFT_WARNING_THRESHOLD: 0.3,
  STABILITY_WARNING_THRESHOLD: 0.5,
  ALIGNMENT_WARNING_THRESHOLD: 0.6,
  COHERENCE_WARNING_THRESHOLD: 0.5,

  // Math
  EPSILON: 1e-8,
  ZERO_VECTOR_THRESHOLD: 1e-10,

  // Defaults
  DEFAULT_DIMENSION: 384,
  DEFAULT_REFLEX_LATENCY_MS: 10,
} as const;

// ============================================================================
// Logger Interface - Configurable logging
// ============================================================================

export type LogLevel = 'debug' | 'info' | 'warn' | 'error';

export interface NeuralLogger {
  log(level: LogLevel, message: string, data?: Record<string, unknown>): void;
}

/** Default console logger */
export const defaultLogger: NeuralLogger = {
  log(level: LogLevel, message: string, data?: Record<string, unknown>): void {
    const prefix = `[Neural:${level.toUpperCase()}]`;
    if (data) {
      console[level === 'debug' ? 'log' : level](`${prefix} ${message}`, data);
    } else {
      console[level === 'debug' ? 'log' : level](`${prefix} ${message}`);
    }
  },
};

/** Silent logger for suppressing output */
export const silentLogger: NeuralLogger = {
  log(): void { /* no-op */ },
};

// ============================================================================
// Types and Interfaces (with readonly modifiers)
// ============================================================================

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
  strength: number;           // Mutable: decays over time
  lastAccess: number;         // Mutable: updated on access
  accessCount: number;        // Mutable: incremented on access
  consolidationLevel: number; // Mutable: increases during consolidation
  interference: number;       // Mutable: accumulated interference
}

export interface AgentState {
  readonly id: string;
  position: Float32Array;     // Mutable: updated as agent moves
  velocity: Float32Array;     // Mutable: direction of movement
  attention: Float32Array;    // Mutable: attention weights
  energy: number;             // Mutable: available compute budget
  mode: string;               // Mutable: current operational mode
  lastUpdate: number;         // Mutable: for cleanup tracking
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

// ============================================================================
// 1. SEMANTIC DRIFT DETECTOR - Embeddings as Control Signals
// ============================================================================

/**
 * Detects semantic drift and triggers reflexes based on embedding movement.
 * Instead of asking "what is similar", asks "how far did we move".
 */
export class SemanticDriftDetector {
  private baseline: Float32Array | null = null;
  private history: Array<{ embedding: Float32Array; timestamp: number }> = [];
  private driftEvents: DriftEvent[] = [];
  private config: Required<Pick<NeuralConfig, 'dimension' | 'driftThreshold' | 'driftWindowMs'>>;
  private logger: NeuralLogger;

  // Reflex callbacks
  private reflexes: Map<string, (event: DriftEvent) => void> = new Map();

  constructor(config: NeuralConfig = {}) {
    this.config = {
      dimension: config.dimension ?? NEURAL_CONSTANTS.DEFAULT_DIMENSION,
      driftThreshold: config.driftThreshold ?? NEURAL_CONSTANTS.DEFAULT_DRIFT_THRESHOLD,
      driftWindowMs: config.driftWindowMs ?? NEURAL_CONSTANTS.DEFAULT_DRIFT_WINDOW_MS,
    };
    this.logger = config.logger ?? defaultLogger;
  }

  /**
   * Set the baseline embedding (reference point)
   */
  setBaseline(embedding: number[] | Float32Array): void {
    this.baseline = embedding instanceof Float32Array
      ? new Float32Array(embedding)
      : new Float32Array(embedding);
  }

  /**
   * Observe a new embedding and detect drift
   */
  observe(embedding: number[] | Float32Array, source?: string): DriftEvent | null {
    const emb = embedding instanceof Float32Array
      ? embedding
      : new Float32Array(embedding);

    const now = Date.now();

    // Add to history
    this.history.push({ embedding: new Float32Array(emb), timestamp: now });

    // Prune old history (with size limit)
    const cutoff = now - this.config.driftWindowMs;
    this.history = this.history.filter(h => h.timestamp > cutoff);
    // Security: Enforce maximum history size
    if (this.history.length > NEURAL_CONSTANTS.MAX_HISTORY_SIZE) {
      this.history = this.history.slice(-NEURAL_CONSTANTS.MAX_HISTORY_SIZE);
    }

    // If no baseline, set first observation as baseline
    if (!this.baseline) {
      this.baseline = new Float32Array(emb);
      return null;
    }

    // Calculate drift from baseline
    const drift = this.calculateDrift(emb, this.baseline);

    // Determine category
    let category: DriftEvent['category'] = 'normal';
    if (drift.magnitude > this.config.driftThreshold * NEURAL_CONSTANTS.DRIFT_CRITICAL_MULTIPLIER) {
      category = 'critical';
    } else if (drift.magnitude > this.config.driftThreshold) {
      category = 'warning';
    }

    const event: DriftEvent = {
      timestamp: now,
      magnitude: drift.magnitude,
      direction: drift.direction,
      category,
      source,
    };

    // Record event if significant (with size limit)
    if (category !== 'normal') {
      this.driftEvents.push(event);
      // Security: Prevent unbounded growth
      if (this.driftEvents.length > NEURAL_CONSTANTS.MAX_DRIFT_EVENTS) {
        this.driftEvents = this.driftEvents.slice(-NEURAL_CONSTANTS.MAX_DRIFT_EVENTS);
      }
      this.triggerReflexes(event);
    }

    return event;
  }

  /**
   * Calculate drift between two embeddings
   */
  private calculateDrift(current: Float32Array, reference: Float32Array): {
    magnitude: number;
    direction: Float32Array;
  } {
    const direction = new Float32Array(current.length);
    let magnitudeSq = 0;

    for (let i = 0; i < current.length; i++) {
      const diff = current[i] - reference[i];
      direction[i] = diff;
      magnitudeSq += diff * diff;
    }

    const magnitude = Math.sqrt(magnitudeSq);

    // Normalize direction
    if (magnitude > 0) {
      for (let i = 0; i < direction.length; i++) {
        direction[i] /= magnitude;
      }
    }

    return { magnitude, direction };
  }

  /**
   * Register a reflex callback for drift events
   */
  registerReflex(name: string, callback: (event: DriftEvent) => void): void {
    this.reflexes.set(name, callback);
  }

  /**
   * Trigger registered reflexes
   */
  private triggerReflexes(event: DriftEvent): void {
    const errors: Array<{ reflex: string; error: unknown }> = [];

    for (const [name, callback] of this.reflexes) {
      try {
        callback(event);
      } catch (e) {
        // Security: Track reflex failures but don't break execution
        errors.push({ reflex: name, error: e });
      }
    }

    // Security: Warn if multiple reflexes fail (potential attack or system issue)
    if (errors.length > 0 && errors.length >= this.reflexes.size / 2) {
      this.logger.log('warn', `${errors.length}/${this.reflexes.size} reflexes failed`, {
        failedReflexes: errors.map(e => e.reflex),
      });
    }
  }

  /**
   * Get recent drift velocity (rate of change)
   */
  getVelocity(): number {
    if (this.history.length < 2) return 0;

    const recent = this.history.slice(-NEURAL_CONSTANTS.VELOCITY_WINDOW_SIZE);
    if (recent.length < 2) return 0;

    let totalDrift = 0;
    for (let i = 1; i < recent.length; i++) {
      const drift = this.calculateDrift(recent[i].embedding, recent[i - 1].embedding);
      totalDrift += drift.magnitude;
    }

    const timeSpan = recent[recent.length - 1].timestamp - recent[0].timestamp;
    return timeSpan > 0 ? totalDrift / timeSpan * 1000 : 0; // drift per second
  }

  /**
   * Get drift statistics
   */
  getStats(): {
    currentDrift: number;
    velocity: number;
    criticalEvents: number;
    warningEvents: number;
    historySize: number;
  } {
    const currentDrift = this.history.length > 0 && this.baseline
      ? this.calculateDrift(this.history[this.history.length - 1].embedding, this.baseline).magnitude
      : 0;

    return {
      currentDrift,
      velocity: this.getVelocity(),
      criticalEvents: this.driftEvents.filter(e => e.category === 'critical').length,
      warningEvents: this.driftEvents.filter(e => e.category === 'warning').length,
      historySize: this.history.length,
    };
  }

  /**
   * Reset baseline to current position
   */
  recenter(): void {
    if (this.history.length > 0) {
      this.baseline = new Float32Array(this.history[this.history.length - 1].embedding);
    }
  }
}

// ============================================================================
// 2. MEMORY PHYSICS - Forgetting, Interference, Consolidation
// ============================================================================

/**
 * Implements hippocampal-like memory dynamics in embedding space.
 * Memory strength decays, similar memories interfere, consolidation strengthens.
 */
export class MemoryPhysics {
  private memories: Map<string, NeuralMemoryEntry> = new Map();
  private config: Required<Pick<NeuralConfig, 'dimension' | 'memoryDecayRate' | 'interferenceThreshold' | 'consolidationRate'>>;
  private lastUpdate: number = Date.now();
  private logger: NeuralLogger;

  constructor(config: NeuralConfig = {}) {
    this.config = {
      dimension: config.dimension ?? NEURAL_CONSTANTS.DEFAULT_DIMENSION,
      memoryDecayRate: config.memoryDecayRate ?? NEURAL_CONSTANTS.DEFAULT_MEMORY_DECAY_RATE,
      interferenceThreshold: config.interferenceThreshold ?? NEURAL_CONSTANTS.DEFAULT_INTERFERENCE_THRESHOLD,
      consolidationRate: config.consolidationRate ?? NEURAL_CONSTANTS.DEFAULT_CONSOLIDATION_RATE,
    };
    this.logger = config.logger ?? defaultLogger;
  }

  /**
   * Encode a new memory
   */
  encode(id: string, embedding: number[] | Float32Array, content: string): NeuralMemoryEntry {
    // Security: Validate inputs
    if (typeof id !== 'string' || id.length === 0 || id.length > NEURAL_CONSTANTS.MAX_ID_LENGTH) {
      throw new Error(`Invalid memory ID: must be string of 1-${NEURAL_CONSTANTS.MAX_ID_LENGTH} characters`);
    }
    if (typeof content !== 'string' || content.length > NEURAL_CONSTANTS.MAX_CONTENT_LENGTH) {
      throw new Error(`Content exceeds maximum length: ${NEURAL_CONSTANTS.MAX_CONTENT_LENGTH}`);
    }
    if (this.memories.size >= NEURAL_CONSTANTS.MAX_MEMORIES && !this.memories.has(id)) {
      // Force cleanup of weak memories before adding new one
      this.forceCleanup();
    }

    const emb = embedding instanceof Float32Array
      ? new Float32Array(embedding)
      : new Float32Array(embedding);

    // Security: Validate embedding dimension
    if (emb.length !== this.config.dimension) {
      throw new Error(`Embedding dimension mismatch: expected ${this.config.dimension}, got ${emb.length}`);
    }

    const now = Date.now();

    // Check for interference with existing memories
    let interference = 0;
    for (const existing of this.memories.values()) {
      const similarity = this.cosineSimilarity(emb, existing.embedding);
      if (similarity > this.config.interferenceThreshold) {
        interference += similarity - this.config.interferenceThreshold;
        existing.interference += (similarity - this.config.interferenceThreshold) * 0.5;
      }
    }

    const entry: NeuralMemoryEntry = {
      id,
      embedding: emb,
      content,
      strength: 1.0 - interference * 0.3, // New memories weaker if interfered
      lastAccess: now,
      accessCount: 1,
      consolidationLevel: 0,
      interference,
    };

    this.memories.set(id, entry);
    return entry;
  }

  /**
   * Recall memories similar to a query (strengthens accessed memories)
   */
  recall(query: number[] | Float32Array, k: number = 5): NeuralMemoryEntry[] {
    const q = query instanceof Float32Array ? query : new Float32Array(query);
    const now = Date.now();

    // Apply decay before recall
    this.applyDecay();

    // Score memories
    const scored: Array<{ entry: NeuralMemoryEntry; score: number }> = [];
    for (const entry of this.memories.values()) {
      const similarity = this.cosineSimilarity(q, entry.embedding);
      // Effective score combines similarity and strength
      const score = similarity * Math.sqrt(entry.strength);
      scored.push({ entry, score });
    }

    // Sort and get top-k
    scored.sort((a, b) => b.score - a.score);
    const results = scored.slice(0, k).map(s => s.entry);

    // Strengthen recalled memories (retrieval practice effect)
    for (const entry of results) {
      entry.lastAccess = now;
      entry.accessCount++;
      entry.strength = Math.min(1.0, entry.strength + NEURAL_CONSTANTS.RECALL_STRENGTH_BOOST);
    }

    return results;
  }

  /**
   * Apply time-based decay to all memories
   */
  private applyDecay(): void {
    const now = Date.now();
    const elapsed = Math.max(0, now - this.lastUpdate) / 60000; // minutes, prevent negative

    // Security: Cap maximum elapsed time to prevent manipulation
    const cappedElapsed = Math.min(elapsed, NEURAL_CONSTANTS.MAX_TIME_JUMP_MINUTES);
    if (elapsed > NEURAL_CONSTANTS.MAX_TIME_JUMP_MINUTES) {
      this.logger.log('warn', `Large time jump detected: ${elapsed.toFixed(0)} minutes`);
    }

    this.lastUpdate = now;
    const decayFactor = Math.exp(-this.config.memoryDecayRate * cappedElapsed);

    for (const entry of this.memories.values()) {
      // Decay is slower for consolidated memories
      const effectiveDecay = decayFactor + entry.consolidationLevel * (1 - decayFactor) * 0.8;
      entry.strength = Math.max(0, entry.strength * effectiveDecay);

      // Very weak memories are forgotten
      if (entry.strength < NEURAL_CONSTANTS.MEMORY_FORGET_THRESHOLD) {
        this.memories.delete(entry.id);
      }
    }
  }

  /**
   * Consolidate memories (like sleep consolidation)
   * Strengthens frequently accessed, weakly interfered memories
   */
  consolidate(): { consolidated: number; forgotten: number } {
    let consolidated = 0;
    let forgotten = 0;

    for (const entry of this.memories.values()) {
      // Consolidation score based on access pattern and low interference
      const consolidationScore =
        Math.log(entry.accessCount + 1) * entry.strength * (1 - entry.interference * 0.5);

      if (consolidationScore > NEURAL_CONSTANTS.CONSOLIDATION_SCORE_THRESHOLD) {
        entry.consolidationLevel = Math.min(1.0, entry.consolidationLevel + this.config.consolidationRate);
        entry.strength = Math.min(1.0, entry.strength + 0.05);
        consolidated++;
      } else if (entry.strength < NEURAL_CONSTANTS.MEMORY_CLEANUP_PERCENT) {
        this.memories.delete(entry.id);
        forgotten++;
      }
    }

    return { consolidated, forgotten };
  }

  /**
   * Get memory statistics
   */
  getStats(): {
    totalMemories: number;
    avgStrength: number;
    avgConsolidation: number;
    avgInterference: number;
  } {
    if (this.memories.size === 0) {
      return { totalMemories: 0, avgStrength: 0, avgConsolidation: 0, avgInterference: 0 };
    }

    let sumStrength = 0, sumConsolidation = 0, sumInterference = 0;
    for (const entry of this.memories.values()) {
      sumStrength += entry.strength;
      sumConsolidation += entry.consolidationLevel;
      sumInterference += entry.interference;
    }

    const n = this.memories.size;
    return {
      totalMemories: n,
      avgStrength: sumStrength / n,
      avgConsolidation: sumConsolidation / n,
      avgInterference: sumInterference / n,
    };
  }

  private cosineSimilarity(a: Float32Array, b: Float32Array): number {
    let dot = 0, normA = 0, normB = 0;
    for (let i = 0; i < a.length; i++) {
      dot += a[i] * b[i];
      normA += a[i] * a[i];
      normB += b[i] * b[i];
    }
    const denom = Math.sqrt(normA * normB);
    if (denom < 1e-10) return 0; // Handle zero vectors
    return Math.max(-1, Math.min(1, dot / denom)); // Clamp to valid range
  }

  /**
   * Force cleanup of weak memories when limit reached
   */
  private forceCleanup(): void {
    const entries = Array.from(this.memories.entries())
      .sort((a, b) => a[1].strength - b[1].strength);
    const removeCount = Math.ceil(this.memories.size * NEURAL_CONSTANTS.MEMORY_CLEANUP_PERCENT);
    for (let i = 0; i < removeCount; i++) {
      this.memories.delete(entries[i][0]);
    }
    this.logger.log('debug', `Force cleanup removed ${removeCount} weak memories`);
  }
}

// ============================================================================
// 3. EMBEDDING STATE MACHINE - Agent State via Geometry
// ============================================================================

/**
 * Manages agent state as movement through embedding space.
 * Decisions become geometric - no explicit state machine.
 */
export class EmbeddingStateMachine {
  private agents: Map<string, AgentState> = new Map();
  private modeRegions: Map<string, { centroid: Float32Array; radius: number }> = new Map();
  private config: { dimension: number };
  private logger: NeuralLogger;
  private lastCleanup: number = Date.now();

  constructor(config: NeuralConfig = {}) {
    this.config = {
      dimension: config.dimension ?? NEURAL_CONSTANTS.DEFAULT_DIMENSION,
    };
    this.logger = config.logger ?? defaultLogger;
  }

  /**
   * Create or update an agent
   */
  updateAgent(id: string, embedding: number[] | Float32Array): AgentState {
    // Periodically clean up stale agents
    this.cleanupStaleAgents();

    // Security: Enforce agent limit
    if (!this.agents.has(id) && this.agents.size >= NEURAL_CONSTANTS.MAX_AGENTS) {
      throw new Error(`Agent limit reached: ${NEURAL_CONSTANTS.MAX_AGENTS}`);
    }

    const position = embedding instanceof Float32Array
      ? new Float32Array(embedding)
      : new Float32Array(embedding);

    const existing = this.agents.get(id);
    const now = Date.now();

    if (existing) {
      // Calculate velocity (direction of movement)
      for (let i = 0; i < position.length; i++) {
        existing.velocity[i] = position[i] - existing.position[i];
      }
      existing.position = position;
      existing.lastUpdate = now;

      // Update mode based on nearest region
      existing.mode = this.determineMode(position);
    } else {
      // New agent
      const state: AgentState = {
        id,
        position,
        velocity: new Float32Array(this.config.dimension),
        attention: new Float32Array(this.config.dimension).fill(1 / this.config.dimension),
        energy: NEURAL_CONSTANTS.DEFAULT_AGENT_ENERGY,
        mode: this.determineMode(position),
        lastUpdate: now,
      };
      this.agents.set(id, state);
      return state;
    }

    return existing;
  }

  /**
   * Remove stale agents that haven't been updated recently
   */
  private cleanupStaleAgents(): void {
    const now = Date.now();
    // Only run cleanup every minute
    if (now - this.lastCleanup < 60000) return;
    this.lastCleanup = now;

    const cutoff = now - NEURAL_CONSTANTS.AGENT_TIMEOUT_MS;
    let removed = 0;

    for (const [id, state] of this.agents) {
      if (state.lastUpdate < cutoff) {
        this.agents.delete(id);
        removed++;
      }
    }

    if (removed > 0) {
      this.logger.log('debug', `Cleaned up ${removed} stale agents`);
    }
  }

  /**
   * Manually remove an agent
   */
  removeAgent(id: string): boolean {
    return this.agents.delete(id);
  }

  /**
   * Define a mode region in embedding space
   */
  defineMode(name: string, centroid: number[] | Float32Array, radius: number = 0.3): void {
    const c = centroid instanceof Float32Array
      ? new Float32Array(centroid)
      : new Float32Array(centroid);
    this.modeRegions.set(name, { centroid: c, radius });
  }

  /**
   * Determine which mode an agent is in based on position
   */
  private determineMode(position: Float32Array): string {
    let bestMode = 'unknown';
    let bestScore = -Infinity;

    for (const [name, region] of this.modeRegions) {
      const distance = this.euclideanDistance(position, region.centroid);
      const score = region.radius - distance;
      if (score > bestScore) {
        bestScore = score;
        bestMode = name;
      }
    }

    return bestScore > 0 ? bestMode : 'exploring';
  }

  /**
   * Get agent trajectory prediction
   */
  predictTrajectory(id: string, steps: number = 5): Float32Array[] {
    // Security: Limit trajectory steps
    if (!Number.isInteger(steps) || steps < 1) {
      throw new Error('Steps must be a positive integer');
    }
    const limitedSteps = Math.min(steps, NEURAL_CONSTANTS.MAX_TRAJECTORY_STEPS);

    const agent = this.agents.get(id);
    if (!agent) return [];

    const trajectory: Float32Array[] = [];
    let current = new Float32Array(agent.position);

    for (let i = 0; i < limitedSteps; i++) {
      const next = new Float32Array(current.length);
      for (let j = 0; j < current.length; j++) {
        next[j] = current[j] + agent.velocity[j] * (1 - i * NEURAL_CONSTANTS.TRAJECTORY_DAMPING);
      }
      trajectory.push(next);
      current = next;
    }

    return trajectory;
  }

  /**
   * Apply attention to agent state
   */
  attendTo(agentId: string, focusEmbedding: number[] | Float32Array): void {
    const agent = this.agents.get(agentId);
    if (!agent) return;

    const focus = focusEmbedding instanceof Float32Array
      ? focusEmbedding
      : new Float32Array(focusEmbedding);

    // Update attention weights based on similarity to focus
    let sum = 0;
    for (let i = 0; i < agent.attention.length; i++) {
      agent.attention[i] = Math.abs(focus[i]) + 0.01;
      sum += agent.attention[i];
    }
    // Normalize
    for (let i = 0; i < agent.attention.length; i++) {
      agent.attention[i] /= sum;
    }
  }

  /**
   * Get all agents in a specific mode
   */
  getAgentsInMode(mode: string): AgentState[] {
    return Array.from(this.agents.values()).filter(a => a.mode === mode);
  }

  private euclideanDistance(a: Float32Array, b: Float32Array): number {
    let sum = 0;
    for (let i = 0; i < a.length; i++) {
      const diff = a[i] - b[i];
      sum += diff * diff;
    }
    return Math.sqrt(sum);
  }
}

// ============================================================================
// 4. SWARM COORDINATOR - Multi-Agent Coordination via Embeddings
// ============================================================================

/**
 * Enables multi-agent coordination through shared embedding space.
 * Swarm behavior emerges from geometry, not protocol.
 */
export class SwarmCoordinator {
  private agents: Map<string, {
    position: Float32Array;
    velocity: Float32Array;
    lastUpdate: number;
    specialty: string;
  }> = new Map();

  private sharedContext: Float32Array;
  private config: { dimension: number };
  private logger: NeuralLogger;

  constructor(config: NeuralConfig = {}) {
    this.config = { dimension: config.dimension ?? NEURAL_CONSTANTS.DEFAULT_DIMENSION };
    this.sharedContext = new Float32Array(this.config.dimension);
    this.logger = config.logger ?? defaultLogger;
  }

  /**
   * Register an agent with the swarm
   */
  register(id: string, embedding: number[] | Float32Array, specialty: string = 'general'): void {
    // Security: Validate inputs
    if (typeof id !== 'string' || id.length === 0 || id.length > NEURAL_CONSTANTS.MAX_ID_LENGTH) {
      throw new Error(`Invalid agent ID: must be string of 1-${NEURAL_CONSTANTS.MAX_ID_LENGTH} characters`);
    }
    if (typeof specialty !== 'string' || specialty.length > NEURAL_CONSTANTS.MAX_SPECIALTY_LENGTH) {
      throw new Error(`Specialty exceeds maximum length: ${NEURAL_CONSTANTS.MAX_SPECIALTY_LENGTH}`);
    }
    if (this.agents.size >= NEURAL_CONSTANTS.MAX_AGENTS && !this.agents.has(id)) {
      throw new Error(`Agent limit reached: ${NEURAL_CONSTANTS.MAX_AGENTS}`);
    }

    const position = embedding instanceof Float32Array
      ? new Float32Array(embedding)
      : new Float32Array(embedding);

    // Security: Validate embedding dimension
    if (position.length !== this.config.dimension) {
      throw new Error(`Embedding dimension mismatch: expected ${this.config.dimension}, got ${position.length}`);
    }

    this.agents.set(id, {
      position,
      velocity: new Float32Array(this.config.dimension),
      lastUpdate: Date.now(),
      specialty,
    });

    this.updateSharedContext();
  }

  /**
   * Update agent position (from their work/observations)
   */
  update(id: string, embedding: number[] | Float32Array): void {
    const agent = this.agents.get(id);
    if (!agent) return;

    const newPosition = embedding instanceof Float32Array
      ? embedding
      : new Float32Array(embedding);

    // Calculate velocity
    for (let i = 0; i < agent.position.length; i++) {
      agent.velocity[i] = newPosition[i] - agent.position[i];
      agent.position[i] = newPosition[i];
    }
    agent.lastUpdate = Date.now();

    this.updateSharedContext();
  }

  /**
   * Update shared context (centroid of all agents)
   */
  private updateSharedContext(): void {
    if (this.agents.size === 0) return;

    this.sharedContext.fill(0);
    for (const agent of this.agents.values()) {
      for (let i = 0; i < this.sharedContext.length; i++) {
        this.sharedContext[i] += agent.position[i];
      }
    }
    for (let i = 0; i < this.sharedContext.length; i++) {
      this.sharedContext[i] /= this.agents.size;
    }
  }

  /**
   * Get coordination signal for an agent (how to align with swarm)
   */
  getCoordinationSignal(id: string): Float32Array {
    const agent = this.agents.get(id);
    if (!agent) return new Float32Array(this.config.dimension);

    // Signal points toward shared context
    const signal = new Float32Array(this.config.dimension);
    for (let i = 0; i < signal.length; i++) {
      signal[i] = this.sharedContext[i] - agent.position[i];
    }
    return signal;
  }

  /**
   * Find agents working on similar things (for collaboration)
   */
  findCollaborators(id: string, k: number = 3): Array<{ id: string; similarity: number; specialty: string }> {
    const agent = this.agents.get(id);
    if (!agent) return [];

    const scored: Array<{ id: string; similarity: number; specialty: string }> = [];
    for (const [otherId, other] of this.agents) {
      if (otherId === id) continue;
      const similarity = this.cosineSimilarity(agent.position, other.position);
      scored.push({ id: otherId, similarity, specialty: other.specialty });
    }

    scored.sort((a, b) => b.similarity - a.similarity);
    return scored.slice(0, k);
  }

  /**
   * Detect emergent clusters (specialization)
   */
  detectClusters(threshold: number = NEURAL_CONSTANTS.DEFAULT_CLUSTER_THRESHOLD): Map<string, string[]> {
    // Security: Validate threshold
    if (threshold < 0 || threshold > 1) {
      throw new Error('Threshold must be between 0 and 1');
    }

    // Security: Limit clustering for performance (O(n²) algorithm)
    if (this.agents.size > NEURAL_CONSTANTS.MAX_CLUSTER_AGENTS) {
      this.logger.log('warn', `Too many agents for clustering: ${this.agents.size} > ${NEURAL_CONSTANTS.MAX_CLUSTER_AGENTS}`);
      // Return single cluster with all agents
      return new Map([['all', Array.from(this.agents.keys())]]);
    }

    const clusters: Map<string, string[]> = new Map();
    const assigned: Set<string> = new Set();

    for (const [id, agent] of this.agents) {
      if (assigned.has(id)) continue;

      const cluster: string[] = [id];
      assigned.add(id);

      for (const [otherId, other] of this.agents) {
        if (assigned.has(otherId)) continue;
        const similarity = this.cosineSimilarity(agent.position, other.position);
        if (similarity > threshold) {
          cluster.push(otherId);
          assigned.add(otherId);
        }
      }

      clusters.set(id, cluster);
    }

    return clusters;
  }

  /**
   * Get swarm coherence (how aligned are agents)
   */
  getCoherence(): number {
    if (this.agents.size < 2) return 1.0;

    let totalSimilarity = 0;
    let pairs = 0;

    const agentList = Array.from(this.agents.values());
    for (let i = 0; i < agentList.length; i++) {
      for (let j = i + 1; j < agentList.length; j++) {
        totalSimilarity += this.cosineSimilarity(agentList[i].position, agentList[j].position);
        pairs++;
      }
    }

    return pairs > 0 ? totalSimilarity / pairs : 1.0;
  }

  private cosineSimilarity(a: Float32Array, b: Float32Array): number {
    let dot = 0, normA = 0, normB = 0;
    for (let i = 0; i < a.length; i++) {
      dot += a[i] * b[i];
      normA += a[i] * a[i];
      normB += b[i] * b[i];
    }
    const denom = Math.sqrt(normA * normB);
    if (denom < NEURAL_CONSTANTS.ZERO_VECTOR_THRESHOLD) return 0;
    return Math.max(-1, Math.min(1, dot / denom));
  }

  /**
   * Remove an agent from the swarm
   */
  removeAgent(id: string): boolean {
    const removed = this.agents.delete(id);
    if (removed) {
      this.updateSharedContext();
    }
    return removed;
  }
}

// ============================================================================
// 5. COHERENCE MONITOR - Safety and Alignment Detection
// ============================================================================

/**
 * Monitors system coherence via embedding patterns.
 * Detects degradation, poisoning, misalignment before explicit failures.
 */
export class CoherenceMonitor {
  private history: Array<{
    embedding: Float32Array;
    timestamp: number;
    source: string;
  }> = [];
  private baselineDistribution: {
    mean: Float32Array;
    variance: Float32Array;
  } | null = null;
  private config: { dimension: number; windowSize: number };
  private logger: NeuralLogger;

  constructor(config: NeuralConfig & { windowSize?: number } = {}) {
    this.config = {
      dimension: config.dimension ?? NEURAL_CONSTANTS.DEFAULT_DIMENSION,
      windowSize: config.windowSize ?? NEURAL_CONSTANTS.DEFAULT_WINDOW_SIZE,
    };
    this.logger = config.logger ?? defaultLogger;
  }

  /**
   * Record an observation
   */
  observe(embedding: number[] | Float32Array, source: string = 'unknown'): void {
    const emb = embedding instanceof Float32Array
      ? new Float32Array(embedding)
      : new Float32Array(embedding);

    this.history.push({
      embedding: emb,
      timestamp: Date.now(),
      source,
    });

    // Keep window size
    while (this.history.length > this.config.windowSize * 2) {
      this.history.shift();
    }
  }

  /**
   * Establish baseline distribution
   */
  calibrate(): void {
    if (this.history.length < NEURAL_CONSTANTS.MIN_CALIBRATION_OBSERVATIONS) {
      throw new Error(`Need at least ${NEURAL_CONSTANTS.MIN_CALIBRATION_OBSERVATIONS} observations to calibrate`);
    }

    const mean = new Float32Array(this.config.dimension);
    const variance = new Float32Array(this.config.dimension);

    // Calculate mean
    for (const obs of this.history) {
      for (let i = 0; i < mean.length; i++) {
        mean[i] += obs.embedding[i];
      }
    }
    for (let i = 0; i < mean.length; i++) {
      mean[i] /= this.history.length;
    }

    // Calculate variance
    for (const obs of this.history) {
      for (let i = 0; i < variance.length; i++) {
        const diff = obs.embedding[i] - mean[i];
        variance[i] += diff * diff;
      }
    }
    for (let i = 0; i < variance.length; i++) {
      variance[i] /= this.history.length;
    }

    this.baselineDistribution = { mean, variance };
  }

  /**
   * Generate coherence report
   */
  report(): CoherenceReport {
    const anomalies: Array<{ type: string; severity: number; description: string }> = [];

    // Drift score: how much has distribution shifted
    const driftScore = this.calculateDriftScore();
    if (driftScore > NEURAL_CONSTANTS.DRIFT_WARNING_THRESHOLD) {
      anomalies.push({
        type: 'distribution_drift',
        severity: driftScore,
        description: 'Embedding distribution has shifted significantly from baseline',
      });
    }

    // Stability score: variance in recent observations
    const stabilityScore = this.calculateStabilityScore();
    if (stabilityScore < NEURAL_CONSTANTS.STABILITY_WARNING_THRESHOLD) {
      anomalies.push({
        type: 'instability',
        severity: 1 - stabilityScore,
        description: 'High variance in recent embeddings suggests instability',
      });
    }

    // Alignment score: consistency of embeddings from same source
    const alignmentScore = this.calculateAlignmentScore();
    if (alignmentScore < NEURAL_CONSTANTS.ALIGNMENT_WARNING_THRESHOLD) {
      anomalies.push({
        type: 'misalignment',
        severity: 1 - alignmentScore,
        description: 'Embeddings from same source show inconsistent patterns',
      });
    }

    // Overall score
    const overallScore = (
      (1 - driftScore) * 0.3 +
      stabilityScore * 0.3 +
      alignmentScore * 0.4
    );

    return {
      timestamp: Date.now(),
      overallScore,
      driftScore,
      stabilityScore,
      alignmentScore,
      anomalies,
    };
  }

  private calculateDriftScore(): number {
    if (!this.baselineDistribution || this.history.length < NEURAL_CONSTANTS.RECENT_OBSERVATIONS_SIZE) return 0;

    const recent = this.history.slice(-NEURAL_CONSTANTS.RECENT_OBSERVATIONS_SIZE);
    const recentMean = new Float32Array(this.config.dimension);

    for (const obs of recent) {
      for (let i = 0; i < recentMean.length; i++) {
        recentMean[i] += obs.embedding[i];
      }
    }
    for (let i = 0; i < recentMean.length; i++) {
      recentMean[i] /= recent.length;
    }

    // Calculate distance between means
    let distance = 0;
    for (let i = 0; i < recentMean.length; i++) {
      const diff = recentMean[i] - this.baselineDistribution.mean[i];
      distance += diff * diff;
    }

    return Math.min(1, Math.sqrt(distance));
  }

  private calculateStabilityScore(): number {
    if (this.history.length < NEURAL_CONSTANTS.STABILITY_WINDOW_SIZE) return 1.0;

    const recent = this.history.slice(-NEURAL_CONSTANTS.STABILITY_WINDOW_SIZE);
    let totalVariance = 0;

    // Calculate pairwise distances
    for (let i = 1; i < recent.length; i++) {
      let distance = 0;
      for (let j = 0; j < recent[i].embedding.length; j++) {
        const diff = recent[i].embedding[j] - recent[i - 1].embedding[j];
        distance += diff * diff;
      }
      totalVariance += Math.sqrt(distance);
    }

    const avgVariance = totalVariance / (recent.length - 1);
    return Math.max(0, 1 - avgVariance * 2);
  }

  private calculateAlignmentScore(): number {
    // Group by source and check consistency
    const bySource: Map<string, Float32Array[]> = new Map();
    for (const obs of this.history.slice(-NEURAL_CONSTANTS.ALIGNMENT_WINDOW_SIZE)) {
      if (!bySource.has(obs.source)) {
        bySource.set(obs.source, []);
      }
      bySource.get(obs.source)!.push(obs.embedding);
    }

    if (bySource.size < 2) return 1.0;

    let totalConsistency = 0;
    let count = 0;

    for (const embeddings of bySource.values()) {
      if (embeddings.length < 2) continue;

      // Calculate average pairwise similarity within source
      for (let i = 0; i < embeddings.length; i++) {
        for (let j = i + 1; j < embeddings.length; j++) {
          totalConsistency += this.cosineSimilarity(embeddings[i], embeddings[j]);
          count++;
        }
      }
    }

    return count > 0 ? totalConsistency / count : 1.0;
  }

  private cosineSimilarity(a: Float32Array, b: Float32Array): number {
    let dot = 0, normA = 0, normB = 0;
    for (let i = 0; i < a.length; i++) {
      dot += a[i] * b[i];
      normA += a[i] * a[i];
      normB += b[i] * b[i];
    }
    return dot / (Math.sqrt(normA * normB) + 1e-8);
  }
}

// ============================================================================
// 6. NEURAL SUBSTRATE - Synthetic Nervous System
// ============================================================================

/**
 * Unified neural embedding substrate combining all components.
 * Acts like a synthetic nervous system with reflexes, memory, and coordination.
 */
export class NeuralSubstrate {
  public readonly drift: SemanticDriftDetector;
  public readonly memory: MemoryPhysics;
  public readonly state: EmbeddingStateMachine;
  public readonly swarm: SwarmCoordinator;
  public readonly coherence: CoherenceMonitor;

  private config: Required<NeuralConfig>;
  private logger: NeuralLogger;
  private reflexLatency: number;

  constructor(config: NeuralConfig = {}) {
    this.logger = config.logger ?? defaultLogger;

    this.config = {
      dimension: config.dimension ?? NEURAL_CONSTANTS.DEFAULT_DIMENSION,
      driftThreshold: config.driftThreshold ?? NEURAL_CONSTANTS.DEFAULT_DRIFT_THRESHOLD,
      driftWindowMs: config.driftWindowMs ?? NEURAL_CONSTANTS.DEFAULT_DRIFT_WINDOW_MS,
      memoryDecayRate: config.memoryDecayRate ?? NEURAL_CONSTANTS.DEFAULT_MEMORY_DECAY_RATE,
      interferenceThreshold: config.interferenceThreshold ?? NEURAL_CONSTANTS.DEFAULT_INTERFERENCE_THRESHOLD,
      consolidationRate: config.consolidationRate ?? NEURAL_CONSTANTS.DEFAULT_CONSOLIDATION_RATE,
      reflexLatencyMs: config.reflexLatencyMs ?? NEURAL_CONSTANTS.DEFAULT_REFLEX_LATENCY_MS,
      logger: this.logger,
    };

    this.reflexLatency = this.config.reflexLatencyMs;

    // Pass logger to all sub-components
    this.drift = new SemanticDriftDetector(this.config);
    this.memory = new MemoryPhysics(this.config);
    this.state = new EmbeddingStateMachine(this.config);
    this.swarm = new SwarmCoordinator(this.config);
    this.coherence = new CoherenceMonitor(this.config);

    // Wire up default reflexes
    this.drift.registerReflex('memory_consolidation', (event) => {
      if (event.category === 'critical') {
        // Consolidate memory on critical drift
        this.memory.consolidate();
      }
    });

    this.drift.registerReflex('coherence_check', (event) => {
      if (event.category !== 'normal') {
        // Check coherence on any significant drift
        const report = this.coherence.report();
        if (report.overallScore < NEURAL_CONSTANTS.COHERENCE_WARNING_THRESHOLD) {
          this.logger.log('warn', 'Neural substrate coherence warning', {
            overallScore: report.overallScore,
            driftScore: report.driftScore,
            stabilityScore: report.stabilityScore,
            alignmentScore: report.alignmentScore,
            anomalyCount: report.anomalies.length,
          });
        }
      }
    });
  }

  /**
   * Process an embedding through the entire substrate
   */
  process(
    embedding: number[] | Float32Array,
    options: {
      agentId?: string;
      memoryId?: string;
      content?: string;
      source?: string;
    } = {}
  ): {
    drift: DriftEvent | null;
    memory: NeuralMemoryEntry | null;
    state: AgentState | null;
  } {
    const emb = embedding instanceof Float32Array
      ? embedding
      : new Float32Array(embedding);

    // 1. Observe for drift
    const driftEvent = this.drift.observe(emb, options.source);

    // 2. Encode to memory if content provided
    let memoryEntry: NeuralMemoryEntry | null = null;
    if (options.memoryId && options.content) {
      memoryEntry = this.memory.encode(options.memoryId, emb, options.content);
    }

    // 3. Update agent state if ID provided
    let agentState: AgentState | null = null;
    if (options.agentId) {
      agentState = this.state.updateAgent(options.agentId, emb);
      this.swarm.register(options.agentId, emb);
    }

    // 4. Record for coherence monitoring
    this.coherence.observe(emb, options.source);

    return { drift: driftEvent, memory: memoryEntry, state: agentState };
  }

  /**
   * Query the substrate
   */
  query(embedding: number[] | Float32Array, k: number = 5): {
    memories: NeuralMemoryEntry[];
    collaborators: Array<{ id: string; similarity: number; specialty: string }>;
    coherence: CoherenceReport;
  } {
    const emb = embedding instanceof Float32Array
      ? embedding
      : new Float32Array(embedding);

    return {
      memories: this.memory.recall(emb, k),
      collaborators: [], // Would need agent context
      coherence: this.coherence.report(),
    };
  }

  /**
   * Get overall system health
   */
  health(): {
    driftStats: ReturnType<SemanticDriftDetector['getStats']>;
    memoryStats: ReturnType<MemoryPhysics['getStats']>;
    swarmCoherence: number;
    coherenceReport: CoherenceReport;
  } {
    return {
      driftStats: this.drift.getStats(),
      memoryStats: this.memory.getStats(),
      swarmCoherence: this.swarm.getCoherence(),
      coherenceReport: this.coherence.report(),
    };
  }

  /**
   * Run consolidation (like "sleep")
   */
  consolidate(): { consolidated: number; forgotten: number } {
    return this.memory.consolidate();
  }

  /**
   * Calibrate coherence baseline
   */
  calibrate(): void {
    this.coherence.calibrate();
  }
}

// ============================================================================
// Exports
// ============================================================================

export default NeuralSubstrate;
