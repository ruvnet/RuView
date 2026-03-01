# ADR-007: Learning System

**Status:** Accepted
**Date:** 2026-01-27
**Decision Makers:** RuVector Architecture Team
**Technical Area:** Machine Learning, Self-Optimization

---

## Context and Problem Statement

RuvBot must continuously improve its responses through:

1. **Trajectory Learning** - Learn from successful/failed conversation paths
2. **Pattern Recognition** - Identify recurring situations for faster responses
3. **Personalization** - Adapt to individual user preferences
4. **Knowledge Consolidation** - Prevent catastrophic forgetting while learning new patterns

The learning system must balance:

- **Stability**: Don't forget important learned behaviors
- **Plasticity**: Quickly adapt to new patterns
- **Efficiency**: Learn without excessive compute
- **Privacy**: Respect tenant data boundaries

---

## Decision Drivers

### Learning Requirements

| Requirement | Priority | Description |
|-------------|----------|-------------|
| Trajectory collection | Critical | Record conversation paths with outcomes |
| Pattern matching | Critical | Find similar past situations quickly |
| Online learning | High | Incrementally update without full retraining |
| EWC protection | High | Prevent forgetting important patterns |
| LoRA fine-tuning | Medium | Efficient adapter training |
| Federated learning | Low | Cross-tenant pattern sharing (opt-in) |

### Performance Requirements

| Operation | Target |
|-----------|--------|
| Pattern match | < 20ms |
| Trajectory record | < 5ms |
| Online update | < 100ms |
| Batch training | Background (hours OK) |

---

## Decision Outcome

### Adopt RJDC Pipeline (Retrieve, Judge, Distill, Consolidate)

We implement a four-stage learning pipeline inspired by RuVector's intelligence system:

```
+-----------------------------------------------------------------------------+
|                           LEARNING PIPELINE (RJDC)                           |
+-----------------------------------------------------------------------------+

   User Interaction
          |
          v
+--------------------+
|  1. RETRIEVE       |  Search for similar past patterns/trajectories
|  (HNSW + Patterns) |  via semantic similarity
+----------+---------+
           |
           v
+--------------------+
|  2. JUDGE          |  Evaluate interaction outcome
|  (Verdict System)  |  positive/negative/neutral
+----------+---------+
           |
           v
+--------------------+
|  3. DISTILL        |  Extract learnings via LoRA
|  (LoRA Training)   |  on successful trajectories
+----------+---------+
           |
           v
+--------------------+
|  4. CONSOLIDATE    |  Prevent forgetting via EWC++
|  (EWC Protection)  |  while integrating new knowledge
+--------------------+
```

---

## Trajectory Collection

### Trajectory Structure

```typescript
// Complete trajectory from session
interface Trajectory {
  id: TrajectoryId;
  sessionId: SessionId;
  tenantId: TenantId;

  // Conversation path
  turns: TurnSnapshot[];

  // Skills invoked
  skillExecutions: SkillExecution[];

  // Retrieved memories used
  memoryRetrievals: MemoryRetrieval[];

  // Timing
  startTime: Date;
  endTime: Date;
  totalLatencyMs: number;

  // Outcome
  verdict?: Verdict;
  verdictReason?: string;
  verdictSource?: VerdictSource;

  // Embedding for pattern matching
  embeddingId?: string;

  // Metadata
  metadata: TrajectoryMetadata;
}

interface TurnSnapshot {
  turnId: TurnId;
  role: 'user' | 'assistant' | 'system';
  content: string;
  contentEmbedding: Float32Array;
  tokenCount: number;
  latencyMs: number;
  timestamp: Date;
}

interface SkillExecution {
  skillId: SkillId;
  params: Record<string, unknown>;
  result: SkillResult;
  success: boolean;
  latencyMs: number;
}

interface MemoryRetrieval {
  memoryId: MemoryId;
  memoryType: 'episodic' | 'semantic' | 'procedural';
  relevanceScore: number;
  wasUseful: boolean; // Determined post-hoc
}

type Verdict = 'positive' | 'negative' | 'neutral';
type VerdictSource = 'explicit_feedback' | 'implicit_signal' | 'heuristic' | 'manual_label';
```

### Trajectory Collector

```typescript
// Automatic trajectory collection during sessions
class TrajectoryCollector {
  private activeTrajectories: Map<SessionId, Trajectory> = new Map();

  constructor(
    private storage: TrajectoryStorage,
    private embedder: EmbeddingService,
    private verdictEngine: VerdictEngine
  ) {}

  // Start tracking a session
  startSession(session: Session): void {
    const trajectory: Trajectory = {
      id: crypto.randomUUID(),
      sessionId: session.id,
      tenantId: session.tenantId,
      turns: [],
      skillExecutions: [],
      memoryRetrievals: [],
      startTime: new Date(),
      endTime: new Date(),
      totalLatencyMs: 0,
      metadata: {
        agentId: session.agentId,
        userId: session.userId,
        channel: session.channel,
      },
    };

    this.activeTrajectories.set(session.id, trajectory);
  }

  // Record a conversation turn
  async recordTurn(sessionId: SessionId, turn: ConversationTurn): Promise<void> {
    const trajectory = this.activeTrajectories.get(sessionId);
    if (!trajectory) return;

    const embedding = await this.embedder.embed(turn.content);

    trajectory.turns.push({
      turnId: turn.id,
      role: turn.role,
      content: turn.content,
      contentEmbedding: embedding,
      tokenCount: turn.metadata.tokenCount ?? 0,
      latencyMs: turn.metadata.latencyMs ?? 0,
      timestamp: turn.timestamp,
    });
  }

  // Record skill execution
  recordSkillExecution(sessionId: SessionId, execution: SkillExecution): void {
    const trajectory = this.activeTrajectories.get(sessionId);
    if (!trajectory) return;

    trajectory.skillExecutions.push(execution);
  }

  // Record memory retrieval
  recordMemoryRetrieval(sessionId: SessionId, retrieval: MemoryRetrieval): void {
    const trajectory = this.activeTrajectories.get(sessionId);
    if (!trajectory) return;

    trajectory.memoryRetrievals.push(retrieval);
  }

  // End session and finalize trajectory
  async endSession(sessionId: SessionId): Promise<Trajectory | null> {
    const trajectory = this.activeTrajectories.get(sessionId);
    if (!trajectory) return null;

    trajectory.endTime = new Date();
    trajectory.totalLatencyMs = trajectory.turns.reduce(
      (sum, t) => sum + t.latencyMs, 0
    );

    // Generate trajectory embedding (average of turn embeddings)
    if (trajectory.turns.length > 0) {
      const avgEmbedding = this.averageEmbeddings(
        trajectory.turns.map(t => t.contentEmbedding)
      );
      const embeddingId = await this.embedder.store(avgEmbedding);
      trajectory.embeddingId = embeddingId;
    }

    // Try to determine verdict automatically
    trajectory.verdict = await this.verdictEngine.evaluateTrajectory(trajectory);
    trajectory.verdictSource = trajectory.verdict ? 'heuristic' : undefined;

    // Save to storage
    await this.storage.save(trajectory);

    this.activeTrajectories.delete(sessionId);
    return trajectory;
  }

  private averageEmbeddings(embeddings: Float32Array[]): Float32Array {
    if (embeddings.length === 0) return new Float32Array(384);

    const avg = new Float32Array(embeddings[0].length);
    for (const emb of embeddings) {
      for (let i = 0; i < emb.length; i++) {
        avg[i] += emb[i];
      }
    }
    for (let i = 0; i < avg.length; i++) {
      avg[i] /= embeddings.length;
    }
    return avg;
  }
}
```

---

## Verdict System

### Verdict Engine

```typescript
// Determine trajectory outcomes
class VerdictEngine {
  constructor(
    private feedbackRepo: FeedbackRepository,
    private config: VerdictConfig
  ) {}

  async evaluateTrajectory(trajectory: Trajectory): Promise<Verdict | undefined> {
    // 1. Check for explicit feedback
    const explicitFeedback = await this.getExplicitFeedback(trajectory.sessionId);
    if (explicitFeedback) {
      return explicitFeedback.verdict;
    }

    // 2. Check implicit signals
    const implicitVerdict = this.evaluateImplicitSignals(trajectory);
    if (implicitVerdict) {
      return implicitVerdict;
    }

    // 3. Apply heuristics
    return this.applyHeuristics(trajectory);
  }

  private async getExplicitFeedback(sessionId: SessionId): Promise<Feedback | null> {
    return this.feedbackRepo.findBySession(sessionId);
  }

  private evaluateImplicitSignals(trajectory: Trajectory): Verdict | undefined {
    const signals: ImplicitSignal[] = [];

    // Long conversations without resolution = negative
    if (trajectory.turns.length > 20) {
      signals.push({ type: 'long_conversation', weight: -0.3 });
    }

    // Repeated similar questions = negative
    const repeatCount = this.countRepeatedQuestions(trajectory);
    if (repeatCount > 2) {
      signals.push({ type: 'repeated_questions', weight: -0.4 });
    }

    // User ended with thank you = positive
    const lastUserTurn = trajectory.turns
      .filter(t => t.role === 'user')
      .pop();
    if (lastUserTurn && this.containsGratitude(lastUserTurn.content)) {
      signals.push({ type: 'gratitude_expression', weight: 0.5 });
    }

    // Skill executed successfully = positive
    const successfulSkills = trajectory.skillExecutions.filter(e => e.success);
    if (successfulSkills.length > 0) {
      signals.push({ type: 'skill_success', weight: 0.3 * successfulSkills.length });
    }

    // Calculate aggregate
    const totalWeight = signals.reduce((sum, s) => sum + s.weight, 0);

    if (totalWeight > 0.3) return 'positive';
    if (totalWeight < -0.3) return 'negative';
    return undefined; // Neutral/uncertain
  }

  private applyHeuristics(trajectory: Trajectory): Verdict {
    // Default heuristics for unlabeled trajectories
    const turnCount = trajectory.turns.length;
    const avgLatency = trajectory.totalLatencyMs / Math.max(turnCount, 1);

    // Short, fast interactions are likely positive
    if (turnCount <= 4 && avgLatency < 2000) {
      return 'positive';
    }

    // Very long or slow = likely negative
    if (turnCount > 15 || avgLatency > 5000) {
      return 'negative';
    }

    return 'neutral';
  }

  private countRepeatedQuestions(trajectory: Trajectory): number {
    const userTurns = trajectory.turns.filter(t => t.role === 'user');
    const embeddings = userTurns.map(t => t.contentEmbedding);

    let repeats = 0;
    for (let i = 1; i < embeddings.length; i++) {
      for (let j = 0; j < i; j++) {
        const similarity = cosineSimilarity(embeddings[i], embeddings[j]);
        if (similarity > 0.9) {
          repeats++;
          break;
        }
      }
    }
    return repeats;
  }

  private containsGratitude(text: string): boolean {
    const gratitudePatterns = [
      /\bthank(s| you)\b/i,
      /\bappreciate\b/i,
      /\bhelpful\b/i,
      /\bgreat\b/i,
      /\bperfect\b/i,
    ];
    return gratitudePatterns.some(p => p.test(text));
  }
}
```

### Feedback Collection

```typescript
// Explicit feedback handling
interface Feedback {
  id: string;
  trajectoryId: TrajectoryId;
  sessionId: SessionId;
  verdict: Verdict;
  rating?: number;  // 1-5 scale
  comment?: string;
  source: 'button' | 'rating' | 'comment' | 'api';
  timestamp: Date;
}

class FeedbackCollector {
  constructor(
    private feedbackRepo: FeedbackRepository,
    private trajectoryRepo: TrajectoryRepository,
    private learningQueue: Queue
  ) {}

  async collectFeedback(feedback: Omit<Feedback, 'id' | 'timestamp'>): Promise<void> {
    const fullFeedback: Feedback = {
      ...feedback,
      id: crypto.randomUUID(),
      timestamp: new Date(),
    };

    await this.feedbackRepo.save(fullFeedback);

    // Update trajectory verdict
    await this.trajectoryRepo.updateVerdict(
      feedback.trajectoryId,
      feedback.verdict,
      'explicit_feedback'
    );

    // Trigger learning if significant feedback
    if (feedback.verdict === 'positive' || feedback.verdict === 'negative') {
      await this.learningQueue.add('process-feedback', {
        feedbackId: fullFeedback.id,
        trajectoryId: feedback.trajectoryId,
      });
    }
  }
}
```

---

## Pattern System

### Pattern Structure

```typescript
// Learned pattern for quick matching
interface LearnedPattern {
  id: PatternId;
  tenantId: TenantId;
  workspaceId?: string;

  // Pattern definition
  patternType: PatternType;
  embedding: Float32Array;
  exemplarTrajectoryIds: TrajectoryId[];

  // Pattern behavior
  suggestedResponse?: string;
  suggestedSkills?: SkillId[];
  contextModifiers?: ContextModifier[];

  // Quality metrics
  confidence: number;
  usageCount: number;
  successCount: number;
  successRate: number;

  // Lifecycle
  isActive: boolean;
  createdAt: Date;
  lastUsedAt: Date;
  supersededBy?: PatternId;
}

type PatternType =
  | 'response'           // Cached response for similar queries
  | 'skill_selection'    // Which skill to invoke
  | 'memory_retrieval'   // What memories are relevant
  | 'conversation_flow'; // Expected next turns
```

### Pattern Index

```typescript
// HNSW-backed pattern matching
class PatternIndex {
  private hnswIndex: WasmHnswIndex;
  private patternStore: PatternStore;
  private cache: LRUCache<string, LearnedPattern>;

  constructor(private config: PatternIndexConfig) {
    this.cache = new LRUCache({ max: 1000 });
  }

  async initialize(): Promise<void> {
    this.hnswIndex = new WasmHnswIndex({
      dimensions: 384,
      m: 32,
      efConstruction: 200,
      efSearch: 100,
      distanceMetric: 'cosine',
    });
    await this.hnswIndex.initialize();

    // Load existing patterns
    const patterns = await this.patternStore.findActive();
    for (const pattern of patterns) {
      await this.hnswIndex.insert(pattern.id, pattern.embedding);
    }
  }

  async findMatches(
    query: Float32Array,
    options: PatternMatchOptions = {}
  ): Promise<PatternMatch[]> {
    const k = options.limit ?? 5;
    const threshold = options.threshold ?? 0.75;

    const searchResults = await this.hnswIndex.search(query, k * 2);

    const matches: PatternMatch[] = [];
    for (const result of searchResults) {
      if (result.score < threshold) continue;

      const pattern = await this.getPattern(result.id);
      if (!pattern || !pattern.isActive) continue;

      // Apply confidence weighting
      const adjustedScore = result.score * pattern.confidence;

      matches.push({
        pattern,
        score: adjustedScore,
        rawSimilarity: result.score,
      });
    }

    // Sort by adjusted score
    matches.sort((a, b) => b.score - a.score);
    return matches.slice(0, k);
  }

  async addPattern(pattern: LearnedPattern): Promise<void> {
    await this.patternStore.save(pattern);
    await this.hnswIndex.insert(pattern.id, pattern.embedding);
    this.cache.set(pattern.id, pattern);
  }

  async updatePatternStats(
    patternId: PatternId,
    used: boolean,
    successful: boolean
  ): Promise<void> {
    const pattern = await this.getPattern(patternId);
    if (!pattern) return;

    pattern.usageCount++;
    pattern.lastUsedAt = new Date();

    if (used && successful) {
      pattern.successCount++;
    }

    pattern.successRate = pattern.successCount / pattern.usageCount;

    // Update confidence based on usage
    pattern.confidence = this.calculateConfidence(pattern);

    // Deactivate low-performing patterns
    if (pattern.usageCount > 10 && pattern.successRate < 0.3) {
      pattern.isActive = false;
    }

    await this.patternStore.update(pattern);
    this.cache.set(patternId, pattern);
  }

  private calculateConfidence(pattern: LearnedPattern): number {
    // Bayesian-ish confidence with prior
    const prior = 0.5;
    const alpha = pattern.successCount + 1;
    const beta = pattern.usageCount - pattern.successCount + 1;
    return alpha / (alpha + beta);
  }

  private async getPattern(id: PatternId): Promise<LearnedPattern | null> {
    const cached = this.cache.get(id);
    if (cached) return cached;

    const pattern = await this.patternStore.findById(id);
    if (pattern) {
      this.cache.set(id, pattern);
    }
    return pattern;
  }
}

interface PatternMatchOptions {
  limit?: number;
  threshold?: number;
  patternTypes?: PatternType[];
}

interface PatternMatch {
  pattern: LearnedPattern;
  score: number;
  rawSimilarity: number;
}
```

---

## LoRA Training

### LoRA Trainer

```typescript
// Efficient fine-tuning via Low-Rank Adaptation
class LoRATrainer {
  constructor(
    private runtime: RuVectorRuntime,
    private config: LoRAConfig
  ) {}

  async train(
    trajectories: LabeledTrajectory[],
    baseModel: ModelWeights
  ): Promise<LoRAWeights> {
    // Filter to positive trajectories
    const positiveTrajectories = trajectories.filter(
      t => t.verdict === 'positive'
    );

    if (positiveTrajectories.length < this.config.minTrajectories) {
      throw new Error(`Insufficient positive trajectories: ${positiveTrajectories.length}`);
    }

    // Prepare training data
    const trainingData = this.prepareTrainingData(positiveTrajectories);

    // Initialize LoRA weights
    const loraWeights = this.initializeLoRA(baseModel);

    // Training loop
    for (let epoch = 0; epoch < this.config.epochs; epoch++) {
      const epochLoss = await this.trainEpoch(loraWeights, trainingData);

      if (epoch % 10 === 0) {
        console.log(`Epoch ${epoch}: loss = ${epochLoss.toFixed(4)}`);
      }

      // Early stopping
      if (epochLoss < this.config.targetLoss) {
        console.log(`Early stopping at epoch ${epoch}`);
        break;
      }
    }

    return loraWeights;
  }

  private prepareTrainingData(trajectories: LabeledTrajectory[]): TrainingData {
    const samples: TrainingSample[] = [];

    for (const trajectory of trajectories) {
      // Create samples from conversation turns
      for (let i = 1; i < trajectory.turns.length; i++) {
        const context = trajectory.turns.slice(0, i);
        const target = trajectory.turns[i];

        if (target.role === 'assistant') {
          samples.push({
            context: context.map(t => ({ role: t.role, content: t.content })),
            target: target.content,
            weight: this.calculateSampleWeight(trajectory, i),
          });
        }
      }
    }

    return { samples, totalSamples: samples.length };
  }

  private calculateSampleWeight(
    trajectory: LabeledTrajectory,
    turnIndex: number
  ): number {
    // Weight later turns higher (closer to outcome)
    const positionWeight = turnIndex / trajectory.turns.length;

    // Weight based on confidence of trajectory verdict
    const verdictWeight = trajectory.verdictConfidence ?? 1.0;

    return positionWeight * verdictWeight;
  }

  private initializeLoRA(baseModel: ModelWeights): LoRAWeights {
    const { learning } = this.runtime.getModules();

    return {
      rank: this.config.rank,
      alpha: this.config.alpha,
      layerAdapters: baseModel.layers.map((_, idx) => ({
        layerIndex: idx,
        A: learning.initializeRandom(baseModel.hiddenSize, this.config.rank),
        B: learning.initializeZeros(this.config.rank, baseModel.hiddenSize),
      })),
    };
  }

  private async trainEpoch(
    loraWeights: LoRAWeights,
    data: TrainingData
  ): Promise<number> {
    const { learning } = this.runtime.getModules();

    let totalLoss = 0;
    const batchSize = this.config.batchSize;

    // Shuffle samples
    const shuffled = this.shuffle(data.samples);

    for (let i = 0; i < shuffled.length; i += batchSize) {
      const batch = shuffled.slice(i, i + batchSize);
      const batchLoss = await learning.trainBatch(loraWeights, batch, {
        learningRate: this.config.learningRate,
        weightDecay: this.config.weightDecay,
      });
      totalLoss += batchLoss;
    }

    return totalLoss / (shuffled.length / batchSize);
  }

  private shuffle<T>(array: T[]): T[] {
    const shuffled = [...array];
    for (let i = shuffled.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
    }
    return shuffled;
  }

  // Merge LoRA weights into base model
  async merge(
    baseModel: ModelWeights,
    loraWeights: LoRAWeights
  ): Promise<ModelWeights> {
    const { learning } = this.runtime.getModules();

    const mergedLayers = baseModel.layers.map((layer, idx) => {
      const adapter = loraWeights.layerAdapters.find(a => a.layerIndex === idx);
      if (!adapter) return layer;

      // W' = W + (alpha/rank) * A * B
      const delta = learning.matmul(adapter.A, adapter.B);
      const scaled = learning.scale(delta, loraWeights.alpha / loraWeights.rank);
      return learning.add(layer, scaled);
    });

    return { ...baseModel, layers: mergedLayers };
  }
}

interface LoRAConfig {
  rank: number;           // Low-rank dimension (8-64)
  alpha: number;          // Scaling factor
  epochs: number;         // Training epochs
  batchSize: number;      // Samples per batch
  learningRate: number;   // Optimizer learning rate
  weightDecay: number;    // L2 regularization
  targetLoss: number;     // Early stopping threshold
  minTrajectories: number; // Minimum training data
}

interface LoRAWeights {
  rank: number;
  alpha: number;
  layerAdapters: LayerAdapter[];
}

interface LayerAdapter {
  layerIndex: number;
  A: Float32Array;  // Down-projection
  B: Float32Array;  // Up-projection
}
```

---

## EWC Consolidation

### Elastic Weight Consolidation

```typescript
// Prevent catastrophic forgetting via EWC++
class EWCConsolidator {
  private fisherMatrices: Map<string, FisherMatrix> = new Map();

  constructor(
    private runtime: RuVectorRuntime,
    private config: EWCConfig
  ) {}

  // Compute Fisher Information Matrix from important trajectories
  async computeFisher(
    model: ModelWeights,
    importantTrajectories: Trajectory[]
  ): Promise<FisherMatrix> {
    const { learning } = this.runtime.getModules();

    // Initialize Fisher matrix (diagonal approximation)
    const fisher: FisherMatrix = {
      diagonal: new Float32Array(model.parameterCount),
      parameterSnapshot: this.cloneWeights(model),
    };

    // Accumulate gradients squared
    for (const trajectory of importantTrajectories) {
      const gradients = await learning.computeGradients(model, trajectory);

      for (let i = 0; i < fisher.diagonal.length; i++) {
        fisher.diagonal[i] += gradients[i] * gradients[i];
      }
    }

    // Normalize
    for (let i = 0; i < fisher.diagonal.length; i++) {
      fisher.diagonal[i] /= importantTrajectories.length;
    }

    return fisher;
  }

  // Apply EWC constraint during training
  ewcLoss(
    currentWeights: ModelWeights,
    fisher: FisherMatrix,
    lambda: number
  ): number {
    const { learning } = this.runtime.getModules();

    let loss = 0;
    const currentParams = learning.flatten(currentWeights);
    const oldParams = learning.flatten(fisher.parameterSnapshot);

    for (let i = 0; i < currentParams.length; i++) {
      const diff = currentParams[i] - oldParams[i];
      loss += fisher.diagonal[i] * diff * diff;
    }

    return (lambda / 2) * loss;
  }

  // Consolidate learned patterns while protecting old ones
  async consolidate(
    oldWeights: ModelWeights,
    newWeights: ModelWeights,
    oldFisher: FisherMatrix,
    newTrajectories: Trajectory[]
  ): Promise<ConsolidationResult> {
    // Compute Fisher for new knowledge
    const newFisher = await this.computeFisher(newWeights, newTrajectories);

    // Merge Fisher matrices (EWC++)
    const mergedFisher = this.mergeFisher(oldFisher, newFisher);

    // Apply soft constraint to keep important old weights
    const consolidatedWeights = this.applyConstraint(
      newWeights,
      oldFisher,
      this.config.lambda
    );

    return {
      weights: consolidatedWeights,
      fisher: mergedFisher,
      oldKnowledgeRetention: this.measureRetention(oldWeights, consolidatedWeights, oldFisher),
      newKnowledgeGain: this.measureGain(oldWeights, consolidatedWeights, newFisher),
    };
  }

  private mergeFisher(old: FisherMatrix, new_: FisherMatrix): FisherMatrix {
    // Online EWC: running average of Fisher matrices
    const gamma = this.config.fisherDecay;

    const merged = new Float32Array(old.diagonal.length);
    for (let i = 0; i < merged.length; i++) {
      merged[i] = gamma * old.diagonal[i] + (1 - gamma) * new_.diagonal[i];
    }

    return {
      diagonal: merged,
      parameterSnapshot: new_.parameterSnapshot, // Use latest weights
    };
  }

  private applyConstraint(
    weights: ModelWeights,
    fisher: FisherMatrix,
    lambda: number
  ): ModelWeights {
    const { learning } = this.runtime.getModules();

    const currentParams = learning.flatten(weights);
    const oldParams = learning.flatten(fisher.parameterSnapshot);
    const constrainedParams = new Float32Array(currentParams.length);

    for (let i = 0; i < currentParams.length; i++) {
      // Pull back towards old weights proportional to importance
      const importance = fisher.diagonal[i];
      const pullback = lambda * importance * (oldParams[i] - currentParams[i]);
      constrainedParams[i] = currentParams[i] + pullback;
    }

    return learning.unflatten(constrainedParams, weights);
  }

  private measureRetention(
    oldWeights: ModelWeights,
    newWeights: ModelWeights,
    fisher: FisherMatrix
  ): number {
    const { learning } = this.runtime.getModules();

    const oldParams = learning.flatten(oldWeights);
    const newParams = learning.flatten(newWeights);

    let weightedDiff = 0;
    let totalImportance = 0;

    for (let i = 0; i < oldParams.length; i++) {
      const diff = Math.abs(oldParams[i] - newParams[i]);
      const importance = fisher.diagonal[i];
      weightedDiff += importance * diff;
      totalImportance += importance;
    }

    // Higher retention = lower weighted difference
    const avgDiff = weightedDiff / totalImportance;
    return Math.exp(-avgDiff); // 0-1 scale, 1 = perfect retention
  }

  private measureGain(
    oldWeights: ModelWeights,
    newWeights: ModelWeights,
    fisher: FisherMatrix
  ): number {
    // Similar to retention but for new knowledge areas
    const { learning } = this.runtime.getModules();

    const oldParams = learning.flatten(oldWeights);
    const newParams = learning.flatten(newWeights);

    let change = 0;
    let totalImportance = 0;

    for (let i = 0; i < oldParams.length; i++) {
      const diff = Math.abs(oldParams[i] - newParams[i]);
      const importance = fisher.diagonal[i];
      change += importance * diff;
      totalImportance += importance;
    }

    return change / totalImportance; // Higher = more new learning
  }

  private cloneWeights(weights: ModelWeights): ModelWeights {
    return JSON.parse(JSON.stringify(weights));
  }
}

interface EWCConfig {
  lambda: number;        // Constraint strength (1000-10000)
  fisherDecay: number;   // Running average decay (0.9-0.99)
  sampleCount: number;   // Trajectories for Fisher estimation
}

interface FisherMatrix {
  diagonal: Float32Array;      // Diagonal approximation
  parameterSnapshot: ModelWeights; // Weights at computation time
}

interface ConsolidationResult {
  weights: ModelWeights;
  fisher: FisherMatrix;
  oldKnowledgeRetention: number; // 0-1
  newKnowledgeGain: number;      // Relative change
}
```

---

## Learning Orchestrator

```typescript
// Coordinate the full learning pipeline
class LearningOrchestrator {
  constructor(
    private trajectoryRepo: TrajectoryRepository,
    private patternIndex: PatternIndex,
    private loraTrainer: LoRATrainer,
    private ewcConsolidator: EWCConsolidator,
    private modelStore: ModelStore,
    private config: LearningConfig
  ) {}

  // Main learning job (runs periodically)
  async runLearningCycle(tenantId: TenantId): Promise<LearningCycleResult> {
    // 1. Collect recent labeled trajectories
    const trajectories = await this.trajectoryRepo.findLabeled({
      tenantId,
      since: this.getLastLearningTime(tenantId),
      minConfidence: 0.7,
    });

    if (trajectories.length < this.config.minTrajectories) {
      return { skipped: true, reason: 'insufficient_data' };
    }

    // 2. Load current model
    const currentModel = await this.modelStore.loadLatest(tenantId);
    const currentFisher = await this.modelStore.loadFisher(tenantId);

    // 3. Train LoRA on positive trajectories
    const positiveTrajectories = trajectories.filter(t => t.verdict === 'positive');
    const loraWeights = await this.loraTrainer.train(positiveTrajectories, currentModel);

    // 4. Merge LoRA into base
    const updatedModel = await this.loraTrainer.merge(currentModel, loraWeights);

    // 5. Apply EWC consolidation
    const consolidationResult = await this.ewcConsolidator.consolidate(
      currentModel,
      updatedModel,
      currentFisher,
      positiveTrajectories
    );

    // 6. Extract new patterns
    const newPatterns = await this.extractPatterns(positiveTrajectories);
    for (const pattern of newPatterns) {
      await this.patternIndex.addPattern(pattern);
    }

    // 7. Save updated model
    await this.modelStore.save(tenantId, consolidationResult.weights);
    await this.modelStore.saveFisher(tenantId, consolidationResult.fisher);

    // 8. Update learning timestamp
    await this.setLastLearningTime(tenantId, new Date());

    return {
      skipped: false,
      trajectoriesProcessed: trajectories.length,
      patternsExtracted: newPatterns.length,
      retention: consolidationResult.oldKnowledgeRetention,
      gain: consolidationResult.newKnowledgeGain,
    };
  }

  private async extractPatterns(trajectories: Trajectory[]): Promise<LearnedPattern[]> {
    const patterns: LearnedPattern[] = [];

    // Cluster similar trajectories
    const clusters = await this.clusterTrajectories(trajectories);

    for (const cluster of clusters) {
      if (cluster.members.length < this.config.minClusterSize) continue;

      // Create pattern from cluster centroid
      const centroid = this.computeCentroid(cluster.members);
      const exemplars = this.selectExemplars(cluster.members, 3);

      patterns.push({
        id: crypto.randomUUID(),
        tenantId: cluster.members[0].tenantId,
        patternType: 'response',
        embedding: centroid,
        exemplarTrajectoryIds: exemplars.map(e => e.id),
        confidence: cluster.cohesion,
        usageCount: 0,
        successCount: 0,
        successRate: 0,
        isActive: true,
        createdAt: new Date(),
        lastUsedAt: new Date(),
      });
    }

    return patterns;
  }
}
```

---

## Consequences

### Benefits

1. **Continuous Improvement**: System gets better with usage
2. **Efficient Updates**: LoRA enables fast adaptation without full retraining
3. **Stability**: EWC prevents forgetting critical learned behaviors
4. **Pattern Reuse**: Common situations handled efficiently via cached patterns
5. **Transparency**: Verdicts and patterns are explainable

### Trade-offs

| Benefit | Trade-off |
|---------|-----------|
| Online learning | Memory overhead for trajectory storage |
| Pattern caching | Stale patterns if not invalidated |
| EWC stability | Slower adaptation to dramatic changes |
| Automatic verdicts | Potential mislabeling |

---

## Related Decisions

- **ADR-001**: Architecture Overview
- **ADR-004**: Background Workers (learning jobs)
- **ADR-006**: WASM Integration (training runtime)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-27 | RuVector Architecture Team | Initial version |
