# Integration Guides

Complete integration guides for Agentic-Synth with popular tools and frameworks.

## Table of Contents

- [Ruvector Integration](#ruvector-integration)
- [AgenticDB Integration](#agenticdb-integration)
- [LangChain Integration](#langchain-integration)
- [Midstreamer Integration](#midstreamer-integration)
- [OpenAI Integration](#openai-integration)
- [Anthropic Claude Integration](#anthropic-claude-integration)
- [HuggingFace Integration](#huggingface-integration)
- [Vector Database Integration](#vector-database-integration)
- [Data Pipeline Integration](#data-pipeline-integration)

---

## Ruvector Integration

Seamless integration with Ruvector vector database for high-performance vector operations.

### Installation

```bash
npm install agentic-synth ruvector
```

### Basic Integration

```typescript
import { SynthEngine } from 'agentic-synth';
import { VectorDB } from 'ruvector';

// Initialize Ruvector
const db = new VectorDB({
  indexType: 'hnsw',
  dimensions: 384,
});

// Initialize SynthEngine with Ruvector
const synth = new SynthEngine({
  provider: 'openai',
  vectorDB: db,
});

// Generate and automatically insert with embeddings
await synth.generateAndInsert({
  schema: productSchema,
  count: 10000,
  collection: 'products',
  batchSize: 1000,
});
```

### Advanced Configuration

```typescript
import { RuvectorAdapter } from 'agentic-synth/integrations';

const adapter = new RuvectorAdapter(synth, db);

// Configure embedding generation
adapter.configure({
  embeddingModel: 'text-embedding-3-small',
  dimensions: 384,
  batchSize: 1000,
  normalize: true,
});

// Generate with custom indexing
await adapter.generateAndIndex({
  schema: documentSchema,
  count: 100000,
  collection: 'documents',
  indexConfig: {
    type: 'hnsw',
    M: 16,
    efConstruction: 200,
  },
});
```

### Streaming to Ruvector

```typescript
import { createVectorStream } from 'agentic-synth/integrations';

const stream = createVectorStream({
  synth,
  db,
  collection: 'embeddings',
  batchSize: 500,
});

for await (const item of synth.generateStream({ schema, count: 1000000 })) {
  await stream.write(item);
}

await stream.end();
```

### Augmenting Existing Collections

```typescript
// Augment existing Ruvector collection with synthetic variations
await adapter.augmentCollection({
  collection: 'user-queries',
  variationsPerItem: 5,
  augmentationType: 'paraphrase',
  preserveSemantics: true,
});
```

---

## AgenticDB Integration

Full compatibility with AgenticDB patterns for agent memory and skills.

### Installation

```bash
npm install agentic-synth agenticdb
```

### Agent Memory Generation

```typescript
import { AgenticDBAdapter } from 'agentic-synth/integrations';
import { AgenticDB } from 'agenticdb';

const agenticDB = new AgenticDB();
const adapter = new AgenticDBAdapter(synth);

// Generate episodic memory for agents
const memory = await adapter.generateMemory({
  agentId: 'assistant-1',
  memoryType: 'episodic',
  count: 5000,
  timeRange: {
    start: new Date('2024-01-01'),
    end: new Date('2024-12-31'),
  },
});

// Insert directly into AgenticDB
await agenticDB.memory.insertBatch(memory);
```

### Skill Library Generation

```typescript
// Generate synthetic skills for agent training
const skills = await adapter.generateSkills({
  domains: ['coding', 'research', 'communication', 'analysis'],
  skillsPerDomain: 100,
  includeExamples: true,
});

await agenticDB.skills.insertBatch(skills);
```

### Reflexion Memory

```typescript
// Generate reflexion-style memory for self-improving agents
const reflexionMemory = await adapter.generateReflexionMemory({
  agentId: 'learner-1',
  trajectories: 1000,
  includeVerdict: true,
  includeMemoryShort: true,
  includeMemoryLong: true,
});

await agenticDB.reflexion.insertBatch(reflexionMemory);
```

---

## LangChain Integration

Use Agentic-Synth with LangChain for agent training and RAG systems.

### Installation

```bash
npm install agentic-synth langchain
```

### Document Generation

```typescript
import { LangChainAdapter } from 'agentic-synth/integrations';
import { Document } from 'langchain/document';
import { VectorStore } from 'langchain/vectorstores';

const adapter = new LangChainAdapter(synth);

// Generate LangChain documents
const documents = await adapter.generateDocuments({
  schema: documentSchema,
  count: 10000,
  includeMetadata: true,
});

// Use with LangChain VectorStore
const vectorStore = await VectorStore.fromDocuments(
  documents,
  embeddings
);
```

### RAG Chain Training Data

```typescript
import { RetrievalQAChain } from 'langchain/chains';

// Generate QA pairs for RAG training
const qaPairs = await adapter.generateRAGTrainingData({
  documents: existingDocuments,
  questionsPerDoc: 10,
  questionTypes: ['factual', 'analytical', 'multi-hop'],
});

// Train RAG chain
const chain = RetrievalQAChain.fromLLM(llm, vectorStore.asRetriever());
```

### Agent Memory for LangChain Agents

```typescript
import { BufferMemory } from 'langchain/memory';

// Generate conversation history for memory
const conversationHistory = await adapter.generateConversationHistory({
  domain: 'customer-support',
  interactions: 1000,
  format: 'langchain-memory',
});

const memory = new BufferMemory({
  chatHistory: conversationHistory,
});
```

---

## Midstreamer Integration

Real-time streaming integration with Midstreamer for live data generation.

### Installation

```bash
npm install agentic-synth midstreamer
```

### Real-Time Data Streaming

```typescript
import { MidstreamerAdapter } from 'agentic-synth/integrations';
import { Midstreamer } from 'midstreamer';

const midstreamer = new Midstreamer({
  region: 'us-east-1',
  streamName: 'synthetic-data-stream',
});

const adapter = new MidstreamerAdapter(synth, midstreamer);

// Stream synthetic data in real-time
await adapter.streamGeneration({
  schema: eventSchema,
  ratePerSecond: 1000,
  duration: 3600, // 1 hour
});
```

### Event Stream Simulation

```typescript
// Simulate realistic event streams
await adapter.simulateEventStream({
  schema: userEventSchema,
  pattern: 'diurnal', // Daily activity pattern
  peakHours: [9, 12, 15, 20],
  baselineRate: 100,
  peakMultiplier: 5,
  duration: 86400, // 24 hours
});
```

### Burst Traffic Simulation

```typescript
// Simulate traffic spikes
await adapter.simulateBurstTraffic({
  schema: requestSchema,
  baselineRate: 100,
  bursts: [
    { start: 3600, duration: 600, multiplier: 50 }, // 50x spike
    { start: 7200, duration: 300, multiplier: 100 }, // 100x spike
  ],
});
```

---

## OpenAI Integration

Configure Agentic-Synth to use OpenAI models for generation.

### Installation

```bash
npm install agentic-synth openai
```

### Basic Configuration

```typescript
import { SynthEngine } from 'agentic-synth';

const synth = new SynthEngine({
  provider: 'openai',
  model: 'gpt-4',
  apiKey: process.env.OPENAI_API_KEY,
  temperature: 0.8,
  maxTokens: 2000,
});
```

### Using OpenAI Embeddings

```typescript
const synth = new SynthEngine({
  provider: 'openai',
  model: 'gpt-4',
  embeddingModel: 'text-embedding-3-small',
  embeddingDimensions: 384,
});

// Embeddings are automatically generated
const data = await synth.generate({
  schema: schemaWithEmbeddings,
  count: 10000,
});
```

### Function Calling for Structured Data

```typescript
import { OpenAIAdapter } from 'agentic-synth/integrations';

const adapter = new OpenAIAdapter(synth);

// Use OpenAI function calling for perfect structure compliance
const data = await adapter.generateWithFunctions({
  schema: complexSchema,
  count: 1000,
  functionDefinition: {
    name: 'generate_item',
    parameters: schemaToJSONSchema(complexSchema),
  },
});
```

---

## Anthropic Claude Integration

Use Anthropic Claude for high-quality synthetic data generation.

### Installation

```bash
npm install agentic-synth @anthropic-ai/sdk
```

### Configuration

```typescript
import { SynthEngine } from 'agentic-synth';

const synth = new SynthEngine({
  provider: 'anthropic',
  model: 'claude-3-opus-20240229',
  apiKey: process.env.ANTHROPIC_API_KEY,
  temperature: 0.8,
  maxTokens: 4000,
});
```

### Long-Form Content Generation

```typescript
// Claude excels at long-form, coherent content
const articles = await synth.generate({
  schema: Schema.define({
    name: 'Article',
    type: 'object',
    properties: {
      title: { type: 'string' },
      content: { type: 'string', minLength: 5000 }, // Long-form
      summary: { type: 'string' },
      keyPoints: { type: 'array', items: { type: 'string' } },
    },
  }),
  count: 100,
});
```

---

## HuggingFace Integration

Use open-source models from HuggingFace for cost-effective generation.

### Installation

```bash
npm install agentic-synth @huggingface/inference
```

### Configuration

```typescript
import { SynthEngine } from 'agentic-synth';

const synth = new SynthEngine({
  provider: 'huggingface',
  model: 'mistralai/Mistral-7B-Instruct-v0.2',
  apiKey: process.env.HF_API_KEY,
});
```

### Using Local Models

```typescript
const synth = new SynthEngine({
  provider: 'huggingface',
  model: 'local',
  modelPath: './models/llama-2-7b',
  deviceMap: 'auto',
});
```

---

## Vector Database Integration

Integration with popular vector databases beyond Ruvector.

### Pinecone

```typescript
import { PineconeAdapter } from 'agentic-synth/integrations';
import { PineconeClient } from '@pinecone-database/pinecone';

const pinecone = new PineconeClient();
await pinecone.init({ apiKey: process.env.PINECONE_API_KEY });

const adapter = new PineconeAdapter(synth, pinecone);
await adapter.generateAndUpsert({
  schema: embeddingSchema,
  count: 100000,
  index: 'my-index',
  namespace: 'synthetic-data',
});
```

### Weaviate

```typescript
import { WeaviateAdapter } from 'agentic-synth/integrations';
import weaviate from 'weaviate-ts-client';

const client = weaviate.client({ scheme: 'http', host: 'localhost:8080' });
const adapter = new WeaviateAdapter(synth, client);

await adapter.generateAndImport({
  schema: documentSchema,
  count: 50000,
  className: 'Document',
});
```

### Qdrant

```typescript
import { QdrantAdapter } from 'agentic-synth/integrations';
import { QdrantClient } from '@qdrant/js-client-rest';

const client = new QdrantClient({ url: 'http://localhost:6333' });
const adapter = new QdrantAdapter(synth, client);

await adapter.generateAndInsert({
  schema: vectorSchema,
  count: 200000,
  collection: 'synthetic-vectors',
});
```

---

## Data Pipeline Integration

Integrate with data engineering pipelines and ETL tools.

### Apache Airflow

```python
from airflow import DAG
from airflow.operators.python import PythonOperator
from datetime import datetime
import subprocess

def generate_synthetic_data():
    subprocess.run([
        'npx', 'agentic-synth', 'generate',
        '--schema', 'customer-support',
        '--count', '10000',
        '--output', '/data/synthetic.jsonl'
    ])

dag = DAG(
    'synthetic_data_generation',
    start_date=datetime(2024, 1, 1),
    schedule_interval='@daily'
)

generate_task = PythonOperator(
    task_id='generate',
    python_callable=generate_synthetic_data,
    dag=dag
)
```

### dbt (Data Build Tool)

```yaml
# dbt_project.yml
models:
  synthetic_data:
    materialized: table
    pre-hook:
      - "{{ run_agentic_synth('customer_events', 10000) }}"

# macros/agentic_synth.sql
{% macro run_agentic_synth(schema_name, count) %}
  {{ run_command('npx agentic-synth generate --schema ' ~ schema_name ~ ' --count ' ~ count) }}
{% endmacro %}
```

### Prefect

```python
from prefect import flow, task
import subprocess

@task
def generate_data(schema: str, count: int):
    result = subprocess.run([
        'npx', 'agentic-synth', 'generate',
        '--schema', schema,
        '--count', str(count),
        '--output', f'/data/{schema}.jsonl'
    ])
    return result.returncode == 0

@flow
def synthetic_data_pipeline():
    generate_data('users', 10000)
    generate_data('products', 50000)
    generate_data('interactions', 100000)

synthetic_data_pipeline()
```

### AWS Step Functions

```json
{
  "Comment": "Synthetic Data Generation Pipeline",
  "StartAt": "GenerateData",
  "States": {
    "GenerateData": {
      "Type": "Task",
      "Resource": "arn:aws:lambda:us-east-1:123456789012:function:agentic-synth-generator",
      "Parameters": {
        "schema": "customer-events",
        "count": 100000,
        "output": "s3://my-bucket/synthetic/"
      },
      "Next": "ValidateQuality"
    },
    "ValidateQuality": {
      "Type": "Task",
      "Resource": "arn:aws:lambda:us-east-1:123456789012:function:quality-validator",
      "End": true
    }
  }
}
```

---

## Custom Integration Template

Create custom integrations for your tools:

```typescript
import { BaseIntegration } from 'agentic-synth/integrations';

export class MyCustomIntegration extends BaseIntegration {
  constructor(
    private synth: SynthEngine,
    private customTool: any
  ) {
    super();
  }

  async generateAndExport(options: GenerateOptions) {
    // Generate data
    const data = await this.synth.generate(options);

    // Custom export logic
    for (const item of data.data) {
      await this.customTool.insert(item);
    }

    return {
      count: data.metadata.count,
      quality: data.metadata.quality,
    };
  }

  async streamToCustomTool(options: GenerateOptions) {
    for await (const item of this.synth.generateStream(options)) {
      await this.customTool.stream(item);
    }
  }
}
```

---

## Best Practices

1. **Connection Pooling**: Reuse database connections across generations
2. **Batch Operations**: Use batching for all database insertions (1000-5000 items)
3. **Error Handling**: Implement retry logic for API and database failures
4. **Rate Limiting**: Respect API rate limits with exponential backoff
5. **Monitoring**: Track generation metrics and quality scores
6. **Resource Management**: Close connections and cleanup resources properly
7. **Configuration**: Externalize configuration for different environments

---

## Troubleshooting

### Common Issues

**Issue**: Slow vector insertions
**Solution**: Increase batch size, use parallel workers

**Issue**: API rate limits
**Solution**: Reduce generation rate, implement exponential backoff

**Issue**: Memory errors with large datasets
**Solution**: Use streaming mode, process in smaller chunks

**Issue**: Low quality synthetic data
**Solution**: Tune temperature, validate schemas, increase quality threshold

---

## Examples Repository

Complete integration examples: https://github.com/ruvnet/ruvector/tree/main/packages/agentic-synth/examples/integrations

---

## Support

- GitHub Issues: https://github.com/ruvnet/ruvector/issues
- Discord: https://discord.gg/ruvnet
- Email: support@ruv.io
