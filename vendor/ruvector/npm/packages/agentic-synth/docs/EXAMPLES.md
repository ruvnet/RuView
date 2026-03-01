# Advanced Examples

Comprehensive examples for Agentic-Synth across various use cases.

## Table of Contents

- [Customer Support Agent](#customer-support-agent)
- [RAG Training Data](#rag-training-data)
- [Code Assistant Memory](#code-assistant-memory)
- [Product Recommendations](#product-recommendations)
- [Test Data Generation](#test-data-generation)
- [Multi-language Support](#multi-language-support)
- [Streaming Generation](#streaming-generation)
- [Batch Processing](#batch-processing)
- [Custom Generators](#custom-generators)
- [Advanced Schemas](#advanced-schemas)

---

## Customer Support Agent

Generate realistic multi-turn customer support conversations.

### Basic Example

```typescript
import { SynthEngine, Schema } from 'agentic-synth';

const synth = new SynthEngine({
  provider: 'openai',
  model: 'gpt-4',
});

const schema = Schema.conversation({
  domain: 'customer-support',
  personas: [
    {
      name: 'customer',
      traits: ['frustrated', 'needs-help', 'time-constrained'],
      temperature: 0.9,
    },
    {
      name: 'agent',
      traits: ['professional', 'empathetic', 'solution-oriented'],
      temperature: 0.7,
    },
  ],
  topics: [
    'billing-dispute',
    'technical-issue',
    'feature-request',
    'shipping-delay',
    'refund-request',
  ],
  turns: { min: 6, max: 15 },
});

const conversations = await synth.generate({
  schema,
  count: 5000,
  progressCallback: (progress) => {
    console.log(`Generated ${progress.current}/${progress.total} conversations`);
  },
});

await conversations.export({
  format: 'jsonl',
  outputPath: './training/customer-support.jsonl',
});
```

### With Quality Filtering

```typescript
import { QualityMetrics } from 'agentic-synth';

const conversations = await synth.generate({ schema, count: 10000 });

// Filter for high-quality examples
const highQuality = conversations.filter(async (conv) => {
  const metrics = await QualityMetrics.evaluate([conv], {
    realism: true,
    coherence: true,
  });
  return metrics.overall > 0.90;
});

console.log(`Kept ${highQuality.data.length} high-quality conversations`);
```

### With Embeddings for Semantic Search

```typescript
const schema = Schema.conversation({
  domain: 'customer-support',
  personas: ['customer', 'agent'],
  topics: ['billing', 'technical', 'shipping'],
  turns: { min: 4, max: 12 },
  includeEmbeddings: true,
});

const conversations = await synth.generateAndInsert({
  schema,
  count: 10000,
  collection: 'support-conversations',
  batchSize: 1000,
});

// Now searchable by semantic similarity
```

---

## RAG Training Data

Generate question-answer pairs with context for retrieval-augmented generation.

### From Documentation

```typescript
import { RAGDataGenerator } from 'agentic-synth';

const ragData = await RAGDataGenerator.create({
  domain: 'technical-documentation',
  sources: [
    './docs/**/*.md',
    './api-specs/**/*.yaml',
    'https://docs.example.com',
  ],
  questionsPerSource: 10,
  includeNegatives: true,  // For contrastive learning
  difficulty: 'mixed',
});

await ragData.export({
  format: 'parquet',
  outputPath: './training/rag-pairs.parquet',
  includeVectors: true,
});
```

### Custom RAG Schema

```typescript
const ragSchema = Schema.define({
  name: 'RAGTrainingPair',
  type: 'object',
  properties: {
    question: {
      type: 'string',
      description: 'User question requiring retrieval',
    },
    context: {
      type: 'string',
      description: 'Retrieved document context',
    },
    answer: {
      type: 'string',
      description: 'Answer derived from context',
    },
    reasoning: {
      type: 'string',
      description: 'Chain-of-thought reasoning',
    },
    difficulty: {
      type: 'string',
      enum: ['easy', 'medium', 'hard'],
    },
    type: {
      type: 'string',
      enum: ['factual', 'analytical', 'creative', 'multi-hop'],
    },
    embedding: {
      type: 'embedding',
      dimensions: 384,
    },
  },
  required: ['question', 'context', 'answer'],
});

const data = await synth.generate({ schema: ragSchema, count: 50000 });
```

### Multi-Hop RAG Questions

```typescript
const multiHopSchema = Schema.define({
  name: 'MultiHopRAG',
  type: 'object',
  properties: {
    question: { type: 'string' },
    requiredContexts: {
      type: 'array',
      items: { type: 'string' },
      minItems: 2,
      maxItems: 5,
    },
    intermediateSteps: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          step: { type: 'string' },
          retrievedInfo: { type: 'string' },
          reasoning: { type: 'string' },
        },
      },
    },
    finalAnswer: { type: 'string' },
  },
});

const multiHopData = await synth.generate({
  schema: multiHopSchema,
  count: 10000,
});
```

---

## Code Assistant Memory

Generate realistic agent interaction histories for code assistants.

### Basic Code Assistant Memory

```typescript
import { AgentMemoryGenerator } from 'agentic-synth';

const memory = await AgentMemoryGenerator.synthesize({
  agentType: 'code-assistant',
  interactions: 5000,
  userPersonas: [
    'junior-developer',
    'senior-developer',
    'tech-lead',
    'student',
  ],
  taskDistribution: {
    'bug-fix': 0.35,
    'feature-implementation': 0.25,
    'code-review': 0.15,
    'refactoring': 0.15,
    'optimization': 0.10,
  },
  includeEmbeddings: true,
});

await memory.export({
  format: 'jsonl',
  outputPath: './training/code-assistant-memory.jsonl',
});
```

### With Code Context

```typescript
const codeMemorySchema = Schema.define({
  name: 'CodeAssistantMemory',
  type: 'object',
  properties: {
    id: { type: 'string', format: 'uuid' },
    timestamp: { type: 'date' },
    userPersona: {
      type: 'string',
      enum: ['junior', 'mid', 'senior', 'lead'],
    },
    language: {
      type: 'string',
      enum: ['typescript', 'python', 'rust', 'go', 'java'],
    },
    taskType: {
      type: 'string',
      enum: ['debug', 'implement', 'review', 'refactor', 'optimize'],
    },
    userCode: { type: 'string' },
    userQuestion: { type: 'string' },
    agentResponse: { type: 'string' },
    suggestedCode: { type: 'string' },
    explanation: { type: 'string' },
    embedding: { type: 'embedding', dimensions: 768 },
  },
});

const codeMemory = await synth.generate({
  schema: codeMemorySchema,
  count: 25000,
});
```

### Multi-Turn Code Sessions

```typescript
const sessionSchema = Schema.conversation({
  domain: 'code-pair-programming',
  personas: [
    {
      name: 'developer',
      traits: ['curious', 'detail-oriented', 'iterative'],
    },
    {
      name: 'assistant',
      traits: ['helpful', 'explanatory', 'code-focused'],
    },
  ],
  topics: [
    'debugging-async-code',
    'implementing-data-structures',
    'optimizing-algorithms',
    'understanding-libraries',
    'refactoring-legacy-code',
  ],
  turns: { min: 10, max: 30 },
});

const sessions = await synth.generate({ schema: sessionSchema, count: 1000 });
```

---

## Product Recommendations

Generate product data with embeddings for recommendation systems.

### E-commerce Products

```typescript
import { EmbeddingDatasetGenerator } from 'agentic-synth';

const products = await EmbeddingDatasetGenerator.create({
  domain: 'e-commerce-products',
  clusters: 100,  // Product categories
  itemsPerCluster: 500,
  vectorDim: 384,
  distribution: 'clustered',
});

await products.exportToRuvector({
  collection: 'product-embeddings',
  index: 'hnsw',
});
```

### Product Schema with Rich Metadata

```typescript
const productSchema = Schema.define({
  name: 'Product',
  type: 'object',
  properties: {
    id: { type: 'string', format: 'uuid' },
    name: { type: 'string' },
    description: { type: 'string' },
    category: {
      type: 'string',
      enum: ['electronics', 'clothing', 'home', 'sports', 'books'],
    },
    subcategory: { type: 'string' },
    price: { type: 'number', minimum: 5, maximum: 5000 },
    rating: { type: 'number', minimum: 1, maximum: 5 },
    reviewCount: { type: 'number', minimum: 0, maximum: 10000 },
    tags: {
      type: 'array',
      items: { type: 'string' },
      minItems: 3,
      maxItems: 10,
    },
    features: {
      type: 'array',
      items: { type: 'string' },
    },
    embedding: { type: 'embedding', dimensions: 384 },
  },
});

const products = await synth.generate({
  schema: productSchema,
  count: 100000,
  streaming: true,
});
```

### User-Item Interactions

```typescript
const interactionSchema = Schema.define({
  name: 'UserItemInteraction',
  type: 'object',
  properties: {
    userId: { type: 'string', format: 'uuid' },
    productId: { type: 'string', format: 'uuid' },
    interactionType: {
      type: 'string',
      enum: ['view', 'click', 'cart', 'purchase', 'review'],
    },
    timestamp: { type: 'date' },
    durationSeconds: { type: 'number', minimum: 0 },
    rating: { type: 'number', minimum: 1, maximum: 5 },
    reviewText: { type: 'string' },
    userContext: {
      type: 'object',
      properties: {
        device: { type: 'string', enum: ['mobile', 'desktop', 'tablet'] },
        location: { type: 'string' },
        sessionId: { type: 'string' },
      },
    },
  },
});

const interactions = await synth.generate({
  schema: interactionSchema,
  count: 1000000,
});
```

---

## Test Data Generation

Generate comprehensive test data including edge cases.

### Edge Cases

```typescript
import { EdgeCaseGenerator } from 'agentic-synth';

const testCases = await EdgeCaseGenerator.create({
  schema: userInputSchema,
  categories: [
    'boundary-values',
    'null-handling',
    'type-mismatches',
    'malicious-input',
    'unicode-edge-cases',
    'sql-injection',
    'xss-attacks',
    'buffer-overflow',
    'race-conditions',
  ],
  coverage: 'exhaustive',
});

await testCases.export({
  format: 'json',
  outputPath: './tests/edge-cases.json',
});
```

### API Test Scenarios

```typescript
const apiTestSchema = Schema.define({
  name: 'APITestScenario',
  type: 'object',
  properties: {
    name: { type: 'string' },
    method: { type: 'string', enum: ['GET', 'POST', 'PUT', 'DELETE'] },
    endpoint: { type: 'string' },
    headers: { type: 'object' },
    body: { type: 'object' },
    expectedStatus: { type: 'number' },
    expectedResponse: { type: 'object' },
    testType: {
      type: 'string',
      enum: ['happy-path', 'error-handling', 'edge-case', 'security'],
    },
  },
});

const apiTests = await synth.generate({
  schema: apiTestSchema,
  count: 1000,
});
```

### Load Testing Data

```typescript
const loadTestSchema = Schema.define({
  name: 'LoadTestScenario',
  type: 'object',
  properties: {
    userId: { type: 'string', format: 'uuid' },
    sessionId: { type: 'string', format: 'uuid' },
    requests: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          endpoint: { type: 'string' },
          method: { type: 'string' },
          payload: { type: 'object' },
          timestamp: { type: 'date' },
          expectedLatency: { type: 'number' },
        },
      },
      minItems: 10,
      maxItems: 100,
    },
  },
});

const loadTests = await synth.generate({
  schema: loadTestSchema,
  count: 10000,
});
```

---

## Multi-language Support

Generate localized content for global applications.

### Multi-language Conversations

```typescript
const languages = ['en', 'es', 'fr', 'de', 'zh', 'ja', 'pt', 'ru'];

for (const lang of languages) {
  const schema = Schema.conversation({
    domain: 'customer-support',
    personas: ['customer', 'agent'],
    topics: ['billing', 'technical', 'shipping'],
    turns: { min: 4, max: 12 },
    language: lang,
  });

  const conversations = await synth.generate({ schema, count: 1000 });
  await conversations.export({
    format: 'jsonl',
    outputPath: `./training/support-${lang}.jsonl`,
  });
}
```

### Localized Product Descriptions

```typescript
const localizedProductSchema = Schema.define({
  name: 'LocalizedProduct',
  type: 'object',
  properties: {
    productId: { type: 'string', format: 'uuid' },
    translations: {
      type: 'object',
      properties: {
        en: { type: 'object', properties: { name: { type: 'string' }, description: { type: 'string' } } },
        es: { type: 'object', properties: { name: { type: 'string' }, description: { type: 'string' } } },
        fr: { type: 'object', properties: { name: { type: 'string' }, description: { type: 'string' } } },
        de: { type: 'object', properties: { name: { type: 'string' }, description: { type: 'string' } } },
      },
    },
  },
});

const products = await synth.generate({
  schema: localizedProductSchema,
  count: 10000,
});
```

---

## Streaming Generation

Generate large datasets efficiently with streaming.

### Basic Streaming

```typescript
import { createWriteStream } from 'fs';
import { pipeline } from 'stream/promises';

const output = createWriteStream('./data.jsonl');

for await (const item of synth.generateStream({ schema, count: 100000 })) {
  output.write(JSON.stringify(item) + '\n');
}

output.end();
```

### Streaming with Transform Pipeline

```typescript
import { Transform } from 'stream';

const transformer = new Transform({
  objectMode: true,
  transform(item, encoding, callback) {
    // Process each item
    const processed = {
      ...item,
      processed: true,
      processedAt: new Date(),
    };
    callback(null, JSON.stringify(processed) + '\n');
  },
});

await pipeline(
  synth.generateStream({ schema, count: 1000000 }),
  transformer,
  createWriteStream('./processed-data.jsonl')
);
```

### Streaming to Database

```typescript
import { VectorDB } from 'ruvector';

const db = new VectorDB();
const batchSize = 1000;
let batch = [];

for await (const item of synth.generateStream({ schema, count: 100000 })) {
  batch.push(item);

  if (batch.length >= batchSize) {
    await db.insertBatch('collection', batch);
    batch = [];
  }
}

// Insert remaining items
if (batch.length > 0) {
  await db.insertBatch('collection', batch);
}
```

---

## Batch Processing

Process large-scale data generation efficiently.

### Parallel Batch Generation

```typescript
import { parallel } from 'agentic-synth/utils';

const schemas = [
  { name: 'users', schema: userSchema, count: 10000 },
  { name: 'products', schema: productSchema, count: 50000 },
  { name: 'reviews', schema: reviewSchema, count: 100000 },
  { name: 'interactions', schema: interactionSchema, count: 500000 },
];

await parallel(schemas, async (config) => {
  const data = await synth.generate({
    schema: config.schema,
    count: config.count,
  });

  await data.export({
    format: 'parquet',
    outputPath: `./data/${config.name}.parquet`,
  });
});
```

### Distributed Generation

```typescript
import { cluster } from 'cluster';
import { cpus } from 'os';

if (cluster.isPrimary) {
  const numWorkers = cpus().length;
  const countPerWorker = Math.ceil(totalCount / numWorkers);

  for (let i = 0; i < numWorkers; i++) {
    cluster.fork({ WORKER_ID: i, WORKER_COUNT: countPerWorker });
  }
} else {
  const workerId = parseInt(process.env.WORKER_ID);
  const count = parseInt(process.env.WORKER_COUNT);

  const data = await synth.generate({ schema, count });
  await data.export({
    format: 'jsonl',
    outputPath: `./data/part-${workerId}.jsonl`,
  });
}
```

---

## Custom Generators

Create custom generators for specialized use cases.

### Custom Generator Class

```typescript
import { BaseGenerator } from 'agentic-synth';

class MedicalReportGenerator extends BaseGenerator {
  async generate(count: number) {
    const reports = [];

    for (let i = 0; i < count; i++) {
      const report = await this.generateSingle();
      reports.push(report);
    }

    return reports;
  }

  private async generateSingle() {
    // Custom generation logic
    return {
      patientId: this.generateUUID(),
      reportDate: this.randomDate(),
      diagnosis: await this.llm.generate('medical diagnosis'),
      treatment: await this.llm.generate('treatment plan'),
      followUp: await this.llm.generate('follow-up instructions'),
    };
  }
}

const generator = new MedicalReportGenerator(synth);
const reports = await generator.generate(1000);
```

### Custom Transformer

```typescript
import { Transform } from 'agentic-synth';

class SentimentEnricher extends Transform {
  async transform(item: any) {
    const sentiment = await this.analyzeSentiment(item.text);
    return {
      ...item,
      sentiment,
      sentimentScore: sentiment.score,
    };
  }

  private async analyzeSentiment(text: string) {
    // Custom sentiment analysis
    return {
      label: 'positive',
      score: 0.92,
    };
  }
}

const enricher = new SentimentEnricher();
const enriched = await synth
  .generate({ schema, count: 10000 })
  .then((data) => enricher.transformAll(data));
```

---

## Advanced Schemas

Complex schema patterns for sophisticated data generation.

### Nested Object Schema

```typescript
const orderSchema = Schema.define({
  name: 'Order',
  type: 'object',
  properties: {
    orderId: { type: 'string', format: 'uuid' },
    customerId: { type: 'string', format: 'uuid' },
    orderDate: { type: 'date' },
    items: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          productId: { type: 'string', format: 'uuid' },
          productName: { type: 'string' },
          quantity: { type: 'number', minimum: 1, maximum: 10 },
          price: { type: 'number', minimum: 1 },
        },
      },
      minItems: 1,
      maxItems: 20,
    },
    shipping: {
      type: 'object',
      properties: {
        address: {
          type: 'object',
          properties: {
            street: { type: 'string' },
            city: { type: 'string' },
            state: { type: 'string' },
            zip: { type: 'string', pattern: '^\\d{5}$' },
            country: { type: 'string' },
          },
        },
        method: { type: 'string', enum: ['standard', 'express', 'overnight'] },
        cost: { type: 'number' },
      },
    },
    payment: {
      type: 'object',
      properties: {
        method: { type: 'string', enum: ['credit-card', 'paypal', 'crypto'] },
        status: { type: 'string', enum: ['pending', 'completed', 'failed'] },
        amount: { type: 'number' },
      },
    },
  },
});
```

### Time-Series Data

```typescript
const timeSeriesSchema = Schema.define({
  name: 'TimeSeriesData',
  type: 'object',
  properties: {
    sensorId: { type: 'string', format: 'uuid' },
    readings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          timestamp: { type: 'date' },
          value: { type: 'number' },
          unit: { type: 'string' },
          quality: { type: 'string', enum: ['good', 'fair', 'poor'] },
        },
      },
      minItems: 100,
      maxItems: 1000,
    },
  },
  constraints: [
    {
      type: 'temporal-consistency',
      field: 'readings.timestamp',
      ordering: 'ascending',
    },
  ],
});
```

---

## Performance Tips

1. **Use Streaming**: For datasets >10K, always use streaming to reduce memory
2. **Batch Operations**: Insert into databases in batches of 1000-5000
3. **Parallel Generation**: Use worker threads or cluster for large datasets
4. **Cache Embeddings**: Cache embedding model outputs to reduce API calls
5. **Quality Sampling**: Validate quality on samples, not entire datasets
6. **Compression**: Use Parquet format for columnar data storage
7. **Progressive Generation**: Generate and export in chunks

---

## More Examples

See the `/examples` directory for complete, runnable examples:

- `customer-support.ts` - Full customer support agent training
- `rag-training.ts` - RAG system with multi-hop questions
- `code-assistant.ts` - Code assistant memory generation
- `recommendations.ts` - E-commerce recommendation system
- `test-data.ts` - Comprehensive test data generation
- `i18n.ts` - Multi-language support
- `streaming.ts` - Large-scale streaming generation
- `batch.ts` - Distributed batch processing

---

## Support

- GitHub: https://github.com/ruvnet/ruvector
- Discord: https://discord.gg/ruvnet
- Email: support@ruv.io
