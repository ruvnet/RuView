"use strict";
/**
 * Multi-Algorithm Learning Engine
 * Supports 9 RL algorithms for intelligent hooks optimization
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.LearningEngine = void 0;
// Default configs for each task type
const TASK_ALGORITHM_MAP = {
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
class LearningEngine {
    constructor() {
        this.configs = new Map();
        this.qTables = new Map();
        this.qTables2 = new Map(); // For Double-Q
        this.eligibilityTraces = new Map();
        this.actorWeights = new Map();
        this.criticValues = new Map();
        this.trajectories = [];
        this.stats = new Map();
        this.rewardHistory = [];
        // Initialize with default configs
        for (const [task, config] of Object.entries(TASK_ALGORITHM_MAP)) {
            this.configs.set(task, { ...config });
        }
        // Initialize stats for all algorithms
        const algorithms = [
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
    configure(task, config) {
        const existing = this.configs.get(task) || TASK_ALGORITHM_MAP[task];
        this.configs.set(task, { ...existing, ...config });
    }
    /**
     * Get current configuration for a task
     */
    getConfig(task) {
        return this.configs.get(task) || TASK_ALGORITHM_MAP[task];
    }
    /**
     * Update Q-value using the appropriate algorithm
     */
    update(task, experience) {
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
    getBestAction(task, state, actions) {
        const config = this.getConfig(task);
        // Epsilon-greedy exploration
        if (Math.random() < config.epsilon) {
            const randomAction = actions[Math.floor(Math.random() * actions.length)];
            return { action: randomAction, confidence: 0.5 };
        }
        let bestAction = actions[0];
        let bestValue = -Infinity;
        let values = [];
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
    getActionProbabilities(state, actions) {
        const probs = new Map();
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
    qLearningUpdate(exp, config) {
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
    sarsaUpdate(exp, config) {
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
    doubleQUpdate(exp, config) {
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
    actorCriticUpdate(exp, config) {
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
    ppoUpdate(exp, config) {
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
    tdLambdaUpdate(exp, config) {
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
    monteCarloUpdate(config) {
        const { learningRate: α, discountFactor: γ } = config;
        const trajectory = this.trajectories[this.trajectories.length - 1];
        if (!trajectory || trajectory.experiences.length === 0)
            return 0;
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
    decisionTransformerUpdate(config) {
        const { learningRate: α, sequenceLength = 20 } = config;
        const trajectory = this.trajectories[this.trajectories.length - 1];
        if (!trajectory || trajectory.experiences.length === 0)
            return 0;
        // Decision Transformer learns to predict actions given (return, state, action) sequences
        // Here we use a simplified version that learns state-action patterns
        let totalDelta = 0;
        const experiences = trajectory.experiences.slice(-sequenceLength);
        // Calculate returns-to-go
        const returns = [];
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
    dqnUpdate(exp, config) {
        // Add to replay buffer (trajectory)
        this.addToCurrentTrajectory(exp);
        // Sample from replay buffer
        const replayExp = this.sampleFromReplay();
        if (!replayExp)
            return this.qLearningUpdate(exp, config);
        // Use sampled experience for update (breaks correlation)
        return this.qLearningUpdate(replayExp, config);
    }
    // ============ Helper Methods ============
    getQTable(state) {
        if (!this.qTables.has(state)) {
            this.qTables.set(state, new Map());
        }
        return this.qTables.get(state);
    }
    getQTable2(state) {
        if (!this.qTables2.has(state)) {
            this.qTables2.set(state, new Map());
        }
        return this.qTables2.get(state);
    }
    getEligibilityTraces(state) {
        if (!this.eligibilityTraces.has(state)) {
            this.eligibilityTraces.set(state, new Map());
        }
        return this.eligibilityTraces.get(state);
    }
    softmaxConfidence(values, selectedIdx) {
        if (values.length === 0)
            return 0.5;
        const maxVal = Math.max(...values);
        const expValues = values.map(v => Math.exp(v - maxVal));
        const sumExp = expValues.reduce((a, b) => a + b, 0);
        return expValues[selectedIdx] / sumExp;
    }
    addToCurrentTrajectory(exp) {
        if (this.trajectories.length === 0 || this.trajectories[this.trajectories.length - 1].completed) {
            this.trajectories.push({
                experiences: [],
                totalReward: 0,
                completed: false,
            });
        }
        this.trajectories[this.trajectories.length - 1].experiences.push(exp);
    }
    sampleFromReplay() {
        const allExperiences = [];
        for (const traj of this.trajectories) {
            allExperiences.push(...traj.experiences);
        }
        if (allExperiences.length === 0)
            return null;
        return allExperiences[Math.floor(Math.random() * allExperiences.length)];
    }
    updateStats(algorithm, reward, delta) {
        const stats = this.stats.get(algorithm);
        if (!stats)
            return;
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
    getStats() {
        return new Map(this.stats);
    }
    /**
     * Get statistics summary
     */
    getStatsSummary() {
        let bestAlgorithm = 'q-learning';
        let bestScore = -Infinity;
        let totalUpdates = 0;
        const algorithms = [];
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
    export() {
        const qTables = {};
        for (const [state, actions] of this.qTables) {
            qTables[state] = Object.fromEntries(actions);
        }
        const qTables2 = {};
        for (const [state, actions] of this.qTables2) {
            qTables2[state] = Object.fromEntries(actions);
        }
        const criticValues = Object.fromEntries(this.criticValues);
        const stats = {};
        for (const [alg, s] of this.stats) {
            stats[alg] = s;
        }
        const configs = {};
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
    import(data) {
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
            this.stats.set(alg, s);
        }
        // Configs
        for (const [task, config] of Object.entries(data.configs || {})) {
            this.configs.set(task, config);
        }
        // Reward history
        this.rewardHistory = data.rewardHistory || [];
    }
    /**
     * Clear all learning data
     */
    clear() {
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
    static getAlgorithms() {
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
exports.LearningEngine = LearningEngine;
exports.default = LearningEngine;
//# sourceMappingURL=learning-engine.js.map