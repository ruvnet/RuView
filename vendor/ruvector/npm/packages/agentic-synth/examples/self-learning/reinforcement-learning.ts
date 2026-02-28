/**
 * Reinforcement Learning Training Data Generation
 *
 * This example demonstrates generating synthetic RL training data including:
 * - State-action-reward tuples
 * - Episode generation with temporal consistency
 * - Exploration vs exploitation scenarios
 * - Reward function testing
 */

import { AgenticSynth, createSynth } from '../../src/index.js';
import type { GenerationResult } from '../../src/types.js';

// ============================================================================
// Example 1: State-Action-Reward Tuples (SAR)
// ============================================================================

/**
 * Generate basic SAR tuples for Q-learning
 */
export async function generateSARTuples() {
  console.log('\n🎮 Example 1: State-Action-Reward Tuples\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
  });

  // Generate SAR tuples for a grid-world environment
  const sarData = await synth.generateStructured({
    count: 1000,
    schema: {
      // State representation
      state: {
        x: 'number (0-10)',
        y: 'number (0-10)',
        has_key: 'boolean',
        health: 'number (0-100)',
      },
      // Action taken
      action: 'up | down | left | right | pickup | use',
      // Immediate reward
      reward: 'number (-10 to 100)',
      // Next state
      next_state: {
        x: 'number (0-10, related to action)',
        y: 'number (0-10, related to action)',
        has_key: 'boolean',
        health: 'number (0-100)',
      },
      // Terminal state flag
      done: 'boolean (true if health <= 0 or goal reached)',
      // Additional metadata
      metadata: {
        step: 'number (0-200)',
        episode_id: 'UUID',
        timestamp: 'ISO timestamp',
      },
    },
    constraints: [
      'Movement actions should change x or y coordinates appropriately',
      'Reward should be positive for goal states, negative for collisions',
      'Health should decrease on collision, increase on health pickup',
      'done should be true when health <= 0 or goal reached',
      'Ensure temporal consistency within episodes',
    ],
  });

  console.log('SAR Tuples Generated:');
  console.log(`- Total transitions: ${sarData.data.length}`);
  console.log(`- Sample transition:`, sarData.data[0]);
  console.log(`- Average reward: ${calculateAverage(sarData.data, 'reward')}`);
  console.log(`- Terminal states: ${sarData.data.filter((d: any) => d.done).length}`);

  return sarData;
}

// ============================================================================
// Example 2: Complete Episodes with Temporal Consistency
// ============================================================================

/**
 * Generate complete RL episodes with consistent state transitions
 */
export async function generateEpisodes() {
  console.log('\n📚 Example 2: Complete Episodes\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate multiple episodes
  const episodes = await synth.generateStructured({
    count: 50, // 50 episodes
    schema: {
      episode_id: 'UUID',
      agent_type: 'dqn | ppo | a3c | sac',
      environment: 'cartpole | mountain_car | lunar_lander',

      // Episode trajectory
      trajectory: [
        {
          step: 'number (sequential)',
          state: 'array of 4-8 numbers (state vector)',
          action: 'number (0-3, discrete action space)',
          reward: 'number (-1 to 1)',
          next_state: 'array of 4-8 numbers',
          done: 'boolean',
        },
      ],

      // Episode statistics
      total_reward: 'number (sum of all rewards)',
      steps: 'number (10-500, episode length)',
      success: 'boolean',

      // Metadata
      timestamp: 'ISO timestamp',
      hyperparameters: {
        learning_rate: 'number (0.0001-0.01)',
        discount_factor: 'number (0.9-0.99)',
        epsilon: 'number (0.01-1.0, exploration rate)',
      },
    },
    constraints: [
      'trajectory array should have length equal to steps',
      'steps should be sequential from 0 to steps-1',
      'total_reward should equal sum of trajectory rewards',
      'last transition should have done=true',
      'state vectors should have consistent dimensions',
      'successful episodes should have positive total_reward',
    ],
  });

  console.log('Episodes Generated:');
  console.log(`- Total episodes: ${episodes.data.length}`);
  console.log(`- Average episode length: ${calculateAverage(episodes.data, 'steps')}`);
  console.log(`- Success rate: ${calculateSuccessRate(episodes.data)}%`);
  console.log(`- Sample episode:`, {
    id: episodes.data[0].episode_id,
    steps: episodes.data[0].steps,
    reward: episodes.data[0].total_reward,
    success: episodes.data[0].success,
  });

  return episodes;
}

// ============================================================================
// Example 3: Exploration vs Exploitation Scenarios
// ============================================================================

/**
 * Generate data for testing exploration-exploitation trade-offs
 */
export async function generateExplorationData() {
  console.log('\n🔍 Example 3: Exploration vs Exploitation\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate multi-armed bandit scenarios
  const banditData = await synth.generateStructured({
    count: 500,
    schema: {
      // Bandit configuration
      bandit_id: 'UUID',
      num_arms: 'number (5-10)',

      // True reward distributions (hidden from agent)
      true_means: 'array of num_arms numbers (0-1)',
      true_stddevs: 'array of num_arms numbers (0.05-0.2)',

      // Agent action
      action_selected: 'number (0 to num_arms-1)',
      strategy: 'epsilon_greedy | ucb | thompson_sampling | softmax',

      // Strategy parameters
      strategy_params: {
        epsilon: 'number (0-1) if epsilon_greedy',
        temperature: 'number (0.1-2.0) if softmax',
        confidence: 'number (0.5-2.0) if ucb',
      },

      // Observed reward
      observed_reward: 'number (sample from true distribution)',

      // Agent knowledge
      q_values: 'array of num_arms numbers (estimated values)',
      action_counts: 'array of num_arms numbers (times each arm pulled)',

      // Metadata
      step: 'number (0-10000)',
      cumulative_regret: 'number (0-100)',
      timestamp: 'ISO timestamp',
    },
    constraints: [
      'Arrays should have length equal to num_arms',
      'action_selected should be valid index (0 to num_arms-1)',
      'observed_reward should be sampled from true_means[action_selected] distribution',
      'cumulative_regret should increase over time',
      'strategy_params should match strategy type',
    ],
  });

  console.log('Exploration Data Generated:');
  console.log(`- Total samples: ${banditData.data.length}`);
  console.log(`- Strategy distribution:`, getStrategyDistribution(banditData.data));
  console.log(`- Average regret: ${calculateAverage(banditData.data, 'cumulative_regret')}`);

  return banditData;
}

// ============================================================================
// Example 4: Reward Function Testing Data
// ============================================================================

/**
 * Generate data for testing and debugging reward functions
 */
export async function generateRewardTestingData() {
  console.log('\n🎯 Example 4: Reward Function Testing\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  // Generate edge cases and scenarios for reward testing
  const rewardTests = await synth.generateStructured({
    count: 200,
    schema: {
      test_id: 'UUID',
      test_category: 'edge_case | normal | boundary | adversarial',

      // State configuration
      state: {
        position: 'array of 2-3 numbers (coordinates)',
        velocity: 'array of 2-3 numbers',
        goal_position: 'array of 2-3 numbers',
        obstacles: ['array of obstacle positions'],
      },

      // Expected reward components
      expected_reward: {
        distance_reward: 'number (-10 to 0)',
        velocity_penalty: 'number (-5 to 0)',
        collision_penalty: 'number (-100 to 0)',
        goal_bonus: 'number (0 to 100)',
        time_penalty: 'number (-1 to 0)',
        total: 'number (sum of components)',
      },

      // Test metadata
      description: 'string (what this test case validates)',
      expected_behavior: 'string (expected agent behavior)',
      tags: ['array of test tags (edge_case, collision, goal_reached, etc.)'],

      // Validation
      passes_validation: 'boolean',
      validation_notes: 'string or null',
    },
    constraints: [
      'edge_case tests should have extreme values',
      'boundary tests should be at limits of valid ranges',
      'collision_penalty should be large negative for nearby obstacles',
      'goal_bonus should be positive only when close to goal',
      'expected_reward.total should equal sum of components',
    ],
  });

  console.log('Reward Testing Data Generated:');
  console.log(`- Total test cases: ${rewardTests.data.length}`);
  console.log(`- Test categories:`, getTestCategories(rewardTests.data));
  console.log(`- Passing tests: ${rewardTests.data.filter((d: any) => d.passes_validation).length}`);

  return rewardTests;
}

// ============================================================================
// Example 5: Policy Gradient Training Data
// ============================================================================

/**
 * Generate training data for policy gradient methods
 */
export async function generatePolicyGradientData() {
  console.log('\n📈 Example 5: Policy Gradient Training Data\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  const policyData = await synth.generateStructured({
    count: 100,
    schema: {
      episode_id: 'UUID',

      // Episode trajectory
      states: ['array of state vectors (each 4-10 numbers)'],
      actions: ['array of actions taken'],
      log_probs: ['array of log probabilities of actions'],
      rewards: ['array of rewards'],
      values: ['array of value estimates (if actor-critic)'],

      // Computed returns and advantages
      returns: ['array of discounted returns'],
      advantages: ['array of advantage estimates (if using baseline)'],

      // Episode metrics
      episode_length: 'number (length of arrays)',
      total_return: 'number (sum of rewards)',

      // Training metadata
      policy_entropy: 'number (0-2, entropy of action distribution)',
      value_loss: 'number (if actor-critic)',
      policy_loss: 'number',

      // Hyperparameters
      gamma: 'number (0.95-0.99, discount factor)',
      lambda_gae: 'number (0.9-0.99, GAE lambda if used)',
    },
    constraints: [
      'All arrays should have same length (episode_length)',
      'returns should be computed using gamma discount',
      'advantages should use GAE if lambda_gae provided',
      'policy_entropy should decrease during training',
      'Higher returns should correlate with lower policy_loss',
    ],
  });

  console.log('Policy Gradient Data Generated:');
  console.log(`- Episodes: ${policyData.data.length}`);
  console.log(`- Average return: ${calculateAverage(policyData.data, 'total_return')}`);
  console.log(`- Average entropy: ${calculateAverage(policyData.data, 'policy_entropy')}`);

  return policyData;
}

// ============================================================================
// Example 6: Multi-Agent RL Data
// ============================================================================

/**
 * Generate data for multi-agent reinforcement learning
 */
export async function generateMultiAgentData() {
  console.log('\n👥 Example 6: Multi-Agent RL Data\n');

  const synth = createSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
  });

  const multiAgentData = await synth.generateStructured({
    count: 50,
    schema: {
      episode_id: 'UUID',
      scenario: 'cooperative | competitive | mixed',
      num_agents: 'number (2-6)',

      // Joint trajectory
      joint_states: [
        {
          step: 'number',
          // Per-agent observations
          observations: ['array of per-agent state vectors'],
          // Joint action
          joint_action: ['array of actions (one per agent)'],
          // Per-agent rewards
          rewards: ['array of rewards (one per agent)'],
          // Global state (if available)
          global_state: 'array of numbers or null',
        },
      ],

      // Episode outcomes
      agent_returns: ['array of cumulative returns per agent'],
      winner: 'number (agent index) or null if cooperative',
      cooperation_score: 'number (0-1, for cooperative scenarios)',

      // Training info
      communication_enabled: 'boolean',
      shared_reward: 'boolean',

      timestamp: 'ISO timestamp',
    },
    constraints: [
      'observations, joint_action, and rewards should have length num_agents',
      'agent_returns should sum to positive for cooperative scenarios',
      'winner should be agent with highest return in competitive scenarios',
      'cooperation_score should be high for successful cooperative episodes',
    ],
  });

  console.log('Multi-Agent Data Generated:');
  console.log(`- Episodes: ${multiAgentData.data.length}`);
  console.log(`- Scenario distribution:`, getScenarioDistribution(multiAgentData.data));
  console.log(`- Average cooperation score: ${calculateAverage(
    multiAgentData.data.filter((d: any) => d.scenario === 'cooperative'),
    'cooperation_score'
  )}`);

  return multiAgentData;
}

// ============================================================================
// Utility Functions
// ============================================================================

function calculateAverage(data: any[], field: string): number {
  const values = data.map((d) => d[field]).filter((v) => typeof v === 'number');
  return values.reduce((a, b) => a + b, 0) / values.length;
}

function calculateSuccessRate(episodes: any[]): number {
  const successful = episodes.filter((e) => e.success).length;
  return (successful / episodes.length) * 100;
}

function getStrategyDistribution(data: any[]): Record<string, number> {
  const dist: Record<string, number> = {};
  data.forEach((d) => {
    dist[d.strategy] = (dist[d.strategy] || 0) + 1;
  });
  return dist;
}

function getTestCategories(data: any[]): Record<string, number> {
  const categories: Record<string, number> = {};
  data.forEach((d) => {
    categories[d.test_category] = (categories[d.test_category] || 0) + 1;
  });
  return categories;
}

function getScenarioDistribution(data: any[]): Record<string, number> {
  const scenarios: Record<string, number> = {};
  data.forEach((d) => {
    scenarios[d.scenario] = (scenarios[d.scenario] || 0) + 1;
  });
  return scenarios;
}

// ============================================================================
// Integration Example: Training Loop with Generated Data
// ============================================================================

/**
 * Example of using generated data in a training loop
 */
export async function trainingLoopIntegration() {
  console.log('\n🔄 Training Loop Integration Example\n');

  // Generate initial training batch
  const trainingBatch = await generateSARTuples();

  console.log('Simulating training loop with generated data...\n');

  // Simulate training epochs
  for (let epoch = 0; epoch < 3; epoch++) {
    console.log(`Epoch ${epoch + 1}:`);

    // In real training, you would:
    // 1. Sample batch from trainingBatch.data
    // 2. Compute loss and gradients
    // 3. Update model parameters
    // 4. Log metrics

    const sampleSize = 32;
    const batchSamples = trainingBatch.data.slice(0, sampleSize);

    // Simulate metrics
    const avgReward = calculateAverage(batchSamples, 'reward');
    console.log(`  - Batch size: ${sampleSize}`);
    console.log(`  - Average reward: ${avgReward.toFixed(2)}`);
    console.log(`  - Loss: ${(Math.random() * 0.5 + 0.1).toFixed(4)}`);
    console.log();
  }

  console.log('✅ Training loop integration complete');
}

// ============================================================================
// Run All Examples
// ============================================================================

export async function runAllRLExamples() {
  console.log('🚀 Reinforcement Learning Data Generation Examples\n');
  console.log('='.repeat(60));

  try {
    await generateSARTuples();
    console.log('='.repeat(60));

    await generateEpisodes();
    console.log('='.repeat(60));

    await generateExplorationData();
    console.log('='.repeat(60));

    await generateRewardTestingData();
    console.log('='.repeat(60));

    await generatePolicyGradientData();
    console.log('='.repeat(60));

    await generateMultiAgentData();
    console.log('='.repeat(60));

    await trainingLoopIntegration();
    console.log('='.repeat(60));

    console.log('\n✅ All RL examples completed!\n');
  } catch (error: any) {
    console.error('❌ Error:', error.message);
  }
}

// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  runAllRLExamples().catch(console.error);
}
