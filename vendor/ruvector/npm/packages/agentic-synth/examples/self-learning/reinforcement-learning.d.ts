/**
 * Reinforcement Learning Training Data Generation
 *
 * This example demonstrates generating synthetic RL training data including:
 * - State-action-reward tuples
 * - Episode generation with temporal consistency
 * - Exploration vs exploitation scenarios
 * - Reward function testing
 */
import type { GenerationResult } from '../../src/types.js';
/**
 * Generate basic SAR tuples for Q-learning
 */
export declare function generateSARTuples(): Promise<GenerationResult<unknown>>;
/**
 * Generate complete RL episodes with consistent state transitions
 */
export declare function generateEpisodes(): Promise<GenerationResult<unknown>>;
/**
 * Generate data for testing exploration-exploitation trade-offs
 */
export declare function generateExplorationData(): Promise<GenerationResult<unknown>>;
/**
 * Generate data for testing and debugging reward functions
 */
export declare function generateRewardTestingData(): Promise<GenerationResult<unknown>>;
/**
 * Generate training data for policy gradient methods
 */
export declare function generatePolicyGradientData(): Promise<GenerationResult<unknown>>;
/**
 * Generate data for multi-agent reinforcement learning
 */
export declare function generateMultiAgentData(): Promise<GenerationResult<unknown>>;
/**
 * Example of using generated data in a training loop
 */
export declare function trainingLoopIntegration(): Promise<void>;
export declare function runAllRLExamples(): Promise<void>;
//# sourceMappingURL=reinforcement-learning.d.ts.map