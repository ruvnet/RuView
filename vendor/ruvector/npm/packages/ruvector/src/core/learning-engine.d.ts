/**
 * Multi-Algorithm Learning Engine
 * Supports 9 RL algorithms for intelligent hooks optimization
 */
export type LearningAlgorithm = 'q-learning' | 'sarsa' | 'double-q' | 'actor-critic' | 'ppo' | 'decision-transformer' | 'monte-carlo' | 'td-lambda' | 'dqn';
export type TaskType = 'agent-routing' | 'error-avoidance' | 'confidence-scoring' | 'trajectory-learning' | 'context-ranking' | 'memory-recall';
export interface LearningConfig {
    algorithm: LearningAlgorithm;
    learningRate: number;
    discountFactor: number;
    epsilon: number;
    lambda?: number;
    clipRange?: number;
    entropyCoef?: number;
    sequenceLength?: number;
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
export declare class LearningEngine {
    private configs;
    private qTables;
    private qTables2;
    private eligibilityTraces;
    private actorWeights;
    private criticValues;
    private trajectories;
    private stats;
    private rewardHistory;
    constructor();
    /**
     * Configure algorithm for a specific task type
     */
    configure(task: TaskType, config: Partial<LearningConfig>): void;
    /**
     * Get current configuration for a task
     */
    getConfig(task: TaskType): LearningConfig;
    /**
     * Update Q-value using the appropriate algorithm
     */
    update(task: TaskType, experience: Experience): number;
    /**
     * Get best action for a state
     */
    getBestAction(task: TaskType, state: string, actions: string[]): {
        action: string;
        confidence: number;
    };
    /**
     * Get action probabilities (for Actor-Critic and PPO)
     */
    getActionProbabilities(state: string, actions: string[]): Map<string, number>;
    /**
     * Standard Q-Learning: Q(s,a) += α * (r + γ * max_a' Q(s',a') - Q(s,a))
     */
    private qLearningUpdate;
    /**
     * SARSA: On-policy, more conservative
     * Q(s,a) += α * (r + γ * Q(s',a') - Q(s,a))
     */
    private sarsaUpdate;
    /**
     * Double Q-Learning: Reduces overestimation bias
     * Uses two Q-tables, randomly updates one using the other for target
     */
    private doubleQUpdate;
    /**
     * Actor-Critic: Policy gradient with value baseline
     */
    private actorCriticUpdate;
    /**
     * PPO: Clipped policy gradient for stable training
     */
    private ppoUpdate;
    /**
     * TD(λ): Temporal difference with eligibility traces
     */
    private tdLambdaUpdate;
    /**
     * Monte Carlo: Full episode learning
     */
    private monteCarloUpdate;
    /**
     * Decision Transformer: Sequence modeling for trajectories
     */
    private decisionTransformerUpdate;
    /**
     * DQN: Deep Q-Network (simplified without actual neural network)
     * Uses experience replay and target network concepts
     */
    private dqnUpdate;
    private getQTable;
    private getQTable2;
    private getEligibilityTraces;
    private softmaxConfidence;
    private addToCurrentTrajectory;
    private sampleFromReplay;
    private updateStats;
    /**
     * Get statistics for all algorithms
     */
    getStats(): Map<LearningAlgorithm, AlgorithmStats>;
    /**
     * Get statistics summary
     */
    getStatsSummary(): {
        bestAlgorithm: LearningAlgorithm;
        totalUpdates: number;
        avgReward: number;
        algorithms: AlgorithmStats[];
    };
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
    };
    /**
     * Import state from persistence
     */
    import(data: ReturnType<LearningEngine['export']>): void;
    /**
     * Clear all learning data
     */
    clear(): void;
    /**
     * Get available algorithms
     */
    static getAlgorithms(): {
        algorithm: LearningAlgorithm;
        description: string;
        bestFor: string;
    }[];
}
export default LearningEngine;
//# sourceMappingURL=learning-engine.d.ts.map