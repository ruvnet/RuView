# DSPy.ts + AgenticSynth Quick Reference

## 🚀 Quick Start

```bash
# Setup
export OPENAI_API_KEY=sk-...
export GEMINI_API_KEY=...

# Verify
npx tsx examples/dspy-verify-setup.ts

# Run
npx tsx examples/dspy-complete-example.ts
```

## 📦 Core Imports

```typescript
// DSPy.ts modules
import {
  ChainOfThought,      // Reasoning module
  Predict,             // Basic prediction
  Refine,              // Iterative refinement
  ReAct,               // Reasoning + Acting
  Retrieve,            // RAG with vector search
  BootstrapFewShot,    // Few-shot optimizer
  MIPROv2,             // Bayesian optimizer
  configureLM,         // Configure LM
  OpenAILM,            // OpenAI provider
  AnthropicLM,         // Anthropic provider
  createMetric,        // Custom metrics
  evaluate             // Evaluation
} from 'dspy.ts';

// AgenticSynth
import { AgenticSynth } from '@ruvector/agentic-synth';
```

## 🔧 Basic Setup

### AgenticSynth

```typescript
const synth = new AgenticSynth({
  provider: 'gemini',              // 'gemini' | 'openrouter' | 'anthropic'
  model: 'gemini-2.0-flash-exp',
  apiKey: process.env.GEMINI_API_KEY,
  cacheStrategy: 'memory',         // 'memory' | 'redis'
  cacheTTL: 3600,
  streaming: false
});
```

### DSPy Language Model

```typescript
// OpenAI
const lm = new OpenAILM({
  model: 'gpt-3.5-turbo',          // or 'gpt-4'
  apiKey: process.env.OPENAI_API_KEY,
  temperature: 0.7,
  maxTokens: 600
});
await lm.init();
configureLM(lm);

// Anthropic
const lm = new AnthropicLM({
  model: 'claude-3-5-sonnet-20241022',
  apiKey: process.env.ANTHROPIC_API_KEY,
  temperature: 0.7,
  maxTokens: 600
});
await lm.init();
configureLM(lm);
```

## 📝 DSPy Modules

### Predict (Basic)

```typescript
const predictor = new Predict({
  name: 'SimplePredictor',
  signature: {
    inputs: [
      { name: 'input', type: 'string', required: true }
    ],
    outputs: [
      { name: 'output', type: 'string', required: true }
    ]
  }
});

const result = await predictor.forward({ input: 'Hello' });
```

### ChainOfThought (Reasoning)

```typescript
const cot = new ChainOfThought({
  name: 'ReasoningModule',
  signature: {
    inputs: [
      { name: 'question', type: 'string', required: true }
    ],
    outputs: [
      { name: 'reasoning', type: 'string', required: true },
      { name: 'answer', type: 'string', required: true }
    ]
  }
});

const result = await cot.forward({ question: 'What is 2+2?' });
// result.reasoning: "Let me think step by step..."
// result.answer: "4"
```

### Refine (Iterative)

```typescript
const refiner = new Refine({
  name: 'Refiner',
  signature: { /* ... */ },
  maxIterations: 3,
  constraints: [
    {
      field: 'output',
      check: (value) => value.length >= 100,
      message: 'Output must be at least 100 characters'
    }
  ]
});

const result = await refiner.forward({ input: '...' });
```

### ReAct (Reasoning + Actions)

```typescript
const reactor = new ReAct({
  name: 'Agent',
  signature: { /* ... */ },
  tools: [
    {
      name: 'search',
      description: 'Search the web',
      execute: async (query: string) => {
        return await searchWeb(query);
      }
    }
  ],
  maxIterations: 5
});

const result = await reactor.forward({ task: '...' });
```

### Retrieve (RAG)

```typescript
import { AgenticDB } from 'agentdb';

const db = new AgenticDB();
await db.init();

const retriever = new Retrieve({
  name: 'RAGRetriever',
  signature: { /* ... */ },
  vectorStore: db,
  topK: 5
});

const result = await retriever.forward({ query: '...' });
```

## 🎯 Optimizers

### BootstrapFewShot

```typescript
const optimizer = new BootstrapFewShot({
  metric: customMetric,           // Evaluation metric
  maxBootstrappedDemos: 10,       // Max examples to generate
  maxLabeledDemos: 5,             // Max labeled examples
  teacherSettings: {
    temperature: 0.5
  },
  maxRounds: 2                     // Optimization rounds
});

const optimizedModule = await optimizer.compile(module, examples);
```

### MIPROv2

```typescript
const optimizer = new MIPROv2({
  metric: customMetric,
  numCandidates: 10,               // Instructions to try
  numTrials: 3,                    // Optimization trials
  miniBatchSize: 25,
  maxBootstrappedDemos: 3,
  maxLabeledDemos: 5
});

const optimizedModule = await optimizer.compile(module, examples);
```

## 📊 Metrics

### Built-in Metrics

```typescript
import {
  exactMatch,           // Exact string match
  f1Score,              // F1 score
  answerSimilarity,     // Semantic similarity
  contains,             // Substring check
  semanticSimilarity,   // Embedding similarity
  bleuScore,            // BLEU score
  rougeL,               // ROUGE-L score
  accuracy,             // Classification accuracy
  passAtK,              // Pass@K
  meanReciprocalRank    // MRR
} from 'dspy.ts';
```

### Custom Metrics

```typescript
const customMetric = createMetric<InputType, OutputType>(
  'metric-name',
  (example, prediction, trace) => {
    // Return score between 0 and 1
    return calculateScore(example, prediction);
  }
);
```

### Evaluation

```typescript
const results = await evaluate(
  module,
  testExamples,
  metric,
  {
    displayProgress: true,
    returnOutputs: true
  }
);

console.log('Average Score:', results.avgScore);
console.log('Outputs:', results.outputs);
```

## 🔄 Complete Workflow

```typescript
// 1. Setup
const lm = new OpenAILM({ /* ... */ });
await lm.init();
configureLM(lm);

// 2. Create module
const module = new ChainOfThought({
  name: 'MyModule',
  signature: { /* ... */ }
});

// 3. Create metric
const metric = createMetric('quality', (ex, pred) => {
  return calculateQuality(pred);
});

// 4. Prepare examples
const trainingExamples = [
  { input: '...', output: '...' },
  // ...
];

// 5. Optimize
const optimizer = new BootstrapFewShot({ metric });
const optimized = await optimizer.compile(module, trainingExamples);

// 6. Use
const result = await optimized.forward({ input: '...' });

// 7. Evaluate
const evalResults = await evaluate(
  optimized,
  testExamples,
  metric
);
```

## 💡 Common Patterns

### Baseline vs Optimized Comparison

```typescript
// Baseline
const synth = new AgenticSynth({ provider: 'gemini' });
const baseline = await synth.generateStructured({ /* ... */ });

// Optimized
const lm = new OpenAILM({ /* ... */ });
configureLM(lm);
const module = new ChainOfThought({ /* ... */ });
const optimizer = new BootstrapFewShot({ /* ... */ });
const optimized = await optimizer.compile(module, examples);
const result = await optimized.forward({ /* ... */ });

// Compare
const improvement = calculateImprovement(baseline, result);
```

### Streaming Generation

```typescript
const synth = new AgenticSynth({ streaming: true });

for await (const item of synth.generateStream('structured', options)) {
  console.log('Generated:', item);
  // Process immediately
}
```

### Batch Processing

```typescript
const batchOptions = [
  { prompt: 'Generate product 1' },
  { prompt: 'Generate product 2' },
  { prompt: 'Generate product 3' }
];

const results = await synth.generateBatch(
  'structured',
  batchOptions,
  3  // concurrency
);
```

### Error Handling

```typescript
try {
  const result = await module.forward({ input });
} catch (error) {
  if (error.message.includes('rate limit')) {
    // Handle rate limiting
    await delay(1000);
    return retry();
  } else if (error.message.includes('timeout')) {
    // Handle timeout
    return null;
  }
  throw error;
}
```

## 🎛️ Configuration

### Environment Variables

```bash
# Required
OPENAI_API_KEY=sk-...
GEMINI_API_KEY=...

# Optional
ANTHROPIC_API_KEY=sk-ant-...
OPENROUTER_API_KEY=sk-or-...
REDIS_URL=redis://localhost:6379
```

### Model Selection

| Use Case | Model | Speed | Cost | Quality |
|----------|-------|-------|------|---------|
| Baseline | gemini-2.0-flash-exp | ⚡⚡⚡ | 💰 | ⭐⭐⭐ |
| Production | gpt-3.5-turbo | ⚡⚡ | 💰💰 | ⭐⭐⭐⭐ |
| High Quality | gpt-4 | ⚡ | 💰💰💰 | ⭐⭐⭐⭐⭐ |
| Premium | claude-3-5-sonnet | ⚡ | 💰💰💰 | ⭐⭐⭐⭐⭐ |

## 📈 Performance Tips

### 1. Parallel Execution

```typescript
const promises = items.map(item =>
  optimized.forward(item)
);
const results = await Promise.all(promises);
```

### 2. Caching

```typescript
const synth = new AgenticSynth({
  cacheStrategy: 'redis',
  cacheTTL: 3600
});
```

### 3. Batch Size

```typescript
const BATCH_SIZE = 5;
for (let i = 0; i < total; i += BATCH_SIZE) {
  const batch = items.slice(i, i + BATCH_SIZE);
  await processBatch(batch);
}
```

### 4. Temperature Control

```typescript
// More consistent (lower temperature)
const lm = new OpenAILM({ temperature: 0.3 });

// More creative (higher temperature)
const lm = new OpenAILM({ temperature: 0.9 });
```

## 🐛 Debugging

### Enable Logging

```typescript
import { logger } from 'dspy.ts';

logger.level = 'debug';  // 'debug' | 'info' | 'warn' | 'error'
```

### Inspect Traces

```typescript
const result = await module.forward({ input }, { trace: true });

console.log('Trace:', result.__trace);
// Shows all LM calls, prompts, and outputs
```

### Check Demos

```typescript
console.log('Learned Demos:', optimized.__demos);
// Shows examples the module learned from
```

## 🔗 Resources

- [Complete Example](../dspy-complete-example.ts)
- [Comprehensive Guide](./dspy-complete-example-guide.md)
- [Examples Index](./README.md)
- [DSPy.ts GitHub](https://github.com/ruvnet/dspy.ts)
- [AgenticSynth README](../README.md)

## 💬 Support

- GitHub Issues: [ruvector/issues](https://github.com/ruvnet/ruvector/issues)
- Discord: [Join](https://discord.gg/ruvector)
- Email: [support@ruv.io](mailto:support@ruv.io)

---

**Quick Ref v1.0 | Built by [rUv](https://github.com/ruvnet)**
