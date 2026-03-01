#!/usr/bin/env node

/**
 * Adaptive Cognitive System
 *
 * A self-optimizing system that learns from performance metrics to automatically
 * select the best attention mechanism for each task.
 *
 * Features:
 * - Performance tracking for each attention mechanism
 * - Adaptive selection based on historical performance
 * - Learning rate adjustment
 * - Automatic optimization
 * - Performance prediction
 */

const { VectorDB } = require('ruvector');
const {
  MultiHeadAttention,
  HyperbolicAttention,
  FlashAttention,
  MoEAttention,
  LinearAttention
} = require('@ruvector/attention');

console.log('🧠 Adaptive Cognitive System\n');
console.log('=' .repeat(70));

class AdaptiveCognitiveSystem {
  constructor() {
    this.attentionMechanisms = new Map();
    this.performanceHistory = new Map();
    this.taskHistory = [];
    this.learningRate = 0.1;
    this.explorationRate = 0.2; // 20% exploration, 80% exploitation
    this.cache = new Map();
  }

  async initialize() {
    console.log('\n🔧 Initializing Adaptive System...\n');

    const dim = 64;

    // Initialize all attention mechanisms with performance tracking
    this.attentionMechanisms.set('multiHead', {
      instance: new MultiHeadAttention(dim, 8),
      expectedPerformance: 0.5, // Initial estimate in ms
      actualPerformance: [],
      successRate: 1.0,
      useCount: 0,
      taskTypes: []
    });

    this.attentionMechanisms.set('hyperbolic', {
      instance: new HyperbolicAttention(dim, -1.0),
      expectedPerformance: 0.8,
      actualPerformance: [],
      successRate: 1.0,
      useCount: 0,
      taskTypes: []
    });

    this.attentionMechanisms.set('flash', {
      instance: new FlashAttention(dim, 32),
      expectedPerformance: 0.2,
      actualPerformance: [],
      successRate: 1.0,
      useCount: 0,
      taskTypes: []
    });

    this.attentionMechanisms.set('moe', {
      instance: new MoEAttention({ dim, numExperts: 4, topK: 2, expertCapacity: 1.25 }),
      expectedPerformance: 0.3,
      actualPerformance: [],
      successRate: 1.0,
      useCount: 0,
      taskTypes: []
    });

    this.attentionMechanisms.set('linear', {
      instance: new LinearAttention(dim, 64),
      expectedPerformance: 0.4,
      actualPerformance: [],
      successRate: 1.0,
      useCount: 0,
      taskTypes: []
    });

    console.log('✅ Initialized 5 attention mechanisms');
    console.log(`   Learning Rate: ${this.learningRate}`);
    console.log(`   Exploration Rate: ${this.explorationRate * 100}%\n`);
  }

  // Select best attention mechanism using epsilon-greedy strategy
  selectAttention(taskType, taskComplexity = 'medium') {
    // Exploration: randomly try different mechanisms
    if (Math.random() < this.explorationRate) {
      const mechanisms = Array.from(this.attentionMechanisms.keys());
      const selected = mechanisms[Math.floor(Math.random() * mechanisms.length)];
      return {
        name: selected,
        reason: 'exploration',
        ...this.attentionMechanisms.get(selected)
      };
    }

    // Exploitation: use best performing mechanism
    const scores = new Map();

    for (const [name, mech] of this.attentionMechanisms.entries()) {
      // Score based on:
      // 1. Expected performance (lower is better)
      // 2. Success rate (higher is better)
      // 3. Experience with similar tasks

      const perfScore = 1.0 / (mech.expectedPerformance || 1.0);
      const successScore = mech.successRate;

      // Task-specific bonus
      const taskBonus = mech.taskTypes.filter(t => t === taskType).length * 0.1;

      const totalScore = perfScore * 0.4 + successScore * 0.4 + taskBonus * 0.2;
      scores.set(name, totalScore);
    }

    // Select highest scoring mechanism
    const bestMechanism = Array.from(scores.entries())
      .sort((a, b) => b[1] - a[1])[0];

    return {
      name: bestMechanism[0],
      reason: 'exploitation',
      score: bestMechanism[1],
      ...this.attentionMechanisms.get(bestMechanism[0])
    };
  }

  // Execute task with selected attention mechanism
  async executeTask(task) {
    const selected = this.selectAttention(task.type, task.complexity);

    console.log(`\n🎯 Task: ${task.name}`);
    console.log(`   Type: ${task.type}`);
    console.log(`   Selected: ${selected.name} (${selected.reason})`);

    if (selected.reason === 'exploitation') {
      console.log(`   Score: ${selected.score.toFixed(3)}`);
      console.log(`   Expected: ${selected.expectedPerformance.toFixed(3)}ms`);
    }

    const startTime = performance.now();

    try {
      // Execute task with selected mechanism
      const result = await task.execute(selected.instance);
      const endTime = performance.now();
      const duration = endTime - startTime;

      // Record performance
      this.recordPerformance(selected.name, task.type, duration, true);

      console.log(`   ✓ Completed in ${duration.toFixed(3)}ms`);
      console.log(`   ✓ Success!`);

      return {
        success: true,
        duration,
        mechanism: selected.name,
        result
      };
    } catch (error) {
      const endTime = performance.now();
      const duration = endTime - startTime;

      this.recordPerformance(selected.name, task.type, duration, false);

      console.log(`   ✗ Failed: ${error.message}`);

      return {
        success: false,
        duration,
        mechanism: selected.name,
        error: error.message
      };
    }
  }

  // Record performance and update expectations
  recordPerformance(mechanismName, taskType, duration, success) {
    const mech = this.attentionMechanisms.get(mechanismName);

    // Add to performance history
    mech.actualPerformance.push(duration);
    mech.taskTypes.push(taskType);
    mech.useCount++;

    // Update success rate with moving average
    const prevSuccessRate = mech.successRate;
    mech.successRate = prevSuccessRate + this.learningRate * (
      (success ? 1.0 : 0.0) - prevSuccessRate
    );

    // Update expected performance with moving average
    const prevExpectedPerf = mech.expectedPerformance;
    mech.expectedPerformance = prevExpectedPerf + this.learningRate * (
      duration - prevExpectedPerf
    );

    // Keep only recent history (last 100 samples)
    if (mech.actualPerformance.length > 100) {
      mech.actualPerformance.shift();
      mech.taskTypes.shift();
    }

    this.taskHistory.push({
      mechanism: mechanismName,
      taskType,
      duration,
      success,
      timestamp: Date.now()
    });
  }

  // Analyze learning progress
  analyzeLearning() {
    console.log('\n\n📈 LEARNING ANALYSIS\n');
    console.log('=' .repeat(70));

    for (const [name, mech] of this.attentionMechanisms.entries()) {
      if (mech.useCount === 0) continue;

      console.log(`\n${name.toUpperCase()}:`);
      console.log(`   Uses: ${mech.useCount}`);
      console.log(`   Expected Performance: ${mech.expectedPerformance.toFixed(3)}ms`);

      if (mech.actualPerformance.length > 0) {
        const actual = mech.actualPerformance;
        const avg = actual.reduce((a, b) => a + b, 0) / actual.length;
        const min = Math.min(...actual);
        const max = Math.max(...actual);

        console.log(`   Actual Performance:`);
        console.log(`     Average: ${avg.toFixed(3)}ms`);
        console.log(`     Min: ${min.toFixed(3)}ms`);
        console.log(`     Max: ${max.toFixed(3)}ms`);
        console.log(`   Success Rate: ${(mech.successRate * 100).toFixed(1)}%`);

        // Task type distribution
        const taskCounts = {};
        mech.taskTypes.forEach(t => taskCounts[t] = (taskCounts[t] || 0) + 1);
        console.log(`   Task Types: ${Object.keys(taskCounts).join(', ')}`);
      }
    }

    // Learning progress over time
    console.log('\n\n📊 LEARNING PROGRESS:\n');

    const recentHistory = this.taskHistory.slice(-20);
    if (recentHistory.length > 0) {
      const avgDuration = recentHistory.reduce((a, b) => a + b.duration, 0) / recentHistory.length;
      const successCount = recentHistory.filter(t => t.success).length;

      console.log(`   Recent 20 tasks:`);
      console.log(`     Average Duration: ${avgDuration.toFixed(3)}ms`);
      console.log(`     Success Rate: ${(successCount / recentHistory.length * 100).toFixed(1)}%`);

      // Most used mechanism
      const mechanismCounts = {};
      recentHistory.forEach(t => mechanismCounts[t.mechanism] = (mechanismCounts[t.mechanism] || 0) + 1);
      const mostUsed = Object.entries(mechanismCounts)
        .sort((a, b) => b[1] - a[1])[0];

      console.log(`     Most Used: ${mostUsed[0]} (${mostUsed[1]} times)`);
    }

    // Optimal mechanism by task type
    console.log('\n\n🎯 OPTIMAL MECHANISM BY TASK TYPE:\n');

    const taskTypePerformance = new Map();

    this.taskHistory.forEach(task => {
      if (!taskTypePerformance.has(task.taskType)) {
        taskTypePerformance.set(task.taskType, new Map());
      }

      const typeMap = taskTypePerformance.get(task.taskType);
      if (!typeMap.has(task.mechanism)) {
        typeMap.set(task.mechanism, []);
      }

      typeMap.get(task.mechanism).push(task.duration);
    });

    for (const [taskType, mechanisms] of taskTypePerformance.entries()) {
      const avgPerformances = Array.from(mechanisms.entries()).map(([mech, durations]) => ({
        mechanism: mech,
        avgDuration: durations.reduce((a, b) => a + b, 0) / durations.length,
        count: durations.length
      })).sort((a, b) => a.avgDuration - b.avgDuration);

      if (avgPerformances.length > 0) {
        const best = avgPerformances[0];
        console.log(`   ${taskType}:`);
        console.log(`     Best: ${best.mechanism} (${best.avgDuration.toFixed(3)}ms avg)`);
        console.log(`     Count: ${best.count} samples`);
      }
    }
  }

  // Predict performance for a task
  predictPerformance(taskType, mechanismName) {
    const mech = this.attentionMechanisms.get(mechanismName);

    // Filter performance history for this task type
    const relevantHistory = this.taskHistory.filter(
      t => t.taskType === taskType && t.mechanism === mechanismName
    );

    if (relevantHistory.length === 0) {
      return mech.expectedPerformance;
    }

    const avg = relevantHistory.reduce((a, b) => a + b.duration, 0) / relevantHistory.length;
    return avg;
  }

  // Adjust learning rate based on performance stability
  adjustLearningRate() {
    const recentHistory = this.taskHistory.slice(-50);

    if (recentHistory.length < 10) return;

    // Calculate variance in recent performance
    const durations = recentHistory.map(t => t.duration);
    const mean = durations.reduce((a, b) => a + b, 0) / durations.length;
    const variance = durations.reduce((a, b) => a + Math.pow(b - mean, 2), 0) / durations.length;
    const stdDev = Math.sqrt(variance);

    // If performance is stable (low variance), reduce learning rate
    // If performance is unstable (high variance), increase learning rate
    const normalizedVariance = stdDev / mean;

    const oldRate = this.learningRate;

    if (normalizedVariance < 0.1) {
      this.learningRate = Math.max(0.01, this.learningRate * 0.9); // Decrease
    } else if (normalizedVariance > 0.3) {
      this.learningRate = Math.min(0.5, this.learningRate * 1.1); // Increase
    }

    if (oldRate !== this.learningRate) {
      console.log(`\n🎚️  Learning rate adjusted: ${oldRate.toFixed(3)} → ${this.learningRate.toFixed(3)}`);
    }
  }

  // Generate optimization report
  generateReport() {
    console.log('\n\n' + '=' .repeat(70));
    console.log('\n📋 ADAPTIVE SYSTEM REPORT\n');
    console.log('=' .repeat(70));

    console.log('\n🎓 LEARNED INSIGHTS:\n');

    // Find most efficient mechanism overall
    const avgPerformances = Array.from(this.attentionMechanisms.entries())
      .filter(([_, mech]) => mech.useCount > 0)
      .map(([name, mech]) => ({
        name,
        avgPerf: mech.actualPerformance.reduce((a, b) => a + b, 0) / mech.actualPerformance.length,
        useCount: mech.useCount,
        successRate: mech.successRate
      }))
      .sort((a, b) => a.avgPerf - b.avgPerf);

    console.log('   Most Efficient Overall:');
    avgPerformances.slice(0, 3).forEach((m, i) => {
      console.log(`     ${i + 1}. ${m.name}: ${m.avgPerf.toFixed(3)}ms (${m.useCount} uses, ${(m.successRate * 100).toFixed(1)}% success)`);
    });

    console.log('\n💡 RECOMMENDATIONS:\n');

    console.log(`   1. Primary mechanism: ${avgPerformances[0].name}`);
    console.log(`   2. Exploration rate: ${(this.explorationRate * 100).toFixed(1)}%`);
    console.log(`   3. Learning rate: ${this.learningRate.toFixed(3)}`);
    console.log(`   4. Total experience: ${this.taskHistory.length} tasks`);

    // Suggest improvements
    const lowUseMechanisms = Array.from(this.attentionMechanisms.entries())
      .filter(([_, mech]) => mech.useCount < 5);

    if (lowUseMechanisms.length > 0) {
      console.log(`\n   ⚠️  Underutilized mechanisms:`);
      lowUseMechanisms.forEach(([name, mech]) => {
        console.log(`      - ${name} (only ${mech.useCount} uses)`);
      });
    }
  }
}

// Create diverse test tasks
function createTasks() {
  const dim = 64;

  return [
    {
      name: 'Relationship Analysis',
      type: 'comparison',
      complexity: 'medium',
      execute: async (attention) => {
        const query = new Float32Array(dim).fill(0.1);
        const keys = [new Float32Array(dim).fill(0.2), new Float32Array(dim).fill(0.3)];
        const values = keys;
        return attention.compute(query, keys, values);
      }
    },
    {
      name: 'Hierarchical Organization',
      type: 'hierarchy',
      complexity: 'high',
      execute: async (attention) => {
        const query = new Float32Array(dim).fill(0.15);
        const keys = Array(5).fill(null).map(() => new Float32Array(dim).fill(Math.random()));
        const values = keys;
        return attention.compute(query, keys, values);
      }
    },
    {
      name: 'Sequence Processing',
      type: 'sequence',
      complexity: 'high',
      execute: async (attention) => {
        const query = new Float32Array(dim).fill(0.2);
        const keys = Array(10).fill(null).map(() => new Float32Array(dim).fill(Math.random()));
        const values = keys;
        return attention.compute(query, keys, values);
      }
    },
    {
      name: 'Quick Pattern Match',
      type: 'pattern',
      complexity: 'low',
      execute: async (attention) => {
        const query = new Float32Array(dim).fill(0.3);
        const keys = [new Float32Array(dim).fill(0.4)];
        const values = keys;
        return attention.compute(query, keys, values);
      }
    },
    {
      name: 'Expert Routing',
      type: 'routing',
      complexity: 'medium',
      execute: async (attention) => {
        const query = new Float32Array(dim).fill(0.25);
        const keys = Array(4).fill(null).map(() => new Float32Array(dim).fill(Math.random()));
        const values = keys;
        return attention.compute(query, keys, values);
      }
    }
  ];
}

async function runAdaptiveSystem() {
  const system = new AdaptiveCognitiveSystem();
  await system.initialize();

  console.log('=' .repeat(70));
  console.log('\n🚀 Running Adaptive Learning Experiment\n');
  console.log('=' .repeat(70));

  const tasks = createTasks();

  // Phase 1: Initial exploration (20 iterations)
  console.log('\n\n📚 PHASE 1: Exploration Phase (20 iterations)\n');

  for (let i = 0; i < 20; i++) {
    const task = tasks[Math.floor(Math.random() * tasks.length)];
    await system.executeTask(task);

    if ((i + 1) % 5 === 0) {
      system.adjustLearningRate();
    }
  }

  system.analyzeLearning();

  // Phase 2: Exploitation phase (30 iterations)
  console.log('\n\n💪 PHASE 2: Exploitation Phase (30 iterations)\n');

  // Reduce exploration rate
  system.explorationRate = 0.1;
  console.log(`   Reduced exploration rate to ${system.explorationRate * 100}%\n`);

  for (let i = 0; i < 30; i++) {
    const task = tasks[Math.floor(Math.random() * tasks.length)];
    await system.executeTask(task);

    if ((i + 1) % 10 === 0) {
      system.adjustLearningRate();
    }
  }

  system.analyzeLearning();

  // Phase 3: Performance prediction
  console.log('\n\n🔮 PHASE 3: Performance Prediction\n');

  const predictions = new Map();

  for (const task of tasks) {
    console.log(`\n   ${task.name} (${task.type}):`);

    const mechanismPredictions = [];

    for (const [name, _] of system.attentionMechanisms.entries()) {
      const predicted = system.predictPerformance(task.type, name);
      mechanismPredictions.push({ name, predicted });
    }

    mechanismPredictions.sort((a, b) => a.predicted - b.predicted);

    console.log(`     Predicted fastest: ${mechanismPredictions[0].name} (${mechanismPredictions[0].predicted.toFixed(3)}ms)`);
    console.log(`     Predicted slowest: ${mechanismPredictions[mechanismPredictions.length - 1].name} (${mechanismPredictions[mechanismPredictions.length - 1].predicted.toFixed(3)}ms)`);
  }

  // Generate final report
  system.generateReport();

  console.log('\n' + '=' .repeat(70));
  console.log('\n✅ Adaptive System Complete!\n');
  console.log(`   System learned optimal attention selection from ${system.taskHistory.length} tasks`);
  console.log(`   Final learning rate: ${system.learningRate.toFixed(3)}`);
  console.log(`   Final exploration rate: ${(system.explorationRate * 100).toFixed(1)}%\n`);
}

runAdaptiveSystem().catch(error => {
  console.error('\n❌ Error:', error);
  console.error('\nStack:', error.stack);
  process.exit(1);
});
