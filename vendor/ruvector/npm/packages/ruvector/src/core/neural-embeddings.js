"use strict";
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
Object.defineProperty(exports, "__esModule", { value: true });
exports.NeuralSubstrate = exports.CoherenceMonitor = exports.SwarmCoordinator = exports.EmbeddingStateMachine = exports.MemoryPhysics = exports.SemanticDriftDetector = exports.silentLogger = exports.defaultLogger = exports.NEURAL_CONSTANTS = void 0;
// ============================================================================
// Constants - Replace magic numbers with named constants
// ============================================================================
exports.NEURAL_CONSTANTS = {
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
};
/** Default console logger */
exports.defaultLogger = {
    log(level, message, data) {
        const prefix = `[Neural:${level.toUpperCase()}]`;
        if (data) {
            console[level === 'debug' ? 'log' : level](`${prefix} ${message}`, data);
        }
        else {
            console[level === 'debug' ? 'log' : level](`${prefix} ${message}`);
        }
    },
};
/** Silent logger for suppressing output */
exports.silentLogger = {
    log() { },
};
// ============================================================================
// 1. SEMANTIC DRIFT DETECTOR - Embeddings as Control Signals
// ============================================================================
/**
 * Detects semantic drift and triggers reflexes based on embedding movement.
 * Instead of asking "what is similar", asks "how far did we move".
 */
class SemanticDriftDetector {
    constructor(config = {}) {
        this.baseline = null;
        this.history = [];
        this.driftEvents = [];
        // Reflex callbacks
        this.reflexes = new Map();
        this.config = {
            dimension: config.dimension ?? exports.NEURAL_CONSTANTS.DEFAULT_DIMENSION,
            driftThreshold: config.driftThreshold ?? exports.NEURAL_CONSTANTS.DEFAULT_DRIFT_THRESHOLD,
            driftWindowMs: config.driftWindowMs ?? exports.NEURAL_CONSTANTS.DEFAULT_DRIFT_WINDOW_MS,
        };
        this.logger = config.logger ?? exports.defaultLogger;
    }
    /**
     * Set the baseline embedding (reference point)
     */
    setBaseline(embedding) {
        this.baseline = embedding instanceof Float32Array
            ? new Float32Array(embedding)
            : new Float32Array(embedding);
    }
    /**
     * Observe a new embedding and detect drift
     */
    observe(embedding, source) {
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
        if (this.history.length > exports.NEURAL_CONSTANTS.MAX_HISTORY_SIZE) {
            this.history = this.history.slice(-exports.NEURAL_CONSTANTS.MAX_HISTORY_SIZE);
        }
        // If no baseline, set first observation as baseline
        if (!this.baseline) {
            this.baseline = new Float32Array(emb);
            return null;
        }
        // Calculate drift from baseline
        const drift = this.calculateDrift(emb, this.baseline);
        // Determine category
        let category = 'normal';
        if (drift.magnitude > this.config.driftThreshold * exports.NEURAL_CONSTANTS.DRIFT_CRITICAL_MULTIPLIER) {
            category = 'critical';
        }
        else if (drift.magnitude > this.config.driftThreshold) {
            category = 'warning';
        }
        const event = {
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
            if (this.driftEvents.length > exports.NEURAL_CONSTANTS.MAX_DRIFT_EVENTS) {
                this.driftEvents = this.driftEvents.slice(-exports.NEURAL_CONSTANTS.MAX_DRIFT_EVENTS);
            }
            this.triggerReflexes(event);
        }
        return event;
    }
    /**
     * Calculate drift between two embeddings
     */
    calculateDrift(current, reference) {
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
    registerReflex(name, callback) {
        this.reflexes.set(name, callback);
    }
    /**
     * Trigger registered reflexes
     */
    triggerReflexes(event) {
        const errors = [];
        for (const [name, callback] of this.reflexes) {
            try {
                callback(event);
            }
            catch (e) {
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
    getVelocity() {
        if (this.history.length < 2)
            return 0;
        const recent = this.history.slice(-exports.NEURAL_CONSTANTS.VELOCITY_WINDOW_SIZE);
        if (recent.length < 2)
            return 0;
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
    getStats() {
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
    recenter() {
        if (this.history.length > 0) {
            this.baseline = new Float32Array(this.history[this.history.length - 1].embedding);
        }
    }
}
exports.SemanticDriftDetector = SemanticDriftDetector;
// ============================================================================
// 2. MEMORY PHYSICS - Forgetting, Interference, Consolidation
// ============================================================================
/**
 * Implements hippocampal-like memory dynamics in embedding space.
 * Memory strength decays, similar memories interfere, consolidation strengthens.
 */
class MemoryPhysics {
    constructor(config = {}) {
        this.memories = new Map();
        this.lastUpdate = Date.now();
        this.config = {
            dimension: config.dimension ?? exports.NEURAL_CONSTANTS.DEFAULT_DIMENSION,
            memoryDecayRate: config.memoryDecayRate ?? exports.NEURAL_CONSTANTS.DEFAULT_MEMORY_DECAY_RATE,
            interferenceThreshold: config.interferenceThreshold ?? exports.NEURAL_CONSTANTS.DEFAULT_INTERFERENCE_THRESHOLD,
            consolidationRate: config.consolidationRate ?? exports.NEURAL_CONSTANTS.DEFAULT_CONSOLIDATION_RATE,
        };
        this.logger = config.logger ?? exports.defaultLogger;
    }
    /**
     * Encode a new memory
     */
    encode(id, embedding, content) {
        // Security: Validate inputs
        if (typeof id !== 'string' || id.length === 0 || id.length > exports.NEURAL_CONSTANTS.MAX_ID_LENGTH) {
            throw new Error(`Invalid memory ID: must be string of 1-${exports.NEURAL_CONSTANTS.MAX_ID_LENGTH} characters`);
        }
        if (typeof content !== 'string' || content.length > exports.NEURAL_CONSTANTS.MAX_CONTENT_LENGTH) {
            throw new Error(`Content exceeds maximum length: ${exports.NEURAL_CONSTANTS.MAX_CONTENT_LENGTH}`);
        }
        if (this.memories.size >= exports.NEURAL_CONSTANTS.MAX_MEMORIES && !this.memories.has(id)) {
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
        const entry = {
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
    recall(query, k = 5) {
        const q = query instanceof Float32Array ? query : new Float32Array(query);
        const now = Date.now();
        // Apply decay before recall
        this.applyDecay();
        // Score memories
        const scored = [];
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
            entry.strength = Math.min(1.0, entry.strength + exports.NEURAL_CONSTANTS.RECALL_STRENGTH_BOOST);
        }
        return results;
    }
    /**
     * Apply time-based decay to all memories
     */
    applyDecay() {
        const now = Date.now();
        const elapsed = Math.max(0, now - this.lastUpdate) / 60000; // minutes, prevent negative
        // Security: Cap maximum elapsed time to prevent manipulation
        const cappedElapsed = Math.min(elapsed, exports.NEURAL_CONSTANTS.MAX_TIME_JUMP_MINUTES);
        if (elapsed > exports.NEURAL_CONSTANTS.MAX_TIME_JUMP_MINUTES) {
            this.logger.log('warn', `Large time jump detected: ${elapsed.toFixed(0)} minutes`);
        }
        this.lastUpdate = now;
        const decayFactor = Math.exp(-this.config.memoryDecayRate * cappedElapsed);
        for (const entry of this.memories.values()) {
            // Decay is slower for consolidated memories
            const effectiveDecay = decayFactor + entry.consolidationLevel * (1 - decayFactor) * 0.8;
            entry.strength = Math.max(0, entry.strength * effectiveDecay);
            // Very weak memories are forgotten
            if (entry.strength < exports.NEURAL_CONSTANTS.MEMORY_FORGET_THRESHOLD) {
                this.memories.delete(entry.id);
            }
        }
    }
    /**
     * Consolidate memories (like sleep consolidation)
     * Strengthens frequently accessed, weakly interfered memories
     */
    consolidate() {
        let consolidated = 0;
        let forgotten = 0;
        for (const entry of this.memories.values()) {
            // Consolidation score based on access pattern and low interference
            const consolidationScore = Math.log(entry.accessCount + 1) * entry.strength * (1 - entry.interference * 0.5);
            if (consolidationScore > exports.NEURAL_CONSTANTS.CONSOLIDATION_SCORE_THRESHOLD) {
                entry.consolidationLevel = Math.min(1.0, entry.consolidationLevel + this.config.consolidationRate);
                entry.strength = Math.min(1.0, entry.strength + 0.05);
                consolidated++;
            }
            else if (entry.strength < exports.NEURAL_CONSTANTS.MEMORY_CLEANUP_PERCENT) {
                this.memories.delete(entry.id);
                forgotten++;
            }
        }
        return { consolidated, forgotten };
    }
    /**
     * Get memory statistics
     */
    getStats() {
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
    cosineSimilarity(a, b) {
        let dot = 0, normA = 0, normB = 0;
        for (let i = 0; i < a.length; i++) {
            dot += a[i] * b[i];
            normA += a[i] * a[i];
            normB += b[i] * b[i];
        }
        const denom = Math.sqrt(normA * normB);
        if (denom < 1e-10)
            return 0; // Handle zero vectors
        return Math.max(-1, Math.min(1, dot / denom)); // Clamp to valid range
    }
    /**
     * Force cleanup of weak memories when limit reached
     */
    forceCleanup() {
        const entries = Array.from(this.memories.entries())
            .sort((a, b) => a[1].strength - b[1].strength);
        const removeCount = Math.ceil(this.memories.size * exports.NEURAL_CONSTANTS.MEMORY_CLEANUP_PERCENT);
        for (let i = 0; i < removeCount; i++) {
            this.memories.delete(entries[i][0]);
        }
        this.logger.log('debug', `Force cleanup removed ${removeCount} weak memories`);
    }
}
exports.MemoryPhysics = MemoryPhysics;
// ============================================================================
// 3. EMBEDDING STATE MACHINE - Agent State via Geometry
// ============================================================================
/**
 * Manages agent state as movement through embedding space.
 * Decisions become geometric - no explicit state machine.
 */
class EmbeddingStateMachine {
    constructor(config = {}) {
        this.agents = new Map();
        this.modeRegions = new Map();
        this.lastCleanup = Date.now();
        this.config = {
            dimension: config.dimension ?? exports.NEURAL_CONSTANTS.DEFAULT_DIMENSION,
        };
        this.logger = config.logger ?? exports.defaultLogger;
    }
    /**
     * Create or update an agent
     */
    updateAgent(id, embedding) {
        // Periodically clean up stale agents
        this.cleanupStaleAgents();
        // Security: Enforce agent limit
        if (!this.agents.has(id) && this.agents.size >= exports.NEURAL_CONSTANTS.MAX_AGENTS) {
            throw new Error(`Agent limit reached: ${exports.NEURAL_CONSTANTS.MAX_AGENTS}`);
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
        }
        else {
            // New agent
            const state = {
                id,
                position,
                velocity: new Float32Array(this.config.dimension),
                attention: new Float32Array(this.config.dimension).fill(1 / this.config.dimension),
                energy: exports.NEURAL_CONSTANTS.DEFAULT_AGENT_ENERGY,
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
    cleanupStaleAgents() {
        const now = Date.now();
        // Only run cleanup every minute
        if (now - this.lastCleanup < 60000)
            return;
        this.lastCleanup = now;
        const cutoff = now - exports.NEURAL_CONSTANTS.AGENT_TIMEOUT_MS;
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
    removeAgent(id) {
        return this.agents.delete(id);
    }
    /**
     * Define a mode region in embedding space
     */
    defineMode(name, centroid, radius = 0.3) {
        const c = centroid instanceof Float32Array
            ? new Float32Array(centroid)
            : new Float32Array(centroid);
        this.modeRegions.set(name, { centroid: c, radius });
    }
    /**
     * Determine which mode an agent is in based on position
     */
    determineMode(position) {
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
    predictTrajectory(id, steps = 5) {
        // Security: Limit trajectory steps
        if (!Number.isInteger(steps) || steps < 1) {
            throw new Error('Steps must be a positive integer');
        }
        const limitedSteps = Math.min(steps, exports.NEURAL_CONSTANTS.MAX_TRAJECTORY_STEPS);
        const agent = this.agents.get(id);
        if (!agent)
            return [];
        const trajectory = [];
        let current = new Float32Array(agent.position);
        for (let i = 0; i < limitedSteps; i++) {
            const next = new Float32Array(current.length);
            for (let j = 0; j < current.length; j++) {
                next[j] = current[j] + agent.velocity[j] * (1 - i * exports.NEURAL_CONSTANTS.TRAJECTORY_DAMPING);
            }
            trajectory.push(next);
            current = next;
        }
        return trajectory;
    }
    /**
     * Apply attention to agent state
     */
    attendTo(agentId, focusEmbedding) {
        const agent = this.agents.get(agentId);
        if (!agent)
            return;
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
    getAgentsInMode(mode) {
        return Array.from(this.agents.values()).filter(a => a.mode === mode);
    }
    euclideanDistance(a, b) {
        let sum = 0;
        for (let i = 0; i < a.length; i++) {
            const diff = a[i] - b[i];
            sum += diff * diff;
        }
        return Math.sqrt(sum);
    }
}
exports.EmbeddingStateMachine = EmbeddingStateMachine;
// ============================================================================
// 4. SWARM COORDINATOR - Multi-Agent Coordination via Embeddings
// ============================================================================
/**
 * Enables multi-agent coordination through shared embedding space.
 * Swarm behavior emerges from geometry, not protocol.
 */
class SwarmCoordinator {
    constructor(config = {}) {
        this.agents = new Map();
        this.config = { dimension: config.dimension ?? exports.NEURAL_CONSTANTS.DEFAULT_DIMENSION };
        this.sharedContext = new Float32Array(this.config.dimension);
        this.logger = config.logger ?? exports.defaultLogger;
    }
    /**
     * Register an agent with the swarm
     */
    register(id, embedding, specialty = 'general') {
        // Security: Validate inputs
        if (typeof id !== 'string' || id.length === 0 || id.length > exports.NEURAL_CONSTANTS.MAX_ID_LENGTH) {
            throw new Error(`Invalid agent ID: must be string of 1-${exports.NEURAL_CONSTANTS.MAX_ID_LENGTH} characters`);
        }
        if (typeof specialty !== 'string' || specialty.length > exports.NEURAL_CONSTANTS.MAX_SPECIALTY_LENGTH) {
            throw new Error(`Specialty exceeds maximum length: ${exports.NEURAL_CONSTANTS.MAX_SPECIALTY_LENGTH}`);
        }
        if (this.agents.size >= exports.NEURAL_CONSTANTS.MAX_AGENTS && !this.agents.has(id)) {
            throw new Error(`Agent limit reached: ${exports.NEURAL_CONSTANTS.MAX_AGENTS}`);
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
    update(id, embedding) {
        const agent = this.agents.get(id);
        if (!agent)
            return;
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
    updateSharedContext() {
        if (this.agents.size === 0)
            return;
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
    getCoordinationSignal(id) {
        const agent = this.agents.get(id);
        if (!agent)
            return new Float32Array(this.config.dimension);
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
    findCollaborators(id, k = 3) {
        const agent = this.agents.get(id);
        if (!agent)
            return [];
        const scored = [];
        for (const [otherId, other] of this.agents) {
            if (otherId === id)
                continue;
            const similarity = this.cosineSimilarity(agent.position, other.position);
            scored.push({ id: otherId, similarity, specialty: other.specialty });
        }
        scored.sort((a, b) => b.similarity - a.similarity);
        return scored.slice(0, k);
    }
    /**
     * Detect emergent clusters (specialization)
     */
    detectClusters(threshold = exports.NEURAL_CONSTANTS.DEFAULT_CLUSTER_THRESHOLD) {
        // Security: Validate threshold
        if (threshold < 0 || threshold > 1) {
            throw new Error('Threshold must be between 0 and 1');
        }
        // Security: Limit clustering for performance (O(n²) algorithm)
        if (this.agents.size > exports.NEURAL_CONSTANTS.MAX_CLUSTER_AGENTS) {
            this.logger.log('warn', `Too many agents for clustering: ${this.agents.size} > ${exports.NEURAL_CONSTANTS.MAX_CLUSTER_AGENTS}`);
            // Return single cluster with all agents
            return new Map([['all', Array.from(this.agents.keys())]]);
        }
        const clusters = new Map();
        const assigned = new Set();
        for (const [id, agent] of this.agents) {
            if (assigned.has(id))
                continue;
            const cluster = [id];
            assigned.add(id);
            for (const [otherId, other] of this.agents) {
                if (assigned.has(otherId))
                    continue;
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
    getCoherence() {
        if (this.agents.size < 2)
            return 1.0;
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
    cosineSimilarity(a, b) {
        let dot = 0, normA = 0, normB = 0;
        for (let i = 0; i < a.length; i++) {
            dot += a[i] * b[i];
            normA += a[i] * a[i];
            normB += b[i] * b[i];
        }
        const denom = Math.sqrt(normA * normB);
        if (denom < exports.NEURAL_CONSTANTS.ZERO_VECTOR_THRESHOLD)
            return 0;
        return Math.max(-1, Math.min(1, dot / denom));
    }
    /**
     * Remove an agent from the swarm
     */
    removeAgent(id) {
        const removed = this.agents.delete(id);
        if (removed) {
            this.updateSharedContext();
        }
        return removed;
    }
}
exports.SwarmCoordinator = SwarmCoordinator;
// ============================================================================
// 5. COHERENCE MONITOR - Safety and Alignment Detection
// ============================================================================
/**
 * Monitors system coherence via embedding patterns.
 * Detects degradation, poisoning, misalignment before explicit failures.
 */
class CoherenceMonitor {
    constructor(config = {}) {
        this.history = [];
        this.baselineDistribution = null;
        this.config = {
            dimension: config.dimension ?? exports.NEURAL_CONSTANTS.DEFAULT_DIMENSION,
            windowSize: config.windowSize ?? exports.NEURAL_CONSTANTS.DEFAULT_WINDOW_SIZE,
        };
        this.logger = config.logger ?? exports.defaultLogger;
    }
    /**
     * Record an observation
     */
    observe(embedding, source = 'unknown') {
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
    calibrate() {
        if (this.history.length < exports.NEURAL_CONSTANTS.MIN_CALIBRATION_OBSERVATIONS) {
            throw new Error(`Need at least ${exports.NEURAL_CONSTANTS.MIN_CALIBRATION_OBSERVATIONS} observations to calibrate`);
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
    report() {
        const anomalies = [];
        // Drift score: how much has distribution shifted
        const driftScore = this.calculateDriftScore();
        if (driftScore > exports.NEURAL_CONSTANTS.DRIFT_WARNING_THRESHOLD) {
            anomalies.push({
                type: 'distribution_drift',
                severity: driftScore,
                description: 'Embedding distribution has shifted significantly from baseline',
            });
        }
        // Stability score: variance in recent observations
        const stabilityScore = this.calculateStabilityScore();
        if (stabilityScore < exports.NEURAL_CONSTANTS.STABILITY_WARNING_THRESHOLD) {
            anomalies.push({
                type: 'instability',
                severity: 1 - stabilityScore,
                description: 'High variance in recent embeddings suggests instability',
            });
        }
        // Alignment score: consistency of embeddings from same source
        const alignmentScore = this.calculateAlignmentScore();
        if (alignmentScore < exports.NEURAL_CONSTANTS.ALIGNMENT_WARNING_THRESHOLD) {
            anomalies.push({
                type: 'misalignment',
                severity: 1 - alignmentScore,
                description: 'Embeddings from same source show inconsistent patterns',
            });
        }
        // Overall score
        const overallScore = ((1 - driftScore) * 0.3 +
            stabilityScore * 0.3 +
            alignmentScore * 0.4);
        return {
            timestamp: Date.now(),
            overallScore,
            driftScore,
            stabilityScore,
            alignmentScore,
            anomalies,
        };
    }
    calculateDriftScore() {
        if (!this.baselineDistribution || this.history.length < exports.NEURAL_CONSTANTS.RECENT_OBSERVATIONS_SIZE)
            return 0;
        const recent = this.history.slice(-exports.NEURAL_CONSTANTS.RECENT_OBSERVATIONS_SIZE);
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
    calculateStabilityScore() {
        if (this.history.length < exports.NEURAL_CONSTANTS.STABILITY_WINDOW_SIZE)
            return 1.0;
        const recent = this.history.slice(-exports.NEURAL_CONSTANTS.STABILITY_WINDOW_SIZE);
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
    calculateAlignmentScore() {
        // Group by source and check consistency
        const bySource = new Map();
        for (const obs of this.history.slice(-exports.NEURAL_CONSTANTS.ALIGNMENT_WINDOW_SIZE)) {
            if (!bySource.has(obs.source)) {
                bySource.set(obs.source, []);
            }
            bySource.get(obs.source).push(obs.embedding);
        }
        if (bySource.size < 2)
            return 1.0;
        let totalConsistency = 0;
        let count = 0;
        for (const embeddings of bySource.values()) {
            if (embeddings.length < 2)
                continue;
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
    cosineSimilarity(a, b) {
        let dot = 0, normA = 0, normB = 0;
        for (let i = 0; i < a.length; i++) {
            dot += a[i] * b[i];
            normA += a[i] * a[i];
            normB += b[i] * b[i];
        }
        return dot / (Math.sqrt(normA * normB) + 1e-8);
    }
}
exports.CoherenceMonitor = CoherenceMonitor;
// ============================================================================
// 6. NEURAL SUBSTRATE - Synthetic Nervous System
// ============================================================================
/**
 * Unified neural embedding substrate combining all components.
 * Acts like a synthetic nervous system with reflexes, memory, and coordination.
 */
class NeuralSubstrate {
    constructor(config = {}) {
        this.logger = config.logger ?? exports.defaultLogger;
        this.config = {
            dimension: config.dimension ?? exports.NEURAL_CONSTANTS.DEFAULT_DIMENSION,
            driftThreshold: config.driftThreshold ?? exports.NEURAL_CONSTANTS.DEFAULT_DRIFT_THRESHOLD,
            driftWindowMs: config.driftWindowMs ?? exports.NEURAL_CONSTANTS.DEFAULT_DRIFT_WINDOW_MS,
            memoryDecayRate: config.memoryDecayRate ?? exports.NEURAL_CONSTANTS.DEFAULT_MEMORY_DECAY_RATE,
            interferenceThreshold: config.interferenceThreshold ?? exports.NEURAL_CONSTANTS.DEFAULT_INTERFERENCE_THRESHOLD,
            consolidationRate: config.consolidationRate ?? exports.NEURAL_CONSTANTS.DEFAULT_CONSOLIDATION_RATE,
            reflexLatencyMs: config.reflexLatencyMs ?? exports.NEURAL_CONSTANTS.DEFAULT_REFLEX_LATENCY_MS,
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
                if (report.overallScore < exports.NEURAL_CONSTANTS.COHERENCE_WARNING_THRESHOLD) {
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
    process(embedding, options = {}) {
        const emb = embedding instanceof Float32Array
            ? embedding
            : new Float32Array(embedding);
        // 1. Observe for drift
        const driftEvent = this.drift.observe(emb, options.source);
        // 2. Encode to memory if content provided
        let memoryEntry = null;
        if (options.memoryId && options.content) {
            memoryEntry = this.memory.encode(options.memoryId, emb, options.content);
        }
        // 3. Update agent state if ID provided
        let agentState = null;
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
    query(embedding, k = 5) {
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
    health() {
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
    consolidate() {
        return this.memory.consolidate();
    }
    /**
     * Calibrate coherence baseline
     */
    calibrate() {
        this.coherence.calibrate();
    }
}
exports.NeuralSubstrate = NeuralSubstrate;
// ============================================================================
// Exports
// ============================================================================
exports.default = NeuralSubstrate;
//# sourceMappingURL=neural-embeddings.js.map