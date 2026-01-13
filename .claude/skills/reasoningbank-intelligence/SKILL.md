---
name: "ReasoningBank Intelligence"
description: "Implement adaptive learning with ReasoningBank for pattern recognition, strategy optimization, and continuous improvement. Use when building self-learning agents, optimizing workflows, or implementing meta-cognitive systems."
---

# ReasoningBank Intelligence

## What This Skill Does

Implements ReasoningBank's adaptive learning system for AI agents to learn from experience, recognize patterns, and optimize strategies over time. Enables meta-cognitive capabilities and continuous improvement.

## Prerequisites

- agentic-flow v1.5.11+
- AgentDB v1.0.4+ (for persistence)
- Node.js 18+

## Quick Start

```typescript
import { ReasoningBank } from 'agentic-flow/reasoningbank';

// Initialize ReasoningBank
const rb = new ReasoningBank({
  persist: true,
  learningRate: 0.1,
  adapter: 'agentdb' // Use AgentDB for storage
});

// Record task outcome
await rb.recordExperience({
  task: 'code_review',
  approach: 'static_analysis_first',
  outcome: {
    success: true,
    metrics: {
      bugs_found: 5,
      time_taken: 120,
      false_positives: 1
    }
  },
  context: {
    language: 'typescript',
    complexity: 'medium'
  }
});

// Get optimal strategy
const strategy = await rb.recommendStrategy('code_review', {
  language: 'typescript',
  complexity: 'high'
});
```

## Core Features

### 1. Pattern Recognition
```typescript
// Learn patterns from data
await rb.learnPattern({
  pattern: 'api_errors_increase_after_deploy',
  triggers: ['deployment', 'traffic_spike'],
  actions: ['rollback', 'scale_up'],
  confidence: 0.85
});

// Match patterns
const matches = await rb.matchPatterns(currentSituation);
```

### 2. Strategy Optimization
```typescript
// Compare strategies
const comparison = await rb.compareStrategies('bug_fixing', [
  'tdd_approach',
  'debug_first',
  'reproduce_then_fix'
]);

// Get best strategy
const best = comparison.strategies[0];
console.log(`Best: ${best.name} (score: ${best.score})`);
```

### 3. Continuous Learning
```typescript
// Enable auto-learning from all tasks
await rb.enableAutoLearning({
  threshold: 0.7,        // Only learn from high-confidence outcomes
  updateFrequency: 100   // Update models every 100 experiences
});
```

## Advanced Usage

### Meta-Learning
```typescript
// Learn about learning
await rb.metaLearn({
  observation: 'parallel_execution_faster_for_independent_tasks',
  confidence: 0.95,
  applicability: {
    task_types: ['batch_processing', 'data_transformation'],
    conditions: ['tasks_independent', 'io_bound']
  }
});
```

### Transfer Learning
```typescript
// Apply knowledge from one domain to another
await rb.transferKnowledge({
  from: 'code_review_javascript',
  to: 'code_review_typescript',
  similarity: 0.8
});
```

### Adaptive Agents
```typescript
// Create self-improving agent
class AdaptiveAgent {
  async execute(task: Task) {
    // Get optimal strategy
    const strategy = await rb.recommendStrategy(task.type, task.context);

    // Execute with strategy
    const result = await this.executeWithStrategy(task, strategy);

    // Learn from outcome
    await rb.recordExperience({
      task: task.type,
      approach: strategy.name,
      outcome: result,
      context: task.context
    });

    return result;
  }
}
```

## Integration with AgentDB

```typescript
// Persist ReasoningBank data
await rb.configure({
  storage: {
    type: 'agentdb',
    options: {
      database: './reasoning-bank.db',
      enableVectorSearch: true
    }
  }
});

// Query learned patterns
const patterns = await rb.query({
  category: 'optimization',
  minConfidence: 0.8,
  timeRange: { last: '30d' }
});
```

## Performance Metrics

```typescript
// Track learning effectiveness
const metrics = await rb.getMetrics();
console.log(`
  Total Experiences: ${metrics.totalExperiences}
  Patterns Learned: ${metrics.patternsLearned}
  Strategy Success Rate: ${metrics.strategySuccessRate}
  Improvement Over Time: ${metrics.improvement}
`);
```

## Best Practices

1. **Record consistently**: Log all task outcomes, not just successes
2. **Provide context**: Rich context improves pattern matching
3. **Set thresholds**: Filter low-confidence learnings
4. **Review periodically**: Audit learned patterns for quality
5. **Use vector search**: Enable semantic pattern matching

## Troubleshooting

### Issue: Poor recommendations
**Solution**: Ensure sufficient training data (100+ experiences per task type)

### Issue: Slow pattern matching
**Solution**: Enable vector indexing in AgentDB

### Issue: Memory growing large
**Solution**: Set TTL for old experiences or enable pruning

## Learn More

- ReasoningBank Guide: agentic-flow/src/reasoningbank/README.md
- AgentDB Integration: packages/agentdb/docs/reasoningbank.md
- Pattern Learning: docs/reasoning/patterns.md
