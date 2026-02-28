# DSPy.ts + AgenticSynth Complete Integration Guide

## Overview

This comprehensive example demonstrates real-world integration between DSPy.ts (v2.1.1) and AgenticSynth for e-commerce product data generation with automatic optimization.

## What This Example Does

### 🎯 Complete Workflow

1. **Baseline Generation**: Uses AgenticSynth with Gemini to generate product data
2. **DSPy Setup**: Configures OpenAI with ChainOfThought reasoning module
3. **Optimization**: Uses BootstrapFewShot to learn from high-quality examples
4. **Comparison**: Analyzes quality improvements, cost, and performance
5. **Reporting**: Generates detailed comparison metrics and visualizations

### 🔧 Technologies Used

- **DSPy.ts v2.1.1**: Real modules (ChainOfThought, BootstrapFewShot, metrics)
- **AgenticSynth**: Baseline synthetic data generation
- **OpenAI GPT-3.5**: Optimized generation with reasoning
- **Gemini Flash**: Fast baseline generation
- **TypeScript**: Type-safe implementation

## Setup

### Prerequisites

```bash
node >= 18.0.0
npm >= 9.0.0
```

### Environment Variables

Create a `.env` file in the package root:

```bash
# Required
OPENAI_API_KEY=sk-...                    # OpenAI API key
GEMINI_API_KEY=...                       # Google AI Studio API key

# Optional
ANTHROPIC_API_KEY=sk-ant-...             # For Claude models
```

### Installation

```bash
# Install dependencies
npm install

# Build the package
npm run build
```

## Running the Example

### Basic Usage

```bash
# Set environment variables
export OPENAI_API_KEY=sk-...
export GEMINI_API_KEY=...

# Run the example
npx tsx examples/dspy-complete-example.ts
```

### Expected Output

```
╔════════════════════════════════════════════════════════════════════════╗
║         DSPy.ts + AgenticSynth Integration Example                    ║
║         E-commerce Product Data Generation with Optimization           ║
╚════════════════════════════════════════════════════════════════════════╝

✅ Environment validated

🔷 PHASE 1: BASELINE GENERATION

📦 Generating baseline data with AgenticSynth (Gemini)...

  ✓ [1/10] UltraSound Pro Wireless Headphones
    Quality: 72.3% | Price: $249.99 | Rating: 4.7/5
  ✓ [2/10] EcoLux Organic Cotton T-Shirt
    Quality: 68.5% | Price: $79.99 | Rating: 4.5/5
  ...

✅ Baseline generation complete: 10/10 products in 8.23s
💰 Estimated cost: $0.0005

🔷 PHASE 2: DSPy OPTIMIZATION

🧠 Setting up DSPy optimization with OpenAI...

  📡 Configuring OpenAI language model...
  ✓ Language model configured

  🔧 Creating ChainOfThought module...
  ✓ Module created

  📚 Loading training examples...
  ✓ Loaded 5 high-quality examples

  🎯 Running BootstrapFewShot optimizer...
  ✓ Optimization complete in 12.45s

✅ DSPy module ready for generation

🔷 PHASE 3: OPTIMIZED GENERATION

🚀 Generating optimized data with DSPy + OpenAI...

  ✓ [1/10] SmartHome Voice Assistant Hub
    Quality: 85.7% | Price: $179.99 | Rating: 4.8/5
  ...

✅ Optimized generation complete: 10/10 products in 15.67s
💰 Estimated cost: $0.0070

🔷 PHASE 4: ANALYSIS & REPORTING

╔════════════════════════════════════════════════════════════════════════╗
║                     COMPARISON REPORT                                  ║
╚════════════════════════════════════════════════════════════════════════╝

📊 BASELINE (AgenticSynth + Gemini)
────────────────────────────────────────────────────────────────────────────
Products Generated:    10
Generation Time:       8.23s
Estimated Cost:        $0.0005

Quality Metrics:
  Overall Quality:     68.2%
  Completeness:        72.5%
  Coherence:           65.0%
  Persuasiveness:      60.8%
  SEO Quality:         74.5%

🚀 OPTIMIZED (DSPy + OpenAI)
────────────────────────────────────────────────────────────────────────────
Products Generated:    10
Generation Time:       15.67s
Estimated Cost:        $0.0070

Quality Metrics:
  Overall Quality:     84.3%
  Completeness:        88.2%
  Coherence:           82.5%
  Persuasiveness:      85.0%
  SEO Quality:         81.5%

📈 IMPROVEMENT ANALYSIS
────────────────────────────────────────────────────────────────────────────
Quality Gain:          +23.6%
Speed Change:          +90.4%
Cost Efficiency:       +14.8%

📊 QUALITY COMPARISON CHART
────────────────────────────────────────────────────────────────────────────
Baseline:  ██████████████████████████████████ 68.2%
Optimized: ██████████████████████████████████████████ 84.3%

💡 KEY INSIGHTS
────────────────────────────────────────────────────────────────────────────
✓ Significant quality improvement with DSPy optimization
✓ Better cost efficiency with optimized approach

════════════════════════════════════════════════════════════════════════════

📁 Results exported to: .../examples/logs/dspy-comparison-results.json

✅ Example complete!

💡 Next steps:
   1. Review the comparison report above
   2. Check exported JSON for detailed results
   3. Experiment with different training examples
   4. Try other DSPy modules (Refine, ReAct, etc.)
   5. Adjust CONFIG parameters for your use case
```

## Configuration

### Customizable Parameters

Edit the `CONFIG` object in the example file:

```typescript
const CONFIG = {
  SAMPLE_SIZE: 10,           // Number of products to generate
  TRAINING_EXAMPLES: 5,      // Examples for DSPy optimization
  BASELINE_MODEL: 'gemini-2.0-flash-exp',
  OPTIMIZED_MODEL: 'gpt-3.5-turbo',

  CATEGORIES: [
    'Electronics',
    'Fashion',
    'Home & Garden',
    'Sports & Outdoors',
    'Books & Media',
    'Health & Beauty'
  ],

  PRICE_RANGES: {
    low: { min: 10, max: 50 },
    medium: { min: 50, max: 200 },
    high: { min: 200, max: 1000 }
  }
};
```

## Understanding the Code

### Phase 1: Baseline Generation

```typescript
const synth = new AgenticSynth({
  provider: 'gemini',
  model: 'gemini-2.0-flash-exp',
  apiKey: process.env.GEMINI_API_KEY
});

const result = await synth.generateStructured<Product>({
  prompt: '...',
  schema: { /* product schema */ },
  count: 1
});
```

**Purpose**: Establishes baseline quality and cost metrics using standard generation.

### Phase 2: DSPy Setup

```typescript
// Configure language model
const lm = new OpenAILM({
  model: 'gpt-3.5-turbo',
  apiKey: process.env.OPENAI_API_KEY,
  temperature: 0.7
});
await lm.init();
configureLM(lm);

// Create reasoning module
const productGenerator = new ChainOfThought({
  name: 'ProductGenerator',
  signature: {
    inputs: [
      { name: 'category', type: 'string', required: true },
      { name: 'priceRange', type: 'string', required: true }
    ],
    outputs: [
      { name: 'name', type: 'string', required: true },
      { name: 'description', type: 'string', required: true },
      { name: 'price', type: 'number', required: true },
      { name: 'rating', type: 'number', required: true }
    ]
  }
});
```

**Purpose**: Sets up DSPy's declarative reasoning framework.

### Phase 3: Optimization

```typescript
const optimizer = new BootstrapFewShot({
  metric: productQualityMetric,
  maxBootstrappedDemos: 5,
  maxLabeledDemos: 3,
  teacherSettings: { temperature: 0.5 },
  maxRounds: 2
});

const optimizedModule = await optimizer.compile(
  productGenerator,
  trainingExamples
);
```

**Purpose**: Learns from high-quality examples to improve generation.

### Phase 4: Generation with Optimized Module

```typescript
const result = await optimizedModule.forward({
  category: 'Electronics',
  priceRange: '$100-$500'
});

const product: Product = {
  name: result.name,
  description: result.description,
  price: result.price,
  rating: result.rating
};
```

**Purpose**: Uses optimized prompts and reasoning chains learned during compilation.

## Quality Metrics Explained

The example calculates four quality dimensions:

### 1. Completeness (40% weight)
- Description length (100-500 words)
- Contains features/benefits
- Has call-to-action

### 2. Coherence (20% weight)
- Sentence structure quality
- Average sentence length (15-25 words ideal)
- Natural flow

### 3. Persuasiveness (20% weight)
- Persuasive language usage
- Emotional appeal
- Value proposition clarity

### 4. SEO Quality (20% weight)
- Product name in description
- Keyword presence
- Discoverability

## Advanced Usage

### Using Different DSPy Modules

#### Refine Module (Iterative Improvement)

```typescript
import { Refine } from 'dspy.ts';

const refiner = new Refine({
  name: 'ProductRefiner',
  signature: { /* ... */ },
  maxIterations: 3,
  constraints: [
    { field: 'description', check: (val) => val.length >= 100 }
  ]
});
```

#### ReAct Module (Reasoning + Acting)

```typescript
import { ReAct } from 'dspy.ts';

const reactor = new ReAct({
  name: 'ProductResearcher',
  signature: { /* ... */ },
  tools: [searchTool, pricingTool]
});
```

### Custom Metrics

```typescript
import { createMetric } from 'dspy.ts';

const customMetric = createMetric(
  'brand-consistency',
  (example, prediction) => {
    // Your custom evaluation logic
    const score = calculateBrandScore(prediction);
    return score;
  }
);
```

### Integration with AgenticDB

```typescript
import { AgenticDB } from 'agentdb';

// Store products in vector database
const db = new AgenticDB();
await db.init();

for (const product of optimizedProducts) {
  await db.add({
    id: product.id,
    text: product.description,
    metadata: { category: product.category, price: product.price }
  });
}

// Semantic search
const similar = await db.search('wireless noise cancelling headphones', {
  limit: 5
});
```

## Troubleshooting

### Common Issues

#### 1. Module Not Found

```bash
Error: Cannot find module 'dspy.ts'
```

**Solution**: Ensure dependencies are installed:
```bash
npm install
```

#### 2. API Key Not Found

```bash
❌ Missing required environment variables:
   - OPENAI_API_KEY
```

**Solution**: Export environment variables:
```bash
export OPENAI_API_KEY=sk-...
export GEMINI_API_KEY=...
```

#### 3. Rate Limiting

```bash
Error: Rate limit exceeded
```

**Solution**: Add delays or reduce `SAMPLE_SIZE`:
```typescript
const CONFIG = {
  SAMPLE_SIZE: 5,  // Reduce from 10
  // ...
};
```

#### 4. Out of Memory

**Solution**: Process in smaller batches:
```typescript
const batchSize = 5;
for (let i = 0; i < totalProducts; i += batchSize) {
  const batch = await generateBatch(batchSize);
  // Process batch
}
```

## Performance Tips

### 1. Parallel Generation

```typescript
const promises = categories.map(category =>
  optimizedModule.forward({ category, priceRange })
);
const results = await Promise.all(promises);
```

### 2. Caching

```typescript
const synth = new AgenticSynth({
  cacheStrategy: 'redis',
  cacheTTL: 3600,
  // ...
});
```

### 3. Streaming

```typescript
for await (const product of synth.generateStream('structured', options)) {
  console.log('Generated:', product);
  // Process immediately
}
```

## Cost Optimization

### Model Selection Strategy

| Use Case | Baseline Model | Optimized Model | Notes |
|----------|---------------|-----------------|-------|
| High Quality | GPT-4 | Claude Opus | Premium quality |
| Balanced | Gemini Flash | GPT-3.5 Turbo | Good quality/cost |
| Cost-Effective | Gemini Flash | Gemini Flash | Minimal cost |
| High Volume | Llama 3.1 | Gemini Flash | Maximum throughput |

### Budget Management

```typescript
const CONFIG = {
  MAX_BUDGET: 1.0,  // $1 USD limit
  COST_PER_TOKEN: 0.0005,
  // ...
};

let totalCost = 0;
for (let i = 0; i < products && totalCost < CONFIG.MAX_BUDGET; i++) {
  const result = await generate();
  totalCost += estimateCost(result);
}
```

## Testing

### Unit Tests

```typescript
import { describe, it, expect } from 'vitest';
import { calculateQualityMetrics } from './dspy-complete-example';

describe('Quality Metrics', () => {
  it('should calculate completeness correctly', () => {
    const product = {
      name: 'Test Product',
      description: 'A'.repeat(150),
      price: 99.99,
      rating: 4.5
    };

    const metrics = calculateQualityMetrics(product);
    expect(metrics.completeness).toBeGreaterThan(0);
  });
});
```

### Integration Tests

```bash
npm run test -- examples/dspy-complete-example.test.ts
```

## Resources

### Documentation
- [DSPy.ts GitHub](https://github.com/ruvnet/dspy.ts)
- [AgenticSynth Docs](https://github.com/ruvnet/ruvector/tree/main/packages/agentic-synth)
- [DSPy Paper](https://arxiv.org/abs/2310.03714)

### Examples
- [Basic Usage](./basic-usage.ts)
- [Integration Examples](./integration-examples.ts)
- [Training Examples](./dspy-training-example.ts)

### Community
- [Discord](https://discord.gg/dspy)
- [GitHub Discussions](https://github.com/ruvnet/dspy.ts/discussions)

## License

MIT License - See LICENSE file for details

## Contributing

Contributions welcome! Please see CONTRIBUTING.md for guidelines.

---

**Built with ❤️ by rUv**
