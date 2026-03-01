# DSPy.ts Learning Session

Production-ready DSPy integration framework for multi-model AI training with automatic prompt optimization, cross-model learning, and comprehensive benchmarking.

## Overview

The DSPy Learning Session provides a powerful orchestration framework for training multiple AI models concurrently, optimizing prompts automatically, and comparing performance across different model providers.

### Key Features

- **🚀 Concurrent Multi-Model Training**: Train 4+ models in parallel (Claude, GPT-4, Llama, Gemini)
- **🧠 DSPy-Powered Optimization**: Automatic prompt optimization using DSPy signatures
- **📊 Real-time Metrics**: Track quality, latency, cost, and convergence in real-time
- **🔄 Cross-Model Learning**: Share successful patterns across different models
- **💰 Cost Tracking**: Monitor and control costs with budget limits
- **⚡ Convergence Detection**: Automatically detect when models reach optimal performance
- **🔗 Hooks Integration**: Seamless integration with Claude Flow swarm coordination
- **📈 Comprehensive Benchmarking**: Generate detailed reports with comparative analysis

## Architecture

### Core Components

#### 1. DSPyTrainingSession
Main orchestrator that manages the entire training pipeline.

```typescript
const session = new DSPyTrainingSession({
  models: [/* model configs */],
  optimizationRounds: 5,
  convergenceThreshold: 0.95,
  maxConcurrency: 4,
  enableCrossLearning: true,
  enableHooksIntegration: true,
  costBudget: 10.0
});
```

#### 2. ModelTrainingAgent
Abstract base class for model-specific agents.

- `ClaudeSonnetAgent`: Claude Sonnet 4 training
- `GPT4Agent`: GPT-4 Turbo training
- `LlamaAgent`: Llama 3.1 training
- `GeminiAgent`: Gemini 2.0 Flash training

#### 3. OptimizationEngine
DSPy-powered prompt optimization engine.

```typescript
const optimizer = new OptimizationEngine();
const signature = optimizer.createSignature(
  'task-name',
  'input description',
  'output description',
  {
    examples: [/* few-shot examples */],
    constraints: [/* validation rules */],
    objectives: [/* optimization goals */]
  }
);
```

#### 4. BenchmarkCollector
Metrics collection and analysis.

```typescript
const collector = new BenchmarkCollector();
collector.addResult(result);
const comparison = collector.getComparison();
const bestModel = collector.getBestModel();
```

## Training Pipeline

### Phase 1: Baseline Generation
All models generate initial outputs to establish baseline performance.

- Runs 3 iterations per model (configurable)
- Collects quality and performance metrics
- No optimization applied

### Phase 2: DSPy Optimization
Prompts are optimized based on previous results.

- 5 rounds of optimization per model (configurable)
- DSPy signatures guide optimization
- Continuous quality improvement
- Convergence detection

### Phase 3: Cross-Model Learning
Best patterns are shared across models.

- Identify best-performing model
- Extract successful patterns
- Apply to other models
- Boost overall performance

### Phase 4: Final Benchmark
Comprehensive performance comparison.

- 50-100 samples per model (configurable)
- Statistical analysis
- Cost-per-quality metrics
- Latency profiling

### Phase 5: Report Generation
Detailed analysis and recommendations.

- Quality score comparisons
- Cost efficiency analysis
- Latency benchmarks
- Best model identification
- Improvement rates

## Metrics

### Quality Metrics (0.0-1.0)

- **Score**: Overall quality score (weighted average)
- **Accuracy**: Output correctness and format compliance
- **Coherence**: Logical flow and consistency
- **Relevance**: Alignment with input requirements
- **Diversity**: Vocabulary richness
- **Creativity**: Novel expression and uncommon patterns

### Performance Metrics

- **Latency**: Generation time (milliseconds)
- **Throughput**: Samples per second
- **Tokens Used**: Total token consumption
- **Cost**: USD per generation
- **Memory Usage**: Heap usage (MB)
- **Error Rate**: Failed generations ratio

### Training Metrics

- **Convergence Rate**: Quality improvement velocity
- **Improvement Rate**: Total quality gain percentage
- **Cost Efficiency**: Quality per dollar spent
- **Learning Speed**: Iterations to convergence

## Usage Examples

### Basic Training

```typescript
import { DSPyTrainingSession, ModelProvider } from './training/dspy-learning-session.js';

const session = new DSPyTrainingSession({
  models: [
    {
      provider: ModelProvider.CLAUDE,
      model: 'claude-sonnet-4',
      apiKey: process.env.ANTHROPIC_API_KEY
    },
    {
      provider: ModelProvider.GEMINI,
      model: 'gemini-2.0-flash-exp',
      apiKey: process.env.GEMINI_API_KEY
    }
  ],
  optimizationRounds: 5,
  costBudget: 5.0
});

// Listen to events
session.on('iteration', (result) => {
  console.log(`${result.modelProvider}: Quality=${result.quality.score.toFixed(3)}`);
});

session.on('complete', (data) => {
  console.log('Training complete!');
  console.log(data.report);
});

// Run training
const signature = optimizer.createSignature(
  'task',
  'input',
  'output',
  { constraints: ['min_length:100'] }
);

await session.run('Your prompt here', signature);
```

### Cost-Optimized Training

```typescript
const session = new DSPyTrainingSession({
  models: [
    {
      provider: ModelProvider.GEMINI, // Low cost
      model: 'gemini-2.0-flash-exp',
      apiKey: process.env.GEMINI_API_KEY
    },
    {
      provider: ModelProvider.LLAMA, // Very low cost
      model: 'llama-3.1-70b',
      apiKey: process.env.TOGETHER_API_KEY
    }
  ],
  optimizationRounds: 3,
  baselineIterations: 2,
  benchmarkSamples: 20,
  costBudget: 1.0 // Strict $1 budget
});
```

### Quality-Focused Training

```typescript
const session = new DSPyTrainingSession({
  models: [
    {
      provider: ModelProvider.CLAUDE,
      model: 'claude-sonnet-4',
      apiKey: process.env.ANTHROPIC_API_KEY,
      temperature: 0.3 // Lower for consistency
    },
    {
      provider: ModelProvider.GPT4,
      model: 'gpt-4-turbo',
      apiKey: process.env.OPENAI_API_KEY,
      temperature: 0.3
    }
  ],
  optimizationRounds: 15,
  convergenceThreshold: 0.98,
  benchmarkSamples: 100
});
```

## Event System

### Available Events

- `start`: Training session begins
- `phase`: Phase transition
- `iteration`: Single iteration complete
- `metrics`: Real-time metrics update
- `optimization_round`: Optimization round starts
- `converged`: Model reaches convergence
- `benchmark_progress`: Benchmark progress update
- `budget_exceeded`: Cost budget exceeded
- `report`: Final report generated
- `complete`: Training session complete
- `stopped`: Session manually stopped
- `error`: Error occurred
- `hooks_integration`: Hooks coordination event

### Event Listeners

```typescript
session.on('iteration', (result: IterationResult) => {
  // Handle each iteration
});

session.on('phase', (phase: TrainingPhase) => {
  // Handle phase transitions
});

session.on('metrics', (metrics) => {
  // Track real-time metrics
});

session.on('complete', (data) => {
  // Process final results
});
```

## Integration

### Claude Flow Hooks

When `enableHooksIntegration: true`, the session automatically:

1. **Pre-Task**: Initialize swarm coordination
2. **During Training**: Store results in shared memory
3. **Post-Task**: Export metrics and best models
4. **Session End**: Generate coordination reports

### Memory Coordination

```typescript
// Results stored in swarm memory
{
  key: 'swarm/training/dspy-results',
  value: {
    bestModel: 'claude',
    comparison: { /* stats */ },
    totalCost: 5.23,
    timestamp: '2025-11-22T...'
  }
}
```

## Configuration

### TrainingConfig

```typescript
interface TrainingConfig {
  models: ModelConfig[];              // Array of model configurations
  optimizationRounds?: number;        // Default: 5
  convergenceThreshold?: number;      // Default: 0.95
  maxConcurrency?: number;            // Default: 4
  enableCrossLearning?: boolean;      // Default: true
  enableHooksIntegration?: boolean;   // Default: true
  costBudget?: number;                // USD, optional
  timeoutPerIteration?: number;       // Default: 30000ms
  baselineIterations?: number;        // Default: 3
  benchmarkSamples?: number;          // Default: 100
}
```

### ModelConfig

```typescript
interface ModelConfig {
  provider: ModelProvider;
  model: string;
  apiKey: string;
  temperature?: number;               // Default: 0.7
  maxTokens?: number;                 // Default: 1000
  topP?: number;                      // Optional
  presencePenalty?: number;           // Optional
  frequencyPenalty?: number;          // Optional
}
```

### DSPySignature

```typescript
interface DSPySignature {
  input: string;                      // Input description
  output: string;                     // Expected output format
  examples?: Array<{                  // Few-shot examples
    input: string;
    output: string;
  }>;
  constraints?: string[];             // Validation rules
  objectives?: string[];              // Optimization goals
}
```

## Cost Information

### Model Pricing (Approximate)

| Model | Cost per 1K tokens | Relative Cost |
|-------|-------------------|---------------|
| Gemini Flash | $0.00025 | 1x (cheapest) |
| Llama 3.1 | $0.0002 | 0.8x |
| Claude Sonnet | $0.003 | 12x |
| GPT-4 Turbo | $0.03 | 120x |

### Budget Planning

For typical training session:

- **Budget $1**: ~200 iterations with Gemini/Llama
- **Budget $5**: ~100 iterations with Claude + mixed models
- **Budget $10**: ~50 iterations with all models including GPT-4

## Best Practices

### 1. Start Small

```typescript
// Begin with 2 models and low iterations
const session = new DSPyTrainingSession({
  models: [
    { provider: ModelProvider.GEMINI, /* ... */ },
    { provider: ModelProvider.CLAUDE, /* ... */ }
  ],
  optimizationRounds: 3,
  benchmarkSamples: 20
});
```

### 2. Use Cost-Effective Models First

Train with Gemini/Llama first, then validate winners with Claude/GPT-4.

### 3. Set Realistic Budgets

Start with $1-2 budgets for experimentation.

### 4. Monitor Convergence

Enable convergence detection to avoid over-training.

### 5. Leverage Cross-Learning

Enable cross-model learning to share best practices.

### 6. Define Clear Signatures

Provide examples, constraints, and objectives for better optimization.

## Troubleshooting

### High Costs

- Reduce `benchmarkSamples`
- Lower `optimizationRounds`
- Use cost-effective models (Gemini, Llama)
- Set strict `costBudget`

### Slow Convergence

- Increase `optimizationRounds`
- Add more examples to DSPy signature
- Adjust model temperature (lower = more consistent)
- Enable cross-model learning

### Low Quality Scores

- Review DSPy signature constraints
- Add more few-shot examples
- Increase `convergenceThreshold`
- Use higher-quality models

### Memory Issues

- Reduce `maxConcurrency`
- Lower `benchmarkSamples`
- Clear results between sessions

## Examples

See `examples/dspy-training-example.ts` for:

1. Basic training session
2. Advanced monitoring
3. Cost-optimized training
4. Quality-focused training
5. Benchmark comparison

Run examples:

```bash
# Run basic example
npm run example:dspy 0

# Run cost-optimized example
npm run example:dspy 2

# Run quality-focused example
npm run example:dspy 3
```

## API Reference

### Classes

- `DSPyTrainingSession`: Main orchestrator
- `ModelTrainingAgent`: Base agent class
- `ClaudeSonnetAgent`: Claude training agent
- `GPT4Agent`: GPT-4 training agent
- `LlamaAgent`: Llama training agent
- `GeminiAgent`: Gemini training agent
- `OptimizationEngine`: DSPy optimization
- `BenchmarkCollector`: Metrics collection

### Enums

- `ModelProvider`: Model provider types
- `TrainingPhase`: Training pipeline phases

### Interfaces

- `TrainingConfig`: Session configuration
- `ModelConfig`: Model configuration
- `DSPySignature`: DSPy signature definition
- `QualityMetrics`: Quality measurement
- `PerformanceMetrics`: Performance measurement
- `IterationResult`: Single iteration result

## License

MIT

## Contributing

Contributions welcome! Please see [CONTRIBUTING.md](../CONTRIBUTING.md).

## Support

- Issues: https://github.com/ruvnet/ruvector/issues
- Documentation: https://github.com/ruvnet/ruvector/tree/main/packages/agentic-synth
