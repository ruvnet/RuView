/**
 * Multi-Algorithm Learning Engine
 * Supports 9 RL algorithms for intelligent hooks optimization
 */

export type LearningAlgorithm =
  | 'q-learning'
  | 'sarsa'
  | 'double-q'
  | 'actor-critic'
  | 'ppo'
  | 'decision-transformer'
  | 'monte-carlo'
  | 'td-lambda'
  | 'dqn';

export type TaskType =
  | 'agent-routing'
  | 'error-avoidance'
  | 'confidence-scoring'
  | 'trajectory-learning'
  | 'context-ranking'
  | 'memory-recall';

export interface LearningConfig {
  algorithm: LearningAlgorithm;
  learningRate: number;
  discountFactor: number;
  epsilon: number;           // Exploration rate
  lambda?: number;           // For TD(λ)
  clipRange?: number;        // For PPO
  entropyCoef?: number;      // For Actor-Critic/PPO
  sequenceLength?: number;   // For Decision Transformer
}

export interface Experience {
  state: string;
  action: string;
  reward: number;
  nextState: string;
  done: boolean;
  timestamp?: number;
}

export interface LearningTrajectory {
  experiences: Experience[];
  totalReward: number;
  completed: boolean;
}

export interface AlgorithmStats {
  algorithm: LearningAlgorithm;
  updates: number;
  avgReward: number;
  convergenceScore: number;
  lastUpdate: number;
}

// Default configs for each task type
const TASK_ALGORITHM_MAP: Record<TaskType, LearningConfig> = {
  'agent-routing': {
    algorithm: 'double-q',
    learningRate: 0.1,
    discountFactor: 0.95,
    epsilon: 0.1,
  },
  'error-avoidance': {
    algorithm: 'sarsa',
    learningRate: 0.05,
    discountFactor: 0.99,
    epsilon: 0.05,
  },
  'confidence-scoring': {
    algorithm: 'actor-critic',
    learningRate: 0.01,
    discountFactor: 0.95,
    epsilon: 0.1,
    entropyCoef: 0.01,
  },
  'trajectory-learning': {
    algorithm: 'decision-transformer',
    learningRate: 0.001,
    discountFactor: 0.99,
    epsilon: 0,
    sequenceLength: 20,
  },
  'context-ranking': {
    algorithm: 'ppo',
    learningRate: 0.0003,
    discountFactor: 0.99,
    epsilon: 0.2,
    clipRange: 0.2,
    entropyCoef: 0.01,
  },
  'memory-recall': {
    algorithm: 'td-lambda',
    learningRate: 0.1,
    discountFactor: 0.9,
    epsilon: 0.1,
    lambda: 0.8,
  },
};

export class LearningEngine {
  private configs: Map<TaskType, LearningConfig> = new Map();
  private qTables: Map<string, Map<string, number>> = new Map();
  private qTables2: Map<string, Map<string, number>> = new Map(); // For Double-Q
  private eligibilityTraces: Map<string, Map<string, number>> = new Map();
  private actorWeights: Map<string, number[]> = new Map();
  private criticValues: Map<string, number> = new Map();
  private trajectories: LearningTrajectory[] = [];
  private stats: Map<LearningAlgorithm, AlgorithmStats> = new Map();
  private rewardHistory: number[] = [];

  constructor() {
    // Initialize with default configs
    for (const [task, config] of Object.entries(TASK_ALGORITHM_MAP)) {
      this.configs.set(task as TaskType, { ...config });
    }

    // Initialize stats for all algorithms
    const algorithms: LearningAlgorithm[] = [
      'q-learning', 'sarsa', 'double-q', 'actor-critic',
      'ppo', 'decision-transformer', 'monte-carlo', 'td-lambda', 'dqn'
    ];
    for (const alg of algorithms) {
      this.stats.set(alg, {
        algorithm: alg,
        updates: 0,
        avgReward: 0,
        convergenceScore: 0,
        lastUpdate: Date.now(),
      });
    }
  }

  /**
   * Configure algorithm for a specific task type
   */
  configure(task: TaskType, config: Partial<LearningConfig>): void {
    const existing = this.configs.get(task) || TASK_ALGORITHM_MAP[task];
    this.configs.set(task, { ...existing, ...config });
  }

  /**
   * Get current configuration for a task
   */
  getConfig(task: TaskType): LearningConfig {
    return this.configs.get(task) || TASK_ALGORITHM_MAP[task];
  }

  /**
   * Update Q-value using the appropriate algorithm
   */
  update(task: TaskType, experience: Experience): number {
    const config = this.getConfig(task);
    let delta = 0;

    switch (config.algorithm) {
      case 'q-learning':
        delta = this.qLearningUpdate(experience, config);
        break;
      case 'sarsa':
        delta = this.sarsaUpdate(experience, config);
        break;
      case 'double-q':
        delta = this.doubleQUpdate(experience, config);
        break;
      case 'actor-critic':
        delta = this.actorCriticUpdate(experience, config);
        break;
      case 'ppo':
        delta = this.ppoUpdate(experience, config);
        break;
      case 'td-lambda':
        delta = this.tdLambdaUpdate(experience, config);
        break;
      case 'monte-carlo':
        // Monte Carlo needs full episodes
        this.addToCurrentTrajectory(experience);
        if (experience.done) {
          delta = this.monteCarloUpdate(config);
        }
        break;
      case 'decision-transformer':
        this.addToCurrentTrajectory(experience);
        if (experience.done) {
          delta = this.decisionTransformerUpdate(config);
        }
        break;
      case 'dqn':
        delta = this.dqnUpdate(experience, config);
        break;
    }

    // Update stats
    this.updateStats(config.algorithm, experience.reward, Math.abs(delta));

    return delta;
  }

  /**
   * Get best action for a state
   */
  getBestAction(task: TaskType, state: string, actions: string[]): { action: string; confidence: number } {
    const config = this.getConfig(task);

    // Epsilon-greedy exploration
    if (Math.random() < config.epsilon) {
      const randomAction = actions[Math.floor(Math.random() * actions.length)];
      return { action: randomAction, confidence: 0.5 };
    }

    let bestAction = actions[0];
    let bestValue = -Infinity;
    let values: number[] = [];

    const qTable = this.getQTable(state);

    for (const action of actions) {
      const value = qTable.get(action) || 0;
      values.push(value);
      if (value > bestValue) {
        bestValue = value;
        bestAction = action;
      }
    }

    // Calculate confidence using softmax
    const confidence = this.softmaxConfidence(values, actions.indexOf(bestAction));

    return { action: bestAction, confidence };
  }

  /**
   * Get action probabilities (for Actor-Critic and PPO)
   */
  getActionProbabilities(state: string, actions: string[]): Map<string, number> {
    const probs = new Map<string, number>();
    const qTable = this.getQTable(state);

    const values = actions.map(a => qTable.get(a) || 0);
    const maxVal = Math.max(...values);
    const expValues = values.map(v => Math.exp(v - maxVal));
    const sumExp = expValues.reduce((a, b) => a + b, 0);

    for (let i = 0; i < actions.length; i++) {
      probs.set(actions[i], expValues[i] / sumExp);
    }

    return probs;
  }

  // ============ Algorithm Implementations ============

  /**
   * Standard Q-Learning: Q(s,a) += α * (r + γ * max_a' Q(s',a') - Q(s,a))
   */
  private qLearningUpdate(exp: Experience, config: LearningConfig): number {
    const { state, action, reward, nextState, done } = exp;
    const { learningRate: α, discountFactor: γ } = config;

    const qTable = this.getQTable(state);
    const nextQTable = this.getQTable(nextState);

    const currentQ = qTable.get(action) || 0;
    const maxNextQ = done ? 0 : Math.max(0, ...Array.from(nextQTable.values()));

    const target = reward + γ * maxNextQ;
    const delta = target - currentQ;
    const newQ = currentQ + α * delta;

    qTable.set(action, newQ);
    return delta;
  }

  /**
   * SARSA: On-policy, more conservative
   * Q(s,a) += α * (r + γ * Q(s',a') - Q(s,a))
   */
  private sarsaUpdate(exp: Experience, config: LearningConfig): number {
    const { state, action, reward, nextState, done } = exp;
    const { learningRate: α, discountFactor: γ, epsilon } = config;

    const qTable = this.getQTable(state);
    const nextQTable = this.getQTable(nextState);

    const currentQ = qTable.get(action) || 0;

    // On-policy: use expected value under current policy (ε-greedy)
    let nextQ = 0;
    if (!done) {
      const nextActions = Array.from(nextQTable.keys());
      if (nextActions.length > 0) {
        const maxQ = Math.max(...Array.from(nextQTable.values()));
        const avgQ = Array.from(nextQTable.values()).reduce((a, b) => a + b, 0) / nextActions.length;
        // Expected value under ε-greedy
        nextQ = (1 - epsilon) * maxQ + epsilon * avgQ;
      }
    }

    const target = reward + γ * nextQ;
    const delta = target - currentQ;
    const newQ = currentQ + α * delta;

    qTable.set(action, newQ);
    return delta;
  }

  /**
   * Double Q-Learning: Reduces overestimation bias
   * Uses two Q-tables, randomly updates one using the other for target
   */
  private doubleQUpdate(exp: Experience, config: LearningConfig): number {
    const { state, action, reward, nextState, done } = exp;
    const { learningRate: α, discountFactor: γ } = config;

    const useFirst = Math.random() < 0.5;
    const qTable = useFirst ? this.getQTable(state) : this.getQTable2(state);
    const otherQTable = useFirst ? this.getQTable2(nextState) : this.getQTable(nextState);
    const nextQTable = useFirst ? this.getQTable(nextState) : this.getQTable2(nextState);

    const currentQ = qTable.get(action) || 0;

    let nextQ = 0;
    if (!done) {
      // Find best action in next state using one table
      let bestAction = '';
      let bestValue = -Infinity;
      for (const [a, v] of nextQTable) {
        if (v > bestValue) {
          bestValue = v;
          bestAction = a;
        }
      }
      // Evaluate using other table
      if (bestAction) {
        nextQ = otherQTable.get(bestAction) || 0;
      }
    }

    const target = reward + γ * nextQ;
    const delta = target - currentQ;
    const newQ = currentQ + α * delta;

    qTable.set(action, newQ);
    return delta;
  }

  /**
   * Actor-Critic: Policy gradient with value baseline
   */
  private actorCriticUpdate(exp: Experience, config: LearningConfig): number {
    const { state, action, reward, nextState, done } = exp;
    const { learningRate: α, discountFactor: γ } = config;

    // Critic update (TD error)
    const V = this.criticValues.get(state) || 0;
    const V_next = done ? 0 : (this.criticValues.get(nextState) || 0);
    const tdError = reward + γ * V_next - V;
    this.criticValues.set(state, V + α * tdError);

    // Actor update (policy gradient)
    const qTable = this.getQTable(state);
    const currentQ = qTable.get(action) || 0;
    // Use TD error as advantage estimate
    const newQ = currentQ + α * tdError;
    qTable.set(action, newQ);

    return tdError;
  }

  /**
   * PPO: Clipped policy gradient for stable training
   */
  private ppoUpdate(exp: Experience, config: LearningConfig): number {
    const { state, action, reward, nextState, done } = exp;
    const { learningRate: α, discountFactor: γ, clipRange = 0.2 } = config;

    // Critic update
    const V = this.criticValues.get(state) || 0;
    const V_next = done ? 0 : (this.criticValues.get(nextState) || 0);
    const advantage = reward + γ * V_next - V;
    this.criticValues.set(state, V + α * advantage);

    // Actor update with clipping
    const qTable = this.getQTable(state);
    const oldQ = qTable.get(action) || 0;

    // Compute probability ratio (simplified)
    const ratio = Math.exp(α * advantage);
    const clippedRatio = Math.max(1 - clipRange, Math.min(1 + clipRange, ratio));

    // PPO objective: min(ratio * A, clip(ratio) * A)
    const update = Math.min(ratio * advantage, clippedRatio * advantage);
    const newQ = oldQ + α * update;

    qTable.set(action, newQ);
    return advantage;
  }

  /**
   * TD(λ): Temporal difference with eligibility traces
   */
  private tdLambdaUpdate(exp: Experience, config: LearningConfig): number {
    const { state, action, reward, nextState, done } = exp;
    const { learningRate: α, discountFactor: γ, lambda = 0.8 } = config;

    const qTable = this.getQTable(state);
    const nextQTable = this.getQTable(nextState);

    const currentQ = qTable.get(action) || 0;
    const maxNextQ = done ? 0 : Math.max(0, ...Array.from(nextQTable.values()));

    const tdError = reward + γ * maxNextQ - currentQ;

    // Update eligibility trace for current state-action
    const traces = this.getEligibilityTraces(state);
    traces.set(action, (traces.get(action) || 0) + 1);

    // Update all state-actions with eligibility traces
    for (const [s, sTraces] of this.eligibilityTraces) {
      const sQTable = this.getQTable(s);
      for (const [a, trace] of sTraces) {
        const q = sQTable.get(a) || 0;
        sQTable.set(a, q + α * tdError * trace);
        // Decay trace
        sTraces.set(a, γ * lambda * trace);
      }
    }

    return tdError;
  }

  /**
   * Monte Carlo: Full episode learning
   */
  private monteCarloUpdate(config: LearningConfig): number {
    const { learningRate: α, discountFactor: γ } = config;
    const trajectory = this.trajectories[this.trajectories.length - 1];
    if (!trajectory || trajectory.experiences.length === 0) return 0;

    let G = 0; // Return
    let totalDelta = 0;

    // Work backwards through episode
    for (let t = trajectory.experiences.length - 1; t >= 0; t--) {
      const exp = trajectory.experiences[t];
      G = exp.reward + γ * G;

      const qTable = this.getQTable(exp.state);
      const currentQ = qTable.get(exp.action) || 0;
      const delta = G - currentQ;
      qTable.set(exp.action, currentQ + α * delta);
      totalDelta += Math.abs(delta);
    }

    trajectory.completed = true;
    trajectory.totalReward = G;

    return totalDelta / trajectory.experiences.length;
  }

  /**
   * Decision Transformer: Sequence modeling for trajectories
   */
  private decisionTransformerUpdate(config: LearningConfig): number {
    const { learningRate: α, sequenceLength = 20 } = config;
    const trajectory = this.trajectories[this.trajectories.length - 1];
    if (!trajectory || trajectory.experiences.length === 0) return 0;

    // Decision Transformer learns to predict actions given (return, state, action) sequences
    // Here we use a simplified version that learns state-action patterns

    let totalDelta = 0;
    const experiences = trajectory.experiences.slice(-sequenceLength);

    // Calculate returns-to-go
    const returns: number[] = [];
    let R = 0;
    for (let i = experiences.length - 1; i >= 0; i--) {
      R += experiences[i].reward;
      returns.unshift(R);
    }

    // Update Q-values weighted by return-to-go
    for (let i = 0; i < experiences.length; i++) {
      const exp = experiences[i];
      const qTable = this.getQTable(exp.state);
      const currentQ = qTable.get(exp.action) || 0;

      // Weight by normalized return
      const normalizedReturn = returns[i] / (Math.abs(returns[0]) + 1);
      const target = currentQ + α * normalizedReturn * exp.reward;
      const delta = target - currentQ;

      qTable.set(exp.action, target);
      totalDelta += Math.abs(delta);
    }

    trajectory.completed = true;
    trajectory.totalReward = returns[0];

    return totalDelta / experiences.length;
  }

  /**
   * DQN: Deep Q-Network (simplified without actual neural network)
   * Uses experience replay and target network concepts
   */
  private dqnUpdate(exp: Experience, config: LearningConfig): number {
    // Add to replay buffer (trajectory)
    this.addToCurrentTrajectory(exp);

    // Sample from replay buffer
    const replayExp = this.sampleFromReplay();
    if (!replayExp) return this.qLearningUpdate(exp, config);

    // Use sampled experience for update (breaks correlation)
    return this.qLearningUpdate(replayExp, config);
  }

  // ============ Helper Methods ============

  private getQTable(state: string): Map<string, number> {
    if (!this.qTables.has(state)) {
      this.qTables.set(state, new Map());
    }
    return this.qTables.get(state)!;
  }

  private getQTable2(state: string): Map<string, number> {
    if (!this.qTables2.has(state)) {
      this.qTables2.set(state, new Map());
    }
    return this.qTables2.get(state)!;
  }

  private getEligibilityTraces(state: string): Map<string, number> {
    if (!this.eligibilityTraces.has(state)) {
      this.eligibilityTraces.set(state, new Map());
    }
    return this.eligibilityTraces.get(state)!;
  }

  private softmaxConfidence(values: number[], selectedIdx: number): number {
    if (values.length === 0) return 0.5;
    const maxVal = Math.max(...values);
    const expValues = values.map(v => Math.exp(v - maxVal));
    const sumExp = expValues.reduce((a, b) => a + b, 0);
    return expValues[selectedIdx] / sumExp;
  }

  private addToCurrentTrajectory(exp: Experience): void {
    if (this.trajectories.length === 0 || this.trajectories[this.trajectories.length - 1].completed) {
      this.trajectories.push({
        experiences: [],
        totalReward: 0,
        completed: false,
      });
    }
    this.trajectories[this.trajectories.length - 1].experiences.push(exp);
  }

  private sampleFromReplay(): Experience | null {
    const allExperiences: Experience[] = [];
    for (const traj of this.trajectories) {
      allExperiences.push(...traj.experiences);
    }
    if (allExperiences.length === 0) return null;
    return allExperiences[Math.floor(Math.random() * allExperiences.length)];
  }

  private updateStats(algorithm: LearningAlgorithm, reward: number, delta: number): void {
    const stats = this.stats.get(algorithm);
    if (!stats) return;

    stats.updates++;
    stats.lastUpdate = Date.now();

    // Running average reward
    this.rewardHistory.push(reward);
    if (this.rewardHistory.length > 1000) {
      this.rewardHistory.shift();
    }
    stats.avgReward = this.rewardHistory.reduce((a, b) => a + b, 0) / this.rewardHistory.length;

    // Convergence score (inverse of recent delta magnitude)
    stats.convergenceScore = 1 / (1 + delta);
  }

  /**
   * Get statistics for all algorithms
   */
  getStats(): Map<LearningAlgorithm, AlgorithmStats> {
    return new Map(this.stats);
  }

  /**
   * Get statistics summary
   */
  getStatsSummary(): {
    bestAlgorithm: LearningAlgorithm;
    totalUpdates: number;
    avgReward: number;
    algorithms: AlgorithmStats[];
  } {
    let bestAlgorithm: LearningAlgorithm = 'q-learning';
    let bestScore = -Infinity;
    let totalUpdates = 0;

    const algorithms: AlgorithmStats[] = [];

    for (const [alg, stats] of this.stats) {
      algorithms.push(stats);
      totalUpdates += stats.updates;

      const score = stats.avgReward * stats.convergenceScore;
      if (score > bestScore && stats.updates > 0) {
        bestScore = score;
        bestAlgorithm = alg;
      }
    }

    return {
      bestAlgorithm,
      totalUpdates,
      avgReward: this.rewardHistory.length > 0
        ? this.rewardHistory.reduce((a, b) => a + b, 0) / this.rewardHistory.length
        : 0,
      algorithms: algorithms.filter(a => a.updates > 0),
    };
  }

  /**
   * Export state for persistence
   */
  export(): {
    qTables: Record<string, Record<string, number>>;
    qTables2: Record<string, Record<string, number>>;
    criticValues: Record<string, number>;
    trajectories: LearningTrajectory[];
    stats: Record<string, AlgorithmStats>;
    configs: Record<string, LearningConfig>;
    rewardHistory: number[];
  } {
    const qTables: Record<string, Record<string, number>> = {};
    for (const [state, actions] of this.qTables) {
      qTables[state] = Object.fromEntries(actions);
    }

    const qTables2: Record<string, Record<string, number>> = {};
    for (const [state, actions] of this.qTables2) {
      qTables2[state] = Object.fromEntries(actions);
    }

    const criticValues = Object.fromEntries(this.criticValues);
    const stats: Record<string, AlgorithmStats> = {};
    for (const [alg, s] of this.stats) {
      stats[alg] = s;
    }

    const configs: Record<string, LearningConfig> = {};
    for (const [task, config] of this.configs) {
      configs[task] = config;
    }

    return {
      qTables,
      qTables2,
      criticValues,
      trajectories: this.trajectories.slice(-100), // Keep last 100 trajectories
      stats,
      configs,
      rewardHistory: this.rewardHistory.slice(-1000),
    };
  }

  /**
   * Import state from persistence
   */
  import(data: ReturnType<LearningEngine['export']>): void {
    // Q-tables
    this.qTables.clear();
    for (const [state, actions] of Object.entries(data.qTables || {})) {
      this.qTables.set(state, new Map(Object.entries(actions)));
    }

    this.qTables2.clear();
    for (const [state, actions] of Object.entries(data.qTables2 || {})) {
      this.qTables2.set(state, new Map(Object.entries(actions)));
    }

    // Critic values
    this.criticValues = new Map(Object.entries(data.criticValues || {}));

    // Trajectories
    this.trajectories = data.trajectories || [];

    // Stats
    for (const [alg, s] of Object.entries(data.stats || {})) {
      this.stats.set(alg as LearningAlgorithm, s as AlgorithmStats);
    }

    // Configs
    for (const [task, config] of Object.entries(data.configs || {})) {
      this.configs.set(task as TaskType, config as LearningConfig);
    }

    // Reward history
    this.rewardHistory = data.rewardHistory || [];
  }

  /**
   * Clear all learning data
   */
  clear(): void {
    this.qTables.clear();
    this.qTables2.clear();
    this.eligibilityTraces.clear();
    this.actorWeights.clear();
    this.criticValues.clear();
    this.trajectories = [];
    this.rewardHistory = [];

    // Reset stats
    for (const stats of this.stats.values()) {
      stats.updates = 0;
      stats.avgReward = 0;
      stats.convergenceScore = 0;
    }
  }

  /**
   * Get available algorithms
   */
  static getAlgorithms(): { algorithm: LearningAlgorithm; description: string; bestFor: string }[] {
    return [
      { algorithm: 'q-learning', description: 'Simple off-policy learning', bestFor: 'General routing' },
      { algorithm: 'sarsa', description: 'On-policy, conservative', bestFor: 'Error avoidance' },
      { algorithm: 'double-q', description: 'Reduces overestimation', bestFor: 'Precise routing' },
      { algorithm: 'actor-critic', description: 'Policy gradient + value', bestFor: 'Confidence scoring' },
      { algorithm: 'ppo', description: 'Stable policy updates', bestFor: 'Preference learning' },
      { algorithm: 'decision-transformer', description: 'Sequence modeling', bestFor: 'Trajectory patterns' },
      { algorithm: 'monte-carlo', description: 'Full episode learning', bestFor: 'Unbiased estimates' },
      { algorithm: 'td-lambda', description: 'Eligibility traces', bestFor: 'Credit assignment' },
      { algorithm: 'dqn', description: 'Experience replay', bestFor: 'High-dim states' },
    ];
  }
}

export default LearningEngine;
