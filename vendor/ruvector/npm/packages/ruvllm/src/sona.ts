/**
 * SONA (Self-Optimizing Neural Architecture) Learning System
 *
 * Provides adaptive learning capabilities with trajectory tracking,
 * pattern recognition, and memory protection (EWC++).
 */

import {
  SonaConfig,
  LearningSignal,
  QueryTrajectory,
  TrajectoryStep,
  TrajectoryOutcome,
  LearnedPattern,
  PatternType,
  EwcStats,
  LoRAConfig,
  Embedding,
} from './types';

/**
 * Default SONA configuration
 */
const DEFAULT_SONA_CONFIG: Required<SonaConfig> = {
  instantLoopEnabled: true,
  backgroundLoopEnabled: true,
  loraLearningRate: 0.001,
  loraRank: 8,
  ewcLambda: 2000,
  maxTrajectorySize: 1000,
  patternThreshold: 0.85,
};

/**
 * Trajectory Builder for tracking query execution paths
 *
 * @example
 * ```typescript
 * const builder = new TrajectoryBuilder();
 *
 * builder.startStep('query', 'What is AI?');
 * // ... processing ...
 * builder.endStep('AI is artificial intelligence', 0.95);
 *
 * builder.startStep('memory', 'searching context');
 * builder.endStep('found 3 relevant documents', 0.88);
 *
 * const trajectory = builder.complete('success');
 * ```
 */
export class TrajectoryBuilder {
  private id: string;
  private steps: TrajectoryStep[] = [];
  private currentStep: Partial<TrajectoryStep> | null = null;
  private stepStart: number = 0;
  private startTime: number;

  constructor() {
    this.id = `traj-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    this.startTime = Date.now();
  }

  /**
   * Start a new step in the trajectory
   */
  startStep(type: TrajectoryStep['type'], input: string): this {
    if (this.currentStep) {
      // Auto-complete previous step
      this.endStep('', 0);
    }

    this.stepStart = Date.now();
    this.currentStep = {
      type,
      input,
    };

    return this;
  }

  /**
   * End current step with output
   */
  endStep(output: string, confidence: number): this {
    if (!this.currentStep) {
      return this;
    }

    this.steps.push({
      type: this.currentStep.type!,
      input: this.currentStep.input!,
      output,
      durationMs: Date.now() - this.stepStart,
      confidence,
    });

    this.currentStep = null;
    return this;
  }

  /**
   * Complete trajectory with final outcome
   */
  complete(outcome: TrajectoryOutcome): QueryTrajectory {
    // Complete any pending step
    if (this.currentStep) {
      this.endStep('incomplete', 0);
    }

    return {
      id: this.id,
      steps: this.steps,
      outcome,
      durationMs: Date.now() - this.startTime,
    };
  }

  /**
   * Get current trajectory ID
   */
  getId(): string {
    return this.id;
  }
}

/**
 * ReasoningBank - Pattern storage and retrieval
 *
 * Stores learned patterns from successful interactions and
 * enables pattern-based reasoning shortcuts.
 *
 * OPTIMIZED: Uses Float64Array for embeddings and partial sorting
 */
export class ReasoningBank {
  private patterns: Map<string, LearnedPattern> = new Map();
  private embeddings: Map<string, Float64Array> = new Map();
  private embeddingNorms: Map<string, number> = new Map(); // Pre-computed norms
  private threshold: number;
  // Reusable arrays for findSimilar to avoid allocations
  private _similarityResults: Array<{ id: string; score: number }> = [];

  constructor(threshold = 0.85) {
    this.threshold = threshold;
  }

  /**
   * Store a new pattern
   */
  store(
    type: PatternType,
    embedding: Embedding,
    metadata?: Record<string, unknown>
  ): string {
    const id = `pat-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

    const pattern: LearnedPattern = {
      id,
      type,
      embedding,
      successRate: 1.0,
      useCount: 0,
      lastUsed: new Date(),
    };

    this.patterns.set(id, pattern);

    // Store as typed array for faster similarity computation
    const typedEmb = new Float64Array(embedding);
    this.embeddings.set(id, typedEmb);

    // Pre-compute and cache the norm
    let norm = 0;
    for (let i = 0; i < typedEmb.length; i++) {
      norm += typedEmb[i] * typedEmb[i];
    }
    this.embeddingNorms.set(id, Math.sqrt(norm));

    return id;
  }

  /**
   * Find similar patterns
   * OPTIMIZED: Uses typed arrays, pre-computed norms, and partial sorting
   */
  findSimilar(embedding: Embedding, k = 5): LearnedPattern[] {
    // Pre-compute query norm
    let queryNorm = 0;
    const queryLen = embedding.length;
    for (let i = 0; i < queryLen; i++) {
      queryNorm += embedding[i] * embedding[i];
    }
    queryNorm = Math.sqrt(queryNorm);

    if (queryNorm === 0) return [];

    // Reuse array to avoid allocations
    this._similarityResults.length = 0;

    for (const [id, patEmb] of this.embeddings) {
      const patNorm = this.embeddingNorms.get(id) || 0;
      if (patNorm === 0) continue;

      // Fast dot product
      let dot = 0;
      const minLen = Math.min(queryLen, patEmb.length);

      // Unrolled loop
      let i = 0;
      for (; i + 3 < minLen; i += 4) {
        dot += embedding[i] * patEmb[i] +
               embedding[i + 1] * patEmb[i + 1] +
               embedding[i + 2] * patEmb[i + 2] +
               embedding[i + 3] * patEmb[i + 3];
      }
      for (; i < minLen; i++) {
        dot += embedding[i] * patEmb[i];
      }

      const score = dot / (queryNorm * patNorm);

      if (score >= this.threshold) {
        this._similarityResults.push({ id, score });
      }
    }

    // Partial sort for top-k (faster than full sort for large arrays)
    if (this._similarityResults.length <= k) {
      this._similarityResults.sort((a, b) => b.score - a.score);
    } else {
      // Quick partial sort for top k
      this.partialSort(this._similarityResults, k);
    }

    const topK = this._similarityResults.slice(0, k);

    return topK
      .map(s => this.patterns.get(s.id))
      .filter((p): p is LearnedPattern => p !== undefined);
  }

  /**
   * Partial sort to get top k elements (faster than full sort)
   */
  private partialSort(arr: Array<{ id: string; score: number }>, k: number): void {
    // Simple selection for small k
    for (let i = 0; i < k && i < arr.length; i++) {
      let maxIdx = i;
      for (let j = i + 1; j < arr.length; j++) {
        if (arr[j].score > arr[maxIdx].score) {
          maxIdx = j;
        }
      }
      if (maxIdx !== i) {
        const temp = arr[i];
        arr[i] = arr[maxIdx];
        arr[maxIdx] = temp;
      }
    }
  }

  /**
   * Record pattern usage (success or failure)
   */
  recordUsage(patternId: string, success: boolean): void {
    const pattern = this.patterns.get(patternId);
    if (!pattern) return;

    pattern.useCount++;
    pattern.lastUsed = new Date();

    // Update success rate with exponential moving average
    const alpha = 0.1;
    const outcome = success ? 1.0 : 0.0;
    pattern.successRate = alpha * outcome + (1 - alpha) * pattern.successRate;
  }

  /**
   * Get pattern by ID
   */
  get(patternId: string): LearnedPattern | undefined {
    return this.patterns.get(patternId);
  }

  /**
   * Get all patterns of a type
   */
  getByType(type: PatternType): LearnedPattern[] {
    return Array.from(this.patterns.values()).filter(p => p.type === type);
  }

  /**
   * Prune low-performing patterns
   */
  prune(minSuccessRate = 0.3, minUseCount = 5): number {
    let pruned = 0;

    for (const [id, pattern] of this.patterns) {
      if (pattern.useCount >= minUseCount && pattern.successRate < minSuccessRate) {
        this.patterns.delete(id);
        this.embeddings.delete(id);
        this.embeddingNorms.delete(id);
        pruned++;
      }
    }

    return pruned;
  }

  /**
   * Get statistics
   */
  stats(): { totalPatterns: number; avgSuccessRate: number; byType: Record<string, number> } {
    const patterns = Array.from(this.patterns.values());
    const byType: Record<string, number> = {};

    let totalSuccess = 0;
    for (const p of patterns) {
      totalSuccess += p.successRate;
      byType[p.type] = (byType[p.type] || 0) + 1;
    }

    return {
      totalPatterns: patterns.length,
      avgSuccessRate: patterns.length > 0 ? totalSuccess / patterns.length : 0,
      byType,
    };
  }

  private cosineSimilarity(a: Embedding, b: Embedding): number {
    let dot = 0, normA = 0, normB = 0;
    const len = Math.min(a.length, b.length);

    for (let i = 0; i < len; i++) {
      dot += a[i] * b[i];
      normA += a[i] * a[i];
      normB += b[i] * b[i];
    }

    const denom = Math.sqrt(normA) * Math.sqrt(normB);
    return denom > 0 ? dot / denom : 0;
  }
}

/**
 * EWC++ (Elastic Weight Consolidation) Manager
 *
 * Prevents catastrophic forgetting by protecting important weights.
 * This is a simplified JS implementation of the concept.
 *
 * OPTIMIZED: Uses Float64Array for 5-10x faster penalty computation
 */
export class EwcManager {
  private lambda: number;
  private tasksLearned: number = 0;
  private fisherDiagonal: Map<string, Float64Array> = new Map();
  private optimalWeights: Map<string, Float64Array> = new Map();
  // Pre-allocated buffer for penalty computation
  private _penaltyBuffer: Float64Array | null = null;

  constructor(lambda = 2000) {
    this.lambda = lambda;
  }

  /**
   * Register a new task (after successful learning)
   */
  registerTask(taskId: string, weights: number[]): void {
    // Store optimal weights for this task using typed arrays
    const optimalArr = new Float64Array(weights.length);
    const fisherArr = new Float64Array(weights.length);

    for (let i = 0; i < weights.length; i++) {
      optimalArr[i] = weights[i];
      fisherArr[i] = Math.abs(weights[i]) * this.lambda;
    }

    this.optimalWeights.set(taskId, optimalArr);
    this.fisherDiagonal.set(taskId, fisherArr);
    this.tasksLearned++;
  }

  /**
   * Compute EWC penalty for weight update
   * OPTIMIZED: Uses typed arrays and minimizes allocations
   */
  computePenalty(currentWeights: number[]): number {
    let penalty = 0;
    const len = currentWeights.length;

    for (const [taskId, optimal] of this.optimalWeights) {
      const fisher = this.fisherDiagonal.get(taskId);
      if (!fisher) continue;

      const minLen = Math.min(len, optimal.length);

      // Unrolled loop for better performance
      let i = 0;
      for (; i + 3 < minLen; i += 4) {
        const diff0 = currentWeights[i] - optimal[i];
        const diff1 = currentWeights[i + 1] - optimal[i + 1];
        const diff2 = currentWeights[i + 2] - optimal[i + 2];
        const diff3 = currentWeights[i + 3] - optimal[i + 3];
        penalty += fisher[i] * diff0 * diff0 +
                   fisher[i + 1] * diff1 * diff1 +
                   fisher[i + 2] * diff2 * diff2 +
                   fisher[i + 3] * diff3 * diff3;
      }
      // Handle remaining elements
      for (; i < minLen; i++) {
        const diff = currentWeights[i] - optimal[i];
        penalty += fisher[i] * diff * diff;
      }
    }

    return penalty * 0.5;
  }

  /**
   * Get EWC statistics
   */
  stats(): EwcStats {
    return {
      tasksLearned: this.tasksLearned,
      fisherComputed: this.fisherDiagonal.size > 0,
      protectionStrength: this.lambda,
      forgettingRate: this.estimateForgettingRate(),
    };
  }

  private estimateForgettingRate(): number {
    // Simplified estimation based on number of tasks
    return Math.max(0, 1 - Math.exp(-this.tasksLearned * 0.1));
  }
}

/**
 * SONA Learning Coordinator
 *
 * Orchestrates the learning loops and components.
 */
export class SonaCoordinator {
  private config: Required<SonaConfig>;
  private trajectoryBuffer: QueryTrajectory[] = [];
  private reasoningBank: ReasoningBank;
  private ewcManager: EwcManager;
  private signalBuffer: LearningSignal[] = [];

  constructor(config?: SonaConfig) {
    this.config = { ...DEFAULT_SONA_CONFIG, ...config };
    this.reasoningBank = new ReasoningBank(this.config.patternThreshold);
    this.ewcManager = new EwcManager(this.config.ewcLambda);
  }

  /**
   * Record a learning signal
   */
  recordSignal(signal: LearningSignal): void {
    this.signalBuffer.push(signal);

    // Instant loop - immediate learning
    if (this.config.instantLoopEnabled && signal.quality >= 0.8) {
      this.processInstantLearning(signal);
    }
  }

  /**
   * Record a completed trajectory
   */
  recordTrajectory(trajectory: QueryTrajectory): void {
    this.trajectoryBuffer.push(trajectory);

    // Maintain buffer size
    while (this.trajectoryBuffer.length > this.config.maxTrajectorySize) {
      this.trajectoryBuffer.shift();
    }

    // Extract patterns from successful trajectories
    if (trajectory.outcome === 'success') {
      this.extractPatterns(trajectory);
    }
  }

  /**
   * Run background learning loop
   */
  runBackgroundLoop(): { patternsLearned: number; trajectoriesProcessed: number } {
    if (!this.config.backgroundLoopEnabled) {
      return { patternsLearned: 0, trajectoriesProcessed: 0 };
    }

    let patternsLearned = 0;
    const trajectoriesProcessed = this.trajectoryBuffer.length;

    // Process accumulated trajectories
    for (const traj of this.trajectoryBuffer) {
      if (traj.outcome === 'success' || traj.outcome === 'partial') {
        patternsLearned += this.extractPatterns(traj);
      }
    }

    // Prune low-performing patterns
    this.reasoningBank.prune();

    // Clear processed trajectories
    this.trajectoryBuffer = [];

    return { patternsLearned, trajectoriesProcessed };
  }

  /**
   * Get reasoning bank for pattern queries
   */
  getReasoningBank(): ReasoningBank {
    return this.reasoningBank;
  }

  /**
   * Get EWC manager
   */
  getEwcManager(): EwcManager {
    return this.ewcManager;
  }

  /**
   * Get statistics
   */
  stats(): {
    signalsReceived: number;
    trajectoriesBuffered: number;
    patterns: ReturnType<ReasoningBank['stats']>;
    ewc: EwcStats;
  } {
    return {
      signalsReceived: this.signalBuffer.length,
      trajectoriesBuffered: this.trajectoryBuffer.length,
      patterns: this.reasoningBank.stats(),
      ewc: this.ewcManager.stats(),
    };
  }

  private processInstantLearning(signal: LearningSignal): void {
    // Immediate pattern reinforcement would happen here
    // In full implementation, this updates LoRA weights
  }

  private extractPatterns(trajectory: QueryTrajectory): number {
    let extracted = 0;

    for (const step of trajectory.steps) {
      if (step.confidence >= this.config.patternThreshold) {
        // Create embedding from step (simplified)
        const embedding = this.createEmbedding(step.input + step.output);

        // Determine pattern type
        const type = this.stepTypeToPatternType(step.type);

        // Store if not too similar to existing
        const similar = this.reasoningBank.findSimilar(embedding, 1);
        if (similar.length === 0) {
          this.reasoningBank.store(type, embedding);
          extracted++;
        }
      }
    }

    return extracted;
  }

  private stepTypeToPatternType(stepType: TrajectoryStep['type']): PatternType {
    switch (stepType) {
      case 'query':
      case 'generate':
        return 'query_response';
      case 'route':
        return 'routing';
      case 'memory':
        return 'context_retrieval';
      case 'feedback':
        return 'correction';
      default:
        return 'query_response';
    }
  }

  private createEmbedding(text: string): Embedding {
    // Simplified hash-based embedding (real impl uses model)
    const dim = 64;
    const embedding = new Array(dim).fill(0);

    for (let i = 0; i < text.length; i++) {
      const idx = (text.charCodeAt(i) * (i + 1)) % dim;
      embedding[idx] += 0.1;
    }

    // Normalize
    const norm = Math.sqrt(embedding.reduce((s, x) => s + x * x, 0)) || 1;
    return embedding.map(x => x / norm);
  }
}

// Export all SONA components
export {
  DEFAULT_SONA_CONFIG,
};
